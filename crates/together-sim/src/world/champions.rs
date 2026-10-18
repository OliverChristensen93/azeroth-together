//! Champions: mini-bosses between the bosses. From the rules' `champion_from` seconds in, and
//! every `champion_every` after, one comes: an elite of a kind a few tiers above the horde's,
//! tougher still and bigger, using its kind's mechanics. It leaves a cache where it falls, like a
//! boss. Clients know one by [`EnemyState::champion`].

use super::*;

/// A champion's health, on top of an elite's, and its size.
pub const CHAMPION_HEALTH: f32 = 3.0;
pub const CHAMPION_SCALE: f32 = 1.3;
/// A champion is of a kind this many tiers above the strongest the horde fields now; a camp's
/// elite, this many.
const CHAMPION_TIERS_AHEAD: usize = 3;
pub(super) const CAMP_TIERS_AHEAD: usize = 2;

impl World {
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
        let (near, heading) = living[self.rng.index(living.len())];
        let angle = self.spawn_angle(heading);
        let pos = self.inside(near + V2::from_angle(angle, SPAWN_RING.0));
        let def = self.tier_ahead(CHAMPION_TIERS_AHEAD);
        let id = self.spawn_enemy(def, pos, near, true);
        let party = 1.0 + 0.6 * (living.len() - 1) as f32;
        if let Some(e) = self.enemies.last_mut() {
            e.hp *= CHAMPION_HEALTH * party;
            e.max_hp *= CHAMPION_HEALTH * party;
            e.radius *= CHAMPION_SCALE;
        }
        self.champions.push(id);
        self.events.push(Event::ChampionCame { id, def });
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
        let def = e.def;
        let elite_hp = w.content.enemies[def].hp * ELITE_HEALTH;
        assert!(e.max_hp > elite_hp * 2.5, "{} vs {}", e.max_hp, elite_hp);
        assert!(w.snapshot().enemies.iter().any(|s| s.id == id && s.champion));
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
}
