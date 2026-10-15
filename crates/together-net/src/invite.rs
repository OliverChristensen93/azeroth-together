//! Invite codes: a host anywhere on the internet, reached by a code instead of an address.
//!
//! A hosting game opens an [iroh](https://iroh.computer) endpoint beside its TCP listener. The
//! endpoint is named by a public key, and that key, written out, is the invite code. A joining
//! game dials the key: iroh finds the host through number 0's address lookup, punches through both
//! home routers for a direct QUIC connection, and falls back to number 0's public relays when a
//! router will not let it. Every connection starts outbound, so nobody forwards a port.
//!
//! The QUIC stream is wrapped in blocking `Read` and `Write` halves ([`Link`]), so the same reader
//! and writer threads serve it as serve a TCP socket.

use std::io::{self, Read, Write};
use std::sync::OnceLock;
use std::time::Duration;

use iroh::endpoint::{presets, Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use tokio::runtime::{Handle, Runtime};

/// The protocol name both ends of an invite connection must agree on.
pub(crate) const ALPN: &[u8] = b"azeroth-together/1";

/// How long a joining game tries a code before giving up.
const DIAL_TIMEOUT: Duration = Duration::from_secs(25);

/// How long a host waits to reach a relay before showing its code anyway: direct connections
/// may still work without one.
const ONLINE_TIMEOUT: Duration = Duration::from_secs(15);

/// The runtime every invite endpoint and stream runs on: two workers, started on first use.
pub(crate) fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("together-invite")
            .enable_all()
            .build()
            .expect("the invite runtime starts")
    })
}

/// A fresh host key: the secret behind an invite code. Keep it to keep the code.
pub fn new_host_key() -> [u8; 32] {
    SecretKey::generate().to_bytes()
}

/// The invite code a host with `key` is reached by.
pub fn code_for(key: &[u8; 32]) -> String {
    SecretKey::from_bytes(key).public().to_z32()
}

/// The host an invite code names, if `text` is one (spaces and line breaks around it allowed).
pub fn parse_code(text: &str) -> Option<EndpointId> {
    let code: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    EndpointId::from_z32(&code.to_ascii_lowercase()).ok()
}

/// One connection's two directions as blocking halves, and how to close it.
pub(crate) struct Link {
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
    pub close: Box<dyn FnOnce() + Send>,
}

/// A QUIC stream's receiving half, read by blocking on the runtime from a plain thread.
struct BlockingRecv {
    rt: Handle,
    recv: RecvStream,
}

impl Read for BlockingRecv {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.rt.block_on(self.recv.read(buf)) {
            Ok(Some(n)) => Ok(n),
            Ok(None) => Ok(0),
            Err(e) => Err(io::Error::other(e)),
        }
    }
}

/// A QUIC stream's sending half, written by blocking on the runtime from a plain thread.
struct BlockingSend {
    rt: Handle,
    send: SendStream,
}

impl Write for BlockingSend {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.rt
            .block_on(self.send.write(buf))
            .map_err(io::Error::other)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The halves of one stream on `conn`; closing ends the connection (and, for a joining game,
/// its endpoint).
fn link(conn: Connection, send: SendStream, recv: RecvStream, endpoint: Option<Endpoint>) -> Link {
    let rt = runtime().handle().clone();
    Link {
        reader: Box::new(BlockingRecv {
            rt: rt.clone(),
            recv,
        }),
        writer: Box::new(BlockingSend {
            rt: rt.clone(),
            send,
        }),
        close: Box::new(move || {
            conn.close(0u32.into(), b"bye");
            if let Some(endpoint) = endpoint {
                rt.spawn(async move { endpoint.close().await });
            }
        }),
    }
}

/// A host's invite endpoint, once it is up: its code, and each connection that arrives.
pub(crate) struct Listener {
    endpoint: Endpoint,
}

impl Listener {
    /// Open the endpoint for `key` and wait (a while) to reach a relay.
    pub(crate) async fn open(key: [u8; 32]) -> Result<Listener, String> {
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(SecretKey::from_bytes(&key))
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| format!("could not open the invite endpoint: {e}"))?;
        let _ = tokio::time::timeout(ONLINE_TIMEOUT, endpoint.online()).await;
        Ok(Listener { endpoint })
    }

    /// The next joining game's connection and its first stream; `None` once the endpoint closed.
    /// A connection that fails on the way in is skipped.
    pub(crate) async fn next(&self) -> Option<(Link, String)> {
        loop {
            let incoming = self.endpoint.accept().await?;
            let Ok(conn) = incoming.await else { continue };
            let Ok((send, recv)) = conn.accept_bi().await else {
                continue;
            };
            let who = conn.remote_id().fmt_short().to_string();
            return Some((link(conn, send, recv, None), who));
        }
    }

    pub(crate) fn endpoint(&self) -> Endpoint {
        self.endpoint.clone()
    }
}

/// Dial the host `id` names and open the stream the game talks over.
pub(crate) fn dial(id: EndpointId) -> Result<Link, String> {
    runtime().block_on(async move {
        let endpoint = Endpoint::bind(presets::N0)
            .await
            .map_err(|e| format!("could not start networking: {e}"))?;
        let attempt = async {
            let conn = endpoint
                .connect(EndpointAddr::from(id), ALPN)
                .await
                .map_err(|e| format!("could not reach the host: {e}"))?;
            let (send, recv) = conn
                .open_bi()
                .await
                .map_err(|e| format!("the host did not answer: {e}"))?;
            Ok::<_, String>((conn, send, recv))
        };
        match tokio::time::timeout(DIAL_TIMEOUT, attempt).await {
            Ok(Ok((conn, send, recv))) => Ok(link(conn, send, recv, Some(endpoint))),
            Ok(Err(why)) => {
                endpoint.close().await;
                Err(why)
            }
            Err(_) => {
                endpoint.close().await;
                Err("no host answered that invite code (is it hosting?)".into())
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_names_its_key_and_survives_spacing_and_case() {
        let key = new_host_key();
        let code = code_for(&key);
        let id = SecretKey::from_bytes(&key).public();
        assert_eq!(parse_code(&code), Some(id));
        assert_eq!(
            parse_code(&format!("  {}\n", code.to_uppercase())),
            Some(id)
        );
    }

    #[test]
    fn an_address_is_not_a_code() {
        assert_eq!(parse_code("192.168.0.4:7777"), None);
        assert_eq!(parse_code("example.org"), None);
        assert_eq!(parse_code(""), None);
    }
}
