//! Casting: how a hero's spells pick their targets and land, the status effects they leave, the
//! lasting ground areas and shields some make, and the mechanics of the augments that act on a
//! cast, a hit, a crit, a kill or a blow taken.
//!
//! A cast is built once into a [`CastSpec`] (its rank, the hero's stats, the party's power-ups and
//! its augments all applied), then fired: missiles become [`PendingHit`]s that land after their
//! flight, novas and cones hit at once, ground areas stay and hit every second, shields wait on
//! the hero. Every hit goes through [`World::strike`], where crits, status effects and the
//! on-hit capstones happen.
//!
//! An augment that casts a spell (Hot Streak's Pyroblast, Final Gale's Frost Nova) queues a
//! [`Trigger`]; triggers run after the cast that set them off, as real casts of the spell with
//! the hero's rank and augments, ignoring its cooldown. Mechanics that could set each other off
//! forever have an internal cooldown ([`ICD`]).

mod talents;

use super::*;
use crate::augments::{Augment, Milestone};
use crate::cards::{boost_lines, Boost};
use crate::spell::{
    control_secs, CastSpec, Cone, Tag, Tags, Targeting, CHILL_SECS, CHILL_SLOW, CONE_HALF_ANGLE,
    FREEZE_SECS,
};

/// Seconds between the casts of a spell's charges.
const CHARGE_GAP: f32 = 0.35;
/// Seconds between the missiles of one volley.
const VOLLEY_GAP: f32 = 0.12;
/// A burning ground area's damage each second, as a share of the nova that left it.
const BURNING_GROUND: f32 = 0.15;
/// How far a piercing missile goes past its target, and how near its line an enemy must be.
const PIERCE_REACH: f32 = 8.0;
const PIERCE_WIDTH: f32 = 1.2;
/// The internal cooldown of mechanics that cast spells, so none can loop.
pub(super) const ICD: f32 = 8.0;
/// Triggers one step may run: a safety net, since internal cooldowns already stop loops.
const TRIGGERS_PER_STEP: usize = 64;
/// Living Bomb: seconds to the blast, its reach, and how much of the hit it deals.
const BOMB_SECS: f32 = 3.0;
const BOMB_RADIUS: f32 = 8.0;
const BOMB_SHARE: f32 = 1.5;
/// Frozen Orb: yards per second.
const ORB_SPEED: f32 = 5.0;
/// Implosion pulls enemies within this many yards to this far from the caster.
const IMPLOSION_REACH: f32 = 12.0;
const IMPLOSION_CLOSE: f32 = 1.5;
/// Frostbite's shatter, and Shatterbolt's splash.
const FROSTBITE_RADIUS: f32 = 4.0;
const FROSTBITE_SHARE: f32 = 0.5;
const SHATTERBOLT_RADIUS: f32 = 5.0;
/// Shattering Barrier's blast: its reach and how many times the absorbed damage it deals.
const BARRIER_BLAST: f32 = 15.0;
const BARRIER_SHARE: f32 = 3.0;
/// Missile Storm's beat.
const STORM_EVERY: f32 = 10.0;
/// Contagion's jump.
const CONTAGION_REACH: f32 = 8.0;

/// What one cast of a spell is aimed from, and how strongly.
#[derive(Clone, Copy, Debug)]
pub(super) struct Shot {
    /// Where it comes from or lands, when not the caster (a Frost Nova at a Blizzard's centre).
    pub at: Option<V2>,
    /// The way a cone points, when not the caster's facing (Wake of Winter's cone behind).
    pub facing: Option<V2>,
    pub scale: f32,
    /// An enemy it must not pick (Presence of Mind's second target).
    pub exclude: Option<u32>,
    /// Its own cast, not an echo, twin or extra: only these echo, twin and set off Firestorm.
    pub primary: bool,
}

impl Shot {
    pub fn plain() -> Shot {
        Shot {
            at: None,
            facing: None,
            scale: 1.0,
            exclude: None,
            primary: true,
        }
    }
}

/// A spell an augment casts: run after the cast that set it off.
#[derive(Clone, Copy, Debug)]
pub(super) struct Trigger {
    pub player: PlayerId,
    pub ability: usize,
    pub shot: Shot,
}

/// A spell's state on an enemy: control effects, burns, Winter's Chill, a Living Bomb.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// On a client: the flags its snapshot carried, shown as they are.
    pub(super) mirrored: u16,
    pub chill: f32,
    /// How much the Chill slows (the strongest that hit it).
    pub slow: f32,
    pub freeze: f32,
    pub stun: f32,
    /// Who froze it, for Frostbite.
    frozen_by: Option<PlayerId>,
    /// It stays Chilled when its Freeze thaws.
    permafrost: bool,
    pub(super) burns: Vec<Burn>,
    /// Winter's Chill: stacks of +2% crit chance taken, and seconds before they fade.
    winters: u8,
    winters_time: f32,
    bomb: Option<Bomb>,
    /// Inside a Whiteout: takes more from every source while above 0.
    exposed: f32,
    /// The spell that hit it last, for Inferno.
    pub(super) last_ability: Option<usize>,
    /// The other classes' effects on it: Fear, Root, Weakened, Marks, Sunder.
    pub fx: super::classes::StatusFx,
}

/// A status's flags as a snapshot carries them, for a client to show.
pub mod status_bits {
    pub const FROZEN: u16 = 1;
    pub const CHILLED: u16 = 2;
    /// Fire (or any other school's) damage over time.
    pub const BURNING: u16 = 4;
    pub const STUNNED: u16 = 8;
    pub const FEARED: u16 = 16;
    pub const ROOTED: u16 = 32;
    /// Marked to take more (Hunter's Mark, Faerie Fire and the like).
    pub const MARKED: u16 = 64;
    pub const BLEEDING: u16 = 128;
    pub const POISONED: u16 = 256;
    /// Shadow damage over time: Corruption, Shadow Word: Pain.
    pub const AFFLICTED: u16 = 512;
}

impl Status {
    pub fn burning(&self) -> bool {
        self.burns.iter().any(|b| b.left > 0.0)
    }

    /// Frozen or Stunned: it neither moves nor strikes.
    pub fn held(&self) -> bool {
        self.freeze > 0.0 || self.stun > 0.0
    }

    pub fn bits(&self) -> u16 {
        // A client's mirrored status has no burns or effects of its own: its flags came as they are.
        let mut b = self.mirrored;
        if self.freeze > 0.0 {
            b |= status_bits::FROZEN;
        }
        if self.chill > 0.0 {
            b |= status_bits::CHILLED;
        }
        if self.stun > 0.0 {
            b |= status_bits::STUNNED;
        }
        if self.fx.fear > 0.0 {
            b |= status_bits::FEARED;
        }
        if self.fx.root > 0.0 {
            b |= status_bits::ROOTED;
        }
        if self.fx.mark_secs > 0.0 || self.fx.marked_by.is_some() {
            b |= status_bits::MARKED;
        }
        for burn in self.burns.iter().filter(|x| x.left > 0.0) {
            b |= if burn.tags.has(Tag::Bleed) {
                status_bits::BLEEDING
            } else if burn.tags.has(Tag::Poison) {
                status_bits::POISONED
            } else if burn.tags.has(Tag::Shadow) {
                status_bits::AFFLICTED
            } else {
                status_bits::BURNING
            };
        }
        b
    }

    /// A status from a snapshot's flags: enough for a client to draw.
    pub fn from_bits(bits: u16) -> Status {
        let on = |flag: u16| if bits & flag != 0 { 1.0 } else { 0.0 };
        Status {
            freeze: on(status_bits::FROZEN),
            chill: on(status_bits::CHILLED),
            stun: on(status_bits::STUNNED),
            mirrored: bits,
            ..Status::default()
        }
    }

    /// Whether one of `by`'s damage over time effects tagged `tag` is on it.
    pub(super) fn has_dot(&self, by: PlayerId, tag: Tag) -> bool {
        self.burns.iter().any(|b| b.by == by && b.tags.has(tag))
    }

    /// The damage its burns would still deal.
    fn burn_left(&self) -> f32 {
        self.burns
            .iter()
            .map(|b| b.dps * b.left.max(0.0) / b.every)
            .sum()
    }
}

/// Damage over time from one spell of one hero: Burning, a Bleed, a Poison.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Burn {
    pub by: PlayerId,
    pub ability: usize,
    /// Damage a tick, every `every` seconds while `left` lasts.
    pub dps: f32,
    pub left: f32,
    /// Seconds to its next tick, and between ticks.
    pub tick: f32,
    pub every: f32,
    pub contagion: bool,
    /// The tags of the spell that left it (a Bleed, a Poison), and the share of each tick that
    /// heals its caster.
    pub tags: Tags,
    pub leech: f32,
}

/// A Living Bomb, waiting to go off.
#[derive(Clone, Debug, PartialEq)]
struct Bomb {
    by: PlayerId,
    ability: usize,
    damage: f32,
    time: f32,
    /// Set off by another bomb: it makes no more.
    child: bool,
}

/// A lasting area on the ground: a Blizzard, Flamestrike's burning ground, a Frozen Orb.
#[derive(Clone, Debug)]
pub struct Ground {
    pub by: PlayerId,
    pub spec: CastSpec,
    pub pos: V2,
    pub left: f32,
    tick: f32,
    /// A trap still waiting for an enemy to step in.
    armed: bool,
    /// Flamestrike's burning ground, dealing a share of the nova each second.
    burning: bool,
    /// A Frozen Orb's travel, yards per second.
    vel: V2,
    final_gale: bool,
    whiteout: bool,
    visual: u32,
}

