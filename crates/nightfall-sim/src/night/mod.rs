//! The night's threats (spec §7.2, §7.3, §8.1): the clock and its stages, the raid's roll,
//! signature monsters, risk zones, scent, the hunt and the ravens. This first part is their
//! content; the host's rules come in the files beside it.

mod places;
mod roll;
mod rules;
#[cfg(test)]
mod tests;

pub(crate) use places::{there_and_back, MonsterFile, RoadFile, ZoneFile};
pub use places::{MonsterDef, RoadDef, ZoneDef};
pub use rules::{
    CueSound, ExitRoll, HuntRules, NightRules, RavenRules, ScentRules, StageDef, StepSound,
    WeatherDef,
};
