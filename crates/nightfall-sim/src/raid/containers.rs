//! Containers: the chests, crates and graves the content places, the bodies enemies and heroes
//! leave, a hero's remains from their last raid, and things dropped on the ground.

use super::{Container, Holder, Raid, RaidRoll};
use crate::combat::Dice;
use crate::{Content, Loudness, PlayerId, Stack, Table, World, DT, V2, V3};

/// How far up or down a container may be from a hero and still be in reach, yards: a chest in a
/// crypt is not opened from the hill above it.
pub(crate) const REACH_UP: f32 = 3.0;
/// Things dropped this near a pile join it, yards.
const PILE_JOIN: f32 = 1.5;
/// How far a hero may stray from where a search began, yards.
const SEARCH_STRAY: f32 = 1.0;
/// Mixed into the seed for enemies' bodies, so a body never rolls what a chest did.
const BODY_SEED: u64 = 0xB0D7_5EED_0000_0001;

/// The `i`th roll from a raid's `seed`: every container its own dice, the same every time. The
/// mix is splitmix64's finalizer, so that seeds that differ in one bit (and [`Dice::new`] sets
/// bit 0 of whatever it is given) still roll differently.
pub(crate) fn seed_for(seed: u64, i: u64) -> u64 {
    let mut z = seed ^ (i + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A whole number from `lo` to `hi`.
fn pick(dice: &mut Dice, lo: u16, hi: u16) -> u16 {
    if hi <= lo {
        return lo;
    }
    let span = u32::from(hi - lo) + 1;
    lo + ((dice.unit() * span as f32) as u32).min(span - 1) as u16
}

/// How many times a table is rolled for a container or body at `at`: once, and once more for
/// each `loot_rolls` of the risk zone it lies in (spec §8.1).
fn rolls_at(content: &Content, at: V2) -> usize {
    1 + content
        .map
        .zone_at(at)
        .map_or(0, |z| usize::from(z.loot_rolls))
}

/// A table's roll (spec §8.1): `rolls` picks, each an entry by weight or nothing, its count
/// within its range; the same item twice is one stack, as far as its stack size lets it be.
/// Everything rolled is found (spec §8.2: world loot counts as found). An entry naming no item
/// rolls nothing, and a table with no weight to roll from rolls nothing at all.
pub(crate) fn roll_table(table: &Table, dice: &mut Dice, content: &Content) -> Vec<Stack> {
    let n = pick(dice, u16::from(table.rolls.0), u16::from(table.rolls.1));
    let total = table.nothing + table.items.iter().map(|e| e.weight).sum::<f32>();
    let mut out: Vec<Stack> = Vec::new();
    if total.is_nan() || total <= 0.0 {
        return out;
    }
    for _ in 0..n {
        let mut x = dice.unit() * total - table.nothing;
        if x < 0.0 {
            continue;
        }
        let Some(e) = table.items.iter().find(|e| {
            if x < e.weight {
                true
            } else {
                x -= e.weight;
                false
            }
        }) else {
            continue;
        };
        let Some(def) = content.items.get(e.item) else {
            continue;
        };
        let mut count = pick(dice, e.count.0, e.count.1);
        let max = def.stack.max(1);
        for s in out.iter_mut().filter(|s| usize::from(s.item) == e.item) {
            let n = max.saturating_sub(s.count).min(count);
            s.count += n;
            count -= n;
        }
        while count > 0 {
            let n = count.min(max);
            out.push(Stack::found(e.item, n));
            count -= n;
        }
    }
    out
}

/// Whether a hero at `pos` (feet at `z`) reaches a container at `at` (at height `cz`, when it is
/// known).
pub(crate) fn in_reach(pos: V2, z: f32, at: V2, cz: Option<f32>, reach: f32) -> bool {
    pos.distance(at) <= reach && cz.is_none_or(|cz| (z - cz).abs() <= REACH_UP)
}

/// How long searching what `holder` holds takes, and how loud it is (spec §5.2).
pub(crate) fn search_of(holder: Holder, content: &Content) -> (f32, Option<Loudness>) {
    match holder {
        // A kind the content no longer has opens at once and says nothing.
        Holder::Placed(k) => content
            .loot
            .kinds
            .get(usize::from(k))
            .map_or((0.0, None), |k| (k.search, Some(k.sound))),
        Holder::Body(_) | Holder::Hero(_) => (content.rules.run.search_body, Some(Loudness::Open)),
        Holder::Remains(_) | Holder::Pile => (0.0, None),
    }
}

impl Raid {
    /// A raid as `roll` has it: every container of the content's loot placed, each filled from
    /// its table by dice of its own from the raid's seed. A placement naming a kind or table the
    /// content lacks is left out.
    pub(crate) fn new(content: &Content, roll: RaidRoll) -> Self {
        let mut raid = Self {
            roll,
            next_id: 1,
            ..Self::default()
        };
        for (i, p) in content.loot.containers.iter().enumerate() {
            let (Some(kind), Some(table)) = (
                content.loot.kinds.get(p.kind),
                content.loot.tables.get(p.table),
            ) else {
                continue;
            };
            let mut dice = Dice::new(seed_for(raid.roll.seed, i as u64));
            let slots = usize::from(kind.slots);
            let mut stacks = Vec::new();
            let rolls = rolls_at(content, p.at);
            for _ in 0..rolls {
                stacks.extend(roll_table(table, &mut dice, content));
            }
            // A zone's extra rolls fill the container, no more; one roll keeps all it found.
            if rolls > 1 {
                stacks.truncate(slots);
            }
            raid.add_container(
                Holder::Placed(p.kind as u16),
                p.at,
                p.z,
                stacks,
                slots,
                false,
                p.lock.map(|l| l as u16),
            );
        }
        raid
    }

    /// A new container holding `stacks`, with at least `slots` slots; its id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_container(
        &mut self,
        holder: Holder,
        at: V2,
        z: Option<f32>,
        stacks: Vec<Stack>,
        slots: usize,
        searched: bool,
        lock: Option<u16>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut held: Vec<Option<Stack>> = stacks.into_iter().map(Some).collect();
        if held.len() < slots {
            held.resize(slots, None);
        }
        self.containers.push(Container {
            id,
            holder,
            at,
            z,
            stacks: held,
            searched,
            lock,
        });
        id
    }

    /// `stack` on the ground at `at`: first onto an equal stack (the same item, found or brought)
    /// with room in a pile there, then into an empty slot of a pile there, or a new pile. So
    /// drops of one thing build up in one stack, not one slot each.
    pub(crate) fn drop_on_ground(&mut self, at: V3, mut stack: Stack, content: &Content) {
        let here = at.ground();
        let most = content
            .items
            .get(usize::from(stack.item))
            .map_or(u16::MAX, |d| d.stack.max(1));
        for c in self
            .containers
            .iter_mut()
            .filter(|c| c.holder == Holder::Pile && c.at.distance(here) <= PILE_JOIN)
        {
            for s in c.stacks.iter_mut().flatten() {
                if s.item == stack.item && s.found == stack.found && s.count < most {
                    let n = (most - s.count).min(stack.count);
                    s.count += n;
                    stack.count -= n;
                }
            }
        }
        if stack.count == 0 {
            return;
        }
        let near = self.containers.iter().position(|c| {
            c.holder == Holder::Pile
                && c.at.distance(here) <= PILE_JOIN
                && c.stacks.iter().any(Option::is_none)
        });
        match near {
            Some(i) => {
                let c = &mut self.containers[i];
                let free = c.stacks.iter().position(Option::is_none).expect("has room");
                c.stacks[free] = Some(stack);
            }
            None => {
                let slots = usize::from(content.rules.run.container_slots);
                self.add_container(
                    Holder::Pile,
                    here,
                    Some(at.z),
                    vec![stack],
                    slots,
                    true,
                    None,
                );
            }
        }
    }
}

impl World {
    /// The raid's part of a step, after the fight's: searches, bodies, deaths, then exits. A
    /// client's mirror has no raid of its own to run: its host's loot view is all it knows. A raid
    /// that is over (everyone out or dead, or abandoned) runs no more: nobody searches, is settled
    /// twice or leaves by an exit after their outcome is final.
    pub(crate) fn step_raid(&mut self) {
        if self.raid.mirror.is_some() || self.raid.over {
            return;
        }
        self.step_searches();
        self.step_bodies();
        self.step_deaths();
        self.step_exits();
    }

    /// Searches move on (spec §8.2): a searcher who stays by the container and up finishes it,
    /// heard every `noise_every` seconds while it lasts (§5.2), and then has it open; one who
    /// walks off or falls stops. What is open closes when its hero is no longer by it.
    pub(crate) fn step_searches(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.run;
        let mut sounds: Vec<(PlayerId, V3, Loudness)> = Vec::new();
        let mut finished: Vec<(PlayerId, u32)> = Vec::new();
        {
            let heroes = &self.heroes;
            let Raid {
                searches,
                containers,
                ..
            } = &mut self.raid;
            searches.retain_mut(|s| {
                let Some(h) = heroes.iter().find(|h| h.player == s.player) else {
                    return false;
                };
                let Some(c) = containers.iter().find(|c| c.id == s.id) else {
                    return false;
                };
                let by = h.fight.up()
                    && h.pos.distance(s.from) <= SEARCH_STRAY
                    && in_reach(h.pos, h.z, c.at, c.z, r.reach);
                if !by {
                    return false;
                }
                s.noise_in -= DT;
                if s.noise_in <= 0.0 {
                    s.noise_in += r.noise_every;
                    if let (_, Some(sound)) = search_of(c.holder, &content) {
                        sounds.push((s.player, V3::at(c.at, c.z.unwrap_or(h.z)), sound));
                    }
                }
                s.left -= DT;
                if s.left <= 1e-4 {
                    finished.push((s.player, s.id));
                    return false;
                }
                true
            });
        }
        for (player, id) in finished {
            if let Some(c) = self.raid.containers.iter_mut().find(|c| c.id == id) {
                c.searched = true;
            }
            self.raid.close(player);
            self.raid.opened.push((player, id));
        }
        // Heard by the senses, which listen later in this same step.
        for (by, at, sound) in sounds {
            self.make_noise(by, at, sound);
        }
        let heroes = &self.heroes;
        let Raid {
            opened, containers, ..
        } = &mut self.raid;
        opened.retain(|(p, id)| {
            let Some(h) = heroes.iter().find(|h| h.player == *p) else {
                return false;
            };
            containers
                .iter()
                .find(|c| c.id == *id)
                .is_some_and(|c| h.fight.up() && in_reach(h.pos, h.z, c.at, c.z, r.reach))
        });
    }

    /// Every enemy dead since the last step leaves a body (spec §8.2) holding a roll of its
    /// kind's table, when it has one and the roll holds something. Each enemy is bodied once,
    /// whether or not it left anything, so a dead enemy that stays in the list is not bodied
    /// again every step.
    pub(crate) fn step_bodies(&mut self) {
        let content = self.content().clone();
        let fresh: Vec<(u32, usize, V2, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.hp <= 0.0 && !self.raid.bodied.contains(&e.id))
            .map(|e| (e.id, e.kind, e.pos, e.z))
            .collect();
        for (id, kind, at, z) in fresh {
            self.raid.bodied.insert(id);
            let Some(table) = content
                .loot
                .bodies
                .iter()
                .find(|(k, _)| *k == kind)
                .and_then(|&(_, t)| content.loot.tables.get(t))
            else {
                continue;
            };
            let mut dice = Dice::new(seed_for(self.raid.roll.seed ^ BODY_SEED, u64::from(id)));
            let slots = usize::from(content.rules.run.container_slots);
            let mut stacks = Vec::new();
            let rolls = rolls_at(&content, at);
            for _ in 0..rolls {
                stacks.extend(roll_table(table, &mut dice, &content));
            }
            if rolls > 1 {
                stacks.truncate(slots);
            }
            if !stacks.is_empty() {
                self.raid
                    .add_container(Holder::Body(id), at, Some(z), stacks, slots, false, None);
            }
        }
    }
}
