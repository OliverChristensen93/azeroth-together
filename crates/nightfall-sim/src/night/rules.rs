//! The night's numbers (spec §7.2, §7.3, §8.1, §11's `night_stages`): `rules.ron`'s `night:`.
//! Every number is a starting value to playtest; each one left out takes the spec's.

use serde::Deserialize;

use crate::content::{non_negative, positive};

/// A sound the client plays: a SoundEntries kit by id or by name (heard from where it is), or a
/// file by its path (heard everywhere alike). The host never plays one; it says when.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum CueSound {
    Kit(u32),
    Name(String),
    File(String),
}

/// A sound an enemy makes while it moves (Stitches' tread, spec §7.2's "loud"): the client's.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct StepSound {
    pub sound: CueSound,
    /// Seconds between two.
    pub every: f32,
    /// Heard within this many yards of the local hero.
    #[serde(default = "sixty")]
    pub heard: f32,
}

fn sixty() -> f32 {
    60.0
}

fn twenty() -> f32 {
    20.0
}

fn one() -> f32 {
    1.0
}

/// One stage of the night clock (spec §7.2).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct StageDef {
    pub key: String,
    pub name: String,
    /// Seconds into the raid it lands.
    pub at: f32,
    /// Seconds before it lands that its cue sounds.
    #[serde(default = "twenty")]
    pub warn: f32,
    /// While it lasts, the fog's distances are times this (Moonrise's 40 to 30 yards is 0.75).
    #[serde(default = "one")]
    pub fog: f32,
    /// While it lasts, enemies' notice range is times this (Moonrise's +20% is 1.2).
    #[serde(default = "one")]
    pub notice: f32,
    /// While it lasts, hunters go after the loudest recent noise (the hunt).
    #[serde(default)]
    pub hunt: bool,
    /// What the players hear `warn` seconds before it lands, and the line their chat shows then.
    #[serde(default)]
    pub cue: Option<CueSound>,
    #[serde(default)]
    pub line: Option<String>,
}

impl StageDef {
    /// A stage with the defaults a file leaves out.
    fn new(key: &str, name: &str, at: f32) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            at,
            warn: twenty(),
            fog: 1.0,
            notice: 1.0,
            hunt: false,
            cue: None,
            line: None,
        }
    }
}

/// One weather a raid may roll (spec §8.1: clear, mist, rain shortening the fog).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WeatherDef {
    pub key: String,
    pub name: String,
    /// How often it is rolled, against the others' weights.
    pub weight: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    /// The fog's colour (sRGB 0..1); the atmosphere's when left out.
    #[serde(default)]
    pub fog_color: Option<(f32, f32, f32)>,
    /// How hard it rains, 0 (no rain) to 1: the engine's own rain at this grade.
    #[serde(default)]
    pub rain: f32,
    /// Its sound loop, a SoundEntries kit (8533 light rain, 8534 medium, 8535 heavy); 0 for none.
    #[serde(default)]
    pub sound: u32,
}

/// How a raid rolls its exits (spec §8.1, §7.2).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct ExitRoll {
    /// The chance each exit that is not late stays shut the whole raid.
    pub shut: f32,
    /// The chance each exit that is not late opens late, at a time within `late_window`.
    pub late: f32,
    pub late_window: (f32, f32),
    /// The late exits (Darkshire's cart) open within this many seconds of `cart_opens_at`.
    pub cart_jitter: f32,
    /// The chance one exit closes as the hunt lands (spec §7.2: "one exit may close").
    pub hunt_close: f32,
    /// However the dice fall, at least this many exits open in a raid, one of them from the start.
    pub open_least: u8,
}

impl Default for ExitRoll {
    fn default() -> Self {
        Self {
            shut: 0.15,
            late: 0.25,
            late_window: (900.0, 1320.0),
            cart_jitter: 120.0,
            hunt_close: 0.5,
            open_least: 2,
        }
    }
}

