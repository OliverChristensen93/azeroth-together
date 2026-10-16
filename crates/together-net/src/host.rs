//! The host's side: a TCP listener (and, opened on request, an invite-code endpoint) that takes
//! players in, and per player a reader and a writer thread.

use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter, Read};
use std::marker::PhantomData;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::handshake::{self, Greeting, Verdict};
use crate::invite::{self, Link};
use crate::wire;
use crate::GameTag;

/// One connected client, numbered from 1 in the order they arrived.
pub type ConnId = u32;

/// What happened on the host's sockets since the last poll.
#[derive(Debug)]
pub enum HostEvent<Up> {
    /// Someone of our game joined (from this address, or by invite code).
    Joined(ConnId, String),
    Message(ConnId, Up),
    /// The connection closed, or failed.
    Left(ConnId),
}

/// A connection's outgoing frames, and how many of them its writer has not yet put on the wire.
struct Writer {
    tx: Sender<Arc<Vec<u8>>>,
    queued: Arc<AtomicUsize>,
}

impl Writer {
    fn send(&self, frame: Arc<Vec<u8>>) {
        self.queued.fetch_add(1, Ordering::Relaxed);
        if self.tx.send(frame).is_err() {
            self.queued.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

type Writers = Arc<Mutex<HashMap<ConnId, Writer>>>;

/// Where a host's invite code stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invite {
    /// Not asked for.
    Off,
    /// The endpoint is opening and finding a relay.
    Opening,
    /// Friends can join with this code.
    Ready(String),
    /// It could not open, and why: friends on the same network can still join by address.
    Failed(String),
}

/// What the accept paths share: where events go, the writers, the next connection's number and
/// the game this host plays.
struct Intake<Up> {
    events: Sender<HostEvent<Up>>,
    writers: Writers,
    next: Arc<AtomicU32>,
    game: GameTag,
}

// Written out: a derived `Clone` would ask `Up: Clone`, which a sender does not need.
impl<Up> Clone for Intake<Up> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            writers: self.writers.clone(),
            next: self.next.clone(),
            game: self.game,
        }
    }
}

impl<Up: DeserializeOwned + Send + 'static> Intake<Up> {
    /// Number a new connection and start its thread, which greets it and announces it.
    fn admit(&self, link: Link, from: String) {
        let conn = self.next.fetch_add(1, Ordering::Relaxed);
        let intake = self.clone();
        let _ = thread::Builder::new()
            .name(format!("together-host-read-{conn}"))
            .spawn(move || serve(conn, link, from, intake));
    }
}

/// A listening host for a game whose clients send `Up` and are sent `Down`. Dropping it stops
/// listening and closes every connection.
pub struct Host<Up, Down> {
    events: Receiver<HostEvent<Up>>,
    intake: Intake<Up>,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    invite: Arc<Mutex<Invite>>,
    /// The invite endpoint, once open, to close with the host.
    endpoint: Arc<Mutex<Option<iroh::Endpoint>>>,
    _down: PhantomData<fn(&Down)>,
}

impl<Up: DeserializeOwned + Send + 'static, Down: Serialize> Host<Up, Down> {
    /// Listen on every interface at `port` (0 picks a free one), for clients playing `game`.
    pub fn listen(port: u16, game: GameTag) -> io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (tx, events) = mpsc::channel();
        let intake = Intake {
            events: tx,
            writers: Arc::default(),
            next: Arc::new(AtomicU32::new(1)),
            game,
        };
        let stop = Arc::new(AtomicBool::new(false));
        {
            let (intake, stop) = (intake.clone(), stop.clone());
            thread::Builder::new()
                .name("together-host-accept".into())
                .spawn(move || accept_loop(listener, intake, stop))?;
        }
        Ok(Host {
            events,
            intake,
            addr,
            stop,
            invite: Arc::new(Mutex::new(Invite::Off)),
            endpoint: Arc::default(),
            _down: PhantomData,
        })
    }

    /// Open the invite-code endpoint for `key` ([`crate::new_host_key`]); the same key gives the
    /// same code every time. [`Host::invite`] tells when the code is ready.
    pub fn open_invites(&self, key: [u8; 32]) {
        *self.invite.lock().unwrap() = Invite::Opening;
        let (intake, state, slot, stop) = (
            self.intake.clone(),
            self.invite.clone(),
            self.endpoint.clone(),
            self.stop.clone(),
        );
        invite::runtime().spawn(async move {
            let listener = match invite::Listener::open(key).await {
                Ok(l) => l,
                Err(why) => {
                    *state.lock().unwrap() = Invite::Failed(why);
                    return;
                }
            };
            if stop.load(Ordering::Relaxed) {
                listener.endpoint().close().await;
                return;
            }
            *slot.lock().unwrap() = Some(listener.endpoint());
            *state.lock().unwrap() = Invite::Ready(invite::code_for(&key));
            while let Some((link, who)) = listener.next().await {
                intake.admit(link, format!("invite {who}"));
            }
        });
    }

    /// Where the invite code stands.
    pub fn invite(&self) -> Invite {
        self.invite.lock().unwrap().clone()
    }

    /// The address the host listens on (its port is the one to share).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Everything since the last poll, oldest first.
    pub fn poll(&self) -> Vec<HostEvent<Up>> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, to: ConnId, msg: &Down) {
        if let Ok(frame) = wire::encode(msg) {
            if let Some(w) = self.intake.writers.lock().unwrap().get(&to) {
                w.send(Arc::new(frame));
            }
        }
    }

    /// The same message to everyone, encoded once.
    pub fn broadcast(&self, msg: &Down) {
        if let Ok(frame) = wire::encode(msg) {
            let frame = Arc::new(frame);
            for w in self.intake.writers.lock().unwrap().values() {
                w.send(frame.clone());
            }
        }
    }

    /// Close one connection (its reader then reports [`HostEvent::Left`]).
    pub fn kick(&self, conn: ConnId) {
        self.intake.writers.lock().unwrap().remove(&conn);
    }

    /// Frames sent to `conn` that its connection has not yet written: more than a couple means the
    /// line is slower than the game, and what is queued is already stale.
    pub fn backlog(&self, conn: ConnId) -> usize {
        self.intake
            .writers
            .lock()
            .unwrap()
            .get(&conn)
            .map_or(0, |w| w.queued.load(Ordering::Relaxed))
    }

    /// How many are connected.
    pub fn connections(&self) -> usize {
        self.intake.writers.lock().unwrap().len()
    }
}

