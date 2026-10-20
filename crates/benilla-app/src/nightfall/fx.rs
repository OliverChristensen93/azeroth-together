//! The messages that make the host's fight look like a server's: `SMSG_SPELL_START`'s precast
//! glow, `SMSG_SPELL_GO`'s cast kit with a missile or an impact at each target, a failed cast, a
//! melee swing with its flinch and sound, and floating combat text. Copied from `together::units`'
//! `Fx`, which this game does not depend on, with the start and the failure added.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::combat_text::CombatTextSpawn;
use crate::creature_anim::{CastEvent, CastEventKind, PlaySeq, SpellGoTargets, SwingMessage};

/// More impacts than this per cast only cost frames.
const MAX_VISUAL_HITS: usize = 12;

/// A spell's damage, 0xAARRGGBB: yellow.
pub(super) const COLOR_SPELL: u32 = 0xFFFF_DE00;
/// A blow's damage: white.
pub(super) const COLOR_HIT: u32 = 0xFFFF_FFFF;
/// Damage to our own hero: red.
pub(super) const COLOR_HURT: u32 = 0xFFFF_3030;
/// A telegraphed blow that missed: pale blue.
pub(super) const COLOR_EVADE: u32 = 0xFF9F_D0FF;
/// A heal's number: green.
pub(super) const COLOR_HEAL: u32 = 0xFF40_FF40;

#[derive(SystemParam)]
pub(super) struct Fx<'w> {
    seq: ResMut<'w, PlaySeq>,
    casts: MessageWriter<'w, CastEvent>,
    gos: MessageWriter<'w, SpellGoTargets>,
    swings: MessageWriter<'w, SwingMessage>,
    text: MessageWriter<'w, CombatTextSpawn>,
}

impl Fx<'_> {
    /// `SMSG_SPELL_START`: the precast kit on the caster, held until the cast goes or fails.
    pub fn start(&mut self, caster: Entity, spell: u32) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Start,
            seq,
        });
    }

    /// A cast that died without going off.
    pub fn fail(&mut self, caster: Entity, spell: u32) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Fail,
            seq,
        });
    }

    /// `SMSG_SPELL_GO`: the cast kit on the caster, and a missile or an impact at each hit.
    pub fn go(&mut self, caster: Entity, spell: u32, hits: &[Entity], dest: Option<Vec3>) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Go,
            seq,
        });
        self.gos.write(SpellGoTargets {
            caster,
            spell_id: spell,
            hits: hits.iter().copied().take(MAX_VISUAL_HITS).collect(),
            misses: Vec::new(),
            dest,
            ammo_display_id: None,
            seq,
        });
    }

    /// A melee swing: the attack animation, the victim's flinch, blood and the hit sound, and
    /// WoW's own portrait feedback for `damage` (a swing that did none reads "MISS" there). Its
    /// floating number comes with the blow's own damage event.
    pub fn swing(&mut self, attacker: Entity, victim: Entity, damage: f32) {
        let seq = self.seq.next();
        self.swings.write(SwingMessage {
            attacker,
            victim: Some(victim),
            hit_info: 0x2,
            victim_state: 1,
            // Rounded as the floating number is, so the portrait and the number agree.
            damage: if damage > 0.0 {
                damage.round().max(1.0) as u32
            } else {
                0
            },
            displayed: false,
            seq,
        });
    }

    /// A floating number over `anchor`, 0xAARRGGBB; a crit bigger.
    pub fn number(&mut self, anchor: Entity, amount: f32, crit: bool, color: u32) {
        let shown = amount.round().max(1.0) as i64;
        self.text.write(CombatTextSpawn {
            anchor,
            text: shown.to_string(),
            category: if crit { 2 } else { 0 },
            color: Some(color),
        });
    }

    /// Floating words over `anchor`, 0xAARRGGBB.
    pub fn text(&mut self, anchor: Entity, text: String, color: u32) {
        self.text.write(CombatTextSpawn {
            anchor,
            text,
            category: 0,
            color: Some(color),
        });
    }
}
