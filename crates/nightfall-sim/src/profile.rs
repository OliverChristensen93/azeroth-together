//! A player's things between raids (spec §8.3): the stash, gold, the loadout they will bring,
//! the raid under way and the body left for the next one. Pure and versioned: the client keeps it
//! in `benilla-config/nightfall/stash.ron`. Items are named by key, so a content change never
//! shuffles a stash, and a key the content no longer knows is kept, not dropped.
//!
//! Nothing here loses or copies an item except by the death rules. A thing with no room in the
//! loadout goes to the stash, and the stash is never refused what a raid brings back: it grows
//! past its slots, and only the panel's own moves in are refused when it is full. A key the
//! content does not know stays in the stash, as written, until the content knows it again.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    Content, EquipSlot, Fate, HeroOutcome, InvError, Inventory, ItemKind, Loadout, Place, Recovery,
    Stack, MAX_BAGS, V3,
};

/// Bumped whenever the file changes shape; [`Profile::from_ron`] brings older files up to it.
pub const PROFILE_VERSION: u32 = 1;

/// Some of one item, by key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedStack {
    pub item: String,
    pub count: u16,
}

/// An inventory, by key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SavedInventory {
    pub equip: Vec<(EquipSlot, SavedStack)>,
    pub bags: Vec<(u8, SavedStack)>,
    pub pack: Vec<SavedStack>,
    pub safe: Vec<(u8, SavedStack)>,
}

impl SavedInventory {
    /// Whether nothing at all is worn, carried or kept.
    pub fn is_empty(&self) -> bool {
        self.equip.is_empty()
            && self.bags.is_empty()
            && self.pack.is_empty()
            && self.safe.is_empty()
    }

    /// Every stack, in a fixed order: worn, bags, backpack, safe slots.
    #[cfg(test)]
    fn stacks(&self) -> impl Iterator<Item = &SavedStack> {
        self.equip
            .iter()
            .map(|(_, s)| s)
            .chain(self.bags.iter().map(|(_, s)| s))
            .chain(self.pack.iter())
            .chain(self.safe.iter().map(|(_, s)| s))
    }
}

/// A body waiting for its player's next raid on its map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedRecovery {
    pub map: String,
    pub at: (f32, f32, f32),
    pub items: Vec<SavedStack>,
}

/// The raid under way: where it is and what it was brought.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedRaid {
    /// The map's key, so settling it touches only that map's body.
    pub map: String,
    pub brought: SavedInventory,
}

/// A player's things between raids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    /// The file's shape; a file written before it had one reads as 0.
    #[serde(default)]
    pub version: u32,
    /// The class picked last.
    pub class: String,
    pub gold: u32,
    pub stash: Vec<SavedStack>,
    /// What the next raid brings.
    pub loadout: SavedInventory,
    /// What the raid under way brought: present from its start to its end, so a raid that never
    /// ends (a crash, a lost connection) is settled as missing, never kept twice.
    pub in_raid: Option<SavedRaid>,
    /// Bodies waiting for their next raid, at most one for each map (spec §8.2).
    pub recovery: Vec<SavedRecovery>,
    pub raids: u32,
    pub extracted: u32,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: PROFILE_VERSION,
            class: String::new(),
            gold: 0,
            stash: Vec::new(),
            loadout: SavedInventory::default(),
            in_raid: None,
            recovery: Vec::new(),
            raids: 0,
            extracted: 0,
        }
    }
}

/// A profile that does not load, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// Not a profile: cut short, or written by hand wrongly.
    Parse(String),
    /// Written by a newer build: read wrongly it would be saved back wrong, so it is not read.
    Newer { found: u32, known: u32 },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Parse(why) => write!(f, "not a stash file: {why}"),
            ProfileError::Newer { found, known } => write!(
                f,
                "a stash from a newer Nightfall (version {found}; this one knows {known})"
            ),
        }
    }
}

/// Why a stash move was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashError {
    /// The player's things are away in a raid.
    InRaid,
    /// No stack at that index or place.
    NoSuchSlot,
    /// The content does not know the item (any more).
    Unknown,
    /// The stash has no room for it.
    StashFull,
    /// The loadout has no room for it.
    LoadoutFull,
    /// It does not go in that slot.
    WrongSlot,
    /// The bag still holds things that would have nowhere to go.
    BagInUse,
    /// Its value is nothing, so it cannot be sold.
    Worthless,
}

impl StashError {
    /// The line the stash panel shows.
    pub fn message(self) -> &'static str {
        match self {
            StashError::InRaid => "Your things are in a raid.",
            StashError::NoSuchSlot => "Nothing there.",
            StashError::Unknown => "This game does not know that item any more.",
            StashError::StashFull => "The stash is full.",
            StashError::LoadoutFull => "No room left to carry it.",
            StashError::WrongSlot => "That does not go there.",
            StashError::BagInUse => "Empty the bag first.",
            StashError::Worthless => "Nobody would pay for that.",
        }
    }
}

/// Whether a body has a place a client can put it: its position is a number.
fn placed(r: &SavedRecovery) -> bool {
    r.at.0.is_finite() && r.at.1.is_finite() && r.at.2.is_finite()
}

/// `s` by key; `None` for a stack naming an item index the content does not have, which the
/// sim never makes.
fn saved(s: &Stack, content: &Content) -> Option<SavedStack> {
    Some(SavedStack {
        item: content.items.get(usize::from(s.item))?.key.clone(),
        count: s.count,
    })
}

/// An inventory by key.
pub fn saved_inventory(inv: &Inventory, content: &Content) -> SavedInventory {
    let mut out = SavedInventory::default();
    for (place, s) in inv.stacks() {
        let Some(st) = saved(&s, content) else {
            continue;
        };
        match place {
            Place::Equip(slot) => out.equip.push((slot, st)),
            Place::Bag(i) => out.bags.push((i, st)),
            Place::Pack(_) => out.pack.push(st),
            Place::Safe(i) => out.safe.push((i, st)),
        }
    }
    out
}

