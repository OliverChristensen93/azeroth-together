//! Heroes against hostile heroes (spec §7.4): a rival is a hero of another side, so a hero's
//! ability may be aimed at a rival and a rival's at a hero. This is the fight's arm for a hero
//! target, the same abilities with the same numbers as on an enemy: the press's checks; what
//! each effect does to a hero (damage, bleeds, slows, holds, combo points); area abilities
//! reaching the hostile heroes in them; auto-attack at one; and the holds, slows and bleeds on
//! heroes running down. A hold on a hero roots them and, unless it is a root, keeps them from
//! pressing; a hero is never turned into a sheep (a sheep on a client-owned body would need its
//! own design), only held.
//!
//! Every number is as untrusted as in [`super::effects`]: a blow that is not a number does
//! nothing, and a bleed, slow or hold with a non-finite number is not laid on.

use super::effects::{boost, span};
use super::heroes::{Aimed, MELEE};
use super::{in_range, stands_right, Burn, Event, Held, Slowed, Stop, Telegraph, UnitRef};
use crate::{
    Creature, Effect, Facing, Loudness, PlayerId, Refusal, Shape, Sound, TelegraphDef, World,
    CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3,
};

impl World {
    /// The players of `player`'s hostile heroes up within `radius` of `at`, in the world's order.
    pub(crate) fn foes_within(&self, player: PlayerId, at: V2, radius: f32) -> Vec<PlayerId> {
        self.heroes
            .iter()
            .filter(|h| h.fight.up() && h.pos.distance(at) <= radius)
            .filter(|h| self.hostile(player, h.player))
            .map(|h| h.player)
            .collect()
    }

    /// Whether hero `hi` may begin `ability` at hostile hero `foe`, after the checks every press
    /// has passed: a hostile hero who is up and a humanoid where the ability wants one, combo
    /// points on them for a finisher, unaware of the caster for an opener, in range, the caster
    /// standing right, and a clear line.
    pub(crate) fn check_foe(
        &self,
        hi: usize,
        ability: usize,
        foe: PlayerId,
    ) -> Result<Aimed, Refusal> {
        let content = &self.content;
        let r = &content.rules.combat;
        let a = &content.abilities[ability];
        let hero = &self.heroes[hi];
        if !self.hostile(hero.player, foe) {
            return Err(Refusal::BadTarget);
        }
        let fi = self.hero_slot(foe).ok_or(Refusal::NoTarget)?;
        let t = &self.heroes[fi];
        let takes = a.needs.creatures.is_empty() || a.needs.creatures.contains(&Creature::Humanoid);
        if !t.fight.up() || !takes {
            return Err(Refusal::BadTarget);
        }
        if a.needs.combo && hero.fight.combo_hero != Some(foe) {
            return Err(Refusal::NeedCombo);
        }
        if a.needs.unaware && !self.foe_unaware(foe, hero.player) {
            return Err(Refusal::Aware);
        }
        if !in_range(hero.pos.distance(t.pos), a.range, r) {
            return Err(Refusal::OutOfRange);
        }
        if !stands_right(a.facing, hero.pos, hero.facing, t.pos, t.facing, r.front) {
            return Err(if a.facing == Facing::Behind {
                Refusal::Behind
            } else {
                Refusal::Facing
            });
        }
        if !self.foe_in_sight(hi, Some(foe)) {
            return Err(Refusal::NotInSight);
        }
        Ok(Aimed {
            ability,
            enemy: None,
            foe: Some(foe),
            at: None,
        })
    }

    /// Whether hero `hi` has a clear line to hero `foe`, at every range, as to an enemy (a blow
    /// through a wall is no blow). No foe needs none.
    pub(crate) fn foe_in_sight(&self, hi: usize, foe: Option<PlayerId>) -> bool {
        let Some(t) = foe.and_then(|p| self.hero(p)) else {
            return true;
        };
        let h = &self.heroes[hi];
        self.clear_line(
            V3::at(h.pos, h.z + EYE_HEIGHT),
            V3::at(t.pos, t.z + CHEST_HEIGHT),
        )
    }

    /// Whether hero `target` is not aware of `by`: for now, not in combat (Task 7 asks a rival's
    /// mind instead).
    pub(crate) fn foe_unaware(&self, target: PlayerId, _by: PlayerId) -> bool {
        self.hero(target).is_some_and(|t| !t.fight.in_combat)
    }

