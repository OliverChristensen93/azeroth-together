//! Nightfall's co-op messages: what a client tells its host and what the host tells its clients.
//! `together-net` carries them (under the game tag `"nightfall"`) without knowing them.

use serde::{Deserialize, Serialize};

use crate::{
    Event, HeroOutcome, HeroPose, Loadout, Mark, PlayerId, PlayerInput, RaidRoll, Snapshot,
};

/// Bumped whenever a message changes shape: a client and host must agree.
pub const PROTOCOL: u32 = 6;

/// The hero a player brings: their name and pick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeroPick {
    pub name: String,
    /// A class key from the content.
    pub class: String,
    pub race: u8,
    pub sex: u8,
    /// What they bring into the raid (spec §8.3: the host trusts it).
    pub loadout: Loadout,
}

impl HeroPick {
    /// Who this pick is, without what it brings: what a [`ClientMsg::Hello`] carries.
    pub fn who(&self) -> Who {
        Who {
            name: self.name.clone(),
            class: self.class.clone(),
            race: self.race,
            sex: self.sex,
        }
    }
}

/// The hero a player is, without what they bring. This is the shape the hero had in protocol 4, and
/// [`ClientMsg::Hello`] keeps it for good: a client of another protocol must still decode as a
/// hello, so the host can refuse it by version rather than drop it. What the hero brings follows
/// as the first [`ClientMsg::Pick`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Who {
    pub name: String,
    /// A class key from the content.
    pub class: String,
    pub race: u8,
    pub sex: u8,
}

impl Who {
    /// The pick of this hero bringing `loadout`.
    pub fn bringing(self, loadout: Loadout) -> HeroPick {
        HeroPick {
            name: self.name,
            class: self.class,
            race: self.race,
            sex: self.sex,
            loadout,
        }
    }
}

/// A player in the host's lobby.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub player: PlayerId,
    pub hero: HeroPick,
    /// Where their hero stands in the inn. The host's lobby leaves it empty: poses travel in
    /// [`ServerMsg::InnPoses`], so a loadout is not resent every time someone walks.
    pub pose: Option<HeroPose>,
    /// Ready at the raid board.
    pub ready: bool,
}

/// What a client tells the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// The first message: who is joining, and whether they play the same version with the same
    /// content (`content` is a hash of the content files). Its shape never changes (see [`Who`]),
    /// so that a host of any protocol can read the version of a client of any other.
    Hello {
        protocol: u32,
        content: u64,
        hero: Who,
    },
    /// A pick made in the lobby: the first one, sent as soon as the host welcomes the client, says
    /// what the hero brings; a new one follows whenever that changes.
    Pick(HeroPick),
    /// Where our hero stands in the inn.
    InnPose(HeroPose),
    /// Ready, or not, at the raid board.
    Ready(bool),
    /// This step's input, during a run: tagged with the number of the raid it was made in (the
    /// `raid` of the host's [`ServerMsg::Start`]), so an input still on its way from the last raid
    /// is never taken for this one's.
    Input { raid: u32, input: PlayerInput },
}

