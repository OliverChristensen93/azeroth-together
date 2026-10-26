//! A hero's turn: walking, rolling, aiming, and the ability on the held key going off at the aim.

use super::{Action, Area, Event, Projectile, World, DT};
use crate::content::Shape;
use crate::math::V2;
use crate::world::Owner;
use crate::PlayerId;

impl World {
    pub(super) fn step_hero(&mut self, h: usize) {
        let now = self.time;
        let player = self.heroes[h].player;
        let input = self.inputs.get(&player).copied().unwrap_or_default();
        let class = &self.content.classes[self.heroes[h].class];
        let (regen, speed, dodge) = (class.mana_regen, class.speed, class.dodge);
        let hero = &mut self.heroes[h];
        if !hero.alive {
            return;
        }
        hero.mana = (hero.mana + regen * DT).min(hero.max_mana);
        hero.cooldowns.iter_mut().for_each(|c| *c = (*c - DT).max(0.0));
        if now >= hero.guard_until {
            hero.absorb = 0.0;
            hero.immune = false;
        }
        hero.roll_cooldown = (hero.roll_cooldown - DT).max(0.0);
        if hero.is_immune(now) {
            // Ice Block: frozen in place, nothing to do but wait.
            return;
        }
        // A roll under way carries the hero; nothing else happens until it ends.
        if let Some((dir, left)) = hero.roll {
            hero.pos += dir * (dodge.distance / dodge.time.max(DT) * DT);
            let left = left - DT;
            if left <= 1e-4 {
                hero.roll = None;
                hero.roll_cooldown = dodge.recover;
            } else {
                hero.roll = Some((dir, left));
            }
            return;
        }
        if input.dodge && hero.roll_cooldown <= 0.0 {
            let dir = {
                let d = input.move_dir.normalize_or_zero();
                if d == V2::ZERO {
                    hero.facing
                } else {
                    d
                }
            };
            let from = hero.pos;
            hero.roll = Some((dir, dodge.time));
            // A roll breaks off any cast or channel.
            if hero.action != Action::Idle {
                self.stop_channel(h);
            }
            self.events.push(Event::Dodged {
                hero: player,
                from,
                to: from + dir * dodge.distance,
            });
            return;
        }
        let hero = &mut self.heroes[h];
        let to_aim = input.aim - hero.pos;
        if to_aim.length() > 0.05 {
            hero.facing = to_aim.normalize_or_zero();
        }
        let walking = input.move_dir.length() > 1e-3;

        let may_walk = match hero.action {
            Action::Idle => true,
            Action::Casting { slot, left, .. } => {
                let left = left - DT;
                if left <= 0.0 {
                    self.heroes[h].action = Action::Idle;
                    if let Some(ab) = self.slot_ability(&self.heroes[h], slot) {
                        self.go(h, slot, ab, input.aim);
                    }
                } else if let Action::Casting { left: l, .. } = &mut self.heroes[h].action {
                    *l = left;
                }
                self.slot_ability(&self.heroes[h], slot)
                    .is_some_and(|ab| self.content.abilities[ab].move_while_casting)
            }
            Action::Channeling {
                slot,
                left,
                next,
                area,
                ..
            } => {
                if walking || input.cast != Some(slot) {
                    self.stop_channel(h);
                    true
                } else {
                    let left = left - DT;
                    let mut next = next - DT;
                    if left <= 0.0 {
                        self.heroes[h].action = Action::Idle;
                        self.end_channel_area(player);
                    } else {
                        if next <= 0.0 && area.is_none() {
                            if let Some(ab) = self.slot_ability(&self.heroes[h], slot) {
                                next += self.content.abilities[ab].channel_tick;
                                self.shape(h, ab, input.aim);
                            }
                        }
                        if let Action::Channeling {
                            left: l, next: n, ..
                        } = &mut self.heroes[h].action
                        {
                            *l = left;
                            *n = next;
                        }
                    }
                    false
                }
            }
        };

        if self.heroes[h].action == Action::Idle {
            if let Some(slot) = input.cast {
                self.try_cast(h, slot, input.aim);
            }
        }
        // A cast that roots the hero holds them from the moment it starts.
        let may_walk = may_walk
            && match self.heroes[h].action {
                Action::Idle => true,
                Action::Casting { slot, .. } => self
                    .slot_ability(&self.heroes[h], slot)
                    .is_some_and(|ab| self.content.abilities[ab].move_while_casting),
                Action::Channeling { .. } => false,
            };
        if may_walk && walking {
            let hero = &mut self.heroes[h];
            hero.pos += input.move_dir.normalize_or_zero() * (speed * DT);
        }
    }

    /// Starts the ability on `slot` if it is ready and paid for.
    fn try_cast(&mut self, h: usize, slot: u8, aim: V2) {
        let Some(ab) = self.slot_ability(&self.heroes[h], slot) else {
            return;
        };
        let def = &self.content.abilities[ab];
        let (cost, cooldown, cast_time) = (def.mana, def.cooldown, def.cast_time);
        let hero = &mut self.heroes[h];
        if hero.cooldowns[slot as usize] > 0.0 || hero.mana < cost {
            return;
        }
        hero.mana -= cost;
        hero.cooldowns[slot as usize] = cooldown;
        if cast_time > 0.0 {
            hero.action = Action::Casting {
                slot,
                left: cast_time,
                total: cast_time,
            };
            let player = hero.player;
            self.events.push(Event::CastStart {
                hero: player,
                ability: ab,
                time: cast_time,
            });
        } else {
            self.go(h, slot, ab, aim);
        }
    }

