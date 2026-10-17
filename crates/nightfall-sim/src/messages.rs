//! Nightfall's co-op messages: what a client tells its host and what the host tells its clients.
//! `together-net` carries them (under the game tag `"nightfall"`) without knowing them.

use serde::{Deserialize, Serialize};

use crate::{Event, Mark, PlayerId, PlayerInput, Snapshot};

/// Bumped whenever a message changes shape: a client and host must agree.
pub const PROTOCOL: u32 = 4;

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
    /// The first message: who is joining, and whether they play the same version with the same
    /// content (`content` is a hash of the content files).
    Hello {
        protocol: u32,
        content: u64,
        hero: HeroPick,
    },
    /// A new pick, made in the lobby.
    Pick(HeroPick),
    /// This step's input, during a run.
    Input(PlayerInput),
}

/// What the host tells a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Joined: the client plays as `you`.
    Welcome { you: PlayerId },
    /// Not joined, and why; the host closes the connection after it.
    Refused(String),
    /// The lobby as it stands.
    Lobby { players: Vec<LobbyPlayer> },
    /// The run begins.
    Start,
    /// The world after a step, as this player gets it: the enemies and telegraphs near their
    /// hero, which enemies their hero sees (spec §4.3), what each thinks of their hero (§5.5), and
    /// what happened since the last state this player got (§6).
    State {
        snapshot: Snapshot,
        seen: Vec<u32>,
        marks: Vec<Mark>,
        events: Vec<Event>,
    },
    /// The host ended the run and went back to its menu.
    Ended,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{Alert, Content, ContentSources, Enemy, Mark, World, V2};

    #[test]
    fn messages_survive_the_wire() {
        let hello = ClientMsg::Hello {
            protocol: PROTOCOL,
            content: 7,
            hero: HeroPick {
                name: "Valeera".into(),
                class: "rogue".into(),
                race: 1,
                sex: 0,
            },
        };
        let bytes = postcard::to_stdvec(&hello).unwrap();
        assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).unwrap(), hello);
    }

    #[test]
    fn a_full_partys_state_is_small() {
        let content = Arc::new(
            Content::from_ron(ContentSources {
                classes: include_str!("../../../content/nightfall/classes.ron"),
                map: include_str!("../../../content/nightfall/map.ron"),
                rules: include_str!("../../../content/nightfall/rules.ron"),
                enemies: include_str!("../../../content/nightfall/enemies.ron"),
                spawns: include_str!("../../../content/nightfall/spawns.ron"),
                abilities: include_str!("../../../content/nightfall/abilities.ron"),
            })
            .unwrap(),
        );
        let mut w = World::new(content);
        for p in 0..4 {
            w.add_hero(
                p,
                if p % 2 == 0 { "rogue" } else { "mage" },
                "Twelve chars",
                1,
                0,
            )
            .unwrap();
        }
        let mut snapshot = w.snapshot();
        snapshot.enemies = (1..=20)
            .map(|id| Enemy {
                id,
                kind: 0,
                pos: V2::new(-10051.8, -792.5),
                z: 36.5,
                facing: V2::new(1.0, 0.0),
                hp: 900.0,
                max_hp: 900.0,
                speed: 2.5,
                asleep: false,
                fight: Default::default(),
            })
            .collect();
        let msg = ServerMsg::State {
            snapshot,
            seen: (1..=20).collect(),
            marks: (1..=20)
                .map(|enemy| Mark {
                    enemy,
                    alert: Alert::Suspicious,
                })
                .collect(),
            events: (0..12)
                .map(|i| crate::Event::Damage {
                    to: crate::UnitRef::Enemy(i),
                    by: Some(crate::UnitRef::Hero(0)),
                    amount: 130.0,
                    crit: false,
                    ambush: false,
                })
                .collect(),
        };
        let bytes = postcard::to_stdvec(&msg).unwrap().len();
        // Four heroes with their senses and their fight, the twenty enemies near one of them, all
        // marked, and a busy step's events, 15 times a second, stay near 35 KB a second.
        assert!(bytes < 2400, "{bytes} bytes");
    }

    #[test]
    fn a_state_with_events_survives_the_wire() {
        use crate::{Event, Refusal, Snapshot, Stop, UnitRef, V3};
        let msg = ServerMsg::State {
            snapshot: Snapshot {
                elapsed: 1.0,
                heroes: Vec::new(),
                enemies: Vec::new(),
                telegraphs: Vec::new(),
            },
            seen: vec![3],
            marks: Vec::new(),
            events: vec![
                Event::CastGo {
                    by: UnitRef::Hero(0),
                    ability: 2,
                    hits: vec![UnitRef::Enemy(3)],
                    at: Some(V3::new(1.0, 2.0, 3.0)),
                },
                Event::CastStop {
                    by: UnitRef::Enemy(3),
                    ability: 21,
                    why: Stop::TargetLost,
                },
                Event::Refused {
                    player: 1,
                    why: Refusal::OutOfRange,
                },
            ],
        };
        let bytes = postcard::to_stdvec(&msg).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), msg);
    }

    #[test]
    fn a_state_with_marks_survives_the_wire() {
        let msg = ServerMsg::State {
            snapshot: crate::Snapshot {
                elapsed: 1.5,
                heroes: Vec::new(),
                enemies: Vec::new(),
                telegraphs: Vec::new(),
            },
            seen: vec![3, 4],
            marks: vec![Mark {
                enemy: 3,
                alert: Alert::Alerted,
            }],
            events: Vec::new(),
        };
        let bytes = postcard::to_stdvec(&msg).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), msg);
    }
}
