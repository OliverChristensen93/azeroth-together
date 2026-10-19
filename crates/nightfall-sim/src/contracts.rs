//! Contracts (spec §8.5): Duskwood's characters give them in the inn, one or two taken at a time;
//! a raid's outcome moves them on (kills where they were asked for, an extract by the exit named);
//! things to bring are taken as the contract is turned in, for gold and stations unlocked. Pure,
//! on the player's [`Profile`].
//!
//! A profile may name a contract the content no longer has (a renamed or dropped key): its state
//! is neither read nor touched here, so it waits in the file in case the key comes back.

use crate::{
    Content, ContractDef, ContractState, Fate, HeroOutcome, InnContent, Objective, Profile,
    SavedStack,
};

/// Why a contract could not be taken, dropped or turned in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractError {
    InRaid,
    Unknown,
    Taken,
    TooMany,
    /// A contract it needs is not finished yet.
    Locked,
    /// Finished, and not to be done again.
    Finished,
    NotTaken,
    NotReady,
}

impl ContractError {
    /// The line the panel shows.
    pub fn message(self) -> &'static str {
        match self {
            ContractError::InRaid => "Not while a raid is under way.",
            ContractError::Unknown => "No such contract.",
            ContractError::Taken => "You have taken it already.",
            ContractError::TooMany => "You have taken as many as you can.",
            ContractError::Locked => "Not yet: there is something to do first.",
            ContractError::Finished => "Done, and done for good.",
            ContractError::NotTaken => "You have not taken it.",
            ContractError::NotReady => "There is more to do.",
        }
    }
}

/// Where a stack a contract may ask for lies. The companion's slot, worn gear and bags are not
/// here: the companion counts for nothing in a raid, and what is worn or in a bag is not
/// something to hand over.
#[derive(Clone, Copy)]
enum Bringable {
    Stash(usize),
    Pack(usize),
    Safe(usize),
}

impl Profile {
    /// Contract `c`'s state, if the player ever took it.
    pub fn contract(&self, c: usize, inn: &InnContent) -> Option<&ContractState> {
        let key = &inn.contracts.get(c)?.key;
        self.inn.contracts.iter().find(|s| s.key == *key)
    }

    /// Contract `c`'s state, made when there is none. `c` must be a contract of `inn`.
    fn contract_mut(&mut self, c: usize, inn: &InnContent) -> &mut ContractState {
        let key = inn.contracts[c].key.clone();
        let i = match self.inn.contracts.iter().position(|s| s.key == key) {
            Some(i) => i,
            None => {
                self.inn.contracts.push(ContractState {
                    key,
                    taken: false,
                    progress: Vec::new(),
                    finished: 0,
                });
                self.inn.contracts.len() - 1
            }
        };
        &mut self.inn.contracts[i]
    }

    /// The contracts taken, by index.
    pub fn taken(&self, inn: &InnContent) -> Vec<usize> {
        (0..inn.contracts.len())
            .filter(|&c| self.contract(c, inn).is_some_and(|s| s.taken))
            .collect()
    }

    /// Whether contract `c` may be taken now. It does not check for a raid under way (only
    /// [`Profile::take_contract`] does), so a panel must gate its Take button on
    /// `take_contract`'s result, never on this alone.
    pub fn offer(&self, c: usize, inn: &InnContent) -> Result<(), ContractError> {
        let def = inn.contracts.get(c).ok_or(ContractError::Unknown)?;
        let state = self.contract(c, inn);
        if state.is_some_and(|s| s.taken) {
            return Err(ContractError::Taken);
        }
        if state.is_some_and(|s| s.finished > 0) && !def.repeatable {
            return Err(ContractError::Finished);
        }
        let done = |n: usize| self.contract(n, inn).is_some_and(|s| s.finished > 0);
        if !def.needs.iter().all(|&n| done(n)) {
            return Err(ContractError::Locked);
        }
        if self.taken(inn).len() >= usize::from(inn.contracts_at_once) {
            return Err(ContractError::TooMany);
        }
        Ok(())
    }

    pub fn take_contract(&mut self, c: usize, inn: &InnContent) -> Result<(), ContractError> {
        if self.in_raid.is_some() {
            return Err(ContractError::InRaid);
        }
        self.offer(c, inn)?;
        let n = inn.contracts[c].objectives.len();
        let s = self.contract_mut(c, inn);
        s.taken = true;
        s.progress = vec![0; n];
        Ok(())
    }

