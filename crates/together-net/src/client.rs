//! The client's side: one connection to a host, by address or by invite code, read and written
//! on its own threads. It greets the host with its game first; a host of another game answers
//! with a refusal, which arrives as [`ClientEvent::Disconnected`] with the reason.

use std::io::{BufReader, BufWriter};
use std::marker::PhantomData;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::handshake::{self, Greeting, Verdict, HANDSHAKE};
use crate::invite::{self, Link};
use crate::{wire, with_default_port, GameTag};

/// How long to try a host before giving up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

/// What happened on the connection since the last poll.
#[derive(Debug)]
pub enum ClientEvent<Down> {
    /// The host plays our game and took us in.
    Connected,
    Message(Down),
    /// The connection could not be made, was refused, or ended; the reason is for the player.
    Disconnected(String),
}

/// A connection to a host of a game whose clients send `Up` and are sent `Down`, being made or
/// made. Dropping it closes the connection.
pub struct Client<Up, Down> {
    events: Receiver<ClientEvent<Down>>,
    out: Sender<Vec<u8>>,
    _up: PhantomData<fn(&Up)>,
}

impl<Up: Serialize, Down: DeserializeOwned + Send + 'static> Client<Up, Down> {
    /// Start connecting to `to`, an invite code or an address (`host` or `host:port`), as a
    /// player of `game`. [`ClientEvent::Connected`] or [`ClientEvent::Disconnected`] tells how it
    /// went.
    pub fn connect(to: &str, game: GameTag) -> Self {
        let to = to.to_owned();
        let (tx, events) = mpsc::channel();
        let (out, outgoing) = mpsc::channel::<Vec<u8>>();
        let _ = thread::Builder::new()
            .name("together-client".into())
            .spawn(move || run::<Down>(to, game, tx, outgoing));
        Client {
            events,
            out,
            _up: PhantomData,
        }
    }

    /// Everything since the last poll, oldest first.
    pub fn poll(&self) -> Vec<ClientEvent<Down>> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, msg: &Up) {
        if let Ok(frame) = wire::encode(msg) {
            let _ = self.out.send(frame);
        }
    }
}

