//! The inn's stations (spec §8.5), its recipes and its trader, on the player's [`Profile`]: a
//! station is locked until a contract unlocks it (or open from the start), then upgraded level by
//! level with gold and materials from the stash, its gifts kept forever (stash room, recipes, the
//! map table's intel, a companion). Every purchase pays and gives, or does neither.
//!
//! Materials come from the stash only (not the loadout, unlike a contract's turn-in): the panel
//! shows the stash's count next to each price. Only a raid's return may grow the stash past its
//! slots, so a craft or a purchase that would not fit is refused, whole.

use crate::{Content, Gift, InnContent, Profile, SavedStack};

/// Why an upgrade, a craft or a trade was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationError {
    InRaid,
    Unknown,
    Locked,
    MaxLevel,
    NoGold,
    Missing,
    NoRecipe,
    Worthless,
    StashFull,
}

impl StationError {
    /// The line the panel shows.
    pub fn message(self) -> &'static str {
        match self {
            StationError::InRaid => "Not while a raid is under way.",
            StationError::Unknown => "No such thing here.",
            StationError::Locked => "Not yet: a contract opens it.",
            StationError::MaxLevel => "Built as far as it goes.",
            StationError::NoGold => "Not enough gold.",
            StationError::Missing => "Not enough of what it needs in the stash.",
            StationError::NoRecipe => "You do not know how to make that yet.",
            StationError::Worthless => "Nobody would pay for that.",
            StationError::StashFull => "The stash has no room for it.",
        }
    }
}

impl Profile {
    /// Station `s`'s level: `None` while locked (or unknown), else the levels built. A station
    /// the profile has but the content no longer does is simply not found.
    pub fn station_level(&self, s: usize, inn: &InnContent) -> Option<u8> {
        let def = inn.stations.get(s)?;
        match self.inn.stations.iter().find(|(k, _)| *k == def.key) {
            Some((_, level)) => Some(*level),
            None => def.open.then_some(0),
        }
    }

    /// How many of `item` the stash holds.
    pub fn stashed(&self, item: &str) -> u32 {
        self.stash
            .iter()
            .filter(|s| s.item == item)
            .fold(0u32, |n, s| n.saturating_add(u32::from(s.count)))
    }

    /// Whether `gold` and `items` (from the stash) can be paid. A line that names an item twice
    /// needs both counts at once.
    fn can_pay(
        &self,
        gold: u32,
        items: &[(usize, u32)],
        content: &Content,
    ) -> Result<(), StationError> {
        for &(i, _) in items {
            let asked = items
                .iter()
                .filter(|&&(j, _)| j == i)
                .fold(0u32, |n, &(_, c)| n.saturating_add(c));
            if self.stashed(&content.items[i].key) < asked {
                return Err(StationError::Missing);
            }
        }
        if self.gold < gold {
            return Err(StationError::NoGold);
        }
        Ok(())
    }

    /// `gold` and `items` paid out of the stash; [`Profile::can_pay`] said they can be.
    fn pay(&mut self, gold: u32, items: &[(usize, u32)], content: &Content) {
        self.gold = self.gold.saturating_sub(gold);
        for &(i, n) in items {
            let key = &content.items[i].key;
            let mut left = n;
            for s in self.stash.iter_mut().filter(|s| s.item == *key) {
                // At most the stack's own count, so it fits a `u16`.
                let k = left.min(u32::from(s.count));
                s.count -= k as u16;
                left -= k;
            }
        }
        self.stash.retain(|s| s.count > 0);
    }

