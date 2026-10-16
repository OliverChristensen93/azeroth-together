//! The stash between runs: every piece of equipment a player has found, and what each hero wears
//! into the next run.
//!
//! An [`Armory`] holds [`Locker`]s: by default one per class (a mage's finds are the mage's, and
//! each class wears its own set), or, with [`Armory::shared`] switched on, one locker that every
//! class draws from (decision 9h: both are built, to play both and keep the better). Switching
//! the setting changes which lockers are used, never what is in them, so nothing is lost either
//! way.
//!
//! The client saves the armory as RON ([`Armory::to_ron`]); everything here is plain data, so it
//! is tested without a screen.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::gear::{loadout, GearPiece, Slot};

/// The most pieces a stash keeps: past it, the weakest unworn ones go to make room.
pub const STASH_MAX: usize = 120;

/// One set of equipment: what is worn, and the stash it is chosen from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locker {
    /// What is worn into the next run, one a slot ([`loadout`]).
    #[serde(default)]
    pub equipped: Vec<GearPiece>,
    /// Everything else found, newest last.
    #[serde(default)]
    pub stash: Vec<GearPiece>,
    /// How many of the newest stash pieces have not been seen at the menu yet.
    #[serde(default)]
    pub unseen: u32,
}

impl Locker {
    /// The piece worn in `slot`; for the off hand, nothing while a two-handed weapon is worn.
    pub fn in_slot(&self, slot: Slot) -> Option<&GearPiece> {
        self.equipped.iter().find(|p| p.slot == slot)
    }

    /// Wear stash piece `index`: whatever it pushes out (the piece in its slot; for a two-handed
    /// weapon also the off hand; for an off hand a two-handed weapon) goes back into the stash.
    /// False if there is no such piece, or it cannot be worn.
    pub fn equip(&mut self, index: usize) -> bool {
        let Some(&piece) = self.stash.get(index) else {
            return false;
        };
        if !piece.valid() {
            return false;
        }
        self.stash.remove(index);
        let pushed_out = |p: &GearPiece| {
            p.slot == piece.slot
                || (piece.two_hand() && p.slot == Slot::OffHand)
                || (piece.slot == Slot::OffHand && p.two_hand())
        };
        let (out, keep): (Vec<GearPiece>, Vec<GearPiece>) =
            self.equipped.iter().partition(|p| pushed_out(p));
        self.stash.extend(out);
        self.equipped = keep;
        self.equipped.push(piece);
        self.equipped = loadout(&self.equipped);
        true
    }

    /// Take off the piece in `slot`, back into the stash. False if the slot is empty.
    pub fn unequip(&mut self, slot: Slot) -> bool {
        let Some(i) = self.equipped.iter().position(|p| p.slot == slot) else {
            return false;
        };
        let p = self.equipped.remove(i);
        self.stash.push(p);
        true
    }

    /// Put newly found pieces in the stash, unseen, and keep it within [`STASH_MAX`].
    pub fn add(&mut self, found: &[GearPiece]) {
        self.stash.extend(found.iter().copied().filter(GearPiece::valid));
        self.unseen = self.unseen.saturating_add(found.len() as u32);
        while self.stash.len() > STASH_MAX {
            // The weakest, oldest first: a unique is never the one to go while anything else
            // could.
            let weakest = self
                .stash
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (a.unique.is_some(), a.power())
                        .partial_cmp(&(b.unique.is_some(), b.power()))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.stash.remove(weakest);
        }
        self.unseen = self.unseen.min(self.stash.len() as u32);
    }

    /// Whether stash piece `index` is among the unseen newest.
    pub fn is_new(&self, index: usize) -> bool {
        index + self.unseen as usize >= self.stash.len()
    }
}

/// Every locker, and the setting that picks between them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Armory {
    /// One locker for every class, instead of one per class (9h). Per class by default.
    #[serde(default)]
    pub shared: bool,
    /// Each class's own locker, by class key.
    #[serde(default)]
    pub classes: BTreeMap<String, Locker>,
    /// The locker every class uses while [`Armory::shared`] is on.
    #[serde(default)]
    pub everyone: Locker,
}

impl Armory {
    /// The locker class `class` uses now.
    pub fn locker(&self, class: &str) -> Locker {
        if self.shared {
            self.everyone.clone()
        } else {
            self.classes.get(class).cloned().unwrap_or_default()
        }
    }