/// A saved inventory for this content, and what of it the content does not know or has no room
/// for: kept for the stash, never dropped.
pub fn inventory_of(from: &SavedInventory, content: &Content) -> (Inventory, Vec<SavedStack>) {
    let mut inv = Inventory::new(content);
    let mut aside: Vec<SavedStack> = Vec::new();
    let mut loose: Vec<Stack> = Vec::new();
    // A stack bigger than its stack size keeps the rest as more stacks, never clipped away.
    let mut known = |s: &SavedStack| -> Option<(Stack, Vec<Stack>)> {
        let Some(i) = content.item(&s.item) else {
            aside.push(s.clone());
            return None;
        };
        let max = content.items[i].stack.max(1);
        let mut rest = Vec::new();
        let mut left = s.count.saturating_sub(max);
        while left > 0 {
            let n = left.min(max);
            rest.push(Stack::new(i, n));
            left -= n;
        }
        Some((Stack::new(i, s.count.min(max)), rest))
    };
    for (slot, s) in &from.equip {
        if let Some((st, rest)) = known(s) {
            if inv.equip[slot.index()].is_none() {
                inv.equip[slot.index()] = Some(st);
            } else {
                loose.push(st);
            }
            loose.extend(rest);
        }
    }
    for (i, s) in &from.bags {
        if let Some((st, rest)) = known(s) {
            let b = usize::from(*i);
            if b < MAX_BAGS && inv.bags[b].is_none() {
                inv.bags[b] = Some(st);
            } else {
                loose.push(st);
            }
            loose.extend(rest);
        }
    }
    for s in &from.pack {
        if let Some((st, rest)) = known(s) {
            loose.push(st);
            loose.extend(rest);
        }
    }
    for (i, s) in &from.safe {
        if let Some((st, rest)) = known(s) {
            let k = usize::from(*i);
            if k < inv.safe.len() && inv.safe[k].is_none() {
                inv.safe[k] = Some(st);
            } else {
                loose.push(st);
            }
            loose.extend(rest);
        }
    }
    inv.pack = loose.into_iter().map(Some).collect();
    for s in inv.sanitize(content) {
        aside.extend(saved(&s, content));
    }
    (inv, aside)
}

/// The free basic kit of class `class` (spec §8.3): worn where it goes, the rest in the
/// backpack. A class the content does not have has none.
pub fn free_kit(class: usize, content: &Content) -> Inventory {
    let mut inv = Inventory::new(content);
    let Some(def) = content.classes.get(class) else {
        return inv;
    };
    for &(item, count) in &def.free_items {
        let stack = Stack::new(item, count);
        match content.items[item].slot {
            Some(s) if inv.equip[s.index()].is_none() => inv.equip[s.index()] = Some(stack),
            _ => {
                let _ = inv.add(stack, content);
            }
        }
    }
    inv
}

impl Profile {
    /// A profile from its file's text: an older shape brought up to this one; a newer one refused,
    /// so it is never saved back wrong.
    pub fn from_ron(text: &str) -> Result<Profile, ProfileError> {
        #[derive(Deserialize)]
        struct Head {
            #[serde(default)]
            version: u32,
        }
        let head: Head = ron::from_str(text).map_err(|e| ProfileError::Parse(e.to_string()))?;
        if head.version > PROFILE_VERSION {
            return Err(ProfileError::Newer {
                found: head.version,
                known: PROFILE_VERSION,
            });
        }
        let mut p: Profile = ron::from_str(text).map_err(|e| ProfileError::Parse(e.to_string()))?;
        // Version 0 (no version written) has this shape: nothing to move.
        p.version = PROFILE_VERSION;
        Ok(p)
    }

