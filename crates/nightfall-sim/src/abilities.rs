//! Abilities as content (`content/nightfall/abilities.ron`, spec §6): what each costs and takes,
//! how far it reaches and how loud it is, what its caster may do with their feet meanwhile, and
//! what it does where it lands. Heroes' bars, enemies' moves and (in sub-project 7) rivals' kits
//! all name abilities from this one table by key, so giving anyone any ability is a content
//! change, not a code change.

use serde::{Deserialize, Serialize};

use crate::content::{non_negative, positive};
use crate::{Loudness, V2};

/// What a caster may do with their feet while an ability is cast (spec §6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Moving {
    /// Instants: nothing holds the caster.
    #[default]
    Free,
    /// Cast-time spells: the caster moves at `cast_move_speed`.
    Slowed,
    /// Channels and the biggest nukes: the caster stands, and must be standing to begin.
    Rooted,
}

/// How loud an ability is where it goes off (spec §5.2's quiet, normal and loud), or silent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Sound {
    Silent,
    Quiet,
    #[default]
    Normal,
    Loud,
}

impl Sound {
    /// The noise it makes, if any.
    pub fn loudness(self) -> Option<Loudness> {
        match self {
            Sound::Silent => None,
            Sound::Quiet => Some(Loudness::Quiet),
            Sound::Normal => Some(Loudness::Normal),
            Sound::Loud => Some(Loudness::Loud),
        }
    }
}

/// What an ability is aimed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Aim {
    /// The caster's target, an enemy.
    Enemy,
    /// The caster, and every enemy within `radius` of them when it has one.
    Me,
    /// A spot on the ground within `range`, and every enemy within `radius` of it.
    Ground,
}

/// Where the caster must stand toward its target (WoW's facing rules, spec §6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Facing {
    #[default]
    Any,
    /// Facing the target.
    Front,
    /// Facing the target, from behind it.
    Behind,
}

/// A creature's kind, for abilities that take only some (Sap humanoids, Polymorph humanoids and
/// beasts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Creature {
    #[default]
    Humanoid,
    Beast,
    Undead,
}

/// What holds an enemy (spec §6.2, §6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ControlKind {
    /// No step, no blow (Cheap Shot); damage does not free it.
    Stun,
    /// No step, no blow (Gouge); freed by damage.
    Incapacitate,
    /// A sheep that wanders (Polymorph); freed by damage.
    Polymorph,
    /// Out of the fight and unaware (Sap); freed by damage.
    Sap,
    /// Held where it stands, still striking what it reaches (Frost Nova); freed by damage.
    Root,
}

impl ControlKind {
    /// Whether a blow frees the enemy.
    pub fn freed_by_damage(self) -> bool {
        self != ControlKind::Stun
    }

    /// Whether it keeps the enemy out of the fight altogether: no looking, thinking or blows.
    pub fn out(self) -> bool {
        self != ControlKind::Root
    }
}

/// What an ability does where it lands.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Effect {
    /// Damage from `min` to `max`, and `per_combo` more for each combo point a finisher spends.
    Damage {
        min: f32,
        max: f32,
        #[serde(default)]
        per_combo: f32,
    },
    /// The class weapon's blow, `percent` of it and `bonus` more.
    Weapon {
        #[serde(default = "hundred")]
        percent: f32,
        #[serde(default)]
        bonus: f32,
    },
    /// `amount` every `every` seconds for `secs` (a bleed, a burn).
    Dot { amount: f32, every: f32, secs: f32 },
    /// Slower by `by` (0.4 is 40% slower) for `secs`.
    Slow { by: f32, secs: f32 },
    /// Held (see [`ControlKind`]) for `secs`.
    Control { kind: ControlKind, secs: f32 },
    /// Combo points on the target (spec §6.3).
    Combo(u8),
    /// The Rogue's Stealth: its button counts a Stealth press (`PlayerInput::stealth_presses`).
    Stealth,
    /// Every enemy after the caster loses them, and they are in stealth again.
    Vanish,
    /// Faster by `by` for `secs`.
    Sprint { by: f32, secs: f32 },
    /// `distance` yards the way the caster faces, stopping short of walls.
    Blink { distance: f32 },
    /// Enemies within the ability's radius of the spot turn to it for `secs`.
    Distract { secs: f32 },
    /// Health (and mana, drinking a conjured drink) back over `secs`, sitting still.
    Eat { secs: f32 },
    /// `count` conjured drinks.
    ConjureWater { count: u8 },
}

impl Effect {
    /// Whether it does anything to the enemies it lands on.
    pub fn on_enemy(&self) -> bool {
        matches!(
            self,
            Effect::Damage { .. }
                | Effect::Weapon { .. }
                | Effect::Dot { .. }
                | Effect::Slow { .. }
                | Effect::Control { .. }
                | Effect::Combo(_)
        )
    }
}

fn hundred() -> f32 {
    100.0
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

/// What an ability needs before it may be used (WoW's own checks).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Needs {
    /// Only from stealth.
    pub stealth: bool,
    /// An opener: its target must not be alerted to the caster (spec §5.4).
    pub unaware: bool,
    /// A finisher: combo points on its target.
    pub combo: bool,
    pub out_of_combat: bool,
    /// Not while moving.
    pub still: bool,
    /// Only these kinds of creature; any when empty.
    pub creatures: Vec<Creature>,
}