    pub fn locker_mut(&mut self, class: &str) -> &mut Locker {
        if self.shared {
            &mut self.everyone
        } else {
            self.classes.entry(class.to_string()).or_default()
        }
    }

    /// What class `class` wears into a run.
    pub fn equipped(&self, class: &str) -> Vec<GearPiece> {
        loadout(&self.locker(class).equipped)
    }

    /// Pieces a hero of class `class` found in a run, into its locker.
    pub fn add_found(&mut self, class: &str, found: &[GearPiece]) {
        if !found.is_empty() {
            self.locker_mut(class).add(found);
        }
    }

    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    /// The armory a save file holds; `None` if it does not read.
    pub fn from_ron(text: &str) -> Option<Armory> {
        ron::from_str(text).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::Rarity;
    use crate::gear::{Armor, Unique};

    fn piece(slot: Slot, rarity: Rarity) -> GearPiece {
        GearPiece::new(slot, rarity, 20, Armor::Cloth, 77)
    }

    #[test]
    fn equipping_swaps_with_what_was_worn() {
        let mut l = Locker::default();
        l.add(&[piece(Slot::Head, Rarity::Common), piece(Slot::Head, Rarity::Rare)]);
        assert!(l.equip(0));
        assert_eq!(l.in_slot(Slot::Head).unwrap().rarity, Rarity::Common);
        assert_eq!(l.stash.len(), 1);
        assert!(l.equip(0));
        assert_eq!(l.in_slot(Slot::Head).unwrap().rarity, Rarity::Rare);
        assert_eq!(l.stash[0].rarity, Rarity::Common);
        assert!(l.unequip(Slot::Head));
        assert!(l.equipped.is_empty() && l.stash.len() == 2);
        assert!(!l.unequip(Slot::Head));
        assert!(!l.equip(9));
    }

    #[test]
    fn a_two_hander_and_an_off_hand_push_each_other_out() {
        let mut l = Locker::default();
        let staff = GearPiece::unique(Unique::Atiesh, 30, 1);
        let orb = piece(Slot::OffHand, Rarity::Rare);
        l.add(&[orb, staff]);
        assert!(l.equip(0)); // the orb
        assert!(l.equip(0)); // the staff: the orb goes back
        assert_eq!(l.equipped, vec![staff]);
        assert_eq!(l.stash, vec![orb]);
        assert!(l.equip(0)); // the orb again: the staff goes back
        assert_eq!(l.equipped, vec![orb]);
        assert_eq!(l.stash, vec![staff]);
    }

    #[test]
    fn per_class_or_shared_lockers_keep_their_own_pieces() {
        let mut a = Armory::default();
        a.add_found("mage", &[piece(Slot::Head, Rarity::Rare)]);
        assert_eq!(a.locker("mage").stash.len(), 1);
        assert!(a.locker("warrior").stash.is_empty());
        a.shared = true;
        assert!(a.locker("mage").stash.is_empty());
        a.add_found("warrior", &[piece(Slot::Feet, Rarity::Common)]);
        assert_eq!(a.locker("mage").stash.len(), 1, "shared: every class sees it");
        a.shared = false;
        assert_eq!(a.locker("mage").stash[0].slot, Slot::Head, "nothing was lost");
    }

    #[test]
    fn the_armory_survives_a_save_and_load() {
        let mut a = Armory::default();
        a.add_found("mage", &[piece(Slot::Head, Rarity::Rare)]);
        a.add_found("mage", &[GearPiece::unique(Unique::GlacialCloak, 40, 3)]);
        a.locker_mut("mage").equip(1);
        let back = Armory::from_ron(&a.to_ron()).expect("reads back");
        assert_eq!(back, a);
        assert_eq!(back.equipped("mage").len(), 1);
        assert!(Armory::from_ron("not ron at all").is_none());
    }

    #[test]
    fn a_full_stash_lets_the_weakest_go_and_marks_the_newest() {
        let mut l = Locker::default();
        let mut found = vec![GearPiece::unique(Unique::Thunderfury, 5, 1)];
        found.extend((0..STASH_MAX).map(|_| piece(Slot::Wrists, Rarity::Rare)));
        l.add(&found);
        l.unseen = 0;
        l.add(&[piece(Slot::Chest, Rarity::Common)]);
        assert_eq!(l.stash.len(), STASH_MAX);
        assert!(l.stash.iter().any(|p| p.unique.is_some()), "the unique stays");
        assert!(l.is_new(l.stash.len() - 1) && !l.is_new(0));
    }
}
