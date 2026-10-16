//! Spells as the world casts them: their tags, how they pick targets, the status effects they
//! leave, and the numbers one cast carries once its rank, its hero's stats and its augments are
//! applied ([`CastSpec`]).
//!
//! Tags are what talents, tomes and augments refer to: "+1 projectile" is for every Projectile
//! spell, a Frostfire conversion for any Fire or Frost spell. A conversion adds a school's tag, so
//! both schools' bonuses then apply.

use serde::{Deserialize, Serialize};

use crate::content::{AbilityDef, AbilityKind};

/// A spell's chance to crit before any bonus.
pub const BASE_CRIT: f32 = 0.05;

/// What a spell is: its school, how it travels, what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tag {
    Physical,
    Fire,
    Frost,
    Arcane,
    Nature,
    Shadow,
    Holy,
    Projectile,
    Melee,
    Chain,
    Area,
    /// An area in a cone from the caster.
    Cone,
    /// A lasting area on the ground.
    Ground,
    Beam,
    Summon,
    Aura,
    Totem,
    Shield,
    Direct,
    DamageOverTime,
    /// Chills: slows what it hits.
    Slow,
    Root,
    Freeze,
    Stun,
    Fear,
    Knockback,
    Heal,
}

impl Tag {
    pub const ALL: [Tag; 27] = [
        Tag::Physical,
        Tag::Fire,
        Tag::Frost,
        Tag::Arcane,
        Tag::Nature,
        Tag::Shadow,
        Tag::Holy,
        Tag::Projectile,
        Tag::Melee,
        Tag::Chain,
        Tag::Area,
        Tag::Cone,
        Tag::Ground,
        Tag::Beam,
        Tag::Summon,
        Tag::Aura,
        Tag::Totem,
        Tag::Shield,
        Tag::Direct,
        Tag::DamageOverTime,
        Tag::Slow,
        Tag::Root,
        Tag::Freeze,
        Tag::Stun,
        Tag::Fear,
        Tag::Knockback,
        Tag::Heal,
    ];

    /// The magic schools and Physical: what a conversion changes.
    pub const SCHOOLS: [Tag; 7] = [
        Tag::Physical,
        Tag::Fire,
        Tag::Frost,
        Tag::Arcane,
        Tag::Nature,
        Tag::Shadow,
        Tag::Holy,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tag::Physical => "Physical",
            Tag::Fire => "Fire",
            Tag::Frost => "Frost",
            Tag::Arcane => "Arcane",
            Tag::Nature => "Nature",
            Tag::Shadow => "Shadow",
            Tag::Holy => "Holy",
            Tag::Projectile => "Projectile",
            Tag::Melee => "Melee",
            Tag::Chain => "Chain",
            Tag::Area => "Area",
            Tag::Cone => "Cone",
            Tag::Ground => "Ground",
            Tag::Beam => "Beam",
            Tag::Summon => "Summon",
            Tag::Aura => "Aura",
            Tag::Totem => "Totem",
            Tag::Shield => "Shield",
            Tag::Direct => "Direct",
            Tag::DamageOverTime => "Damage over time",
            Tag::Slow => "Slow",
            Tag::Root => "Root",
            Tag::Freeze => "Freeze",
            Tag::Stun => "Stun",
            Tag::Fear => "Fear",
            Tag::Knockback => "Knockback",
            Tag::Heal => "Heal",
        }
    }

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// A set of [`Tag`]s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tags(pub u32);

impl Tags {
    pub fn of(tags: &[Tag]) -> Tags {
        Tags(tags.iter().fold(0, |acc, t| acc | t.bit()))
    }

    pub fn has(self, tag: Tag) -> bool {
        self.0 & tag.bit() != 0
    }

    /// Whether any of `tags` is here.
    pub fn any(self, tags: &[Tag]) -> bool {
        tags.iter().any(|&t| self.has(t))
    }

    pub fn with(self, tag: Tag) -> Tags {
        Tags(self.0 | tag.bit())
    }

