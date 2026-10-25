//! Enemies: burns ticking, walking to the nearest hero, telegraphed blows, and the spawner that
//! keeps the field full.

use super::{
    Burn, Enemy, EnemyState, Event, Owner, Projectile, Telegraph, World, BURN_TICK, CHILL_SLOW,
    DT, HERO_RADIUS,
};
use crate::content::{Behaviour, School};
use crate::math::V2;

/// How wide a melee swing is, in radians.
const SWING_ARC: f32 = 2.1;
/// A Ranged enemy backs off when a hero is nearer than this share of its reach.
const TOO_CLOSE: f32 = 0.45;
/// How far a Ranged enemy's missile flies, as a share of its reach.
const THROW_RANGE: f32 = 1.6;

impl World {
    pub(super) fn step_enemies(&mut self) {
        let now = self.time;
        for e in 0..self.enemies.len() {
            self.step_burn(e);
            if self.enemies[e].hp <= 0.0 || self.enemies[e].is_frozen(now) {
                continue;
            }
            self.step_enemy(e);
        }
        self.separate();
    }

    fn step_burn(&mut self, e: usize) {
        let Some(burn) = &mut self.enemies[e].burn else {
            return;
        };
        burn.next -= DT;
        if burn.next > 0.0 {
            return;
        }
        let Burn { per_tick, by, .. } = *burn;
        burn.next += BURN_TICK;
        burn.ticks = burn.ticks.saturating_sub(1);
        if burn.ticks == 0 {
            self.enemies[e].burn = None;
        }
        self.damage_enemy(e, per_tick, false, School::Fire, Some(by));
    }

    fn step_enemy(&mut self, e: usize) {
        let now = self.time;
        let def = &self.content.enemies[self.enemies[e].kind];
        let slow = if self.enemies[e].is_chilled(now) {
            1.0 - CHILL_SLOW
        } else {
            1.0
        };
        let Some(h) = self.nearest_hero(self.enemies[e].pos) else {
            return;
        };
        let target = self.heroes[h].pos;
        let enemy = &self.enemies[e];
        let to = target - enemy.pos;
        let dist = to.length();
        let dir = to.normalize_or_zero();
        let step = def.speed * slow * DT;
        let (behaviour, reach, windup, recover) =
            (def.behaviour, def.reach, def.windup, def.recover);
        let body = def.radius + HERO_RADIUS;

        match enemy.state {
            EnemyState::Chase => {
                let enemy = &mut self.enemies[e];
                if dir != V2::ZERO {
                    enemy.facing = dir;
                }
                let start = match behaviour {
                    // Close enough that stepping back takes you out of the swing.
                    Behaviour::Melee => dist <= body + reach * 0.6,
                    Behaviour::Slam => dist <= body + reach * 0.5,
                    Behaviour::Ranged => dist <= reach,
                };
                if behaviour == Behaviour::Ranged && dist < reach * TOO_CLOSE {
                    enemy.pos += -dir * step;
                } else if start {
                    let at = match behaviour {
                        Behaviour::Slam => enemy.pos,
                        _ => target,
                    };
                    enemy.state = EnemyState::Windup {
                        left: windup,
                        total: windup,
                        at,
                    };
                    let id = enemy.id;
                    self.events.push(Event::EnemyWindup { id });
                } else {
                    enemy.pos += dir * step.min((dist - body).max(0.0));
                }
            }
            EnemyState::Windup { left, total, at } => {
                // A Chill slows the windup too.
                let left = left - DT * slow;
                if left > 0.0 {
                    self.enemies[e].state = EnemyState::Windup { left, total, at };
                } else {
                    self.strike(e, at);
                    self.enemies[e].state = EnemyState::Recover { left: recover };
                }
            }
            EnemyState::Recover { left } => {
                let left = left - DT * slow;
                self.enemies[e].state = if left > 0.0 {
                    EnemyState::Recover { left }
                } else {
                    EnemyState::Chase
                };
            }
        }
    }

    /// The telegraphed blow lands, on whoever is still in it.
    fn strike(&mut self, e: usize, at: V2) {
        let enemy = &self.enemies[e];
        let def = &self.content.enemies[enemy.kind];
        let (id, pos, damage, reach, radius) =
            (enemy.id, enemy.pos, def.damage, def.reach, def.radius);
        self.events.push(Event::EnemyStrike { id });
        match def.behaviour {
            Behaviour::Melee => {
                let dir = (at - pos).normalize_or_zero();
                let hit: Vec<usize> = (0..self.heroes.len())
                    .filter(|&h| {
                        let to = self.heroes[h].pos - pos;
                        to.length() <= reach + radius + HERO_RADIUS
                            && (to.length() < radius
                                || to.normalize_or_zero().dot(dir) >= (SWING_ARC / 2.0).cos())
                    })
                    .collect();
                for h in hit {
                    self.damage_hero(h, damage, School::Physical);
                }
            }
            Behaviour::Slam => {
                let hit: Vec<usize> = (0..self.heroes.len())
                    .filter(|&h| self.heroes[h].pos.distance(at) <= reach + HERO_RADIUS)
                    .collect();
                for h in hit {
                    self.damage_hero(h, damage, School::Physical);
                }
            }
            Behaviour::Ranged => {
                let dir = (at - pos).normalize_or_zero();
                let (speed, visual) = (def.missile_speed.max(1.0), def.missile_visual);
                let pid = self.id();
                self.projectiles.push(Projectile {
                    id: pid,
                    owner: Owner::Enemy(id),
                    ability: None,
                    visual,
                    pos,
                    vel: dir * speed,
                    radius: 0.5,
                    range_left: reach * THROW_RANGE,
                    pierce_left: 0,
                    damage,
                    school: School::Fire,
                    hit: Vec::new(),
                });
                self.events.push(Event::MissileLaunched { id: pid });
            }
        }
    }

