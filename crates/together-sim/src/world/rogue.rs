//! The rogue's capstones (docs/ROGUE-DRAFT.md), called from the hooks in `classes.rs`.

use super::combat::Death;
use super::*;

impl World {
    /// The rogue capstones that change a cast with the hero's state.
    pub(super) fn rogue_spec(&self, _h: usize, _owned: &Owned, _s: &mut CastSpec) {}

    /// How fast a rogue's spell comes back beside its buffs.
    pub(super) fn rogue_rate(&self, _h: usize, _owned: &Owned) -> f32 {
        1.0
    }

    /// Before a rogue spell fires on its own cooldown.
    pub(super) fn rogue_before(&mut self, _h: usize, _owned: &Owned, _spec: &mut CastSpec) {}

    /// A Finisher of hero `h` spent `spent` combo points.
    pub(super) fn rogue_spent(&mut self, _h: usize, _owned: &Owned, _spec: &CastSpec, _spent: u8) {}

    /// How much harder a rogue's hit lands.
    pub(super) fn rogue_scale(&self, _h: usize, _ei: usize, _spec: &CastSpec) -> f32 {
        1.0
    }

    /// A hit of a rogue's spell `spec` on enemy `id`.
    pub(super) fn rogue_strike(
        &mut self,
        _h: usize,
        _id: u32,
        _spec: &CastSpec,
        _damage: f32,
        _crit: bool,
        _scale: f32,
    ) {
    }

    /// A killing blow at a rogue, turned aside by a capstone. Whether it was.
    pub(super) fn rogue_cheat(&mut self, _h: usize) -> bool {
        false
    }

    /// Each step for a rogue.
    pub(super) fn rogue_tend(&mut self, _h: usize) {}

    /// What dying enemies set off for rogues.
    pub(super) fn rogue_deaths(&mut self, _deaths: &[Death]) {}

    /// A rogue's nova hit `hit`.
    pub(super) fn rogue_nova(&mut self, _h: usize, _spec: &CastSpec, _center: V2, _hit: &[u32]) {}
}
