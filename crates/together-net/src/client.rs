//! The client's side: one connection to a host, by address or by invite code, read and written
//! on its own threads.

use std::io::{BufReader, BufWriter};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use crate::invite::{self, Link};
use crate::wire;
use crate::{with_default_port, ClientMsg, ServerMsg};

/// How long to try a host before giving up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

/// What happened on the connection since the last poll.
#[derive(Debug)]
pub enum ClientEvent {
    Connected,
    Message(ServerMsg),
    /// The connection could not be made, or ended; the reason is for the player.
    Disconnected(String),
}

/// A connection to a host, being made or made. Dropping it closes the connection.
pub struct Client {
    events: Receiver<ClientEvent>,
    out: Sender<Vec<u8>>,
}

impl Client {
    /// Start connecting to `to`: an invite code, or an address (`host` or `host:port`).
    /// [`ClientEvent::Connected`] or [`ClientEvent::Disconnected`] tells how it went.
    pub fn connect(to: &str) -> Client {
        let to = to.to_owned();
        let (tx, events) = mpsc::channel();
        let (out, outgoing) = mpsc::channel::<Vec<u8>>();
        let _ = thread::Builder::new()
            .name("together-client".into())
            .spawn(move || run(to, tx, outgoing));
        Client { events, out }
    }

    /// Everything since the last poll, oldest first.
    pub fn poll(&self) -> Vec<ClientEvent> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, msg: &ClientMsg) {
        if let Ok(frame) = wire::encode(msg) {
            let _ = self.out.send(frame);
        }
    }
}

fn run(to: String, events: Sender<ClientEvent>, outgoing: Receiver<Vec<u8>>) {
    let link = match invite::parse_code(&to) {
        Some(host) => invite::dial(host),
        None => connect(&with_default_port(&to)),
    };
    let Link {
        reader,
        writer,
        close,
    } = match link {
        Ok(l) => l,
        Err(why) => {
            let _ = events.send(ClientEvent::Disconnected(why));
            return;
        }
    };
    let _ = events.send(ClientEvent::Connected);

    // Writer: until the game drops the client.
    let _ = thread::Builder::new()
        .name("together-client-write".into())
        .spawn(move || {
            let mut out = BufWriter::new(writer);
            for frame in outgoing {
                if wire::write_frame(&mut out, &frame).is_err() {
                    break;
                }
            }
            close();
        });

    let mut input = BufReader::new(reader);
    loop {
        match wire::read::<ServerMsg>(&mut input) {
            Ok(msg) => {
                if events.send(ClientEvent::Message(msg)).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = events.send(ClientEvent::Disconnected(
                    "the host closed the connection".into(),
                ));
                return;
            }
        }
    }
}