    pub fn without(self, tag: Tag) -> Tags {
        Tags(self.0 & !tag.bit())
    }

    /// The tags here, in [`Tag::ALL`]'s order.
    pub fn iter(self) -> impl Iterator<Item = Tag> {
        Tag::ALL.into_iter().filter(move |&t| self.has(t))
    }

    /// How many tags the two sets share.
    pub fn shared(self, other: Tags) -> u32 {
        (self.0 & other.0).count_ones()
    }
}

/// How a spell picks where it lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Targeting {
    /// The nearest enemies.
    Nearest,
    /// The nearest elite or boss in range; else the nearest enemy.
    ElitesFirst,
    /// The middle of the thickest crowd in range.
    Densest,
    /// Everything around the caster.
    Around,
    /// A cone ahead of the caster.
    Ahead,
    /// The caster itself (a shield).
    Caster,
}

/// Which way a cone points: ahead of the caster (its own way), behind it, or four narrower cones
/// in every direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cone {
    Ahead,
    Behind,
    Four,
}

/// Seconds a Chill lasts, and how much it slows.
pub const CHILL_SECS: f32 = 3.0;
pub const CHILL_SLOW: f32 = 0.4;
/// Seconds a Freeze lasts before augments.
pub const FREEZE_SECS: f32 = 2.0;
/// Seconds a Burning effect lasts.
pub const BURN_SECS: f32 = 4.0;
/// Yards a Knockback pushes before augments.
pub const KNOCKBACK: f32 = 2.5;
/// How long control effects hold an elite, and a boss, against a normal enemy.
pub const ELITE_CONTROL: f32 = 0.5;
pub const BOSS_CONTROL: f32 = 0.25;
/// The half-angle of a cone, in radians (45°).
pub const CONE_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

/// One cast's numbers and behaviour, after rank, stats, power-ups and augments.
#[derive(Clone, Debug, PartialEq)]
pub struct CastSpec {
    /// Index into the content's abilities: what it is, and its damage meter row.
    pub ability: usize,
    pub tags: Tags,
    pub targeting: Targeting,
    /// Per hit.
    pub damage: f32,
    pub cooldown: f32,
    /// Targets for the single-target kinds; missiles for a volley.
    pub count: u32,
    /// Hits on each target (Arcane Missiles' volley).
    pub volley: u32,
    /// Enemies a projectile passes through after its target.
    pub pierce: u32,
    pub range: f32,
    /// Area radius, splash radius, or ground radius.
    pub radius: f32,
    /// Inner edge of a ring, 0 for a full circle.
    pub ring: f32,
    /// Which way a cone points, for a spell aimed [`Targeting::Ahead`].
    pub cone: Cone,
    pub missile_speed: f32,
    pub crit: f32,
    /// What a critical strike multiplies by.
    pub crit_mult: f32,
    /// Seconds of Chill on a hit, 0 for none.
    pub chill: f32,
    /// How much its Chill slows (0.4 is 40%).
    pub slow: f32,
    /// Seconds of Freeze on a hit, 0 for none.
    pub freeze: f32,
    /// Chance a Chill also Freezes.
    pub freeze_chance: f32,
    /// Chill when a Freeze thaws (Permafrost).
    pub permafrost: bool,
    /// Seconds of Stun on a hit, 0 for none.
    pub stun: f32,
    /// Yards pushed away on a hit, 0 for none.
    pub knockback: f32,
    /// Share of each hit dealt again as Burning over [`BURN_SECS`].
    pub burn: f32,
    /// How much harder this spell's damage over time hits, how much faster it ticks, and how much
    /// longer it lasts.
    pub dot_mult: f32,
    pub dot_speed: f32,
    pub dot_secs: f32,
    /// Ticks a second of its burns and ground areas (1 is one a second): each tick as strong, so
    /// a faster rate deals more.
    pub tick_rate: f32,
    /// A Burning effect jumps to a nearby enemy when its target dies.
    pub contagion: bool,
    /// Seconds a lasting ground area stays.
    pub ground: f32,
    /// The ground area follows its caster.
    pub follows: bool,
    /// A nova or ground area lands on the thickest crowd instead of around the caster.
    pub targeted: bool,
    /// Cast again at half strength a second later.
    pub echo: bool,
    /// Cast twice.
    pub twin: bool,
    /// Missiles come back and hit again.
    pub returning: bool,
    /// Extra damage against Chilled enemies, Frozen ones, and those under 30% health.
    pub vs_chilled: f32,
    pub vs_frozen: f32,
    pub execute: f32,
    /// Extra crit chance against Frozen enemies.
    pub shatter: f32,
    /// Max health a shield is worth, for a shield spell.
    pub shield: f32,
    /// Charges held at most.
    pub charges: u8,
}