    fn nearest_hero(&self, from: V2) -> Option<usize> {
        self.heroes
            .iter()
            .enumerate()
            .filter(|(_, h)| h.alive)
            .min_by(|a, b| a.1.pos.distance(from).total_cmp(&b.1.pos.distance(from)))
            .map(|(i, _)| i)
    }

    /// Pushes overlapping enemies apart, and off the heroes.
    fn separate(&mut self) {
        let n = self.enemies.len();
        for i in 0..n {
            for j in i + 1..n {
                let ri = self.content.enemies[self.enemies[i].kind].radius;
                let rj = self.content.enemies[self.enemies[j].kind].radius;
                let d = self.enemies[j].pos - self.enemies[i].pos;
                let len = d.length();
                let overlap = ri + rj - len;
                if overlap > 0.0 {
                    let push = if len > 1e-4 {
                        d * (1.0 / len)
                    } else {
                        V2::new(1.0, 0.0)
                    } * (overlap * 0.5);
                    self.enemies[i].pos += -push;
                    self.enemies[j].pos += push;
                }
            }
            for h in 0..self.heroes.len() {
                if !self.heroes[h].alive {
                    continue;
                }
                let r = self.content.enemies[self.enemies[i].kind].radius + HERO_RADIUS;
                let d = self.enemies[i].pos - self.heroes[h].pos;
                let len = d.length();
                if len < r && len > 1e-4 {
                    self.enemies[i].pos += d * ((r - len) / len);
                }
            }
        }
    }

    /// Keeps the field filled: more enemies the longer the run lasts, up to the zone's most.
    pub(super) fn step_spawner(&mut self) {
        let zone = &self.content.zones[self.zone];
        let minutes = self.time / 60.0;
        let cap = ((zone.alive_start as f32 + zone.alive_per_minute * minutes) as u32)
            .min(zone.alive_max) as usize;
        let every = zone.spawn_every;
        self.spawn_timer -= DT;
        let alive = self.enemies.len();
        if alive >= cap {
            return;
        }
        // The opening crowd comes at once; after that one every so often.
        if self.time > DT * 1.5 && self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = every;
        let count = if self.time <= DT * 1.5 { cap - alive } else { 1 };
        for _ in 0..count {
            self.spawn_one();
        }
    }

    fn spawn_one(&mut self) {
        let Some(h) = self.nearest_hero(self.start()) else {
            return;
        };
        let zone = &self.content.zones[self.zone];
        let table: Vec<(usize, f32)> = self.content.zone_spawns[self.zone]
            .iter()
            .filter(|&&(_, _, after)| after <= self.time)
            .map(|&(k, w, _)| (k, w))
            .collect();
        let total: f32 = table.iter().map(|(_, w)| w).sum();
        if total <= 0.0 {
            return;
        }
        let (near, far) = zone.spawn_ring;
        let mut roll = self.rng.f32() * total;
        let kind = table
            .iter()
            .find(|&&(_, w)| {
                roll -= w;
                roll <= 0.0
            })
            .map_or(table[0].0, |&(k, _)| k);
        let angle = self.rng.range(0.0, std::f32::consts::TAU);
        let dist = self.rng.range(near, far);
        let pos = self.heroes[h].pos + V2::from_angle(angle, dist);
        self.spawn_enemy(kind, pos);
    }

    /// Puts an enemy of `kind` at `pos`; returns its id.
    pub fn spawn_enemy(&mut self, kind: usize, pos: V2) -> u32 {
        let hp = self.content.enemies[kind].hp;
        let id = self.id();
        self.enemies.push(Enemy {
            id,
            kind,
            pos,
            facing: V2::new(1.0, 0.0),
            hp,
            max_hp: hp,
            state: EnemyState::Chase,
            chill_until: 0.0,
            frozen_until: 0.0,
            burn: None,
        });
        self.events.push(Event::EnemySpawned { id });
        id
    }

    /// What an enemy's windup looks like on the ground, while it winds up.
    pub fn telegraph(&self, enemy: &Enemy) -> Option<Telegraph> {
        let EnemyState::Windup { left, total, at } = enemy.state else {
            return None;
        };
        let def = &self.content.enemies[enemy.kind];
        let progress = (1.0 - left / total.max(1e-3)).clamp(0.0, 1.0);
        Some(match def.behaviour {
            Behaviour::Melee => Telegraph::Arc {
                from: enemy.pos,
                dir: (at - enemy.pos).normalize_or_zero(),
                radius: def.reach + def.radius,
                angle: SWING_ARC,
                progress,
            },
            Behaviour::Slam => Telegraph::Circle {
                at,
                radius: def.reach,
                progress,
            },
            Behaviour::Ranged => Telegraph::Line {
                from: enemy.pos,
                to: enemy.pos + (at - enemy.pos).normalize_or_zero() * def.reach * THROW_RANGE,
                progress,
            },
        })
    }
}