    /// Contract `c` dropped, its progress with it.
    pub fn drop_contract(&mut self, c: usize, inn: &InnContent) {
        if self.in_raid.is_some() || self.contract(c, inn).is_none_or(|s| !s.taken) {
            return;
        }
        let s = self.contract_mut(c, inn);
        s.taken = false;
        s.progress.clear();
    }

    /// Every stack a contract may ask for, in the order they are taken: the stash, then the
    /// loadout's backpack, then its safe slots. [`Profile::held`] counts this list and
    /// [`Profile::take_items`] takes from it, so what is counted is exactly what is taken.
    fn bringable(&self) -> impl Iterator<Item = (Bringable, &SavedStack)> {
        let stash = self
            .stash
            .iter()
            .enumerate()
            .map(|(i, s)| (Bringable::Stash(i), s));
        let pack = self
            .loadout
            .pack
            .iter()
            .enumerate()
            .map(|(i, s)| (Bringable::Pack(i), s));
        let safe = self
            .loadout
            .safe
            .iter()
            .enumerate()
            .map(|(i, (_, s))| (Bringable::Safe(i), s));
        stash.chain(pack).chain(safe)
    }

    /// How many of `item` the player can hand over to a contract: the stash, the loadout's
    /// backpack and its safe slots. Not the companion's slot, nor worn gear or bags.
    pub fn held(&self, item: &str) -> u32 {
        self.bringable()
            .filter(|(_, s)| s.item == item)
            .fold(0u32, |n, (_, s)| n.saturating_add(u32::from(s.count)))
    }

    /// `count` of `item` taken from [`Profile::bringable`], in its order; nothing at all when
    /// there are not as many there.
    fn take_items(&mut self, item: &str, count: u32) -> bool {
        let places: Vec<(Bringable, u16)> = self
            .bringable()
            .filter(|(_, s)| s.item == item)
            .map(|(at, s)| (at, s.count))
            .collect();
        let total = places
            .iter()
            .fold(0u32, |n, (_, c)| n.saturating_add(u32::from(*c)));
        if total < count {
            return false;
        }
        let mut left = count;
        for (at, have) in places {
            let n = left.min(u32::from(have));
            let stack = match at {
                Bringable::Stash(i) => &mut self.stash[i],
                Bringable::Pack(i) => &mut self.loadout.pack[i],
                Bringable::Safe(i) => &mut self.loadout.safe[i].1,
            };
            // `n` is at most the stack's own count, so it fits.
            stack.count -= n as u16;
            left -= n;
        }
        self.stash.retain(|s| s.count > 0);
        self.loadout.pack.retain(|s| s.count > 0);
        self.loadout.safe.retain(|(_, s)| s.count > 0);
        true
    }

    /// What contract `def` asks to be brought, as (item key, count) with one entry for each
    /// item: two objectives asking for the same item add up, since one stack cannot pay for both
    /// twice. An item the content lacks is left out here (and its objective is never met).
    fn bring_needs(def: &ContractDef, content: &Content) -> Vec<(String, u32)> {
        let mut needs: Vec<(String, u32)> = Vec::new();
        for o in &def.objectives {
            let Objective::Bring { item, count } = o else {
                continue;
            };
            let Some(key) = content.items.get(*item).map(|i| i.key.as_str()) else {
                continue;
            };
            match needs.iter_mut().find(|(k, _)| k == key) {
                Some((_, n)) => *n = n.saturating_add(*count),
                None => needs.push((key.to_string(), *count)),
            }
        }
        needs
    }

