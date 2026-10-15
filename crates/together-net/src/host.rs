//! The host's side: a listener that takes players in, and per player a reader and a writer thread.

use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::wire;
use crate::{ClientMsg, ServerMsg};

/// One connected client, numbered from 1 in the order they arrived.
pub type ConnId = u32;

/// What happened on the host's sockets since the last poll.
#[derive(Debug)]
pub enum HostEvent {
    /// Someone connected; their [`ClientMsg::Hello`] comes next.
    Joined(ConnId, SocketAddr),
    Message(ConnId, ClientMsg),
    /// The connection closed, or failed.
    Left(ConnId),
}

type Writers = Arc<Mutex<HashMap<ConnId, Sender<Arc<Vec<u8>>>>>>;

/// A listening host. Dropping it stops listening and closes every connection.
pub struct Host {
    events: Receiver<HostEvent>,
    writers: Writers,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
}

impl Host {
    /// Listen on every interface at `port` (0 picks a free one).
    pub fn listen(port: u16) -> io::Result<Host> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (tx, events) = mpsc::channel();
        let writers: Writers = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        {
            let (writers, stop) = (writers.clone(), stop.clone());
            thread::Builder::new()
                .name("together-host-accept".into())
                .spawn(move || accept_loop(listener, tx, writers, stop))?;
        }
        Ok(Host {
            events,
            writers,
            addr,
            stop,
        })
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
            if let Some(w) = self.writers.lock().unwrap().get(&to) {
                let _ = w.send(Arc::new(frame));
            }
        }
    }

    /// The same message to everyone, encoded once.
    pub fn broadcast(&self, msg: &ServerMsg) {
        if let Ok(frame) = wire::encode(msg) {
            let frame = Arc::new(frame);
            for w in self.writers.lock().unwrap().values() {
                let _ = w.send(frame.clone());
            }
        }
    }

    /// Close one connection (its reader then reports [`HostEvent::Left`]).
    pub fn kick(&self, conn: ConnId) {
        self.writers.lock().unwrap().remove(&conn);
    }

    /// How many are connected.
    pub fn connections(&self) -> usize {
        self.writers.lock().unwrap().len()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.writers.lock().unwrap().clear();
    }
}

fn accept_loop(
    listener: TcpListener,
    events: Sender<HostEvent>,
    writers: Writers,
    stop: Arc<AtomicBool>,
) {
    let mut next: ConnId = 1;
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let conn = next;
                next += 1;
                if start_connection(conn, stream, &events, &writers).is_ok() {
                    let _ = events.send(HostEvent::Joined(conn, peer));
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
    stream: TcpStream,
    events: &Sender<HostEvent>,
    writers: &Writers,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    let reader = stream.try_clone()?;
    let (tx, rx) = mpsc::channel::<Arc<Vec<u8>>>();
    writers.lock().unwrap().insert(conn, tx);

    // Writer: frames out until the host drops its sender (kick, or the host goes).
    let shutdown = stream.try_clone()?;
    thread::Builder::new()
        .name(format!("together-host-write-{conn}"))
        .spawn(move || {
            let mut out = BufWriter::new(stream);
            for frame in rx {
                if wire::write_frame(&mut out, &frame).is_err() {
                    break;
                }
            }
            let _ = shutdown.shutdown(std::net::Shutdown::Both);
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
