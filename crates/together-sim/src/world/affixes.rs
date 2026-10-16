//! Affixes (Mythic+ style): rules that join a run in its later stages, so its last minutes get
//! harder in new ways rather than only in numbers. One more joins at each of [`AFFIX_AT`], drawn
//! from those not yet in play; `TOGETHER_AFFIXES` starts a test run with some already on.
//!
//! - **Sanguine:** a dead enemy leaves a pool that heals the enemies standing in it.
//! - **Bolstering:** each death makes the enemies around it tougher and harder-hitting.
//! - **Volcanic:** fire erupts under each hero every few seconds, after a short warning.
//! - **Spiteful:** a dead enemy sometimes rises as a ghost that chases a hero for a few seconds.

use super::hazards::NO_OWNER;
use super::*;
use crate::mechanics::{Affix, Mechanic, Shape};

/// Seconds into a run at which one more affix joins.
pub const AFFIX_AT: [f32; 3] = [240.0, 480.0, 720.0];
/// Sanguine: the chance a death leaves a pool (a survivors horde dies by the dozen), its size, how
/// long it lasts, and the share of health it heals a second.
const POOL_CHANCE: f32 = 0.35;
const POOL_RADIUS: f32 = 2.5;
const POOL_SECS: f32 = 6.0;
const POOL_HEAL: f32 = 0.05;
/// Bolstering: its reach, what each death adds, and the most stacks.
const BOLSTER_REACH: f32 = 10.0;
const BOLSTER: f32 = 0.2;
const BOLSTER_MAX: u8 = 5;
/// Volcanic: seconds between eruptions, their warning, size, and the share of a hero's health
/// they take.
const VOLCANO_EVERY: f32 = 7.0;
const VOLCANO_WARNING: f32 = 1.5;
const VOLCANO_RADIUS: f32 = 2.2;
const VOLCANO_HIT: f32 = 0.15;
/// Spiteful: the chance a death leaves a ghost, its share of the dead one's health, its speed,
/// and how long it lasts.
const SPITE_CHANCE: f32 = 0.25;
const SPITE_HEALTH: f32 = 0.2;
const SPITE_SPEED: f32 = 1.3;
const SPITE_SECS: f32 = 8.0;
/// Visuals: a bolstered enemy (Battle Shout), a pool's heal (Renew), a ghost rising (Shadow
/// Bolt's impact).
const BOLSTER_VISUAL: u32 = 6673;
const POOL_VISUAL: u32 = 139;
const SPITE_VISUAL: u32 = 686;

impl World {
    /// Affix `affix` joins the run, if it is not in play already.
    pub fn add_affix(&mut self, affix: Affix) {
        if !self.affixes.contains(&affix) {
            self.affixes.push(affix);
            self.events.push(Event::AffixGained { affix });
        }
    }

    /// Each step: a new affix at each stage, eruptions under the heroes, pools healing, ghosts
    /// fading.
    pub(super) fn tend_affixes(&mut self) {
        let due = AFFIX_AT.iter().filter(|&&t| self.elapsed >= t).count();
        while self.affix_stage < due {
            self.affix_stage += 1;
            let left: Vec<Affix> = Affix::ALL
                .into_iter()
                .filter(|a| !self.affixes.contains(a))
                .collect();
            if !left.is_empty() {
                let a = left[self.rng.index(left.len())];
                self.add_affix(a);
            }
        }
        if self.affixes.contains(&Affix::Volcanic) {
            self.volcano_timer -= DT;
            if self.volcano_timer <= 0.0 {
                self.volcano_timer = VOLCANO_EVERY;
                let spots: Vec<(V2, f32)> = self
                    .heroes
                    .iter()
                    .filter(|h| h.alive)
                    .map(|h| (h.pos, h.max_hp * VOLCANO_HIT))
                    .collect();
                for (at, hit) in spots {
                    let shape = Shape::Circle {
                        radius: VOLCANO_RADIUS,
                    };
                    self.mark(
                        Mechanic::Volcanic,
                        at,
                        shape,
                        VOLCANO_WARNING,
                        NO_OWNER,
                        hit,
                    );
                }
            }
        }
        // Sanguine pools heal the enemies standing in them.
        self.pool_glow();
        let pools: Vec<(V2, Shape)> = self.pools();
        if !pools.is_empty() {
            for e in &mut self.enemies {
                if pools
                    .iter()
                    .any(|(at, shape)| shape.contains(*at, e.pos, e.radius))
                {
                    e.hp = (e.hp + e.max_hp * POOL_HEAL * DT).min(e.max_hp);
                }
            }
        }
        // Ghosts fade when their time is up, counting for nothing.
        let mut faded = Vec::new();
        for e in &mut self.enemies {
            if let Some(left) = e.act.expires.as_mut() {
                *left -= DT;
                if *left <= 0.0 {
                    faded.push(e.id);
                }
            }
        }
        if !faded.is_empty() {
            self.enemies.retain(|e| !faded.contains(&e.id));
            for id in faded {
                self.events.push(Event::EnemyDied { id });
            }
        }
    }

