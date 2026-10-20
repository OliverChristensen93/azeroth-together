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
use crate::raid::drinks_in;
use crate::{Aim, ControlKind, Effect, ItemKind, Loudness, PlayerId, Sound, Stack, World, V2, V3};

/// Yards up or down a Blink may end from where it began.
const BLINK_CLIMB: f32 = 3.0;
/// Yards a Blink shortens by, each try, until nothing stands in its way.
const BLINK_STEP: f32 = 0.5;
/// `ClassDef::power` of a class whose power is mana.
pub(crate) const MANA: u8 = 0;

/// A blow of `amount` with the ambush's multiplier on it; one too big for a float stays as big as
/// one can be rather than becoming infinity (which `strike` would drop). A blow that is not a
/// number stays so, for `strike` to drop.
pub(super) fn boost(amount: f32, by: f32) -> f32 {
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
pub(super) fn span(secs: f32) -> bool {
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
        // Area abilities reach the hostile heroes in them too (spec §7.4), those it has a clear
        // line to as an enemy's area blow has.
        let foes: Vec<PlayerId> = match (a.aim, centre) {
            (Aim::Enemy, _) => Vec::new(),
            (_, Some(c)) if reaches && a.radius > 0.0 => self
                .foes_within(player, c, a.radius)
                .into_iter()
                .filter(|&p| self.foe_in_sight(hi, Some(p)))
                .collect(),
            _ => Vec::new(),
        };
        // A finisher spends every combo point at once.
        let (combo, combo_on, combo_hero) = if a.needs.combo {
            let f = &mut self.heroes[hi].fight;
            let on = f.combo_on.take();
            let on_hero = f.combo_hero.take();
            (std::mem::take(&mut f.combo), on, on_hero)
        } else {
            (0, None, None)
        };
        for &ei in &targets {
            let id = self.enemies[ei].id;
            // Combo points belong to their target: on any other a finisher gets no bonus.
            let combo = if combo_on == Some(id) { combo } else { 0 };
            self.land_on(player, ability, ei, combo);
        }
        for &p in &foes {
            // Points on another unit give a finisher no bonus here either.
            let combo = if combo_hero == Some(p) { combo } else { 0 };
            self.land_foe(player, ability, p, combo);
        }
        for effect in &a.effects {
            self.land_self(player, effect, centre, a.radius);
        }
        let mut hits: Vec<UnitRef> = targets
            .iter()
            .map(|&ei| UnitRef::Enemy(self.enemies[ei].id))
            .collect();
        hits.extend(foes.into_iter().map(UnitRef::Hero));
        hits
    }

    /// Ability `ability` lands on enemy `ei`, a finisher's `combo` points behind it. The ambush is
    /// asked once, before anything lands, and multiplies every damage and weapon effect of the
    /// ability: one ability is one strike, so an opener's every blow is a blow from the shadows.
    pub(crate) fn land_on(&mut self, player: PlayerId, ability: usize, ei: usize, combo: u8) {
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
                    f.combo_hero = None;
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
            // One target at a time: a hero they were swinging at is let be.
            let ci = self.combat.hero_index(player);
            self.combat.heroes[ci].swing_at = None;
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
        // Worn gear's share of damage (spec §6.1: power comes from gear). A blow that is not a
        // number stays as it is, for the check below to refuse; one the multiplier would push
        // past what a float holds is as deadly as any.
        let amount = if amount.is_finite() {
            (amount * self.gear_damage(by)).min(f32::MAX)
        } else {
            amount
        };
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
        self.telegraphs.retain(|t| t.by.is_some() || t.enemy != id);
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
    pub(crate) fn land_self(&mut self, player: PlayerId, effect: &Effect, spot: Option<V2>, radius: f32) {
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
                // A rival's body is the host's to move: its bot walks on from there.
                if self.heroes[hi].side.is_rival() {
                    let h = &mut self.heroes[hi];
                    h.pos = to.ground();
                    h.z = to.z;
                }
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
                // Food and drink are items (spec §8.2): one of each from the backpack, the drink
                // only for a mana user; with `free_meal`, health comes back without food. The
                // count shown follows at once.
                let mana_user = content.classes[self.heroes[hi].class].power == MANA;
                let food = self.raid.eat(player, ItemKind::Food, &content);
                let drink = if mana_user {
                    self.raid.eat(player, ItemKind::Drink, &content)
                } else {
                    None
                };
                let health = food
                    .or(content.rules.run.free_meal.then_some(1.0))
                    .unwrap_or(0.0);
                let hero = &mut self.heroes[hi];
                hero.fight.eating = secs;
                self.combat.heroes[ci].meal = Some(Meal {
                    health: hero.max_hp * health / secs,
                    mana: hero.fight.max_power * drink.unwrap_or(0.0) / secs,
                });
                self.refresh_carry(player);
            }
            Effect::ConjureWater { count } => {
                // Conjured drinks are items in the backpack (found: they were made here), so
                // that the drinks in the backpack, of any kind, come to at most `max_drinks`.
                // What does not fit a full backpack is put on the ground at the hero's feet, a
                // pile to pick up, so the spell's mana always buys real drinks.
                if let Some(drink) = content.conjured {
                    let have = self.raid.pack(player).map_or(0, |i| drinks_in(i, &content));
                    let room = u32::from(content.rules.combat.max_drinks).saturating_sub(have);
                    let n = u32::from(count).min(room) as u16;
                    if n > 0 {
                        let left = self
                            .raid
                            .pack_mut(player, &content)
                            .add(Stack::found(drink, n), &content);
                        if let Some(mut left) = left {
                            let here = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
                            let most = content.items[drink].stack.max(1);
                            while left.count > 0 {
                                let part = left.count.min(most);
                                left.count -= part;
                                let pile = Stack {
                                    count: part,
                                    ..left
                                };
                                self.raid.drop_on_ground(here, pile, &content);
                            }
                        }
                    }
                    self.refresh_carry(player);
                }
            }
            Effect::Heal { min, max } => {
                let amount = self.combat.dice.between(min, max);
                let h = &mut self.heroes[hi];
                let before = h.hp;
                h.hp = (h.hp + amount).min(h.max_hp);
                let healed = h.hp - before;
                if healed > 0.0 {
                    self.events.push(Event::Healed {
                        player,
                        amount: healed,
                    });
                }
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
