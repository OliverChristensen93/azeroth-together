//! Rivals (spec §7.4): parties of two or three bot adventurers with goals of their own, in the
//! manner of Single Player Tarkov's questing bots (§1.1). A rival is a hero of another side:
//! in [`World::heroes`] with [`Hero::side`] a [`Side::Rival`], a class of its own (its kind's,
//! [`RivalContent::load`]), a pack, a fight and a stance, and moved only by the input its bot
//! sets each step as a player's client would. So the enemies see, hear and fight rivals as they
//! do heroes; the raid's packs, searches, bodies and exits are theirs too. The raid itself stays
//! the party's: it ends with the party, rivals settle no outcome, and only a side revives its
//! own.

mod content;
mod drive;
mod goals;
mod kit;
mod mind;
#[cfg(test)]
mod tests;
mod travel;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub use content::{
    KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalRules, RouteDef, RouteEnd,
    RoutePlace,
};
use goals::Goal;
use kit::{kit_of, KitSlot};
use travel::{Progress, Travel};
pub use travel::{RIVAL_BUDGET, RIVAL_POOL};

use crate::combat::Dice;
use crate::raid::{roll_table, seed_for};
use crate::senses::Mind;
use crate::{
    free_kit, ActPress, CastPress, Content, Event, HeroPose, ItemKind, Loadout, Place, PlayerId,
    PlayerInput, World, WorldError, V2, V3,
};

/// Rivals' player numbers start here, past any party's (a host gives 0 to 3).
pub const RIVAL_BASE: PlayerId = 64;

/// Mixed into a raid's seed for the dice a rival's carried find is rolled with, so no rival rolls
/// what a chest did.
const CARRY_STREAM: u64 = 0x5249_5641_4c53_0001;

/// Mixed into the seed a party is brought in with, for the dice of its names, its points and its
/// own choices.
const PARTY_STREAM: u64 = 0x5249_5641_4c53_0002;

/// Yards from a rival entry at which a party counts as having come in by it.
const ENTRY_NEAR: f32 = 3.0;

/// Whose side a hero is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// The players' party.
    #[default]
    Party,
    /// A rival party, by its number in the raid.
    Rival(u8),
}

impl Side {
    pub fn is_rival(self) -> bool {
        matches!(self, Side::Rival(_))
    }
}

/// The rivals' host-only state: never sent (a client's mirror keeps only [`Rivals::mirror`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct Rivals {
    /// One per rival in the raid, the dead included.
    pub(crate) bodies: Vec<RivalBody>,
    /// The parties a roll brought.
    pub(crate) parties: Vec<Party>,
    /// Which of the roll's parties have come.
    // The arrivals that read them come with the raid's roll (Task 9).
    #[allow(dead_code)]
    pub(crate) arrived: Vec<bool>,
    /// Seconds until arrivals are tried again.
    #[allow(dead_code)]
    pub(crate) try_in: f32,
    /// Rival numbers handed out: the next is `RIVAL_BASE + next`.
    pub(crate) next: u8,
    /// Path searches left to the rivals this step.
    pub(crate) paths_left: u32,
    /// Rivals that left the raid alive, by an exit or unseen, in the order they went.
    pub(crate) out: Vec<PlayerId>,
    /// A client's mirror: the view of rivals its host sent.
    pub(crate) mirror: Option<RivalView>,
    /// The loudest noise a hero of the party made lately: where, its radius and when. A hunting
    /// party goes toward it when its own ears have nothing, as the night's hunters go toward the
    /// loudest noise (spec §7.2).
    pub(crate) rumour: Option<(V3, f32, f32)>,
    /// Floors the navigator had looked at when the rivals' turn began, to keep their searches to
    /// [`RIVAL_POOL`].
    pub(crate) floors_start: u64,
}

/// What a client is told of the rivals: which its hero sees, and the marks they show it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RivalView {
    pub seen: Vec<PlayerId>,
    pub marks: Vec<RivalMark>,
}

/// One rival's mark for one player (spec §5.5: "?" and "!").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalMark {
    pub rival: PlayerId,
    pub alert: crate::Alert,
}

