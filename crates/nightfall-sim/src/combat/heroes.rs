//! Heroes in the fight (spec §6.1): each new ability press taken once, the checks it must pass
//! (WoW's own refusals, sent back to its player), the global cooldown, cooldowns and power, casts
//! that take time and channels that tick, what a caster may do with their feet meanwhile,
//! auto-attack, eating, regeneration out of combat, Sprint, and Stealth's own cooldown.
//!
//! Clients own their position, so the host cannot hold a caster's feet: it lowers a slowed
//! caster's speed (which their controller takes, as stealth's does), roots a rooted one (the
//! client's `HeroFight::rooted`), and breaks any cast whose hero went further from where it began
//! than [`allowance`] allows.

use super::{
    allowance, in_range, stands_right, within_arc, CastPress, CastState, Casting, Cooldown, Event,
    Refusal, Stop, UnitRef, MANA,
};
use crate::{
    Aim, Effect, Facing, Gait, Moving, PlayerId, PlayerInput, World, CHEST_HEIGHT, DT, EYE_HEIGHT,
    V3,
};

/// Melee abilities' and auto-attack's range, yards (WoW's 5).
pub(crate) const MELEE: f32 = 5.0;

/// A press that passed its checks: the ability, its enemy (an index), its spot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Aimed {
    pub(crate) ability: usize,
    pub(crate) enemy: Option<usize>,
    pub(crate) at: Option<V3>,
}

