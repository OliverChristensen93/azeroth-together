//! The stash between runs, on disk: `benilla-config/together-armory.ron`, the
//! [`together_sim::Armory`] as RON. Pieces a hero finds in a run are put in the stash as they
//! drop ([`collect_found`]), so a crash or a quit loses nothing; they can be worn from the next
//! run on.
//!
//! `TOGETHER_GEAR_SEED=1` starts from a test stash instead (one of every unique and a spread of
//! rarities, a set worn by the mage) and keeps it in `together-armory-test.ron`, so a test run
//! never touches the real one. `TOGETHER_GEAR_ON=1` switches every brought piece on as a run
//! starts, for a test shot of a fully geared hero.

use bevy::prelude::*;
use together_sim::{Armor, Armory, GearPiece, Rarity, Slot, Unique, World, MAX_ITEM_LEVEL};

use super::{Phase, Sim};

/// The armory, and how much of the current run's finds is in it already.
#[derive(Resource)]
pub(super) struct GearStore {
    pub armory: Armory,
    /// The local hero's finds this run already put in the stash.
    saved: usize,
    /// Bumped on every change, so the screens that show it refresh once.
    pub version: u32,
}

impl Default for GearStore {
    fn default() -> Self {
        let armory = if seeded() {
            load().unwrap_or_else(test_armory)
        } else {
            load().unwrap_or_default()
        };
        Self {
            armory,
            saved: 0,
            version: 0,
        }
    }
}

impl GearStore {
    /// Write the armory to disk, and let the screens know it changed.
    pub fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
        if let Some(path) = path() {
            if let Err(e) = crate::local_state::write_atomic(&path, &self.armory.to_ron()) {
                warn!("together: could not save the stash: {e}");
            }
        }
    }

    /// What a hero of class `class` brings into a run.
    pub fn equipped(&self, class: &str) -> Vec<GearPiece> {
        self.armory.equipped(class)
    }
}

fn seeded() -> bool {
    std::env::var_os("TOGETHER_GEAR_SEED").is_some()
}

fn path() -> Option<std::path::PathBuf> {
    let name = if seeded() {
        "together-armory-test.ron"
    } else {
        "together-armory.ron"
    };
    crate::local_state::home().map(|h| h.join(name))
}

fn load() -> Option<Armory> {
    let text = crate::local_state::read_to_string(&path()?).ok()?;
    let armory = Armory::from_ron(&text);
    if armory.is_none() {
        warn!("together: the stash file does not read; starting an empty one");
    }
    armory
}

/// A stash to test with: every unique, a piece of every rarity for every slot, and the mage
/// wearing a set of uniques and rares.
fn test_armory() -> Armory {
    let mut a = Armory::default();
    let mut found: Vec<GearPiece> = Unique::ALL
        .into_iter()
        .enumerate()
        .map(|(i, u)| GearPiece::unique(u, 40 + i as u16, 100 + i as u32))
        .collect();
    let rarities = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic];
    for (i, slot) in Slot::ALL.into_iter().enumerate() {
        for (j, &r) in rarities.iter().enumerate() {
            let level = 10 + ((i * 7 + j * 13) as u16 % MAX_ITEM_LEVEL);
            found.push(GearPiece::new(
                slot,
                r,
                level,
                Armor::Cloth,
                (i * 31 + j * 7) as u32,
            ));
        }
    }
    a.add_found("mage", &found);
    let locker = a.locker_mut("mage");
    // The mage wears the cloth uniques, Atiesh, and epics where no unique fits.
    for want in [
        Some(Unique::Atiesh),
        Some(Unique::RobeOfTheArchmage),
        Some(Unique::MantleOfTheBlackwingCabal),
        Some(Unique::GlacialCloak),
        Some(Unique::FrostfireSandals),
        Some(Unique::GlovesOfTheHypnoticFlame),
        Some(Unique::NemesisSkullcap),
    ] {
        if let Some(i) = locker.stash.iter().position(|p| p.unique == want) {
            locker.equip(i);
        }
    }
    for slot in [Slot::Wrists, Slot::Waist, Slot::Legs] {
        if let Some(i) = locker
            .stash
            .iter()
            .position(|p| p.slot == slot && p.rarity == Rarity::Epic)
        {
            locker.equip(i);
        }
    }
    locker.unseen = 4;
    a
}

/// During a run: the local hero's newest finds into its class's locker, saved at once.
pub(super) fn collect_found(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    mut store: ResMut<GearStore>,
) {
    let Some(sim) = sim.filter(|_| *phase.get() == Phase::Playing) else {
        return;
    };
    if sim.is_added() {
        store.saved = 0;
    }
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let found = &hero.gear.found;
    if found.len() <= store.saved {
        return;
    }
    let class = sim.world.content().classes[hero.class].key.clone();
    let new = found[store.saved..].to_vec();
    store.saved = found.len();
    for p in &new {
        info!("together: found {} (item level {})", p.name(), p.item_level);
    }
    store.armory.add_found(&class, &new);
    store.changed();
}

/// `TOGETHER_GEAR_ON=1`: every hero's brought gear is on from the start, for a test shot.
pub(super) fn gear_on_for_tests(world: &mut World) {
    if std::env::var_os("TOGETHER_GEAR_ON").is_none() {
        return;
    }
    let players: Vec<_> = world.heroes.iter().map(|h| h.player).collect();
    for p in players {
        world.switch_all_on(p);
    }
}