/// Scent trails (spec §7.1, §7.3: the trackers).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct ScentRules {
    /// A hero leaves a crumb every so many yards walked.
    pub every: f32,
    /// Seconds a crumb lasts (the spec's "trail of the last 30 s").
    pub lasts: f32,
    /// A tracker smells a crumb within this many yards.
    pub pick: f32,
}

impl Default for ScentRules {
    fn default() -> Self {
        Self {
            every: 2.0,
            lasts: 30.0,
            pick: 3.0,
        }
    }
}

/// The hunt (spec §7.2: "worgen packs hunt toward the loudest recent noise").
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct HuntRules {
    /// Seconds between two calls to the hunt.
    pub every: f32,
    /// Seconds a noise counts as recent.
    pub memory: f32,
    /// Hunters within this many yards of the noise go.
    pub reach: f32,
    /// Seconds a hunter looks about where the noise was before it goes home.
    pub rest: f32,
}

impl Default for HuntRules {
    fn default() -> Self {
        Self {
            every: 15.0,
            memory: 30.0,
            reach: 150.0,
            rest: 4.0,
        }
    }
}

/// The ravens (spec §7.1): an alarm on the ground.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct RavenRules {
    /// The chance each of the map's flocks is out in a raid.
    pub odds: f32,
    /// A flock takes off when a hero runs within this many yards, or fights within the next.
    pub run_within: f32,
    pub fight_within: f32,
    /// Seconds before a flock that took off settles again, once no hero is within `clear` yards.
    pub back_after: f32,
    pub clear: f32,
    /// What the client draws: so many birds of this `CreatureDisplayInfo` id at this scale, and
    /// the cry it plays as they take off.
    pub birds: u8,
    pub display: u32,
    pub scale: f32,
    pub cry: Option<CueSound>,
}

impl Default for RavenRules {
    fn default() -> Self {
        Self {
            odds: 0.7,
            run_within: 8.0,
            fight_within: 15.0,
            back_after: 120.0,
            clear: 20.0,
            birds: 3,
            display: 14562,
            scale: 0.3,
            cry: None,
        }
    }
}

/// The night (spec §7.2, §7.3, §8.1).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct NightRules {
    /// The clock's stages, the first at 0.
    pub stages: Vec<StageDef>,
    /// The weathers a raid may roll; none for the atmosphere's own fog every raid.
    pub weather: Vec<WeatherDef>,
    pub exits: ExitRoll,
    /// How many of the signature monsters that are not always out a raid rolls (fewest, most).
    pub monsters_out: (u8, u8),
    /// A stage's newcomers wait until no hero is this near where they come, yards.
    pub arrive_unseen: f32,
    /// Mor'Ladim counts as looked at within this many degrees of a hero's facing (the whole cone).
    pub look_cone: f32,
    pub scent: ScentRules,
    pub hunt: HuntRules,
    pub ravens: RavenRules,
}

impl Default for NightRules {
    fn default() -> Self {
        Self {
            stages: spec_stages(),
            weather: Vec::new(),
            exits: ExitRoll::default(),
            monsters_out: (1, 3),
            arrive_unseen: 50.0,
            look_cone: 90.0,
            scent: ScentRules::default(),
            hunt: HuntRules::default(),
            ravens: RavenRules::default(),
        }
    }
}

/// The spec's §7.2 table: Dusk at 0:00, the howling at 8:00, Moonrise at 14:00 (fog 40 to 30
/// yards, notice +20%), the hunt at 20:00, Stitches at 26:00; Moonrise's fog and eyes last.
fn spec_stages() -> Vec<StageDef> {
    let late = |key: &str, name: &str, at: f32, hunt: bool| StageDef {
        fog: 0.75,
        notice: 1.2,
        hunt,
        ..StageDef::new(key, name, at)
    };
    vec![
        StageDef::new("dusk", "Dusk", 0.0),
        StageDef::new("howling", "The howling", 480.0),
        late("moonrise", "Moonrise", 840.0, false),
        late("hunt", "The hunt", 1200.0, true),
        late("stitches", "Stitches", 1560.0, true),
    ]
}