    /// The ability goes off at `aim`; a channelled one keeps going.
    fn go(&mut self, h: usize, slot: u8, ab: usize, aim: V2) {
        let def = &self.content.abilities[ab];
        let (channel, tick) = (def.channel, def.channel_tick);
        let ground = matches!(def.shape, Shape::Ground { .. });
        let area = self.shape(h, ab, aim);
        if channel > 0.0 {
            self.heroes[h].action = Action::Channeling {
                slot,
                left: channel,
                total: channel,
                next: tick,
                area: if ground { area } else { None },
            };
        }
    }

    /// One go of the ability's shape, aimed at `aim`. Returns the area it laid, if any.
    pub(super) fn shape(&mut self, h: usize, ab: usize, aim: V2) -> Option<u32> {
        let def = self.content.abilities[ab].clone();
        let hero = &self.heroes[h];
        let (player, pos) = (hero.player, hero.pos);
        let dir = {
            let d = (aim - pos).normalize_or_zero();
            if d == V2::ZERO {
                hero.facing
            } else {
                d
            }
        };
        let at = match def.shape {
            Shape::Ground { .. } | Shape::Blink => {
                let d = pos.distance(aim).min(def.range);
                pos + dir * d
            }
            _ => aim,
        };
        self.events.push(Event::Cast {
            hero: player,
            ability: ab,
            at,
        });
        match def.shape {
            Shape::Projectile {
                speed,
                radius,
                pierce,
                count,
                spread,
                ..
            } => {
                let spread = spread.to_radians();
                for i in 0..count {
                    let turn = if count > 1 {
                        -spread / 2.0 + spread * i as f32 / (count - 1) as f32
                    } else {
                        0.0
                    };
                    let id = self.id();
                    self.projectiles.push(Projectile {
                        id,
                        owner: Owner::Hero(player),
                        ability: Some(ab),
                        visual: def.visual,
                        pos,
                        vel: dir.rotate(turn) * speed,
                        radius,
                        range_left: def.range,
                        pierce_left: pierce,
                        damage: def.damage,
                        school: def.school,
                        hit: Vec::new(),
                    });
                    self.events.push(Event::MissileLaunched { id });
                }
                None
            }
            Shape::Ground {
                radius,
                delay,
                duration,
                tick,
                ..
            } => {
                let id = self.id();
                let channel = def.channel > 0.0;
                self.areas.push(Area {
                    id,
                    caster: player,
                    ability: ab,
                    pos: at,
                    radius,
                    delay,
                    left: if channel { def.channel } else { duration },
                    next: tick,
                    channel,
                    opened: false,
                });
                self.events.push(Event::AreaStarted { id });
                Some(id)
            }
            Shape::Cone { angle } => {
                let half = (angle / 2.0).to_radians().cos();
                let hits: Vec<usize> = (0..self.enemies.len())
                    .filter(|&e| {
                        let to = self.enemies[e].pos - pos;
                        let reach = def.range + self.content.enemies[self.enemies[e].kind].radius;
                        to.length() <= reach
                            && (to.length() < 1.0 || to.normalize_or_zero().dot(dir) >= half)
                    })
                    .collect();
                for e in hits {
                    self.hit_enemy(e, player, ab, def.damage, pos);
                }
                None
            }
            Shape::Nova { radius } => {
                let hits: Vec<usize> = (0..self.enemies.len())
                    .filter(|&e| {
                        let r = radius + self.content.enemies[self.enemies[e].kind].radius;
                        self.enemies[e].pos.distance(pos) <= r
                    })
                    .collect();
                for e in hits {
                    self.hit_enemy(e, player, ab, def.damage, pos);
                }
                None
            }
            Shape::Guard {
                duration,
                immune,
                absorb,
            } => {
                let now = self.time;
                let hero = &mut self.heroes[h];
                hero.guard_until = now + duration;
                hero.immune = immune;
                hero.absorb = absorb;
                if immune {
                    // Ice Block ends whatever the hero was doing.
                    hero.action = Action::Idle;
                }
                None
            }
            Shape::Blink => {
                self.heroes[h].pos = at;
                self.events.push(Event::Blinked {
                    hero: player,
                    from: pos,
                    to: at,
                });
                None
            }
        }
    }

    /// Stops a cast or channel early.
    fn stop_channel(&mut self, h: usize) {
        let player = self.heroes[h].player;
        self.heroes[h].action = Action::Idle;
        self.end_channel_area(player);
        self.events.push(Event::CastStopped { hero: player });
    }

    /// Ends the area `player`'s channel holds up, if there is one.
    pub(super) fn end_channel_area(&mut self, player: PlayerId) {
        let mut ended = Vec::new();
        self.areas.retain(|a| {
            let held = a.channel && a.caster == player;
            if held {
                ended.push(a.id);
            }
            !held
        });
        for id in ended {
            self.events.push(Event::AreaEnded { id });
        }
    }
}
