//! The senses (spec §5): how visible and how loud each hero is, what each enemy makes of them,
//! and what it does about it. The host's alone: a client's mirror gets each hero's visibility and
//! noise in the snapshot, and its own player's marks beside it.
//!
//! Each step, once the heroes have their inputs, [`step_heroes`] works out every hero's stance,
//! visibility, speed and footsteps; then, once the map is attached, [`step_enemies`] lets every
//! enemy look, listen, think and act.

mod awareness;
mod noise;
mod react;
#[cfg(test)]
mod tests;
mod visibility;

pub use awareness::{Alert, Mark};
pub use noise::Loudness;
pub(crate) use noise::Noise;
pub(crate) use react::step_enemies;
pub use visibility::{Gait, Stance};

use awareness::{Mind, Shout, State};
use noise::step_radius;
use react::Way;
use visibility::{visibility, Light};

use crate::{AwarenessRules, Behaviour, Content, Hero, PlayerId, PlayerInput, World, DT, V3};

/// What the host keeps of one hero's senses: never sent.
#[derive(Clone, Debug, Default)]
struct HeroSense {
    player: PlayerId,
    /// The Stealth count of the last input; `None` until the first, which is taken as it stands
    /// (a new run must not toggle Stealth with the last run's presses).
    presses: Option<u8>,
    /// How the hero moves, as the last input said.
    gait: Gait,
    /// Seconds until the next footstep, while moving.
    step_in: f32,
    /// Seconds of a fire spell's flash left.
    flash: f32,
    /// The noises the hero made lately: (seconds ago, radius).
    recent: Vec<(f32, f32)>,
}

/// The senses' state on the host.
#[derive(Clone, Debug, Default)]
pub(crate) struct Senses {
    heroes: Vec<HeroSense>,
    /// The noises made since the enemies last listened.
    pub(crate) noises: Vec<Noise>,
    /// Each enemy's mind, beside it in the world's enemies.
    minds: Vec<Mind>,
    /// Each enemy's way while its senses hold it.
    ways: Vec<Way>,
    /// Shouts raised between steps (an alert from a hit), given out at the next.
    shouts: Vec<(usize, Shout)>,
    /// Which enemy gets its path asks first next step.
    cursor: usize,
}

impl Senses {
    /// `player`'s sense state, made if need be.
    fn hero_index(&mut self, player: PlayerId) -> usize {
        match self.heroes.iter().position(|h| h.player == player) {
            Some(i) => i,
            None => {
                self.heroes.push(HeroSense {
                    player,
                    ..HeroSense::default()
                });
                self.heroes.len() - 1
            }
        }
    }

    /// A player left: nothing of their hero stays, and no enemy keeps a thought of them. Their
    /// noises made this step stay: a noise marks a place, and the beat that gets a hero out of an
    /// exit is made in the step they go, so the enemies still hear it (see `step_enemies`).
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.shouts.retain(|(_, s)| s.player != player);
        for m in &mut self.minds {
            m.forget(player);
        }
    }

    /// `player` slipped away (Vanish): every enemy after them searches instead.
    pub(crate) fn lose(&mut self, player: PlayerId, r: &AwarenessRules) {
        for m in &mut self.minds {
            m.lose(player, r);
        }
    }

    /// Whether any enemy is alerted to `player`'s hero: they are in combat (spec §6.1).
    pub(crate) fn hunted(&self, player: PlayerId) -> bool {
        self.minds.iter().any(|m| m.alerted_to(player))
    }

    /// Whether enemy `i` is alerted to anyone.
    pub(crate) fn alerted(&self, i: usize) -> bool {
        self.minds.get(i).is_some_and(Mind::is_alerted)
    }

    /// The hero enemy `i` is after, and where it last saw them, when it is alerted.
    pub(crate) fn target_of(&self, i: usize) -> Option<(PlayerId, V3)> {
        match self.minds.get(i)?.state {
            State::Alerted {
                player, last_seen, ..
            } => Some((player, last_seen)),
            _ => None,
        }
    }

    /// Enemy `i` died: it thinks of nobody, follows no way, and a shout it raised this step is
    /// never heard. The row stays, so the lists stay beside the world's enemies.
    pub(crate) fn clear_enemy(&mut self, i: usize) {
        if let Some(m) = self.minds.get_mut(i) {
            m.calm();
        }
        if let Some(w) = self.ways.get_mut(i) {
            *w = Way::default();
        }
        self.shouts.retain(|(e, _)| *e != i);
    }

    /// A calm mind for each enemy placed from these spawn lines: a sentry's wide front cone, a
    /// sleeper's closed eyes, everyone else's own cone.
    pub(crate) fn place(&mut self, content: &Content, spawns: impl IntoIterator<Item = usize>) {
        self.minds = spawns
            .into_iter()
            .map(|si| calm_mind(content, si))
            .collect();
        self.ways = vec![Way::default(); self.minds.len()];
        self.shouts.clear();
    }

    /// A calm mind, and no way yet, for one more enemy placed from spawn line `spawn`, beside the
    /// others in the world's enemies. Nothing of the others is touched.
    pub(crate) fn add(&mut self, content: &Content, spawn: usize) {
        self.minds.push(calm_mind(content, spawn));
        self.ways.push(Way::default());
    }

    /// Whether there is a mind and a way for each of `n` enemies.
    pub(crate) fn rows_match(&self, n: usize) -> bool {
        self.minds.len() == n && self.ways.len() == n
    }
}

