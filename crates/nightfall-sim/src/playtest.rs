//! The playtest settings (the playtest kit, spec §13): the rules the design leaves open, or a
//! tester wants to vary, as one small value the inn's Playtest panel edits and the host applies
//! to the raid it starts. Pure: [`Playtest::content_for`] turns the settings into the rules a
//! raid is rolled and played under, [`Playtest::roll`] rolls a raid under them, and the text form
//! ([`Playtest::to_text`], [`Playtest::from_text`]) is what the client saves. The default
//! settings change nothing: the content comes back as it was and the roll is the plain roll.

use std::sync::Arc;

use crate::{Content, DeathLoss, RaidRoll};

/// How the rival parties are rolled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RivalsOut {
    /// By the rivals' rules and odds.
    #[default]
    Rolled,
    /// None come.
    Off,
    /// Exactly one party, the likeliest of the roll.
    One,
}

/// How the signature monsters are rolled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MonstersOut {
    /// By the night's rules and the monsters' odds.
    #[default]
    Rolled,
    /// Every monster that can come, comes.
    All,
    /// Only those that always come.
    None,
}

/// How fast the night's clock runs against the raid's real time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NightSpeed {
    #[default]
    X1,
    X2,
    X4,
}

/// Whether the rivals' simplified kits stay simplified.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RivalKits {
    /// As `rivals.ron` has them (the three parties that come by odds have a few abilities).
    #[default]
    Simplified,
    /// Every kind plays the whole bar of its base hero class.
    Full,
}

/// The playtest settings. `Default` is how the game plays with no panel at all.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Playtest {
    /// What a death takes; `None` keeps `rules.ron`'s.
    pub death_loss: Option<DeathLoss>,
    pub rival_kits: RivalKits,
    pub rivals: RivalsOut,
    pub monsters: MonstersOut,
    pub night_speed: NightSpeed,
    /// A weather by key; `None` is the raid's rolled one.
    pub weather: Option<String>,
}

/// The value after `now` in `all`, round the ring; the first when `now` is not in it.
pub fn next_of<T: Copy + PartialEq>(all: &[T], now: T) -> T {
    match all.iter().position(|v| *v == now) {
        Some(i) => all[(i + 1) % all.len()],
        None => all[0],
    }
}

impl NightSpeed {
    pub const ALL: [Self; 3] = [Self::X1, Self::X2, Self::X4];

    /// Seconds of night a second of play.
    pub fn factor(self) -> f32 {
        match self {
            Self::X1 => 1.0,
            Self::X2 => 2.0,
            Self::X4 => 4.0,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::X1 => "1",
            Self::X2 => "2",
            Self::X4 => "4",
        }
    }
}

impl RivalsOut {
    pub const ALL: [Self; 3] = [Self::Rolled, Self::Off, Self::One];

    fn key(self) -> &'static str {
        match self {
            Self::Rolled => "rolled",
            Self::Off => "off",
            Self::One => "one",
        }
    }
}

impl MonstersOut {
    pub const ALL: [Self; 3] = [Self::Rolled, Self::All, Self::None];

    fn key(self) -> &'static str {
        match self {
            Self::Rolled => "rolled",
            Self::All => "all",
            Self::None => "none",
        }
    }
}

impl RivalKits {
    pub const ALL: [Self; 2] = [Self::Simplified, Self::Full];

    fn key(self) -> &'static str {
        match self {
            Self::Simplified => "simplified",
            Self::Full => "full",
        }
    }
}

/// `rules.ron`'s name for a death rule.
pub fn death_loss_key(d: DeathLoss) -> &'static str {
    match d {
        DeathLoss::Everything => "everything",
        DeathLoss::EverythingOneRecovery => "everything_one_recovery",
        DeathLoss::FoundOnly => "found_only",
    }
}

/// The death rules in the order the panel cycles them.
pub const DEATH_LOSSES: [DeathLoss; 3] = [
    DeathLoss::Everything,
    DeathLoss::EverythingOneRecovery,
    DeathLoss::FoundOnly,
];

impl Playtest {
    /// Whether these are the default settings, which change nothing.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The death rule after the current one: the content's, then each of the three, then the
    /// content's again.
    pub fn cycle_death_loss(&mut self) {
        self.death_loss = match self.death_loss {
            None => Some(DEATH_LOSSES[0]),
            Some(d) => DEATH_LOSSES
                .iter()
                .position(|x| *x == d)
                .and_then(|i| DEATH_LOSSES.get(i + 1))
                .copied(),
        };
    }