impl CastSpec {
    /// The control effects a spell can lose to Brute Force.
    pub fn has_control(&self) -> bool {
        self.chill > 0.0 || self.freeze > 0.0 || self.stun > 0.0
    }
}

/// How long a control effect of `secs` holds an enemy: shorter on elites and bosses.
pub fn control_secs(secs: f32, elite: bool, boss: bool) -> f32 {
    if boss {
        secs * BOSS_CONTROL
    } else if elite {
        secs * ELITE_CONTROL
    } else {
        secs
    }
}

impl CastSpec {
    /// Ability `ability` (`def`) cast with nothing on it: no rank, stats, upgrades or augments.
    pub fn base(ability: usize, def: &AbilityDef) -> CastSpec {
        let tags = def.tag_set;
        CastSpec {
            ability,
            tags,
            targeting: def.targeting(),
            damage: def.damage,
            cooldown: def.cooldown,
            count: def.count,
            volley: def.volley.max(1),
            pierce: 0,
            range: def.range,
            radius: def.radius,
            ring: 0.0,
            cone: Cone::Ahead,
            missile_speed: def.missile_speed,
            crit: BASE_CRIT,
            crit_mult: 2.0,
            chill: if tags.has(Tag::Slow) { CHILL_SECS } else { 0.0 },
            slow: CHILL_SLOW,
            freeze: if tags.has(Tag::Freeze) {
                def.freeze
            } else {
                0.0
            },
            freeze_chance: 0.0,
            permafrost: false,
            stun: if tags.has(Tag::Stun) { 1.0 } else { 0.0 },
            knockback: if tags.has(Tag::Knockback) {
                def.knockback
            } else {
                0.0
            },
            burn: def.burn,
            dot_mult: 1.0,
            dot_speed: 1.0,
            dot_secs: 1.0,
            tick_rate: 1.0,
            contagion: false,
            ground: if def.kind == AbilityKind::Ground {
                def.duration
            } else {
                def.leaves_ground
            },
            follows: false,
            targeted: false,
            echo: false,
            twin: false,
            returning: false,
            vs_chilled: 0.0,
            vs_frozen: 0.0,
            execute: 0.0,
            shatter: 0.0,
            shield: def.shield,
            charges: 1,
        }
    }
}

/// The placeholders an ability's description may hold, each `{name}`; [`describe`] fills them
/// from the cast, so the text always gives its real numbers.
pub const PLACEHOLDERS: [&str; 19] = [
    "damage",
    "range",
    "radius",
    "area",
    "targets",
    "count",
    "volley",
    "cooldown",
    "duration",
    "tick",
    "ground",
    "chill",
    "slow",
    "freeze",
    "stun",
    "burn",
    "burn_secs",
    "shield",
    "shield_hp",
];

/// A number as a description gives it: whole when it is, else to one decimal.
fn num(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{:.0}", v)
    } else {
        format!("{:.1}", v)
    }
}