/// A calm mind for an enemy placed from spawn line `spawn`: a sentry's wide front cone, a sleeper's
/// closed eyes, everyone else's own cone.
fn calm_mind(content: &Content, spawn: usize) -> Mind {
    let r = &content.rules.senses;
    match content.spawns[spawn].behaviour {
        Behaviour::Sentry { .. } => Mind::new(r.sentry_cone, false),
        Behaviour::Sleeper { .. } => Mind::new(r.cone, true),
        _ => Mind::new(r.cone, false),
    }
}

/// A hero takes its player's input: the gait, and the Stealth presses since the last input,
/// counted round so that a press is never lost when one input replaces another between steps.
/// Only a class that can stealth does. Stepping into stealth also needs `may_stealth`: not in
/// combat, Stealth not cooling down; stepping out is always allowed. Returns whether a press to
/// step in was turned down, for the host to tell its player.
pub(crate) fn take_input(
    s: &mut Senses,
    hero: &mut Hero,
    input: &PlayerInput,
    content: &Content,
    may_stealth: bool,
) -> bool {
    let i = s.hero_index(hero.player);
    let hs = &mut s.heroes[i];
    hs.gait = input.gait;
    let pressed = hs
        .presses
        .map_or(0, |last| input.stealth_presses.wrapping_sub(last));
    hs.presses = Some(input.stealth_presses);
    if pressed.is_multiple_of(2) || !content.classes[hero.class].stealth {
        return false;
    }
    if hero.stealthed || may_stealth {
        hero.stealthed = !hero.stealthed;
        return false;
    }
    true
}

/// What follows from a hero's gait, Stealth, place and flash: their stance, how visible they are
/// and how fast they go. [`step_heroes`] works it out each step, and whatever changes one of its
/// inputs between steps ([`World::break_stealth`], [`World::flash`]) works it out again at once, so
/// nothing is judged or sent on the old values.
fn derive(hs: &HeroSense, hero: &mut Hero, content: &Content) {
    let r = &content.rules.senses;
    // The downed and the dead stand still whatever their client says: no footsteps, and a noise
    // ring that only runs down.
    let gait = if hero.fight.up() {
        hs.gait
    } else {
        Gait::Still
    };
    hero.stance = Stance::of(gait, hero.stealthed);
    let lit_area = content
        .map
        .lit
        .iter()
        .any(|l| l.at.distance(hero.pos) <= l.radius);
    let light = Light {
        // Always false here: a lit lantern's factor (its own, else the rules') is applied below,
        // so `visibility` is not asked to apply the rules' factor on top of it.
        lantern: false,
        lit_area,
        flash: hs.flash > 0.0,
    };
    hero.visibility = visibility(hero.stance, light, r);
    // A lit lantern (spec §5.1): its own factor, else the rules' (×1.6). Every factor is 1
    // or more, so clamping last equals clamping once.
    if let Some(def) = hero
        .carry
        .lantern
        .and_then(|i| content.items.get(usize::from(i)))
    {
        let factor = def.light.and_then(|l| l.seen).unwrap_or(r.lantern);
        hero.visibility = (hero.visibility * factor).min(100.0);
    }
    hero.speed = base_speed(hero, content);
}

