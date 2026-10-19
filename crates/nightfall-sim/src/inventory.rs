//! What a hero carries (spec §8.2): what they wear, the bags that widen their backpack, the
//! backpack's slots, and the safe slots nothing takes. Pure, so the host's raid and the stash
//! between raids hold things by the same rules.
//!
//! An inventory can come from elsewhere (a client's loadout, an old stash file), so nothing here
//! trusts that a stack names a known item, holds at least one, or sits in a slot that exists: such
//! stacks are refused, skipped or weighed as nothing, and [`Inventory::sanitize`] drops them.

use serde::{Deserialize, Serialize};

use crate::{Bonus, Content, DeathLoss, EquipSlot, ItemDef, ItemKind};

/// Bags a hero can wear at once.
pub const MAX_BAGS: usize = 4;

/// Some of one item; `found` marks what was picked up this raid (spec §8.2's `found_only`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    /// Index into [`Content::items`].
    pub item: u16,
    /// How many, at least one in a sound inventory and at most the item's stack size.
    pub count: u16,
    /// Picked up this raid, so a `found_only` death takes it; found and brought stacks of one
    /// item do not merge.
    pub found: bool,
}

impl Stack {
    /// `count` of `item`, brought.
    pub fn new(item: usize, count: u16) -> Self {
        Self {
            item: item as u16,
            count,
            found: false,
        }
    }

    /// `count` of `item`, found this raid.
    pub fn found(item: usize, count: u16) -> Self {
        Self {
            found: true,
            ..Self::new(item, count)
        }
    }
}

/// Where in an inventory a stack sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// What is worn, one slot for each [`EquipSlot`].
    Equip(EquipSlot),
    /// A bag worn in one of the [`MAX_BAGS`] bag slots.
    Bag(u8),
    /// A backpack slot. The backpack is compacted when a bag comes off and by a death, so an
    /// index is only good until then.
    Pack(u16),
    /// A safe slot, which a death never takes.
    Safe(u8),
    /// The companion's one stack (spec §8.2).
    Pet,
}

/// Why a move was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvError {
    /// Nothing there (or nothing the content knows, or a stack of none).
    Empty,
    /// No such slot.
    NoSuchPlace,
    /// It does not go there (a helm in the boots' slot, a sword in a bag slot).
    WrongSlot,
    /// No room for more of it there.
    Full,
    /// Taking the bag off would leave its things nowhere.
    BagInUse,
}

/// A sum of many `f32`s held to a finite `f32`, whatever the stacks and counts add up to.
fn finite(sum: f64) -> f32 {
    if sum.is_nan() {
        0.0
    } else {
        sum.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
    }
}

/// What [`Inventory::split_for_death`] took out of an inventory.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Death {
    /// Stacks the death takes: they go onto the body, or are gone, by the death-loss mode.
    pub lost: Vec<Stack>,
    /// Brought stacks that stay the hero's under `found_only` but no longer fit the backpack
    /// once a found bag is gone. Never lost; the caller files them home.
    pub spill: Vec<Stack>,
}

/// What one hero carries: what is worn, the bags, the backpack and the safe slots (see the
/// module docs). A plain value, so it can be saved, sent and compared whole. The default has no
/// slots at all (for messages and tests); [`Inventory::new`] is the one with the rules' slots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    /// By [`EquipSlot::index`].
    pub equip: [Option<Stack>; EquipSlot::COUNT],
    /// Worn bags, by bag slot.
    pub bags: [Option<Stack>; MAX_BAGS],
    /// As many slots as [`Inventory::pack_size`].
    pub pack: Vec<Option<Stack>>,
    /// As many slots as the rules give; nothing takes them from a hero.
    pub safe: Vec<Option<Stack>>,
    /// The companion's one stack: `None` without a companion. Never lost, never weighed: the
    /// companion carries it.
    pub pet: Option<Option<Stack>>,
}

impl Inventory {
    /// Empty, with the rules' backpack and safe slots.
    pub fn new(content: &Content) -> Self {
        let r = &content.rules.run;
        Self {
            equip: [None; EquipSlot::COUNT],
            bags: [None; MAX_BAGS],
            pack: vec![None; usize::from(r.backpack_slots)],
            safe: vec![None; usize::from(r.safe_slots)],
            pet: None,
        }
    }

    /// The backpack's slots with the bags worn now.
    pub fn pack_size(&self, content: &Content) -> usize {
        let bags: usize = self
            .bags
            .iter()
            .flatten()
            .filter_map(|b| content.items.get(usize::from(b.item)))
            .map(|def| usize::from(def.bag_slots))
            .sum();
        usize::from(content.rules.run.backpack_slots) + bags
    }

    /// The stack at `at`; `None` for an empty slot or one that does not exist.
    pub fn get(&self, at: Place) -> Option<&Stack> {
        match at {
            Place::Equip(s) => self.equip[s.index()].as_ref(),
            Place::Bag(i) => self.bags.get(usize::from(i))?.as_ref(),
            Place::Pack(i) => self.pack.get(usize::from(i))?.as_ref(),
            Place::Safe(i) => self.safe.get(usize::from(i))?.as_ref(),
            Place::Pet => self.pet.as_ref()?.as_ref(),
        }
    }

    fn slot(&mut self, at: Place) -> Option<&mut Option<Stack>> {
        match at {
            Place::Equip(s) => Some(&mut self.equip[s.index()]),
            Place::Bag(i) => self.bags.get_mut(usize::from(i)),
            Place::Pack(i) => self.pack.get_mut(usize::from(i)),
            Place::Safe(i) => self.safe.get_mut(usize::from(i)),
            Place::Pet => self.pet.as_mut(),
        }
    }

    /// Whether `at` is a slot of this inventory, empty or not.
    fn exists(&self, at: Place) -> bool {
        match at {
            Place::Equip(_) => true,
            Place::Bag(i) => usize::from(i) < self.bags.len(),
            Place::Pack(i) => usize::from(i) < self.pack.len(),
            Place::Safe(i) => usize::from(i) < self.safe.len(),
            Place::Pet => self.pet.is_some(),
        }
    }

