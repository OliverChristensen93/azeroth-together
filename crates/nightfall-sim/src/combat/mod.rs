//! The fight (spec §6, §5.4, §8.1's falling): heroes' presses and casts, what abilities do where
//! they land, enemies held and hurt over time, heroes falling, revived or dead, and enemies
//! striking back with plain swings and telegraphed blows that a dodge makes miss. The host's: a
//! client's mirror takes each hero's and enemy's fight from the snapshot, the telegraphs beside
//! them, and what happened from the [`Event`]s that travel with it.

mod effects;
mod falling;
pub(crate) mod heroes;
mod hold;
mod rules;
mod strikes;
#[cfg(test)]
mod tests;

pub(crate) use effects::MANA;
pub use rules::Charges;
pub(crate) use rules::{allowance, in_range, stands_right, within_arc, Dice};

use serde::{Deserialize, Serialize};

use crate::{ClassDef, CombatRules, Content, ControlKind, Enemy, PlayerId, Shape, World, V2, V3};

/// The seed of the fight's dice: every run rolls the same with the same inputs.
const DICE_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// A unit an ability or a blow is aimed at, or comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitRef {
    Hero(PlayerId),
    Enemy(u32),
}

/// How a hero stands (spec §8.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Life {
    #[default]
    Up,
    /// At 0 health and crawling: `left` seconds from death, `revive` seconds into a revive.
    Downed { left: f32, revive: f32 },
    /// The body stays where it fell.
    Dead,
}

/// A cast or channel under way, as every client sees it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Casting {
    /// Index into the content's abilities.
    pub ability: u16,
    pub left: f32,
    pub total: f32,
    pub channel: bool,
    pub target: Option<UnitRef>,
}

/// An ability waiting out its own cooldown.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cooldown {
    pub ability: u16,
    pub left: f32,
}

/// A hero's side of the fight, as every client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroFight {
    pub life: Life,
    /// Mana or energy, as the class has.
    pub power: f32,
    pub max_power: f32,
    /// Combo points, on enemy `combo_on`.
    pub combo: u8,
    pub combo_on: Option<u32>,
    pub casting: Option<Casting>,
    /// Seconds of global cooldown left.
    pub gcd: f32,
    pub cooldowns: Vec<Cooldown>,
    pub dodge: Charges,
    /// In combat (spec §6.1: nothing regenerates): an enemy is after them, or they dealt or took
    /// a blow lately.
    pub in_combat: bool,
    /// The enemy their auto-attack swings at.
    pub attacking: Option<u32>,
    /// Their feet are held: a rooted cast, or dead.
    pub rooted: bool,
    /// Seconds of Sprint left.
    pub sprint: f32,
    /// Seconds of eating and drinking left.
    pub eating: f32,
    /// Conjured drinks carried (spec §6.2).
    pub drinks: u8,
}

impl HeroFight {
    /// A fresh hero of `class`: up, full power, every dodge.
    pub fn new(class: &ClassDef, r: &CombatRules) -> Self {
        Self {
            power: class.power_max,
            max_power: class.power_max,
            dodge: Charges::full(r.dodge.charges),
            ..Self::default()
        }
    }

    /// Up and fighting.
    pub fn up(&self) -> bool {
        self.life == Life::Up
    }
}

/// An enemy's side of the fight, as every client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EnemyFight {
    pub control: Option<ControlKind>,
    pub slowed: bool,
    pub casting: Option<Casting>,
    /// The spells of what is on it (a hold, a slow, a bleed), for the target frame's debuffs.
    pub auras: Vec<u32>,
}

/// A telegraphed blow about to land (spec §6.1): where, which way, and how soon.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Telegraph {
    pub id: u32,
    pub enemy: u32,
    pub ability: u16,
    pub shape: Shape,
    pub at: V3,
    pub facing: V2,
    pub left: f32,
    pub total: f32,
}

/// Why a cast stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    /// A channel ran its course.
    Finished,
    /// Its caster moved further than it allows.
    Moved,
    /// Its target died, or went out of sight.
    TargetLost,
    /// Its caster dodged.
    Cancelled,
    /// Its caster fell, or was held.
    Interrupted,
}

