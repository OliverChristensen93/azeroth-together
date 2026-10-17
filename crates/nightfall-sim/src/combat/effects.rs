//! What abilities do where they land (spec §5.4, §6): damage, with the ambush on an enemy caught
//! unaware of its attacker and a chance to crit; the enemy alerted to whoever hit it (all but Sap
//! and Distract) and the 15-yard noise of the hit and of a death; holds, slows, bleeds and burns;
//! combo points and the finishers that spend them; auto-attack begun; and the moves a hero makes
//! on themselves: Vanish, Sprint, Blink, Distract, eating and drinking, conjuring water. Casts
//! going off and channels ticking ([`super::heroes`]) call [`World::land`]; auto-attack, bleeds
//! and burns call [`World::strike`].
//!
//! Every number that arrives here may come from content or a client, so none is trusted to be
//! finite: a blow of a non-finite amount does nothing, and a hold, bleed, slow, sprint, meal or
//! blink with a non-finite number is not applied (a Blink longer than the map is wide is cut to it), so no health, timer or position goes NaN.

use super::{Burn, Event, Held, Meal, Slowed, UnitRef};
use crate::{Aim, ControlKind, Effect, Loudness, PlayerId, Sound, World, V2, V3};

/// Yards up or down a Blink may end from where it began.
const BLINK_CLIMB: f32 = 3.0;
/// Yards a Blink shortens by, each try, until nothing stands in its way.
const BLINK_STEP: f32 = 0.5;
/// `ClassDef::power` of a class whose power is mana.
pub(crate) const MANA: u8 = 0;

/// A blow of `amount` with the ambush's multiplier on it; one too big for a float stays as big as
/// one can be rather than becoming infinity (which `strike` would drop). A blow that is not a
/// number stays so, for `strike` to drop.
fn boost(amount: f32, by: f32) -> f32 {
    let boosted = amount * by;
    if amount.is_finite() && by.is_finite() && !boosted.is_finite() {
        f32::MAX.copysign(boosted)
    } else {
        boosted
    }
}

