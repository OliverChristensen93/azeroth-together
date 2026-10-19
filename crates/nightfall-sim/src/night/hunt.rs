//! The hunt (spec §7.2: "worgen packs hunt toward the loudest recent noise"): while a hunting
//! stage lasts, every `hunt.every` seconds each hunter that is calm, free of the fight and within
//! `hunt.reach` yards of the loudest noise of the last `hunt.memory` seconds runs there, looks
//! about for `hunt.rest` seconds and goes home. What it finds there, its senses deal with.

use crate::combat::Hold;
use crate::{World, DT, V3};

/// The loudest noise heard lately, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Loud {
    pub(crate) at: V3,
    pub(crate) radius: f32,
    pub(crate) t: f32,
}

impl World {
    /// (A quieter noise that comes while a louder one is still remembered is lost for good.)
    /// The step's noises, kept when one is the loudest of late. It must run before the enemies'
    /// senses take the step's noises (`step_enemies` empties the list), and after every noise of
    /// the step is made. One older than the memory is forgotten.
    pub(crate) fn night_listen(&mut self) {
        if self.night.mirror.is_some() {
            return;
        }
        let memory = self.content().rules.night.hunt.memory;
        let now = self.elapsed;
        if self.night.loudest.is_some_and(|l| now - l.t > memory) {
            self.night.loudest = None;
        }
        for n in &self.senses.noises {
            if self.night.loudest.is_none_or(|l| n.radius >= l.radius) {
                self.night.loudest = Some(Loud {
                    at: n.at,
                    radius: n.radius,
                    t: now,
                });
            }
        }
    }

    /// The hunt's call, while the stage hunts: calm hunters near the loudest noise go to it.
    pub(crate) fn step_hunt(&mut self) {
        let content = self.content().clone();
        let n = &content.rules.night;
        let hunting = n.stages.get(self.night.stage).is_some_and(|s| s.hunt);
        if !hunting {
            self.night.hunt_in = 0.0;
            return;
        }
        self.night.hunt_in -= DT;
        if self.night.hunt_in > 0.0 {
            return;
        }
        self.night.hunt_in = n.hunt.every;
        let Some(loud) = self.night.loudest else {
            return;
        };
        if self.elapsed - loud.t > n.hunt.memory {
            return;
        }
        for i in 0..self.enemies.len().min(self.brains.len()) {
            let e = &self.enemies[i];
            let free = self
                .combat
                .enemies
                .get(i)
                .is_none_or(|c| c.engage.hold == Hold::Free);
            let calm = self.senses.is_calm(i);
            if content.enemies[e.kind].hunter
                && e.hp > 0.0
                && !e.asleep
                && free
                && calm
                && e.pos.distance(loud.at.ground()) <= n.hunt.reach
            {
                self.brains[i].send(loud.at);
            }
        }
    }
}