impl NightRules {
    /// What is wrong with these numbers, if anything: each check is one the night relies on.
    pub(crate) fn check(&self) -> Result<(), String> {
        let unit = |x: f32| (0.0..=1.0).contains(&x);
        let Some(first) = self.stages.first() else {
            return Err("needs a stage".into());
        };
        if first.at != 0.0 {
            return Err(format!("the first stage, \"{}\", lands at 0", first.key));
        }
        if self.stages.len() > usize::from(u8::MAX) || self.weather.len() > usize::from(u8::MAX) {
            return Err("at most 255 stages and 255 weathers".into());
        }
        for (i, s) in self.stages.iter().enumerate() {
            let who = format!("stage \"{}\"", s.key);
            if self.stages[..i].iter().any(|t| t.key == s.key) {
                return Err(format!("{who} is listed twice"));
            }
            if !(s.at.is_finite()
                && non_negative(s.warn)
                && positive(s.fog)
                && s.fog <= 2.0
                && positive(s.notice)
                && s.notice <= 3.0)
            {
                return Err(format!(
                    "{who}: warn 0 or more, fog above 0 to 2, notice above 0 to 3"
                ));
            }
            if i > 0 && !(s.at > self.stages[i - 1].at && s.at - s.warn >= self.stages[i - 1].at) {
                return Err(format!(
                    "{who} lands after the stage before it, and is cued after that one lands"
                ));
            }
            // A line goes into Lua between long brackets.
            if s.line.as_deref().is_some_and(|l| l.contains("]]")) {
                return Err(format!("{who}: a line may not hold \"]]\""));
            }
        }
        for (i, w) in self.weather.iter().enumerate() {
            let who = format!("weather \"{}\"", w.key);
            if self.weather[..i].iter().any(|v| v.key == w.key) {
                return Err(format!("{who} is listed twice"));
            }
            if !(non_negative(w.weight)
                && non_negative(w.fog_start)
                && w.fog_start < w.fog_end
                && w.fog_end.is_finite()
                && unit(w.rain))
            {
                return Err(format!(
                    "{who}: weight 0 or more, fog_start from 0 and below fog_end, rain 0 to 1"
                ));
            }
        }
        if !self.weather.is_empty() && !positive(self.weather.iter().map(|w| w.weight).sum::<f32>())
        {
            return Err("the weathers' weights add up to more than 0".into());
        }
        let x = &self.exits;
        if !(unit(x.shut)
            && unit(x.late)
            && unit(x.hunt_close)
            && non_negative(x.cart_jitter)
            && non_negative(x.late_window.0)
            && x.late_window.1.is_finite()
            && x.late_window.0 <= x.late_window.1
            && x.open_least >= 1)
        {
            return Err(
                "exits: chances 0 to 1, a late_window from 0 up in order, cart_jitter 0 or more, open_least 1 or more"
                    .into(),
            );
        }
        if self.monsters_out.0 > self.monsters_out.1 {
            return Err("monsters_out is (fewest, most)".into());
        }
        if !(non_negative(self.arrive_unseen)
            && positive(self.look_cone)
            && self.look_cone <= 360.0)
        {
            return Err("arrive_unseen 0 or more, look_cone above 0 to 360".into());
        }
        let (s, h, r) = (&self.scent, &self.hunt, &self.ravens);
        if !(positive(s.every) && positive(s.lasts) && positive(s.pick)) {
            return Err("scent: every, lasts and pick above 0".into());
        }
        if !(positive(h.every) && positive(h.memory) && positive(h.reach) && non_negative(h.rest)) {
            return Err("hunt: every, memory and reach above 0, rest 0 or more".into());
        }
        if !(unit(r.odds)
            && non_negative(r.run_within)
            && non_negative(r.fight_within)
            && positive(r.back_after)
            && non_negative(r.clear)
            && positive(r.scale))
        {
            return Err(
                "ravens: odds 0 to 1, distances 0 or more, back_after and scale above 0".into(),
            );
        }
        Ok(())
    }
}
