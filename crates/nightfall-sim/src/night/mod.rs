//! The night's threats (spec §7.2, §7.3, §8.1): the clock and its stages, the raid's roll,
//! signature monsters, risk zones, scent, the hunt and the ravens. The content is in `rules.rs`
//! and `places.rs`; the host's rules in the files beside them. A client's mirror runs none of
//! them: it has the night its host sends in each snapshot ([`NightView`]), and draws its fog.

mod arrivals;
mod clock;
mod hunt;
mod places;
mod roll;
mod scent;
mod signature;
mod rules;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use scent::Crumb;
pub(crate) use places::{there_and_back, MonsterFile, RoadFile, ZoneFile};
pub use places::{MonsterDef, RoadDef, ZoneDef};
pub use rules::{
    CueSound, ExitRoll, HuntRules, NightRules, RavenRules, ScentRules, StageDef, StepSound,
    WeatherDef,
};

use serde::{Deserialize, Serialize};

use crate::{Content, World};

/// A flock not out this raid.
pub const FLOCK_GONE: u8 = 0;
/// A flock on the ground.
pub const FLOCK_SETTLED: u8 = 1;
/// A flock that has taken off and not settled again.
pub const FLOCK_FLOWN: u8 = 2;

/// The night as every client sees it, in each snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NightView {
    /// The stage the night is in, an index into the night's stages.
    pub stage: u8,
    /// The raid's weather, an index into the night's weathers.
    pub weather: u8,
    /// Each of the map's flocks: [`FLOCK_GONE`], [`FLOCK_SETTLED`] or [`FLOCK_FLOWN`].
    pub flocks: Vec<u8>,
}

/// The fog as the night has it now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub start: f32,
    pub end: f32,
    /// sRGB 0..1.
    pub color: (f32, f32, f32),
}

/// The fog of `weather` (the atmosphere's own when the night has no weathers) in `stage`: its
/// distances times the stage's `fog` (spec §7.2's Moonrise, §8.1's mist and rain).
pub fn fog_of(content: &Content, stage: usize, weather: usize) -> Fog {
    let a = &content.rules.atmosphere;
    let n = &content.rules.night;
    let (start, end, color) = n.weather.get(weather).map_or(
        (a.fog_start, a.fog_end, a.fog_color),
        |w| (w.fog_start, w.fog_end, w.fog_color.unwrap_or(a.fog_color)),
    );
    let k = n.stages.get(stage).map_or(1.0, |s| s.fog);
    Fog {
        start: start * k,
        end: end * k,
        color,
    }
}

/// The night's state on the host; on a client's mirror only `mirror`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Night {
    /// A client's mirror: the night its host sent.
    pub(crate) mirror: Option<NightView>,
    /// The stage the clock has reached.
    pub(crate) stage: usize,
    /// How many stages have been cued (the first counts as cued: the raid starts in it).
    pub(crate) cued: usize,
    /// The hunt's exit has closed, and said so.
    pub(crate) closed: bool,
    /// Spawn lines whose stage has landed, waiting until no hero is near where they come.
    pub(crate) waiting: Vec<usize>,
    /// Seconds to the next try at placing them.
    pub(crate) try_in: f32,
    /// The loudest noise heard lately, for the hunt.
    pub(crate) loudest: Option<hunt::Loud>,
    /// Seconds to the hunt's next call.
    pub(crate) hunt_in: f32,
    /// Each hero's scent trail.
    pub(crate) trails: Vec<scent::Trail>,
}

impl World {
    /// The night as every client sees it: on the host from its clock and its roll, on a client's
    /// mirror as the host sent it.
    pub fn night_view(&self) -> NightView {
        if let Some(v) = &self.night.mirror {
            return v.clone();
        }
        NightView {
            stage: self.night.stage as u8,
            weather: self.raid.roll.weather,
            flocks: Vec::new(),
        }
    }

    /// The stage and the weather now, by index.
    fn sky(&self) -> (usize, usize) {
        match &self.night.mirror {
            Some(v) => (usize::from(v.stage), usize::from(v.weather)),
            None => (self.night.stage, usize::from(self.raid.roll.weather)),
        }
    }

    /// The fog now: how far heroes and enemies see on the host, what every client draws.
    pub fn fog(&self) -> Fog {
        let (stage, weather) = self.sky();
        fog_of(self.content(), stage, weather)
    }

    /// What enemies' notice range is times now (Moonrise's +20%, spec §7.2).
    pub fn notice_scale(&self) -> f32 {
        let (stage, _) = self.sky();
        self.content()
            .rules
            .night
            .stages
            .get(stage)
            .map_or(1.0, |s| s.notice)
    }

    /// The night's part of a step, after the raid's and before the enemies'. A mirror's night
    /// never runs: its host's comes in each snapshot.
    pub(crate) fn step_night(&mut self) {
        if self.night.mirror.is_some() {
            return;
        }
        self.step_clock();
        self.step_arrivals();
        self.lay_scent();
    }

    /// The night's hold on the enemies, after the fight's own ([`World::hold_enemies`]) and before
    /// they look and move. A mirror and a world without a map have none.
    pub(crate) fn night_holds(&mut self) {
        if self.night.mirror.is_some() || self.map().is_none() {
            return;
        }
        // Mor'Ladim first: a watched one is out of the fight for everything after.
        self.watch_morladim();
        self.follow_scent();
        self.step_hunt();
    }
}