/// Spell state a hero carries through a run, for its augments' mechanics.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroSpells {
    /// An Ice Barrier's shield left, its seconds left, and what it has absorbed.
    pub shield: f32,
    pub shield_time: f32,
    pub soaked: f32,
    /// Fire crits in a row, for Hot Streak.
    pub streak: u8,
    /// Arcane Charges held.
    pub arcane: u8,
    /// Arcane Potency is primed: the next Arcane spell deals triple.
    pub potency: bool,
    /// Blazing Speed's move speed, and seconds before it fades.
    pub haste: f32,
    pub haste_time: f32,
    /// Seconds stood still, for Channelled Focus.
    pub still: f32,
    /// Seconds left of Frozen Core's damage reduction.
    pub guard: f32,
    /// Seconds to Missile Storm's next storm.
    pub storm: f32,
    /// Seconds left of Phoenix Ground's Fire bonus.
    pub phoenix: f32,
    /// Internal cooldowns of augments that cast spells.
    pub icds: Vec<(Augment, f32)>,
    /// Seconds left of a boss's spore's crit bonus, and of a carried bomb's fuse.
    #[serde(default)]
    pub spore: f32,
    #[serde(default)]
    pub bomb: f32,
    /// The other classes' state: wards, buffs, combo points, the defensive button.
    #[serde(default)]
    pub fx: super::classes::HeroFx,
}

impl HeroSpells {
    pub(super) fn ready(&self, a: Augment) -> bool {
        !self.icds.iter().any(|&(x, t)| x == a && t > 0.0)
    }

    pub(super) fn spend(&mut self, a: Augment, secs: f32) {
        match self.icds.iter_mut().find(|(x, _)| *x == a) {
            Some((_, t)) => *t = secs,
            None => self.icds.push((a, secs)),
        }
    }
}

/// A shield's size with its learning card's `bonus`, as a share of max health; upgrades' Shield
/// boosts come on top.
pub fn shield_size(base: f32, bonus: f32) -> f32 {
    base * (1.0 + bonus)
}

/// Whether `pos` lies in an area of `spec` around `center`, pointing `facing` for a cone.
fn in_area(spec: &CastSpec, center: V2, facing: V2, pos: V2) -> bool {
    let off = pos - center;
    let d = off.length();
    if d > spec.radius || d < spec.ring {
        return false;
    }
    if spec.targeting != Targeting::Ahead || d < 0.5 {
        return true;
    }
    let f = facing.normalize_or_zero();
    let side = V2::new(-f.y, f.x);
    let (dirs, half): (&[V2], f32) = match spec.cone {
        Cone::Ahead => (&[f], CONE_HALF_ANGLE),
        Cone::Behind => (&[f * -1.0], CONE_HALF_ANGLE),
        // Four cones half as wide, so together they cover half the circle.
        Cone::Four => (&[f, side, f * -1.0, side * -1.0], CONE_HALF_ANGLE * 0.5),
    };
    dirs.iter()
        .any(|dir| (off.x * dir.x + off.y * dir.y) / d >= half.cos())
}

/// The distance from `p` to the line through `a` toward `b`, and how far along it `p` lies (in
/// yards from `a`).
fn along(a: V2, b: V2, p: V2) -> (f32, f32) {
    let dir = (b - a).normalize_or_zero();
    let off = p - a;
    let t = off.x * dir.x + off.y * dir.y;
    let side = (off - dir * t).length();
    (side, t)
}

impl World {
    /// Whether hero `h` holds augment `a` on any spell.
    pub(super) fn knows(&self, h: usize, a: Augment) -> bool {
        self.heroes[h]
            .abilities
            .iter()
            .any(|o| o.augments.contains(&a))
    }

    /// Whether `player`'s spell `ability` holds augment `a`.
    pub(super) fn spell_has(&self, player: PlayerId, ability: usize, a: Augment) -> bool {
        self.hero(player).is_some_and(|hero| {
            hero.abilities
                .iter()
                .any(|o| o.ability == ability && o.augments.contains(&a))
        })
    }

    fn hero_index(&self, player: PlayerId) -> Option<usize> {
        self.heroes.iter().position(|h| h.player == player)
    }

    /// A cast of `owned` by hero `h`: its rank, the hero's stats, the party's power-ups and its
    /// augments, with the hero-wide capstones' bonuses (Critical Mass, Arcane Charge, Channelled
    /// Focus, Phoenix Ground, Fortified) on top.
    pub fn spell_spec(&self, h: usize, owned: &Owned) -> CastSpec {
        let hero = &self.heroes[h];
        let def = &self.content.abilities[owned.ability];
        let st = self.sharpen(owned.stats(def).with(&hero.stats));
        let mut s = CastSpec::base(owned.ability, def);
        s.damage = st.damage;
        s.cooldown = st.cooldown;
        s.count = st.count;
        s.range = st.range;
        s.radius = st.radius;
        s.crit = CRIT_CHANCE + hero.stats.get(Stat::Crit);
        if hero.spell.spore > 0.0 {
            s.crit += super::hazards::SPORE_CRIT;
        }
        s.crit_mult += hero.stats.get(Stat::CritDamage);
        s.shield = shield_size(def.shield, owned.bonus);
        owned.mods.apply(&mut s);
        for &a in &owned.augments {
            a.shape(&mut s);
        }
        self.gear_shape(h, def, &mut s);
        let sp = &hero.spell;
        if owned.augments.contains(&Augment::ShardBurst) {
            s.vs_frozen += 1.0;
        }
        if s.tags.has(Tag::Fire) && self.knows(h, Augment::CriticalMass) {
            s.crit_mult += 0.5;
        }
        if s.tags.has(Tag::Arcane) && self.knows(h, Augment::ArcaneCharge) {
            s.damage *= 1.0 + 0.1 * f32::from(sp.arcane);
        }
        if s.tags.has(Tag::Fire) && sp.phoenix > 0.0 {
            s.damage *= 1.2;
        }
        if sp.shield > 0.0 && self.knows(h, Augment::Fortified) {
            s.damage *= 1.15;
        }
        if owned.augments.contains(&Augment::ChannelledFocus) && sp.still > 0.0 {
            s.cooldown *= 0.5;
            s.damage *= 1.0 + (0.15 * sp.still).min(0.6);
        }
        self.class_spec(h, owned, &mut s);
        self.talent_spec(h, &mut s);
        s
    }

