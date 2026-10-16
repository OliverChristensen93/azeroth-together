//! Azeroth Together's co-op wire.
//!
//! The host runs the game ([`together_sim::World`]) and every client mirrors it: after each step the
//! host sends a [`ServerMsg::State`] (the world's [`Snapshot`] and the step's events), and each
//! client sends its [`ClientMsg::Input`] (where its own controller put its hero, and its card
//! choices). Before a run, the host's lobby collects who joins with which hero.
//!
//! Messages travel over TCP, or over QUIC for a game joined by invite code ([`invite`]), as a
//! little-endian `u32` length and a `postcard` body. [`Host`] and
//! [`Client`] run the sockets on their own threads; the game polls them once a frame and never
//! blocks.

mod client;
mod host;
mod invite;
mod wire;

use serde::{Deserialize, Serialize};
use together_sim::{Event, GearPiece, PlayerId, PlayerInput, Snapshot};

pub use client::{Client, ClientEvent};
pub use host::{ConnId, Host, HostEvent, Invite};
pub use invite::{code_for, new_host_key, parse_code};

/// Bumped whenever a message changes shape: a client and host must agree.
pub const PROTOCOL: u32 = 11;

/// The port a host listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7777;

/// The hero a player brings: their name and pick, and the equipment it wears into the run
/// (from the player's own stash; the host's world checks it and starts it dormant).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroPick {
    pub name: String,
    /// A class key from the content.
    pub class: String,
    pub race: u8,
    pub sex: u8,
    /// The talent web nodes this hero takes for its class, by index into the shared
    /// `talents.ron` (the content hash makes sure both sides read the same web). The host keeps
    /// only what connects to the class's start, up to the web's top number of points.
    pub talents: Vec<u16>,
    #[serde(default)]
    pub gear: Vec<GearPiece>,
}

/// A player in the host's lobby.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub player: PlayerId,
    pub hero: HeroPick,
}

/// What a client tells the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// The first message: who is joining, and whether they play the same game. `content` is a
    /// hash of the content files, which must match the host's.
    Hello {
        protocol: u32,
        content: u64,
        hero: HeroPick,
    },
    /// A new pick, made in the lobby.
    Pick(HeroPick),
    /// This frame's input, during a run.
    Input(PlayerInput),
    /// Asks for a [`ServerMsg::Pong`] with the same stamp, to time the round trip.
    Ping(u32),
}

/// What the host tells a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Joined: the client plays as `you`.
    Welcome { you: PlayerId },
    /// Not joined, and why; the host closes the connection after it.
    Refused(String),
    /// The lobby as it stands: the zone picked and everyone in it.
    Lobby {
        zone: String,
        players: Vec<LobbyPlayer>,
    },
    /// The run begins in `zone`.
    Start { zone: String },
    /// One step of the run: the world after it, and what happened in it.
    State {
        snapshot: Snapshot,
        events: Vec<Event>,
    },
    /// The host ended the run and went back to its menu.
    Ended,
    /// The answer to a [`ClientMsg::Ping`].
    Pong(u32),
}

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