    /// Every stack and where it sits: worn, bags, backpack, safe slots. In that order,
    /// with the backpack's slots in their current numbering.
    pub fn stacks(&self) -> Vec<(Place, Stack)> {
        let mut out = Vec::new();
        for s in EquipSlot::ALL {
            if let Some(st) = self.equip[s.index()] {
                out.push((Place::Equip(s), st));
            }
        }
        for (i, b) in self.bags.iter().enumerate() {
            if let Some(st) = b {
                out.push((Place::Bag(i as u8), *st));
            }
        }
        for (i, p) in self.pack.iter().enumerate() {
            if let Some(st) = p {
                out.push((Place::Pack(i as u16), *st));
            }
        }
        for (i, p) in self.safe.iter().enumerate() {
            if let Some(st) = p {
                out.push((Place::Safe(i as u8), *st));
            }
        }
        if let Some(Some(st)) = self.pet {
            out.push((Place::Pet, st));
        }
        out
    }

    /// Whether nothing at all is carried or worn.
    pub fn is_empty(&self) -> bool {
        self.stacks().is_empty()
    }

    /// The stacks a hero can use in the raid: everything but the companion's slot, which only
    /// carries (its keys open nothing, its lantern lights nothing, its things are weighed and
    /// counted for nothing). Anything in the raid that asks what a hero has goes through this, so
    /// the companion's slot is left out in one place.
    pub fn usable(&self) -> impl Iterator<Item = (Place, Stack)> {
        self.stacks().into_iter().filter(|(p, _)| *p != Place::Pet)
    }

    /// How many of `item` the hero can use ([`Inventory::usable`]).
    pub fn usable_count_of(&self, item: usize) -> u32 {
        self.usable()
            .filter(|(_, s)| usize::from(s.item) == item)
            .map(|(_, s)| u32::from(s.count))
            .sum()
    }

    /// Everything carried, weighed (spec §8.2). Items the content does not know weigh nothing,
    /// and the sum is always finite. The companion carries its own stack, so it weighs nothing.
    pub fn weight(&self, content: &Content) -> f32 {
        finite(
            self.usable()
                .filter_map(|(_, s)| {
                    let def = content.items.get(usize::from(s.item))?;
                    def.weight
                        .is_finite()
                        .then(|| f64::from(def.weight) * f64::from(s.count))
                })
                .sum(),
        )
    }

    /// How many of `item` are carried or worn, found or brought, in any slot.
    pub fn count_of(&self, item: usize) -> u32 {
        self.stacks()
            .iter()
            .filter(|(_, s)| usize::from(s.item) == item)
            .map(|(_, s)| u32::from(s.count))
            .sum()
    }

    /// What the worn gear adds up to; finite whatever the content holds.
    pub fn bonus(&self, content: &Content) -> Bonus {
        let (mut health, mut damage) = (0.0_f64, 0.0_f64);
        for s in self.equip.iter().flatten() {
            if let Some(def) = content.items.get(usize::from(s.item)) {
                if def.bonus.health.is_finite() {
                    health += f64::from(def.bonus.health);
                }
                if def.bonus.damage.is_finite() {
                    damage += f64::from(def.bonus.damage);
                }
            }
        }
        Bonus {
            health: finite(health),
            damage: finite(damage),
        }
    }

    /// The first backpack slot holding something of `kind`.
    pub fn first_of(&self, kind: ItemKind, content: &Content) -> Option<Place> {
        self.pack
            .iter()
            .position(|s| {
                s.and_then(|s| content.items.get(usize::from(s.item)))
                    .is_some_and(|def| def.kind == kind)
            })
            .map(|i| Place::Pack(i as u16))
    }

    /// The first empty backpack slot, or safe slot.
    pub fn first_empty(&self, in_safe: bool) -> Option<Place> {
        if in_safe {
            self.safe
                .iter()
                .position(Option::is_none)
                .map(|i| Place::Safe(i as u8))
        } else {
            self.pack
                .iter()
                .position(Option::is_none)
                .map(|i| Place::Pack(i as u16))
        }
    }

    /// `stack` into the backpack: onto stacks of the same item and the same `found` first, then
    /// into empty slots. What does not fit comes back; so does a stack of an item the content
    /// does not know, and a stack of none adds nothing.
    pub fn add(&mut self, mut stack: Stack, content: &Content) -> Option<Stack> {
        let Some(def) = content.items.get(usize::from(stack.item)) else {
            return Some(stack);
        };
        if stack.count == 0 {
            return None;
        }
        let max = def.stack.max(1);
        for s in self.pack.iter_mut().flatten() {
            if s.item == stack.item && s.found == stack.found && s.count < max {
                let n = (max - s.count).min(stack.count);
                s.count += n;
                stack.count -= n;
                if stack.count == 0 {
                    return None;
                }
            }
        }
        for s in self.pack.iter_mut() {
            if s.is_none() {
                let n = stack.count.min(max);
                *s = Some(Stack { count: n, ..stack });
                stack.count -= n;
                if stack.count == 0 {
                    return None;
                }
            }
        }
        Some(stack)
    }

    /// The whole stack at `at`. A bag comes off only by [`Inventory::move_to`], which minds its
    /// things.
    pub fn take(&mut self, at: Place) -> Option<Stack> {
        if matches!(at, Place::Bag(_)) {
            return None;
        }
        self.slot(at)?.take()
    }

    /// One of the stack at `at`. A stack of none is cleared away and gives nothing.
    pub fn take_one(&mut self, at: Place) -> Option<Stack> {
        if matches!(at, Place::Bag(_)) {
            return None;
        }
        let slot = self.slot(at)?;
        let s = slot.as_mut()?;
        let Some(left) = s.count.checked_sub(1) else {
            *slot = None;
            return None;
        };
        let one = Stack { count: 1, ..*s };
        if left == 0 {
            *slot = None;
        } else {
            s.count = left;
        }
        Some(one)
    }

    /// Whether `stack` of `def` may sit at `at`. A worn thing is one thing, so a pile does not go
    /// in a worn slot or a bag slot.
    fn fits(at: Place, stack: &Stack, def: &ItemDef) -> bool {
        match at {
            Place::Equip(s) => def.slot == Some(s) && stack.count == 1,
            Place::Bag(_) => def.kind == ItemKind::Bag && stack.count == 1,
            Place::Pack(_) | Place::Safe(_) | Place::Pet => true,
        }
    }

    /// Move the stack at `from` to `to`: onto a stack of the same item, or swapping with what is
    /// there. Wearing or taking off a bag resizes the backpack. Nothing changes when the move
    /// cannot be made.
    pub fn move_to(&mut self, from: Place, to: Place, content: &Content) -> Result<(), InvError> {
        if !self.exists(from) || !self.exists(to) {
            return Err(InvError::NoSuchPlace);
        }
        if from == to {
            return Ok(());
        }
        let before = self.clone();
        let done = self.try_move(from, to, content);
        if done.is_err() {
            *self = before;
        }
        done
    }

