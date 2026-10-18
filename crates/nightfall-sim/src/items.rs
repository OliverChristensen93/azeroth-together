//! Nightfall's things (spec §8.2) as content: what an item is, weighs and does, how it looks worn,
//! and the loot tables, containers and bodies that hold items on the map. Read from
//! `content/nightfall/items.ron` and `loot.ron`, and checked as they load, so a mistake is a clear
//! line at startup rather than a chest that never opens.

use serde::{Deserialize, Serialize};

use crate::content::{non_negative, positive};
use crate::{Bounds, EnemyDef, Loudness, V2};

/// What an item is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon,
    Armour,
    /// Used from the backpack: health or mana at once.
    Potion,
    /// Eaten by Eat and Drink: health over the meal.
    Food,
    /// Drunk by Eat and Drink: mana over the meal.
    Drink,
    /// Worth gold, and nothing else.
    Valuable,
    /// Opens a locked container, and is kept.
    Key,
    /// Worn in a bag slot: more backpack slots.
    Bag,
    /// Lit from the backpack: light, and a hero easier to see (spec §5.1).
    Lantern,
    /// Kept for the inn's stations (sub-project 5b).
    Material,
}

/// Where a piece of gear is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Head,
    Shoulders,
    Chest,
    Legs,
    Feet,
    Hands,
    Back,
    MainHand,
    OffHand,
}

impl EquipSlot {
    pub const COUNT: usize = 9;
    pub const ALL: [EquipSlot; 9] = [
        EquipSlot::Head,
        EquipSlot::Shoulders,
        EquipSlot::Chest,
        EquipSlot::Legs,
        EquipSlot::Feet,
        EquipSlot::Hands,
        EquipSlot::Back,
        EquipSlot::MainHand,
        EquipSlot::OffHand,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// WoW's equipment slot, which `PLAYER_VISIBLE_ITEM_*` is indexed by.
    pub fn wow_slot(self) -> u8 {
        match self {
            EquipSlot::Head => 0,
            EquipSlot::Shoulders => 2,
            EquipSlot::Chest => 4,
            EquipSlot::Legs => 6,
            EquipSlot::Feet => 7,
            EquipSlot::Hands => 9,
            EquipSlot::Back => 14,
            EquipSlot::MainHand => 15,
            EquipSlot::OffHand => 16,
        }
    }

    /// The slot's name, for the panels.
    pub fn name(self) -> &'static str {
        match self {
            EquipSlot::Head => "Head",
            EquipSlot::Shoulders => "Shoulders",
            EquipSlot::Chest => "Chest",
            EquipSlot::Legs => "Legs",
            EquipSlot::Feet => "Feet",
            EquipSlot::Hands => "Hands",
            EquipSlot::Back => "Back",
            EquipSlot::MainHand => "Main hand",
            EquipSlot::OffHand => "Off hand",
        }
    }
}

fn armour_class() -> u8 {
    4
}

fn cloth() -> u8 {
    1
}

fn cloth_material() -> u8 {
    7
}

/// How a worn item looks on the hero (spec §8.2: "each with a WoW item display").
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum Look {
    /// An `ItemDisplayInfo.dbc` id with the template fields a stand-in item needs
    /// (`nightfall::dress`), as the classes' weapons give them.
    Item {
        display: u32,
        inventory_type: u8,
        #[serde(default = "armour_class")]
        class: u8,
        #[serde(default = "cloth")]
        subclass: u8,
        #[serde(default = "cloth_material")]
        material: u8,
        #[serde(default)]
        sheath: u8,
    },
    /// The piece a new character of class `class_id` (`ChrClasses.dbc`) starts in for this
    /// item's slot (`CharStartOutfit.dbc`), in the hero's own race and sex: a look the data is
    /// sure to have. A slot that outfit leaves bare shows nothing.
    Outfit { class_id: u8 },
}

/// What worn gear adds (spec §6.1: power comes from gear).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Bonus {
    /// Health, added to the class's.
    pub health: f32,
    /// Damage, a fraction added to every blow and spell (0.1 is 10% more).
    pub damage: f32,
}

/// What a potion does when used.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum ItemUse {
    Heal(f32),
    Mana(f32),
}

/// A lit lantern's light.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct LightDef {
    /// Yards.
    pub range: f32,
    pub intensity: f32,
}

fn one_u8() -> u8 {
    1
}

fn one_u16() -> u16 {
    1
}

fn one_f32() -> f32 {
    1.0
}