    /// What the affixes do when enemies die: `dead` are their positions and kinds, ghosts left
    /// out (a ghost leaves nothing behind).
    pub(super) fn affix_deaths(&mut self, dead: &[(V2, usize, f32)]) {
        if self.affixes.is_empty() {
            return;
        }
        for &(pos, def, max_hp) in dead {
            if self.affixes.contains(&Affix::Sanguine) && self.rng.f32() < POOL_CHANCE {
                let shape = Shape::Circle {
                    radius: POOL_RADIUS,
                };
                self.mark(Mechanic::Sanguine, pos, shape, POOL_SECS, NO_OWNER, 0.0);
            }
            if self.affixes.contains(&Affix::Bolstering) {
                let mut lifted = Vec::new();
                for e in self.enemies.iter_mut().filter(|e| {
                    !e.boss && e.act.bolster < BOLSTER_MAX && e.pos.distance(pos) <= BOLSTER_REACH
                }) {
                    e.act.bolster += 1;
                    e.max_hp *= 1.0 + BOLSTER;
                    e.hp *= 1.0 + BOLSTER;
                    e.damage *= 1.0 + BOLSTER;
                    lifted.push(Target::Enemy(e.id));
                }
                if let Some(Target::Enemy(first)) = lifted.first().copied() {
                    self.events.push(Event::EnemyCast {
                        enemy: first,
                        visual: BOLSTER_VISUAL,
                        targets: lifted,
                        dest: None,
                    });
                }
            }
            if self.affixes.contains(&Affix::Spiteful) && self.rng.f32() < SPITE_CHANCE {
                let toward = self
                    .heroes
                    .iter()
                    .filter(|h| h.alive)
                    .min_by(|a, b| a.pos.distance(pos).total_cmp(&b.pos.distance(pos)))
                    .map_or(pos, |h| h.pos);
                let id = self.spawn_enemy(def, pos, toward, false);
                if let Some(ghost) = self.enemies.iter_mut().find(|e| e.id == id) {
                    ghost.max_hp = max_hp * SPITE_HEALTH;
                    ghost.hp = ghost.max_hp;
                    ghost.speed *= SPITE_SPEED;
                    ghost.act.expires = Some(SPITE_SECS);
                }
                self.events.push(Event::EnemyCast {
                    enemy: id,
                    visual: SPITE_VISUAL,
                    targets: vec![Target::Enemy(id)],
                    dest: None,
                });
            }
        }
    }

    /// The Sanguine pools on the ground.
    fn pools(&self) -> Vec<(V2, Shape)> {
        self.hazard_shapes(Mechanic::Sanguine)
    }

