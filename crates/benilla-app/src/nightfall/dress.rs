//! Clothes and weapons for a player-hero. A creature wears display ids straight off its fields; a
//! player wears item *entries* (`PLAYER_VISIBLE_ITEM_*`), which the client turns into looks
//! through item templates a server would send. So each piece gets a stand-in template carrying
//! just its look, filed in [`Items`] as a server answer would be.

use benilla_protocol::messages::ItemInfo;
use bevy::prelude::*;

use super::units::{UnitKind, UnitSpec, Weapon};
use crate::items::Items;

/// `PLAYER_VISIBLE_ITEM_1_CREATOR`: each equipment slot has 12 fields, the entry third.
const FIELD_VISIBLE_ITEM_BASE: u16 = 258;
const VISIBLE_ITEM_STRIDE: u16 = 12;

/// Stand-in entries live far above any real item's: base + slot × 100 000 + display id.
const STAND_IN_ENTRY_BASE: u32 = 0x7000_0000;

/// The equipment slot a worn armour piece of `inventory_type` goes in; `None` for weapons (the
/// class's own come from the content) and for what never shows (rings, trinkets, bags).
fn armour_slot(inventory_type: u8) -> Option<u8> {
    Some(match inventory_type {
        1 => 0,      // head
        3 => 2,      // shoulders
        4 => 3,      // shirt
        5 | 20 => 4, // chest, robe
        6 => 5,      // waist
        7 => 6,      // legs
        8 => 7,      // feet
        9 => 8,      // wrists
        10 => 9,     // hands
        16 => 14,    // back
        19 => 18,    // tabard
        _ => return None,
    })
}

/// Weapon slot (0 main hand, 1 off hand, 2 ranged) to equipment slot.
const WEAPON_SLOTS: [u8; 3] = [15, 16, 17];

/// One piece to wear: its equipment slot and look.
pub(super) struct Piece {
    pub slot: u8,
    pub display: u32,
    pub inventory_type: u32,
    pub class: u32,
    pub subclass: u32,
    pub material: u32,
    pub sheath: u32,
}

impl Piece {
    /// Cloth armour in `slot` showing `display`.
    fn armour(slot: u8, inventory_type: u32, display: u32) -> Self {
        Self {
            slot,
            display,
            inventory_type,
            class: 4,
            subclass: 1,
            material: 7,
            sheath: 0,
        }
    }
}

/// The pieces for a starting outfit (`CharStartOutfit.dbc`, the clothes a new character of the
/// race, class and sex wears) and the class's weapons.
pub(super) fn pieces(
    outfit: &[benilla_formats::StartOutfitItem],
    weapons: &[nightfall_sim::WeaponDef],
) -> Vec<Piece> {
    let mut out: Vec<Piece> = outfit
        .iter()
        .filter_map(|item| {
            armour_slot(item.inv_type)
                .map(|slot| Piece::armour(slot, u32::from(item.inv_type), item.display_id))
        })
        .collect();
    for w in weapons {
        let Some(&slot) = WEAPON_SLOTS.get(usize::from(w.slot)) else {
            continue;
        };
        out.push(Piece {
            slot,
            display: w.display,
            inventory_type: u32::from(w.inventory_type),
            class: u32::from(w.class),
            subclass: u32::from(w.subclass),
            material: u32::from(w.material),
            sheath: u32::from(w.sheath),
        });
    }
    out
}

/// A player body of `race` and `sex` (its `ChrRaces` display), in the class's starting outfit and
/// holding the class's weapons. The hero and the menu's preview are both this.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_player_body(
    commands: &mut Commands,
    items: &mut Items,
    char_create: &crate::entities::CharCreate,
    class: &nightfall_sim::ClassDef,
    (race, sex): (u8, u8),
    serial: u64,
    level: u32,
    pos: Vec3,
    yaw: f32,
) -> Option<Entity> {
    let display = char_create.0.body_display(race, sex)?;
    let outfit = char_create.0.start_outfit(race, class.class_id, sex);
    let worn = wear(items, &pieces(outfit, &class.weapons));
    let weapons: Vec<Weapon> = class
        .weapons
        .iter()
        .map(|w| {
            (
                w.slot,
                w.display,
                w.class,
                w.subclass,
                w.material,
                w.inventory_type,
                w.sheath,
            )
        })
        .collect();
    Some(super::units::spawn_unit(
        commands,
        &UnitSpec {
            kind: UnitKind::Player,
            serial,
            display,
            scale: 1.0,
            level,
            faction: super::units::FACTION_FRIENDLY,
            bytes0: u32::from(race)
                | u32::from(class.class_id) << 8
                | u32::from(sex) << 16
                | u32::from(class.power) << 24,
            weapons: &weapons,
            sheath: class.sheath,
            extra: &worn,
        },
        pos,
        yaw,
    ))
}

/// File each piece's stand-in template and return the `PLAYER_VISIBLE_ITEM_*` field pairs that
/// wear them.
pub(super) fn wear(items: &mut Items, pieces: &[Piece]) -> Vec<(u16, u32)> {
    pieces
        .iter()
        .map(|p| {
            let entry = STAND_IN_ENTRY_BASE + u32::from(p.slot) * 100_000 + p.display;
            if items.template_cached(entry).is_none() {
                items.insert_template(entry, Some(stand_in(p)));
            }
            (
                FIELD_VISIBLE_ITEM_BASE + 2 + VISIBLE_ITEM_STRIDE * u16::from(p.slot),
                entry,
            )
        })
        .collect()
}

/// A template with nothing but a piece's look.
fn stand_in(p: &Piece) -> ItemInfo {
    ItemInfo {
        class: p.class,
        subclass: p.subclass,
        name: String::new(),
        display_info_id: p.display,
        quality: 1,
        flags: 0,
        buy_price: 0,
        sell_price: 0,
        inventory_type: p.inventory_type,
        allowable_class: -1,
        allowable_race: -1,
        item_level: 1,
        required_level: 0,
        required_skill: 0,
        required_skill_rank: 0,
        required_spell: 0,
        required_honor_rank: 0,
        required_city_rank: 0,
        required_rep_faction: 0,
        required_rep_rank: 0,
        max_count: 0,
        stackable: 1,
        container_slots: 0,
        stats: Vec::new(),
        damages: Vec::new(),
        dmg_min: 0.0,
        dmg_max: 0.0,
        dmg_type: 0,
        armor: 0,
        resistances: [0; 6],
        delay_ms: 0,
        ammo_type: 0,
        ranged_mod_range: 0.0,
        spells: Vec::new(),
        spell_charges_0: 0,
        use_spell: None,
        bonding: 0,
        description: String::new(),
        page_text: 0,
        language_id: 0,
        page_material: 0,
        start_quest: 0,
        lock_id: 0,
        material: p.material,
        sheath: p.sheath,
        random_property: 0,
        block: 0,
        item_set: 0,
        max_durability: 0,
        area: 0,
        map: 0,
        bag_family: 0,
    }
}
