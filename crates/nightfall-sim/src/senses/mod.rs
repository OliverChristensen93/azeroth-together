//! The senses (spec §5): how visible and how loud each hero is, what each enemy makes of them,
//! and what it does about it. The host's alone: a client's mirror gets each hero's visibility and
//! noise in the snapshot, and its own player's marks beside it.
//!
//! Each step, once the heroes have their inputs, [`step_heroes`] works out every hero's stance,
//! visibility, speed and footsteps.

mod awareness;
mod noise;
#[cfg(test)]
mod tests;
mod visibility;

pub use awareness::{Alert, Mark};
pub use noise::Loudness;
pub(crate) use noise::Noise;
pub use visibility::{Gait, Stance};

use noise::step_radius;
use visibility::{visibility, Light};

use crate::{Content, Hero, PlayerId, PlayerInput, World, DT, V3};

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

    /// A player left: nothing of their hero stays.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.noises.retain(|n| n.by != player);
    }
}

/// A hero takes its player's input: the gait, and the Stealth presses since the last input,
/// counted round so that a press is never lost when one input replaces another between steps.
/// Only a class that can stealth does.
pub(crate) fn take_input(s: &mut Senses, hero: &mut Hero, input: &PlayerInput, content: &Content) {
    let i = s.hero_index(hero.player);
    let hs = &mut s.heroes[i];
    hs.gait = input.gait;
    let pressed = hs
        .presses
        .map_or(0, |last| input.stealth_presses.wrapping_sub(last));
    hs.presses = Some(input.stealth_presses);
    if pressed % 2 == 1 && content.classes[hero.class].stealth {
        hero.stealthed = !hero.stealthed;
    }
}

/// Every hero's stance, visibility and speed for this step, and their footsteps: one every
/// `steps.every` seconds of moving, the first at once, as loud as the stance. The rings remember
/// the loudest noise of the last `noise_memory` seconds.
pub(crate) fn step_heroes(s: &mut Senses, heroes: &mut [Hero], content: &Content) {
    let r = &content.rules.senses;
    for hero in heroes.iter_mut() {
        let i = s.hero_index(hero.player);
        let hs = &mut s.heroes[i];
        hero.stance = Stance::of(hs.gait, hero.stealthed);
        hs.flash = (hs.flash - DT).max(0.0);
        let lit_area = content
            .map
            .lit
            .iter()
            .any(|l| l.at.distance(hero.pos) <= l.radius);
        let light = Light {
            lantern: false,
            lit_area,
            flash: hs.flash > 0.0,
        };
        hero.visibility = visibility(hero.stance, light, r);
        let class = &content.classes[hero.class];
        hero.speed = class.speed * if hero.stealthed { r.stealth_speed } else { 1.0 };
        for n in &mut hs.recent {
            n.0 += DT;
        }
        hs.recent.retain(|n| n.0 <= r.noise_memory);
        let mut step = None;
        if hero.stance.moving() {
            hs.step_in -= DT;
            if hs.step_in <= 0.0 {
                hs.step_in += r.steps.every;
                let radius = step_radius(hero.stance, &r.steps);
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
    /// A noise `by`'s hero made at `at` (the floor it came from), as loud as `loudness` (spec
    /// §5.2): heard by the enemies at the next step and shown in the maker's rings. The seam the
    /// abilities, chests, graves and exits of later sub-projects make their noise through.
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
    }

    /// `player`'s hero drops out of stealth (spec §6.3: attacking or taking damage breaks it);
    /// their speed comes back at the next step. Sub-project 4's combat calls it.
    pub fn break_stealth(&mut self, player: PlayerId) {
        if let Some(h) = self.heroes.iter_mut().find(|h| h.player == player) {
            h.stealthed = false;
        }
    }
}