/// Why the host turned a press down: WoW's own refusals, shown in its red error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    Down,
    Busy,
    NotReady,
    NoPower,
    Moving,
    NeedStealth,
    InCombat,
    NeedCombo,
    NoTarget,
    BadTarget,
    Aware,
    OutOfRange,
    Facing,
    Behind,
    NotInSight,
}

/// An ability press: the latest, repeated in every input until the next, so an input replaced
/// before the host steps cannot lose it; `seq` tells a new press from the same one again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CastPress {
    pub seq: u8,
    /// The WoW action id pressed (1 to 120), which the class's bar maps to an ability.
    pub slot: u8,
    pub target: Option<UnitRef>,
    /// The spot, for an ability aimed at the ground.
    pub at: Option<V3>,
}

/// What happened in a step, for every client to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A cast began: the caster's hands glow, its player's cast bar fills.
    CastStart {
        by: UnitRef,
        ability: u16,
        target: Option<UnitRef>,
        secs: f32,
    },
    /// A channel began.
    ChannelStart {
        by: UnitRef,
        ability: u16,
        target: Option<UnitRef>,
        secs: f32,
    },
    /// An ability went off (or a channel ticked): its visual, at what it hit and where.
    CastGo {
        by: UnitRef,
        ability: u16,
        hits: Vec<UnitRef>,
        at: Option<V3>,
    },
    /// A cast stopped without going off, or a channel ended.
    CastStop {
        by: UnitRef,
        ability: u16,
        why: Stop,
    },
    Damage {
        to: UnitRef,
        by: Option<UnitRef>,
        amount: f32,
        crit: bool,
        ambush: bool,
    },
    /// A melee blow's swing (its damage is a `Damage` of its own).
    Swing {
        by: UnitRef,
        to: UnitRef,
    },
    /// A telegraphed blow missed a hero who dodged.
    Evaded {
        player: PlayerId,
        enemy: u32,
    },
    /// A press the host turned down: shown to its player.
    Refused {
        player: PlayerId,
        why: Refusal,
    },
    Dodged {
        player: PlayerId,
    },
    /// A Blink's end, which its player's client moves the hero to.
    Blinked {
        player: PlayerId,
        to: V3,
    },
    Downed {
        player: PlayerId,
    },
    Revived {
        player: PlayerId,
        by: PlayerId,
    },
    Died {
        player: PlayerId,
    },
    EnemyDied {
        id: u32,
        by: Option<PlayerId>,
    },
}

/// The fight's host-only state: never sent.
#[derive(Clone, Debug)]
pub(crate) struct Combat {
    pub(crate) heroes: Vec<HeroCombat>,
    /// One per enemy, beside the world's enemies.
    pub(crate) enemies: Vec<EnemyCombat>,
    pub(crate) dice: Dice,
    pub(crate) next_telegraph: u32,
}

impl Default for Combat {
    fn default() -> Self {
        Self {
            heroes: Vec::new(),
            enemies: Vec::new(),
            dice: Dice::new(DICE_SEED),
            next_telegraph: 1,
        }
    }
}

impl Combat {
    /// `player`'s state, made if need be.
    pub(crate) fn hero_index(&mut self, player: PlayerId) -> usize {
        match self.heroes.iter().position(|h| h.player == player) {
            Some(i) => i,
            None => {
                self.heroes.push(HeroCombat {
                    player,
                    ..HeroCombat::default()
                });
                self.heroes.len() - 1
            }
        }
    }

    pub(crate) fn hero(&self, player: PlayerId) -> Option<&HeroCombat> {
        self.heroes.iter().find(|h| h.player == player)
    }

    /// A player left: nothing of their hero's fight stays.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
    }

    /// A fresh fight for each enemy placed: its moves half ready, so none opens with its big
    /// move at once.
    pub(crate) fn place(&mut self, content: &Content, enemies: &[Enemy]) {
        self.enemies = enemies.iter().map(|e| fresh_fight(content, e)).collect();
    }

    /// A fresh fight for one more enemy, beside the others' (which are not touched).
    pub(crate) fn add_enemy(&mut self, content: &Content, enemy: &Enemy) {
        self.enemies.push(fresh_fight(content, enemy));
    }
}