    /// The file's text.
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("a profile is always writable")
    }

    /// The class picked last, when the content has it.
    pub fn class_index(&self, content: &Content) -> Option<usize> {
        content.class(&self.class)
    }

    /// What the next raid brings, as the content resolves it: the loadout, or the class's free
    /// kit when nothing of it is left (empty, or all of it unknown to the content), spec §8.3.
    /// What the content has no room for or does not know comes back to be stashed.
    fn next_kit(&self, class: usize, content: &Content) -> (Inventory, Vec<SavedStack>) {
        let (inv, aside) = inventory_of(&self.loadout, content);
        if inv.is_empty() {
            (free_kit(class, content), aside)
        } else {
            (inv, aside)
        }
    }

    /// What a hero of `class` brings into a raid now: the raid under way's, else the loadout,
    /// else the class's free kit when the loadout is empty (spec §8.3); and the body from their
    /// last raid on this map. A body whose place is not a number (a hand-edited file) is not
    /// offered, so no client sends it round and round.
    pub fn raid_loadout(&self, class: usize, content: &Content) -> Loadout {
        let inventory = match &self.in_raid {
            Some(raid) => inventory_of(&raid.brought, content).0,
            None => self.next_kit(class, content).0,
        };
        let recovery = self
            .recovery
            .iter()
            .find(|r| r.map == content.map.key)
            .filter(|r| placed(r))
            .map(|r| Recovery {
                map: r.map.clone(),
                at: V3::new(r.at.0, r.at.1, r.at.2),
                items: r
                    .items
                    .iter()
                    .filter_map(|s| content.item(&s.item).map(|i| Stack::new(i, s.count)))
                    .collect(),
            });
        Loadout {
            inventory,
            recovery,
        }
    }

    /// A raid begins: the loadout (or the free kit) leaves for it, so a raid that never ends is
    /// never a copy. What the content no longer knows, or has no room for, goes back to the stash.
    pub fn begin_raid(&mut self, class: usize, content: &Content) {
        if self.in_raid.is_some() {
            return;
        }
        let (inv, aside) = self.next_kit(class, content);
        for s in aside {
            self.stash_add(s, content);
        }
        self.in_raid = Some(SavedRaid {
            map: content.map.key.clone(),
            brought: saved_inventory(&inv, content),
        });
        self.loadout = SavedInventory::default();
        self.raids += 1;
    }

    /// The raid ended for this player: what came out is the loadout again, what they recovered
    /// and what a death left with no room goes to the stash, and a body left behind waits for the
    /// next raid on its map, replacing that map's older one. The body of the raid's own map had
    /// its one chance and is gone; the bodies of other maps stay.
    ///
    /// Only a raid under way is settled: an outcome that comes again, or with no raid begun, does
    /// nothing and gives `false`, so a repeat is never a second haul. Whether it was settled.
    pub fn settle(&mut self, o: &HeroOutcome, content: &Content) -> bool {
        let Some(raid) = self.in_raid.take() else {
            return false;
        };
        self.loadout = saved_inventory(&o.carried, content);
        for s in o.recovered.iter().chain(&o.overflow) {
            if let Some(st) = saved(s, content) {
                self.stash_add(st, content);
            }
        }
        self.use_up_body(&raid.map, content);
        if let Some(r) = &o.recovery {
            self.use_up_body(&r.map, content);
            self.recovery.push(SavedRecovery {
                map: r.map.clone(),
                at: (r.at.x, r.at.y, r.at.z),
                items: r.items.iter().filter_map(|s| saved(s, content)).collect(),
            });
        }
        if matches!(o.fate, Fate::Extracted { .. }) {
            self.extracted += 1;
        }
        true
    }

    /// A raid that never told this player its end (the game closed, the host went): settled as a
    /// death with no body, so `death_loss` takes what it would and the raid's map has no body
    /// left. Whether there was a raid to settle.
    ///
    /// What the death rules would let them keep but the content leaves no room for, or no longer
    /// knows, goes to the stash instead of through the rules: that can only happen after a
    /// content change.
    pub fn settle_missing(&mut self, content: &Content) -> bool {
        let Some(raid) = self.in_raid.take() else {
            return false;
        };
        let (mut inv, aside) = inventory_of(&raid.brought, content);
        for s in aside {
            self.stash_add(s, content);
        }
        let death = inv.split_for_death(content.rules.run.death_loss, content);
        for s in &death.spill {
            if let Some(st) = saved(s, content) {
                self.stash_add(st, content);
            }
        }
        inv.clear_found();
        self.loadout = saved_inventory(&inv, content);
        self.use_up_body(&raid.map, content);
        true
    }

    /// The body on `map`, if any, leaves the profile: its one chance is used up. What the chance
    /// never covered is not lost with it: items the content does not know, or a body whose place
    /// is not a number, were never offered, so they go to the stash.
    fn use_up_body(&mut self, map: &str, content: &Content) {
        let Some(at) = self.recovery.iter().position(|r| r.map == map) else {
            return;
        };
        let body = self.recovery.remove(at);
        let offered = placed(&body);
        for s in body.items {
            if !offered || content.item(&s.item).is_none() {
                self.stash_add(s, content);
            }
        }
    }

    /// How many slots the stash has before the panel refuses more moves in.
    pub fn stash_capacity(&self, content: &Content) -> usize {
        usize::from(content.rules.run.stash_slots)
    }

    /// The stash stack at `index` into the loadout: worn when its slot is free, a bag in a free
    /// bag slot, else into the backpack.
    pub fn take_from_stash(&mut self, index: usize, content: &Content) -> Result<(), StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let s = self
            .stash
            .get(index)
            .cloned()
            .ok_or(StashError::NoSuchSlot)?;
        let item = content.item(&s.item).ok_or(StashError::Unknown)?;
        let (mut inv, aside) = inventory_of(&self.loadout, content);
        let def = &content.items[item];
        let stack = Stack::new(item, s.count.min(def.stack.max(1)));
        let rest = s.count - stack.count;
        let worn = def.slot.filter(|slot| inv.equip[slot.index()].is_none());
        let bag = if def.kind == ItemKind::Bag {
            inv.bags.iter().position(Option::is_none)
        } else {
            None
        };
        if let Some(slot) = worn {
            inv.equip[slot.index()] = Some(stack);
        } else if let Some(b) = bag {
            inv.bags[b] = Some(stack);
            let _ = inv.sanitize(content);
        } else if inv.add(stack, content).is_some() {
            return Err(StashError::LoadoutFull);
        }
        if rest > 0 {
            self.stash[index].count = rest;
        } else {
            self.stash.remove(index);
        }
        for a in aside {
            self.stash.push(a);
        }
        self.loadout = saved_inventory(&inv, content);
        Ok(())
    }

    /// The loadout's stack at `at` into the stash.
    pub fn put_in_stash(&mut self, at: Place, content: &Content) -> Result<(), StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let (mut inv, aside) = inventory_of(&self.loadout, content);
        let stack = match at {
            Place::Bag(i) => {
                let s = inv
                    .bags
                    .get_mut(usize::from(i))
                    .and_then(Option::take)
                    .ok_or(StashError::NoSuchSlot)?;
                if !inv.sanitize(content).is_empty() {
                    return Err(StashError::BagInUse);
                }
                s
            }
            _ => inv.take(at).ok_or(StashError::NoSuchSlot)?,
        };
        let s = saved(&stack, content).ok_or(StashError::Unknown)?;
        if !self.stash_has_room(&s, content) {
            return Err(StashError::StashFull);
        }
        self.stash_add(s, content);
        for a in aside {
            self.stash.push(a);
        }
        self.loadout = saved_inventory(&inv, content);
        Ok(())
    }

    /// The loadout's stack at `from` moved to `to` (worn, a bag slot, the backpack, a safe slot),
    /// by the same rules as in a raid.
    pub fn loadout_move(
        &mut self,
        from: Place,
        to: Place,
        content: &Content,
    ) -> Result<(), StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let (mut inv, aside) = inventory_of(&self.loadout, content);
        inv.move_to(from, to, content).map_err(|e| match e {
            InvError::BagInUse => StashError::BagInUse,
            InvError::Empty | InvError::NoSuchPlace => StashError::NoSuchSlot,
            InvError::WrongSlot => StashError::WrongSlot,
            InvError::Full => StashError::LoadoutFull,
        })?;
        for a in aside {
            self.stash.push(a);
        }
        self.loadout = saved_inventory(&inv, content);
        Ok(())
    }

    /// The stash stack at `index` sold: its value in gold. The purse holds what a `u32` does and
    /// no more; a sale past it fills it and pays no more.
    pub fn sell(&mut self, index: usize, content: &Content) -> Result<u32, StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let s = self.stash.get(index).ok_or(StashError::NoSuchSlot)?;
        let item = content.item(&s.item).ok_or(StashError::Unknown)?;
        let value = content.items[item].value.saturating_mul(u32::from(s.count));
        if value == 0 {
            return Err(StashError::Worthless);
        }
        self.stash.remove(index);
        self.gold = self.gold.saturating_add(value);
        Ok(value)
    }

    /// Whether `s` fits the stash: onto stacks of it with room, then into free slots.
    fn stash_has_room(&self, s: &SavedStack, content: &Content) -> bool {
        let max = content
            .item(&s.item)
            .map_or(u16::MAX, |i| content.items[i].stack.max(1));
        let free: u32 = self
            .stash
            .iter()
            .filter(|o| o.item == s.item)
            .map(|o| u32::from(max.saturating_sub(o.count)))
            .sum();
        let need = u32::from(s.count).saturating_sub(free);
        let slots = need.div_ceil(u32::from(max)) as usize;
        self.stash.len() + slots <= self.stash_capacity(content)
    }

    /// `s` into the stash, onto stacks of it first. Never refused: what a raid brings back is
    /// kept even past the stash's room (the panel then refuses more moves in).
    pub fn stash_add(&mut self, s: SavedStack, content: &Content) {
        let Some(i) = content.item(&s.item) else {
            self.stash.push(s);
            return;
        };
        let max = content.items[i].stack.max(1);
        let mut count = s.count;
        for o in self.stash.iter_mut().filter(|o| o.item == s.item) {
            let n = max.saturating_sub(o.count).min(count);
            o.count += n;
            count -= n;
        }
        while count > 0 {
            let n = count.min(max);
            self.stash.push(SavedStack {
                item: s.item.clone(),
                count: n,
            });
            count -= n;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{ContentSources, DeathLoss, Fate, Recovery};

    fn content(run: &str) -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0, free_kit: [("knife", 1), ("bread", 2)])]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: &format!(
                r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                run: (backpack_slots: 3, safe_slots: 1, stash_slots: 2, conjured_drink: "water", {run}))"#
            ),
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand)),
                (key: "helm", name: "Helm", kind: Armour, icon: "a", weight: 2.0, slot: Some(Head),
                 value: 10),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 5),
                (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10,
                 value: 40),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 2),
                (key: "water", name: "Water", kind: Drink, icon: "a", weight: 0.5, stack: 5),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    /// A body of the profile, on `map`, with these things.
    fn body(map: &str, at: f32, items: Vec<SavedStack>) -> SavedRecovery {
        SavedRecovery {
            map: map.into(),
            at: (at, 0.0, 0.0),
            items,
        }
    }

    /// The raid under way on the test map, brought with `pack`.
    fn raid_with(pack: Vec<SavedStack>) -> Option<SavedRaid> {
        Some(SavedRaid {
            map: "test".into(),
            brought: SavedInventory {
                pack,
                ..SavedInventory::default()
            },
        })
    }

    const KNIFE: usize = 0;
    const HELM: usize = 1;
    const BREAD: usize = 2;
    const GEM: usize = 3;
    const POUCH: usize = 4;

    fn stack(item: &str, count: u16) -> SavedStack {
        SavedStack {
            item: item.into(),
            count,
        }
    }

    fn outcome(c: &Content, carried: Inventory, recovered: Vec<Stack>, fate: Fate) -> HeroOutcome {
        let _ = c;
        HeroOutcome {
            player: 0,
            name: "A".into(),
            fate,
            time: 100.0,
            kills: 2,
            carried,
            overflow: Vec::new(),
            recovered,
            recovery: None,
        }
    }

    #[test]
    fn a_profile_round_trips() {
        let p = Profile {
            gold: 7,
            class: "rogue".into(),
            stash: vec![stack("gem", 2)],
            loadout: SavedInventory {
                equip: vec![(EquipSlot::Head, stack("helm", 1))],
                bags: vec![(1, stack("pouch", 1))],
                safe: vec![(0, stack("bread", 1))],
                ..SavedInventory::default()
            },
            in_raid: raid_with(vec![stack("gem", 3)]),
            recovery: vec![
                SavedRecovery {
                    map: "test".into(),
                    at: (1.0, 2.5, -3.25),
                    items: vec![stack("bread", 1)],
                },
                body("elsewhere", 4.0, vec![stack("gem", 1)]),
            ],
            raids: 4,
            extracted: 2,
            ..Profile::default()
        };
        let text = p.to_ron();
        assert_eq!(Profile::from_ron(&text).unwrap(), p);
        assert_eq!(
            Profile::from_ron(&text).unwrap().to_ron(),
            text,
            "the text is stable"
        );
    }

    #[test]
    fn an_old_file_without_a_version_loads() {
        let old = Profile::from_ron(r#"(gold: 12, stash: [(item: "bread", count: 3)])"#).unwrap();
        assert_eq!(old.version, PROFILE_VERSION);
        assert_eq!(old.gold, 12);
        assert_eq!(old.stash, vec![stack("bread", 3)]);
    }

    #[test]
    fn a_newer_profile_is_refused_not_misread() {
        assert_eq!(
            Profile::from_ron("(version: 99, gold: 5)"),
            Err(ProfileError::Newer {
                found: 99,
                known: PROFILE_VERSION
            })
        );
        assert!(matches!(
            Profile::from_ron("(gold: "),
            Err(ProfileError::Parse(_))
        ));
        assert!(matches!(
            Profile::from_ron("(gold: \"lots\")"),
            Err(ProfileError::Parse(_))
        ));
    }

    #[test]
    fn a_misspelt_field_is_refused_not_read_as_empty() {
        for text in [
            "(stsh: [(item: \"gem\", count: 1)])",
            "(stash: [(item: \"gem\", count: 1, extra: 2)])",
            "(loadout: (pak: []))",
            "(recovery: [(map: \"x\", at: (0.0, 0.0, 0.0), items: [], why: 1)])",
            "(in_raid: Some((map: \"x\", brought: (), extra: 1)))",
        ] {
            assert!(
                matches!(Profile::from_ron(text), Err(ProfileError::Parse(_))),
                "{text}"
            );
        }
        // A newer file may hold fields this build has never heard of: it is still read as newer.
        assert_eq!(
            Profile::from_ron("(version: 7, brand_new: 1)"),
            Err(ProfileError::Newer {
                found: 7,
                known: PROFILE_VERSION
            })
        );
    }

    #[test]
    fn garbage_is_an_error_never_a_panic() {
        let deep = "(".repeat(5000);
        for text in [
            "",
            "   ",
            "(",
            ")",
            "nonsense",
            "(gold: -1)",
            "(gold: 99999999999999999999)",
            "(stash: [(item: 5, count: 1)])",
            "(stash: [(item: \"gem\", count: 70000)])",
            "(loadout: (equip: [(Nowhere, (item: \"x\", count: 1))]))",
            "\u{0}\u{1}\u{ffff}",
            "(version: -3)",
            deep.as_str(),
        ] {
            assert!(
                matches!(Profile::from_ron(text), Err(ProfileError::Parse(_))),
                "{text:.30?}"
            );
        }
    }

    #[test]
    fn an_empty_loadout_raids_with_the_free_kit() {
        let c = content("");
        let mut p = Profile::default();
        let lo = p.raid_loadout(0, &c);
        assert_eq!(
            lo.inventory.get(Place::Equip(EquipSlot::MainHand)),
            Some(&Stack::new(KNIFE, 1))
        );
        assert_eq!(lo.inventory.count_of(BREAD), 2);
        assert_eq!(lo.recovery, None);
        p.begin_raid(0, &c);
        assert!(p.in_raid.is_some());
        assert!(p.loadout.is_empty());
        assert_eq!(p.raids, 1);
        assert_eq!(p.raid_loadout(0, &c).inventory, lo.inventory);
        p.begin_raid(0, &c);
        assert_eq!(p.raids, 1, "a raid under way does not begin again");
        // A class the content does not have brings nothing, and does not panic.
        assert!(free_kit(9, &c).is_empty());
    }

    #[test]
    fn beginning_a_raid_moves_the_loadout_out_and_settling_brings_back_what_came_out() {
        let c = content("");
        let mut p = Profile::default();
        p.loadout.pack.push(stack("gem", 2));
        p.loadout.equip.push((EquipSlot::Head, stack("helm", 1)));
        p.begin_raid(0, &c);
        assert!(p.loadout.is_empty());
        let raid = p.in_raid.as_ref().unwrap();
        assert_eq!(raid.brought.pack, vec![stack("gem", 2)]);
        assert_eq!(raid.map, "test");
        let mut carried = Inventory::new(&c);
        carried.add(Stack::new(GEM, 5), &c);
        let o = outcome(
            &c,
            carried,
            vec![Stack::new(BREAD, 3)],
            Fate::Extracted { exit: "x".into() },
        );
        p.settle(&o, &c);
        assert_eq!(p.in_raid, None);
        assert_eq!(p.loadout.pack, vec![stack("gem", 5)]);
        assert_eq!(p.stash, vec![stack("bread", 3)]);
        assert_eq!(p.extracted, 1);
    }

    #[test]
    fn what_a_death_left_with_no_room_is_stashed_with_what_was_recovered() {
        let c = content("");
        let mut p = Profile::default();
        p.begin_raid(0, &c);
        let mut o = outcome(
            &c,
            Inventory::new(&c),
            vec![Stack::new(BREAD, 3)],
            Fate::Died,
        );
        o.overflow = vec![Stack::new(GEM, 12), Stack::new(HELM, 1)];
        p.settle(&o, &c);
        assert_eq!(
            p.stash,
            vec![
                stack("bread", 3),
                stack("gem", 10),
                stack("gem", 2),
                stack("helm", 1)
            ],
            "the stash grows past its two slots rather than lose a thing"
        );
        assert_eq!(p.extracted, 0);
    }

    #[test]
    fn an_outcome_with_no_raid_under_way_changes_nothing() {
        let c = content("");
        let mut p = Profile {
            loadout: SavedInventory {
                pack: vec![stack("gem", 2)],
                ..SavedInventory::default()
            },
            stash: vec![stack("bread", 1)],
            ..Profile::default()
        };
        let mut carried = Inventory::new(&c);
        carried.add(Stack::new(BREAD, 4), &c);
        let o = outcome(
            &c,
            carried,
            vec![Stack::new(GEM, 1)],
            Fate::Extracted { exit: "x".into() },
        );
        let before = p.clone();
        assert!(!p.settle(&o, &c));
        assert_eq!(p, before);
    }

    #[test]
    fn settling_the_same_outcome_twice_changes_nothing_the_second_time() {
        let c = content("");
        let mut p = Profile::default();
        p.begin_raid(0, &c);
        let mut carried = Inventory::new(&c);
        carried.add(Stack::new(GEM, 5), &c);
        let mut o = outcome(
            &c,
            carried,
            vec![Stack::new(BREAD, 3)],
            Fate::Extracted { exit: "x".into() },
        );
        o.overflow = vec![Stack::new(HELM, 1)];
        assert!(p.settle(&o, &c));
        let once = p.clone();
        assert!(!p.settle(&o, &c));
        assert_eq!(p, once);
        assert_eq!(p.extracted, 1);
    }

    #[test]
    fn a_raid_that_never_ended_counts_as_missing() {
        let brought = |p: &mut Profile| {
            p.loadout
                .equip
                .push((EquipSlot::MainHand, stack("knife", 1)));
            p.loadout.pack.push(stack("gem", 4));
            p.loadout.safe.push((0, stack("bread", 1)));
        };
        let c = content("");
        let mut p = Profile::default();
        brought(&mut p);
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.loadout.safe, vec![(0, stack("bread", 1))]);
        assert!(p.loadout.equip.is_empty() && p.loadout.pack.is_empty());
        assert!(
            !p.settle_missing(&c),
            "nothing under way: nothing to settle"
        );
        let c = content("death_loss: found_only");
        let mut p = Profile::default();
        brought(&mut p);
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(
            p.loadout.pack,
            vec![stack("gem", 4)],
            "found_only keeps what was brought"
        );
        let c = content("death_loss: everything");
        let mut p = Profile::default();
        brought(&mut p);
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.loadout.safe, vec![(0, stack("bread", 1))]);
        assert!(p.loadout.equip.is_empty() && p.loadout.pack.is_empty());
    }

    #[test]
    fn a_missing_raid_leaves_no_body_and_takes_the_old_one_on_this_map() {
        let c = content("");
        let mut p = Profile {
            recovery: vec![body("test", 1.0, vec![stack("gem", 1)])],
            ..Profile::default()
        };
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.recovery, Vec::new());
        // A body on another map is not this raid's to take.
        p.recovery = vec![body("elsewhere", 0.0, vec![stack("gem", 1)])];
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.recovery.len(), 1);
        assert_eq!(p.recovery[0].map, "elsewhere");
    }

    #[test]
    fn a_missing_raid_uses_up_the_body_on_the_raids_own_map() {
        let c = content("");
        let mut p = Profile {
            recovery: vec![
                body("test", 1.0, vec![stack("gem", 1)]),
                body("elsewhere", 2.0, vec![stack("gem", 2)]),
            ],
            in_raid: Some(SavedRaid {
                map: "elsewhere".into(),
                brought: SavedInventory::default(),
            }),
            ..Profile::default()
        };
        assert!(p.settle_missing(&c));
        assert_eq!(p.recovery.len(), 1);
        assert_eq!(
            p.recovery[0].map, "test",
            "the raid's own map is used, not the content's"
        );
    }

    #[test]
    fn recovered_things_go_to_the_stash_and_the_new_body_replaces_the_old() {
        let c = content("");
        let mut p = Profile {
            recovery: vec![body("test", 1.0, vec![stack("gem", 1)])],
            ..Profile::default()
        };
        let r = p.raid_loadout(0, &c).recovery.unwrap();
        assert_eq!(r.items, vec![Stack::new(GEM, 1)]);
        assert_eq!(r.at, V3::new(1.0, 0.0, 0.0));
        p.begin_raid(0, &c);
        let mut died = outcome(&c, Inventory::new(&c), Vec::new(), Fate::Died);
        died.recovery = Some(Recovery {
            map: "test".into(),
            at: V3::new(5.0, 6.0, 7.0),
            items: vec![Stack::new(KNIFE, 1)],
        });
        p.settle(&died, &c);
        assert_eq!(p.recovery.len(), 1, "one body for the map, the new one");
        assert_eq!(p.recovery[0].at, (5.0, 6.0, 7.0));
        assert_eq!(p.recovery[0].items, vec![stack("knife", 1)]);
        // Out without reaching it: its one chance is gone.
        p.begin_raid(0, &c);
        let out = outcome(
            &c,
            Inventory::new(&c),
            Vec::new(),
            Fate::Extracted { exit: "x".into() },
        );
        p.settle(&out, &c);
        assert_eq!(p.recovery, Vec::new());
        // A body on another map waits for that map.
        p.recovery = vec![body("elsewhere", 0.0, vec![stack("gem", 1)])];
        p.begin_raid(0, &c);
        p.settle(&out, &c);
        assert_eq!(p.recovery.len(), 1);
        assert_eq!(p.recovery[0].map, "elsewhere");
    }

    #[test]
    fn a_death_on_another_map_leaves_the_first_maps_body() {
        let c = content("");
        let elsewhere = body("elsewhere", 0.0, vec![stack("gem", 1)]);
        let mut p = Profile {
            recovery: vec![elsewhere.clone()],
            ..Profile::default()
        };
        let died_with = |at: f32| {
            let mut o = outcome(&c, Inventory::new(&c), Vec::new(), Fate::Died);
            o.recovery = Some(Recovery {
                map: "test".into(),
                at: V3::new(at, 0.0, 0.0),
                items: vec![Stack::new(KNIFE, 1)],
            });
            o
        };
        p.begin_raid(0, &c);
        assert!(p.settle(&died_with(5.0), &c));
        assert_eq!(
            p.recovery,
            vec![
                elsewhere.clone(),
                body("test", 5.0, vec![stack("knife", 1)])
            ]
        );
        // A second death on the same map replaces only that map's body.
        p.begin_raid(0, &c);
        assert!(p.settle(&died_with(8.0), &c));
        assert_eq!(
            p.recovery,
            vec![
                elsewhere.clone(),
                body("test", 8.0, vec![stack("knife", 1)])
            ]
        );
        // Only the map the player is on offers its body.
        assert_eq!(p.raid_loadout(0, &c).recovery.unwrap().at.x, 8.0);
    }

    #[test]
    fn a_body_that_is_dropped_keeps_what_it_never_offered() {
        let egg_and_gems = |at: f32| {
            vec![body(
                "test",
                at,
                vec![stack("dragon_egg", 1), stack("gem", 2)],
            )]
        };
        let c = content("");
        let out = outcome(
            &c,
            Inventory::new(&c),
            Vec::new(),
            Fate::Extracted { exit: "x".into() },
        );
        // Dropped by settle: the egg was never offered, the gems were and had their chance.
        let mut p = Profile {
            recovery: egg_and_gems(1.0),
            ..Profile::default()
        };
        p.begin_raid(0, &c);
        assert!(p.settle(&out, &c));
        assert_eq!(p.recovery, Vec::new());
        assert_eq!(p.stash, vec![stack("dragon_egg", 1)]);
        // Dropped by settle_missing: the same.
        let mut p = Profile {
            recovery: egg_and_gems(1.0),
            ..Profile::default()
        };
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.recovery, Vec::new());
        assert_eq!(p.stash, vec![stack("dragon_egg", 1)]);
        // A body with no place was never offered at all: everything in it is kept.
        let mut p = Profile {
            recovery: egg_and_gems(f32::NAN),
            ..Profile::default()
        };
        p.begin_raid(0, &c);
        assert!(p.settle(&out, &c));
        assert_eq!(p.stash, vec![stack("dragon_egg", 1), stack("gem", 2)]);
        // A body replaced by a new death is dropped the same way.
        let mut p = Profile {
            recovery: egg_and_gems(1.0),
            ..Profile::default()
        };
        p.begin_raid(0, &c);
        let mut died = outcome(&c, Inventory::new(&c), Vec::new(), Fate::Died);
        died.recovery = Some(Recovery {
            map: "test".into(),
            at: V3::new(5.0, 6.0, 7.0),
            items: vec![Stack::new(KNIFE, 1)],
        });
        assert!(p.settle(&died, &c));
        assert_eq!(p.stash, vec![stack("dragon_egg", 1)]);
        assert_eq!(p.recovery[0].items, vec![stack("knife", 1)]);
    }

    #[test]
    fn a_body_with_no_place_is_not_offered() {
        let c = content("");
        let mut p = Profile {
            recovery: vec![body("test", f32::NAN, vec![stack("gem", 1)])],
            ..Profile::default()
        };
        assert_eq!(p.raid_loadout(0, &c).recovery, None);
        p.recovery[0].at = (0.0, f32::INFINITY, 0.0);
        assert_eq!(p.raid_loadout(0, &c).recovery, None);
    }

    #[test]
    fn a_loadout_the_content_knows_nothing_of_raids_with_the_free_kit() {
        let c = content("");
        let mut p = Profile {
            loadout: SavedInventory {
                pack: vec![stack("dragon_egg", 1)],
                equip: vec![(EquipSlot::Head, stack("moon_helm", 1))],
                ..SavedInventory::default()
            },
            ..Profile::default()
        };
        let kit = free_kit(0, &c);
        assert_eq!(p.raid_loadout(0, &c).inventory, kit, "what is offered");
        p.begin_raid(0, &c);
        assert_eq!(
            p.raid_loadout(0, &c).inventory,
            kit,
            "what the raid then brings is the same"
        );
        assert_eq!(
            p.stash,
            vec![stack("moon_helm", 1), stack("dragon_egg", 1)],
            "and the egg and the helm wait in the stash"
        );
    }

    #[test]
    fn stash_moves_respect_room_and_selling_pays() {
        let c = content("");
        let mut p = Profile {
            stash: vec![stack("gem", 3), stack("knife", 1)],
            ..Profile::default()
        };
        p.take_from_stash(1, &c).unwrap();
        assert_eq!(
            p.loadout.equip,
            vec![(EquipSlot::MainHand, stack("knife", 1))]
        );
        assert_eq!(p.stash, vec![stack("gem", 3)]);
        p.loadout.pack.push(stack("bread", 2));
        p.loadout.pack.push(stack("helm", 1));
        p.put_in_stash(Place::Equip(EquipSlot::MainHand), &c)
            .unwrap();
        assert_eq!(p.stash.len(), 2);
        assert_eq!(
            p.put_in_stash(Place::Pack(0), &c),
            Err(StashError::StashFull)
        );
        assert_eq!(p.sell(0, &c), Ok(120));
        assert_eq!(p.gold, 120);
        assert_eq!(p.sell(0, &c), Err(StashError::Worthless));
        assert_eq!(p.stash, vec![stack("knife", 1)]);
        p.begin_raid(0, &c);
        assert_eq!(p.take_from_stash(0, &c), Err(StashError::InRaid));
        assert_eq!(p.sell(0, &c), Err(StashError::InRaid));
        assert_eq!(p.stash, vec![stack("knife", 1)], "nothing was sold");
    }

    #[test]
    fn selling_never_overflows_the_purse() {
        let c = content("");
        let mut p = Profile {
            gold: u32::MAX - 5,
            stash: vec![stack("gem", 10), stack("gem", 0)],
            ..Profile::default()
        };
        assert_eq!(p.sell(0, &c), Ok(400));
        assert_eq!(p.gold, u32::MAX);
        assert_eq!(
            p.sell(0, &c),
            Err(StashError::Worthless),
            "a stack of none is worth nothing"
        );
        assert_eq!(p.sell(5, &c), Err(StashError::NoSuchSlot));
        p.stash.push(stack("dragon_egg", 1));
        assert_eq!(p.sell(1, &c), Err(StashError::Unknown));
        assert_eq!(p.stash.len(), 2, "a refused sale takes nothing");
    }

    #[test]
    fn the_loadout_moves_between_its_own_slots() {
        let c = content("");
        let mut p = Profile::default();
        p.loadout.pack.push(stack("bread", 2));
        p.loadout_move(Place::Pack(0), Place::Safe(0), &c).unwrap();
        assert_eq!(p.loadout.safe, vec![(0, stack("bread", 2))]);
        assert!(p.loadout.pack.is_empty());
        assert_eq!(
            p.loadout_move(Place::Safe(0), Place::Equip(EquipSlot::Head), &c),
            Err(StashError::WrongSlot)
        );
        assert_eq!(
            p.loadout_move(Place::Pack(2), Place::Safe(0), &c),
            Err(StashError::NoSuchSlot)
        );
    }

    #[test]
    fn unknown_or_overflowing_things_go_back_to_the_stash() {
        let c = content("");
        let mut p = Profile::default();
        p.loadout.pack = vec![stack("dragon_egg", 1), stack("bread", 5), stack("gem", 25)];
        p.begin_raid(0, &c);
        // Three backpack slots: the bread and two full stacks of gems; the egg the content does
        // not know and the five gems with no room go back to the stash, never away.
        assert_eq!(p.stash, vec![stack("dragon_egg", 1), stack("gem", 5)]);
        let raid = p.in_raid.as_ref().unwrap();
        assert_eq!(
            raid.brought.pack,
            vec![stack("bread", 5), stack("gem", 10), stack("gem", 10)]
        );
        assert_eq!(p.take_from_stash(0, &c), Err(StashError::InRaid));
    }

    #[test]
    fn a_key_the_content_does_not_know_is_kept_through_every_move_and_the_file() {
        let c = content("");
        let mut p = Profile {
            stash: vec![stack("dragon_egg", 1), stack("gem", 1)],
            loadout: SavedInventory {
                pack: vec![stack("moon_blade", 1), stack("bread", 1)],
                ..SavedInventory::default()
            },
            ..Profile::default()
        };
        // Moves that touch the loadout set the unknown aside into the stash; none drops it.
        p.loadout_move(Place::Pack(0), Place::Safe(0), &c).unwrap();
        p.take_from_stash(1, &c).unwrap();
        assert_eq!(p.take_from_stash(0, &c), Err(StashError::Unknown));
        let keys = |p: &Profile| {
            let mut k: Vec<String> = p.stash.iter().map(|s| s.item.clone()).collect();
            k.sort();
            k
        };
        assert_eq!(keys(&p), vec!["dragon_egg", "moon_blade"]);
        // And the file says the same, byte for byte, so a newer build finds them again.
        let back = Profile::from_ron(&p.to_ron()).unwrap();
        assert_eq!(back, p);
        assert_eq!(keys(&back), vec!["dragon_egg", "moon_blade"]);
    }

    /// Every item this profile holds, whether stashed, carried, under way or left on a body.
    fn tally(p: &Profile) -> BTreeMap<String, u32> {
        let mut t = BTreeMap::new();
        let mut put = |s: &SavedStack| *t.entry(s.item.clone()).or_insert(0) += u32::from(s.count);
        p.stash.iter().for_each(&mut put);
        p.loadout.stacks().for_each(&mut put);
        p.in_raid
            .iter()
            .flat_map(|r| r.brought.stacks())
            .for_each(&mut put);
        p.recovery.iter().flat_map(|r| &r.items).for_each(&mut put);
        t.retain(|_, n| *n > 0);
        t
    }

    fn tally_stacks(c: &Content, stacks: &[Stack]) -> BTreeMap<String, u32> {
        let mut t = BTreeMap::new();
        for s in stacks {
            *t.entry(c.items[usize::from(s.item)].key.clone())
                .or_insert(0) += u32::from(s.count);
        }
        t.retain(|_, n| *n > 0);
        t
    }

    fn sum(a: &BTreeMap<String, u32>, b: &BTreeMap<String, u32>) -> BTreeMap<String, u32> {
        let mut t = a.clone();
        for (k, n) in b {
            *t.entry(k.clone()).or_insert(0) += n;
        }
        t
    }

    /// What a raid was brought and what it found, to follow through every fate: a found bag
    /// widens the backpack to five, with brought things in four of them and found things in the
    /// rest, so that a found-only death must spill a brought helm.
    fn a_raid_in_the_pack(c: &Content) -> (Profile, Inventory, Vec<Stack>) {
        let p = Profile {
            in_raid: Some(SavedRaid {
                map: "test".into(),
                brought: SavedInventory {
                    pack: vec![
                        stack("bread", 5),
                        stack("gem", 10),
                        stack("gem", 4),
                        stack("helm", 1),
                    ],
                    safe: vec![(0, stack("bread", 1))],
                    ..SavedInventory::default()
                },
            }),
            ..Profile::default()
        };
        // The raid: the same things plus a found pouch and found gems, laid out as played.
        let mut inv = Inventory::new(c);
        inv.bags[0] = Some(Stack::found(POUCH, 1));
        inv.pack = vec![
            Some(Stack::found(GEM, 3)),
            Some(Stack::new(BREAD, 5)),
            Some(Stack::new(GEM, 10)),
            Some(Stack::new(GEM, 4)),
            Some(Stack::new(HELM, 1)),
        ];
        inv.safe[0] = Some(Stack::new(BREAD, 1));
        let everything: Vec<Stack> = inv.stacks().into_iter().map(|(_, s)| s).collect();
        (p, inv, everything)
    }

    #[test]
    fn no_fate_makes_or_loses_an_item_beyond_the_death_rules() {
        for mode in [
            DeathLoss::Everything,
            DeathLoss::EverythingOneRecovery,
            DeathLoss::FoundOnly,
        ] {
            let name = match mode {
                DeathLoss::Everything => "everything",
                DeathLoss::EverythingOneRecovery => "everything_one_recovery",
                DeathLoss::FoundOnly => "found_only",
            };
            let c = content(&format!("death_loss: {name}"));
            let (before, raid, everything) = a_raid_in_the_pack(&c);
            let all = tally_stacks(&c, &everything);

            // Out alive: everything carried is theirs, found or brought.
            let mut p = before.clone();
            let mut out = raid.clone();
            out.clear_found();
            p.settle(
                &outcome(&c, out, Vec::new(), Fate::Extracted { exit: "x".into() }),
                &c,
            );
            assert_eq!(tally(&p), all, "extracted, {name}");

            // Dead: what the mode takes is gone, unless the body keeps it for the next raid; the
            // rest, spill included, is theirs.
            let mut p = before.clone();
            let mut carried = raid.clone();
            let death = carried.split_for_death(mode, &c);
            carried.clear_found();
            let body =
                (mode == DeathLoss::EverythingOneRecovery && !death.lost.is_empty()).then(|| {
                    Recovery {
                        map: "test".into(),
                        at: V3::new(1.0, 2.0, 3.0),
                        items: death
                            .lost
                            .iter()
                            .map(|s| Stack { found: false, ..*s })
                            .collect(),
                    }
                });
            let mut o = outcome(&c, carried, Vec::new(), Fate::Died);
            o.overflow = death.spill.clone();
            o.recovery = body;
            p.settle(&o, &c);
            let gone = if mode == DeathLoss::EverythingOneRecovery {
                BTreeMap::new()
            } else {
                tally_stacks(&c, &death.lost)
            };
            assert_eq!(sum(&tally(&p), &gone), all, "died, {name}");
            if mode == DeathLoss::FoundOnly {
                assert_eq!(tally(&p).get("helm"), Some(&1), "the spilled helm is kept");
            }

            // Never heard from: only the brought things, by the same rules, and no body.
            let mut p = before.clone();
            let brought = tally(&p);
            assert!(p.settle_missing(&c));
            let mut probe = inventory_of(&before.in_raid.as_ref().unwrap().brought, &c).0;
            let aside_to_stash = inventory_of(&before.in_raid.as_ref().unwrap().brought, &c).1;
            let death = probe.split_for_death(mode, &c);
            let gone = tally_stacks(&c, &death.lost);
            assert_eq!(sum(&tally(&p), &gone), brought, "missing, {name}");
            assert_eq!(p.recovery, Vec::new());
            assert_eq!(p.in_raid, None);
            // The four brought stacks do not fit three slots: one went to the stash at once.
            assert_eq!(aside_to_stash.len(), 1);
        }
    }
}