    fn try_move(&mut self, from: Place, to: Place, content: &Content) -> Result<(), InvError> {
        let moving = *self.get(from).ok_or(InvError::Empty)?;
        let def = content
            .items
            .get(usize::from(moving.item))
            .filter(|_| moving.count > 0)
            .ok_or(InvError::Empty)?;
        let there = self.get(to).copied();
        let max = def.stack.max(1);
        if let Some(t) = there {
            let pile = matches!(to, Place::Pack(_) | Place::Safe(_));
            if pile && t.item == moving.item && t.found == moving.found && max > 1 {
                let n = max.saturating_sub(t.count).min(moving.count);
                if n == 0 {
                    return Err(InvError::Full);
                }
                *self.slot(to).expect("checked") = Some(Stack {
                    count: t.count + n,
                    ..t
                });
                let left = moving.count - n;
                *self.slot(from).expect("checked") = (left > 0).then_some(Stack {
                    count: left,
                    ..moving
                });
                if matches!(from, Place::Bag(_)) {
                    self.fit_pack(content)?;
                }
                return Ok(());
            }
        }
        if !Self::fits(to, &moving, def) {
            return Err(InvError::WrongSlot);
        }
        if let Some(t) = there {
            let back = content
                .items
                .get(usize::from(t.item))
                .is_some_and(|d| t.count > 0 && Self::fits(from, &t, d));
            if !back {
                return Err(InvError::WrongSlot);
            }
        }
        *self.slot(to).expect("checked") = Some(moving);
        *self.slot(from).expect("checked") = there;
        if matches!(from, Place::Bag(_)) || matches!(to, Place::Bag(_)) {
            self.fit_pack(content)?;
        }
        Ok(())
    }

    /// The backpack resized to its bags: grown with empty slots, or shrunk by first dropping
    /// empty slots at its end, then packing its stacks to the front; refused when they would not
    /// fit. Shrinking compacts the backpack, which renumbers its slots.
    fn fit_pack(&mut self, content: &Content) -> Result<(), InvError> {
        let size = self.pack_size(content);
        while self.pack.len() > size && self.pack.last() == Some(&None) {
            self.pack.pop();
        }
        if self.pack.len() > size {
            let stacks: Vec<Stack> = self.pack.iter().flatten().copied().collect();
            if stacks.len() > size {
                return Err(InvError::BagInUse);
            }
            self.pack = stacks.into_iter().map(Some).collect();
        }
        self.pack.resize(size, None);
        Ok(())
    }

    /// Death (spec §8.2): what a death takes, by `mode`, out of the inventory; the safe slots are
    /// always kept. Under `found_only` only what was found is lost ([`Death::lost`]). When a
    /// found bag goes and brought stacks no longer fit the smaller backpack, those are not lost:
    /// they leave the inventory as [`Death::spill`], for the caller to file with the rest of the
    /// dead hero's kept things. Under the other modes everything but the safe slots is lost, so
    /// nothing spills.
    ///
    /// What stays is packed to the front of the backpack, which renumbers its slots.
    pub fn split_for_death(&mut self, mode: DeathLoss, content: &Content) -> Death {
        let lose = |s: &Stack| match mode {
            DeathLoss::Everything | DeathLoss::EverythingOneRecovery => true,
            DeathLoss::FoundOnly => s.found,
        };
        let mut death = Death::default();
        for slot in self
            .equip
            .iter_mut()
            .chain(self.bags.iter_mut())
            .chain(self.pack.iter_mut())
        {
            if slot.as_ref().is_some_and(&lose) {
                death.lost.extend(slot.take());
            }
        }
        let size = self.pack_size(content);
        let kept: Vec<Stack> = self.pack.iter().flatten().copied().collect();
        self.pack = vec![None; size];
        for (i, s) in kept.into_iter().enumerate() {
            if i < size {
                self.pack[i] = Some(s);
            } else {
                death.spill.push(s);
            }
        }
        death
    }

    /// Out of the raid: what was found is now simply carried.
    pub fn clear_found(&mut self) {
        for s in self
            .equip
            .iter_mut()
            .chain(self.bags.iter_mut())
            .chain(self.pack.iter_mut())
            .chain(self.safe.iter_mut())
            .chain(self.pet.iter_mut())
            .flatten()
        {
            s.found = false;
        }
    }

    /// An inventory from elsewhere (a client's loadout, an old stash file) made right for this
    /// content: unknown items and empty stacks go, counts are held to the stack size, things in
    /// slots they do not fit move to the backpack, and the backpack and safe slots take the
    /// rules' sizes. The backpack is repacked, which renumbers its slots. What has no room is
    /// returned, so the caller decides (a host leaves it out; the stash keeps it). So is what a
    /// count held to the stack size cut off, as stacks of their own: an old stash whose stack
    /// size shrank keeps its items.
    pub fn sanitize(&mut self, content: &Content) -> Vec<Stack> {
        let items = &content.items;
        let mut excess = Vec::new();
        let mut loose = Vec::new();
        if let Some(slot) = self.pet.as_mut() {
            *slot = slot.take().and_then(|s| clean(items, s, &mut excess));
        }
        for slot in EquipSlot::ALL {
            if let Some(st) = self.equip[slot.index()]
                .take()
                .and_then(|s| clean(items, s, &mut excess))
            {
                if items[usize::from(st.item)].slot == Some(slot) && st.count == 1 {
                    self.equip[slot.index()] = Some(st);
                } else {
                    loose.push(st);
                }
            }
        }
        for b in self.bags.iter_mut() {
            if let Some(st) = b.take().and_then(|s| clean(items, s, &mut excess)) {
                if items[usize::from(st.item)].kind == ItemKind::Bag && st.count == 1 {
                    *b = Some(st);
                } else {
                    loose.push(st);
                }
            }
        }
        let pack: Vec<Stack> = self
            .pack
            .drain(..)
            .flatten()
            .filter_map(|s| clean(items, s, &mut excess))
            .collect();
        let safe: Vec<Stack> = self
            .safe
            .drain(..)
            .flatten()
            .filter_map(|s| clean(items, s, &mut excess))
            .collect();
        let safe_size = usize::from(content.rules.run.safe_slots);
        self.safe = vec![None; safe_size];
        let mut out = Vec::new();
        for (i, s) in safe.into_iter().enumerate() {
            if i < safe_size {
                self.safe[i] = Some(s);
            } else {
                out.push(s);
            }
        }
        let size = self.pack_size(content);
        self.pack = vec![None; size];
        for (i, s) in pack.into_iter().chain(loose).enumerate() {
            if i < size {
                self.pack[i] = Some(s);
            } else {
                out.push(s);
            }
        }
        out.append(&mut excess);
        out
    }
}