    /// A pool's heal is shown once a second on the enemies in it.
    pub(super) fn pool_glow(&mut self) {
        if self.tick % u64::from(TICK_HZ) != 0 {
            return;
        }
        let pools = self.pools();
        let healed: Vec<Target> = self
            .enemies
            .iter()
            .filter(|e| e.hp < e.max_hp)
            .filter(|e| pools.iter().any(|(at, s)| s.contains(*at, e.pos, e.radius)))
            .map(|e| Target::Enemy(e.id))
            .collect();
        if let Some(Target::Enemy(first)) = healed.first().copied() {
            self.events.push(Event::EnemyCast {
                enemy: first,
                visual: POOL_VISUAL,
                targets: healed,
                dest: None,
            });
        }
    }
}

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

    /// A mage with no spells in an emptied arena.
    fn arena() -> World {
        let mut w = World::new(shipped(), "elwynn", 5).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.heroes[0].abilities.clear();
        w.enemies.clear();
        w.interactables.clear();
        w.camps.clear();
        w.pickups.clear();
        w.spawn_timer = f32::MAX;
        w.next_boss = f32::MAX;
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 1000.0;
        w
    }

    fn grunt(w: &mut World, dx: f32, hp: f32) -> u32 {
        let def = w.zone().enemy_ix[0];
        let at = w.heroes[0].pos + V2::new(dx, 0.0);
        let id = w.spawn_enemy(def, at, at, false);
        let e = w.enemies.iter_mut().find(|e| e.id == id).unwrap();
        e.speed = 0.0;
        e.damage = 0.0;
        e.max_hp = hp;
        e.hp = hp;
        id
    }

    fn run(w: &mut World, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            w.step();
            w.drain_events();
        }
    }

    #[test]
    fn affixes_join_at_their_stages() {
        let mut w = arena();
        w.elapsed = AFFIX_AT[1] + 1.0;
        run(&mut w, 0.1);
        assert_eq!(w.affixes.len(), 2);
    }

    #[test]
    fn a_sanguine_pool_heals_the_enemies_in_it() {
        let mut w = arena();
        w.add_affix(Affix::Sanguine);
        let near = grunt(&mut w, 21.0, 100.0);
        w.enemies.iter_mut().find(|e| e.id == near).unwrap().hp = 50.0;
        // Enough deaths beside it that one leaves a pool.
        for _ in 0..12 {
            let dying = grunt(&mut w, 20.0, 100.0);
            w.enemies.iter_mut().find(|e| e.id == dying).unwrap().hp = 0.0;
        }
        run(&mut w, 2.0);
        assert!(w.enemy(near).unwrap().hp > 50.0);
    }

    #[test]
    fn bolstering_toughens_the_survivors() {
        let mut w = arena();
        w.add_affix(Affix::Bolstering);
        let dying = grunt(&mut w, 20.0, 100.0);
        let near = grunt(&mut w, 22.0, 100.0);
        w.enemies.iter_mut().find(|e| e.id == dying).unwrap().hp = 0.0;
        run(&mut w, 0.1);
        assert!(w.enemy(near).unwrap().max_hp > 100.0);
    }

    #[test]
    fn volcanic_erupts_under_a_hero_who_stands_still() {
        let mut w = arena();
        w.add_affix(Affix::Volcanic);
        w.volcano_timer = 0.0;
        run(&mut w, VOLCANO_WARNING + 0.3);
        assert!(w.heroes[0].hp < 1000.0);
    }

    #[test]
    fn spiteful_ghosts_fade_without_a_kill() {
        let mut w = arena();
        w.add_affix(Affix::Spiteful);
        // Enough deaths that one rises.
        for i in 0..20 {
            let id = grunt(&mut w, 20.0 + i as f32, 100.0);
            w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        }
        run(&mut w, 0.1);
        let ghosts = w.enemies.iter().filter(|e| e.act.expires.is_some()).count();
        assert!(ghosts > 0);
        let kills = w.kills;
        run(&mut w, SPITE_SECS + 0.5);
        assert_eq!(
            w.enemies.iter().filter(|e| e.act.expires.is_some()).count(),
            0
        );
        assert_eq!(w.kills, kills, "a ghost fading is no kill");
    }
}