    /// The weather after the current one: the rolled one, then each of the content's, then the
    /// rolled one again. A weather the content does not have starts the ring over.
    pub fn cycle_weather(&mut self, content: &Content) {
        let keys: Vec<&str> = content
            .rules
            .night
            .weather
            .iter()
            .map(|w| w.key.as_str())
            .collect();
        self.weather = match &self.weather {
            None => keys.first().map(|k| (*k).to_owned()),
            Some(now) => keys
                .iter()
                .position(|k| *k == now.as_str())
                .and_then(|i| keys.get(i + 1))
                .map(|k| (*k).to_owned()),
        };
    }

    /// The rules a raid under these settings is rolled and played with: `base` itself when the
    /// settings leave the content alone, else a copy with the death rule, the rivals' and
    /// monsters' counts and the rivals' kits as set. The weather and the night's speed are not
    /// rules: see [`Playtest::roll`] and [`Playtest::night_factor`].
    pub fn content_for(&self, base: &Arc<Content>) -> Result<Arc<Content>, String> {
        if self.death_loss.is_none()
            && self.rival_kits == RivalKits::Simplified
            && self.rivals == RivalsOut::Rolled
            && self.monsters == MonstersOut::Rolled
        {
            return Ok(base.clone());
        }
        let mut c = if self.rival_kits == RivalKits::Full {
            base.with_full_rival_kits()?
        } else {
            (**base).clone()
        };
        if let Some(d) = self.death_loss {
            c.rules.run.death_loss = d;
        }
        match self.rivals {
            RivalsOut::Rolled => {}
            RivalsOut::Off => c.rules.rivals.parties_out = (0, 0),
            RivalsOut::One => c.rules.rivals.parties_out = (1, 1),
        }
        match self.monsters {
            MonstersOut::Rolled => {}
            MonstersOut::All => c.rules.night.monsters_out = (u8::MAX, u8::MAX),
            MonstersOut::None => c.rules.night.monsters_out = (0, 0),
        }
        Ok(Arc::new(c))
    }

    /// The raid of `seed` rolled under these settings. `content` is [`Playtest::content_for`]'s,
    /// so the counts are the settings'; the weather, if one is set, replaces the rolled one (a
    /// key the content lacks leaves it).
    pub fn roll(&self, content: &Content, seed: u64) -> RaidRoll {
        let mut roll = RaidRoll::roll(content, seed);
        if let Some(i) = self.weather.as_ref().and_then(|key| {
            content
                .rules
                .night
                .weather
                .iter()
                .position(|w| w.key == *key)
        }) {
            roll.weather = i as u8;
        }
        roll
    }

    /// Seconds of night a second of play.
    pub fn night_factor(&self) -> f32 {
        self.night_speed.factor()
    }

    /// The saved form, one `key=value` a line; a setting at its default is left out.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if let Some(d) = self.death_loss {
            out.push_str(&format!("death_loss={}\n", death_loss_key(d)));
        }
        if self.rival_kits != RivalKits::Simplified {
            out.push_str(&format!("rival_kits={}\n", self.rival_kits.key()));
        }
        if self.rivals != RivalsOut::Rolled {
            out.push_str(&format!("rivals={}\n", self.rivals.key()));
        }
        if self.monsters != MonstersOut::Rolled {
            out.push_str(&format!("monsters={}\n", self.monsters.key()));
        }
        if self.night_speed != NightSpeed::X1 {
            out.push_str(&format!("night_speed={}\n", self.night_speed.key()));
        }
        if let Some(w) = &self.weather {
            out.push_str(&format!("weather={w}\n"));
        }
        out
    }

    /// Settings from their saved form: a line that is not understood keeps its default.
    pub fn from_text(text: &str) -> Self {
        let mut t = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "death_loss" => {
                    t.death_loss = DEATH_LOSSES
                        .iter()
                        .copied()
                        .find(|d| death_loss_key(*d) == value);
                }
                "rival_kits" => {
                    if let Some(v) = RivalKits::ALL.into_iter().find(|v| v.key() == value) {
                        t.rival_kits = v;
                    }
                }
                "rivals" => {
                    if let Some(v) = RivalsOut::ALL.into_iter().find(|v| v.key() == value) {
                        t.rivals = v;
                    }
                }
                "monsters" => {
                    if let Some(v) = MonstersOut::ALL.into_iter().find(|v| v.key() == value) {
                        t.monsters = v;
                    }
                }
                "night_speed" => {
                    if let Some(v) = NightSpeed::ALL.into_iter().find(|v| v.key() == value) {
                        t.night_speed = v;
                    }
                }
                "weather" if !value.is_empty() => t.weather = Some(value.to_owned()),
                _ => {}
            }
        }
        t
    }
}