/// `def`'s description with its placeholders filled from cast `s`, for a hero of `max_hp`
/// health. An unknown placeholder is left as written.
pub fn describe(def: &AbilityDef, s: &CastSpec, max_hp: f32) -> String {
    let value = |name: &str| -> Option<String> {
        Some(match name {
            "damage" => num(s.damage.round()),
            "range" => num(s.range),
            "radius" => num(s.radius),
            "area" => match (s.targeting, s.cone) {
                (Targeting::Ahead, Cone::Ahead) => {
                    format!("in a {}-yard cone ahead of you", num(s.radius))
                }
                (Targeting::Ahead, Cone::Behind) => {
                    format!("in a {}-yard cone behind you", num(s.radius))
                }
                (Targeting::Ahead, Cone::Four) => {
                    format!("in {}-yard cones on all four sides of you", num(s.radius))
                }
                _ if s.ring > 0.0 => format!(
                    "between {} and {} yards from you",
                    num(s.ring),
                    num(s.radius)
                ),
                _ => format!("within {} yards of you", num(s.radius)),
            },
            "targets" => match (s.targeting, s.count) {
                (Targeting::ElitesFirst, 1) => {
                    "the closest enemy (prioritizing bosses and elites)".to_string()
                }
                (Targeting::ElitesFirst, n) => {
                    format!("the {n} closest enemies (prioritizing bosses and elites)")
                }
                (Targeting::Densest, _) => "the thickest crowd".to_string(),
                (_, 1) => "the closest enemy".to_string(),
                (_, n) => format!("the {n} closest enemies"),
            },
            "count" => s.count.to_string(),
            "volley" => s.volley.to_string(),
            "cooldown" => num(s.cooldown),
            "duration" => num(if def.kind == AbilityKind::Ground {
                s.ground
            } else {
                def.duration
            }),
            "tick" => num(1.0 / s.tick_rate.max(0.1)),
            "ground" => num(s.ground),
            "chill" => num(s.chill),
            "slow" => num(s.slow * 100.0),
            "freeze" => num(s.freeze),
            "stun" => num(s.stun),
            "burn" => num((s.burn * s.dot_mult * s.dot_secs * s.tick_rate * 100.0).round()),
            "burn_secs" => num(BURN_SECS * s.dot_secs / s.dot_speed.max(0.1)),
            "shield" => num((s.shield * 100.0).round()),
            "shield_hp" => num((s.shield * max_hp).round()),
            _ => return None,
        })
    };
    let text = &def.description;
    let mut out = String::with_capacity(text.len() + 32);
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after
            .find('}')
            .and_then(|close| Some((close, value(&after[..close])?)))
        {
            Some((close, v)) => {
                out.push_str(&v);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_description_gives_the_casts_own_numbers() {
        let mut def = crate::content::tests::sample().abilities[0].clone();
        def.description = "Hits {targets} within {range} yards for {damage}, {nope}.".into();
        let mut s = CastSpec::base(0, &def);
        s.damage = 12.4;
        s.range = 20.0 * 1.1;
        s.targeting = Targeting::Nearest;
        s.count = 3;
        assert_eq!(
            describe(&def, &s, 100.0),
            "Hits the 3 closest enemies within 22 yards for 12, {nope}."
        );
    }

    #[test]
    fn tags_hold_what_they_are_given() {
        let t = Tags::of(&[Tag::Fire, Tag::Projectile]);
        assert!(t.has(Tag::Fire) && t.has(Tag::Projectile) && !t.has(Tag::Frost));
        assert!(t.with(Tag::Frost).has(Tag::Frost));
        assert!(!t.without(Tag::Fire).has(Tag::Fire));
        assert_eq!(
            t.iter().collect::<Vec<_>>(),
            vec![Tag::Fire, Tag::Projectile]
        );
        assert_eq!(t.shared(Tags::of(&[Tag::Fire, Tag::Frost])), 1);
        assert!(t.any(&[Tag::Frost, Tag::Projectile]));
    }

    #[test]
    fn control_is_shorter_on_elites_and_bosses() {
        assert_eq!(control_secs(2.0, false, false), 2.0);
        assert_eq!(control_secs(2.0, true, false), 1.0);
        assert_eq!(control_secs(2.0, true, true), 0.5);
    }
}
