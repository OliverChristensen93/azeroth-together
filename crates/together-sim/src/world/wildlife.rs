//! Extraction mode's wildlife ([`crate::extraction::WildlifeDef`]): a map's creatures, kept for a
//! run by its seed, waking near living heroes and sleeping when they are gone; noticing heroes
//! who come close, with their kind; giving up and walking home when left alone; and walking
//! their patrols meanwhile. A killed one stays dead for its respawn time. The state is the
//! host's alone: the creatures travel to co-op clients as ordinary enemies.

use super::*;
use crate::extraction::ExtractionDef;

/// Steps between wake-and-sleep passes.
const WAKE_EVERY: u64 = 15;
/// Yards past `wake` every living hero must be before an idle creature sleeps again.
const SLEEP_MARGIN: f32 = 20.0;

/// One of the map's creatures kept for a run.
#[derive(Clone, Debug)]
pub(super) struct Wild {
    /// Index into the map's [`ExtractionDef::spawns`].
    pub spawn: usize,
    /// The enemy it is while awake.
    pub enemy: Option<u32>,
    /// Seconds into the run it may wake again after dying.
    pub dead_until: f32,
}

/// What an awake creature knows of itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WildLink {
    /// Index into [`World::wild`].
    pub slot: usize,
    /// The patrol point it walks toward.
    pub path_at: usize,
    /// Where a wanderer is walking.
    pub goal: Option<V2>,
    /// Seconds engaged without hurting or being hurt.
    pub quiet: f32,
    /// Gave up, walking home.
    pub returning: bool,
}

impl World {
    /// The run's share of the map's creatures, chosen by the run's seed: every one in a hot
    /// zone, none in the safe ring's margin.
    pub(super) fn keep_wildlife(&mut self) {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        for (i, s) in def.spawns.iter().enumerate() {
            let at = s.at();
            if def.in_margin(at) {
                continue;
            }
            let hot = def.hot_zone_at(at).is_some();
            if hot || self.rng.f32() < def.wildlife.share {
                self.wild.push(Wild {
                    spawn: i,
                    enemy: None,
                    dead_until: 0.0,
                });
            }
        }
    }

    /// Each step's wildlife upkeep. Nothing in the arena.
    pub(super) fn tend_wildlife(&mut self) {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        if self.tick % WAKE_EVERY == 0 {
            self.wake_and_sleep(def);
        }
    }

    /// Idle creatures with every living hero far off sleep; sleeping ones near a living hero,
    /// and not dead, wake at their spawn, up to the map's cap.
    fn wake_and_sleep(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        if living.is_empty() {
            return;
        }
        let wake = def.wildlife.wake;
        let mut sleepers = Vec::new();
        for (slot, w) in self.wild.iter().enumerate() {
            let Some(id) = w.enemy else {
                continue;
            };
            match self.enemy(id) {
                Some(e) if e.engaged => {}
                Some(e)
                    if living
                        .iter()
                        .any(|p| p.distance(e.pos) <= wake + SLEEP_MARGIN) => {}
                _ => sleepers.push((slot, id)),
            }
        }
        for (slot, id) in sleepers {
            self.enemies.retain(|e| e.id != id);
            self.wild[slot].enemy = None;
        }
        let cap = def.wildlife.max_awake as usize * living.len();
        let mut awake = self.wild.iter().filter(|w| w.enemy.is_some()).count();
        for slot in 0..self.wild.len() {
            if awake >= cap {
                break;
            }
            let w = &self.wild[slot];
            if w.enemy.is_some() || self.elapsed < w.dead_until {
                continue;
            }
            let at = def.spawns[w.spawn].at();
            if living.iter().any(|p| p.distance(at) <= wake) {
                self.spawn_wild(def, slot);
                awake += 1;
            }
        }
    }

