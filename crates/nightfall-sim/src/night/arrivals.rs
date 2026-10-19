//! Who is out tonight (spec §7.2, §7.3, §8.1): the enemies a raid begins with (the content's
//! spawn lines of the first stage, a signature monster's only when the raid rolled it out), the
//! newcomers each later stage brings, placed once no up hero is within `arrive_unseen` yards of
//! where they come, and the risk zones' might in every enemy blow.
//!
//! Newcomers join through the same seams as any enemy that comes mid-raid: a mind
//! (`Senses::add`) and a fight (`Combat::add_enemy`) beside the enemy and its brain, so every
//! list stays as long as the enemies (`World::rows_match`).

use crate::enemies::{self, Brain};
use crate::{UnitRef, World, DT};

/// Seconds between two tries at placing the newcomers who wait.
const TRY_EVERY: f32 = 1.0;

impl World {
    /// Every enemy this raid begins with, on the attached map; any placed before start over.
    /// Nothing happens without a map (a client's mirror).
    pub(crate) fn place_enemies(&mut self) {
        let Some(map) = self.map().cloned() else {
            return;
        };
        let content = self.content().clone();
        let out = self.raid.roll.monsters.clone();
        let (enemies, brains, warnings) = enemies::spawn_all(
            &content,
            &map,
            |si| enemies::from_start(&content, si, &out),
            1,
        );
        self.enemies = enemies;
        self.brains = brains;
        // Placed from both `attach_map` and `begin_raid`: a content mistake is told once.
        self.warnings.clear();
        self.warnings.extend(warnings);
        // Every enemy starts calm, with the eyes its behaviour gives it.
        self.senses
            .place(&content, self.brains.iter().map(Brain::spawn));
        self.combat.place(&content, &self.enemies);
        self.night.waiting.clear();
    }

    /// Stage `stage` has landed: its spawn lines that this raid wants wait to come.
    pub(crate) fn stage_landed(&mut self, stage: usize) {
        let content = self.content().clone();
        let out = &self.raid.roll.monsters;
        for si in 0..content.spawns.len() {
            if content.spawns[si].stage_at == Some(stage) && enemies::wanted(&content, si, out) {
                self.night.waiting.push(si);
            }
        }
        self.night.try_in = 0.0;
    }

    /// Every second, each waiting line whose place no up hero is near comes.
    pub(crate) fn step_arrivals(&mut self) {
        if self.night.waiting.is_empty() || self.map().is_none() {
            return;
        }
        self.night.try_in -= DT;
        if self.night.try_in > 0.0 {
            return;
        }
        self.night.try_in = TRY_EVERY;
        let content = self.content().clone();
        let clear = content.rules.night.arrive_unseen;
        // Only the line's anchor is tested; a patrol or wanderer's members may land a little
        // nearer a hero than `arrive_unseen` (known, small: the spread of one line).
        for si in std::mem::take(&mut self.night.waiting) {
            let at = enemies::anchor(&content.spawns[si]);
            let near = self
                .heroes
                .iter()
                .any(|h| h.fight.up() && h.pos.distance(at) <= clear);
            if near {
                self.night.waiting.push(si);
            } else {
                self.add_line(si);
            }
        }
    }

    /// Spawn line `si`'s enemies join the raid, with a mind and a fight each, ids after every
    /// enemy's so far (the dead stay in the lists, so an id is never given twice).
    fn add_line(&mut self, si: usize) {
        let Some(map) = self.map().cloned() else {
            return;
        };
        let content = self.content().clone();
        let first = self.enemies.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let (enemies, brains, warnings) = enemies::spawn_all(&content, &map, |i| i == si, first);
        self.warnings.extend(warnings);
        for (e, b) in enemies.into_iter().zip(brains) {
            self.senses.add(&content, b.spawn());
            self.combat.add_enemy(&content, &e);
            self.enemies.push(e);
            self.brains.push(b);
        }
    }

    /// What blows from `by` are times: an enemy's zone's `damage` (spec §8.1), else 1.
    pub(crate) fn might_of(&self, by: Option<UnitRef>) -> f32 {
        match by {
            Some(UnitRef::Enemy(id)) => self
                .enemies
                .iter()
                .position(|e| e.id == id)
                .and_then(|i| self.brains.get(i))
                .map_or(1.0, Brain::might),
            _ => 1.0,
        }
    }
}
