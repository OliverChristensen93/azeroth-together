//! Champions: mini-bosses between the bosses. From the rules' `champion_from` seconds in, and
//! every `champion_every` after, one appears: an elite of a kind a few tiers above the horde's,
//! tougher still and bigger, using its kind's mechanics. It stands at a spot of its own, away
//! from the heroes, and guards it like a camp's elite: it fights whoever comes near, but past
//! its leash ([`CAMP_LEASH`]) it runs home and heals. It leaves a cache where it falls, like a
//! boss. Clients know one by [`EnemyState::champion`].

use super::*;

/// A champion's size, on top of an elite's. Its health is the rules' `champion_health` of the
/// horde's strongest.
pub const CHAMPION_SCALE: f32 = 1.3;
/// A champion is of a kind this many tiers above the strongest the horde fields now; a camp's
/// elite, this many.
const CHAMPION_TIERS_AHEAD: usize = 3;
pub(super) const CAMP_TIERS_AHEAD: usize = 2;

impl World {
    /// The health of an ordinary enemy of the horde's strongest kind now, grown for the run's
    /// progress and the party's difficulty: what champions' and bosses' health is counted in.
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

    /// Each step: a champion when one is due, near a living hero, outside the view.
    pub(super) fn spawn_champions(&mut self) {
        if self.elapsed < self.next_champion {
            return;
        }
        let living: Vec<(V2, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.pos, h.heading))
            .collect();
        if living.is_empty() {
            return;
        }
        self.next_champion = self.elapsed + self.content.rules.champion_every.max(10.0);
        let pos = self.champion_spot(&living);
        let def = self.tier_ahead(CHAMPION_TIERS_AHEAD);
        let id = self.spawn_enemy(def, pos, pos + V2::new(1.0, 0.0), true);
        let party = 1.0 + 0.6 * (living.len() - 1) as f32;
        let hp = self.content.rules.champion_health * self.horde_health() * party;
        if let Some(e) = self.enemies.last_mut() {
            e.hp = hp;
            e.max_hp = hp;
            e.radius *= CHAMPION_SCALE;
            e.home = Some(pos);
        }
        self.champions.push(id);
        self.events.push(Event::ChampionCame { id, def });
    }

    /// Where a champion stands: of a few open spots over the arena, clear of its shrines and
    /// camps, the one furthest from the nearest hero, so it is a place to go to.
    fn champion_spot(&mut self, living: &[(V2, V2)]) -> V2 {
        let content = self.content.clone();
        let zone = &content.zones[self.zone];
        let center = self.center();
        let ring = ((zone.leash * 0.25).clamp(6.0, 30.0), (zone.leash - 8.0).max(8.0));
        let mut taken: Vec<V2> = self.interactables.iter().map(|i| i.pos).collect();
        taken.extend(self.camps.iter().map(|c| c.pos));
        let mut best: Option<(V2, f32)> = None;
        for _ in 0..8 {
            let at = crate::interact::spot(zone, center, ring, &taken, &mut self.rng);
            if self.obstacles.as_ref().is_some_and(|o| !o.reachable(at)) {
                continue;
            }
            let near = living
                .iter()
                .map(|(p, _)| p.distance(at))
                .fold(f32::INFINITY, f32::min);
            if best.is_none_or(|(_, d)| near > d) {
                best = Some((at, near));
            }
        }
        best.map_or(center, |(at, _)| at)
    }

    /// Enemy `id` died at `pos`: a champion leaves a cache. Whether it was one.
    pub(super) fn champion_fell(&mut self, id: u32, pos: V2) -> bool {
        let Some(k) = self.champions.iter().position(|&c| c == id) else {
            return false;
        };
        self.champions.remove(k);
        self.place(InteractableKind::Cache, pos);
        self.events.push(Event::ChampionDied { id });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::tests::sample;

    #[test]
    fn a_champion_comes_on_time_tougher_than_an_elite_and_leaves_a_cache() {
        let mut w = World::new(Arc::new(sample()), "elwynn", 11).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.interactables.clear();
        w.enemies.clear();
        w.elapsed = w.content.rules.champion_from;
        w.spawn_champions();
        let id = *w.champions.first().expect("a champion came");
        let e = w.enemy(id).unwrap();
        assert!(e.elite);
        let want = w.content.rules.champion_health * w.horde_health();
        assert!((e.max_hp - want).abs() < 1.0, "{} vs {want}", e.max_hp);
        assert!(w
            .snapshot()
            .enemies
            .iter()
            .any(|s| s.id == id && s.champion));
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.reap_enemies();
        assert!(w.champions.is_empty());
        assert!(w
            .interactables
            .iter()
            .any(|i| i.kind == InteractableKind::Cache));
        assert!(w.drain_events().contains(&Event::ChampionDied { id }));
        assert!(w.next_champion > w.elapsed);
    }

    #[test]
    fn a_champion_dragged_past_its_leash_runs_home_and_heals() {
        let mut w = World::new(Arc::new(sample()), "elwynn", 11).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.interactables.clear();
        w.enemies.clear();
        w.elapsed = w.content.rules.champion_from;
        w.spawn_champions();
        let id = w.champions[0];
        let home = w.enemy(id).unwrap().home.unwrap();
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