/// One rival's mind and bot.
#[derive(Clone, Debug)]
pub(crate) struct RivalBody {
    pub(crate) player: PlayerId,
    /// Its party's number ([`Side::Rival`]).
    pub(crate) party: u8,
    /// Index into the rivals' kinds.
    pub(crate) kind: usize,
    /// What it makes of each hero of the party (spec §7.4: "notice heroes the way enemies do").
    pub(crate) mind: Mind,
    pub(crate) kit: Vec<KitSlot>,
    /// The last hero of the party to strike it.
    pub(crate) struck_by: Option<PlayerId>,
    pub(crate) way: Way,
    /// Its presses, numbered as a client numbers a player's.
    pub(crate) cast_seq: u8,
    pub(crate) cast: Option<CastPress>,
    /// When it last pressed: a bot presses at most every so often.
    pub(crate) press_at: f32,
    pub(crate) stealth_presses: u8,
    pub(crate) stealth_at: f32,
    pub(crate) dodge_presses: u8,
    /// Telegraphs it has rolled to dodge.
    pub(crate) dodged: Vec<u32>,
    pub(crate) act_seq: u8,
    pub(crate) act: Option<ActPress>,
    /// The container it is looting.
    pub(crate) loot: Option<u32>,
    /// Containers it is done with.
    pub(crate) done: Vec<u32>,
    /// Seconds it has wanted to move and could not (no way, no floors): at `stuck_for` it slips
    /// out unseen.
    pub(crate) pinned: f32,
}

/// Where a rival is walking: its path and the goal it was searched for.
#[derive(Clone, Debug, Default)]
pub(crate) struct Way {
    pub(crate) path: Option<Arc<[V3]>>,
    pub(crate) next: usize,
    pub(crate) toward: Option<V2>,
    /// The last search found no way.
    pub(crate) failed: bool,
    /// A search that found no way is not tried again for the same goal before this time.
    pub(crate) retry_at: f32,
}

/// One rival party in the raid.
#[derive(Clone, Debug)]
pub(crate) struct Party {
    pub(crate) number: u8,
    /// Index into the rivals' parties.
    pub(crate) def: usize,
    pub(crate) aggression: f32,
    /// Its members, the first living one leading.
    pub(crate) members: Vec<PlayerId>,
    pub(crate) goal: Goal,
    /// Points of interest visited or given up.
    pub(crate) visited: Vec<usize>,
    pub(crate) pois_left: u8,
    pub(crate) next_poi: Option<usize>,
    pub(crate) looting: Option<usize>,
    /// A hero of the party struck one of them.
    pub(crate) hurt_by: Option<PlayerId>,
    pub(crate) hunted: bool,
    /// Exits given up (no way there).
    pub(crate) shut: Vec<usize>,
    /// The leader's nearest yet to its goal's place, and the seconds since it got nearer.
    pub(crate) best: f32,
    pub(crate) stalled: f32,
    pub(crate) dice: Dice,
    /// The rival entry it came in by, if it came in at one.
    pub(crate) entry: Option<usize>,
    /// The last place of the content's routes it was at: its entry, then each point it looted.
    pub(crate) place: Option<RoutePlace>,
    /// The route its leader is walking.
    pub(crate) journey: Option<Journey>,
    /// The backstop's clock: seconds with no headway toward any goal.
    pub(crate) progress: Progress,
    /// The backstop has called: its members slip away unseen as soon as no hero sees them.
    pub(crate) leaving: bool,
}

/// A leader's walk to a place of the content's routes.
#[derive(Clone, Debug)]
pub(crate) struct Journey {
    pub(crate) to: RoutePlace,
    pub(crate) travel: Travel,
}

impl Rivals {
    pub(crate) fn body(&self, player: PlayerId) -> Option<&RivalBody> {
        self.bodies.iter().find(|b| b.player == player)
    }

    pub(crate) fn body_mut(&mut self, player: PlayerId) -> Option<&mut RivalBody> {
        self.bodies.iter_mut().find(|b| b.player == player)
    }

    /// `player` left the world: a rival's body and place in its party go; a hero's are
    /// forgotten by every rival's mind.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.bodies.retain(|b| b.player != player);
        for p in &mut self.parties {
            p.members.retain(|&m| m != player);
        }
        for b in &mut self.bodies {
            b.mind.forget(player);
        }
    }

    /// The last hero of the party to strike rival `player`.
    pub(crate) fn struck_by(&self, player: PlayerId) -> Option<PlayerId> {
        self.body(player).and_then(|b| b.struck_by)
    }
}

