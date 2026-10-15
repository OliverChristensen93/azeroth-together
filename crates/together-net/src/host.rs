//! The host's side: a TCP listener (and, opened on request, an invite-code endpoint) that takes
//! players in, and per player a reader and a writer thread.

use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::invite::{self, Link};
use crate::wire;
use crate::{ClientMsg, ServerMsg};

/// One connected client, numbered from 1 in the order they arrived.
pub type ConnId = u32;

/// What happened on the host's sockets since the last poll.
#[derive(Debug)]
pub enum HostEvent {
    /// Someone connected (from this address, or by invite code); their [`ClientMsg::Hello`]
    /// comes next.
    Joined(ConnId, String),
    Message(ConnId, ClientMsg),
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

/// What the accept paths share: where events go, the writers, and the next connection's number.
#[derive(Clone)]
struct Intake {
    events: Sender<HostEvent>,
    writers: Writers,
    next: Arc<AtomicU32>,
}

impl Intake {
    /// Number a new connection, start its threads and announce it.
    fn admit(&self, link: Link, from: String) {
        let conn = self.next.fetch_add(1, Ordering::Relaxed);
        if start_connection(conn, link, &self.events, &self.writers).is_ok() {
            let _ = self.events.send(HostEvent::Joined(conn, from));
        }
    }
}

/// A listening host. Dropping it stops listening and closes every connection.
pub struct Host {
    events: Receiver<HostEvent>,
    intake: Intake,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    invite: Arc<Mutex<Invite>>,
    /// The invite endpoint, once open, to close with the host.
    endpoint: Arc<Mutex<Option<iroh::Endpoint>>>,
}

impl Host {
    /// Listen on every interface at `port` (0 picks a free one).
    pub fn listen(port: u16) -> io::Result<Host> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (tx, events) = mpsc::channel();
        let intake = Intake {
            events: tx,
            writers: Arc::default(),
            next: Arc::new(AtomicU32::new(1)),
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
    pub fn poll(&self) -> Vec<HostEvent> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, to: ConnId, msg: &ServerMsg) {
        if let Ok(frame) = wire::encode(msg) {
            if let Some(w) = self.intake.writers.lock().unwrap().get(&to) {
                w.send(Arc::new(frame));
            }
        }
    }

    /// The same message to everyone, encoded once.
    pub fn broadcast(&self, msg: &ServerMsg) {
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

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.intake.writers.lock().unwrap().clear();
        if let Some(endpoint) = self.endpoint.lock().unwrap().take() {
            invite::runtime().spawn(async move { endpoint.close().await });
        }
    }
}

/// A TCP connection as the two halves the connection threads take.
fn tcp_link(stream: TcpStream) -> io::Result<Link> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    let reader = stream.try_clone()?;
    let shutdown = stream.try_clone()?;
    Ok(Link {
        reader: Box::new(reader),
        writer: Box::new(stream),
        close: Box::new(move || {
            let _ = shutdown.shutdown(std::net::Shutdown::Both);
        }),
    })
}

fn accept_loop(listener: TcpListener, intake: Intake, stop: Arc<AtomicBool>) {
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

fn start_connection(
    conn: ConnId,
    link: Link,
    events: &Sender<HostEvent>,
    writers: &Writers,
) -> io::Result<()> {
    let Link {
        reader,
        writer,
        close,
    } = link;
    let (tx, rx) = mpsc::channel::<Arc<Vec<u8>>>();
    let queued = Arc::new(AtomicUsize::new(0));
    writers.lock().unwrap().insert(
        conn,
        Writer {
            tx,
            queued: queued.clone(),
        },
    );

    // Writer: frames out until the host drops its sender (kick, or the host goes).
    thread::Builder::new()
        .name(format!("together-host-write-{conn}"))
        .spawn(move || {
            let mut out = BufWriter::new(writer);
            for frame in rx {
                let sent = wire::write_frame(&mut out, &frame);
                queued.fetch_sub(1, Ordering::Relaxed);
                if sent.is_err() {
                    break;
                }
            }
            close();
        })?;

    // Reader: messages in until the stream ends.
    let (events, writers) = (events.clone(), writers.clone());
    thread::Builder::new()
        .name(format!("together-host-read-{conn}"))
        .spawn(move || {
            let mut input = BufReader::new(reader);
            while let Ok(msg) = wire::read::<ClientMsg>(&mut input) {
                if events.send(HostEvent::Message(conn, msg)).is_err() {
                    break;
                }
            }
            writers.lock().unwrap().remove(&conn);
            let _ = events.send(HostEvent::Left(conn));
        })?;
    Ok(())
}