/// One kind of thing.
#[derive(Clone, Debug, Deserialize)]
pub struct ItemDef {
    pub key: String,
    pub name: String,
    pub kind: ItemKind,
    /// The icon, `Interface\Icons\<icon>.blp`.
    pub icon: String,
    /// WoW's quality: 0 poor, 1 common, 2 uncommon, 3 rare, 4 epic.
    #[serde(default = "one_u8")]
    pub quality: u8,
    /// Each one's weight (spec §8.2).
    pub weight: f32,
    /// How many stack in one slot.
    #[serde(default = "one_u16")]
    pub stack: u16,
    /// Gold each, when sold.
    #[serde(default)]
    pub value: u32,
    /// Where it is worn (weapons and armour).
    #[serde(default)]
    pub slot: Option<EquipSlot>,
    #[serde(default)]
    pub look: Option<Look>,
    #[serde(default)]
    pub bonus: Bonus,
    /// A bag's slots.
    #[serde(default)]
    pub bag_slots: u8,
    /// Food and drink: the share of health or mana a meal of it gives back.
    #[serde(default = "one_f32")]
    pub restore: f32,
    /// A potion's effect.
    #[serde(default)]
    pub on_use: Option<ItemUse>,
    /// A lantern's light.
    #[serde(default)]
    pub light: Option<LightDef>,
    /// A line for the panels.
    #[serde(default)]
    pub text: String,
}

/// A kind of container the content places: how it looks, how long it takes to search and how
/// loud that is.
#[derive(Clone, Debug, Deserialize)]
pub struct ContainerKindDef {
    pub key: String,
    pub name: String,
    /// `GameObjectDisplayInfo.dbc` id.
    pub display: u32,
    #[serde(default = "one_f32")]
    pub scale: f32,
    /// Seconds to search it, standing by it.
    pub search: f32,
    /// How loud searching it is (spec §5.2: a chest 15 yards, a grave dug 30).
    pub sound: Loudness,
    #[serde(default = "eight")]
    pub slots: u8,
}

fn eight() -> u8 {
    8
}

fn one_one() -> (u16, u16) {
    (1, 1)
}

#[derive(Clone, Debug, Deserialize)]
struct TableEntryFile {
    item: String,
    weight: f32,
    #[serde(default = "one_one")]
    count: (u16, u16),
}

#[derive(Clone, Debug, Deserialize)]
struct TableFile {
    key: String,
    rolls: (u8, u8),
    #[serde(default)]
    nothing: f32,
    items: Vec<TableEntryFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct PlacedFile {
    kind: String,
    table: String,
    at: (f32, f32),
    #[serde(default)]
    z: Option<f32>,
    #[serde(default)]
    lock: Option<String>,
    #[serde(default)]
    place: String,
}

#[derive(Clone, Debug, Deserialize)]
struct BodyFile {
    enemy: String,
    table: String,
}

/// `loot.ron` as written.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LootFile {
    kinds: Vec<ContainerKindDef>,
    tables: Vec<TableFile>,
    containers: Vec<PlacedFile>,
    bodies: Vec<BodyFile>,
    pile_display: u32,
    remains_display: u32,
}

/// One line of a loot table: an item, how likely, how many.
#[derive(Clone, Debug, PartialEq)]
pub struct TableEntry {
    /// Index into [`crate::Content::items`].
    pub item: usize,
    pub weight: f32,
    pub count: (u16, u16),
}

/// A loot table: `rolls` picks, each an entry by weight, or nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub key: String,
    pub rolls: (u8, u8),
    pub nothing: f32,
    pub items: Vec<TableEntry>,
}

/// A container the content places on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedContainer {
    /// Index into [`LootContent::kinds`].
    pub kind: usize,
    /// Index into [`LootContent::tables`].
    pub table: usize,
    pub at: V2,
    /// Its height, when measured; without it, only the ground distance counts.
    pub z: Option<f32>,
    /// The key item it needs, an index into the items.
    pub lock: Option<usize>,
    /// Where it is, for the content's readers.
    pub place: String,
}

/// The loot of a map, resolved.
#[derive(Clone, Debug, Default)]
pub struct LootContent {
    pub kinds: Vec<ContainerKindDef>,
    pub tables: Vec<Table>,
    pub containers: Vec<PlacedContainer>,
    /// (enemy kind, table): what that kind's body holds.
    pub bodies: Vec<(usize, usize)>,
    /// `GameObjectDisplayInfo.dbc` ids for things dropped on the ground and for a hero's remains.
    pub pile_display: u32,
    pub remains_display: u32,
}