impl RivalBody {
    fn new(player: PlayerId, party: u8, kind: usize, content: &Content) -> Self {
        let k = &content.rivals.kinds[kind];
        Self {
            player,
            party,
            kind,
            mind: Mind::new(content.rules.rivals.cone, false),
            kit: kit_of(k.class, k.big_ability, content),
            struck_by: None,
            way: Way::default(),
            cast_seq: 0,
            cast: None,
            press_at: f32::NEG_INFINITY,
            stealth_presses: 0,
            stealth_at: f32::NEG_INFINITY,
            dodge_presses: 0,
            dodged: Vec::new(),
            act_seq: 0,
            act: None,
            loot: None,
            done: Vec::new(),
            pinned: 0.0,
        }
    }
}

/// The dice `player`'s carried find is rolled with in a raid of `seed`: a stream of the seed and
/// the player's own, so raids of twin seeds and rivals of twin numbers all roll differently
/// ([`seed_for`] mixes what [`Dice::new`] would leave of a single bit).
fn carry_dice(seed: u64, player: PlayerId) -> Dice {
    Dice::new(seed_for(seed ^ CARRY_STREAM, u64::from(player)))
}

impl World {
    /// Whether `a` and `b` fight each other: the party against any rival; rival parties leave
    /// one another be. Nobody is hostile to a player with no hero.
    pub fn hostile(&self, a: PlayerId, b: PlayerId) -> bool {
        match (self.hero(a), self.hero(b)) {
            (Some(x), Some(y)) => {
                x.side != y.side && (x.side == Side::Party || y.side == Side::Party)
            }
            _ => false,
        }
    }

    /// `player`'s side: the party's for a player with no hero.
    pub fn side_of(&self, player: PlayerId) -> Side {
        self.hero(player).map_or(Side::Party, |h| h.side)
    }