/// An enemy's fight as it begins: its moves half ready, so none opens with its big move at once.
fn fresh_fight(content: &Content, e: &Enemy) -> EnemyCombat {
    EnemyCombat {
        moves_in: content.enemies[e.kind]
            .kit
            .iter()
            .map(|&(_, every)| every * 0.5)
            .collect(),
        engage: Engage::free(content.rules.senses.chase_reach),
        ..EnemyCombat::default()
    }
}

/// One hero's host-only fight.
#[derive(Clone, Debug, Default)]
pub(crate) struct HeroCombat {
    pub(crate) player: PlayerId,
    /// An input has been seen: the first is a baseline, so a new run casts nothing old.
    pub(crate) started: bool,
    /// The last ability press's `seq`.
    pub(crate) seq: Option<u8>,
    /// The last dodge count.
    pub(crate) dodges: u8,
    /// This step's input moves the hero.
    pub(crate) moving: bool,
    pub(crate) cast: Option<CastState>,
    /// Seconds until auto-attack may swing again.
    pub(crate) swing_in: f32,
    /// Seconds in which telegraphed blows miss them.
    pub(crate) immune: f32,
    /// Seconds left in combat after their last blow dealt or taken.
    pub(crate) combat_for: f32,
    /// In stealth at the last step: Stealth's cooldown starts as stealth ends.
    pub(crate) was_stealthed: bool,
    pub(crate) meal: Option<Meal>,
    /// How much faster Sprint makes them while it lasts.
    pub(crate) sprint_by: f32,
}

/// A meal under way: health and mana back a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Meal {
    pub(crate) health: f32,
    pub(crate) mana: f32,
}

/// A hero's cast under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CastState {
    pub(crate) ability: usize,
    /// Its enemy, an index into the world's enemies.
    pub(crate) enemy: Option<usize>,
    pub(crate) at: Option<V3>,
    /// Where the hero stood as it began.
    pub(crate) from: V2,
    pub(crate) elapsed: f32,
    /// A channel's ticks so far.
    pub(crate) ticks: u32,
}

/// One enemy's host-only fight.
#[derive(Clone, Debug, Default)]
pub(crate) struct EnemyCombat {
    pub(crate) held: Option<Held>,
    pub(crate) slow: Option<Slowed>,
    pub(crate) burns: Vec<Burn>,
    /// Turned to a Distract's spot, for so many seconds more.
    pub(crate) distracted: Option<(V2, f32)>,
    /// A sheep's wandering: where it was turned, and where it walks to.
    pub(crate) wander: Option<(V2, V2)>,
    /// Seconds until its melee swing.
    pub(crate) swing_in: f32,
    /// Seconds until each of its kind's moves is ready.
    pub(crate) moves_in: Vec<f32>,
    pub(crate) cast: Option<EnemyCast>,
    /// What its senses are told this step.
    pub(crate) engage: Engage,
}

/// A hold on an enemy, and who put it there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Held {
    pub(crate) kind: ControlKind,
    pub(crate) left: f32,
    pub(crate) by: PlayerId,
    pub(crate) spell: u32,
}

/// A slow on an enemy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slowed {
    pub(crate) by: f32,
    pub(crate) left: f32,
    pub(crate) spell: u32,
}

/// A bleed or a burn: `amount` every `every` seconds, the next in `next`, for `left` more.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Burn {
    pub(crate) by: PlayerId,
    pub(crate) amount: f32,
    pub(crate) every: f32,
    pub(crate) next: f32,
    pub(crate) left: f32,
    pub(crate) spell: u32,
    /// Its ticks make no noise (Garrote).
    pub(crate) silent: bool,
}

/// An enemy's cast or telegraphed windup under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EnemyCast {
    pub(crate) ability: usize,
    pub(crate) player: PlayerId,
    pub(crate) left: f32,
    pub(crate) total: f32,
    pub(crate) telegraph: Option<u32>,
}