    /// Creature `slot` awake at its spawn, tougher than the horde, not yet engaged.
    fn spawn_wild(&mut self, def: &ExtractionDef, slot: usize) {
        let spawn = &def.spawns[self.wild[slot].spawn];
        let at = spawn.at();
        let id = self.spawn_enemy(spawn.enemy_ix, at, at + V2::new(1.0, 0.0), false);
        self.toughen(id, def.wildlife.toughness, 1.0);
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
            e.engaged = false;
            e.wild = Some(WildLink {
                slot,
                ..WildLink::default()
            });
        }
        self.wild[slot].enemy = Some(id);
    }

    /// The creature in `slot` died (`None`: not a creature of the forest): it stays dead for its
    /// spawn's respawn time. What its kill pays, times.
    pub(super) fn wild_died(&mut self, slot: Option<usize>) -> f32 {
        let Some(slot) = slot else {
            return 1.0;
        };
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return 1.0;
        };
        let respawn = def.spawns[self.wild[slot].spawn].respawn * def.wildlife.respawn;
        let now = self.elapsed;
        let w = &mut self.wild[slot];
        w.enemy = None;
        w.dead_until = now + respawn;
        def.wildlife.reward
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::tests::test_content;
    use crate::extraction::HotZoneDef;
    use crate::world::extraction::tests::{at, secs};

    /// Wolves at (150, 0), (155, 5) and (180, 0); a neutral boar at (0, 200); kobolds that
    /// call for help at (0, -150) and (0, -175); a wolf patrolling from (-200, 0) to (-200, 50).
    const SPAWNS: &str = r#"[
        (map: "goldshire", spawns: [
            (enemy: "wolf", pos: (150.0, 0.0), respawn: 30.0),
            (enemy: "wolf", pos: (155.0, 5.0), respawn: 30.0),
            (enemy: "wolf", pos: (180.0, 0.0), respawn: 30.0),
            (enemy: "boar", pos: (0.0, 200.0), respawn: 30.0),
            (enemy: "kobold", pos: (0.0, -150.0), respawn: 30.0),
            (enemy: "kobold", pos: (0.0, -175.0), respawn: 30.0),
            (enemy: "wolf", pos: (-200.0, 0.0), respawn: 30.0,
             path: [(-200.0, 0.0), (-200.0, 50.0)]),
        ]),
    ]"#;

    /// A mage in the test map with every creature kept, the horde hushed, and `edit` made to the
    /// map.
    fn forest(edit: impl FnOnce(&mut ExtractionDef)) -> World {
        let mut c = test_content("").with_spawns(SPAWNS).unwrap();
        c.extractions[0].wildlife.share = 1.0;
        edit(&mut c.extractions[0]);
        let mut w = World::new(Arc::new(c), "goldshire", 3).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.spawn_timer = f32::MAX;
        w
    }

    /// The awake creature nearest `(x, y)`, within 2 yd.
    fn creature(w: &World, x: f32, y: f32) -> Option<&Enemy> {
        let at = V2::new(x, y);
        w.enemies
            .iter()
            .filter(|e| e.is_wild() && e.pos.distance(at) < 2.0)
            .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
    }

    #[test]
    fn a_creature_wakes_near_a_hero_and_sleeps_once_they_have_gone() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        assert!(creature(&w, 150.0, 0.0).is_some());
        assert!(creature(&w, -200.0, 0.0).is_none(), "too far to wake");
        at(&mut w, 0, -100.0, 0.0);
        secs(&mut w, 0.5);
        assert!(creature(&w, 150.0, 0.0).is_none(), "asleep again");
        assert!(creature(&w, -200.0, 0.0).is_some());
    }

    #[test]
    fn heroes_spells_never_seek_an_idle_creature() {
        let mut w = forest(|_| {});
        // 22 yd from the wolf at (150, 0): in Frostbolt's reach, out of its notice.
        at(&mut w, 0, 128.0, 0.0);
        secs(&mut w, 2.0);
        let wolf = creature(&w, 150.0, 0.0).unwrap();
        assert!(!wolf.engaged);
        assert_eq!(wolf.hp, wolf.max_hp);
    }

    #[test]
    fn a_hurt_creature_engages() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.damage_enemy(0, 0, id, 1.0, false);
        assert!(w.enemy(id).unwrap().engaged);
        assert!(w.drain_events().contains(&Event::Engaged { id }));
    }

    #[test]
    fn a_killed_creature_stays_dead_for_its_respawn_time() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        secs(&mut w, 25.0);
        assert!(creature(&w, 150.0, 0.0).is_none(), "still dead");
        secs(&mut w, 6.0);
        assert!(creature(&w, 150.0, 0.0).is_some(), "back after 30 s");
    }

    #[test]
    fn a_creature_of_the_forest_is_worth_more_than_the_horde() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.drain_events();
        w.step();
        let xp = w.drain_events().into_iter().find_map(|e| match e {
            Event::GemDropped { xp, .. } => Some(xp),
            _ => None,
        });
        assert_eq!(xp, Some(2), "a wolf's 1, times the wildlife's reward of 2");
    }

    #[test]
    fn no_more_than_the_cap_are_awake() {
        let mut w = forest(|d| d.wildlife.max_awake = 1);
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        assert_eq!(w.enemies.iter().filter(|e| e.is_wild()).count(), 1);
    }

    #[test]
    fn a_share_of_the_forest_is_kept_but_all_of_a_hot_zone() {
        let w = forest(|d| {
            d.wildlife.share = 0.0;
            d.hot_zones.push(HotZoneDef {
                name: "Den".into(),
                center: [160.0, 0.0],
                radius: 30.0,
            });
        });
        let kept: Vec<usize> = w.wild.iter().map(|x| x.spawn).collect();
        assert_eq!(kept, vec![0, 1, 2]);
    }

    #[test]
    fn the_horde_recycle_leaves_the_forest_to_sleep() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        // 120 to 150 yd off: past the horde's recycle reach, inside waking reach.
        at(&mut w, 0, 30.0, 0.0);
        w.step();
        assert!(creature(&w, 155.0, 5.0).is_some());
        assert!(creature(&w, 180.0, 0.0).is_some());
    }

    #[test]
    fn an_arena_enemy_is_always_engaged() {
        let mut w = World::new(Arc::new(crate::content::tests::sample()), "elwynn", 1).unwrap();
        w.add_hero(0, "mage").unwrap();
        secs(&mut w, 2.0);
        assert!(!w.enemies.is_empty());
        assert!(w.enemies.iter().all(|e| e.engaged && !e.is_wild()));
    }
}