    /// Every living hero's spells, each fired when its cooldown is up; then the triggers they set
    /// off. Ice Floes runs Frost cooldowns faster while the shield holds.
    pub(super) fn cast_abilities(&mut self) {
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            self.casting = Some(self.heroes[h].player);
            let floes = self.heroes[h].spell.shield > 0.0 && self.knows(h, Augment::IceFloes);
            for a in 0..self.heroes[h].abilities.len() {
                let owned = self.heroes[h].abilities[a].clone();
                let frost = self.content.abilities[owned.ability]
                    .tag_set
                    .has(Tag::Frost)
                    || owned
                        .augments
                        .iter()
                        .any(|x| matches!(x, Augment::Frostfire | Augment::Spellfrost));
                let rate = if floes && frost { 1.0 / 0.75 } else { 1.0 };
                let rate = rate * self.cast_rate(h, &owned);
                let o = &mut self.heroes[h].abilities[a];
                o.cd -= DT * rate;
                if o.cd > 0.0 {
                    continue;
                }
                let mut spec = self.spell_spec(h, &owned);
                let nth = owned.casts;
                if owned.augments.contains(&Augment::GlacialSpike) && nth % 4 == 3 {
                    spec.damage *= 3.0;
                    spec.freeze = spec.freeze.max(FREEZE_SECS);
                }
                self.before_cast(h, &owned, &mut spec);
                let fired = self.fire(h, &spec, Shot::plain());
                let o = &mut self.heroes[h].abilities[a];
                if !fired {
                    // No target in reach: try again next step rather than waste the cooldown.
                    o.cd = 0.0;
                    continue;
                }
                self.after_cast(h, &owned, &spec);
                let o = &mut self.heroes[h].abilities[a];
                o.casts += 1;
                o.burst += 1;
                if o.burst < u32::from(spec.charges) {
                    o.cd = CHARGE_GAP;
                } else {
                    o.burst = 0;
                    o.cd = spec.cooldown;
                }
                let player = self.heroes[h].player;
                if owned.augments.contains(&Augment::PresenceOfMind) && nth % 3 == 2 {
                    self.triggers.push(Trigger {
                        player,
                        ability: owned.ability,
                        shot: Shot {
                            exclude: self.last_target,
                            primary: false,
                            ..Shot::plain()
                        },
                    });
                }
                if owned.augments.contains(&Augment::ArcaneCharge) {
                    let sp = &mut self.heroes[h].spell;
                    sp.arcane = (sp.arcane + 1).min(4);
                }
            }
        }
        self.casting = None;
        self.run_triggers();
    }

    /// Fire one cast of `spec` by hero `h`; false when it had nothing to hit.
    pub(super) fn fire(&mut self, h: usize, spec: &CastSpec, shot: Shot) -> bool {
        let hero = &self.heroes[h];
        let origin = shot.at.unwrap_or(hero.pos);
        let facing = shot.facing.unwrap_or(hero.facing);
        let def = &self.content.abilities[spec.ability];
        let kind = def.kind;
        if spec.fx.skip {
            // A capstone gave the cast up for something else (Demonic Sacrifice).
            return true;
        }
        let mut spec = spec.clone();
        spec.damage *= shot.scale;
        let potent = spec.tags.has(Tag::Arcane) && hero.spell.potency;
        if potent {
            spec.damage *= 3.0;
        }
        let charged = def.key == "arcane_explosion"
            && hero.spell.arcane >= 4
            && self.knows(h, Augment::ArcaneCharge);
        if charged {
            spec.damage *= 2.0;
            spec.radius *= 2.0;
        }
        let fired = match kind {
            AbilityKind::Shield => self.raise_shield(h, &spec),
            AbilityKind::Ward => self.bank_charge(h, &spec),
            AbilityKind::Buff => self.buff(h, &spec),
            AbilityKind::Ground => self.place_ground(h, &spec, origin, facing),
            AbilityKind::Nova => self.nova(h, &spec, origin, facing, shot),
            AbilityKind::Bolt | AbilityKind::Strike => {
                self.launch(h, &spec, origin, shot, kind == AbilityKind::Bolt)
            }
        };
        if !fired {
            return false;
        }
        let sp = &mut self.heroes[h].spell;
        if potent {
            sp.potency = false;
        }
        if charged {
            sp.arcane = 0;
        }
        if shot.primary {
            let player = self.heroes[h].player;
            if spec.echo {
                self.delayed.push((
                    self.elapsed + 1.0,
                    Trigger {
                        player,
                        ability: spec.ability,
                        shot: Shot {
                            scale: shot.scale * 0.5,
                            primary: false,
                            ..shot
                        },
                    },
                ));
            }
            if spec.twin {
                self.fire(
                    h,
                    &spec,
                    Shot {
                        primary: false,
                        ..shot
                    },
                );
            }
        }
        true
    }

    /// The `count` enemies nearest `from` within `range`, elites and bosses first when
    /// `elites_first`, never `exclude`; nearest first.
    pub(super) fn pick_targets(
        &self,
        from: V2,
        range: f32,
        count: usize,
        elites_first: bool,
        exclude: Option<u32>,
    ) -> Vec<(u32, f32)> {
        let mut near: Vec<(u32, f32, bool)> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && Some(e.id) != exclude)
            .map(|e| (e.id, from.distance(e.pos), e.elite || e.boss))
            .filter(|&(_, d, _)| d <= range)
            .collect();
        near.sort_by(|a, b| {
            let rank = |x: &(u32, f32, bool)| elites_first && !x.2;
            rank(a).cmp(&rank(b)).then(a.1.total_cmp(&b.1))
        });
        near.truncate(count);
        near.into_iter().map(|(id, d, _)| (id, d)).collect()
    }

    /// The middle of the thickest crowd within `range` of `from`, judged by how many enemies stand
    /// within `radius` of each; `None` when nobody is in range.
    pub(super) fn densest(&self, from: V2, range: f32, radius: f32) -> Option<V2> {
        let near: Vec<V2> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && from.distance(e.pos) <= range)
            .map(|e| e.pos)
            .collect();
        // Every candidate against every other is plenty for a few hundred; beyond that, sample.
        let step = (near.len() / 120).max(1);
        near.iter()
            .step_by(step)
            .max_by_key(|&&p| near.iter().filter(|&&q| p.distance(q) <= radius).count())
            .copied()
    }

    /// Missiles and strikes: each target gets its volley, landing after the flight.
    fn launch(&mut self, h: usize, spec: &CastSpec, origin: V2, shot: Shot, bolt: bool) -> bool {
        let elites_first = spec.targeting == Targeting::ElitesFirst;
        let targets = if spec.targeting == Targeting::Weakest {
            self.pick_weakest(origin, spec.range, spec.count as usize, shot.exclude)
        } else if spec.fx.fresh {
            self.pick_fresh(h, spec, origin)
        } else if spec.targeting == Targeting::Ahead {
            let facing = shot.facing.unwrap_or(self.heroes[h].facing);
            let mut cone = spec.clone();
            cone.radius = spec.range;
            let mut near: Vec<(u32, f32)> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && in_area(&cone, origin, facing, e.pos))
                .map(|e| (e.id, origin.distance(e.pos)))
                .collect();
            near.sort_by(|a, b| a.1.total_cmp(&b.1));
            near.truncate(spec.count as usize);
            near
        } else {
            self.pick_targets(
                origin,
                spec.range,
                spec.count as usize,
                elites_first,
                shot.exclude,
            )
        };
        if targets.is_empty() {
            return false;
        }
        let player = self.heroes[h].player;
        for &(target, dist) in &targets {
            let flight = if bolt && spec.missile_speed > 0.0 {
                dist / spec.missile_speed
            } else {
                0.0
            };
            for v in 0..spec.volley {
                let lands_at = self.elapsed + flight + v as f32 * VOLLEY_GAP;
                self.pending.push(PendingHit {
                    lands_at,
                    caster: player,
                    spec: spec.clone(),
                    target,
                    from: origin,
                    primary: true,
                });
                if spec.returning {
                    self.pending.push(PendingHit {
                        lands_at: lands_at + flight.max(0.3),
                        caster: player,
                        spec: spec.clone(),
                        target,
                        from: origin,
                        primary: false,
                    });
                }
            }
        }
        self.last_target = targets.first().map(|&(id, _)| id);
        self.events.push(Event::Cast {
            caster: player,
            ability: spec.ability,
            visual: self.content.abilities[spec.ability].visual,
            targets: targets.iter().map(|&(id, _)| Target::Enemy(id)).collect(),
            dest: shot.at,
            cooldown: spec.cooldown,
        });
        true
    }

    /// Missiles and strikes whose flight is over: the hit, its splash and what it pierces.
    pub(super) fn land_hits(&mut self) {
        let now = self.elapsed;
        let (due, flying): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| p.lands_at <= now);
        self.pending = flying;
        for hit in due {
            let Some(at) = self.enemy(hit.target).map(|e| e.pos) else {
                // Its target died while the missile flew.
                continue;
            };
            let crit = self
                .strike(hit.caster, &hit.spec, hit.target, 1.0, hit.from, Crit::Roll)
                .unwrap_or(false);
            if hit.primary && hit.spec.fx.jumps > 0 {
                self.chain(hit.caster, &hit.spec, hit.target, at);
            }
            if hit.spec.radius > 0.0 {
                let burst = if crit
                    && self.spell_has(hit.caster, hit.spec.ability, Augment::CriticalMass)
                {
                    Crit::Always
                } else {
                    Crit::Never
                };
                let around: Vec<u32> = self
                    .enemies
                    .iter()
                    .filter(|e| e.id != hit.target && at.distance(e.pos) <= hit.spec.radius)
                    .map(|e| e.id)
                    .collect();
                for id in around {
                    self.strike(hit.caster, &hit.spec, id, 0.5, at, burst);
                }
            }
            if hit.primary && hit.spec.pierce > 0 {
                let mut past: Vec<(u32, f32)> = self
                    .enemies
                    .iter()
                    .filter(|e| e.id != hit.target)
                    .filter_map(|e| {
                        let (side, t) = along(hit.from, at, e.pos);
                        let beyond = t - hit.from.distance(at);
                        (side <= PIERCE_WIDTH && beyond > 0.0 && beyond <= PIERCE_REACH)
                            .then_some((e.id, beyond))
                    })
                    .collect();
                past.sort_by(|a, b| a.1.total_cmp(&b.1));
                for (id, _) in past.into_iter().take(hit.spec.pierce as usize) {
                    self.strike(hit.caster, &hit.spec, id, 0.7, hit.from, Crit::Roll);
                }
            }
        }
    }

    /// Everything in the area at once: around the caster, at a spot, or in a cone. Implosion
    /// gathers first; Cold Snap and Arcane Potency count what was hit; Flamestrike leaves burning
    /// ground; Firestorm adds two more.
    fn nova(&mut self, h: usize, spec: &CastSpec, origin: V2, facing: V2, shot: Shot) -> bool {
        let player = self.heroes[h].player;
        let mut center = origin;
        if spec.targeted {
            match self.densest(self.heroes[h].pos, 25.0, spec.radius) {
                Some(p) => center = p,
                None => return false,
            }
        }
        let implosion = self.spell_has(player, spec.ability, Augment::Implosion);
        if implosion || spec.fx.pull > 0.0 {
            let reach = if implosion {
                IMPLOSION_REACH
            } else {
                spec.fx.pull
            };
            for e in self.enemies.iter_mut().filter(|e| !e.boss) {
                let off = e.pos - center;
                let d = off.length();
                if d <= reach && d > IMPLOSION_CLOSE {
                    e.pos = center + off.normalize_or_zero() * IMPLOSION_CLOSE;
                }
            }
        }
        self.gear_pull(h, spec, center);
        let mut hit: Vec<(u32, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && in_area(spec, center, facing, e.pos))
            .filter(|e| !spec.fx.only_dotted || e.status.burning())
            .map(|e| (e.id, center.distance(e.pos)))
            .collect();
        if hit.is_empty() {
            return false;
        }
        hit.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.events.push(Event::Cast {
            caster: player,
            ability: spec.ability,
            visual: self.content.abilities[spec.ability].visual,
            targets: hit.iter().map(|&(id, _)| Target::Enemy(id)).collect(),
            dest: Some(center),
            cooldown: spec.cooldown,
        });
        let frozen_cone = self.spell_has(player, spec.ability, Augment::FrozenCone);
        for (i, &(id, _)) in hit.iter().enumerate() {
            if frozen_cone && i < 5 {
                let mut cold = spec.clone();
                cold.freeze = cold.freeze.max(FREEZE_SECS);
                cold.chill = 0.0;
                self.strike(player, &cold, id, 1.0, center, Crit::Roll);
            } else {
                self.strike(player, spec, id, 1.0, center, Crit::Roll);
            }
        }
        let count = hit.len();
        let ids: Vec<u32> = hit.iter().map(|&(id, _)| id).collect();
        self.after_nova(h, spec, center, &ids);
        if count >= 10
            && self.spell_has(player, spec.ability, Augment::ColdSnap)
            && self.heroes[h].spell.ready(Augment::ColdSnap)
        {
            self.heroes[h].spell.spend(Augment::ColdSnap, ICD);
            let content = self.content.clone();
            for o in &mut self.heroes[h].abilities {
                let def = &content.abilities[o.ability];
                if o.ability != spec.ability && def.tag_set.has(Tag::Frost) {
                    o.cd = 0.0;
                }
            }
        }
        if count >= 15 && self.spell_has(player, spec.ability, Augment::ArcanePotency) {
            self.heroes[h].spell.potency = true;
        }
        if spec.ground > 0.0 {
            let visual = self.content.abilities[spec.ability].visual;
            self.grounds.push(Ground {
                by: player,
                spec: spec.clone(),
                pos: center,
                left: spec.ground,
                tick: 1.0 / spec.tick_rate.max(0.1),
                armed: false,
                burning: true,
                vel: V2::ZERO,
                final_gale: false,
                whiteout: false,
                visual,
            });
        }
        if shot.primary && self.spell_has(player, spec.ability, Augment::Firestorm) {
            // Two other crowds: each clear of the first strike and of each other.
            let mut others: Vec<V2> = Vec::new();
            for _ in 0..2 {
                let far: Vec<V2> = self
                    .enemies
                    .iter()
                    .map(|e| e.pos)
                    .filter(|p| p.distance(self.heroes[h].pos) <= 25.0)
                    .filter(|p| {
                        std::iter::once(center)
                            .chain(others.iter().copied())
                            .all(|c| p.distance(c) > spec.radius * 2.0)
                    })
                    .collect();
                if far.is_empty() {
                    break;
                }
                others.push(far[self.rng_pick(far.len())]);
            }
            for at in others {
                self.nova(
                    h,
                    spec,
                    at,
                    facing,
                    Shot {
                        at: Some(at),
                        scale: 0.6,
                        primary: false,
                        ..shot
                    },
                );
            }
        }
        true
    }

    fn rng_pick(&mut self, n: usize) -> usize {
        self.rng.index(n)
    }

    /// A lasting ground area on the thickest crowd in range, or following the caster (Mobile), or
    /// rolling away from it (Frozen Orb).
    fn place_ground(&mut self, h: usize, spec: &CastSpec, origin: V2, facing: V2) -> bool {
        let player = self.heroes[h].player;
        let orb = self.spell_has(player, spec.ability, Augment::FrozenOrb);
        let anyone = self
            .enemies
            .iter()
            .any(|e| origin.distance(e.pos) <= spec.range);
        if !anyone {
            return false;
        }
        let pos = if spec.follows || orb {
            origin
        } else if let Some(p) = self.ground_spot(h, spec, origin) {
            p
        } else {
            match self.densest(origin, spec.range, spec.radius) {
                Some(p) => p,
                None => return false,
            }
        };
        let visual = self.content.abilities[spec.ability].visual;
        self.grounds.push(Ground {
            by: player,
            spec: spec.clone(),
            pos,
            left: if spec.fx.trap {
                Self::trap_wait()
            } else {
                spec.ground
            },
            tick: 0.0,
            armed: spec.fx.trap,
            burning: false,
            vel: if orb {
                facing.normalize_or_zero() * ORB_SPEED
            } else {
                V2::ZERO
            },
            final_gale: self.spell_has(player, spec.ability, Augment::FinalGale),
            whiteout: self.spell_has(player, spec.ability, Augment::Whiteout),
            visual,
        });
        true
    }

    /// A plain lasting area of `spec` by `player` at `pos` for `secs`, hitting what stands in it on
    /// its beat: a capstone's cloud (Toxic Cloud, Smoke Bomb).
    pub(super) fn add_ground(&mut self, player: PlayerId, spec: &CastSpec, pos: V2, secs: f32) {
        let visual = self.content.abilities[spec.ability].visual;
        self.grounds.push(Ground {
            by: player,
            spec: spec.clone(),
            pos,
            left: secs,
            tick: 0.0,
            armed: false,
            burning: false,
            vel: V2::ZERO,
            final_gale: false,
            whiteout: false,
            visual,
        });
    }

    /// An armed trap of `spec` by `player` at `pos`, waiting for an enemy (a capstone's extra
    /// trap).
    pub(super) fn add_trap(&mut self, player: PlayerId, spec: &CastSpec, pos: V2) {
        let visual = self.content.abilities[spec.ability].visual;
        self.grounds.push(Ground {
            by: player,
            spec: spec.clone(),
            pos,
            left: Self::trap_wait(),
            tick: 0.0,
            armed: true,
            burning: false,
            vel: V2::ZERO,
            final_gale: false,
            whiteout: false,
            visual,
        });
    }

    /// A shield on hero `h`, ending any shield it still had.
    fn raise_shield(&mut self, h: usize, spec: &CastSpec) -> bool {
        if self.heroes[h].spell.shield > 0.0 {
            self.end_shield(h);
        }
        let hero = &mut self.heroes[h];
        let secs = self.content.abilities[spec.ability].duration.max(1.0);
        hero.spell.shield = hero.max_hp * spec.shield;
        hero.spell.shield_time = secs;
        hero.spell.soaked = 0.0;
        let player = hero.player;
        self.events.push(Event::Cast {
            caster: player,
            ability: spec.ability,
            visual: self.content.abilities[spec.ability].visual,
            targets: vec![Target::Hero(player)],
            dest: None,
            cooldown: spec.cooldown,
        });
        true
    }

    /// Hero `h`'s shield breaks or runs out: Shattering Barrier's blast and Frost Nova.
    pub(super) fn end_shield(&mut self, h: usize) {
        let soaked = self.heroes[h].spell.soaked;
        self.heroes[h].spell.shield = 0.0;
        self.heroes[h].spell.soaked = 0.0;
        if soaked <= 0.0 || !self.knows(h, Augment::ShatteringBarrier) {
            return;
        }
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        let Some(barrier) = self.content.ability("ice_barrier") else {
            return;
        };
        let near: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.pos.distance(pos) <= BARRIER_BLAST)
            .map(|e| e.id)
            .collect();
        for id in near {
            self.damage_enemy(player, barrier, id, soaked * BARRIER_SHARE, false);
        }
        if let Some(nova) = self.content.ability("frost_nova") {
            self.triggers.push(Trigger {
                player,
                ability: nova,
                shot: Shot {
                    primary: false,
                    ..Shot::plain()
                },
            });
        }
    }

    /// One hit of `spec` on enemy `id`, scaled by `scale`, from `from` (for knockback): its damage
    /// with every bonus, its crit, its status effects and the capstones that act on a hit or a
    /// crit. `None` if the enemy is gone; else whether it crit.
    pub(super) fn strike(
        &mut self,
        by: PlayerId,
        spec: &CastSpec,
        id: u32,
        scale: f32,
        from: V2,
        rule: Crit,
    ) -> Option<bool> {
        let h = self.hero_index(by)?;
        let ei = self.enemies.iter().position(|e| e.id == id)?;
        if self.enemies[ei].hp <= 0.0 {
            return None;
        }
        let has = |a: Augment| {
            self.heroes[h]
                .abilities
                .iter()
                .any(|o| o.ability == spec.ability && o.augments.contains(&a))
        };
        let (shatterbolt, ignition, combustion, winters, bomb, impact, blazing) = (
            has(Augment::Shatterbolt),
            has(Augment::Ignition),
            has(Augment::Combustion),
            has(Augment::WintersChill),
            has(Augment::LivingBomb),
            has(Augment::Impact),
            has(Augment::BlazingSpeed),
        );
        let e = &self.enemies[ei];
        let st = &e.status;
        let frozen = st.freeze > 0.0;
        let mut damage = spec.damage * scale;
        if st.chill > 0.0 {
            damage *= 1.0 + spec.vs_chilled;
        }
        if frozen {
            damage *= 1.0 + spec.vs_frozen;
        }
        if e.hp < spec.fx.execute_at * e.max_hp {
            damage *= 1.0 + spec.execute;
        }
        damage *= self.hit_scale(h, ei, spec);
        if st.exposed > 0.0 {
            damage *= 1.2;
        }
        damage *= self.talent_hit_scale(h, ei, spec);
        let st = &self.enemies[ei].status;
        let mut chance = spec.crit + 0.02 * f32::from(st.winters) + self.class_crit(h, ei, spec);
        if frozen {
            chance += spec.shatter;
            if shatterbolt {
                chance = 1.0;
            }
        }
        if ignition && st.burning() {
            chance = 1.0;
        }
        let crit = match rule {
            Crit::Roll => self.rng.f32() < chance,
            Crit::Always => true,
            Crit::Never => false,
        };
        if crit {
            damage *= spec.crit_mult;
        }
        if combustion {
            let left = self.enemies[ei].status.burn_left();
            self.enemies[ei].status.burns.clear();
            damage += left * 2.0;
        }
        self.enemies[ei].status.last_ability = Some(spec.ability);
        // A damage over time spell that is not Direct has no hit: it all comes over time.
        let direct = spec.tags.has(Tag::Direct) || spec.burn <= 0.0;
        if direct {
            self.damage_enemy(by, spec.ability, id, damage, crit);
        }
        self.afflict(ei, by, spec, damage, from);
        self.afflict_fx(ei, spec, from);
        self.on_strike(h, ei, id, spec, damage, crit, scale);
        let Some(ei) = self.enemies.iter().position(|e| e.id == id) else {
            return Some(crit);
        };
        self.talent_after_hit(h, ei, spec, damage, crit);
        if winters {
            let st = &mut self.enemies[ei].status;
            st.winters = (st.winters + 1).min(5);
            st.winters_time = 6.0;
        }
        if bomb && self.enemies[ei].status.bomb.is_none() {
            self.enemies[ei].status.bomb = Some(Bomb {
                by,
                ability: spec.ability,
                damage: damage * BOMB_SHARE,
                time: BOMB_SECS,
                child: false,
            });
        }
        if impact {
            self.impact(ei, by);
        }
        if blazing {
            let sp = &mut self.heroes[h].spell;
            sp.haste = (sp.haste + 0.03).min(0.3);
            sp.haste_time = 4.0;
            self.refresh(h);
        }
        if spec.tags.has(Tag::Fire) {
            self.fire_crit(h, crit);
        }
        if crit && frozen && shatterbolt {
            let at = self.enemies[ei].pos;
            let around: Vec<u32> = self
                .enemies
                .iter()
                .filter(|x| x.id != id && x.pos.distance(at) <= SHATTERBOLT_RADIUS)
                .map(|x| x.id)
                .collect();
            for other in around {
                self.damage_enemy(by, spec.ability, other, damage * 0.5, false);
            }
        }
        self.on_hit(by, id, damage);
        Some(crit)
    }

    /// The status effects one hit of `spec` leaves on enemy `ei`.
    fn afflict(&mut self, ei: usize, by: PlayerId, spec: &CastSpec, damage: f32, from: V2) {
        let freeze_roll = spec.chill > 0.0 && self.rng.f32() < spec.freeze_chance;
        let dot_leech = self.dot_leech(by, spec);
        let e = &mut self.enemies[ei];
        let (elite, boss) = (e.elite, e.boss);
        let st = &mut e.status;
        if spec.chill > 0.0 {
            st.chill = st
                .chill
                .max(control_secs(spec.chill, elite, boss).max(spec.chill * 0.5));
            st.slow = st.slow.max(spec.slow);
        }
        let freeze = if freeze_roll {
            spec.freeze.max(FREEZE_SECS * 0.5)
        } else {
            spec.freeze
        };
        if freeze > 0.0 {
            st.freeze = st.freeze.max(control_secs(freeze, elite, boss));
            st.frozen_by = Some(by);
            st.permafrost |= spec.permafrost;
        }
        if spec.stun > 0.0 {
            let elite = elite && !spec.fx.full_control;
            st.stun = st.stun.max(control_secs(spec.stun, elite, boss));
        }
        if spec.burn > 0.0 {
            // A faster burn (Quickened) ticks sooner for the same total; a faster tick rate
            // (an upgrade) adds ticks of the same size.
            let speed = spec.dot_speed.max(0.1);
            let secs = spec.fx.dot_time;
            let every = 1.0 / (spec.tick_rate.max(0.1) * speed);
            let burn = Burn {
                by,
                ability: spec.ability,
                dps: damage * spec.burn * spec.dot_mult / secs,
                left: secs * spec.dot_secs / speed,
                tick: every,
                every,
                contagion: spec.contagion,
                tags: spec.tags,
                leech: spec.fx.leech + dot_leech,
            };
            let mine = st
                .burns
                .iter()
                .filter(|b| b.by == by && b.ability == spec.ability)
                .count();
            if mine < usize::from(spec.fx.stacks.max(1)) {
                st.burns.push(burn);
            } else if let Some(old) = st
                .burns
                .iter_mut()
                .filter(|b| b.by == by && b.ability == spec.ability)
                .min_by(|a, b| a.left.total_cmp(&b.left))
            {
                *old = burn;
            }
        }
        if spec.knockback > 0.0 && !boss {
            let push = if elite {
                spec.knockback * 0.5
            } else {
                spec.knockback
            };
            e.pos += (e.pos - from).normalize_or_zero() * push;
        }
    }

    /// Impact: the target is Stunned, and its burns spread to three enemies near it.
    fn impact(&mut self, ei: usize, by: PlayerId) {
        let (pos, id, boss) = (
            self.enemies[ei].pos,
            self.enemies[ei].id,
            self.enemies[ei].boss,
        );
        if !boss {
            self.enemies[ei].status.stun = self.enemies[ei].status.stun.max(1.0);
        }
        let burns: Vec<Burn> = self.enemies[ei]
            .status
            .burns
            .iter()
            .filter(|b| b.by == by && b.left > 0.0)
            .cloned()
            .collect();
        if burns.is_empty() {
            return;
        }
        let mut near: Vec<(usize, f32)> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.id != id && e.hp > 0.0)
            .map(|(i, e)| (i, e.pos.distance(pos)))
            .filter(|&(_, d)| d <= 8.0)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (i, _) in near.into_iter().take(3) {
            for b in &burns {
                let st = &mut self.enemies[i].status;
                match st
                    .burns
                    .iter_mut()
                    .find(|x| x.by == b.by && x.ability == b.ability)
                {
                    Some(old) => *old = b.clone(),
                    None => st.burns.push(b.clone()),
                }
            }
        }
    }

    /// A Fire hit of hero `h`: Hot Streak counts crits in a row, Kindling shortens Pyroblast.
    fn fire_crit(&mut self, h: usize, crit: bool) {
        let pyro = self.content.ability("pyroblast");
        if crit && self.knows(h, Augment::Kindling) {
            if let Some(o) = self.heroes[h]
                .abilities
                .iter_mut()
                .find(|o| Some(o.ability) == pyro)
            {
                o.cd = (o.cd - 0.5).max(0.0);
            }
        }
        if !self.knows(h, Augment::HotStreak) {
            return;
        }
        let sp = &mut self.heroes[h].spell;
        if !crit {
            sp.streak = 0;
            return;
        }
        sp.streak += 1;
        if sp.streak < 2 || !sp.ready(Augment::HotStreak) {
            return;
        }
        sp.streak = 0;
        sp.spend(Augment::HotStreak, 1.0);
        if let Some(pyro) = pyro {
            let player = self.heroes[h].player;
            self.triggers.push(Trigger {
                player,
                ability: pyro,
                shot: Shot {
                    primary: false,
                    ..Shot::plain()
                },
            });
        }
    }

    /// Cast the spells augments asked for, as real casts of the hero's own spell (rank 1 if it does
    /// not know it), ignoring cooldowns; echoes whose second has come join them.
    pub(super) fn run_triggers(&mut self) {
        let now = self.elapsed;
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.delayed)
            .into_iter()
            .partition(|&(at, _)| at <= now);
        self.delayed = later;
        self.triggers.extend(due.into_iter().map(|(_, t)| t));
        let mut ran = 0;
        while !self.triggers.is_empty() && ran < TRIGGERS_PER_STEP {
            let t = self.triggers.remove(0);
            ran += 1;
            let Some(h) = self.hero_index(t.player) else {
                continue;
            };
            if !self.heroes[h].alive {
                continue;
            }
            let owned = self.heroes[h]
                .abilities
                .iter()
                .find(|o| o.ability == t.ability)
                .cloned()
                .unwrap_or_else(|| Owned::new(t.ability, 0.0));
            let spec = self.spell_spec(h, &owned);
            self.fire(h, &spec, t.shot);
        }
        self.triggers.clear();
    }

    /// Each step's spell upkeep on the heroes: standing still, Blazing Speed and Frozen Core
    /// fading, internal cooldowns, the shield running out, and Missile Storm's beat.
    pub(super) fn tend_spells(&mut self) {
        self.tend_heroes_fx();
        for h in 0..self.heroes.len() {
            if !self.heroes[h].alive {
                continue;
            }
            let moving = self.heroes[h].moving;
            let sp = &mut self.heroes[h].spell;
            sp.still = if moving { 0.0 } else { sp.still + DT };
            sp.guard = (sp.guard - DT).max(0.0);
            sp.phoenix = (sp.phoenix - DT).max(0.0);
            for (_, t) in &mut sp.icds {
                *t = (*t - DT).max(0.0);
            }
            let faded = sp.haste > 0.0 && {
                sp.haste_time -= DT;
                sp.haste_time <= 0.0
            };
            if faded {
                sp.haste = 0.0;
                self.refresh(h);
            }
            let sp = &mut self.heroes[h].spell;
            if sp.shield > 0.0 {
                sp.shield_time -= DT;
                if sp.shield_time <= 0.0 {
                    self.end_shield(h);
                }
            }
            if self.knows(h, Augment::MissileStorm) {
                let sp = &mut self.heroes[h].spell;
                sp.storm -= DT;
                if sp.storm <= 0.0 {
                    sp.storm = STORM_EVERY;
                    self.missile_storm(h);
                }
            }
        }
        self.run_triggers();
    }

    /// Missile Storm: one Arcane Missile at every enemy in range.
    fn missile_storm(&mut self, h: usize) {
        let Some(owned) = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.augments.contains(&Augment::MissileStorm))
            .cloned()
        else {
            return;
        };
        let mut spec = self.spell_spec(h, &owned);
        spec.volley = 1;
        spec.count = u32::MAX;
        self.launch(h, &spec, self.heroes[h].pos, Shot::plain(), true);
    }

    /// Each step's status upkeep on the enemies: control effects running out, burns ticking,
    /// Living Bombs going off, Winter's Chill fading.
    pub(super) fn tend_statuses(&mut self) {
        self.tend_status_fx();
        let mut ticks: Vec<(PlayerId, usize, u32, f32, f32, f32)> = Vec::new();
        let mut blasts: Vec<(V2, u32, Bomb)> = Vec::new();
        for e in &mut self.enemies {
            let st = &mut e.status;
            st.chill = (st.chill - DT).max(0.0);
            if st.chill == 0.0 {
                st.slow = 0.0;
            }
            st.stun = (st.stun - DT).max(0.0);
            st.exposed = (st.exposed - DT).max(0.0);
            if st.freeze > 0.0 {
                st.freeze -= DT;
                if st.freeze <= 0.0 {
                    st.freeze = 0.0;
                    if std::mem::take(&mut st.permafrost) {
                        st.chill = st.chill.max(CHILL_SECS);
                    }
                }
            }
            if st.winters > 0 {
                st.winters_time -= DT;
                if st.winters_time <= 0.0 {
                    st.winters = 0;
                }
            }
            // A burn that ran out last step goes now: one that ends with the tick that kills
            // its enemy is still on it when its death is seen (Contagion, Living Wave).
            st.burns.retain(|b| b.left > 0.0);
            for b in &mut st.burns {
                b.tick -= DT;
                if b.tick <= 0.0 {
                    // The last tick deals only the share of a tick it has left.
                    let share = (b.left / b.every).min(1.0);
                    b.tick += b.every;
                    b.left -= b.every;
                    ticks.push((b.by, b.ability, e.id, b.dps * share, b.leech, b.left));
                }
            }
            if let Some(bomb) = &mut st.bomb {
                bomb.time -= DT;
                if bomb.time <= 0.0 {
                    blasts.push((e.pos, e.id, bomb.clone()));
                    st.bomb = None;
                }
            }
        }
        for (by, ability, id, dps, leech, left) in ticks {
            self.damage_enemy(by, ability, id, dps, false);
            if leech > 0.0 {
                if let Some(h) = self.hero_index(by) {
                    self.leech(h, dps * leech, 0.0);
                }
            }
            self.class_tick(by, ability, id, dps, left);
        }
        for (pos, source, bomb) in blasts {
            self.detonate(pos, source, bomb);
        }
    }

    /// A Living Bomb goes off at `pos`: what it hits takes its damage, and becomes a smaller bomb
    /// unless this one was already a child.
    fn detonate(&mut self, pos: V2, source: u32, bomb: Bomb) {
        let hit: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= BOMB_RADIUS)
            .map(|e| e.id)
            .collect();
        self.events.push(Event::Cast {
            caster: bomb.by,
            ability: bomb.ability,
            visual: self.content.abilities[bomb.ability].visual,
            targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
            dest: Some(pos),
            cooldown: 0.0,
        });
        for id in hit {
            self.damage_enemy(bomb.by, bomb.ability, id, bomb.damage, false);
            if bomb.child || id == source {
                continue;
            }
            if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                if e.status.bomb.is_none() {
                    e.status.bomb = Some(Bomb {
                        damage: bomb.damage * 0.5,
                        time: BOMB_SECS,
                        child: true,
                        ..bomb.clone()
                    });
                }
            }
        }
    }

    /// Each step's ground areas: they follow or roll, hit what stands in them every second, mark
    /// it for Whiteout, warm their caster's Phoenix Ground, and end (Final Gale's Frost Nova).
    pub(super) fn tend_grounds(&mut self) {
        let mut i = 0;
        while i < self.grounds.len() {
            let g = self.grounds[i].clone();
            let Some(h) = self.hero_index(g.by) else {
                self.grounds.remove(i);
                continue;
            };
            let mut pos = g.pos + g.vel * DT;
            if g.spec.follows {
                pos = self.heroes[h].pos;
            }
            let pos = self.ground_pos(&g, pos);
            if g.armed {
                // A trap waits for an enemy to step in, then goes off and starts its own clock.
                if self.trap_sprung(&g.spec, pos) {
                    let ground = &mut self.grounds[i];
                    ground.armed = false;
                    ground.left = g.spec.ground.max(DT);
                    ground.tick = 1.0 / g.spec.tick_rate.max(0.1);
                    let hit: Vec<u32> = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= g.spec.radius)
                        .map(|e| e.id)
                        .collect();
                    self.events.push(Event::Cast {
                        caster: g.by,
                        ability: g.spec.ability,
                        visual: g.visual,
                        targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
                        dest: Some(pos),
                        cooldown: 0.0,
                    });
                    self.trap_sprung_fx(&g, pos, &hit);
                    for id in hit {
                        self.strike(g.by, &g.spec, id, 1.0, pos, Crit::Roll);
                    }
                    i += 1;
                    continue;
                }
                self.grounds[i].left -= DT;
                if self.grounds[i].left <= 0.0 {
                    self.grounds.remove(i);
                } else {
                    i += 1;
                }
                continue;
            }
            let tick = g.tick - DT;
            let left = g.left - DT;
            let r = g.spec.radius;
            if g.whiteout {
                for e in self.enemies.iter_mut().filter(|e| e.pos.distance(pos) <= r) {
                    e.status.exposed = 0.25;
                }
            }
            if g.burning
                && self.heroes[h].alive
                && self.heroes[h].pos.distance(pos) <= r
                && self.knows(h, Augment::PhoenixGround)
            {
                let hero = &mut self.heroes[h];
                hero.hp = (hero.hp + hero.max_hp * 0.02 * DT).min(hero.max_hp);
                hero.spell.phoenix = 0.25;
            }
            let ticking = tick <= 0.0;
            {
                let ground = &mut self.grounds[i];
                ground.pos = pos;
                ground.left = left;
                ground.tick = if ticking {
                    tick + 1.0 / g.spec.tick_rate.max(0.1)
                } else {
                    tick
                };
            }
            if ticking && self.ground_beat(g.by, &g.spec, pos) {
                // A summon, totem or healing ground did its own work.
            } else if ticking {
                let hit: Vec<u32> = self
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= r)
                    .map(|e| e.id)
                    .collect();
                self.events.push(Event::Cast {
                    caster: g.by,
                    ability: g.spec.ability,
                    visual: g.visual,
                    targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
                    dest: Some(pos),
                    cooldown: 0.0,
                });
                let scale = if g.spec.fx.trap {
                    crate::world::classes::TRAP_TICK
                } else if g.burning && g.spec.fx.ground_share > 0.0 {
                    g.spec.fx.ground_share
                } else if g.burning {
                    BURNING_GROUND
                } else {
                    1.0
                };
                let mut spec = g.spec.clone();
                if g.burning && !g.spec.fx.ground_control {
                    // The ground burns; it does not chill or freeze again.
                    spec.chill = 0.0;
                    spec.freeze = 0.0;
                    spec.stun = 0.0;
                    spec.knockback = 0.0;
                }
                for id in hit {
                    self.strike(g.by, &spec, id, scale, pos, Crit::Roll);
                }
            }
            if left <= 0.0 {
                self.grounds.remove(i);
                self.ground_ended(&g, pos);
                if g.final_gale {
                    if let Some(nova) = self.content.ability("frost_nova") {
                        self.triggers.push(Trigger {
                            player: g.by,
                            ability: nova,
                            shot: Shot {
                                at: Some(pos),
                                primary: false,
                                ..Shot::plain()
                            },
                        });
                    }
                }
                continue;
            }
            i += 1;
        }
        self.run_triggers();
    }

    /// What a dying enemy's statuses set off: Frostbite's shatter, Contagion's jump, Living
    /// Wave's blast, Inferno's next Fire Blast.
    pub(super) fn spell_deaths(&mut self, deaths: Vec<Death>) {
        self.class_deaths(&deaths);
        let fire_blast = self.content.ability("fire_blast");
        let blast_wave = self.content.ability("blast_wave");
        let nova = self.content.ability("frost_nova");
        for d in deaths {
            if let Some(by) = d.status.frozen_by {
                if let Some(h) = self.hero_index(by) {
                    if self.knows(h, Augment::Frostbite) {
                        if let Some(nova) = nova {
                            self.shatter(by, nova, d.pos, d.max_hp * FROSTBITE_SHARE);
                        }
                    }
                }
            }
            for b in d.status.burns.iter().filter(|b| b.contagion) {
                let next = self
                    .enemies
                    .iter_mut()
                    .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= CONTAGION_REACH)
                    .min_by(|a, b| a.pos.distance(d.pos).total_cmp(&b.pos.distance(d.pos)));
                if let Some(e) = next {
                    if !e
                        .status
                        .burns
                        .iter()
                        .any(|x| x.by == b.by && x.ability == b.ability)
                    {
                        e.status.burns.push(b.clone());
                    }
                }
            }
            let waves: Vec<PlayerId> = d
                .status
                .burns
                .iter()
                .map(|b| b.by)
                .filter(|&by| {
                    self.hero_index(by)
                        .is_some_and(|h| self.knows(h, Augment::LivingWave))
                })
                .collect();
            if let (Some(by), Some(wave)) = (waves.first(), blast_wave) {
                self.triggers.push(Trigger {
                    player: *by,
                    ability: wave,
                    shot: Shot {
                        at: Some(d.pos),
                        scale: 0.5,
                        primary: false,
                        ..Shot::plain()
                    },
                });
            }
            if let (Some(by), Some(fb)) = (d.killer, fire_blast) {
                if d.status.last_ability == Some(fb) && self.spell_has(by, fb, Augment::Inferno) {
                    self.triggers.push(Trigger {
                        player: by,
                        ability: fb,
                        shot: Shot {
                            primary: false,
                            ..Shot::plain()
                        },
                    });
                }
            }
        }
        self.run_triggers();
    }

    /// Frostbite: a Frozen enemy shatters, `damage` to everything within reach of `pos`.
    fn shatter(&mut self, by: PlayerId, ability: usize, pos: V2, damage: f32) {
        let near: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= FROSTBITE_RADIUS)
            .map(|e| e.id)
            .collect();
        if near.is_empty() {
            return;
        }
        self.events.push(Event::Cast {
            caster: by,
            ability,
            visual: self.content.abilities[ability].visual,
            targets: near.iter().map(|&id| Target::Enemy(id)).collect(),
            dest: Some(pos),
            cooldown: 0.0,
        });
        for id in near {
            self.damage_enemy(by, ability, id, damage, false);
        }
    }

    /// A blow at hero `h`, after armor, against its shield: what the shield took, with Reflective
    /// and Frost Armor striking back at `from`; the rest for its health.
    pub(super) fn shield_blow(&mut self, h: usize, amount: f32, from: Option<u32>) -> f32 {
        let sp = &mut self.heroes[h].spell;
        if sp.shield <= 0.0 {
            return amount;
        }
        let taken = amount.min(sp.shield);
        sp.shield -= taken;
        sp.soaked += taken;
        let broke = sp.shield <= 0.0;
        let player = self.heroes[h].player;
        if let (Some(enemy), Some(barrier)) = (from, self.content.ability("ice_barrier")) {
            if self.knows(h, Augment::Reflective) {
                self.damage_enemy(player, barrier, enemy, taken * 0.2, false);
            }
            if self.knows(h, Augment::FrostArmor) {
                self.damage_enemy(player, barrier, enemy, amount * 0.2, false);
                if let Some(e) = self.enemies.iter_mut().find(|e| e.id == enemy) {
                    e.status.chill = e.status.chill.max(CHILL_SECS);
                }
            }
        }
        if broke {
            self.class_shield_broke(h);
            self.end_shield(h);
        }
        amount - taken
    }

    /// Frozen Core: below 30% health, Frost Nova casts itself and blows hurt half as much for a
    /// moment.
    pub(super) fn frozen_core(&mut self, h: usize) {
        let hero = &self.heroes[h];
        if !hero.alive
            || hero.hp >= hero.max_hp * 0.3
            || !hero.spell.ready(Augment::FrozenCore)
            || !self.knows(h, Augment::FrozenCore)
        {
            return;
        }
        let player = hero.player;
        let sp = &mut self.heroes[h].spell;
        sp.spend(Augment::FrozenCore, 15.0);
        sp.guard = 3.0;
        if let Some(nova) = self.content.ability("frost_nova") {
            self.triggers.push(Trigger {
                player,
                ability: nova,
                shot: Shot {
                    primary: false,
                    ..Shot::plain()
                },
            });
        }
    }

    /// A hero's movement ability: Blink Detonation's explosions where it left and landed, Wake of
    /// Winter's cone behind it.
    pub(super) fn dashed(&mut self, h: usize, left: V2, landed: V2) {
        let player = self.heroes[h].player;
        let facing = self.heroes[h].facing;
        if self.knows(h, Augment::BlinkDetonation) {
            if let Some(ae) = self.content.ability("arcane_explosion") {
                for at in [left, landed] {
                    self.triggers.push(Trigger {
                        player,
                        ability: ae,
                        shot: Shot {
                            at: Some(at),
                            primary: false,
                            ..Shot::plain()
                        },
                    });
                }
            }
        }
        if self.knows(h, Augment::WakeOfWinter) {
            if let Some(cone) = self.content.ability("cone_of_cold") {
                self.triggers.push(Trigger {
                    player,
                    ability: cone,
                    shot: Shot {
                        at: Some(landed),
                        facing: Some(facing * -1.0),
                        primary: false,
                        ..Shot::plain()
                    },
                });
            }
        }
    }

    /// The boosts an upgrade card of `rarity` for hero `h`'s spell `ability` gives: as many as the
    /// rarity allows, different, each one the spell's tags make sense of.
    pub(super) fn roll_boosts(
        &mut self,
        h: usize,
        ability: usize,
        rarity: Rarity,
    ) -> [Option<Boost>; 3] {
        let mut out = [None; 3];
        let Some(owned) = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == ability)
            .cloned()
        else {
            return out;
        };
        let spec = self.spell_spec(h, &owned);
        let kind = self.content.abilities[ability].kind;
        let mut pool: Vec<Boost> = Boost::ALL
            .into_iter()
            .filter(|b| b.fits(&spec, kind, rarity))
            .collect();
        for slot in out.iter_mut().take(boost_lines(rarity)) {
            if pool.is_empty() {
                break;
            }
            let i = self.rng.index(pool.len());
            *slot = Some(pool.swap_remove(i));
        }
        out
    }

    /// The augment screen for hero `h`'s spell `ability`, if it waits at a milestone: its three
    /// capstones, or three general augments that fit it, leaning toward the tags the hero's other
    /// spells share.
    pub(super) fn augment_offer(&mut self, h: usize, ability: usize) -> Option<Offer> {
        let owned = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == ability)?
            .clone();
        let key = self.content.abilities[ability].key.clone();
        let capstones = Augment::capstones_of(&key);
        let milestone = owned.awaiting(!capstones.is_empty())?;
        let cards: Vec<Card> = match milestone {
            Milestone::Capstone => capstones
                .into_iter()
                .map(|a| Card {
                    kind: CardKind::Augment(ability, a),
                    rarity: Rarity::Legendary,
                })
                .collect(),
            Milestone::General => {
                let spec = self.spell_spec(h, &owned);
                let others: Tags = self.heroes[h]
                    .abilities
                    .iter()
                    .filter(|o| o.ability != ability)
                    .fold(Tags::default(), |acc, o| {
                        Tags(acc.0 | self.content.abilities[o.ability].tag_set.0)
                    });
                let mut pool: Vec<(Augment, f32)> = Augment::GENERAL
                    .into_iter()
                    .filter(|&a| {
                        a.fits(&spec, &owned.augments)
                            || self.talent_unlocks(h, a, &spec, &owned.augments)
                    })
                    .map(|a| {
                        let weight = 1.0 + 0.5 * a.synergy(others) as f32;
                        (a, weight * self.talent_augment_weight(h, a))
                    })
                    .collect();
                let mut cards = Vec::new();
                while cards.len() < HAND && !pool.is_empty() {
                    let total: f32 = pool.iter().map(|p| p.1).sum();
                    let mut pick = self.rng.f32() * total;
                    let mut i = 0;
                    while i + 1 < pool.len() && pick >= pool[i].1 {
                        pick -= pool[i].1;
                        i += 1;
                    }
                    let (a, _) = pool.swap_remove(i);
                    cards.push(Card {
                        kind: CardKind::Augment(ability, a),
                        rarity: Rarity::Rare,
                    });
                }
                cards
            }
        };
        (!cards.is_empty()).then_some(Offer {
            cards,
            source: Source::Augment(ability),
        })
    }

    /// Queue hero `h`'s augment screens: one for each spell waiting at a milestone that has none
    /// queued yet; `first` puts them before every other hand (a spell just reached its milestone).
    pub(super) fn queue_augments(&mut self, h: usize, first: bool) {
        let waiting: Vec<usize> = self.heroes[h].abilities.iter().map(|o| o.ability).collect();
        for ability in waiting {
            let queued = self.heroes[h]
                .offers
                .iter()
                .any(|o| o.source == Source::Augment(ability));
            if queued {
                continue;
            }
            if let Some(offer) = self.augment_offer(h, ability) {
                if first {
                    self.heroes[h].offers.insert(0, offer);
                } else {
                    self.heroes[h].offers.push(offer);
                }
            }
        }
    }
}

