//! Enemies striking back (spec §6.1, §7.1) and the dodge. An enemy after an up hero uses a move
//! of its kind when one is ready and reaches (a caster's bolt from afar; an elite's big move,
//! shown on the ground as a telegraph before it lands), else swings plainly when in reach. Every
//! blow needs a clear line to its hero at every range, a telegraph's too: it lands on each up
//! hero inside its shape that its caster has a clear line to, except one whose dodge's moment
//! (`immune`, from the press) covers it. A dodge spends a charge (each comes back in turn), makes
//! a 20-yard noise and stops a cast or a meal; the hop itself is the client's.

use super::{in_range, EnemyCast, Event, Hold, Refusal, Stop, Telegraph, UnitRef};
use crate::enemies::heading;
use crate::{Aim, Effect, Loudness, PlayerId, Shape, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V3};

impl World {
    /// Every enemy after a hero fights for a step; every telegraph that is due lands; and the
    /// dodges' moments run down.
    pub(crate) fn strike_back(&mut self) {
        for ei in 0..self.enemies.len().min(self.combat.enemies.len()) {
            self.fight_back(ei);
        }
        self.land_telegraphs();
        for hc in &mut self.combat.heroes {
            hc.immune = (hc.immune - DT).max(0.0);
        }
    }

    /// Enemy `ei`'s turn ([`World::turn`]), then what the clients see of its fight (its cast) is
    /// written afresh, and when its cast began or ended what its senses are told too (it stands
    /// still while it casts), so neither lags a step behind this step's events.
    fn fight_back(&mut self, ei: usize) {
        let cast = self.combat.enemies[ei].cast.is_some();
        self.turn(ei);
        self.refresh_fight(ei, self.combat.enemies[ei].cast.is_some() != cast);
    }

    /// Enemy `ei`'s fight for the clients from its state now, and its engage for the senses when
    /// `engage` (working it out casts a ray for a caster).
    fn refresh_fight(&mut self, ei: usize, engage: bool) {
        if engage {
            let content = self.content.clone();
            self.combat.enemies[ei].engage = self.engage(ei, &content);
        }
        self.enemies[ei].fight = self.enemy_fight(ei);
    }

    /// Enemy `ei`'s turn: its clocks run, its cast goes on or lands, else it begins a ready move
    /// that reaches its hero, else it swings when in reach.
    fn turn(&mut self, ei: usize) {
        let content = self.content.clone();
        {
            let ec = &mut self.combat.enemies[ei];
            ec.swing_in = (ec.swing_in - DT).max(0.0);
            for t in &mut ec.moves_in {
                *t = (*t - DT).max(0.0);
            }
        }
        if self.enemies[ei].hp <= 0.0 || self.combat.enemies[ei].engage.hold == Hold::Out {
            return;
        }
        let id = self.enemies[ei].id;
        if let Some(mut c) = self.combat.enemies[ei].cast {
            // A telegraph lands by itself ([`World::land_telegraphs`]); one that is gone leaves a
            // cast with nothing to wait for, so the cast is dropped.
            if let Some(tid) = c.telegraph {
                if !self.telegraphs.iter().any(|t| t.id == tid) {
                    self.combat.enemies[ei].cast = None;
                    self.events.push(Event::CastStop {
                        by: UnitRef::Enemy(id),
                        ability: c.ability as u16,
                        why: Stop::Interrupted,
                    });
                }
                return;
            }
            c.left -= DT;
            let up = self.hero(c.player).is_some_and(|h| h.fight.up());
            if !up {
                self.combat.enemies[ei].cast = None;
                self.events.push(Event::CastStop {
                    by: UnitRef::Enemy(id),
                    ability: c.ability as u16,
                    why: Stop::TargetLost,
                });
            } else if c.left <= 1e-4 {
                self.combat.enemies[ei].cast = None;
                self.spell_lands(ei, c.ability, c.player);
            } else {
                self.combat.enemies[ei].cast = Some(c);
            }
            return;
        }
        let Some((player, _)) = self.senses.target_of(ei) else {
            return;
        };
        let Some(hi) = self
            .hero_slot(player)
            .filter(|&hi| self.heroes[hi].fight.up())
        else {
            return;
        };
        let def = &content.enemies[self.enemies[ei].kind];
        for (m, &(a, every)) in def.kit.iter().enumerate() {
            let ready = self.combat.enemies[ei]
                .moves_in
                .get(m)
                .is_some_and(|t| *t <= 0.0);
            if ready && self.move_reaches(ei, hi, a) {
                self.combat.enemies[ei].moves_in[m] = every;
                self.begin_move(ei, hi, a);
                return;
            }
        }
        let Some(blow) = def.swing else {
            return;
        };
        let (e, h) = (&self.enemies[ei], &self.heroes[hi]);
        if e.pos.distance(h.pos) > def.reach
            || self.combat.enemies[ei].swing_in > 0.0
            || !self.line_to(ei, hi)
        {
            return;
        }
        let to = h.pos;
        self.enemies[ei].facing = heading(to - self.enemies[ei].pos);
        self.combat.enemies[ei].swing_in = blow.every;
        let amount = self.combat.dice.between(blow.min, blow.max);
        self.events.push(Event::Swing {
            by: UnitRef::Enemy(id),
            to: UnitRef::Hero(player),
        });
        self.blow_lands(id, player, amount);
    }