    /// Station `s` up one level: its price paid (the gold, and the materials from the stash,
    /// not the loadout), its gifts given. The new level. A companion gift replaces the one
    /// already kept; every other gift only adds.
    pub fn upgrade_station(
        &mut self,
        s: usize,
        inn: &InnContent,
        content: &Content,
    ) -> Result<u8, StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let def = inn.stations.get(s).ok_or(StationError::Unknown)?;
        let level = self.station_level(s, inn).ok_or(StationError::Locked)?;
        let next = def
            .levels
            .get(usize::from(level))
            .ok_or(StationError::MaxLevel)?;
        self.can_pay(next.gold, &next.items, content)?;
        self.pay(next.gold, &next.items, content);
        for g in &next.gives {
            match g {
                Gift::StashSlots(n) => {
                    self.inn.stash_bonus = self.inn.stash_bonus.saturating_add(*n);
                }
                Gift::Recipe(r) => {
                    let key = &inn.recipes[*r].key;
                    if !self.inn.recipes.contains(key) {
                        self.inn.recipes.push(key.clone());
                    }
                }
                Gift::Intel => self.inn.intel = true,
                Gift::Companion(c) => self.inn.companion = Some(inn.companions[*c].key.clone()),
            }
        }
        let new = level.saturating_add(1);
        match self.inn.stations.iter_mut().find(|(k, _)| *k == def.key) {
            Some(entry) => entry.1 = new,
            None => self.inn.stations.push((def.key.clone(), new)),
        }
        Ok(new)
    }

    /// Whether recipe `r` has been learned.
    pub fn knows(&self, r: usize, inn: &InnContent) -> bool {
        inn.recipes
            .get(r)
            .is_some_and(|d| self.inn.recipes.contains(&d.key))
    }

    /// Recipe `r` made: its gold and its materials (from the stash, not the loadout) paid, what
    /// it makes into the stash. The room is counted after the materials are gone, since they
    /// free theirs.
    pub fn craft(
        &mut self,
        r: usize,
        inn: &InnContent,
        content: &Content,
    ) -> Result<(), StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let def = inn.recipes.get(r).ok_or(StationError::Unknown)?;
        if !self.knows(r, inn) {
            return Err(StationError::NoRecipe);
        }
        self.can_pay(def.gold, &def.needs, content)?;
        let (item, count) = def.makes;
        let made = SavedStack {
            item: content.items[item].key.clone(),
            count,
        };
        // Pay on a copy first: a refusal for room must leave the real stash and purse alone.
        let mut after = self.clone();
        after.pay(def.gold, &def.needs, content);
        if !after.stash_has_room(&made, content) {
            return Err(StationError::StashFull);
        }
        after.stash_add(made, content);
        *self = after;
        Ok(())
    }

    /// One of the trader's stock line `stock`, into the stash, for its price.
    pub fn buy(
        &mut self,
        stock: usize,
        inn: &InnContent,
        content: &Content,
    ) -> Result<(), StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let &(item, price) = inn.trader.stock.get(stock).ok_or(StationError::Unknown)?;
        if self.gold < price {
            return Err(StationError::NoGold);
        }
        let bought = SavedStack {
            item: content.items[item].key.clone(),
            count: 1,
        };
        if !self.stash_has_room(&bought, content) {
            return Err(StationError::StashFull);
        }
        self.gold -= price;
        self.stash_add(bought, content);
        Ok(())
    }

    /// The stash stack at `index` sold to the trader, at their share of its value. The gold.
    pub fn sell_to_trader(
        &mut self,
        index: usize,
        inn: &InnContent,
        content: &Content,
    ) -> Result<u32, StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let s = self.stash.get(index).ok_or(StationError::Unknown)?;
        let item = content.item(&s.item).ok_or(StationError::Unknown)?;
        let worth = content.items[item].value.saturating_mul(u32::from(s.count));
        // A float to u32 `as` saturates, so a huge stack cannot wrap.
        let gold = (worth as f32 * inn.trader.sells_for).floor() as u32;
        if gold == 0 {
            return Err(StationError::Worthless);
        }
        self.stash.remove(index);
        self.gold = self.gold.saturating_add(gold);
        Ok(gold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::tests::{inn, raid};
    use crate::{InnProgress, SavedInventory, SavedRaid};

    fn stack(item: &str, count: u16) -> SavedStack {
        SavedStack {
            item: item.into(),
            count,
        }
    }

    #[test]
    fn a_station_is_locked_until_unlocked_unless_open() {
        let c = raid();
        let inn = inn(&c);
        let p = Profile::default();
        assert_eq!(
            p.station_level(0, &inn),
            Some(0),
            "the stash is open from the start"
        );
        assert_eq!(p.station_level(1, &inn), None);
        let mut q = Profile {
            gold: 40,
            inn: InnProgress {
                stations: vec![("bench".into(), 0)],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        assert_eq!(q.station_level(1, &inn), Some(0));
        assert_eq!(q.upgrade_station(1, &inn, &c), Ok(1));
        assert!(q.knows(0, &inn));
    }

    #[test]
    fn a_locked_station_refuses_an_upgrade() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 1000,
            ..Profile::default()
        };
        assert_eq!(p.upgrade_station(1, &inn, &c), Err(StationError::Locked));
        assert_eq!(p.upgrade_station(9, &inn, &c), Err(StationError::Unknown));
        assert_eq!(p.gold, 1000);
    }

    #[test]
    fn an_upgrade_pays_and_gives_or_does_neither() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 100,
            stash: vec![stack("cloth", 9)],
            ..Profile::default()
        };
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::Missing));
        assert_eq!((p.gold, p.stashed("cloth"), p.inn.stash_bonus), (100, 9, 0));
        p.stash = vec![stack("cloth", 12)];
        p.gold = 49;
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::NoGold));
        assert_eq!((p.gold, p.stashed("cloth")), (49, 12));
        p.gold = 50;
        assert_eq!(p.upgrade_station(0, &inn, &c), Ok(1));
        assert_eq!((p.gold, p.stashed("cloth"), p.inn.stash_bonus), (0, 2, 20));
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::MaxLevel));
        let mut q = Profile {
            gold: 90,
            stash: vec![stack("cloth", 4)],
            inn: InnProgress {
                stations: vec![("bench".into(), 1)],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        assert_eq!(q.upgrade_station(1, &inn, &c), Ok(2));
        assert!(q.inn.intel);
        assert_eq!(q.inn.companion.as_deref(), Some("hound"));
    }

    #[test]
    fn an_upgrade_takes_its_materials_from_the_stash_only() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 100,
            loadout: SavedInventory {
                pack: vec![stack("cloth", 20)],
                ..SavedInventory::default()
            },
            ..Profile::default()
        };
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::Missing));
        assert_eq!((p.gold, p.loadout.pack[0].count), (100, 20));
    }

    #[test]
    fn gifts_are_given_once_and_stack_up_without_overflow() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 1000,
            stash: vec![stack("cloth", 20)],
            inn: InnProgress {
                stations: vec![("bench".into(), 0)],
                recipes: vec!["pouch".into()],
                stash_bonus: u16::MAX - 5,
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        // The recipe the bench teaches is known already: it is not filed twice.
        assert_eq!(p.upgrade_station(1, &inn, &c), Ok(1));
        assert_eq!(p.inn.recipes, ["pouch"]);
        // The stash's 20 slots saturate rather than wrap.
        assert_eq!(p.upgrade_station(0, &inn, &c), Ok(1));
        assert_eq!(p.inn.stash_bonus, u16::MAX);
        // A second go at the same level is not offered again: it is built.
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::MaxLevel));
        assert_eq!(p.inn.stash_bonus, u16::MAX);
    }

    #[test]
    fn a_craft_pays_and_makes_or_does_neither() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 10,
            stash: vec![stack("cloth", 6)],
            ..Profile::default()
        };
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::NoRecipe));
        p.inn.recipes.push("pouch".into());
        p.stash = vec![stack("cloth", 5)];
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::Missing));
        p.stash = vec![stack("cloth", 6)];
        p.gold = 4;
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::NoGold));
        assert_eq!(p.stashed("cloth"), 6);
        p.gold = 5;
        assert_eq!(p.craft(0, &inn, &c), Ok(()));
        assert_eq!((p.gold, p.stashed("cloth"), p.stashed("pouch")), (0, 0, 1));
        assert_eq!(p.craft(9, &inn, &c), Err(StationError::Unknown));
    }

    #[test]
    fn the_trader_sells_and_buys_at_their_share() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 3,
            ..Profile::default()
        };
        assert_eq!(p.buy(1, &inn, &c), Err(StationError::NoGold));
        assert_eq!(p.buy(0, &inn, &c), Ok(()));
        assert_eq!((p.gold, p.stashed("bread")), (1, 1));
        // Bread is worth nothing in gold.
        assert_eq!(p.sell_to_trader(0, &inn, &c), Err(StationError::Worthless));
        assert_eq!(p.stashed("bread"), 1, "a refused sale keeps the bread");
        p.stash.push(stack("cloth", 4));
        // Four cloth worth 10 each; this fixture's trader pays 0.5 of value (the shipped one 1.0).
        assert_eq!(p.sell_to_trader(1, &inn, &c), Ok(20));
        assert_eq!(p.gold, 21);
        assert_eq!(p.buy(9, &inn, &c), Err(StationError::Unknown));
        assert_eq!(
            p.sell_to_trader(7, &inn, &c),
            Err(StationError::Unknown),
            "no such stack"
        );
    }

    fn in_a_raid() -> SavedRaid {
        let mut p = Profile::default();
        p.begin_raid(0, &raid());
        p.in_raid.expect("a raid begun")
    }

    #[test]
    fn nothing_is_done_while_a_raid_is_under_way() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 500,
            stash: vec![stack("cloth", 20)],
            in_raid: Some(in_a_raid()),
            inn: InnProgress {
                stations: vec![("bench".into(), 0)],
                recipes: vec!["pouch".into()],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        let before = p.clone();
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::InRaid));
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::InRaid));
        assert_eq!(p.buy(0, &inn, &c), Err(StationError::InRaid));
        assert_eq!(p.sell_to_trader(0, &inn, &c), Err(StationError::InRaid));
        assert_eq!(p, before);
    }

    /// A stash with every slot taken, `cloth` of cloth in the first.
    fn full_stash(c: &Content, cloth: u16) -> Vec<SavedStack> {
        let mut stash = vec![stack("cloth", cloth)];
        while stash.len() < Profile::default().stash_capacity(c) {
            stash.push(stack("tome", 1));
        }
        stash
    }

    #[test]
    fn a_full_stash_refuses_a_purchase_and_a_craft_and_nothing_is_spent() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 100,
            stash: full_stash(&c, 20),
            inn: InnProgress {
                recipes: vec!["pouch".into()],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        let before = p.clone();
        // The pouch (30 gold) has no free slot; nor has a crafted one, with 14 cloth left over
        // keeping its slot.
        assert_eq!(p.buy(1, &inn, &c), Err(StationError::StashFull));
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::StashFull));
        assert_eq!(p, before);
        // The slots the stash station gave count.
        p.inn.stash_bonus = 1;
        assert_eq!(p.buy(1, &inn, &c), Ok(()));
        assert_eq!(p.gold, 70);
        assert_eq!(p.stashed("pouch"), 1);
    }

    #[test]
    fn a_craft_may_use_the_room_its_own_materials_free() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 5,
            stash: full_stash(&c, 6),
            inn: InnProgress {
                recipes: vec!["pouch".into()],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        assert_eq!(p.craft(0, &inn, &c), Ok(()));
        assert_eq!((p.stashed("cloth"), p.stashed("pouch")), (0, 1));
        assert_eq!(p.stash.len(), p.stash_capacity(&c));
    }

    #[test]
    fn bread_bought_goes_onto_a_stack_with_room_in_a_full_stash() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 2,
            stash: full_stash(&c, 1),
            ..Profile::default()
        };
        p.stash[1] = stack("bread", 3);
        assert_eq!(p.buy(0, &inn, &c), Ok(()));
        assert_eq!(p.stashed("bread"), 4);
    }

    #[test]
    fn a_profile_with_keys_the_content_dropped_does_not_panic() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 1000,
            stash: vec![stack("cloth", 20), stack("gone_item", 3)],
            inn: InnProgress {
                stations: vec![("gone_station".into(), 3), ("bench".into(), 9)],
                recipes: vec!["gone_recipe".into()],
                companion: Some("gone_pet".into()),
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        assert_eq!(p.station_level(0, &inn), Some(0), "the stash is still open");
        assert_eq!(p.station_level(1, &inn), Some(9));
        assert!(!p.knows(0, &inn), "the pouch was never learned");
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::NoRecipe));
        // A bench level the content does not have is a bench built as far as it goes.
        assert_eq!(p.upgrade_station(1, &inn, &c), Err(StationError::MaxLevel));
        assert_eq!(p.upgrade_station(0, &inn, &c), Ok(1));
        // A stack of an item the content lacks cannot be sold, and stays.
        let gone = p.stash.iter().position(|s| s.item == "gone_item").unwrap();
        assert_eq!(p.sell_to_trader(gone, &inn, &c), Err(StationError::Unknown));
        assert_eq!(p.stashed("gone_item"), 3);
    }

    #[test]
    fn a_line_that_names_an_item_twice_needs_both_counts() {
        let c = raid();
        let p = Profile {
            stash: vec![stack("cloth", 8)],
            ..Profile::default()
        };
        let cloth = c.item("cloth").unwrap();
        assert_eq!(
            p.can_pay(0, &[(cloth, 5), (cloth, 5)], &c),
            Err(StationError::Missing)
        );
        assert_eq!(p.can_pay(0, &[(cloth, 4), (cloth, 4)], &c), Ok(()));
    }

    /// A stash a raid's return grew one slot past its capacity.
    fn over_full_stash(c: &Content, cloth: u16) -> Vec<SavedStack> {
        let mut stash = full_stash(c, cloth);
        stash[1] = stack("bread", 3);
        stash.push(stack("tome", 1));
        stash
    }

    #[test]
    fn an_over_full_stash_still_takes_what_only_merges_into_a_stack() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: 100,
            stash: over_full_stash(&c, 20),
            inn: InnProgress {
                recipes: vec!["pouch".into()],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        assert!(p.stash.len() > p.stash_capacity(&c));
        // Bread merges into the stack of 3; a pouch needs a new slot.
        assert_eq!(p.buy(0, &inn, &c), Ok(()));
        assert_eq!(p.stashed("bread"), 4);
        let before = p.clone();
        assert_eq!(p.buy(1, &inn, &c), Err(StationError::StashFull));
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::StashFull));
        assert_eq!(p, before);
        // A recipe that makes bread merges too, and pays.
        let bread = c.item("bread").unwrap();
        let mut baker = inn.clone();
        baker.recipes[0].makes = (bread, 2);
        assert_eq!(p.craft(0, &baker, &c), Ok(()));
        assert_eq!((p.stashed("bread"), p.stashed("cloth")), (6, 14));
        assert_eq!(p.gold, 100 - 2 - 5);
    }

    #[test]
    fn a_companion_gift_replaces_the_companion_already_kept() {
        let c = raid();
        let mut inn = inn(&c);
        inn.companions.push(crate::CompanionDef {
            key: "cat".into(),
            name: "Cat".into(),
            display: 1,
            scale: 1.0,
        });
        inn.stations[1].levels[1].gives = vec![Gift::Companion(1)];
        let mut p = Profile {
            gold: 90,
            stash: vec![stack("cloth", 4)],
            inn: InnProgress {
                stations: vec![("bench".into(), 1)],
                companion: Some("hound".into()),
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        assert_eq!(p.upgrade_station(1, &inn, &c), Ok(2));
        assert_eq!(p.inn.companion.as_deref(), Some("cat"));
    }

    #[test]
    fn a_recipe_naming_an_item_twice_takes_both_counts_when_made() {
        let c = raid();
        let mut inn = inn(&c);
        let cloth = c.item("cloth").unwrap();
        inn.recipes[0].needs = vec![(cloth, 3), (cloth, 3)];
        let mut p = Profile {
            gold: 5,
            stash: vec![stack("cloth", 5)],
            inn: InnProgress {
                recipes: vec!["pouch".into()],
                ..InnProgress::default()
            },
            ..Profile::default()
        };
        // Five cloth pay either line alone but not both: nothing is taken.
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::Missing));
        assert_eq!((p.gold, p.stashed("cloth")), (5, 5));
        p.stash = vec![stack("cloth", 8)];
        assert_eq!(p.craft(0, &inn, &c), Ok(()));
        assert_eq!((p.gold, p.stashed("cloth"), p.stashed("pouch")), (0, 2, 1));
    }
}