fn run<Down: DeserializeOwned>(
    to: String,
    game: GameTag,
    events: Sender<ClientEvent<Down>>,
    outgoing: Receiver<Vec<u8>>,
) {
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
    let mut input = BufReader::new(reader);
    let mut out = BufWriter::new(writer);
    let greeting = Greeting {
        handshake: HANDSHAKE,
        game: game.to_owned(),
    };
    if wire::encode(&greeting)
        .and_then(|frame| wire::write_frame(&mut out, &frame))
        .is_err()
    {
        let _ = events.send(ClientEvent::Disconnected("could not greet the host".into()));
        drop(out);
        close();
        return;
    }
    match wire::read_capped::<Verdict>(&mut input, handshake::MAX_FRAME) {
        Ok(Verdict::Welcome) => {}
        Ok(refused) => {
            let _ = events.send(ClientEvent::Disconnected(handshake::refusal(
                &refused, game,
            )));
            drop(out);
            close();
            return;
        }
        Err(_) => {
            let _ = events.send(ClientEvent::Disconnected(
                "the host closed the connection".into(),
            ));
            drop(out);
            close();
            return;
        }
    }
    let _ = events.send(ClientEvent::Connected);

    // Writer: until the game drops the client.
    let _ = thread::Builder::new()
        .name("together-client-write".into())
        .spawn(move || {
            for frame in outgoing {
                if wire::write_frame(&mut out, &frame).is_err() {
                    break;
                }
            }
            // The game's last messages are flushed by the close.
            drop(out);
            close();
        });

    loop {
        match wire::read::<Down>(&mut input) {
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
                return crate::host::tcp_link(stream)
                    .map_err(|_| "could not open the connection".to_string());
            }
            Err(e) => last = format!("cannot reach {addr}: {e}"),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::Instant;

    use serde::Deserialize;

    use super::*;
    use crate::{Host, HostEvent};

    const GAME: GameTag = "test";

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Up {
        Hello(String),
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Down {
        Welcome(u8),
        Start(String),
    }

    type TestHost = Host<Up, Down>;
    type TestClient = Client<Up, Down>;

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

    fn local(host: &TestHost) -> String {
        format!("127.0.0.1:{}", host.local_addr().port())
    }

    #[test]
    fn a_client_joins_talks_and_leaves() {
        let host = TestHost::listen(0, GAME).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
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

        let hello = Up::Hello("Thrall".into());
        client.send(&hello);
        let got = wait(|| {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Message(c, m) if c == conn => Some(m),
                _ => None,
            })
        });
        assert_eq!(got, hello);

        host.send(conn, &Down::Welcome(1));
        host.broadcast(&Down::Start("duskwood".into()));
        let mut seen = Vec::new();
        wait(|| {
            for e in client.poll() {
                if let ClientEvent::Message(m) = e {
                    seen.push(m);
                }
            }
            (seen.len() == 2).then_some(())
        });
        assert_eq!(seen[0], Down::Welcome(1));

        drop(client);
        wait(|| {
            host.poll()
                .into_iter()
                .find(|e| matches!(e, HostEvent::Left(c) if *c == conn))
        });
        assert_eq!(host.connections(), 0);
    }

    #[test]
    fn a_client_of_another_game_is_told_which_game_the_host_plays() {
        let host = TestHost::listen(0, "survivors").unwrap();
        let client = TestClient::connect(&local(&host), "nightfall");
        let why = wait(|| {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Disconnected(why) => Some(why),
                ClientEvent::Connected => panic!("connected to another game's host"),
                ClientEvent::Message(_) => None,
            })
        });
        assert_eq!(why, "That host is playing Survivors, not Nightfall.");
        thread::sleep(Duration::from_millis(100));
        assert!(
            host.poll().is_empty(),
            "the host announced a refused client"
        );
        assert_eq!(host.connections(), 0);
    }

    /// A game's refusal ("party full") is a message and then a kick: the message must arrive
    /// before the connection ends.
    #[test]
    fn a_message_sent_just_before_a_kick_arrives() {
        let host = TestHost::listen(0, GAME).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
        let conn = wait(|| {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Joined(c, _) => Some(c),
                _ => None,
            })
        });
        host.send(conn, &Down::Start("the party is full".into()));
        host.kick(conn);
        let mut seen = Vec::new();
        let why = wait(|| {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Message(m) => {
                    seen.push(m);
                    None
                }
                ClientEvent::Disconnected(why) => Some(why),
                ClientEvent::Connected => None,
            })
        });
        assert_eq!(seen, [Down::Start("the party is full".into())], "{why}");
    }

    #[test]
    fn a_connection_that_never_greets_never_joins_and_blocks_nobody() {
        let host = TestHost::listen(0, GAME).unwrap();
        // Not a greeting: a length prefix far over the limit, as a stray program might send.
        let mut stray = TcpStream::connect(local(&host)).unwrap();
        stray.write_all(&u32::MAX.to_le_bytes()).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Connected))
        });
        let joined: Vec<crate::ConnId> = wait(|| {
            let j: Vec<_> = host
                .poll()
                .into_iter()
                .filter_map(|e| match e {
                    HostEvent::Joined(c, _) => Some(c),
                    _ => None,
                })
                .collect();
            (!j.is_empty()).then_some(j)
        });
        assert_eq!(joined.len(), 1, "only the real client joined");
        assert_eq!(host.connections(), 1);
    }

    /// A greeting is a few bytes; one announcing a kilobyte is not read, and its connection is
    /// closed without joining.
    #[test]
    fn an_oversized_greeting_is_refused() {
        use std::io::Read;

        let host = TestHost::listen(0, GAME).unwrap();
        let mut stray = TcpStream::connect(local(&host)).unwrap();
        stray.write_all(&1024u32.to_le_bytes()).unwrap();
        stray.write_all(&[0u8; 1024]).unwrap();
        stray
            .set_read_timeout(Some(crate::LINGER + Duration::from_secs(2)))
            .unwrap();
        let mut reply = Vec::new();
        let closed = stray.read_to_end(&mut reply);
        assert!(
            matches!(closed, Ok(_))
                || closed.as_ref().is_err_and(|e| {
                    e.kind() == std::io::ErrorKind::ConnectionReset
                        || e.kind() == std::io::ErrorKind::ConnectionAborted
                }),
            "the host kept the connection open: {closed:?}"
        );
        assert!(reply.is_empty(), "the host answered an oversized greeting");
        assert!(host.poll().is_empty());
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
        let client = TestClient::connect(&format!("127.0.0.1:{port}"), GAME);
        // Windows retries a refused localhost connection for a couple of seconds.
        let why = wait_for(CONNECT_TIMEOUT + Duration::from_secs(2), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Disconnected(why) => Some(why),
                _ => None,
            })
        });
        assert!(why.contains("cannot reach"), "{why}");
    }

    #[test]
    fn the_host_going_away_disconnects_the_client() {
        let host = TestHost::listen(0, GAME).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
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

    /// The whole invite path over the internet: number 0's address lookup and relays. Needs a
    /// network, so it runs only when asked: `cargo test -p together-net -- --ignored`.
    #[test]
    #[ignore]
    fn a_client_joins_by_invite_code() {
        let host = TestHost::listen(0, GAME).unwrap();
        let key = crate::new_host_key();
        host.open_invites(key);
        let code = wait_for(Duration::from_secs(30), || match host.invite() {
            crate::Invite::Ready(code) => Some(code),
            crate::Invite::Failed(why) => panic!("{why}"),
            _ => None,
        });
        assert_eq!(code, crate::code_for(&key));
        let client = TestClient::connect(&code, GAME);
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
        let hello = Up::Hello("Jaina".into());
        client.send(&hello);
        let (conn, got) = wait_for(Duration::from_secs(20), || {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Message(c, m) => Some((c, m)),
                _ => None,
            })
        });
        assert_eq!(got, hello);
        host.send(conn, &Down::Welcome(1));
        let welcome = wait_for(Duration::from_secs(20), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Message(m) => Some(m),
                _ => None,
            })
        });
        assert_eq!(welcome, Down::Welcome(1));

        // A refusal ("party full") is a message and a kick; over QUIC a plain close would drop it.
        host.send(conn, &Down::Start("the party is full".into()));
        host.kick(conn);
        let refused = wait_for(Duration::from_secs(20), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Message(m) => Some(m),
                _ => None,
            })
        });
        assert_eq!(refused, Down::Start("the party is full".into()));

        // A player of another game on this host's code is told which game the host plays.
        let other = TestClient::connect(&code, "nightfall");
        let why = wait_for(Duration::from_secs(40), || {
            other.poll().into_iter().find_map(|e| match e {
                ClientEvent::Disconnected(why) => Some(why),
                ClientEvent::Connected => panic!("connected to another game's host"),
                ClientEvent::Message(_) => None,
            })
        });
        assert_eq!(why, "That host is playing Test, not Nightfall.");
    }
}