/// `s` held to its item's stack size, the count cut off pushed on `excess` as stacks of their
/// own. `None` for an item the content does not know and for a stack of none.
fn clean(items: &[ItemDef], s: Stack, excess: &mut Vec<Stack>) -> Option<Stack> {
    let def = items.get(usize::from(s.item))?;
    if s.count == 0 {
        return None;
    }
    let max = def.stack.max(1);
    let mut cut = s.count.saturating_sub(max);
    while cut > 0 {
        let n = cut.min(max);
        excess.push(Stack { count: n, ..s });
        cut -= n;
    }
    Some(Stack {
        count: s.count.min(max),
        ..s
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentSources;

    fn content(run: &str) -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: &format!(
                r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                run: (backpack_slots: 3, safe_slots: 1, conjured_drink: "water", {run}))"#
            ),
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand), bonus: (damage: 0.2)),
                (key: "helm", name: "Helm", kind: Armour, icon: "a", weight: 2.0, slot: Some(Head),
                 bonus: (health: 50.0)),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 5),
                (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 2),
                (key: "sack", name: "Sack", kind: Bag, icon: "a", weight: 0.5, bag_slots: 1),
                (key: "lead", name: "Lead", kind: Material, icon: "a", weight: 3.0e38, stack: 10),
                (key: "titan_helm", name: "Titan Helm", kind: Armour, icon: "a", weight: 1.0,
                 slot: Some(Head), bonus: (health: 3.0e38, damage: 3.0e38)),
                (key: "titan_plate", name: "Titan Plate", kind: Armour, icon: "a", weight: 1.0,
                 slot: Some(Chest), bonus: (health: 3.0e38, damage: 3.0e38)),
                (key: "water", name: "Water", kind: Drink, icon: "a", weight: 0.5, stack: 5),
            ]"#,
            loot: "()",
            monsters: "[]",
        })
        .unwrap()
    }

    const KNIFE: usize = 0;
    const HELM: usize = 1;
    const BREAD: usize = 2;
    const GEM: usize = 3;
    const POUCH: usize = 4;
    const SACK: usize = 5;
    const LEAD: usize = 6;
    const TITAN_HELM: usize = 7;
    const TITAN_PLATE: usize = 8;
    /// An item index the test content does not have.
    const UNKNOWN: usize = 42;

    #[test]
    fn stacks_merge_then_fill_empty_slots_and_overflow_comes_back() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        assert_eq!((inv.pack.len(), inv.safe.len()), (3, 1));
        assert_eq!(inv.add(Stack::new(BREAD, 3), &c), None);
        assert_eq!(inv.add(Stack::new(BREAD, 4), &c), None);
        assert_eq!(inv.get(Place::Pack(0)).unwrap().count, 5);
        assert_eq!(inv.get(Place::Pack(1)).unwrap().count, 2);
        // A found stack does not merge with a brought one.
        assert_eq!(inv.add(Stack::found(BREAD, 1), &c), None);
        assert!(inv.get(Place::Pack(2)).unwrap().found);
        let left = inv.add(Stack::new(GEM, 4), &c);
        assert_eq!(left, Some(Stack::new(GEM, 4)));
        assert_eq!(inv.count_of(BREAD), 8);
    }

    #[test]
    fn gear_goes_only_in_its_slot_and_swaps() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(KNIFE, 1), &c);
        inv.add(Stack::new(HELM, 1), &c);
        assert_eq!(
            inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::Head), &c),
            Err(InvError::WrongSlot)
        );
        inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::MainHand), &c)
            .unwrap();
        inv.move_to(Place::Pack(1), Place::Equip(EquipSlot::Head), &c)
            .unwrap();
        assert_eq!(
            inv.bonus(&c),
            Bonus {
                health: 50.0,
                damage: 0.2
            }
        );
        inv.add(Stack::new(BREAD, 1), &c);
        // Bread cannot go where the helm was, so the helm cannot swap with it.
        assert_eq!(
            inv.move_to(Place::Equip(EquipSlot::Head), Place::Pack(0), &c),
            Err(InvError::WrongSlot)
        );
        inv.move_to(Place::Equip(EquipSlot::Head), Place::Pack(1), &c)
            .unwrap();
        assert_eq!(inv.get(Place::Pack(1)).unwrap().item, HELM as u16);
        assert!(inv.get(Place::Equip(EquipSlot::Head)).is_none());
    }

    #[test]
    fn a_bag_comes_off_only_when_its_things_fit() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(POUCH, 1), &c);
        inv.move_to(Place::Pack(0), Place::Bag(0), &c).unwrap();
        assert_eq!(inv.pack_size(&c), 5);
        assert_eq!(inv.pack.len(), 5);
        for item in [BREAD, GEM, KNIFE, HELM] {
            inv.add(Stack::new(item, 1), &c);
        }
        // Four stacks and the pouch itself would need 5 of the 3 slots left.
        assert_eq!(
            inv.move_to(Place::Bag(0), Place::Pack(4), &c),
            Err(InvError::BagInUse)
        );
        assert_eq!(inv.pack.len(), 5);
        assert!(inv.get(Place::Bag(0)).is_some());
        inv.take(Place::Pack(3));
        inv.take(Place::Pack(2));
        inv.move_to(Place::Bag(0), Place::Pack(4), &c).unwrap();
        assert_eq!(inv.pack.len(), 3);
        assert_eq!(inv.count_of(POUCH), 1);
        assert_eq!(inv.count_of(BREAD) + inv.count_of(GEM), 2);
        // Wearing the smaller sack where the pouch was: 4 slots, and the 3 stacks fit.
        inv.take(Place::Pack(0));
        inv.add(Stack::new(SACK, 1), &c);
        let sack = inv
            .pack
            .iter()
            .position(|s| s.is_some_and(|s| s.item == SACK as u16));
        inv.move_to(Place::Pack(sack.unwrap() as u16), Place::Bag(1), &c)
            .unwrap();
        assert_eq!(inv.pack.len(), 4);
        assert_eq!(
            inv.take(Place::Bag(1)),
            None,
            "a bag comes off only by moving it"
        );
    }

    #[test]
    fn weight_counts_everything_carried() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(GEM, 4), &c);
        inv.add(Stack::new(KNIFE, 1), &c);
        inv.move_to(Place::Pack(1), Place::Equip(EquipSlot::MainHand), &c)
            .unwrap();
        inv.add(Stack::new(BREAD, 2), &c);
        inv.move_to(Place::Pack(1), Place::Safe(0), &c).unwrap();
        assert!((inv.weight(&c) - (4.0 + 1.0 + 1.0)).abs() < 1e-5);
        assert_eq!(inv.first_of(ItemKind::Valuable, &c), Some(Place::Pack(0)));
        assert_eq!(inv.first_empty(false), Some(Place::Pack(1)));
        assert_eq!(inv.first_empty(true), None);
    }

    #[test]
    fn death_takes_what_its_mode_says_and_never_the_safe_slot() {
        let c = content("");
        let filled = || {
            let mut inv = Inventory::new(&c);
            inv.add(Stack::new(KNIFE, 1), &c);
            inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::MainHand), &c)
                .unwrap();
            inv.add(Stack::found(POUCH, 1), &c);
            inv.move_to(Place::Pack(0), Place::Bag(0), &c).unwrap();
            inv.add(Stack::new(BREAD, 2), &c);
            inv.add(Stack::found(GEM, 3), &c);
            inv.add(Stack::new(HELM, 1), &c);
            inv.add(Stack::new(SACK, 1), &c);
            inv.add(Stack::found(BREAD, 1), &c);
            inv.move_to(Place::Pack(4), Place::Safe(0), &c).unwrap();
            inv.add(Stack::new(KNIFE, 1), &c);
            inv
        };
        let mut all = filled();
        let death = all.split_for_death(DeathLoss::Everything, &c);
        assert_eq!(death.lost.len(), 7);
        assert!(
            death.spill.is_empty(),
            "everything is lost, so nothing spills"
        );
        assert_eq!(all.stacks(), vec![(Place::Safe(0), Stack::found(BREAD, 1))]);
        assert_eq!(all.pack.len(), 3);
        let mut found = filled();
        let death = found.split_for_death(DeathLoss::FoundOnly, &c);
        // The pouch and the gem were found, so they are lost. With the pouch gone, the four
        // brought stacks left in the pack must fit 3 slots: the last (the second knife) does not,
        // but it was brought, so it spills instead of being lost.
        assert!(death.lost.contains(&Stack::found(POUCH, 1)));
        assert!(death.lost.contains(&Stack::found(GEM, 3)));
        assert_eq!(death.lost.len(), 2);
        assert_eq!(death.spill, vec![Stack::new(KNIFE, 1)]);
        assert_eq!(found.count_of(KNIFE), 1);
        assert_eq!(found.get(Place::Safe(0)), Some(&Stack::found(BREAD, 1)));
        // Nothing made, nothing lost: 8 stacks before, 8 between what is kept, spilled and lost.
        assert_eq!(
            found.stacks().len() + death.lost.len() + death.spill.len(),
            8
        );
    }

    #[test]
    fn sanitize_returns_what_has_no_room() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.equip[EquipSlot::Head.index()] = Some(Stack::new(KNIFE, 1));
        inv.bags[0] = Some(Stack::new(GEM, 1));
        inv.pack = vec![
            Some(Stack::new(BREAD, 7)),
            Some(Stack::new(UNKNOWN, 1)),
            Some(Stack::new(GEM, 0)),
            Some(Stack::new(HELM, 1)),
            Some(Stack::new(GEM, 2)),
        ];
        inv.safe = vec![Some(Stack::new(GEM, 1)), Some(Stack::new(BREAD, 1))];
        let left = inv.sanitize(&c);
        assert!(inv.get(Place::Equip(EquipSlot::Head)).is_none());
        assert!(inv.bags.iter().all(Option::is_none));
        assert_eq!(inv.pack.len(), 3);
        assert_eq!(inv.safe.len(), 1);
        assert_eq!(inv.get(Place::Pack(0)), Some(&Stack::new(BREAD, 5)));
        assert_eq!(inv.get(Place::Safe(0)), Some(&Stack::new(GEM, 1)));
        // The pack keeps bread, helm and gems; the second safe stack, the misplaced knife and
        // the bag-slot gem have no room left, and the 2 bread over the
        // stack size of 5 come back as a stack of their own.
        assert_eq!(left.len(), 4);
        assert!(left.contains(&Stack::new(BREAD, 2)));
        assert!(left.contains(&Stack::new(KNIFE, 1)));
    }

    #[test]
    fn a_failed_move_changes_nothing_and_take_one_takes_one() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(BREAD, 3), &c);
        let before = inv.clone();
        assert_eq!(
            inv.move_to(Place::Pack(1), Place::Pack(0), &c),
            Err(InvError::Empty)
        );
        assert_eq!(
            inv.move_to(Place::Pack(0), Place::Pack(9), &c),
            Err(InvError::NoSuchPlace)
        );
        assert_eq!(inv, before);
        assert_eq!(inv.take_one(Place::Pack(0)), Some(Stack::new(BREAD, 1)));
        assert_eq!(inv.get(Place::Pack(0)).unwrap().count, 2);
        inv.clear_found();
        assert_eq!(inv.take(Place::Pack(0)), Some(Stack::new(BREAD, 2)));
        assert!(inv.is_empty());
    }

    #[test]
    fn a_stack_of_none_or_of_an_unknown_item_is_never_added() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        let before = inv.clone();
        assert_eq!(inv.add(Stack::new(BREAD, 0), &c), None);
        assert_eq!(inv, before, "nothing to add leaves no empty stack behind");
        assert_eq!(
            inv.add(Stack::new(UNKNOWN, 2), &c),
            Some(Stack::new(UNKNOWN, 2)),
            "an item the content lacks comes back"
        );
        assert_eq!(inv, before);
    }

    #[test]
    fn bad_stacks_already_inside_are_refused_or_skipped_not_indexed() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.pack[0] = Some(Stack::new(BREAD, 0));
        inv.pack[1] = Some(Stack::new(UNKNOWN, 3));
        inv.pack[2] = Some(Stack::new(BREAD, 9));
        // A stack of none gives nothing and is cleared away.
        assert_eq!(inv.take_one(Place::Pack(0)), None);
        assert!(inv.get(Place::Pack(0)).is_none());
        // Moving a stack of none, or of an unknown item, is refused and changes nothing.
        inv.pack[0] = Some(Stack::new(BREAD, 0));
        let before = inv.clone();
        assert_eq!(
            inv.move_to(Place::Pack(0), Place::Safe(0), &c),
            Err(InvError::Empty)
        );
        assert_eq!(
            inv.move_to(Place::Pack(1), Place::Safe(0), &c),
            Err(InvError::Empty)
        );
        // Swapping with an unknown item cannot be undone into a worn slot.
        assert_eq!(
            inv.move_to(Place::Pack(1), Place::Equip(EquipSlot::Head), &c),
            Err(InvError::Empty)
        );
        assert_eq!(inv, before);
        // A pile past the stack size is full, not a subtraction that wraps.
        inv.safe[0] = Some(Stack::new(BREAD, 9));
        assert_eq!(
            inv.move_to(Place::Pack(2), Place::Safe(0), &c),
            Err(InvError::Full)
        );
        // A pile cannot be worn.
        inv.pack[2] = Some(Stack::new(HELM, 2));
        assert_eq!(
            inv.move_to(Place::Pack(2), Place::Equip(EquipSlot::Head), &c),
            Err(InvError::WrongSlot)
        );
        // Weight and bonus skip what the content does not know.
        assert!(inv.weight(&c).is_finite());
        inv.equip[EquipSlot::Head.index()] = Some(Stack::new(UNKNOWN, 1));
        assert_eq!(inv.bonus(&c), Bonus::default());
        assert_eq!(inv.first_of(ItemKind::Weapon, &c), None);
        assert_eq!(inv.pack_size(&c), 3);
    }

    #[test]
    fn places_past_the_end_are_refused() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(BREAD, 1), &c);
        let before = inv.clone();
        for bad in [
            Place::Pack(3),
            Place::Safe(1),
            Place::Bag(4),
            Place::Pack(u16::MAX),
        ] {
            assert_eq!(inv.get(bad), None);
            assert_eq!(inv.take(bad), None);
            assert_eq!(inv.take_one(bad), None);
            assert_eq!(
                inv.move_to(Place::Pack(0), bad, &c),
                Err(InvError::NoSuchPlace)
            );
            assert_eq!(
                inv.move_to(bad, Place::Pack(0), &c),
                Err(InvError::NoSuchPlace)
            );
        }
        assert_eq!(inv, before);
    }

    #[test]
    fn weight_and_bonus_stay_finite_whatever_is_carried() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.pack[0] = Some(Stack::new(LEAD, u16::MAX));
        inv.pack[1] = Some(Stack::new(LEAD, u16::MAX));
        inv.equip[EquipSlot::Head.index()] = Some(Stack::new(TITAN_HELM, 1));
        inv.equip[EquipSlot::Chest.index()] = Some(Stack::new(TITAN_PLATE, 1));
        assert_eq!(inv.weight(&c), f32::MAX);
        let b = inv.bonus(&c);
        assert!(b.health.is_finite() && b.damage.is_finite());
        assert_eq!(b.health, f32::MAX);
    }

    #[test]
    fn sanitize_drops_what_the_content_no_longer_knows_and_stays_sound() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.equip[EquipSlot::Head.index()] = Some(Stack::new(UNKNOWN, 1));
        inv.equip[EquipSlot::MainHand.index()] = Some(Stack::new(GEM, 2));
        inv.bags[0] = Some(Stack::new(UNKNOWN, 1));
        inv.bags[1] = Some(Stack::new(POUCH, 1));
        inv.pack = vec![Some(Stack::new(UNKNOWN, 5)); 7];
        inv.safe = vec![Some(Stack::new(UNKNOWN, 5)); 4];
        let left = inv.sanitize(&c);
        // The misplaced gems move to the backpack, which the worn pouch has widened.
        assert_eq!(left, vec![]);
        assert_eq!(inv.pack.len(), 5);
        assert_eq!(inv.safe, vec![None]);
        assert_eq!(inv.get(Place::Bag(0)), None);
        assert_eq!(inv.get(Place::Bag(1)), Some(&Stack::new(POUCH, 1)));
        assert_eq!(inv.get(Place::Pack(0)), Some(&Stack::new(GEM, 2)));
        assert!(inv.get(Place::Equip(EquipSlot::MainHand)).is_none());
        // Sound afterwards: all of it is known, and a second pass changes nothing.
        let again = inv.clone();
        assert_eq!(inv.sanitize(&c), vec![]);
        assert_eq!(inv, again);
        assert!(inv.weight(&c).is_finite());
    }

    /// Every item's count carried plus every stack that left the inventory.
    fn totals(c: &Content, inv: &Inventory, outside: &[Stack]) -> Vec<u32> {
        (0..c.items.len())
            .map(|i| {
                let out: u32 = outside
                    .iter()
                    .filter(|s| usize::from(s.item) == i)
                    .map(|s| u32::from(s.count))
                    .sum();
                inv.count_of(i) + out
            })
            .collect()
    }

    fn in_pack(inv: &Inventory, item: usize) -> Place {
        let i = inv
            .pack
            .iter()
            .position(|s| s.is_some_and(|s| usize::from(s.item) == item))
            .expect("in the backpack");
        Place::Pack(i as u16)
    }

    /// Adds `stack` and records it as wanted, keeping what did not fit outside.
    fn give(
        c: &Content,
        inv: &mut Inventory,
        outside: &mut Vec<Stack>,
        want: &mut [u32],
        stack: Stack,
    ) {
        want[usize::from(stack.item)] += u32::from(stack.count);
        outside.extend(inv.add(stack, c));
    }

    #[test]
    fn nothing_is_made_or_lost_by_any_operation() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        let mut outside = Vec::new();
        let mut want = vec![0u32; c.items.len()];
        let check = |inv: &Inventory, outside: &[Stack], want: &[u32], what: &str| {
            assert_eq!(totals(&c, inv, outside), want, "after {what}");
        };
        for stack in [
            Stack::found(POUCH, 1),
            Stack::new(SACK, 1),
            Stack::new(HELM, 1),
            Stack::new(BREAD, 3),
            Stack::new(BREAD, 4),
            Stack::found(GEM, 12),
            Stack::new(KNIFE, 1),
        ] {
            give(&c, &mut inv, &mut outside, &mut want, stack);
        }
        assert_eq!(outside.len(), 4, "a full backpack hands the rest back");
        check(&inv, &outside, &want, "adds");
        // Wearing bags widens the backpack, then what was handed back goes in.
        inv.move_to(in_pack(&inv, POUCH), Place::Bag(0), &c)
            .unwrap();
        inv.move_to(in_pack(&inv, SACK), Place::Bag(1), &c).unwrap();
        check(&inv, &outside, &want, "wearing bags");
        for s in std::mem::take(&mut outside) {
            outside.extend(inv.add(s, &c));
        }
        assert!(outside.is_empty());
        check(&inv, &outside, &want, "re-adding");
        // Merges: refused when full, partial, then whole.
        let bread = |inv: &Inventory| {
            (
                inv.get(Place::Pack(0)).unwrap().count,
                inv.get(Place::Pack(1)).map(|s| s.count),
            )
        };
        assert_eq!(bread(&inv), (5, Some(2)));
        assert_eq!(
            inv.move_to(Place::Pack(1), Place::Pack(0), &c),
            Err(InvError::Full)
        );
        check(&inv, &outside, &want, "a refused merge");
        outside.extend(inv.take_one(Place::Pack(0)));
        inv.move_to(Place::Pack(1), Place::Pack(0), &c).unwrap();
        assert_eq!(bread(&inv), (5, Some(1)), "a partial merge");
        check(&inv, &outside, &want, "a partial merge");
        outside.extend(inv.take_one(Place::Pack(0)));
        inv.move_to(Place::Pack(1), Place::Pack(0), &c).unwrap();
        assert_eq!(bread(&inv), (5, None), "a whole merge");
        check(&inv, &outside, &want, "a whole merge");
        // The same for the found gems.
        let gems = in_pack(&inv, GEM);
        assert_eq!(gems, Place::Pack(3));
        outside.extend(inv.take_one(gems));
        inv.move_to(Place::Pack(4), gems, &c).unwrap();
        outside.extend(inv.take_one(gems));
        inv.move_to(Place::Pack(4), gems, &c).unwrap();
        assert!(inv.get(Place::Pack(4)).is_none());
        check(&inv, &outside, &want, "gem merges");
        // A swap of worn gear.
        give(
            &c,
            &mut inv,
            &mut outside,
            &mut want,
            Stack::new(TITAN_HELM, 1),
        );
        inv.move_to(in_pack(&inv, HELM), Place::Equip(EquipSlot::Head), &c)
            .unwrap();
        inv.move_to(in_pack(&inv, TITAN_HELM), Place::Equip(EquipSlot::Head), &c)
            .unwrap();
        assert!(inv.pack.iter().flatten().any(|s| s.item == HELM as u16));
        check(&inv, &outside, &want, "a gear swap");
        // A bag for a bag: the bigger pouch where the sack was, then back.
        give(&c, &mut inv, &mut outside, &mut want, Stack::new(POUCH, 1));
        inv.move_to(in_pack(&inv, POUCH), Place::Bag(1), &c)
            .unwrap();
        assert_eq!(inv.pack.len(), 7);
        check(&inv, &outside, &want, "wearing the pouch over the sack");
        inv.move_to(in_pack(&inv, SACK), Place::Bag(1), &c).unwrap();
        assert_eq!(inv.pack.len(), 6);
        check(&inv, &outside, &want, "the sack back over the pouch");
        // Fill up so a found bag leaving spills a brought stack, and keep a gem in the safe.
        give(&c, &mut inv, &mut outside, &mut want, Stack::new(KNIFE, 1));
        inv.move_to(in_pack(&inv, GEM), Place::Safe(0), &c).unwrap();
        check(&inv, &outside, &want, "the safe slot");
        // The companion's slot: a stack set there, swapped with the backpack's knife and back.
        inv.pet = Some(Some(Stack::found(GEM, 3)));
        want[GEM] += 3;
        check(&inv, &outside, &want, "the companion's slot");
        let knife = in_pack(&inv, KNIFE);
        inv.move_to(knife, Place::Pet, &c).unwrap();
        assert_eq!(inv.get(Place::Pet), Some(&Stack::new(KNIFE, 1)));
        check(&inv, &outside, &want, "a swap into the companion's slot");
        inv.move_to(Place::Pet, knife, &c).unwrap();
        assert_eq!(inv.get(Place::Pet), Some(&Stack::found(GEM, 3)));
        check(&inv, &outside, &want, "a swap out of the companion's slot");
        for mode in [
            DeathLoss::Everything,
            DeathLoss::EverythingOneRecovery,
            DeathLoss::FoundOnly,
        ] {
            let mut dead = inv.clone();
            let death = dead.split_for_death(mode, &c);
            let mut gone = outside.clone();
            gone.extend(death.lost.iter().chain(&death.spill));
            assert_eq!(totals(&c, &dead, &gone), want, "death by {mode:?}");
            assert_eq!(dead.get(Place::Safe(0)), inv.get(Place::Safe(0)));
            assert_eq!(dead.get(Place::Pet), inv.get(Place::Pet), "the companion's");
            match mode {
                DeathLoss::FoundOnly => {
                    assert!(death.lost.iter().all(|s| s.found));
                    assert_eq!(death.spill.len(), 1, "one brought stack lost its slot");
                    assert!(death.spill.iter().all(|s| !s.found));
                }
                _ => assert!(death.spill.is_empty()),
            }
        }
        // Taking a bag off, then a clean-up pass over the lot.
        outside.extend(inv.take(in_pack(&inv, KNIFE)));
        let free = inv.first_empty(false).expect("the knife left a slot");
        inv.move_to(Place::Bag(1), free, &c).unwrap();
        check(&inv, &outside, &want, "taking a bag off");
        let mut cleaned = inv.clone();
        let left = cleaned.sanitize(&c);
        let mut gone = outside.clone();
        gone.extend(left);
        assert_eq!(totals(&c, &cleaned, &gone), want, "sanitize");
    }

    #[test]
    fn the_companion_keeps_its_stack_through_every_death() {
        let c = content("");
        for mode in [
            DeathLoss::Everything,
            DeathLoss::EverythingOneRecovery,
            DeathLoss::FoundOnly,
        ] {
            let mut inv = Inventory::new(&c);
            inv.pet = Some(Some(Stack::found(GEM, 4)));
            inv.add(Stack::found(BREAD, 1), &c);
            let death = inv.split_for_death(mode, &c);
            assert_eq!(inv.get(Place::Pet), Some(&Stack::found(GEM, 4)), "{mode:?}");
            assert!(death.lost.iter().all(|s| s.item != GEM as u16));
            assert!(death.spill.iter().all(|s| s.item != GEM as u16));
        }
        let mut inv = Inventory::new(&c);
        inv.pet = Some(Some(Stack::new(GEM, 10)));
        assert_eq!(
            inv.weight(&c),
            0.0,
            "the companion carries it, not the hero"
        );
        inv.add(Stack::new(BREAD, 2), &c);
        inv.move_to(Place::Pack(0), Place::Pet, &c).unwrap();
        assert_eq!(inv.get(Place::Pet), Some(&Stack::new(BREAD, 2)));
        assert_eq!(inv.get(Place::Pack(0)), Some(&Stack::new(GEM, 10)));
        assert!(inv.sanitize(&c).is_empty());
        assert_eq!(inv.get(Place::Pet), Some(&Stack::new(BREAD, 2)));
        let mut none = Inventory::new(&c);
        none.add(Stack::new(BREAD, 1), &c);
        assert_eq!(
            none.move_to(Place::Pack(0), Place::Pet, &c),
            Err(InvError::NoSuchPlace)
        );
        // Out of the raid, what was found there is simply carried.
        let mut out = Inventory::new(&c);
        out.pet = Some(Some(Stack::found(GEM, 2)));
        out.clear_found();
        assert_eq!(out.get(Place::Pet), Some(&Stack::new(GEM, 2)));
    }

    #[test]
    fn dying_by_every_mode_keeps_the_safe_slot_and_takes_the_rest() {
        let c = content("");
        let filled = || {
            let mut inv = Inventory::new(&c);
            inv.add(Stack::new(KNIFE, 1), &c);
            inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::MainHand), &c)
                .unwrap();
            inv.add(Stack::new(BREAD, 2), &c);
            inv.add(Stack::found(GEM, 2), &c);
            inv.move_to(Place::Pack(1), Place::Safe(0), &c).unwrap();
            inv.add(Stack::found(GEM, 1), &c);
            inv
        };
        // Everything and EverythingOneRecovery both take all but the safe slot: the one-recovery
        // part (the body waiting in the next raid) is the caller's.
        for mode in [DeathLoss::Everything, DeathLoss::EverythingOneRecovery] {
            let mut inv = filled();
            let death = inv.split_for_death(mode, &c);
            assert_eq!(death.lost.len(), 3, "{mode:?}");
            assert!(death.spill.is_empty(), "{mode:?}");
            assert_eq!(inv.stacks().len(), 1, "{mode:?}");
            assert_eq!(inv.get(Place::Safe(0)), Some(&Stack::found(GEM, 2)));
        }
        let mut inv = filled();
        let death = inv.split_for_death(DeathLoss::FoundOnly, &c);
        assert_eq!(death.lost, vec![Stack::found(GEM, 1)]);
        assert!(death.spill.is_empty());
        assert_eq!(inv.count_of(KNIFE) + inv.count_of(BREAD), 3);
    }

    #[test]
    fn the_companions_slot_only_carries() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        let slots = inv.pack_size(&c);
        // A bag there widens nothing, and gear there is worth nothing.
        inv.pet = Some(Some(Stack::new(POUCH, 1)));
        assert_eq!(inv.pack_size(&c), slots);
        inv.pet = Some(Some(Stack::new(TITAN_HELM, 1)));
        let bonus = inv.bonus(&c);
        assert_eq!((bonus.health, bonus.damage), (0.0, 0.0));
        // Nothing in it is usable in the raid or weighed, though it is counted as carried.
        assert_eq!(inv.usable().count(), 0);
        assert_eq!(inv.usable_count_of(TITAN_HELM), 0);
        assert_eq!(inv.count_of(TITAN_HELM), 1);
        assert_eq!(inv.weight(&c), 0.0);
        inv.add(Stack::new(BREAD, 1), &c);
        assert_eq!(inv.usable().count(), 1);
    }

    #[test]
    fn clear_found_clears_it_everywhere() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.equip[EquipSlot::Head.index()] = Some(Stack::found(HELM, 1));
        inv.bags[0] = Some(Stack::found(POUCH, 1));
        inv.pack = vec![
            Some(Stack::found(BREAD, 2)),
            None,
            Some(Stack::found(GEM, 1)),
        ];
        inv.safe = vec![Some(Stack::found(GEM, 4))];
        assert_eq!(inv.stacks().len(), 5);
        assert!(inv.stacks().iter().all(|(_, s)| s.found));
        inv.clear_found();
        assert_eq!(inv.stacks().len(), 5);
        assert!(inv.stacks().iter().all(|(_, s)| !s.found));
        assert_eq!(inv.get(Place::Safe(0)), Some(&Stack::new(GEM, 4)));
    }

    #[test]
    fn sanitize_hands_back_what_a_smaller_stack_size_cuts_off() {
        // An old stash held 12 bread to a stack and 11 gems; bread now stacks to 5, gems to 10.
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.pack = vec![Some(Stack::found(BREAD, 12)), Some(Stack::new(GEM, 11))];
        inv.safe = vec![Some(Stack::new(BREAD, 7))];
        let before = (inv.count_of(BREAD), inv.count_of(GEM));
        let left = inv.sanitize(&c);
        assert_eq!(inv.get(Place::Pack(0)), Some(&Stack::found(BREAD, 5)));
        assert_eq!(inv.get(Place::Pack(1)), Some(&Stack::new(GEM, 10)));
        assert_eq!(inv.get(Place::Safe(0)), Some(&Stack::new(BREAD, 5)));
        assert!(left
            .iter()
            .all(|s| s.count <= c.items[usize::from(s.item)].stack));
        let sum = |item: usize| -> u32 {
            left.iter()
                .filter(|s| usize::from(s.item) == item)
                .map(|s| u32::from(s.count))
                .sum()
        };
        assert_eq!(inv.count_of(BREAD) + sum(BREAD), before.0);
        assert_eq!(inv.count_of(GEM) + sum(GEM), before.1);
        assert!(left.contains(&Stack::found(BREAD, 5)));
        assert!(left.contains(&Stack::found(BREAD, 2)));
        assert!(left.contains(&Stack::new(BREAD, 2)));
        assert!(left.contains(&Stack::new(GEM, 1)));
        assert_eq!(left.len(), 4);
    }
}