    /// What a Bring objective of `def` for item index `item` needs held: the total over all of
    /// `def`'s Bring objectives for that item (two for one item need both amounts at once), with
    /// the item's name and key. `None` for an item the content lacks.
    fn bring_total<'a>(
        def: &ContractDef,
        item: usize,
        content: &'a Content,
    ) -> Option<(&'a str, &'a str, u32)> {
        let d = content.items.get(item)?;
        let total = Self::bring_needs(def, content)
            .into_iter()
            .find(|(k, _)| *k == d.key)
            .map_or(0, |(_, n)| n);
        Some((d.name.as_str(), d.key.as_str(), total))
    }

    /// Whether objective `o` of taken contract `def`, with `progress`, is met. A Bring is met by
    /// the contract's whole need of that item, exactly what [`Profile::turn_in`] takes.
    fn met(&self, def: &ContractDef, o: &Objective, progress: u32, content: &Content) -> bool {
        match o {
            Objective::Kill { count, .. } => progress >= *count,
            Objective::Extract { .. } => progress >= 1,
            Objective::Bring { item, .. } => Self::bring_total(def, *item, content)
                .is_some_and(|(_, key, total)| self.held(key) >= total),
        }
    }

    /// Each objective of contract `c` as a line, and whether it is met.
    pub fn objective_lines(
        &self,
        c: usize,
        inn: &InnContent,
        content: &Content,
    ) -> Vec<(String, bool)> {
        let Some(def) = inn.contracts.get(c) else {
            return Vec::new();
        };
        let state = self.contract(c, inn).filter(|s| s.taken);
        def.objectives
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let p = state.and_then(|s| s.progress.get(i).copied()).unwrap_or(0);
                let line = match o {
                    Objective::Kill { enemy, count, near } => format!(
                        "Kill {}{}: {} / {count}",
                        content.enemies.get(*enemy).map_or("?", |e| e.name.as_str()),
                        if near.is_some() {
                            " near the place"
                        } else {
                            ""
                        },
                        p.min(*count)
                    ),
                    Objective::Bring { item, .. } => {
                        // The whole need of that item, so the line says what the turn-in takes.
                        let (name, held, total) = Self::bring_total(def, *item, content)
                            .map_or(("?", 0, 0), |(name, key, total)| {
                                (name, self.held(key), total)
                            });
                        format!("Bring {name}: {} / {total}", held.min(total))
                    }
                    Objective::Extract { exit } => match exit {
                        Some(e) => format!(
                            "Get out by {}",
                            content.map.exits.get(*e).map_or("?", |x| x.name.as_str())
                        ),
                        None => "Get out alive".to_string(),
                    },
                };
                (line, state.is_some() && self.met(def, o, p, content))
            })
            .collect()
    }

    /// Whether taken contract `c` can be turned in: every objective met, and the things to
    /// bring held all together (two objectives for one item need both amounts at once).
    pub fn contract_ready(&self, c: usize, inn: &InnContent, content: &Content) -> bool {
        let (Some(def), Some(s)) = (inn.contracts.get(c), self.contract(c, inn)) else {
            return false;
        };
        s.taken
            && def
                .objectives
                .iter()
                .enumerate()
                .all(|(i, o)| self.met(def, o, s.progress.get(i).copied().unwrap_or(0), content))
    }

    /// Contract `c` turned in: what it asks to be brought taken, its gold paid (saturating at
    /// the purse's top), its stations unlocked (once, however often it is done); it is finished
    /// once more (and may be taken again when repeatable). The gold. It pays only if everything
    /// it asked for was taken.
    pub fn turn_in(
        &mut self,
        c: usize,
        inn: &InnContent,
        content: &Content,
    ) -> Result<u32, ContractError> {
        if self.in_raid.is_some() {
            return Err(ContractError::InRaid);
        }
        let def = inn.contracts.get(c).ok_or(ContractError::Unknown)?;
        if self.contract(c, inn).is_none_or(|s| !s.taken) {
            return Err(ContractError::NotTaken);
        }
        if !self.contract_ready(c, inn, content) {
            return Err(ContractError::NotReady);
        }
        // The snapshot guards only a mismatch between the content and its indices (an item the
        // content lacks cannot be ready, so a take failing here would be such a mismatch).
        let before = (self.stash.clone(), self.loadout.clone());
        for (key, n) in Self::bring_needs(def, content) {
            if !self.take_items(&key, n) {
                // Ready means held, so this is not reached; if it ever is, nothing is paid for.
                (self.stash, self.loadout) = before;
                return Err(ContractError::NotReady);
            }
        }
        self.gold = self.gold.saturating_add(def.reward.gold);
        for &s in &def.reward.unlocks {
            let Some(station) = inn.stations.get(s) else {
                continue;
            };
            if !self.inn.stations.iter().any(|(k, _)| *k == station.key) {
                self.inn.stations.push((station.key.clone(), 0));
            }
        }
        let state = self.contract_mut(c, inn);
        state.taken = false;
        state.progress.clear();
        state.finished = state.finished.saturating_add(1);
        Ok(def.reward.gold)
    }

    /// A raid's outcome moves the taken contracts on, once per raid: kills of the kind asked for
    /// (inside the circle, when there is one), an extract by the exit named. A raid that went
    /// missing has no outcome, and moves nothing.
    ///
    /// Kills count by the enemy's kind (the index `slain` holds) and by position on the ground
    /// plane; a kill whose position is not a number never counts. An exit is matched by its
    /// display name, as [`Fate::Extracted`] holds it, so two exits sharing a name are one to a
    /// contract (accepted: the content gives its exits distinct names).
    ///
    /// The caller's contract: call it only for a raid still under way, that is, before
    /// [`Profile::settle`] and only while `in_raid` is set. An outcome that arrives after the raid
    /// was settled as missing must not be applied, or it moves the contracts for a raid that
    /// already ended without it. Applying the same raid twice is harmless: `applied` remembers
    /// the raid (by `raids`) and the second call does nothing.
    pub fn apply_contracts(&mut self, o: &HeroOutcome, inn: &InnContent, content: &Content) {
        if self.inn.applied >= self.raids {
            return;
        }
        self.inn.applied = self.raids;
        for c in self.taken(inn) {
            let def = &inn.contracts[c];
            let adds: Vec<u32> = def
                .objectives
                .iter()
                .map(|ob| match ob {
                    Objective::Kill { enemy, near, .. } => {
                        let n = o
                            .slain
                            .iter()
                            .filter(|(kind, at)| {
                                usize::from(*kind) == *enemy
                                    && at.is_finite()
                                    && near.is_none_or(|(centre, r)| {
                                        centre.is_finite() && centre.distance(*at) <= r
                                    })
                            })
                            .count();
                        u32::try_from(n).unwrap_or(u32::MAX)
                    }
                    Objective::Extract { exit } => match &o.fate {
                        Fate::Extracted { exit: by } => u32::from(exit.is_none_or(|e| {
                            content.map.exits.get(e).is_some_and(|x| x.name == *by)
                        })),
                        _ => 0,
                    },
                    Objective::Bring { .. } => 0,
                })
                .collect();
            let state = self.contract_mut(c, inn);
            state.progress.resize(def.objectives.len(), 0);
            for (i, ob) in def.objectives.iter().enumerate() {
                let cap = match ob {
                    Objective::Kill { count, .. } => *count,
                    _ => 1,
                };
                state.progress[i] = state.progress[i].saturating_add(adds[i]).min(cap);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{ContentSources, EquipSlot, Fate, InnSources, Inventory, SavedStack, V2};

    pub(crate) fn raid() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                entries: [(50.0, 50.0, 0.0)],
                exits: [(key: "road", name: "The Road", at: (90.0, 90.0)),
                        (key: "lane", name: "The Lane", at: (10.0, 90.0))])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: r#"[
                (key: "worgen", name: "Worgen", display: 657, level: 27, hp: 900.0,
                 walk_speed: 2.5, run_speed: 8.0),
                (key: "spider", name: "Spider", display: 955, level: 26, hp: 800.0,
                 walk_speed: 2.2, run_speed: 7.0),
            ]"#,
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "tome", name: "Tome", kind: Valuable, icon: "a", weight: 1.0),
                (key: "cloth", name: "Cloth", kind: Material, icon: "a", weight: 0.1, stack: 20,
                 value: 10),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 4),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.2, stack: 20),
                (key: "conjured_water", name: "Water", kind: Drink, icon: "a", weight: 0.1,
                 stack: 20),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    pub(crate) fn inn(raid: &Content) -> InnContent {
        InnContent::from_ron(
            InnSources {
                hub: r#"(
                    place: (name: "Inn", map: 0, spawn: (10.0, 10.0, 0.0),
                            bounds: [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
                            door: (at: (19.0, 10.0), radius: 2.0), minute: 1170,
                            view_distance: 200.0),
                    npcs: [
                        (key: "banker", name: "Farley", role: Banker,
                         body: (race: 1, sex: 0, class_id: 1), at: (5.0, 5.0, 0.0)),
                        (key: "trader", name: "Brog", role: Trader,
                         body: (race: 1, sex: 0, class_id: 1), at: (5.0, 15.0, 0.0)),
                        (key: "watch", name: "Ebonlock", role: Contracts,
                         body: (race: 1, sex: 1, class_id: 2), at: (15.0, 5.0, 0.0)),
                        (key: "smith", name: "Smith", role: Station("bench"),
                         body: (race: 1, sex: 0, class_id: 1), at: (15.0, 15.0, 0.0)),
                    ],
                    contracts_at_once: 2,
                    trader: (stock: [(item: "bread", price: 2), (item: "pouch", price: 30)],
                             sells_for: 0.5),
                    companions: [(key: "hound", name: "Hound", display: 1052)],
                )"#,
                contracts: r#"[
                    (key: "pack", giver: "watch", name: "Thin the pack",
                     objectives: [Kill(enemy: "worgen", count: 3, near: Some((50.0, 50.0, 20.0)))],
                     reward: (gold: 60, unlocks: ["bench"]), repeatable: true),
                    (key: "tome", giver: "watch", name: "The tome", needs: ["pack"],
                     objectives: [Bring(item: "tome", count: 1), Extract(exit: Some("road"))],
                     reward: (gold: 100)),
                    (key: "out", giver: "watch", name: "Out alive",
                     objectives: [Extract(exit: None)], reward: (gold: 10)),
                    (key: "cloth", giver: "watch", name: "Cloth for the watch",
                     objectives: [Bring(item: "cloth", count: 5)], reward: (gold: 7),
                     repeatable: true),
                    (key: "twice", giver: "watch", name: "Cloth, twice over",
                     objectives: [Bring(item: "cloth", count: 4), Bring(item: "cloth", count: 4)],
                     reward: (gold: 3)),
                    (key: "strays", giver: "watch", name: "Strays anywhere",
                     objectives: [Kill(enemy: "spider", count: 2, near: None)],
                     reward: (gold: 1)),
                ]"#,
                stations: r#"(
                    stations: [
                        (key: "stash", name: "The Stash", keeper: "banker", open: true,
                         levels: [(gold: 50, items: [("cloth", 10)], gives: [StashSlots(20)])]),
                        (key: "bench", name: "Workbench", keeper: "smith",
                         levels: [(gold: 40, gives: [Recipe("pouch")]),
                                  (gold: 90, items: [("cloth", 4)], gives: [Intel, Companion("hound")])]),
                    ],
                    recipes: [(key: "pouch", station: "bench", makes: ("pouch", 1),
                               needs: [("cloth", 6)], gold: 5)],
                )"#,
            },
            raid,
        )
        .unwrap()
    }

    const PACK: usize = 0;
    const OUT: usize = 2;
    const CLOTH: usize = 3;
    const TWICE: usize = 4;
    const STRAYS: usize = 5;

    fn outcome(fate: Fate, slain: Vec<(u16, V2)>) -> HeroOutcome {
        HeroOutcome {
            player: 0,
            name: "A".into(),
            fate,
            time: 600.0,
            kills: slain.len() as u32,
            carried: Inventory::default(),
            overflow: Vec::new(),
            recovered: Vec::new(),
            recovery: None,
            slain,
        }
    }

    fn out_by(exit: &str) -> HeroOutcome {
        outcome(Fate::Extracted { exit: exit.into() }, vec![])
    }

    fn cloth(count: u16) -> SavedStack {
        SavedStack {
            item: "cloth".into(),
            count,
        }
    }

    /// The profile as it stands after a raid: begun, then this outcome applied.
    fn after_raid(p: &mut Profile, o: &HeroOutcome, inn: &InnContent, c: &Content) {
        p.raids += 1;
        p.apply_contracts(o, inn, c);
    }

    #[test]
    fn a_contract_is_offered_once_what_it_needs_is_done() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        assert_eq!(p.offer(1, &inn), Err(ContractError::Locked));
        p.take_contract(0, &inn).unwrap();
        assert_eq!(p.take_contract(0, &inn), Err(ContractError::Taken));
        p.take_contract(2, &inn).unwrap();
        assert_eq!(p.taken(&inn), vec![0, 2]);
        p.drop_contract(2, &inn);
        assert_eq!(p.taken(&inn), vec![0]);
        assert_eq!(p.offer(99, &inn), Err(ContractError::Unknown));
    }

    #[test]
    fn progress_comes_once_per_outcome_and_only_to_taken_contracts() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.take_contract(0, &inn).unwrap();
        let near = V2::new(55.0, 50.0);
        let far = V2::new(95.0, 5.0);
        let o = outcome(Fate::Died, vec![(0, near), (0, near), (0, far), (1, near)]);
        after_raid(&mut p, &o, &inn, &c);
        // Applied again for the same raid: nothing more.
        p.apply_contracts(&o, &inn, &c);
        assert_eq!(p.contract(0, &inn).unwrap().progress, vec![2]);
        assert!(!p.contract_ready(0, &inn, &c));
        // A contract not taken moves on with nobody.
        assert!(p.contract(2, &inn).is_none_or(|s| s.progress.is_empty()));
        let two = outcome(Fate::Died, vec![(0, near), (0, near)]);
        after_raid(&mut p, &two, &inn, &c);
        assert_eq!(
            p.contract(0, &inn).unwrap().progress,
            vec![3],
            "held at the count"
        );
        assert!(p.contract_ready(0, &inn, &c));
        assert_eq!(
            p.objective_lines(0, &inn, &c),
            vec![("Kill Worgen near the place: 3 / 3".to_string(), true)]
        );
    }

    #[test]
    fn kills_count_by_kind_inside_the_circle_in_two_dimensions_and_never_when_not_numbers() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.take_contract(PACK, &inn).unwrap();
        let o = outcome(
            Fate::Died,
            vec![
                (0, V2::new(70.0, 50.0)),
                (0, V2::new(70.1, 50.0)),
                (1, V2::new(50.0, 50.0)),
                (0, V2::new(f32::NAN, 50.0)),
                (0, V2::new(50.0, f32::INFINITY)),
                (9, V2::new(50.0, 50.0)),
            ],
        );
        after_raid(&mut p, &o, &inn, &c);
        assert_eq!(
            p.contract(PACK, &inn).unwrap().progress,
            vec![1],
            "the one on the edge only"
        );
    }

    #[test]
    fn a_kill_that_is_not_a_number_never_counts_with_or_without_a_circle() {
        let c = raid();
        let mut inn = inn(&c);
        let mut p = Profile::default();
        p.take_contract(STRAYS, &inn).unwrap();
        let o = outcome(
            Fate::Died,
            vec![
                (1, V2::new(f32::NAN, 5.0)),
                (1, V2::new(5.0, f32::NEG_INFINITY)),
                (0, V2::new(5.0, 5.0)),
                (1, V2::new(1000.0, -1000.0)),
            ],
        );
        after_raid(&mut p, &o, &inn, &c);
        assert_eq!(
            p.contract(STRAYS, &inn).unwrap().progress,
            vec![1],
            "no circle: any place, but a number, and the right kind"
        );
        // A circle whose centre is not a number holds nothing.
        let mut q = Profile::default();
        q.take_contract(PACK, &inn).unwrap();
        let Objective::Kill { near, .. } = &mut inn.contracts[PACK].objectives[0] else {
            panic!("the pack contract kills");
        };
        *near = Some((V2::new(f32::NAN, 50.0), 20.0));
        let o = outcome(Fate::Died, vec![(0, V2::new(50.0, 50.0)); 3]);
        after_raid(&mut q, &o, &inn, &c);
        assert_eq!(q.contract(PACK, &inn).unwrap().progress, vec![0]);
    }

    #[test]
    fn turning_in_pays_unlocks_and_lets_a_repeatable_one_be_taken_again() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.take_contract(0, &inn).unwrap();
        assert_eq!(p.turn_in(0, &inn, &c), Err(ContractError::NotReady));
        let near = V2::new(50.0, 50.0);
        after_raid(&mut p, &outcome(Fate::Died, vec![(0, near); 3]), &inn, &c);
        assert_eq!(p.turn_in(0, &inn, &c), Ok(60));
        assert_eq!(p.gold, 60);
        assert_eq!(p.inn.stations, vec![("bench".to_string(), 0)]);
        assert_eq!(p.contract(0, &inn).unwrap().finished, 1);
        assert!(p.taken(&inn).is_empty());
        assert!(p.offer(0, &inn).is_ok(), "repeatable");
        assert!(p.offer(1, &inn).is_ok(), "its follower is offered now");
        p.take_contract(2, &inn).unwrap();
        after_raid(&mut p, &out_by("The Lane"), &inn, &c);
        assert_eq!(p.turn_in(2, &inn, &c), Ok(10));
        assert_eq!(
            p.offer(2, &inn),
            Err(ContractError::Finished),
            "not repeatable"
        );
        // Done again, the repeatable one pays again, and its station is not unlocked twice.
        p.take_contract(0, &inn).unwrap();
        after_raid(&mut p, &outcome(Fate::Died, vec![(0, near); 3]), &inn, &c);
        assert_eq!(p.turn_in(0, &inn, &c), Ok(60));
        assert_eq!(p.gold, 130);
        assert_eq!(p.inn.stations, vec![("bench".to_string(), 0)]);
        assert_eq!(p.contract(0, &inn).unwrap().finished, 2);
    }

    #[test]
    fn gold_from_a_contract_saturates() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            gold: u32::MAX - 3,
            ..Profile::default()
        };
        p.take_contract(OUT, &inn).unwrap();
        after_raid(&mut p, &out_by("The Lane"), &inn, &c);
        assert_eq!(p.turn_in(OUT, &inn, &c), Ok(10));
        assert_eq!(p.gold, u32::MAX);
    }

    #[test]
    fn bringing_takes_the_items_once() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.inn.contracts.push(ContractState {
            key: "pack".into(),
            taken: false,
            progress: Vec::new(),
            finished: 1,
        });
        p.take_contract(1, &inn).unwrap();
        // Out by the wrong exit: the extract is not made.
        after_raid(&mut p, &out_by("The Lane"), &inn, &c);
        assert!(!p.contract_ready(1, &inn, &c));
        after_raid(&mut p, &out_by("The Road"), &inn, &c);
        assert!(!p.contract_ready(1, &inn, &c), "and the tome is not here");
        p.loadout.pack.push(SavedStack {
            item: "tome".into(),
            count: 1,
        });
        assert_eq!(p.held("tome"), 1);
        assert!(p.contract_ready(1, &inn, &c));
        assert_eq!(p.turn_in(1, &inn, &c), Ok(100));
        assert_eq!(p.held("tome"), 0);
        assert_eq!(p.turn_in(1, &inn, &c), Err(ContractError::NotTaken));
        assert_eq!(p.gold, 100);
    }

    #[test]
    fn a_bring_counts_and_takes_the_stash_then_the_backpack_then_the_safe_slots() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            stash: vec![cloth(3)],
            ..Profile::default()
        };
        p.loadout.pack.push(cloth(4));
        p.loadout.safe.push((0, cloth(3)));
        assert_eq!(p.held("cloth"), 10);
        p.take_contract(CLOTH, &inn).unwrap();
        assert_eq!(p.turn_in(CLOTH, &inn, &c), Ok(7));
        assert!(p.stash.is_empty(), "the stash first");
        assert_eq!(p.loadout.pack, vec![cloth(2)], "then the backpack");
        assert_eq!(p.loadout.safe, vec![(0, cloth(3))]);
        assert_eq!(p.held("cloth"), 5);
    }

    #[test]
    fn what_is_not_in_the_stash_backpack_or_safe_slots_is_not_brought() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        // The companion's slot, worn gear and a bag hold plenty, and count for nothing.
        p.loadout.pet = Some(cloth(20));
        p.loadout.equip.push((EquipSlot::Head, cloth(20)));
        p.loadout.bags.push((0, cloth(20)));
        p.take_contract(CLOTH, &inn).unwrap();
        assert_eq!(p.held("cloth"), 0);
        assert!(!p.contract_ready(CLOTH, &inn, &c));
        assert_eq!(p.turn_in(CLOTH, &inn, &c), Err(ContractError::NotReady));
        assert_eq!(p.gold, 0, "and nothing is paid");
        assert_eq!(p.loadout.pet, Some(cloth(20)), "nor taken");
        // Part of it in the backpack: the rest is not made up from the companion's slot.
        p.loadout.pack.push(cloth(4));
        assert_eq!(p.turn_in(CLOTH, &inn, &c), Err(ContractError::NotReady));
        assert_eq!(p.loadout.pack, vec![cloth(4)]);
        assert_eq!(p.loadout.pet, Some(cloth(20)));
        assert_eq!(p.gold, 0);
    }

    #[test]
    fn a_repeatable_bring_takes_the_items_each_time_and_is_not_ready_once_they_are_gone() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            stash: vec![cloth(5)],
            ..Profile::default()
        };
        // The companion holds more, as the exploit would have it: it must make no difference.
        p.loadout.pet = Some(cloth(10));
        p.take_contract(CLOTH, &inn).unwrap();
        assert_eq!(p.turn_in(CLOTH, &inn, &c), Ok(7));
        assert!(p.stash.is_empty());
        p.take_contract(CLOTH, &inn).unwrap();
        assert_eq!(p.turn_in(CLOTH, &inn, &c), Err(ContractError::NotReady));
        assert_eq!(p.gold, 7, "paid once, for the items taken once");
        assert_eq!(p.loadout.pet, Some(cloth(10)));
        // With more to bring, the second turn-in takes its items too.
        p.stash.push(cloth(5));
        assert_eq!(p.turn_in(CLOTH, &inn, &c), Ok(7));
        assert_eq!(p.gold, 14);
        assert!(p.stash.is_empty());
        assert_eq!(p.contract(CLOTH, &inn).unwrap().finished, 2);
    }

    #[test]
    fn two_objectives_for_one_item_add_up() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile {
            stash: vec![cloth(5)],
            ..Profile::default()
        };
        p.take_contract(TWICE, &inn).unwrap();
        assert!(!p.contract_ready(TWICE, &inn, &c), "5 of the 8 it asks");
        // The lines say what the turn-in will take, so neither is ticked with 5 of the 8.
        let short = ("Bring Cloth: 5 / 8".to_string(), false);
        assert_eq!(
            p.objective_lines(TWICE, &inn, &c),
            vec![short.clone(), short]
        );
        assert_eq!(p.turn_in(TWICE, &inn, &c), Err(ContractError::NotReady));
        assert_eq!(p.stash, vec![cloth(5)], "nothing taken");
        p.stash.push(cloth(3));
        let full = ("Bring Cloth: 8 / 8".to_string(), true);
        assert_eq!(p.objective_lines(TWICE, &inn, &c), vec![full.clone(), full]);
        assert_eq!(p.turn_in(TWICE, &inn, &c), Ok(3));
        assert!(p.stash.is_empty(), "all eight taken");
    }

    #[test]
    fn no_more_than_the_inn_allows_at_once_and_nothing_during_a_raid() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.inn.contracts.push(ContractState {
            key: "pack".into(),
            taken: false,
            progress: Vec::new(),
            finished: 1,
        });
        p.take_contract(0, &inn).unwrap();
        p.take_contract(1, &inn).unwrap();
        assert_eq!(p.take_contract(2, &inn), Err(ContractError::TooMany));
        p.begin_raid(0, &c);
        p.drop_contract(0, &inn);
        assert_eq!(
            p.taken(&inn),
            vec![0, 1],
            "nothing changes while a raid is under way"
        );
        assert_eq!(p.turn_in(0, &inn, &c), Err(ContractError::InRaid));
    }

    #[test]
    fn a_contract_the_content_no_longer_has_is_left_alone_and_breaks_nothing() {
        let c = raid();
        let inn = inn(&c);
        let gone = ContractState {
            key: "gone".into(),
            taken: true,
            progress: vec![9, 9, 9, 9],
            finished: 3,
        };
        let mut p = Profile::default();
        p.inn.contracts.push(gone.clone());
        // Known contracts whose states have too few, and too many, numbers for their objectives.
        p.inn.contracts.push(ContractState {
            key: "pack".into(),
            taken: true,
            progress: Vec::new(),
            finished: 0,
        });
        p.inn.contracts.push(ContractState {
            key: "tome".into(),
            taken: true,
            progress: vec![7; 6],
            finished: 0,
        });
        assert_eq!(p.taken(&inn), vec![0, 1], "the stale one is not counted");
        assert_eq!(p.offer(OUT, &inn), Err(ContractError::TooMany));
        assert!(!p.contract_ready(PACK, &inn, &c));
        assert_eq!(p.objective_lines(PACK, &inn, &c).len(), 1);
        assert_eq!(p.objective_lines(1, &inn, &c).len(), 2);
        assert_eq!(p.turn_in(PACK, &inn, &c), Err(ContractError::NotReady));
        let o = outcome(Fate::Died, vec![(0, V2::new(50.0, 50.0))]);
        after_raid(&mut p, &o, &inn, &c);
        assert_eq!(p.contract(PACK, &inn).unwrap().progress, vec![1]);
        assert_eq!(p.contract(1, &inn).unwrap().progress.len(), 2);
        p.drop_contract(PACK, &inn);
        assert_eq!(
            p.inn.contracts.iter().find(|s| s.key == "gone"),
            Some(&gone),
            "kept as it was"
        );
    }

    #[test]
    fn a_profile_with_huge_numbers_does_not_overflow() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.inn.contracts.push(ContractState {
            key: "pack".into(),
            taken: true,
            progress: vec![u32::MAX],
            finished: u32::MAX,
        });
        let o = outcome(Fate::Died, vec![(0, V2::new(50.0, 50.0)); 2]);
        after_raid(&mut p, &o, &inn, &c);
        assert_eq!(p.contract(PACK, &inn).unwrap().progress, vec![3], "capped");
        assert_eq!(p.turn_in(PACK, &inn, &c), Ok(60));
        assert_eq!(p.contract(PACK, &inn).unwrap().finished, u32::MAX);
    }
}