/// Every item's numbers and kind-specific parts make sense.
pub(crate) fn check_items(items: &[ItemDef]) -> Result<(), String> {
    for (i, it) in items.iter().enumerate() {
        let k = &it.key;
        if items[..i].iter().any(|o| o.key == *k) {
            return Err(format!("items.ron: item \"{k}\" is listed twice"));
        }
        if !non_negative(it.weight) || it.stack == 0 {
            return Err(format!(
                "items.ron: \"{k}\": weight is 0 or more and stack at least 1"
            ));
        }
        let gear = matches!(it.kind, ItemKind::Weapon | ItemKind::Armour);
        if gear != it.slot.is_some() {
            return Err(format!(
                "items.ron: \"{k}\": weapons and armour have a slot, nothing else does"
            ));
        }
        let one = gear || matches!(it.kind, ItemKind::Bag | ItemKind::Lantern);
        if one && it.stack != 1 {
            return Err(format!(
                "items.ron: \"{k}\": gear, bags and lanterns do not stack"
            ));
        }
        if (it.kind == ItemKind::Bag) != (it.bag_slots > 0) {
            return Err(format!(
                "items.ron: \"{k}\": a bag has slots, nothing else does"
            ));
        }
        if it.kind == ItemKind::Lantern && it.light.is_none() {
            return Err(format!("items.ron: \"{k}\": a lantern needs its light"));
        }
        if it.kind == ItemKind::Potion && it.on_use.is_none() {
            return Err(format!("items.ron: \"{k}\": a potion needs on_use"));
        }
        if matches!(it.kind, ItemKind::Food | ItemKind::Drink)
            && !(positive(it.restore) && it.restore <= 1.0)
        {
            return Err(format!(
                "items.ron: \"{k}\": restore is above 0 and at most 1"
            ));
        }
        if !(it.bonus.health.is_finite() && it.bonus.damage.is_finite()) {
            return Err(format!("items.ron: \"{k}\": a bonus is a number"));
        }
        if let Some(ItemUse::Heal(n) | ItemUse::Mana(n)) = it.on_use {
            if !positive(n) {
                return Err(format!("items.ron: \"{k}\": on_use is an amount above 0"));
            }
        }
        if let Some(light) = it.light {
            if !(positive(light.range) && positive(light.intensity)) {
                return Err(format!(
                    "items.ron: \"{k}\": a light's range and intensity are above 0"
                ));
            }
        }
    }
    Ok(())
}

impl LootFile {
    /// Names into indices, every name and number checked.
    pub(crate) fn resolve(
        self,
        items: &[ItemDef],
        enemies: &[EnemyDef],
        bounds: &Bounds,
    ) -> Result<LootContent, String> {
        let item = |key: &str| {
            items
                .iter()
                .position(|i| i.key == key)
                .ok_or_else(|| format!("loot.ron: no item \"{key}\" in items.ron"))
        };
        for (i, k) in self.kinds.iter().enumerate() {
            if self.kinds[..i].iter().any(|o| o.key == k.key) {
                return Err(format!(
                    "loot.ron: container kind \"{}\" is listed twice",
                    k.key
                ));
            }
            if !non_negative(k.search) {
                return Err(format!("loot.ron: \"{}\": search is 0 or more", k.key));
            }
            if !positive(k.scale) {
                return Err(format!("loot.ron: \"{}\": scale is above 0", k.key));
            }
            if k.slots == 0 {
                return Err(format!("loot.ron: \"{}\": slots is at least 1", k.key));
            }
        }
        let mut tables = Vec::new();
        for (i, t) in self.tables.iter().enumerate() {
            if self.tables[..i].iter().any(|o| o.key == t.key) {
                return Err(format!("loot.ron: table \"{}\" is listed twice", t.key));
            }
            if t.rolls.0 > t.rolls.1 || !non_negative(t.nothing) {
                return Err(format!(
                    "loot.ron: table \"{}\": rolls go from least to most, nothing is 0 or more",
                    t.key
                ));
            }
            let mut entries = Vec::new();
            for e in &t.items {
                if !non_negative(e.weight) || e.count.0 == 0 || e.count.0 > e.count.1 {
                    return Err(format!(
                        "loot.ron: table \"{}\", \"{}\": weight 0 or more, count from 1",
                        t.key, e.item
                    ));
                }
                entries.push(TableEntry {
                    item: item(&e.item)?,
                    weight: e.weight,
                    count: e.count,
                });
            }
            tables.push(Table {
                key: t.key.clone(),
                rolls: t.rolls,
                nothing: t.nothing,
                items: entries,
            });
        }
        let table = |key: &str| {
            tables
                .iter()
                .position(|t| t.key == key)
                .ok_or_else(|| format!("loot.ron: no table \"{key}\""))
        };
        let mut containers = Vec::new();
        for c in &self.containers {
            let kind = self
                .kinds
                .iter()
                .position(|k| k.key == c.kind)
                .ok_or_else(|| format!("loot.ron: no container kind \"{}\"", c.kind))?;
            let at = V2::new(c.at.0, c.at.1);
            if !bounds.contains(at) {
                return Err(format!(
                    "loot.ron: the {} at ({}, {}) is outside the map",
                    c.kind, c.at.0, c.at.1
                ));
            }
            containers.push(PlacedContainer {
                kind,
                table: table(&c.table)?,
                at,
                z: c.z,
                lock: c.lock.as_deref().map(item).transpose()?,
                place: c.place.clone(),
            });
        }
        let mut bodies = Vec::new();
        for b in &self.bodies {
            let enemy = enemies
                .iter()
                .position(|e| e.key == b.enemy)
                .ok_or_else(|| format!("loot.ron: no enemy \"{}\" in enemies.ron", b.enemy))?;
            bodies.push((enemy, table(&b.table)?));
        }
        Ok(LootContent {
            kinds: self.kinds,
            tables,
            containers,
            bodies,
            pile_display: self.pile_display,
            remains_display: self.remains_display,
        })
    }
}