/// The ground a telegraphed blow lands on (spec §6.1).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum Shape {
    Circle {
        radius: f32,
    },
    /// From the caster the way it faces, `angle` degrees wide.
    Cone {
        length: f32,
        angle: f32,
    },
}

impl Shape {
    /// Whether the ground point `p` is in this shape put at `at`, turned toward `facing`. Nothing
    /// is hit by a shape put or turned at a point that is not finite, and a cone has no hit
    /// without a direction to face.
    pub fn contains(&self, at: V2, facing: V2, p: V2) -> bool {
        if !(at.is_finite() && facing.is_finite() && p.is_finite()) {
            return false;
        }
        let d = p - at;
        match *self {
            Shape::Circle { radius } => d.length() <= radius,
            Shape::Cone { length, angle } => {
                if d.length() > length {
                    return false;
                }
                let (f, n) = (facing.normalize_or_zero(), d.normalize_or_zero());
                if f == V2::ZERO {
                    return false;
                }
                n == V2::ZERO || f.dot(n) >= (angle * 0.5).to_radians().cos() - 1e-5
            }
        }
    }

    /// How far from where it is put it reaches.
    pub fn reach(&self) -> f32 {
        match *self {
            Shape::Circle { radius } => radius,
            Shape::Cone { length, .. } => length,
        }
    }
}

/// A blow shown on the ground before it lands.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct TelegraphDef {
    pub shape: Shape,
    /// Seconds from the shape showing to the blow landing (spec §6.1: 1 to 1.5).
    pub windup: f32,
    /// A circle around the caster, not at its target.
    #[serde(default)]
    pub on_self: bool,
}

/// What an AI may use an ability for: enemies now, rivals' kits later (spec §7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Tag {
    Opener,
    Builder,
    Finisher,
    Nuke,
    Control,
    Escape,
    Heal,
    Utility,
}

/// One ability.
#[derive(Clone, Debug, Deserialize)]
pub struct AbilityDef {
    pub key: String,
    pub name: String,
    /// `Spell.dbc` id: the button's icon and tooltip, the cast bar's name, the visual.
    pub spell: u32,
    /// The visual of each tick of a channel (Arcane Missiles' missile), when not `spell`'s.
    #[serde(default)]
    pub tick_spell: Option<u32>,
    pub aim: Aim,
    /// Yards to the target or spot; the target's size is added (`CombatRules::reach`).
    #[serde(default)]
    pub range: f32,
    /// Yards around the caster or the spot an area ability reaches.
    #[serde(default)]
    pub radius: f32,
    /// Mana or energy.
    #[serde(default)]
    pub cost: f32,
    /// Seconds to cast; 0 for an instant.
    #[serde(default)]
    pub cast: f32,
    /// Seconds a channel lasts, in `ticks` even ticks; 0 for none.
    #[serde(default)]
    pub channel: f32,
    #[serde(default = "one")]
    pub ticks: u32,
    #[serde(default)]
    pub cooldown: f32,
    /// The global cooldown it starts (a mage's 1.5, a rogue's 1); 0 for none.
    #[serde(default)]
    pub gcd: f32,
    #[serde(default)]
    pub moving: Moving,
    #[serde(default)]
    pub sound: Sound,
    /// A fire spell's flash on the caster (spec §5.1).
    #[serde(default)]
    pub flash: bool,
    #[serde(default)]
    pub facing: Facing,
    #[serde(default)]
    pub needs: Needs,
    /// Landing on an enemy alerts it to the caster (all but Sap and Distract).
    #[serde(default = "yes")]
    pub alerts: bool,
    /// The caster stays in stealth (Sap, Sprint, Distract, Vanish, Stealth itself).
    #[serde(default)]
    pub keeps_stealth: bool,
    /// Landing starts the caster's auto-attack on the target.
    #[serde(default)]
    pub attack: bool,
    /// An enemy's big move: shown on the ground before it lands.
    #[serde(default)]
    pub telegraph: Option<TelegraphDef>,
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

impl AbilityDef {
    /// Seconds from beginning to going off: a telegraph's windup, else its cast time.
    pub fn windup(&self) -> f32 {
        self.telegraph.map_or(self.cast, |t| t.windup)
    }

    /// Whether any of its effects is one `pick` picks.
    pub fn has(&self, pick: impl Fn(&Effect) -> bool) -> bool {
        self.effects.iter().any(pick)
    }