/// The most distance there is within `points`: the diagonal of the box around them.
fn diameter(points: &[V2]) -> f32 {
    let (mut lo, mut hi) = (V2::new(f32::MAX, f32::MAX), V2::new(f32::MIN, f32::MIN));
    for p in points {
        lo = V2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = V2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    if points.is_empty() {
        0.0
    } else {
        lo.distance(hi)
    }
}

/// A length of time a timer can run: a number, and above 0.
fn span(secs: f32) -> bool {
    secs.is_finite() && secs > 0.0
}

impl World {
    /// `player`'s ability `ability` lands: on enemy `enemy` (an index into the enemies), on every
    /// enemy within its radius of the hero or of the spot `at`, and on the hero themselves. The
    /// enemies it reached, for the cast's visual.
    pub(crate) fn land(
        &mut self,
        player: PlayerId,
        ability: usize,
        enemy: Option<usize>,
        at: Option<V3>,
    ) -> Vec<UnitRef> {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return Vec::new();
        };
        let centre = match a.aim {
            Aim::Ground => at.filter(|p| p.is_finite()).map(V3::ground),
            Aim::Me | Aim::Enemy => Some(self.heroes[hi].pos),
        };
        let reaches = a.effects.iter().any(Effect::on_enemy);
        let targets: Vec<usize> = match (a.aim, centre) {
            (Aim::Enemy, _) => enemy
                .filter(|&ei| self.enemies.get(ei).is_some_and(|e| e.hp > 0.0))
                .into_iter()
                .collect(),
            (_, Some(c)) if reaches && a.radius > 0.0 => self.enemies_within(c, a.radius),
            _ => Vec::new(),
        };
        // A finisher spends every combo point at once.
        let (combo, combo_on) = if a.needs.combo {
            let f = &mut self.heroes[hi].fight;
            let on = f.combo_on.take();
            (std::mem::take(&mut f.combo), on)
        } else {
            (0, None)
        };
        for &ei in &targets {
            let id = self.enemies[ei].id;
            // Combo points belong to their target: on any other a finisher gets no bonus.
            let combo = if combo_on == Some(id) { combo } else { 0 };
            self.land_on(player, ability, ei, combo);
        }
        for effect in &a.effects {
            self.land_self(player, effect, centre, a.radius);
        }
        targets
            .iter()
            .map(|&ei| UnitRef::Enemy(self.enemies[ei].id))
            .collect()
    }

    /// Ability `ability` lands on enemy `ei`, a finisher's `combo` points behind it. The ambush is
    /// asked once, before anything lands, and multiplies every damage and weapon effect of the
    /// ability: one ability is one strike, so an opener's every blow is a blow from the shadows.
    fn land_on(&mut self, player: PlayerId, ability: usize, ei: usize, combo: u8) {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if self.enemies[ei].hp <= 0.0 {
            return;
        }
        let id = self.enemies[ei].id;
        // Asked before the blow, which alerts it: only each hero's first blow is an ambush.
        let ambush = self.ambush(id, player);
        let silent = a.sound == Sound::Silent;
        for effect in &a.effects {
            match *effect {
                Effect::Damage {
                    min,
                    max,
                    per_combo,
                } => {
                    let amount = self.combat.dice.between(min, max) + per_combo * f32::from(combo);
                    self.strike(
                        player,
                        ei,
                        boost(amount, ambush),
                        ambush > 1.0,
                        silent,
                        true,
                    );
                }
                Effect::Weapon { percent, bonus } => {
                    let Some(blow) = content.classes[self.heroes[hi].class].swing else {
                        continue;
                    };
                    let amount =
                        self.combat.dice.between(blow.min, blow.max) * percent / 100.0 + bonus;
                    self.strike(
                        player,
                        ei,
                        boost(amount, ambush),
                        ambush > 1.0,
                        silent,
                        true,
                    );
                }
                Effect::Dot {
                    amount,
                    every,
                    secs,
                } => {
                    if !(amount.is_finite() && span(every) && span(secs)) {
                        continue;
                    }
                    let burns = &mut self.combat.enemies[ei].burns;
                    burns.retain(|b| !(b.by == player && b.spell == a.spell));
                    burns.push(Burn {
                        by: player,
                        amount,
                        every,
                        next: every,
                        left: secs,
                        spell: a.spell,
                        silent,
                    });
                }
                Effect::Slow { by, secs } => {
                    if !(by.is_finite() && span(secs)) {
                        continue;
                    }
                    self.combat.enemies[ei].slow = Some(Slowed {
                        by,
                        left: secs,
                        spell: a.spell,
                    });
                }
                Effect::Control { kind, secs } => {
                    if !span(secs) {
                        continue;
                    }
                    let here = self.enemies[ei].pos;
                    let ec = &mut self.combat.enemies[ei];
                    ec.held = Some(Held {
                        kind,
                        left: secs,
                        by: player,
                        spell: a.spell,
                    });
                    ec.wander = (kind == ControlKind::Polymorph).then_some((here, here));
                    // Its caster stops swinging at what a blow would free.
                    if kind.freed_by_damage() && self.heroes[hi].fight.attacking == Some(id) {
                        self.heroes[hi].fight.attacking = None;
                    }
                }
                Effect::Combo(n) => {
                    let max = content.rules.combat.combo_max;
                    let f = &mut self.heroes[hi].fight;
                    if f.combo_on != Some(id) {
                        f.combo = 0;
                    }
                    f.combo = f.combo.saturating_add(n).min(max);
                    f.combo_on = Some(id);
                }
                _ => {}
            }
            if self.enemies[ei].hp <= 0.0 {
                return;
            }
        }
        if a.alerts {
            self.alert(id, player);
        }
        if a.attack && content.classes[self.heroes[hi].class].swing.is_some() {
            self.heroes[hi].fight.attacking = Some(id);
        }
    }

    /// `by`'s blow of `amount` on enemy `ei`: a crit when `may_crit` and the dice say so; a blow
    /// frees what damage frees; it keeps `by` in combat; it is heard 15 yards off unless
    /// `silent`; and at 0 health the enemy dies. An `amount` that is not finite does nothing.
    pub(crate) fn strike(
        &mut self,
        by: PlayerId,
        ei: usize,
        amount: f32,
        ambush: bool,
        silent: bool,
        may_crit: bool,
    ) {
        // A blow that is not a number, or does no harm, is no blow: no event, no noise, nothing
        // freed, no combat. Nor is a blow on one already dead.
        if !amount.is_finite() || amount <= 0.0 || self.enemies.get(ei).is_none_or(|e| e.hp <= 0.0)
        {
            return;
        }
        let (chance, times, linger) = {
            let r = &self.content.rules.combat;
            (r.crit_chance, r.crit_times, r.combat_linger)
        };
        let crit = may_crit && self.combat.dice.chance(chance);
        // A crit of a huge blow may overflow: it is as deadly as any, and no more.
        let amount = (if crit { amount * times } else { amount }).min(f32::MAX);
        let e = &mut self.enemies[ei];
        // The killing blow leaves the enemy's health to `kill_enemy`, which ends it once.
        let dead = amount >= e.hp;
        if !dead {
            e.hp -= amount;
        }
        let (id, at) = (e.id, V3::at(e.pos, e.z));
        self.events.push(Event::Damage {
            to: UnitRef::Enemy(id),
            by: Some(UnitRef::Hero(by)),
            amount,
            crit,
            ambush,
        });
        let ec = &mut self.combat.enemies[ei];
        if ec.held.is_some_and(|h| h.kind.freed_by_damage()) {
            ec.held = None;
            ec.wander = None;
        }
        if self.hero_slot(by).is_some() {
            let ci = self.combat.hero_index(by);
            self.combat.heroes[ci].combat_for = linger;
        }
        if !silent {
            self.make_noise(by, at, Loudness::Hit);
        }
        if dead {
            self.kill_enemy(ei, Some(by));
        }
    }

    /// Enemy `ei` dies (by `by`'s hand, when a hero's): what held it and what it was about to do
    /// go, its senses go quiet (no "!" over a corpse, no shout it raised), heroes stop swinging at
    /// it and lose their combo points on it, and the death is heard 15 yards off. Its body stays
    /// where it fell, for sub-project 5 to search. An enemy already dead (or none) is not killed
    /// twice: one death, one `EnemyDied`.
    pub(crate) fn kill_enemy(&mut self, ei: usize, by: Option<PlayerId>) {
        if self.enemies.get(ei).is_none_or(|e| e.hp <= 0.0) {
            return;
        }
        let e = &mut self.enemies[ei];
        e.hp = 0.0;
        e.speed = 0.0;
        e.asleep = false;
        let (id, at) = (e.id, V3::at(e.pos, e.z));
        self.senses.clear_enemy(ei);
        let ec = &mut self.combat.enemies[ei];
        ec.held = None;
        ec.slow = None;
        ec.burns.clear();
        ec.cast = None;
        ec.distracted = None;
        ec.wander = None;
        self.telegraphs.retain(|t| t.enemy != id);
        for h in &mut self.heroes {
            if h.fight.attacking == Some(id) {
                h.fight.attacking = None;
            }
            if h.fight.combo_on == Some(id) {
                h.fight.combo = 0;
                h.fight.combo_on = None;
            }
        }
        self.events.push(Event::EnemyDied { id, by });
        if let Some(by) = by {
            self.make_noise(by, at, Loudness::Hit);
        }
    }

    /// What `effect` does to `player`'s hero themselves; `spot` is where a ground ability went,
    /// `radius` how far around it reaches.
    fn land_self(&mut self, player: PlayerId, effect: &Effect, spot: Option<V2>, radius: f32) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let content = self.content.clone();
        let ci = self.combat.hero_index(player);
        match *effect {
            Effect::Vanish => {
                self.heroes[hi].fight.attacking = None;
                self.combat.heroes[ci].combat_for = 0.0;
                self.senses.lose(player, &content.rules.senses.awareness);
                self.enter_stealth(player);
            }
            Effect::Sprint { by, secs } => {
                if !(by.is_finite() && span(secs)) {
                    return;
                }
                self.heroes[hi].fight.sprint = secs;
                self.combat.heroes[ci].sprint_by = by;
            }
            Effect::Blink { distance } => {
                let to = self.blink_to(hi, distance);
                self.events.push(Event::Blinked { player, to });
            }
            Effect::Distract { secs } => {
                let Some(spot) = spot else {
                    return;
                };
                if !span(secs) {
                    return;
                }
                for ei in self.enemies_within(spot, radius) {
                    if !self.senses.alerted(ei) {
                        self.combat.enemies[ei].distracted = Some((spot, secs));
                    }
                }
            }
            Effect::Eat { secs } => {
                if !span(secs) {
                    return;
                }
                let hero = &mut self.heroes[hi];
                let drinks = content.classes[hero.class].power == MANA && hero.fight.drinks > 0;
                if drinks {
                    hero.fight.drinks -= 1;
                }
                hero.fight.eating = secs;
                let mana = if drinks {
                    hero.fight.max_power / secs
                } else {
                    0.0
                };
                self.combat.heroes[ci].meal = Some(Meal {
                    health: hero.max_hp / secs,
                    mana,
                });
            }
            Effect::ConjureWater { count } => {
                let f = &mut self.heroes[hi].fight;
                f.drinks = f
                    .drinks
                    .saturating_add(count)
                    .min(content.rules.combat.max_drinks);
            }
            _ => {}
        }
    }

    /// Where a Blink of `distance` yards ends for hero `hi`: straight the way they face, shortened
    /// until no wall stands between and a floor lies near their height; where they stand when no
    /// such spot is found (or the distance or their facing is not a number). Without a map (a
    /// mirror) nothing stands in the way.
    fn blink_to(&self, hi: usize, distance: f32) -> V3 {
        let h = &self.heroes[hi];
        let from = V3::at(h.pos, h.z);
        let way = h.facing.normalize_or_zero();
        if !(distance.is_finite() && way.is_finite() && from.is_finite()) {
            return from;
        }
        // No Blink goes further than the map is wide: a huge distance is not millions of tries.
        let mut d = distance.min(diameter(self.content.map.bounds.points()));
        while d > 0.0 {
            let to = self.content.map.bounds.clamp(h.pos + way * d);
            match self.map() {
                None => return V3::at(to, h.z),
                Some(map) => {
                    if let Some(z) = map.floor_within(to, h.z, BLINK_CLIMB) {
                        let end = V3::at(to, z);
                        if map.walk_line(from, end) {
                            return end;
                        }
                    }
                }
            }
            d -= BLINK_STEP;
        }
        from
    }
}