/// How a hit's crit is decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Crit {
    Roll,
    Always,
    Never,
}

/// An enemy as it died, for the mechanics its death sets off.
pub(super) struct Death {
    pub pos: V2,
    pub max_hp: f32,
    pub killer: Option<PlayerId>,
    pub status: Status,
}

/// The slowed speed of a Chilled enemy; an elite or a boss (`tough`) feels half the slow.
pub(super) fn chilled(speed: f32, status: &Status, tough: bool) -> f32 {
    if status.chill > 0.0 {
        let slow = status.slow.max(CHILL_SLOW);
        let slow = if tough {
            slow * crate::spell::TOUGH_SLOW
        } else {
            slow
        };
        speed * (1.0 - slow)
    } else {
        speed
    }
}

#[cfg(test)]
mod augment_checks;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentSources;

    fn shipped() -> Arc<Content> {
        Arc::new(
            Content::from_ron(ContentSources {
                classes: include_str!("../../../../content/classes.ron"),
                abilities: include_str!("../../../../content/abilities.ron"),
                enemies: include_str!("../../../../content/enemies.ron"),
                zones: include_str!("../../../../content/zones.ron"),
            })
            .expect("shipped content is valid"),
        )
    }

    /// A mage alone in an emptied arena: no horde, no shrines, no boss, plenty of health.
    fn mage() -> World {
        let mut w = World::new(shipped(), "elwynn", 5).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.enemies.clear();
        w.interactables.clear();
        w.camps.clear();
        w.pickups.clear();
        w.spawn_timer = f32::MAX;
        w.next_boss = f32::MAX;
        w.heroes[0].max_hp = 1e6;
        w.heroes[0].hp = 1e6;
        w
    }

    fn ability(w: &World, key: &str) -> usize {
        w.content.ability(key).unwrap()
    }

    /// Give the mage exactly these spells, each at `rank` with these augments.
    fn spells(w: &mut World, list: &[(&str, u8, &[Augment])]) {
        w.heroes[0].abilities = list
            .iter()
            .map(|&(key, rank, augments)| Owned {
                level: rank,
                augments: augments.to_vec(),
                cd: 0.0,
                ..Owned::new(ability(w, key), 0.0)
            })
            .collect();
    }

    /// A standing enemy with `hp`, `dx` and `dy` yards from the hero; it does not move or strike
    /// unless asked.
    fn foe(w: &mut World, dx: f32, dy: f32, hp: f32) -> u32 {
        let pos = w.heroes[0].pos + V2::new(dx, dy);
        let id = w.id();
        w.enemies.push(Enemy {
            id,
            def: 0,
            pos,
            facing: V2::new(1.0, 0.0),
            moving: false,
            hp,
            max_hp: hp,
            damage: 0.0,
            speed: 0.0,
            radius: 0.5,
            elite: false,
            boss: false,
            attack_cd: 99.0,
            last_hit: None,
            home: None,
            status: Status::default(),
            act: Default::default(),
        });
        id
    }

    fn run(w: &mut World, secs: f32) -> Vec<Event> {
        let mut seen = Vec::new();
        for _ in 0..(secs / DT) as usize {
            w.step();
            seen.extend(w.drain_events());
        }
        seen
    }

    fn casts_of(events: &[Event], ability: usize) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, Event::Cast { ability: a, .. } if *a == ability))
            .count()
    }

    fn input(pick: Option<u8>, reroll: bool, skip: bool) -> PlayerInput {
        PlayerInput {
            pick,
            reroll,
            skip,
            ..Default::default()
        }
    }

    #[test]
    fn rank_five_opens_a_general_augment_screen_and_hold_keeps_the_spell_waiting() {
        let mut w = mage();
        spells(&mut w, &[("frostbolt", 4, &[])]);
        let bolt = ability(&w, "frostbolt");
        w.take_card(
            0,
            Card {
                kind: CardKind::Upgrade(bolt, [Some(Boost::Damage), None, None]),
                rarity: Rarity::Common,
            },
        );
        assert_eq!(w.heroes[0].abilities[0].level, 5);
        assert!(
            w.heroes[0].abilities[0].mods.damage > 0.0,
            "the boost was not applied"
        );
        let offer = w.heroes[0].offers[0].clone();
        assert_eq!(offer.source, Source::Augment(bolt));
        assert_eq!(offer.cards.len(), 3);
        assert!(offer
            .cards
            .iter()
            .all(|c| matches!(c.kind, CardKind::Augment(_, a) if a.capstone_of().is_none())));
        // Hold: the screen goes, the spell stays at rank 5 and is not offered higher.
        w.set_input(0, input(None, false, true));
        w.step();
        assert!(w.heroes[0].offers.is_empty());
        assert!(!w
            .spells(0)
            .iter()
            .any(|k| matches!(k, CardKind::Upgrade(a, _) if *a == bolt)));
        // The next level-up brings it back, after the level-up's own hand.
        w.xp = xp_to_next(w.level);
        w.set_input(0, PlayerInput::default());
        w.step();
        let sources: Vec<Source> = w.heroes[0].offers.iter().map(|o| o.source).collect();
        assert_eq!(sources, vec![Source::LevelUp, Source::Augment(bolt)]);
    }

    #[test]
    fn rank_ten_offers_the_three_capstones_and_a_reroll_changes_nothing() {
        let mut w = mage();
        spells(&mut w, &[("frostbolt", 9, &[Augment::Precise])]);
        let bolt = ability(&w, "frostbolt");
        w.take_card(
            0,
            Card {
                kind: CardKind::Upgrade(bolt, [None; 3]),
                rarity: Rarity::Common,
            },
        );
        let caps: Vec<Augment> = w.heroes[0].offers[0]
            .cards
            .iter()
            .filter_map(|c| match c.kind {
                CardKind::Augment(_, a) => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(caps, Augment::capstones_of("frostbolt"));
        let rerolls = w.heroes[0].rerolls;
        w.set_input(0, input(None, true, false));
        w.step();
        assert_eq!(
            w.heroes[0].rerolls, rerolls,
            "a capstone screen is not rerolled"
        );
        w.set_input(0, input(Some(1), false, false));
        w.step();
        assert!(w.heroes[0].abilities[0].has(Augment::Shatterbolt));
    }

    #[test]
    fn frost_nova_freezes_and_a_frozen_enemy_stands_still() {
        let mut w = mage();
        spells(&mut w, &[("frost_nova", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.enemies[0].speed = 5.0;
        w.step();
        let e = w.enemy(id).unwrap();
        assert!(e.status.freeze > 0.0, "not frozen");
        let at = e.pos;
        run(&mut w, 1.0);
        assert!(
            w.enemy(id).unwrap().pos.distance(at) < 0.05,
            "a frozen enemy moved"
        );
    }

    #[test]
    fn cone_shapes_point_where_their_augment_says() {
        // The hero faces +x. (augments, enemy offset, hit?)
        let cases: [(&[Augment], f32, f32, bool); 6] = [
            (&[], 4.0, 0.0, true),
            (&[], -4.0, 0.0, false),
            (&[Augment::Reversed], -4.0, 0.0, true),
            (&[Augment::Reversed], 4.0, 0.0, false),
            (&[Augment::FourWinds], 0.0, 4.0, true),
            (&[Augment::FullCircle], 0.0, -4.0, true),
        ];
        for (augments, dx, dy, hit) in cases {
            let mut w = mage();
            spells(&mut w, &[("cone_of_cold", 5, augments)]);
            w.heroes[0].facing = V2::new(1.0, 0.0);
            let id = foe(&mut w, dx, dy, 1e6);
            w.step();
            let struck = w.enemy(id).unwrap().hp < 1e6;
            assert_eq!(struck, hit, "{augments:?} at ({dx}, {dy})");
        }
    }

    #[test]
    fn brute_force_hits_harder_and_freezes_nothing() {
        let mut w = mage();
        spells(&mut w, &[("frost_nova", 5, &[Augment::BruteForce])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.enemy(id).unwrap().status.freeze, 0.0);
    }

    #[test]
    fn frostbolt_seeks_the_elite_and_fire_blast_the_closest_enemy() {
        let mut w = mage();
        spells(&mut w, &[("frostbolt", 1, &[]), ("fire_blast", 1, &[])]);
        let close = foe(&mut w, 4.0, 0.0, 1e6);
        let elite = foe(&mut w, 10.0, 0.0, 1e6);
        w.enemies[1].elite = true;
        let events = run(&mut w, DT * 2.0);
        let target_of = |ability: usize| {
            events.iter().find_map(|e| match e {
                Event::Cast {
                    ability: a,
                    targets,
                    ..
                } if *a == ability => targets.first().copied(),
                _ => None,
            })
        };
        assert_eq!(
            target_of(ability(&w, "frostbolt")),
            Some(Target::Enemy(elite))
        );
        assert_eq!(
            target_of(ability(&w, "fire_blast")),
            Some(Target::Enemy(close))
        );
    }

    #[test]
    fn blizzard_lands_on_the_crowd_and_chills_it_over_time() {
        let mut w = mage();
        spells(&mut w, &[("blizzard", 1, &[])]);
        let ids: Vec<u32> = (0..5)
            .map(|i| foe(&mut w, 12.0 + i as f32 * 0.5, 0.0, 1e6))
            .collect();
        run(&mut w, 2.5);
        for id in ids {
            let e = w.enemy(id).unwrap();
            assert!(
                e.status.chill > 0.0 && e.hp < 1e6,
                "untouched by the blizzard"
            );
        }
    }

    #[test]
    fn frostfire_makes_fireball_chill() {
        let mut w = mage();
        spells(&mut w, &[("fireball", 5, &[Augment::Frostfire])]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        run(&mut w, 1.0);
        assert!(w.enemy(id).unwrap().status.chill > 0.0);
    }

    #[test]
    fn a_tick_rate_upgrade_makes_a_ground_area_hit_more_often() {
        let dealt = |ticks: f32| {
            let mut w = mage();
            spells(&mut w, &[("blizzard", 1, &[])]);
            w.heroes[0].abilities[0].mods.ticks = ticks;
            let id = foe(&mut w, 6.0, 0.0, 1e6);
            run(&mut w, 4.0);
            1e6 - w.enemy(id).unwrap().hp
        };
        let (plain, fast) = (dealt(0.0), dealt(1.0));
        assert!(plain > 0.0 && fast > plain * 1.5, "{plain} vs {fast}");
    }

    #[test]
    fn echo_casts_again_a_second_later() {
        let mut w = mage();
        spells(&mut w, &[("arcane_explosion", 5, &[Augment::Echo])]);
        w.heroes[0].abilities[0].cd = 0.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        let ae = ability(&w, "arcane_explosion");
        let events = run(&mut w, 1.5);
        assert_eq!(casts_of(&events, ae), 2, "one cast and its echo");
    }

    #[test]
    fn hot_streak_casts_pyroblast_after_two_fire_crits() {
        let mut w = mage();
        spells(
            &mut w,
            &[("fireball", 10, &[Augment::Precise, Augment::HotStreak])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 6.0, 0.0, 1e7);
        let pyro = ability(&w, "pyroblast");
        let events = run(&mut w, 6.0);
        assert!(casts_of(&events, pyro) > 0, "no Pyroblast from Hot Streak");
    }

    #[test]
    fn living_bomb_explodes_onto_the_crowd() {
        let mut w = mage();
        spells(
            &mut w,
            &[("fireball", 10, &[Augment::Precise, Augment::LivingBomb])],
        );
        let target = foe(&mut w, 8.0, 0.0, 1e7);
        let bystander = foe(&mut w, 8.0, 6.0, 1e7);
        run(&mut w, 0.8);
        assert!(w.enemy(target).unwrap().status.bomb.is_some(), "no bomb");
        let before = w.enemy(bystander).unwrap().hp;
        run(&mut w, BOMB_SECS + 0.5);
        assert!(
            w.enemy(bystander).unwrap().hp < before,
            "the blast missed the bystander"
        );
    }

    #[test]
    fn ice_barrier_absorbs_and_shattering_barrier_strikes_back_with_frost_nova() {
        let mut w = mage();
        spells(
            &mut w,
            &[
                (
                    "ice_barrier",
                    10,
                    &[Augment::Fortified, Augment::ShatteringBarrier],
                ),
                ("frost_nova", 1, &[]),
            ],
        );
        w.heroes[0].max_hp = 200.0;
        w.heroes[0].hp = 200.0;
        w.heroes[0].abilities[1].cd = 99.0;
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        let shield = w.heroes[0].spell.shield;
        assert!(shield > 0.0, "no shield");
        w.drain_events();
        let took = w.hurt_hero_from(0, shield + 10.0, Some(id));
        assert!(
            (took - 10.0).abs() < 1.0,
            "the shield did not absorb: {took}"
        );
        assert!(w.enemy(id).unwrap().hp < 1e6, "no blast");
        w.run_triggers();
        let nova = ability(&w, "frost_nova");
        assert!(
            casts_of(&w.drain_events(), nova) > 0,
            "no Frost Nova from the barrier"
        );
    }

    #[test]
    fn cold_snap_and_shattering_barrier_cannot_loop_forever() {
        let mut w = mage();
        spells(
            &mut w,
            &[
                ("frost_nova", 10, &[Augment::Wide, Augment::ColdSnap]),
                (
                    "ice_barrier",
                    10,
                    &[Augment::Fortified, Augment::ShatteringBarrier],
                ),
            ],
        );
        w.heroes[0].max_hp = 100.0;
        w.heroes[0].hp = 100.0;
        let ids: Vec<u32> = (0..14)
            .map(|i| foe(&mut w, 2.0 + 0.3 * i as f32, 0.0, 1e9))
            .collect();
        let nova = ability(&w, "frost_nova");
        let mut novas = 0;
        for _ in 0..(10.0 / DT) as usize {
            w.step();
            // A blow every step breaks each new barrier at once.
            let shield = w.heroes[0].spell.shield;
            w.hurt_hero_from(0, shield + 1.0, Some(ids[0]));
            w.heroes[0].hp = 100.0;
            novas += casts_of(&w.drain_events(), nova);
        }
        assert!(novas > 0, "the combo never fired");
        assert!(novas < 200, "{novas} Frost Novas in 10 seconds: a loop");
    }
}