/// How fast a hero goes by their class, Stealth and the weight they carry alone (spec §8.2:
/// weight slows walking and sneaking alike): what the fight's Sprint, slowed casts and crawl are
/// laid on ([`World::set_pace`]). The weight is here, not laid on after, because `set_pace`
/// rebuilds the speed from this each step and between steps.
pub(crate) fn base_speed(hero: &Hero, content: &Content) -> f32 {
    let class = &content.classes[hero.class];
    let (pace, _) = crate::burden(hero.carry.load, &content.rules.run);
    class.speed
        * if hero.stealthed {
            content.rules.senses.stealth_speed
        } else {
            1.0
        }
        * pace
}

/// Every hero's stance, visibility and speed for this step, and their footsteps: one every
/// `steps.every` seconds of moving, the first at once, as loud as the stance. The rings remember
/// the loudest noise of the last `noise_memory` seconds.
pub(crate) fn step_heroes(s: &mut Senses, heroes: &mut [Hero], content: &Content) {
    let r = &content.rules.senses;
    for hero in heroes.iter_mut() {
        let i = s.hero_index(hero.player);
        let hs = &mut s.heroes[i];
        hs.flash = (hs.flash - DT).max(0.0);
        derive(hs, hero, content);
        for n in &mut hs.recent {
            n.0 += DT;
        }
        hs.recent.retain(|n| n.0 <= r.noise_memory);
        let mut step = None;
        if hero.stance.moving() {
            hs.step_in -= DT;
            if hs.step_in <= 0.0 {
                hs.step_in += r.steps.every;
                // A heavy hero's footsteps carry further (sneaking stays silent: 0 times
                // anything).
                let (_, loud) = crate::burden(hero.carry.load, &content.rules.run);
                let radius = step_radius(hero.stance, &r.steps) * loud;
                if radius > 0.0 {
                    hs.recent.push((0.0, radius));
                    step = Some(Noise {
                        by: hero.player,
                        at: V3::at(hero.pos, hero.z),
                        radius,
                    });
                }
            }
        } else {
            hs.step_in = 0.0;
        }
        hero.noise = hs.recent.iter().map(|n| n.1).fold(0.0, f32::max);
        if let Some(n) = step {
            s.noises.push(n);
        }
    }
}

impl World {
    /// What each enemy thinks of `player`'s hero, for the marks over their heads (spec §5.5). The
    /// host's: a client's mirror has no minds and gets its own player's marks from its host.
    pub fn marks_for(&self, player: PlayerId) -> Vec<Mark> {
        // Nobody out of the raid is shown what the enemies think.
        if self.is_parted(player) {
            return Vec::new();
        }
        let r = &self.content().rules.senses.awareness;
        self.enemies
            .iter()
            .zip(&self.senses.minds)
            .filter(|(e, _)| e.hp > 0.0)
            .filter_map(|(e, m)| m.mark(player, r).map(|alert| Mark { enemy: e.id, alert }))
            .collect()
    }

    /// What the first hit by `by`'s hero on enemy `enemy` is multiplied by (spec §5.4): `ambush`
    /// while the enemy is not alerted to that hero (alerted to another, it is still caught
    /// unawares by this one), else 1. Sub-project 4's damage calls it.
    pub fn ambush(&self, enemy: u32, by: PlayerId) -> f32 {
        match self.mind_of(enemy) {
            Some(m) if !m.alerted_to(by) => self.content().rules.senses.ambush,
            _ => 1.0,
        }
    }