    /// A rival of `kind` (an index into the rivals' kinds) on `side` at `at`, as `player`: a hero
    /// of the kind's class named `name`, standing on the floor there, wearing its gear (its bag in
    /// its first bag slot) and carrying its kind's `carry` roll. Its first input is set, the
    /// baseline every hero's first input is.
    pub(crate) fn add_rival_hero(
        &mut self,
        player: PlayerId,
        kind: usize,
        side: Side,
        at: V2,
        name: &str,
    ) -> Result<(), WorldError> {
        let content = self.content().clone();
        let def = &content.rivals.kinds[kind];
        let class_key = content.classes[def.class].key.clone();
        self.add_hero(player, &class_key, name, def.race, def.sex)?;
        let at = content.map.bounds.clamp(at);
        let z = self.map().and_then(|m| m.ground_at(at)).unwrap_or(0.0);
        if let Some(hi) = self.hero_slot(player) {
            let h = &mut self.heroes[hi];
            h.side = side;
            h.pos = at;
            h.z = z;
        }
        let mut inventory = free_kit(def.class, &content);
        if let Some(bag) = inventory.first_of(ItemKind::Bag, &content) {
            let _ = inventory.move_to(bag, Place::Bag(0), &content);
        }
        if let Some(t) = def.carry_table {
            let mut dice = carry_dice(self.raid.roll.seed, player);
            for stack in roll_table(&content.loot.tables[t], &mut dice, &content) {
                let _ = inventory.add(stack, &content);
            }
        }
        self.give_loadout(
            player,
            &Loadout {
                inventory,
                ..Loadout::default()
            },
        );
        let party = match side {
            Side::Rival(n) => n,
            Side::Party => 0,
        };
        self.rivals
            .bodies
            .push(RivalBody::new(player, party, kind, &content));
        self.set_input(
            player,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: at,
                    facing: V2::new(1.0, 0.0),
                    z,
                }),
                ..PlayerInput::default()
            },
        );
        Ok(())
    }

    /// Rival `player` was struck by `by`: a hero of the party, remembered as the one who slew
    /// it if it dies of it, and the rival and its party are alerted to them.
    pub(crate) fn rival_struck(&mut self, player: PlayerId, by: PlayerId) {
        if !(self.side_of(player).is_rival() && self.hostile(player, by)) {
            return;
        }
        if let Some(b) = self.rivals.body_mut(player) {
            b.struck_by = Some(by);
        }
        self.alert_rival(player, by);
    }

    /// Rival `player` is out by `exit`, seen: everything it carries goes with it, and nobody
    /// settles an outcome for it (spec §7.4: it extracts, as a hero would, but the raid is not
    /// its).
    pub(crate) fn rival_leaves(&mut self, player: PlayerId, exit: usize) {
        self.events.push(Event::RivalOut {
            player,
            exit: exit as u8,
        });
        self.rival_goes(player);
    }

    /// Rival `player` slips out of the raid unseen (the backstop of a party that gets nowhere,
    /// spec §7.4), with all it carries, once no hero of the party sees it. Nothing is told:
    /// nobody saw it go. Whether it went is returned; it stays while any hero sees it, and when
    /// it is no rival, is not up, or is gone already.
    pub(crate) fn rival_leaves_unseen(&mut self, player: PlayerId) -> bool {
        let Some(h) = self.hero(player) else {
            return false;
        };
        if !(h.side.is_rival() && h.fight.up()) || self.party_sees(h.pos, h.z) {
            return false;
        }
        self.rival_goes(player);
        true
    }

    /// Rival `player` is gone with what it carried, counted out. Its pack is taken first: removing
    /// the hero would otherwise drop it at its feet, as a hero who left the session drops theirs.
    fn rival_goes(&mut self, player: PlayerId) {
        let _ = self.raid.take_pack(player);
        self.remove_hero(player);
        // It left alive: it is no hero who parted the raid, and no client watches from it.
        self.raid.parted.retain(|p| *p != player);
        self.rivals.out.push(player);
    }

    /// Party kind `def` (an index into the rivals' parties) comes in at `at` as party `number`,
    /// at `aggression`, its dice from `seed`: each member a rival of its kind, numbered from
    /// [`RIVAL_BASE`] up, the first at `at` and the rest 1.5 yards around it, each named by the
    /// dice from its kind's names; the party visits between `pois` points. The members made.
    ///
    /// Public so the real-bake tests can bring a party in until the raid's roll does (Task 9).
    pub fn add_party(
        &mut self,
        def: usize,
        number: u8,
        aggression: f32,
        at: V2,
        seed: u64,
    ) -> Vec<PlayerId> {
        let content = self.content().clone();
        let d = &content.rivals.parties[def];
        let (lo, hi) = content.rules.rivals.pois;
        // Hashed, so twin seeds roll different parties (`Dice::new` alone keeps one bit less).
        let mut dice = Dice::new(seed_for(seed ^ PARTY_STREAM, u64::from(number)));
        let mut members = Vec::new();
        for (i, &kind) in d.kinds.iter().enumerate() {
            let Some(player) = RIVAL_BASE.checked_add(self.rivals.next) else {
                break;
            };
            self.rivals.next = self.rivals.next.saturating_add(1);
            let spot = if i == 0 {
                at
            } else {
                at + V2::from_angle(i as f32 * std::f32::consts::TAU / 3.0, 1.5)
            };
            let k = &content.rivals.kinds[kind];
            let name = match k.names.len() {
                0 => k.name.clone(),
                n => k.names[((dice.unit() * n as f32) as usize).min(n - 1)].clone(),
            };
            if self
                .add_rival_hero(player, kind, Side::Rival(number), spot, &name)
                .is_ok()
            {
                members.push(player);
            }
        }
        let span = f32::from(hi - lo + 1);
        let pois_left = lo + ((dice.unit() * span) as u8).min(hi - lo);
        let entry = content
            .rivals
            .entries
            .iter()
            .position(|e| e.distance(at) <= ENTRY_NEAR);
        self.rivals.parties.push(Party {
            number,
            def,
            aggression,
            members: members.clone(),
            goal: Goal::Wait,
            visited: Vec::new(),
            pois_left,
            next_poi: None,
            looting: None,
            hurt_by: None,
            hunted: false,
            shut: Vec::new(),
            best: f32::INFINITY,
            stalled: 0.0,
            dice,
            entry,
            place: entry.map(RoutePlace::Entry),
            journey: None,
            progress: Progress::default(),
            leaving: false,
        });
        members
    }

    /// One line per rival party, for `NIGHTFALL_TRACE_RIVALS`: its goal, aggression and members
    /// (with health and where they stand). Empty on a client's mirror.
    pub fn rival_debug(&self) -> Vec<String> {
        let content = self.content();
        self.rivals
            .parties
            .iter()
            .map(|p| {
                let members: Vec<String> = p
                    .members
                    .iter()
                    .filter_map(|&m| self.hero(m))
                    .map(|h| {
                        format!(
                            "{} {} {:.0}/{:.0} ({:.1}, {:.1})",
                            h.player, h.name, h.hp, h.max_hp, h.pos.x, h.pos.y
                        )
                    })
                    .collect();
                format!(
                    "party {} {}: {:?}, aggression {:.2}, {} points left [{}]",
                    p.number,
                    content.rivals.parties[p.def].name,
                    p.goal,
                    p.aggression,
                    p.pois_left,
                    members.join("; ")
                )
            })
            .collect()
    }
}