    /// The ambush multiplier on a blow from `by` at hero `target` (spec §5.4): the senses'
    /// `ambush` while the target is unaware of them, else 1.
    pub(crate) fn foe_ambush(&self, target: PlayerId, by: PlayerId) -> f32 {
        if self.foe_unaware(target, by) {
            self.content.rules.senses.ambush
        } else {
            1.0
        }
    }

    /// `player`'s ability `ability`, aimed at hostile hero `foe`, goes off: a finisher spends its
    /// points, it lands on the foe, and its effects on the caster land. What it hit.
    pub(crate) fn land_at_foe(
        &mut self,
        player: PlayerId,
        ability: usize,
        foe: PlayerId,
    ) -> Vec<UnitRef> {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return Vec::new();
        };
        let combo = if a.needs.combo {
            let f = &mut self.heroes[hi].fight;
            f.combo_on = None;
            let on = f.combo_hero.take();
            let points = std::mem::take(&mut f.combo);
            // Combo points belong to their target: on any other a finisher gets no bonus.
            if on == Some(foe) {
                points
            } else {
                0
            }
        } else {
            0
        };
        let centre = Some(self.heroes[hi].pos);
        self.land_foe(player, ability, foe, combo);
        for effect in &a.effects {
            self.land_self(player, effect, centre, a.radius);
        }
        vec![UnitRef::Hero(foe)]
    }

    /// Ability `ability` of `player` lands on hostile hero `foe`, a finisher's `combo` points
    /// behind it: the same effects as on an enemy, and the blow frees what damage frees.
    pub(crate) fn land_foe(&mut self, player: PlayerId, ability: usize, foe: PlayerId, combo: u8) {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if !self.hero(foe).is_some_and(|h| h.fight.up()) {
            return;
        }
        // Asked before the blow, which alerts it: only the first blow is an ambush.
        let ambush = self.foe_ambush(foe, player);
        let silent = a.sound == Sound::Silent;
        for effect in &a.effects {
            match *effect {
                Effect::Damage {
                    min,
                    max,
                    per_combo,
                } => {
                    let amount = self.combat.dice.between(min, max) + per_combo * f32::from(combo);
                    self.strike_foe(
                        player,
                        foe,
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
                    self.strike_foe(
                        player,
                        foe,
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
                    let ci = self.combat.hero_index(foe);
                    let burns = &mut self.combat.heroes[ci].burns;
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
                    let ci = self.combat.hero_index(foe);
                    self.combat.heroes[ci].slow = Some(Slowed {
                        by,
                        left: secs,
                        spell: a.spell,
                    });
                }
                Effect::Control { kind, secs } => {
                    if !span(secs) {
                        continue;
                    }
                    let ci = self.combat.hero_index(foe);
                    self.combat.heroes[ci].held = Some(Held {
                        kind,
                        left: secs,
                        by: player,
                        spell: a.spell,
                    });
                    if let Some(fi) = self.hero_slot(foe) {
                        self.heroes[fi].fight.held = Some(kind);
                        if kind.out() {
                            self.stop_cast(fi, Stop::Interrupted);
                        }
                    }
                    // Its caster stops swinging at what a blow would free.
                    let pi = self.combat.hero_index(player);
                    if kind.freed_by_damage() && self.combat.heroes[pi].swing_at == Some(foe) {
                        self.combat.heroes[pi].swing_at = None;
                    }
                }
                Effect::Combo(n) => {
                    let max = content.rules.combat.combo_max;
                    let f = &mut self.heroes[hi].fight;
                    if f.combo_hero != Some(foe) {
                        f.combo = 0;
                    }
                    f.combo = f.combo.saturating_add(n).min(max);
                    f.combo_hero = Some(foe);
                    f.combo_on = None;
                }
                _ => {}
            }
            if !self.hero(foe).is_some_and(|h| h.fight.up()) {
                return;
            }
        }
        if a.attack && content.classes[self.heroes[hi].class].swing.is_some() {
            // One target at a time: the enemy they were swinging at is let be.
            self.heroes[hi].fight.attacking = None;
            let pi = self.combat.hero_index(player);
            self.combat.heroes[pi].swing_at = Some(foe);
        }
    }

    /// `by`'s blow of `amount` on hero `foe`: worn gear's share on it, a crit when `may_crit`
    /// and the dice say so; a blow frees what damage frees; it keeps `by` in combat; it is heard
    /// 15 yards off unless `silent`; and it wounds. An `amount` that is not finite does nothing.
    pub(crate) fn strike_foe(
        &mut self,
        by: PlayerId,
        foe: PlayerId,
        amount: f32,
        ambush: bool,
        silent: bool,
        may_crit: bool,
    ) {
        // As `strike` has it: gear's share, never past what a float holds; a blow that is not a
        // number stays so, for the check below.
        let amount = if amount.is_finite() {
            (amount * self.gear_damage(by)).min(f32::MAX)
        } else {
            amount
        };
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let Some(fi) = self.hero_slot(foe).filter(|&fi| self.heroes[fi].fight.up()) else {
            return;
        };
        let (chance, times, linger) = {
            let r = &self.content.rules.combat;
            (r.crit_chance, r.crit_times, r.combat_linger)
        };
        let crit = may_crit && self.combat.dice.chance(chance);
        let amount = (if crit { amount * times } else { amount }).min(f32::MAX);
        let at = V3::at(self.heroes[fi].pos, self.heroes[fi].z);
        let ci = self.combat.hero_index(foe);
        if self.combat.heroes[ci]
            .held
            .is_some_and(|h| h.kind.freed_by_damage())
        {
            self.combat.heroes[ci].held = None;
            self.heroes[fi].fight.held = None;
        }
        if self.hero_slot(by).is_some() {
            let bi = self.combat.hero_index(by);
            self.combat.heroes[bi].combat_for = linger;
        }
        if !silent {
            self.make_noise(by, at, Loudness::Hit);
        }
        self.wound_by(foe, amount, Some(UnitRef::Hero(by)), crit, ambush);
    }

    /// Hero `hi`'s holds, slow and bleeds run down a step; a bleed's tick strikes. The fallen
    /// lose them all.
    pub(crate) fn tick_foe_holds(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let up = self.heroes[hi].fight.up();
        let hc = &mut self.combat.heroes[ci];
        if !up {
            hc.held = None;
            hc.slow = None;
            hc.burns.clear();
            hc.swing_at = None;
            self.heroes[hi].fight.held = None;
            return;
        }
        if let Some(h) = &mut hc.held {
            h.left -= DT;
        }
        hc.held = hc.held.filter(|h| h.left > 1e-4);
        if let Some(s) = &mut hc.slow {
            s.left -= DT;
        }
        hc.slow = hc.slow.filter(|s| s.left > 1e-4);
        let mut ticks: Vec<(PlayerId, f32, bool)> = Vec::new();
        for b in &mut hc.burns {
            b.left -= DT;
            b.next -= DT;
            if b.next <= 1e-4 {
                b.next += b.every;
                ticks.push((b.by, b.amount, b.silent));
            }
        }
        hc.burns.retain(|b| b.left > 1e-4);
        self.heroes[hi].fight.held = self.combat.heroes[ci].held.map(|h| h.kind);
        for (by, amount, silent) in ticks {
            self.strike_foe(by, player, amount, false, silent, false);
        }
    }

    /// Hero `hi`'s pace under a hostile hero's slow and hold, laid on after the fight's own
    /// ([`World::set_pace`] calls it last): slower by the slow's share, feet held by any hold.
    pub(crate) fn foe_pace(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let Some(hc) = self.combat.hero(player) else {
            return;
        };
        let (held, slow) = (hc.held.is_some(), hc.slow.map(|s| s.by));
        let h = &mut self.heroes[hi];
        if let Some(by) = slow {
            h.speed *= (1.0 - by).max(0.0);
        }
        if held {
            h.fight.rooted = true;
        }
    }

    /// Hero `hi`'s auto-attack at the hostile hero it swings at: in melee reach, facing them, in
    /// clear sight, out of stealth, a blow of its class's swing every so often.
    pub(crate) fn swing_foe(&mut self, hi: usize) {
        let content = self.content.clone();
        let player = self.heroes[hi].player;
        let Some(blow) = content.classes[self.heroes[hi].class].swing else {
            return;
        };
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.foe_swing_in = (hc.foe_swing_in - DT).max(0.0);
        let Some(foe) = hc.swing_at else {
            return;
        };
        let target = self
            .hero(foe)
            .filter(|t| t.fight.up() && self.hostile(player, foe))
            .map(|t| (t.pos, t.facing));
        let Some((at, at_facing)) = target else {
            self.combat.heroes[ci].swing_at = None;
            return;
        };
        let h = &self.heroes[hi];
        let r = &content.rules.combat;
        let held = self.combat.heroes[ci].held.is_some_and(|x| x.kind.out());
        if !h.fight.up() || h.stealthed || held || h.fight.casting.is_some() {
            return;
        }
        if self.combat.heroes[ci].foe_swing_in > 0.0
            || !in_range(h.pos.distance(at), MELEE, r)
            || !stands_right(Facing::Front, h.pos, h.facing, at, at_facing, r.front)
            || !self.foe_in_sight(hi, Some(foe))
        {
            return;
        }
        self.combat.heroes[ci].foe_swing_in = blow.every;
        let amount = self.combat.dice.between(blow.min, blow.max);
        self.events.push(Event::Swing {
            by: UnitRef::Hero(player),
            to: UnitRef::Hero(foe),
        });
        self.strike_foe(player, foe, amount, false, false, true);
    }

    /// Hero `hi`'s telegraph for `aimed`, landing in `windup` seconds: around them for one on
    /// themselves or a cone, else under their target as it stands now; turned toward the target.
    pub(crate) fn put_telegraph(&mut self, hi: usize, aimed: &Aimed, t: TelegraphDef, windup: f32) {
        let h = &self.heroes[hi];
        let (player, from) = (h.player, V3::at(h.pos, h.z));
        let target = aimed
            .foe
            .and_then(|p| self.hero(p))
            .map(|t| V3::at(t.pos, t.z))
            .or(aimed
                .enemy
                .map(|ei| V3::at(self.enemies[ei].pos, self.enemies[ei].z)))
            .or(aimed.at);
        let at = if t.on_self || matches!(t.shape, Shape::Cone { .. }) {
            from
        } else {
            target.unwrap_or(from)
        };
        let facing = target
            .map(|p| (p.ground() - h.pos).normalize_or_zero())
            .filter(|f| *f != V2::ZERO)
            .unwrap_or(h.facing);
        let id = self.combat.next_telegraph;
        self.combat.next_telegraph += 1;
        self.telegraphs.push(Telegraph {
            id,
            enemy: 0,
            by: Some(player),
            ability: aimed.ability as u16,
            shape: t.shape,
            at,
            facing,
            left: windup,
            total: windup,
        });
    }

    /// Hero `by`'s telegraph `t` lands: on every hostile hero up inside it that the caster has a
    /// clear line to (a wall shields what is behind it, as it does from an enemy's blow; those
    /// whose dodge's moment covers it see it miss) and on every enemy alive inside it in the same
    /// way, with the ability's own effects. Nothing lands from a caster no longer up.
    pub(crate) fn foe_telegraph_lands(&mut self, by: PlayerId, t: Telegraph) {
        let Some(caster) = self.hero(by).filter(|h| h.fight.up()) else {
            return;
        };
        let eye = V3::at(caster.pos, caster.z + EYE_HEIGHT);
        let ability = usize::from(t.ability);
        let at = t.at.ground();
        let inside: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.fight.up() && t.shape.contains(at, t.facing, h.pos))
            .filter(|h| self.hostile(by, h.player))
            .filter(|h| self.clear_line(eye, V3::at(h.pos, h.z + CHEST_HEIGHT)))
            .map(|h| h.player)
            .collect();
        let mut hits = Vec::new();
        for p in inside {
            if self.combat.hero(p).is_some_and(|c| c.immune > 1e-4) {
                self.events.push(Event::Evaded {
                    player: p,
                    enemy: 0,
                });
                continue;
            }
            self.land_foe(by, ability, p, 0);
            hits.push(UnitRef::Hero(p));
        }
        let struck: Vec<usize> = (0..self.enemies.len())
            .filter(|&ei| {
                let e = &self.enemies[ei];
                e.hp > 0.0
                    && t.shape.contains(at, t.facing, e.pos)
                    && self.clear_line(eye, V3::at(e.pos, e.z + CHEST_HEIGHT))
            })
            .collect();
        for ei in struck {
            self.land_on(by, ability, ei, 0);
            hits.push(UnitRef::Enemy(self.enemies[ei].id));
        }
        self.events.push(Event::CastGo {
            by: UnitRef::Hero(by),
            ability: t.ability,
            hits,
            at: Some(t.at),
        });
    }
}