/// A TCP connection to `addr`, as the two halves the connection threads take.
fn connect(addr: &str) -> Result<Link, String> {
    let targets: Vec<_> = addr
        .to_socket_addrs()
        .map_err(|e| format!("cannot find {addr}: {e}"))?
        .collect();
    let mut last = format!("cannot find {addr}");
    for target in targets {
        match TcpStream::connect_timeout(&target, CONNECT_TIMEOUT) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                let opened = || -> std::io::Result<Link> {
                    let reader = stream.try_clone()?;
                    let shutdown = stream.try_clone()?;
                    Ok(Link {
                        reader: Box::new(reader),
                        writer: Box::new(stream),
                        close: Box::new(move || {
                            let _ = shutdown.shutdown(std::net::Shutdown::Both);
                        }),
                    })
                };
                return opened().map_err(|_| "could not open the connection".to_string());
            }
            Err(e) => last = format!("cannot reach {addr}: {e}"),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::{HeroPick, Host, HostEvent, PROTOCOL};

    /// Poll until `f` finds something, for up to two seconds.
    fn wait<T>(f: impl FnMut() -> Option<T>) -> T {
        wait_for(Duration::from_secs(2), f)
    }

    /// [`wait`], for up to `limit`.
    fn wait_for<T>(limit: Duration, mut f: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(start.elapsed() < limit, "timed out");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn pick() -> HeroPick {
        HeroPick {
            name: "Thrall".into(),
            class: "shaman".into(),
            race: 2,
            sex: 0,
            talents: Vec::new(),
        }
    }

    #[test]
    fn a_client_joins_talks_and_leaves() {
        let host = Host::listen(0).unwrap();
        let client = Client::connect(&format!("127.0.0.1:{}", host.local_addr().port()));
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Connected))
        });
        let conn = wait(|| {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Joined(c, _) => Some(c),
                _ => None,
            })
        });

        let hello = ClientMsg::Hello {
            protocol: PROTOCOL,
            content: 42,
            hero: pick(),
        };
        client.send(&hello);
        let got = wait(|| {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Message(c, m) if c == conn => Some(m),
                _ => None,
            })
        });
        assert_eq!(got, hello);

        host.send(conn, &ServerMsg::Welcome { you: 1 });
        host.broadcast(&ServerMsg::Start {
            zone: "elwynn".into(),
        });
        let mut seen = Vec::new();
        wait(|| {
            for e in client.poll() {
                if let ClientEvent::Message(m) = e {
                    seen.push(m);
                }
            }
            (seen.len() == 2).then_some(())
        });
        assert_eq!(seen[0], ServerMsg::Welcome { you: 1 });

        drop(client);
        wait(|| {
            host.poll()
                .into_iter()
                .find(|e| matches!(e, HostEvent::Left(c) if *c == conn))
        });
        assert_eq!(host.connections(), 0);
    }

    #[test]
    fn nobody_listening_is_a_disconnect_with_a_reason() {
        // Bind and drop, so the port is very likely closed.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let client = Client::connect(&format!("127.0.0.1:{port}"));
        // Windows retries a refused localhost connection for a couple of seconds.
        let why = wait_for(CONNECT_TIMEOUT + Duration::from_secs(2), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Disconnected(why) => Some(why),
                _ => None,
            })
        });
        assert!(why.contains("cannot reach"), "{why}");
    }

    /// The whole invite path over the internet: number 0's address lookup and relays. Needs a
    /// network, so it runs only when asked: `cargo test -p together-net -- --ignored`.
    #[test]
    #[ignore]
    fn a_client_joins_by_invite_code() {
        let host = Host::listen(0).unwrap();
        let key = crate::new_host_key();
        host.open_invites(key);
        let code = wait_for(Duration::from_secs(30), || match host.invite() {
            crate::Invite::Ready(code) => Some(code),
            crate::Invite::Failed(why) => panic!("{why}"),
            _ => None,
        });
        assert_eq!(code, crate::code_for(&key));
        let client = Client::connect(&code);
        let mut why = None;
        wait_for(Duration::from_secs(40), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Connected => Some(()),
                ClientEvent::Disconnected(w) => {
                    why = Some(w);
                    Some(())
                }
                _ => None,
            })
        });
        assert_eq!(why, None);
        let hello = ClientMsg::Hello {
            protocol: PROTOCOL,
            content: 7,
            hero: pick(),
        };
        client.send(&hello);
        let (conn, got) = wait_for(Duration::from_secs(20), || {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Message(c, m) => Some((c, m)),
                _ => None,
            })
        });
        assert_eq!(got, hello);
        host.send(conn, &ServerMsg::Welcome { you: 1 });
        let welcome = wait_for(Duration::from_secs(20), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Message(m) => Some(m),
                _ => None,
            })
        });
        assert_eq!(welcome, ServerMsg::Welcome { you: 1 });
    }

    #[test]
    fn the_host_going_away_disconnects_the_client() {
        let host = Host::listen(0).unwrap();
        let client = Client::connect(&format!("127.0.0.1:{}", host.local_addr().port()));
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Connected))
        });
        wait(|| {
            host.poll()
                .into_iter()
                .find(|e| matches!(e, HostEvent::Joined(..)))
        });
        drop(host);
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Disconnected(_)))
        });
    }
}
