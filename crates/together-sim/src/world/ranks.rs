//! The four ranks of enemy, by how tough they are (their health, in ordinary enemies, is in the
//! rules):
//!
//! - **Normal**: the horde.
//! - **Rare** ([`Enemy::elite`] set): a horde spawn now and then, and what a Challenge Idol
//!   raises. `rare_health` of its own kind, a little bigger, uses its kind's mechanics.
//! - **Elite** (also [`Enemy::elite`], and one of [`World::guards`]): a camp's guard. A kind two
//!   tiers above the horde, `elite_health` of it, bigger still. It keeps to its camp: it fights
//!   a hero who comes near or hurts it, and past its leash ([`CAMP_LEASH`]) runs home out of reach
//!   and heals, as WoW's mobs evade. Clients know one by [`EnemyState::guard`].
//! - **Boss** ([`Enemy::boss`]): the zone's boss, `boss_health` of the horde's strongest kind,
//!   `final_boss_health` for the run's last one, and at least twice an ordinary enemy's size.

use super::*;

/// An elite's size on top of a rare's, and a boss's least size against its kind's own.
pub const CAMP_SCALE: f32 = 1.25;
pub(super) const BOSS_MIN_SCALE: f32 = 2.0;
/// A camp's elite is of a kind this many tiers above the strongest the horde fields now.
pub(super) const CAMP_TIERS_AHEAD: usize = 2;
/// A camp elite's experience and gold, in ordinary enemies of its kind (a rare's is
/// `ELITE_REWARD`).
pub(super) const CAMP_REWARD: u32 = 10;

impl World {
    /// The health of an ordinary enemy of the horde's strongest kind now, grown for the run's
    /// progress and the party's difficulty: what bosses' health is counted in.
    pub(super) fn horde_health(&self) -> f32 {
        let (_, top) = self.tiers();
        let def = &self.content.enemies[self.zone().enemy_ix[top]];
        let progress = (self.elapsed / RUN_SECS).min(1.0);
        def.hp * (1.0 + progress * 0.75) * (1.0 + self.difficulty.enemy_health)
    }

    /// The zone's enemy kind `ahead` tiers above the horde's strongest now, held to its roster.
    pub(super) fn tier_ahead(&self, ahead: usize) -> usize {
        let (_, top) = self.tiers();
        let roster = &self.zone().enemy_ix;
        roster[(top + ahead).min(roster.len() - 1)]
    }

    /// A camp's elite at `pos`: a rare of a kind above the horde, made an elite (tougher and
    /// bigger) that guards the spot. Its id.
    pub(super) fn spawn_camp_elite(&mut self, pos: V2) -> u32 {
        let def = self.tier_ahead(CAMP_TIERS_AHEAD);
        let id = self.spawn_enemy(def, pos, pos + V2::new(1.0, 0.0), true);
        let rules = &self.content.rules;
        let tougher = rules.elite_health / rules.rare_health.max(0.01);
        if let Some(e) = self.enemies.last_mut() {
            e.hp *= tougher;
            e.max_hp *= tougher;
            e.radius *= CAMP_SCALE;
            e.home = Some(pos);
        }
        self.guards.push(id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::sample;

    fn world() -> World {
        let mut w = World::new(Arc::new(sample()), "elwynn", 11).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.interactables.clear();
        w.enemies.clear();
        w.guards.clear();
        w
    }

    #[test]
    fn rares_elites_and_bosses_have_their_ranks_health() {
        let mut w = world();
        let rules = w.content.rules.clone();
        let def = w.tier_ahead(CAMP_TIERS_AHEAD);
        let kind_hp = w.content.enemies[def].hp;
        let rare = w.spawn_enemy(def, w.center(), w.center(), true);
        let rare_hp = w.enemy(rare).unwrap().max_hp;
        assert!((rare_hp - kind_hp * rules.rare_health).abs() < 0.5, "{rare_hp}");
        let elite = w.spawn_camp_elite(w.center() + V2::new(10.0, 0.0));
        let elite_hp = w.enemy(elite).unwrap().max_hp;
        assert!((elite_hp - kind_hp * rules.elite_health).abs() < 0.5, "{elite_hp}");
        assert!(w.snapshot().enemies.iter().any(|s| s.id == elite && s.guard));
        assert!(!w.snapshot().enemies.iter().any(|s| s.id == rare && s.guard));
    }

    #[test]
    fn the_last_boss_is_the_toughest() {
        let mut w = world();
        let rules = w.content.rules.clone();
        for n in 1..=rules.final_boss {
            w.enemies.clear();
            w.next_boss = w.elapsed;
            w.spawn_bosses();
            let boss = w.enemies.iter().find(|e| e.boss).expect("a boss came");
            let want = if n == rules.final_boss {
                rules.final_boss_health
            } else {
                rules.boss_health
            } * w.horde_health();
            assert!((boss.max_hp - want).abs() < 1.0, "boss {n}: {} vs {want}", boss.max_hp);
            assert!(boss.radius >= w.content.enemies[boss.def].radius * BOSS_MIN_SCALE - 1e-4);
        }
    }

    #[test]
    fn an_elite_dragged_past_its_leash_runs_home_and_heals() {
        let mut w = world();
        let home = w.center() + V2::new(10.0, 0.0);
        let id = w.spawn_camp_elite(home);
        // Hurt, and pulled far from home by a hero standing well away.
        {
            let e = w.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.hp = e.max_hp * 0.5;
            e.pos = home + V2::new(CAMP_LEASH + 2.0, 0.0);
        }
        w.heroes[0].pos = home + V2::new(CAMP_LEASH + 6.0, 0.0);
        w.move_enemies();
        assert!(w.evading.contains(&id));
        // Out of reach on the way home.
        let hp = w.enemy(id).unwrap().hp;
        w.damage_enemy(0, 0, id, 50.0, false);
        assert_eq!(w.enemy(id).unwrap().hp, hp);
        for _ in 0..600 {
            w.move_enemies();
        }
        let e = w.enemy(id).unwrap();
        assert!(e.pos.distance(home) <= 1.5, "{:?} vs {home:?}", e.pos);
        assert_eq!(e.hp, e.max_hp);
        assert!(!w.evading.contains(&id));
    }
}