impl<Up, Down> Drop for Host<Up, Down> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.intake.writers.lock().unwrap().clear();
        if let Some(endpoint) = self.endpoint.lock().unwrap().take() {
            invite::runtime().spawn(async move { endpoint.close().await });
        }
    }
}

/// A TCP connection as the two halves the connection threads take, on either end.
///
/// Its close sends what is queued and our end-of-stream first, then waits (up to
/// [`crate::LINGER`]) for the peer to hang up, reading and dropping whatever it still sends:
/// closing a socket with unread data resets it, and a reset can drop a refusal not yet read.
pub(crate) fn tcp_link(stream: TcpStream) -> io::Result<Link> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    let reader = stream.try_clone()?;
    let closing = stream.try_clone()?;
    Ok(Link {
        reader: Box::new(reader),
        writer: Box::new(stream),
        close: Box::new(move || {
            let _ = closing.shutdown(std::net::Shutdown::Write);
            wait_for_hang_up(&closing);
            let _ = closing.shutdown(std::net::Shutdown::Both);
        }),
    })
}

/// Read and drop until the peer closes its end, or [`crate::LINGER`] passes.
fn wait_for_hang_up(mut stream: &TcpStream) {
    let deadline = Instant::now() + crate::LINGER;
    let mut sink = [0u8; 1024];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || stream.set_read_timeout(Some(left)).is_err() {
            return;
        }
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

fn accept_loop<Up: DeserializeOwned + Send + 'static>(
    listener: TcpListener,
    intake: Intake<Up>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                if let Ok(link) = tcp_link(stream) {
                    intake.admit(link, peer.to_string());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// One connection's life, on its own thread: the handshake, then a writer thread and the messages
/// in. A connection that never greets, greets wrongly or plays another game never joins: no
/// [`HostEvent`] is sent for it and nothing is broadcast to it.
fn serve<Up: DeserializeOwned + Send + 'static>(
    conn: ConnId,
    link: Link,
    from: String,
    intake: Intake<Up>,
) {
    let Link {
        reader,
        writer,
        close,
    } = link;
    let mut input = BufReader::new(reader);
    let mut out = BufWriter::new(writer);
    let verdict = match wire::read_capped::<Greeting>(&mut input, handshake::MAX_FRAME) {
        Ok(greeting) => handshake::judge(&greeting, intake.game),
        Err(_) => {
            drop(out);
            close();
            return;
        }
    };
    let told = wire::encode(&verdict).and_then(|frame| wire::write_frame(&mut out, &frame));
    if verdict != Verdict::Welcome || told.is_err() {
        // The writer goes first, so the close flushes the refusal to the peer before it closes.
        drop(out);
        close();
        return;
    }

    let (tx, rx) = mpsc::channel::<Arc<Vec<u8>>>();
    let queued = Arc::new(AtomicUsize::new(0));
    intake.writers.lock().unwrap().insert(
        conn,
        Writer {
            tx,
            queued: queued.clone(),
        },
    );
    // Writer: frames out until the host drops its sender (kick, or the host goes).
    let writer = thread::Builder::new()
        .name(format!("together-host-write-{conn}"))
        .spawn(move || {
            for frame in rx {
                let sent = wire::write_frame(&mut out, &frame);
                queued.fetch_sub(1, Ordering::Relaxed);
                if sent.is_err() {
                    break;
                }
            }
            // A kick's last frames (a refusal) are flushed by the close.
            drop(out);
            close();
        });
    if writer.is_err() {
        intake.writers.lock().unwrap().remove(&conn);
        return;
    }
    let _ = intake.events.send(HostEvent::Joined(conn, from));
    while let Ok(msg) = wire::read::<Up>(&mut input) {
        // A kicked connection stays open while its close flushes; what it says meanwhile is
        // not the game's to hear.
        if !intake.writers.lock().unwrap().contains_key(&conn) {
            break;
        }
        if intake.events.send(HostEvent::Message(conn, msg)).is_err() {
            break;
        }
    }
    intake.writers.lock().unwrap().remove(&conn);
    let _ = intake.events.send(HostEvent::Left(conn));
}
