//! The four ranks of enemy, by how tough they are (their health, in ordinary enemies, is in the
//! rules):
//!
//! - **Normal**: the horde.
//! - **Elite** ([`Enemy::elite`] set): a horde spawn now and then, and what a Challenge Idol
//!   raises. `elite_health` of its own kind, a little bigger, uses its kind's mechanics.
//! - **Rare** (also [`Enemy::elite`], and one of [`World::guards`]): a camp's guard, a named rare
//!   of the zone's own (its `rares`) with its own model and mechanics, as tough as `rare_health`
//!   of the kind two tiers above the horde, and bigger still. It keeps to its camp: it fights a
//!   hero who comes near or hurts it, and past its leash ([`CAMP_LEASH`]) for a few seconds runs
//!   home out of reach and heals, as WoW's mobs evade. Clients know one by [`EnemyState::guard`].
//! - **Boss** ([`Enemy::boss`]): the zone's boss, `boss_health` of the horde's strongest kind,
//!   `final_boss_health` for the run's last one, and at least twice an ordinary enemy's size.

use super::*;

/// A rare's size on top of an elite's, and a boss's least size against its kind's own.
pub const CAMP_SCALE: f32 = 1.25;
pub(super) const BOSS_MIN_SCALE: f32 = 2.0;
/// A camp's rare is as tough as a kind this many tiers above the strongest the horde fields now.
pub(super) const CAMP_TIERS_AHEAD: usize = 2;
/// A camp rare's gold, in ordinary enemies of its kind (an elite's is `ELITE_REWARD`); its
/// experience is one level's and it leaves a Champion's Cache.
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

    /// Camp `camp`'s rare at `pos`: as tough as an elite of a kind above the horde, and
    /// `rare_health / elite_health` times more, wearing the zone's rare for that camp (its name,
    /// model, size, pace and mechanics), guarding the spot. Its id.
    pub(super) fn spawn_camp_rare(&mut self, pos: V2, camp: usize) -> u32 {
        let kind = self.tier_ahead(CAMP_TIERS_AHEAD);
        let id = self.spawn_enemy(kind, pos, pos + V2::new(1.0, 0.0), true);
        let rules = self.content.rules.clone();
        let rares = &self.zone().rare_ix;
        let rare = (!rares.is_empty()).then(|| rares[camp % rares.len()]);
        let content = self.content.clone();
        let tougher = rules.rare_health / rules.elite_health.max(0.01);
        if let Some(e) = self.enemies.last_mut() {
            e.hp *= tougher;
            e.max_hp *= tougher;
            e.radius *= CAMP_SCALE;
            e.home = Some(pos);
            if let Some(r) = rare {
                let def = &content.enemies[r];
                e.def = r;
                e.radius = def.radius * def.scale * ELITE_SCALE * CAMP_SCALE;
                let base = (def.speed * rules.enemy_speed).min(rules.enemy_speed_cap.max(def.speed));
                e.speed = base * (1.0 + self.difficulty.enemy_speed);
            }
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
    fn elites_rares_and_bosses_have_their_ranks_health() {
        let mut w = world();
        let rules = w.content.rules.clone();
        let def = w.tier_ahead(CAMP_TIERS_AHEAD);
        let kind_hp = w.content.enemies[def].hp;
        let elite = w.spawn_enemy(def, w.center(), w.center(), true);
        let elite_hp = w.enemy(elite).unwrap().max_hp;
        assert!((elite_hp - kind_hp * rules.elite_health).abs() < 0.5, "{elite_hp}");
        let rare = w.spawn_camp_rare(w.center() + V2::new(10.0, 0.0), 0);
        let rare_hp = w.enemy(rare).unwrap().max_hp;
        assert!((rare_hp - kind_hp * rules.rare_health).abs() < 0.5, "{rare_hp}");
        assert!(rules.rare_health > rules.elite_health);
        assert!(w.snapshot().enemies.iter().any(|s| s.id == rare && s.guard));
        assert!(!w.snapshot().enemies.iter().any(|s| s.id == elite && s.guard));
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
    fn a_rare_gives_one_level_and_leaves_a_cache() {
        let mut w = world();
        let at = w.center() + V2::new(15.0, 0.0);
        let id = w.spawn_camp_rare(at, 0);
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.reap_enemies();
        let gem = w.gems.last().expect("a gem");
        assert_eq!(gem.xp, xp_to_next(w.level).ceil() as u32);
        assert!(gem.gold > 0);
        assert!(w
            .interactables
            .iter()
            .any(|i| i.kind == InteractableKind::Cache && i.pos.distance(at) < 15.0));
    }

    #[test]
    fn a_frenzied_elite_never_outruns_the_speed_cap() {
        let mut w = world();
        let kind = w.zone().enemy_ix[0];
        Arc::make_mut(&mut w.content).enemies[kind]
            .mechanics
            .push(crate::mechanics::Mechanic::Frenzy);
        let cap = w.content.rules.enemy_speed_cap;
        let id = w.spawn_enemy(kind, w.center(), w.center(), true);
        let from = w.center() + V2::new(30.0, 0.0);
        {
            let e = w.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.hp = e.max_hp * 0.3;
            e.speed = cap;
            e.pos = from;
        }
        w.move_enemies();
        let moved = w.enemy(id).unwrap().pos.distance(from);
        assert!(moved > 0.0 && moved <= cap * DT + 1e-3, "{moved}");
    }

    #[test]
    fn a_rare_dragged_past_its_leash_runs_home_and_heals() {
        let mut w = world();
        let home = w.center() + V2::new(10.0, 0.0);
        let id = w.spawn_camp_rare(home, 0);
        // Hurt, and pulled far from home by a hero standing well away.
        {
            let e = w.enemies.iter_mut().find(|e| e.id == id).unwrap();
            e.hp = e.max_hp * 0.5;
            e.pos = home + V2::new(CAMP_LEASH + 2.0, 0.0);
        }
        w.heroes[0].pos = home + V2::new(CAMP_LEASH + 6.0, 0.0);
        // A moment past the leash is forgiven...
        w.move_enemies();
        assert!(!w.evading.contains(&id));
        // ...a few seconds are not.
        for _ in 0..(LEASH_GRACE / DT) as usize + 2 {
            w.move_enemies();
        }
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
