//! Azeroth Together's co-op wire, shared by every game the launcher plays.
//!
//! A game's host runs its world and every client mirrors it. Each game brings its own messages:
//! `Up`, what a client sends, and `Down`, what the host sends; this crate carries them and knows
//! nothing of either game. Before any game message, the two ends exchange a handshake
//! ([`handshake`]): a host refuses a client of another game with a reason, since Survivors and
//! Nightfall share the port and the invite code.
//!
//! Messages travel over TCP, or over QUIC for a game joined by invite code ([`invite`]), as a
//! little-endian `u32` length and a `postcard` body. [`Host`] and [`Client`] run the sockets on
//! their own threads; the game polls them once a frame and never blocks.

mod client;
mod handshake;
mod host;
mod invite;
mod wire;

pub use client::{Client, ClientEvent};
pub use host::{ConnId, Host, HostEvent, Invite};
pub use invite::{code_for, new_host_key, parse_code};

/// Which game a host or client plays, the same on both ends: `"survivors"`, `"nightfall"`.
pub type GameTag = &'static str;

/// How long closing a connection waits for what was written to reach the peer, or for the peer
/// to hang up, before closing anyway: long enough for a refusal to be read, short enough that a
/// silent peer holds nothing up.
pub(crate) const LINGER: std::time::Duration = std::time::Duration::from_secs(2);

/// The port a host listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7777;

/// A stable hash of some text (FNV-1a), for comparing content files across machines.
pub fn content_hash(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// `host`, `host:port` or `[v6]:port`, with [`DEFAULT_PORT`] when no port is given.
pub fn with_default_port(addr: &str) -> String {
    let addr = addr.trim();
    let has_port = match addr.rfind(':') {
        Some(i) => addr.starts_with('[') && addr[..i].ends_with(']') || !addr[..i].contains(':'),
        None => false,
    };
    if has_port {
        addr.to_owned()
    } else {
        format!("{addr}:{DEFAULT_PORT}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_added_only_when_missing() {
        assert_eq!(with_default_port("192.168.1.5"), "192.168.1.5:7777");
        assert_eq!(with_default_port(" 10.0.0.2:9000 "), "10.0.0.2:9000");
        assert_eq!(with_default_port("example.org"), "example.org:7777");
        assert_eq!(with_default_port("[::1]:7000"), "[::1]:7000");
    }

    #[test]
    fn content_hash_depends_on_every_part_and_its_boundary() {
        assert_eq!(content_hash(&["a", "b"]), content_hash(&["a", "b"]));
        assert_ne!(content_hash(&["a", "b"]), content_hash(&["ab", ""]));
        assert_ne!(content_hash(&["a", "b"]), content_hash(&["a", "c"]));
    }
}
