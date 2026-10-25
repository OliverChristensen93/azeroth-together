//! Missiles that really fly, and areas on the ground.

use super::{Event, Owner, World, DT, HERO_RADIUS};
use crate::content::Shape;
use crate::math::V2;

impl World {
    /// Moves every missile one tick along its line; one that touches something on the way hits it.
    pub(super) fn step_projectiles(&mut self) {
        let mut gone: Vec<(u32, V2, bool)> = Vec::new();
        for p in 0..self.projectiles.len() {
            let (from, step) = {
                let m = &self.projectiles[p];
                let travel = (m.vel.length() * DT).min(m.range_left);
                (m.pos, m.vel.normalize_or_zero() * travel)
            };
            let to = from + step;
            let mut end = None;
            match self.projectiles[p].owner {
                Owner::Hero(player) => {
                    // What its path crosses this tick, nearest first.
                    let mut crossed: Vec<(usize, f32)> = self
                        .enemies
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.hp > 0.0 && !self.projectiles[p].hit.contains(&e.id))
                        .filter_map(|(i, e)| {
                            let r = self.projectiles[p].radius + self.content.enemies[e.kind].radius;
                            let (d, t) = V2::segment_distance(e.pos, from, to);
                            (d <= r).then_some((i, t))
                        })
                        .collect();
                    crossed.sort_by(|a, b| a.1.total_cmp(&b.1));
                    for (e, t) in crossed {
                        let at = from + step * t;
                        self.missile_hit(p, e, player, at);
                        let m = &mut self.projectiles[p];
                        if m.pierce_left == 0 {
                            end = Some((at, true));
                            break;
                        }
                        m.pierce_left -= 1;
                    }
                }
                Owner::Enemy(_) => {
                    let struck = self.heroes.iter().position(|h| {
                        h.alive
                            && V2::segment_distance(h.pos, from, to).0
                                <= self.projectiles[p].radius + HERO_RADIUS
                    });
                    if let Some(h) = struck {
                        let (damage, school) =
                            (self.projectiles[p].damage, self.projectiles[p].school);
                        self.damage_hero(h, damage, school);
                        end = Some((self.heroes[h].pos, true));
                    }
                }
            }
            let m = &mut self.projectiles[p];
            m.pos = to;
            m.range_left -= step.length();
            if end.is_none() && m.range_left <= 1e-3 {
                end = Some((to, false));
            }
            if let Some((at, struck)) = end {
                gone.push((m.id, at, struck));
            }
        }
        if !gone.is_empty() {
            self.projectiles
                .retain(|m| !gone.iter().any(|&(id, _, _)| id == m.id));
            for (id, at, struck) in gone {
                self.events.push(Event::MissileGone { id, at, struck });
            }
        }
    }

    /// A hero's missile `p` strikes enemy `e` at `at`: its damage, and its splash around `at`.
    fn missile_hit(&mut self, p: usize, e: usize, player: u8, at: V2) {
        let (ab, damage, origin) = {
            let m = &mut self.projectiles[p];
            m.hit.push(self.enemies[e].id);
            (m.ability, m.damage, m.pos)
        };
        let Some(ab) = ab else { return };
        self.hit_enemy(e, player, ab, damage, origin);
        if let Shape::Projectile { splash, .. } = self.content.abilities[ab].shape {
            if splash > 0.0 {
                let others: Vec<usize> = (0..self.enemies.len())
                    .filter(|&o| o != e && self.enemies[o].hp > 0.0)
                    .filter(|&o| {
                        let r = splash + self.content.enemies[self.enemies[o].kind].radius;
                        self.enemies[o].pos.distance(at) <= r
                    })
                    .collect();
                for o in others {
                    self.hit_enemy(o, player, ab, damage * 0.5, at);
                }
            }
        }
    }

    /// Areas count down: the opening hit after the delay, then a tick every so often.
    pub(super) fn step_areas(&mut self) {
        let mut ended = Vec::new();
        for a in 0..self.areas.len() {
            let ab = self.areas[a].ability;
            let def = &self.content.abilities[ab];
            let (damage, tick, tick_damage) = match def.shape {
                Shape::Ground {
                    tick, tick_damage, ..
                } => (def.damage, tick, tick_damage),
                _ => (def.damage, 1.0, 0.0),
            };
            let area = &mut self.areas[a];
            if area.delay > 0.0 {
                area.delay -= DT;
                if area.delay > 0.0 {
                    continue;
                }
            }
            let hit = if !area.opened {
                area.opened = true;
                area.next = tick;
                Some(damage)
            } else {
                area.next -= DT;
                if area.next <= 0.0 {
                    area.next += tick;
                    Some(tick_damage)
                } else {
                    None
                }
            };
            area.left -= DT;
            let (pos, radius, caster, done) = (area.pos, area.radius, area.caster, area.left <= 0.0);
            if let Some(amount) = hit.filter(|&d| d > 0.0) {
                self.area_hit(ab, caster, pos, radius, amount);
            }
            if done {
                ended.push(self.areas[a].id);
            }
        }
        if !ended.is_empty() {
            self.areas.retain(|a| !ended.contains(&a.id));
            for id in ended {
                self.events.push(Event::AreaEnded { id });
            }
        }
    }

    fn area_hit(&mut self, ab: usize, caster: u8, pos: V2, radius: f32, damage: f32) {
        let hits: Vec<usize> = (0..self.enemies.len())
            .filter(|&e| {
                let r = radius + self.content.enemies[self.enemies[e].kind].radius;
                self.enemies[e].hp > 0.0 && self.enemies[e].pos.distance(pos) <= r
            })
            .collect();
        for e in hits {
            self.hit_enemy(e, caster, ab, damage, pos);
        }
    }
}