    /// Whether `by`'s hero may open on enemy `enemy` (spec §5.4: Sap, Cheap Shot, Garrote and
    /// Ambush need it not alerted to them). An enemy that is not there takes no opener.
    pub fn opener_allowed(&self, enemy: u32, by: PlayerId) -> bool {
        self.mind_of(enemy).is_some_and(|m| !m.alerted_to(by))
    }

    /// The mind of enemy `enemy`, on the host.
    fn mind_of(&self, enemy: u32) -> Option<&Mind> {
        let i = self.enemies.iter().position(|e| e.id == enemy)?;
        self.senses.minds.get(i)
    }

    /// Enemy `enemy` is alerted to `by`'s hero where they stand, and shouts at the next step: the
    /// seam sub-project 4's hits use. Nothing happens for an enemy or a hero that is not there.
    pub fn alert(&mut self, enemy: u32, by: PlayerId) {
        let Some(at) = self.hero(by).map(|h| V3::at(h.pos, h.z)) else {
            return;
        };
        let Some(i) = self.enemies.iter().position(|e| e.id == enemy) else {
            return;
        };
        let content = self.content().clone();
        let Some(mind) = self.senses.minds.get_mut(i) else {
            return;
        };
        mind.alert(by, at, &content.rules.senses.awareness);
        self.senses.shouts.push((i, Shout { player: by, at }));
    }

    /// A noise `by`'s hero made at `at` (the floor it came from), as loud as `loudness` (spec
    /// §5.2): heard by the enemies at the next step and shown in the maker's rings. The seam the
    /// abilities, chests, graves and exits of later sub-projects make their noise through.
    ///
    /// Call it between steps, or inside [`World::step`] before the senses listen (before
    /// `senses::step_enemies`): `World::step` clears the noises once the enemies have listened, so a
    /// noise made after that, still inside the step, is dropped unheard.
    pub fn make_noise(&mut self, by: PlayerId, at: V3, loudness: Loudness) {
        if self.hero(by).is_none() || !at.is_finite() {
            return;
        }
        let radius = loudness.radius(&self.content().rules.senses.loudness);
        if !radius.is_finite() || radius <= 0.0 {
            return;
        }
        self.senses.noises.push(Noise { by, at, radius });
        let i = self.senses.hero_index(by);
        self.senses.heroes[i].recent.push((0.0, radius));
    }

    /// A fire spell's flash on `player`'s hero (spec §5.1): visibility ×`flash` for `flash_for`
    /// seconds. Sub-project 4's fire spells call it.
    pub fn flash(&mut self, player: PlayerId) {
        if self.hero(player).is_none() {
            return;
        }
        let secs = self.content().rules.senses.flash_for;
        let i = self.senses.hero_index(player);
        self.senses.heroes[i].flash = secs;
        self.rederive(player);
    }

    /// `player`'s hero drops out of stealth (spec §6.3: attacking or taking damage breaks it); their
    /// stance, visibility and speed change at once, so the hit that broke it is judged and sent as
    /// the unstealthed hero's. Sub-project 4's combat calls it.
    pub fn break_stealth(&mut self, player: PlayerId) {
        if let Some(h) = self.heroes.iter_mut().find(|h| h.player == player) {
            h.stealthed = false;
            self.rederive(player);
        }
    }

    /// `player`'s hero goes into stealth (Vanish), their stance, visibility and speed changing at
    /// once like [`World::break_stealth`]'s, so nothing is judged or sent on the old values.
    pub fn enter_stealth(&mut self, player: PlayerId) {
        if let Some(h) = self.heroes.iter_mut().find(|h| h.player == player) {
            h.stealthed = true;
            self.rederive(player);
        }
    }

    /// `player`'s hero's stance, visibility and speed again, after something between steps changed
    /// what they follow from. The fight's pace goes back on top ([`World::set_pace`]), so a hero's
    /// Sprint, slowed cast or crawl is not lost until the next step.
    pub(crate) fn rederive(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let i = self.senses.hero_index(player);
        let Some(hi) = self.heroes.iter().position(|h| h.player == player) else {
            return;
        };
        derive(&self.senses.heroes[i], &mut self.heroes[hi], &content);
        self.set_pace(hi);
    }
}