/// What an enemy's senses may do this step, from what holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Hold {
    /// Look, think and move as its senses say.
    Free,
    /// Look and think, but stand turned to its hero (rooted, or casting).
    Still,
    /// Out of the fight (stunned, sapped, a sheep, dead): neither look, think nor move.
    Out,
    /// Unless something worries it more, stand turned to this spot (Distract).
    Facing(V2),
}

/// What an enemy's senses are told each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Engage {
    pub(crate) hold: Hold,
    /// Yards short of its hero a chase stops: its melee reach, or a caster's range.
    pub(crate) reach: f32,
    /// Its speed's share (under a slow, less than 1).
    pub(crate) speed: f32,
}

impl Engage {
    /// Free, stopping `reach` yards short, at full speed.
    pub(crate) fn free(reach: f32) -> Self {
        Self {
            hold: Hold::Free,
            reach,
            speed: 1.0,
        }
    }
}

impl Default for Engage {
    fn default() -> Self {
        Self::free(2.0)
    }
}

impl World {
    /// What happened in the steps since the last drain (host) or push (mirror), oldest first: a
    /// host sends each step's slice of them.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The events so far, taken.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// A mirror takes its host's events beside its snapshot.
    pub fn push_events(&mut self, events: Vec<Event>) {
        self.events.extend(events);
    }

    /// `player`'s hero, by index.
    pub(crate) fn hero_slot(&self, player: PlayerId) -> Option<usize> {
        self.heroes.iter().position(|h| h.player == player)
    }

    /// The living enemies within `radius` of `at`, by index.
    pub(crate) fn enemies_within(&self, at: V2, radius: f32) -> Vec<usize> {
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0.0 && e.pos.distance(at) <= radius)
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod state_tests {
    use std::sync::Arc;

    use super::*;
    use crate::{ContentSources, HeroPose, PlayerInput};

    fn world() -> World {
        let content = crate::Content::from_ron(ContentSources {
            classes: r#"[(key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1,
                base_hp: 880.0, speed: 7.0, power_max: 1300.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: "[]",
            loot: "()",
        })
        .unwrap();
        World::new(Arc::new(content))
    }

    #[test]
    fn a_new_hero_is_up_with_full_power_and_both_dodges() {
        let mut w = world();
        w.add_hero(0, "mage", "Jaina", 1, 1).unwrap();
        let f = &w.hero(0).unwrap().fight;
        assert!(f.up());
        assert_eq!((f.power, f.max_power), (1300.0, 1300.0));
        assert_eq!(f.dodge, Charges::full(2));
        assert!(f.casting.is_none() && f.cooldowns.is_empty());
    }

    #[test]
    fn a_mirror_takes_the_telegraphs_and_the_events() {
        let mut host = world();
        host.add_hero(0, "mage", "Jaina", 1, 1).unwrap();
        host.telegraphs.push(Telegraph {
            id: 1,
            enemy: 9,
            ability: 0,
            shape: Shape::Circle { radius: 8.0 },
            at: V3::new(10.0, 10.0, 0.0),
            facing: V2::new(1.0, 0.0),
            left: 1.0,
            total: 1.4,
        });
        host.events.push(Event::Dodged { player: 0 });
        let mut mirror = world();
        mirror.apply_snapshot(host.snapshot());
        mirror.push_events(host.drain_events());
        assert_eq!(mirror.telegraphs, host.telegraphs);
        assert_eq!(mirror.drain_events(), vec![Event::Dodged { player: 0 }]);
        assert!(host.events().is_empty());
    }

    #[test]
    fn an_input_carries_its_press_and_dodges() {
        let input = PlayerInput {
            pose: Some(HeroPose::default()),
            cast: Some(CastPress {
                seq: 3,
                slot: 2,
                target: Some(UnitRef::Enemy(7)),
                at: None,
            }),
            dodge_presses: 4,
            ..Default::default()
        };
        let bytes = postcard::to_stdvec(&input).unwrap();
        assert_eq!(postcard::from_bytes::<PlayerInput>(&bytes).unwrap(), input);
    }
}
