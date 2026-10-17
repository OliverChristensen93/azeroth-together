//! Survivors' co-op messages: what a client tells its host and what the host tells its clients.
//! `together-net` carries them without knowing them.

use serde::{Deserialize, Serialize};

use crate::{Event, PlayerId, PlayerInput, Snapshot};

/// Bumped whenever a message changes shape: a client and host must agree.
pub const PROTOCOL: u32 = 8;

/// The hero a player brings: their name and pick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroPick {
    pub name: String,
    /// A class key from the content.
    pub class: String,
    pub race: u8,
    pub sex: u8,
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