    /// An enemy's blow of `amount` lands on `player`'s hero: the damage, and the noise of the hit
    /// (spec §5.2), made where they stand and before the senses listen. Nothing is heard of a blow
    /// that does no harm (a hero not up, an amount that is no number).
    pub(super) fn blow_lands(&mut self, enemy: u32, player: PlayerId, amount: f32) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let (up, at) = (
            self.heroes[hi].fight.up(),
            V3::at(self.heroes[hi].pos, self.heroes[hi].z),
        );
        self.wound(player, amount, Some(UnitRef::Enemy(enemy)));
        if up && amount.is_finite() && amount > 0.0 {
            self.make_noise(player, at, Loudness::Hit);
        }
    }

    /// Enemy `ei` makes the noise of its ability `a` where it stands, as loud as the ability's
    /// sound (a stomp is loud), credited to the hero it is after, `player`.
    fn ability_sounds(&mut self, ei: usize, a: usize, player: PlayerId) {
        if let Some(loud) = self.content.abilities[a].sound.loudness() {
            let e = &self.enemies[ei];
            self.make_noise(player, V3::at(e.pos, e.z), loud);
        }
    }

    /// Whether enemy `ei` has a clear line to hero `hi`, at every range: a blow struck through a
    /// wall, a swing included, is no blow (as for a hero's own).
    fn line_to(&self, ei: usize, hi: usize) -> bool {
        let (e, h) = (&self.enemies[ei], &self.heroes[hi]);
        self.clear_line(
            V3::at(e.pos, e.z + EYE_HEIGHT),
            V3::at(h.pos, h.z + CHEST_HEIGHT),
        )
    }

    /// Whether enemy `ei`'s move `a` reaches hero `hi` from where it stands, with a clear line: a
    /// circle around it within its size, else the ability's range.
    fn move_reaches(&self, ei: usize, hi: usize, a: usize) -> bool {
        let ab = &self.content.abilities[a];
        let d = self.enemies[ei].pos.distance(self.heroes[hi].pos);
        let near = match ab.telegraph {
            Some(t) if t.on_self => d <= t.shape.reach(),
            _ if ab.aim == Aim::Me => d <= ab.radius,
            _ => d <= ab.range,
        };
        near && self.line_to(ei, hi)
    }

    /// Enemy `ei` begins move `a` at hero `hi`: turned to them, a telegraph put down where it
    /// will land (around itself, from itself for a cone, else under the hero as they stand now),
    /// and a cast or windup started; an instant lands at once.
    fn begin_move(&mut self, ei: usize, hi: usize, a: usize) {
        let content = self.content.clone();
        let ab = &content.abilities[a];
        let (id, from, from_z) = (
            self.enemies[ei].id,
            self.enemies[ei].pos,
            self.enemies[ei].z,
        );
        let (player, to, to_z) = (
            self.heroes[hi].player,
            self.heroes[hi].pos,
            self.heroes[hi].z,
        );
        let facing = heading(to - from);
        self.enemies[ei].facing = facing;
        let windup = ab.windup();
        let mut telegraph = None;
        if let Some(t) = ab.telegraph {
            let at = if t.on_self || matches!(t.shape, Shape::Cone { .. }) {
                V3::at(from, from_z)
            } else {
                V3::at(to, to_z)
            };
            let tid = self.combat.next_telegraph;
            self.combat.next_telegraph += 1;
            self.telegraphs.push(Telegraph {
                id: tid,
                enemy: id,
                ability: a as u16,
                shape: t.shape,
                at,
                facing,
                left: windup,
                total: windup,
            });
            telegraph = Some(tid);
        }
        if windup <= 0.0 {
            self.spell_lands(ei, a, player);
            return;
        }
        self.combat.enemies[ei].cast = Some(EnemyCast {
            ability: a,
            player,
            left: windup,
            total: windup,
            telegraph,
        });
        self.events.push(Event::CastStart {
            by: UnitRef::Enemy(id),
            ability: a as u16,
            target: Some(UnitRef::Hero(player)),
            secs: windup,
        });
    }

    /// An enemy's ability does its damage: its `Damage` effects, and `Weapon` as its kind's
    /// swing. Other effects are heroes' alone for now.
    fn enemy_damage(&mut self, ei: usize, a: usize) -> f32 {
        let content = self.content.clone();
        let swing = content.enemies[self.enemies[ei].kind].swing;
        let mut total = 0.0;
        for effect in &content.abilities[a].effects {
            match *effect {
                Effect::Damage { min, max, .. } => total += self.combat.dice.between(min, max),
                Effect::Weapon { percent, bonus } => {
                    if let Some(s) = swing {
                        total += self.combat.dice.between(s.min, s.max) * percent / 100.0 + bonus;
                    }
                }
                _ => {}
            }
        }
        total
    }

    /// Enemy `ei`'s cast of `a` at `player` goes off. An ability aimed at an enemy's own place
    /// (`Aim::Me`) lands on every up hero within its radius it has a clear line to, as a telegraph
    /// does; any other on `player` alone, if they are still up, in range and in sight.
    fn spell_lands(&mut self, ei: usize, a: usize, player: PlayerId) {
        let id = self.enemies[ei].id;
        let ab = &self.content.abilities[a];
        let area = ab.aim == Aim::Me;
        let range = if area { ab.radius } else { ab.range };
        let reach = &self.content.rules.combat;
        let struck: Vec<PlayerId> = (0..self.heroes.len())
            .filter(|&hi| {
                let (e, h) = (&self.enemies[ei], &self.heroes[hi]);
                (area || h.player == player)
                    && h.fight.up()
                    && in_range(e.pos.distance(h.pos), range, reach)
                    && self.line_to(ei, hi)
            })
            .map(|hi| self.heroes[hi].player)
            .collect();
        let mut hits = Vec::new();
        for p in struck {
            let amount = self.enemy_damage(ei, a);
            self.blow_lands(id, p, amount);
            hits.push(UnitRef::Hero(p));
        }
        // A wail (spec §7.3) wakes everything near, when it landed on its hero.
        let alarm = self.content.abilities[a].effects.iter().find_map(|e| match e {
            Effect::Alarm { radius } => Some(*radius),
            _ => None,
        });
        if let (Some(radius), true) = (alarm, hits.contains(&UnitRef::Hero(player))) {
            self.alarm(ei, player, radius);
        }
        self.ability_sounds(ei, a, player);
        self.events.push(Event::CastGo {
            by: UnitRef::Enemy(id),
            ability: a as u16,
            hits,
            at: None,
        });
    }

    /// Telegraphs run down, and those due land.
    fn land_telegraphs(&mut self) {
        for t in &mut self.telegraphs {
            t.left -= DT;
        }
        let due: Vec<Telegraph> = self
            .telegraphs
            .iter()
            .filter(|t| t.left <= 1e-4)
            .copied()
            .collect();
        self.telegraphs.retain(|t| t.left > 1e-4);
        for t in due {
            self.telegraph_lands(t);
        }
    }

    /// Telegraph `t` lands on every up hero inside its shape that its caster has a clear line to
    /// (a wall shields what is behind it, an area blow's too), except those whose dodge's moment
    /// covers it, who see it miss. A caster that is gone leaves nothing to land: a dead enemy's
    /// telegraphs are taken down with it.
    fn telegraph_lands(&mut self, t: Telegraph) {
        let Some(ei) = self.enemies.iter().position(|e| e.id == t.enemy) else {
            return;
        };
        // The noise of its own sound is credited to the hero it was cast at, else the first inside.
        let mut target = None;
        if let Some(c) = &self.combat.enemies[ei].cast {
            if c.telegraph == Some(t.id) {
                target = Some(c.player);
                self.combat.enemies[ei].cast = None;
            }
        }
        if target.is_some() {
            self.refresh_fight(ei, true);
        }
        let inside: Vec<PlayerId> = (0..self.heroes.len())
            .filter(|&hi| {
                let h = &self.heroes[hi];
                h.fight.up()
                    && t.shape.contains(t.at.ground(), t.facing, h.pos)
                    && self.line_to(ei, hi)
            })
            .map(|hi| self.heroes[hi].player)
            .collect();
        let mut hits = Vec::new();
        for &p in &inside {
            let ci = self.combat.hero_index(p);
            if self.combat.heroes[ci].immune > 1e-4 {
                self.events.push(Event::Evaded {
                    player: p,
                    enemy: t.enemy,
                });
                continue;
            }
            let amount = self.enemy_damage(ei, usize::from(t.ability));
            self.blow_lands(t.enemy, p, amount);
            hits.push(UnitRef::Hero(p));
        }
        if let Some(p) = target.or(inside.first().copied()) {
            self.ability_sounds(ei, usize::from(t.ability), p);
        }
        self.events.push(Event::CastGo {
            by: UnitRef::Enemy(t.enemy),
            ability: t.ability,
            hits,
            at: Some(t.at),
        });
    }

    /// `player` dodges (spec §6.1): a charge spent if there is one; telegraphed blows miss them
    /// for `dodge.immune` seconds; a cast or a meal stops; a 20-yard noise. Their client has
    /// already hopped them away. A hero who is not up is refused (told so), one with no charge
    /// left does nothing. Whether the dodge was made.
    pub(crate) fn dodge(&mut self, player: PlayerId) -> bool {
        let Some(hi) = self.hero_slot(player) else {
            return false;
        };
        if !self.heroes[hi].fight.up() {
            self.events.push(Event::Refused {
                player,
                why: Refusal::Down,
            });
            return false;
        }
        let r = self.content.rules.combat.dodge;
        if !self.heroes[hi].fight.dodge.spend(&r) {
            return false;
        }
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].immune = r.immune;
        self.stop_cast(hi, Stop::Cancelled);
        self.stop_eating(hi);
        let at = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
        self.make_noise(player, at, Loudness::Normal);
        self.events.push(Event::Dodged { player });
        true
    }
}