impl World {
    /// `player`'s presses in this step's input: whether they move (a rooted cast cannot begin on
    /// the move), and the latest ability press, once. The first input a host sees of a hero is a
    /// baseline, so a new run never casts the last run's press.
    pub(crate) fn take_presses(&mut self, player: PlayerId, input: &PlayerInput) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        // Stealth may begin and be broken by a press in this one step: it must still be seen to
        // have ended, or its cooldown would never start.
        hc.was_stealthed |= self.heroes[hi].stealthed;
        hc.moving = input.gait != Gait::Still;
        let first = !hc.started;
        hc.started = true;
        let last = hc.seq;
        if let Some(p) = input.cast {
            hc.seq = Some(p.seq);
        }
        if first {
            return;
        }
        if let Some(p) = input.cast.filter(|p| last != Some(p.seq)) {
            self.press(hi, p);
        }
    }

    /// A new press on hero `hi`'s bar: begun if it passes its checks, else refused to its player.
    fn press(&mut self, hi: usize, p: CastPress) {
        let player = self.heroes[hi].player;
        let content = self.content.clone();
        let kit = &content.classes[self.heroes[hi].class].kit;
        let Some(&(_, ability)) = kit.iter().find(|(s, _)| *s == p.slot) else {
            return;
        };
        // Stealth's button counts presses of its own (`PlayerInput::stealth_presses`).
        if content.abilities[ability].has(|e| matches!(e, Effect::Stealth)) {
            return;
        }
        match self.check(hi, ability, p.target, p.at) {
            Ok(aimed) => self.begin(hi, aimed),
            Err(why) => self.events.push(Event::Refused { player, why }),
        }
    }

    /// Whether hero `hi` may begin `ability` at `target` or `at`, in WoW's order of checks.
    pub(crate) fn check(
        &self,
        hi: usize,
        ability: usize,
        target: Option<UnitRef>,
        at: Option<V3>,
    ) -> Result<Aimed, Refusal> {
        let content = &self.content;
        let r = &content.rules.combat;
        let a = &content.abilities[ability];
        let hero = &self.heroes[hi];
        let f = &hero.fight;
        let moving = self.combat.hero(hero.player).is_some_and(|hc| hc.moving);
        if !f.up() {
            return Err(Refusal::Down);
        }
        if f.casting.is_some() {
            return Err(Refusal::Busy);
        }
        let cooling = f
            .cooldowns
            .iter()
            .any(|c| usize::from(c.ability) == ability);
        if cooling || (a.gcd > 0.0 && f.gcd > 0.0) {
            return Err(Refusal::NotReady);
        }
        if f.power + 1e-3 < a.cost {
            return Err(Refusal::NoPower);
        }
        let rooted = a.moving == Moving::Rooted && (a.cast > 0.0 || a.channel > 0.0);
        if moving && (a.needs.still || rooted) {
            return Err(Refusal::Moving);
        }
        if a.needs.stealth && !hero.stealthed {
            return Err(Refusal::NeedStealth);
        }
        if a.needs.out_of_combat && f.in_combat {
            return Err(Refusal::InCombat);
        }
        if a.needs.combo && f.combo == 0 {
            return Err(Refusal::NeedCombo);
        }
        match a.aim {
            Aim::Me => Ok(Aimed {
                ability,
                enemy: None,
                at: None,
            }),
            Aim::Ground => {
                let at = at.filter(|p| p.is_finite()).ok_or(Refusal::NoTarget)?;
                if !in_range(hero.pos.distance(at.ground()), a.range, r) {
                    return Err(Refusal::OutOfRange);
                }
                Ok(Aimed {
                    ability,
                    enemy: None,
                    at: Some(at),
                })
            }
            Aim::Enemy => {
                let id = match target {
                    Some(UnitRef::Enemy(id)) => id,
                    Some(UnitRef::Hero(_)) => return Err(Refusal::BadTarget),
                    None => return Err(Refusal::NoTarget),
                };
                let ei = self
                    .enemies
                    .iter()
                    .position(|e| e.id == id)
                    .ok_or(Refusal::NoTarget)?;
                let e = &self.enemies[ei];
                let kind = content.enemies[e.kind].creature;
                if e.hp <= 0.0
                    || !(a.needs.creatures.is_empty() || a.needs.creatures.contains(&kind))
                {
                    return Err(Refusal::BadTarget);
                }
                if a.needs.combo && f.combo_on != Some(id) {
                    return Err(Refusal::NeedCombo);
                }
                if a.needs.unaware && !self.opener_allowed(id, hero.player) {
                    return Err(Refusal::Aware);
                }
                if !in_range(hero.pos.distance(e.pos), a.range, r) {
                    return Err(Refusal::OutOfRange);
                }
                if !stands_right(a.facing, hero.pos, hero.facing, e.pos, e.facing, r.front) {
                    return Err(if a.facing == Facing::Behind {
                        Refusal::Behind
                    } else {
                        Refusal::Facing
                    });
                }
                if !self.in_sight_of(hi, Some(ei)) {
                    return Err(Refusal::NotInSight);
                }
                Ok(Aimed {
                    ability,
                    enemy: Some(ei),
                    at: None,
                })
            }
        }
    }

    /// Nothing solid between `a` and `b` (always, without a map).
    pub(crate) fn clear_line(&self, a: V3, b: V3) -> bool {
        self.map().is_none_or(|m| m.line_of_sight(a, b))
    }

    /// Whether hero `hi` has a clear line to enemy `ei`, at every range: a blow struck through a
    /// wall, melee included, is no blow. An ability aimed at no enemy needs none.
    fn in_sight_of(&self, hi: usize, ei: Option<usize>) -> bool {
        let Some(ei) = ei else {
            return true;
        };
        let (h, e) = (&self.heroes[hi], &self.enemies[ei]);
        self.clear_line(
            V3::at(h.pos, h.z + EYE_HEIGHT),
            V3::at(e.pos, e.z + CHEST_HEIGHT),
        )
    }

    /// A checked press begins: the global cooldown, a meal ended, and then it goes off at once, or
    /// a cast or a channel starts (a channel pays as it starts).
    fn begin(&mut self, hi: usize, aimed: Aimed) {
        let content = self.content.clone();
        let a = &content.abilities[aimed.ability];
        let player = self.heroes[hi].player;
        let target = aimed.enemy.map(|ei| UnitRef::Enemy(self.enemies[ei].id));
        let ability = aimed.ability as u16;
        if a.gcd > 0.0 {
            self.heroes[hi].fight.gcd = a.gcd;
        }
        self.stop_eating(hi);
        let channel = a.channel > 0.0;
        if a.cast <= 0.0 && !channel {
            self.pay(hi, aimed);
            self.go(hi, aimed);
            return;
        }
        let total = if channel { a.channel } else { a.cast };
        let from = self.heroes[hi].pos;
        self.heroes[hi].fight.casting = Some(Casting {
            ability,
            left: total,
            total,
            channel,
            target,
        });
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].cast = Some(CastState {
            ability: aimed.ability,
            enemy: aimed.enemy,
            at: aimed.at,
            from,
            elapsed: 0.0,
            ticks: 0,
        });
        let by = UnitRef::Hero(player);
        self.events.push(if channel {
            Event::ChannelStart {
                by,
                ability,
                target,
                secs: total,
            }
        } else {
            Event::CastStart {
                by,
                ability,
                target,
                secs: total,
            }
        });
        if channel {
            self.pay(hi, aimed);
        }
    }

    /// What going off costs and makes: the power, the ability's own cooldown, its sound (at the
    /// spot, for a ground ability), a fire spell's flash, and stealth unless it keeps it.
    fn pay(&mut self, hi: usize, aimed: Aimed) {
        let content = self.content.clone();
        let a = &content.abilities[aimed.ability];
        let player = self.heroes[hi].player;
        let here = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
        let f = &mut self.heroes[hi].fight;
        f.power = (f.power - a.cost).max(0.0);
        if a.cooldown > 0.0 {
            f.cooldowns.push(Cooldown {
                ability: aimed.ability as u16,
                left: a.cooldown,
            });
        }
        if let Some(loud) = a.sound.loudness() {
            self.make_noise(player, aimed.at.unwrap_or(here), loud);
        }
        if a.flash {
            self.flash(player);
        }
        if !a.keeps_stealth {
            self.break_stealth(player);
        }
    }

    /// It goes off (or a channel ticks): it lands, and every client sees it.
    fn go(&mut self, hi: usize, aimed: Aimed) {
        let player = self.heroes[hi].player;
        let hits = self.land(player, aimed.ability, aimed.enemy, aimed.at);
        self.events.push(Event::CastGo {
            by: UnitRef::Hero(player),
            ability: aimed.ability as u16,
            hits,
            at: aimed.at,
        });
    }

    /// Hero `hi`'s cast ends without going off, or their channel ends.
    pub(crate) fn stop_cast(&mut self, hi: usize, why: Stop) {
        let player = self.heroes[hi].player;
        let Some(c) = self.heroes[hi].fight.casting.take() else {
            return;
        };
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].cast = None;
        self.events.push(Event::CastStop {
            by: UnitRef::Hero(player),
            ability: c.ability,
            why,
        });
    }

    /// Hero `hi` stops eating and drinking.
    pub(crate) fn stop_eating(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        self.heroes[hi].fight.eating = 0.0;
        if let Some(c) = self.combat.heroes.iter_mut().find(|c| c.player == player) {
            c.meal = None;
        }
    }

    /// Whether `player`'s hero may step into stealth now: up, out of combat, and Stealth not
    /// cooling down. Leaving stealth is always allowed.
    pub(crate) fn may_stealth(&self, player: PlayerId) -> bool {
        let Some(h) = self.hero(player) else {
            return false;
        };
        let cooling = self.stealth_of(h.class).is_some_and(|s| {
            h.fight
                .cooldowns
                .iter()
                .any(|c| usize::from(c.ability) == s)
        });
        h.fight.up() && !h.fight.in_combat && !cooling
    }

    /// Why `player`'s hero may not step into stealth, in WoW's own words: down, in combat, or
    /// Stealth not ready.
    pub(crate) fn stealth_refusal(&self, player: PlayerId) -> Refusal {
        match self.hero(player) {
            Some(h) if !h.fight.up() => Refusal::Down,
            Some(h) if h.fight.in_combat => Refusal::InCombat,
            _ => Refusal::NotReady,
        }
    }

    /// The class's Stealth ability, if its bar has one.
    fn stealth_of(&self, class: usize) -> Option<usize> {
        self.content.classes[class]
            .kit
            .iter()
            .map(|&(_, a)| a)
            .find(|&a| self.content.abilities[a].has(|e| matches!(e, Effect::Stealth)))
    }

    /// How much faster Sprint makes hero `hi` this step.
    fn sprint_by(&self, hi: usize) -> f32 {
        let h = &self.heroes[hi];
        if h.fight.sprint > 0.0 {
            self.combat.hero(h.player).map_or(0.0, |c| c.sprint_by)
        } else {
            0.0
        }
    }

    /// Every hero's fight moves on a step: their clocks and power, their cast, their swing, their
    /// meal, and their pace.
    pub(crate) fn step_fighters(&mut self) {
        for hi in 0..self.heroes.len() {
            self.tick_hero(hi);
            self.step_cast(hi);
            self.swing(hi);
            self.step_meal(hi);
            self.set_pace(hi);
        }
    }

    /// The global cooldown, cooldowns, Sprint and power; in combat or not (an enemy after them,
    /// or a blow lately); Stealth's cooldown once stealth ends; regeneration out of combat.
    fn tick_hero(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let player = self.heroes[hi].player;
        let hunted = self.senses.hunted(player);
        let stealth = self.stealth_of(self.heroes[hi].class);
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.combat_for = (hc.combat_for - DT).max(0.0);
        let lingering = hc.combat_for > 0.0;
        let was_stealthed = std::mem::replace(&mut hc.was_stealthed, self.heroes[hi].stealthed);
        let hero = &mut self.heroes[hi];
        let class = &content.classes[hero.class];
        let up = hero.fight.up();
        let f = &mut hero.fight;
        f.gcd = (f.gcd - DT).max(0.0);
        for c in &mut f.cooldowns {
            c.left -= DT;
        }
        f.cooldowns.retain(|c| c.left > 1e-4);
        f.sprint = (f.sprint - DT).max(0.0);
        f.in_combat = up && (hunted || lingering);
        f.power = (f.power + class.power_regen * DT).min(f.max_power);
        // Stealth's own cooldown starts as stealth ends (WoW's 10 seconds).
        if let (true, false, Some(s)) = (was_stealthed, hero.stealthed, stealth) {
            let cd = content.abilities[s].cooldown;
            f.cooldowns.retain(|c| usize::from(c.ability) != s);
            if cd > 0.0 {
                f.cooldowns.push(Cooldown {
                    ability: s as u16,
                    left: cd,
                });
            }
        }
        if up && !f.in_combat {
            if class.power == MANA {
                f.power = (f.power + r.regen.1 * DT).min(f.max_power);
            }
            hero.hp = (hero.hp + r.regen.0 * DT).min(hero.max_hp);
        }
    }

    /// Hero `hi`'s cast moves on: broken by moving further than it allows, or by its target dying;
    /// a channel ticks; a cast goes off at its end, if its target is still in sight.
    fn step_cast(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let Some(mut cs) = self.combat.heroes[ci].cast else {
            return;
        };
        let content = self.content.clone();
        let a = &content.abilities[cs.ability];
        cs.elapsed += DT;
        let run = self.heroes[hi].speed * (1.0 + self.sprint_by(hi));
        let allowed = allowance(a.moving, run, cs.elapsed, &content.rules.combat);
        if self.heroes[hi].pos.distance(cs.from) > allowed {
            return self.stop_cast(hi, Stop::Moved);
        }
        if cs.enemy.is_some_and(|ei| self.enemies[ei].hp <= 0.0) {
            return self.stop_cast(hi, Stop::TargetLost);
        }
        if let Some(c) = &mut self.heroes[hi].fight.casting {
            c.left = (c.total - cs.elapsed).max(0.0);
        }
        let aimed = Aimed {
            ability: cs.ability,
            enemy: cs.enemy,
            at: cs.at,
        };
        if a.channel > 0.0 {
            // Like a cast's end, every tick needs the target in sight: checked each step, so a
            // channel breaks the moment its target steps behind cover, before any tick lands.
            if !self.in_sight_of(hi, aimed.enemy) {
                return self.stop_cast(hi, Stop::TargetLost);
            }
            let every = a.channel / a.ticks as f32;
            while cs.ticks < a.ticks && cs.elapsed + 1e-4 >= every * (cs.ticks + 1) as f32 {
                cs.ticks += 1;
                self.go(hi, aimed);
            }
            self.combat.heroes[ci].cast = Some(cs);
            if cs.ticks >= a.ticks {
                self.stop_cast(hi, Stop::Finished);
            }
        } else if cs.elapsed + 1e-4 >= a.cast {
            // WoW checks range only as a cast begins; at its end the target must still be seen.
            if !self.in_sight_of(hi, aimed.enemy) {
                return self.stop_cast(hi, Stop::TargetLost);
            }
            self.heroes[hi].fight.casting = None;
            self.combat.heroes[ci].cast = None;
            self.pay(hi, aimed);
            self.go(hi, aimed);
        } else {
            self.combat.heroes[ci].cast = Some(cs);
        }
    }

    /// Hero `hi`'s auto-attack: a blow every `swing.every` seconds at the enemy they attack, while
    /// it lives, stands in melee range in front of them in clear sight, and they are out of
    /// stealth and not casting. Like any blow, the first from an unseen hero is an ambush.
    fn swing(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.swing_in = (hc.swing_in - DT).max(0.0);
        let ready = hc.swing_in <= 0.0;
        let hero = &self.heroes[hi];
        let Some(id) = hero.fight.attacking else {
            return;
        };
        let target = self.enemies.iter().position(|e| e.id == id && e.hp > 0.0);
        let blow = content.classes[hero.class].swing;
        let keep = blow.is_some() && target.is_some() && !hero.stealthed && hero.fight.up();
        if !keep {
            self.heroes[hi].fight.attacking = None;
            return;
        }
        let (Some(blow), Some(ei)) = (blow, target) else {
            return;
        };
        let (hero, e) = (&self.heroes[hi], &self.enemies[ei]);
        let close = in_range(hero.pos.distance(e.pos), MELEE, r)
            && within_arc(hero.facing, e.pos - hero.pos, r.front);
        if !ready || !close || hero.fight.casting.is_some() || !self.in_sight_of(hi, Some(ei)) {
            return;
        }
        self.combat.heroes[ci].swing_in = blow.every;
        let ambush = self.ambush(id, player);
        let amount = self.combat.dice.between(blow.min, blow.max) * ambush;
        self.events.push(Event::Swing {
            by: UnitRef::Hero(player),
            to: UnitRef::Enemy(id),
        });
        self.strike(player, ei, amount, ambush > 1.0, false, true);
        if self.enemies[ei].hp > 0.0 {
            self.alert(id, player);
        }
    }

    /// Hero `hi`'s meal: health and mana back a second while they sit still out of combat, until
    /// it is eaten.
    fn step_meal(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let Some(meal) = self.combat.heroes[ci].meal else {
            return;
        };
        let moving = self.combat.heroes[ci].moving;
        let hero = &mut self.heroes[hi];
        hero.fight.eating = (hero.fight.eating - DT).max(0.0);
        if moving || hero.fight.in_combat || hero.fight.eating <= 0.0 || !hero.fight.up() {
            return self.stop_eating(hi);
        }
        hero.hp = (hero.hp + meal.health * DT).min(hero.max_hp);
        hero.fight.power = (hero.fight.power + meal.mana * DT).min(hero.fight.max_power);
    }

    /// Hero `hi`'s pace this step: the senses' speed (stealth's) times Sprint, times
    /// `cast_move_speed` while casting a slowed ability; feet held for a rooted one.
    fn set_pace(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let sprint = 1.0 + self.sprint_by(hi);
        let hero = &mut self.heroes[hi];
        let moving = hero
            .fight
            .casting
            .map(|c| content.abilities[usize::from(c.ability)].moving);
        let cast = if moving == Some(Moving::Slowed) {
            r.cast_move_speed
        } else {
            1.0
        };
        hero.speed *= sprint * cast;
        hero.fight.rooted = moving == Some(Moving::Rooted);
    }
}