/// What the host tells a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Joined: the client plays as `you`.
    Welcome { you: PlayerId },
    /// Not joined, and why; the host closes the connection after it.
    Refused(String),
    /// The lobby as it stands, and the next raid as the host plans it (the map table's intel
    /// reads it).
    Lobby {
        players: Vec<LobbyPlayer>,
        raid: RaidRoll,
    },
    /// The run begins. `raid` numbers it (each start is the next number), and a client tags its
    /// inputs with it.
    Start { raid: u32 },
    /// The world after a step, as this player gets it: the enemies and telegraphs near their
    /// hero, which enemies their hero sees (spec §4.3), what each thinks of their hero (§5.5), and
    /// what happened since the last state this player got (§6).
    State {
        snapshot: Snapshot,
        seen: Vec<u32>,
        marks: Vec<Mark>,
        events: Vec<Event>,
    },
    /// Your raid is over: what you keep, sent once, as soon as it is settled.
    Outcome(HeroOutcome),
    /// Everyone is out or dead: every hero's outcome, for the end screen.
    RunOver { outcomes: Vec<HeroOutcome> },
    /// The host ended the run and went back to its menu.
    Ended,
    /// Where each player's hero stands in the inn (the host's included, as player 0), sent on its
    /// own, often, while they move; the [`ServerMsg::Lobby`] that carries their loadouts goes only
    /// when something they bring, their ready or the party changes.
    InnPoses(Vec<(PlayerId, HeroPose)>),
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
            hero: Who {
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
    fn a_hello_of_protocol_4_still_reads_so_that_it_is_refused_by_version() {
        // Protocol 4's hello, byte for byte: its hero had no loadout. A hello whose hero changed
        // shape would fail to decode and the host would drop the client without a word.
        #[derive(Serialize)]
        struct OldHero {
            name: String,
            class: String,
            race: u8,
            sex: u8,
        }
        #[derive(Serialize)]
        enum OldClientMsg {
            Hello {
                protocol: u32,
                content: u64,
                hero: OldHero,
            },
        }
        // Protocol 5 kept the same hello (its loadout moved to the first pick), so a v5 client
        // reads the same way; both are refused by version, not dropped.
        for version in [4, 5] {
            let old = OldClientMsg::Hello {
                protocol: version,
                content: 99,
                hero: OldHero {
                    name: "Valeera".into(),
                    class: "rogue".into(),
                    race: 1,
                    sex: 0,
                },
            };
            let bytes = postcard::to_stdvec(&old).unwrap();
            match postcard::from_bytes::<ClientMsg>(&bytes) {
                Ok(ClientMsg::Hello {
                    protocol,
                    content,
                    hero,
                }) => {
                    assert_eq!((protocol, content), (version, 99));
                    assert_eq!(hero.name, "Valeera");
                    assert_ne!(protocol, PROTOCOL, "the host refuses it for its version");
                }
                other => panic!("a protocol {version} hello must read as a hello: {other:?}"),
            }
        }
    }

    #[test]
    fn a_pick_is_a_hello_hero_bringing_a_loadout() {
        let pick = HeroPick {
            name: "Valeera".into(),
            class: "rogue".into(),
            race: 1,
            sex: 0,
            loadout: Loadout::default(),
        };
        assert_eq!(pick.who().bringing(Loadout::default()), pick);
    }

    #[test]
    fn a_refusal_and_a_welcome_keep_their_protocol_4_shape() {
        // The host's refusal of a protocol 4 client is read by that client: `Welcome` and
        // `Refused` must stay the first two variants, in this order, with these fields.
        // (Variants added after them, `Outcome` and `RunOver` before `Ended`, only moved `Ended`.)
        #[derive(Debug, PartialEq, Deserialize)]
        enum OldServerMsg {
            Welcome { you: PlayerId },
            Refused(String),
        }
        let refused = ServerMsg::Refused("The host has a different version of the game.".into());
        let bytes = postcard::to_stdvec(&refused).unwrap();
        assert_eq!(bytes[0], 1, "Refused is variant 1");
        assert_eq!(
            postcard::from_bytes::<OldServerMsg>(&bytes).unwrap(),
            OldServerMsg::Refused("The host has a different version of the game.".into())
        );
        let welcome = postcard::to_stdvec(&ServerMsg::Welcome { you: 2 }).unwrap();
        assert_eq!(welcome[0], 0, "Welcome is variant 0");
        assert_eq!(
            postcard::from_bytes::<OldServerMsg>(&welcome).unwrap(),
            OldServerMsg::Welcome { you: 2 }
        );
    }

    #[test]
    fn the_inns_lobby_survives_the_wire() {
        use crate::{ExitPlan, HeroPose, Loadout, RaidRoll, V2};
        let msg = ServerMsg::Lobby {
            players: vec![LobbyPlayer {
                player: 1,
                hero: HeroPick {
                    name: "Friend".into(),
                    class: "mage".into(),
                    race: 1,
                    sex: 1,
                    loadout: Loadout::default(),
                },
                pose: Some(HeroPose {
                    pos: V2::new(-9460.0, 20.0),
                    facing: V2::new(1.0, 0.0),
                    z: 57.6,
                }),
                ready: true,
            }],
            raid: RaidRoll {
                seed: 7,
                entry: 0,
                exits: vec![ExitPlan {
                    open: true,
                    opens_at: 1080.0,
                }],
            },
        };
        let bytes = postcard::to_stdvec(&msg).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), msg);
        let poses = ServerMsg::InnPoses(vec![(
            0,
            HeroPose {
                pos: V2::new(-9460.0, 20.0),
                facing: V2::new(1.0, 0.0),
                z: 57.6,
            },
        )]);
        let bytes = postcard::to_stdvec(&poses).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), poses);
        for up in [
            ClientMsg::Ready(true),
            ClientMsg::InnPose(HeroPose {
                pos: V2::new(-9460.0, 20.0),
                facing: V2::new(0.0, 1.0),
                z: 57.6,
            }),
        ] {
            let bytes = postcard::to_stdvec(&up).unwrap();
            assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).unwrap(), up);
        }
    }

    #[test]
    fn a_loadout_and_an_outcome_survive_the_wire() {
        use crate::{Fate, HeroOutcome, Inventory, Loadout, Recovery, Stack, V3};
        let inventory = Inventory {
            pack: vec![Some(Stack::found(3, 7)), None],
            ..Inventory::default()
        };
        let pick = ClientMsg::Pick(HeroPick {
            name: "Valeera".into(),
            class: "rogue".into(),
            race: 1,
            sex: 0,
            loadout: Loadout {
                inventory: inventory.clone(),
                recovery: Some(Recovery {
                    map: "duskwood".into(),
                    at: V3::new(1.0, 2.0, 3.0),
                    items: vec![Stack::new(2, 1)],
                }),
                pet: None,
            },
        });
        let bytes = postcard::to_stdvec(&pick).unwrap();
        assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).unwrap(), pick);
        let outcome = HeroOutcome {
            player: 1,
            name: "Jaina".into(),
            fate: Fate::Extracted {
                exit: "The bridge".into(),
            },
            time: 1200.0,
            kills: 4,
            carried: inventory,
            overflow: vec![Stack::new(5, 2)],
            recovered: Vec::new(),
            recovery: None,
            slain: Vec::new(),
        };
        let to_one = ServerMsg::Outcome(outcome.clone());
        let bytes = postcard::to_stdvec(&to_one).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), to_one);
        let all = ServerMsg::RunOver {
            outcomes: vec![outcome],
        };
        let bytes = postcard::to_stdvec(&all).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), all);
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
        // Our own full pack (every bag slot, worn gear, the safe slots) and an open sixteen-slot
        // container, twenty containers around us and every exit; four heroes in full gear.
        let items = w.content().items.len();
        let satchel = w.content().item("duskwood_satchel").unwrap();
        let mut pack = crate::Inventory::new(w.content());
        pack.bags = [Some(crate::Stack::new(satchel, 1)); crate::MAX_BAGS];
        pack.pack = vec![None; pack.pack_size(w.content())];
        for (i, s) in pack.pack.iter_mut().chain(pack.safe.iter_mut()).enumerate() {
            *s = Some(crate::Stack::found(i % items, 3));
        }
        for (i, s) in pack.equip.iter_mut().enumerate() {
            *s = Some(crate::Stack::new(i % items, 1));
        }
        let slots = pack.stacks().len();
        snapshot.pack = Some((0, pack));
        snapshot.loot.marks = (1..=20)
            .map(|id| crate::ContainerMark {
                id,
                holder: crate::Holder::Placed(0),
                at: V2::new(-10051.8, -792.5),
                z: Some(36.5),
                searched: false,
                empty: false,
                locked: false,
            })
            .collect();
        snapshot.loot.open = Some(crate::OpenView {
            id: 7,
            stacks: (0..16)
                .map(|i| Some(crate::Stack::found(i % items, 2)))
                .collect(),
            search: None,
        });
        snapshot.loot.exits = (0..4)
            .map(|exit| crate::ExitMark {
                exit,
                open: true,
                opens_in: Some(12.5),
            })
            .collect();
        for h in &mut snapshot.heroes {
            h.carry.worn = crate::EquipSlot::ALL.iter().map(|&s| (s, 3)).collect();
            h.carry.load = 30.5;
        }
        assert!(slots >= 60, "{slots} stacks in the pack");
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
        // Four heroes in full gear casting with their cooldowns and their senses, the twenty
        // enemies near one of them, all marked, casting, held and slowed, three telegraphs, two
        // busy steps of events, our own pack (all four bags worn, every slot full: 60 or more
        // stacks, 63 today), an open sixteen-slot container, twenty containers about us and four
        // exits (2963 bytes measured, a quarter to spare), 15 times a second, stay near 44 KB a
        // second.
        assert!(bytes < 3700, "{bytes} bytes");
    }
    #[test]
    fn a_player_without_a_hero_is_sent_the_heroes_and_the_exits_only() {
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
        let mut w = World::new(content.clone());
        w.begin_raid(crate::RaidRoll::standard(&content, 1))
            .unwrap();
        for p in 0..4 {
            w.add_hero(p, "rogue", "Twelve chars", 1, 0).unwrap();
        }
        // Enemies near the heroes (the content's own wait on a baked map this test has not got).
        let near = w.hero(0).unwrap().pos;
        for id in 1..=5 {
            w.enemies.push(Enemy {
                id,
                kind: 0,
                pos: near,
                z: 0.0,
                facing: V2::new(1.0, 0.0),
                hp: 900.0,
                max_hp: 900.0,
                speed: 0.0,
                asleep: false,
                fight: EnemyFight::default(),
            });
        }
        w.step();
        let state = |w: &World, p| ServerMsg::State {
            snapshot: w.snapshot_for(p),
            seen: w.seen_by(p),
            marks: w.marks_for(p),
            events: Vec::new(),
        };
        // Before the run has begun for a player (no hero yet, never had one) nothing changes: the
        // lobby and the menu are sent the whole world.
        let stranger = w.snapshot_for(9);
        assert_eq!(stranger.enemies.len(), w.snapshot().enemies.len());
        assert!(!w.snapshot().enemies.is_empty());
        assert!(!stranger.loot.marks.is_empty());
        // A hero gets out, and their client waits for the others.
        w.remove_hero(3);
        let waiting = w.snapshot_for(3);
        assert!(waiting.enemies.is_empty() && waiting.telegraphs.is_empty());
        assert!(waiting.loot.marks.is_empty() && waiting.loot.open.is_none());
        assert!(waiting.pack.is_none());
        assert_eq!(waiting.heroes.len(), 3, "the others, to watch");
        assert_eq!(waiting.loot.exits.len(), content.map.exits.len());
        assert!(w.seen_by(3).is_empty() && w.marks_for(3).is_empty());
        assert!(!w.in_reach(3, UnitRef::Enemy(1)));
        assert!(w.in_reach(3, UnitRef::Hero(0)));
        assert!(
            !w.in_reach(3, UnitRef::Hero(3)),
            "the hero who left is gone"
        );
        // And the others are still sent what they were.
        assert!(!w.snapshot_for(0).enemies.is_empty());
        let bytes = postcard::to_stdvec(&state(&w, 3)).unwrap().len();
        assert!(
            bytes < 600,
            "{bytes} bytes for a player waiting out the run"
        );
        // A hero who comes back (a rejoin) is a player in the raid again.
        w.add_hero(3, "rogue", "Twelve chars", 1, 0).unwrap();
        assert!(!w.snapshot_for(3).enemies.is_empty());
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
                pack: None,
                loot: Default::default(),
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
                pack: None,
                loot: Default::default(),
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