    /// Why it cannot work, if it cannot. Every number must be finite: a range of infinity would
    /// reach across the map, and a NaN fails a comparison only by luck.
    pub(crate) fn check(&self) -> Result<(), String> {
        let k = &self.key;
        let numbers = [
            self.range,
            self.radius,
            self.cost,
            self.cast,
            self.channel,
            self.cooldown,
            self.gcd,
        ];
        if !numbers.into_iter().all(non_negative) {
            return Err(format!(
                "\"{k}\": range, radius, cost, cast, channel, cooldown and gcd are 0 or more (and finite)"
            ));
        }
        if matches!(self.aim, Aim::Enemy | Aim::Ground) && self.range <= 0.0 {
            return Err(format!(
                "\"{k}\": an ability aimed at an enemy or the ground needs a range"
            ));
        }
        if self.channel > 0.0 && (self.cast > 0.0 || self.ticks == 0) {
            return Err(format!("\"{k}\": a channel has ticks and no cast time"));
        }
        if let Some(t) = &self.telegraph {
            let sized = match t.shape {
                Shape::Circle { radius } => positive(radius),
                Shape::Cone { length, angle } => {
                    positive(length) && positive(angle) && angle <= 360.0
                }
            };
            if !(positive(t.windup) && sized) {
                return Err(format!("\"{k}\": a telegraph needs a windup and a size"));
            }
        }
        for e in &self.effects {
            let works = match *e {
                Effect::Damage {
                    min,
                    max,
                    per_combo,
                } => {
                    non_negative(min) && non_negative(max) && max >= min && non_negative(per_combo)
                }
                Effect::Weapon { percent, bonus } => non_negative(percent) && non_negative(bonus),
                Effect::Dot {
                    amount,
                    every,
                    secs,
                } => non_negative(amount) && positive(every) && positive(secs) && secs >= every,
                Effect::Slow { by, secs } => positive(by) && by < 1.0 && positive(secs),
                Effect::Control { secs, .. } => positive(secs),
                Effect::Combo(n) => (1..=5).contains(&n),
                Effect::Sprint { by, secs } => positive(by) && positive(secs),
                Effect::Blink { distance } => positive(distance),
                Effect::Distract { secs } | Effect::Eat { secs } => positive(secs),
                Effect::ConjureWater { count } => count > 0,
                Effect::Stealth | Effect::Vanish => true,
            };
            if !works {
                return Err(format!("\"{k}\": {e:?} cannot work"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circle_holds_what_is_within_its_radius() {
        let s = Shape::Circle { radius: 8.0 };
        let at = V2::new(10.0, 10.0);
        assert!(s.contains(at, V2::new(1.0, 0.0), V2::new(17.9, 10.0)));
        assert!(!s.contains(at, V2::new(1.0, 0.0), V2::new(18.1, 10.0)));
        assert_eq!(s.reach(), 8.0);
    }

    #[test]
    fn a_cone_holds_what_is_ahead_within_its_angle_and_length() {
        let s = Shape::Cone {
            length: 8.0,
            angle: 120.0,
        };
        let at = V2::new(0.0, 0.0);
        let north = V2::new(1.0, 0.0);
        assert!(s.contains(at, north, V2::new(5.0, 0.0)));
        // 50° off straight ahead: inside the 60° half-angle.
        assert!(s.contains(at, north, V2::from_angle(50f32.to_radians(), 5.0)));
        assert!(!s.contains(at, north, V2::from_angle(70f32.to_radians(), 5.0)));
        assert!(!s.contains(at, north, V2::new(9.0, 0.0)));
        assert!(!s.contains(at, north, V2::new(-3.0, 0.0)));
    }

    #[test]
    fn no_hit_without_a_finite_point_or_a_direction() {
        let circle = Shape::Circle { radius: 8.0 };
        let cone = Shape::Cone {
            length: 8.0,
            angle: 120.0,
        };
        let at = V2::ZERO;
        let north = V2::new(1.0, 0.0);
        let inside = V2::new(3.0, 0.0);
        let nan = V2::new(f32::NAN, 0.0);
        let inf = V2::new(f32::INFINITY, 0.0);
        // A cone needs a direction to face: zero, NaN and infinite facings hit nothing.
        for facing in [V2::ZERO, nan, inf] {
            assert!(!cone.contains(at, facing, inside), "{facing:?}");
        }
        // A circle does not turn, so a zero facing is no reason to miss it; a facing that is not
        // a number is still refused.
        assert!(circle.contains(at, V2::ZERO, inside));
        assert!(!circle.contains(at, nan, inside));
        assert!(!circle.contains(at, inf, inside));
        // A point that is not a number is never in either shape, however it was put.
        assert!(!circle.contains(at, north, nan));
        assert!(!cone.contains(at, north, nan));
        assert!(!cone.contains(at, north, inf));
        assert!(!circle.contains(nan, north, inside));
        assert!(!cone.contains(inf, north, inside));
        assert!(cone.contains(at, north, inside));
    }

    #[test]
    fn holds_and_their_freeing() {
        assert!(!ControlKind::Stun.freed_by_damage());
        assert!(ControlKind::Sap.freed_by_damage());
        assert!(ControlKind::Root.freed_by_damage());
        assert!(!ControlKind::Root.out());
        assert!(ControlKind::Polymorph.out());
        assert_eq!(Sound::Silent.loudness(), None);
        assert_eq!(Sound::Loud.loudness(), Some(Loudness::Loud));
        assert!(Effect::Combo(1).on_enemy());
        assert!(!Effect::Vanish.on_enemy());
    }
}
