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
    use crate::{
        Alert, Casting, Content, ContentSources, ControlKind, Cooldown, Enemy, EnemyFight, Event,
        Mark, Shape, Telegraph, UnitRef, World, V2, V3,
    };

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
                items: include_str!("../../../content/nightfall/items.ron"),
                loot: include_str!("../../../content/nightfall/loot.ron"),
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
        // A busy fight: every hero casting with a handful of cooldowns running, combo points,
        // and a dodge spent.
        for h in &mut w.heroes {
            h.fight.casting = Some(Casting {
                ability: 3,
                left: 1.25,
                total: 2.0,
                channel: false,
                target: Some(UnitRef::Enemy(7)),
            });
            h.fight.cooldowns = (0..6)
                .map(|ability| Cooldown {
                    ability,
                    left: 4.5 + f32::from(ability),
                })
                .collect();
            h.fight.combo = 3;
            h.fight.combo_on = Some(7);
            h.fight.gcd = 0.75;
            h.fight.in_combat = true;
            h.fight.attacking = Some(7);
            h.fight.sprint = 3.5;
            h.fight.dodge.have = 1;
            h.fight.dodge.next_in = 5.5;
            h.fight.drinks = 4;
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
                fight: EnemyFight {
                    control: Some(ControlKind::Stun),
                    slowed: true,
                    casting: Some(Casting {
                        ability: 1,
                        left: 1.5,
                        total: 2.5,
                        channel: false,
                        target: Some(UnitRef::Hero(0)),
                    }),
                    auras: vec![116, 1833, 8632],
                },
            })
            .collect();
        snapshot.telegraphs = (1..=3)
            .map(|id| Telegraph {
                id,
                enemy: id,
                ability: 2,
                shape: Shape::Cone {
                    length: 8.0,
                    angle: 120.0,
                },
                at: V3::new(-10051.8, -792.5, 36.5),
                facing: V2::new(0.0, 1.0),
                left: 0.8,
                total: 1.2,
            })
            .collect();
        // Two steps of events (a state goes out every other step): blows, casts, an evaded
        // stomp and a dodge.
        let step = |n: u32| {
            let mut events: Vec<Event> = (0..4)
                .map(|i| Event::Damage {
                    to: UnitRef::Enemy(n * 4 + i),
                    by: Some(UnitRef::Hero(i as u8)),
                    amount: 130.5,
                    crit: i == 0,
                    ambush: false,
                })
                .collect();
            events.extend([
                Event::Damage {
                    to: UnitRef::Hero(1),
                    by: Some(UnitRef::Enemy(n)),
                    amount: 61.25,
                    crit: false,
                    ambush: false,
                },
                Event::Swing {
                    by: UnitRef::Enemy(n),
                    to: UnitRef::Hero(1),
                },
                Event::CastStart {
                    by: UnitRef::Enemy(n + 3),
                    ability: 4,
                    target: Some(UnitRef::Hero(2)),
                    secs: 2.5,
                },
                Event::CastGo {
                    by: UnitRef::Enemy(n + 5),
                    ability: 5,
                    hits: vec![UnitRef::Hero(0), UnitRef::Hero(2), UnitRef::Hero(3)],
                    at: Some(V3::new(-10051.8, -792.5, 36.5)),
                },
                Event::Evaded {
                    player: 3,
                    enemy: n + 5,
                },
                Event::Dodged { player: 3 },
            ]);
            events
        };
        let events: Vec<Event> = (0..2).flat_map(step).collect();
        let msg = ServerMsg::State {
            snapshot,
            seen: (1..=20).collect(),
            marks: (1..=20)
                .map(|enemy| Mark {
                    enemy,
                    alert: Alert::Suspicious,
                })
                .collect(),
            events,
        };
        let bytes = postcard::to_stdvec(&msg).unwrap().len();
        // Four heroes casting with their cooldowns and their senses, the twenty enemies near one
        // of them, all marked, casting, held and slowed, three telegraphs and two busy steps of
        // events (2113 bytes measured, a quarter to spare), 15 times a second, stay near 32 KB a
        // second.
        assert!(bytes < 2650, "{bytes} bytes");
    }

    #[test]
    fn a_state_with_events_survives_the_wire() {
        use crate::{Refusal, Snapshot, Stop};
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
