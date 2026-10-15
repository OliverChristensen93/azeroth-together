//! WoW's action bar and spellbook, filled with the hero's abilities as a server's
//! `SMSG_INITIAL_SPELLS` and `SMSG_ACTION_BUTTONS` would fill them, with each auto-cast starting
//! its button's cooldown sweep as an `SMSG_SPELL_COOLDOWN` would.
//!
//! Abilities cast themselves, so a button press has nothing to ask: the client's cast request goes
//! to no server, and its pending cast is dropped at once rather than left waiting for an answer.

use benilla_protocol::messages::{ActionButton, ACTION_KIND_SPELL};
use bevy::platform::time::Instant;
use bevy::prelude::*;

use super::Sim;
use crate::spell::{Cooldowns, PendingCast};
use crate::ui_action::PlayerActions;

/// The local hero's abilities, one per button from the first, and in the spellbook. Rewritten
/// only when they change (a card learned one).
pub(super) fn fill_action_bar(
    sim: Res<Sim>,
    mut shown: Local<Vec<u32>>,
    mut actions: ResMut<PlayerActions>,
) {
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let content = sim.world.content();
    let spells: Vec<u32> = hero
        .abilities
        .iter()
        .map(|o| content.abilities[o.ability].visual)
        .collect();
    if *shown == spells {
        return;
    }
    actions.buttons.clear();
    actions.spells.clear();
    for (slot, &spell) in spells.iter().enumerate() {
        let slot = slot as u8;
        actions.buttons.insert(
            slot,
            ActionButton {
                slot,
                action: spell,
                kind: ACTION_KIND_SPELL,
            },
        );
        actions.spells.insert(spell);
    }
    actions.dirty = true;
    *shown = spells;
}

/// An auto-cast's cooldown on its button: the ability's own, with no global cooldown on the rest.
pub(super) fn start_cooldown(cooldowns: &mut Cooldowns, spell: u32, seconds: f32) {
    let ms = (seconds * 1000.0).round().max(0.0) as u32;
    cooldowns.apply_wire_cooldown(spell, ms, None, Instant::now());
}

/// A pressed button's cast request has no server to answer it: drop it, so the next press (and
/// the card keys, which share 1 to 3 with the bar) is not held behind it.
pub(super) fn drop_manual_casts(mut pending: ResMut<PendingCast>) {
    if pending.current(Instant::now()).is_some() {
        *pending = PendingCast::default();
    }
}
