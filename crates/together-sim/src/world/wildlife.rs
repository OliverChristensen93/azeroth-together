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
/// Steps between notice checks.
const NOTICE_EVERY: u64 = 3;
/// How near a walking creature must come to a point to have reached it.
const ARRIVE: f32 = 1.5;

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
        if self.tick % NOTICE_EVERY == 0 {
            self.notice(def);
        }
        self.give_up(def);
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

    /// Awake creatures that are not chasing notice a living hero within their range: the map's
    /// `notice`, more for creatures above the party's level and less below (never under
    /// `notice_min`), and more for each skull of the party's heat. Neutral kinds notice only a
    /// hero who walks into them.
    fn notice(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        if living.is_empty() {
            return;
        }
        let wl = &def.wildlife;
        let skulls = def.skull_index(self.party_heat()) as f32;
        let sharp = 1.0 + wl.notice_per_skull * skulls;
        let party = self.level as f32;
        let content = self.content.clone();
        let seen: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| {
                let Some(link) = &e.wild else {
                    return false;
                };
                let kind = &content.enemies[e.def];
                if e.engaged || link.returning {
                    return false;
                }
                let range = if kind.neutral {
                    HERO_RADIUS + e.radius + 0.5
                } else {
                    (wl.notice + (kind.level as f32 - party) * wl.notice_per_level)
                        .max(wl.notice_min)
                        * sharp
                };
                living.iter().any(|p| p.distance(e.pos) <= range)
            })
            .map(|e| e.id)
            .collect();
        for id in seen {
            self.engage(def, id);
        }
    }

    /// Creature `id` gives chase, and with it every idle creature of its kind within `social`,
    /// or within `help` when its kind calls for help (it shouts). Each adds the map's notice heat.
    fn engage(&mut self, def: &ExtractionDef, id: u32) {
        let Some(e) = self.enemy(id).filter(|e| !e.engaged) else {
            return;
        };
        let (kind, at) = (e.def, e.pos);
        let calls = self.content.enemies[kind].calls_help;
        let reach = if calls {
            def.wildlife.help.max(def.wildlife.social)
        } else {
            def.wildlife.social
        };
        let pack: Vec<u32> = self
            .enemies
            .iter()
            .filter(|o| {
                o.id == id
                    || (o.def == kind
                        && !o.engaged
                        && o.wild.as_ref().is_some_and(|l| !l.returning)
                        && o.pos.distance(at) <= reach)
            })
            .map(|o| o.id)
            .collect();
        if calls {
            self.events.push(Event::CalledHelp { id });
        }
        for m in pack {
            if let Some(o) = self.enemies.iter_mut().find(|o| o.id == m) {
                o.engaged = true;
                if let Some(l) = &mut o.wild {
                    l.quiet = 0.0;
                }
            }
            self.events.push(Event::Engaged { id: m });
            self.add_heat(None, def.heat.notice);
        }
    }

    /// An engaged creature that nobody has hurt, and that has hurt nobody, for `give_up`
    /// seconds, with every living hero further than `leash`, gives up and walks home.
    fn give_up(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.pos)
            .collect();
        let wl = def.wildlife;
        for e in &mut self.enemies {
            if !e.engaged {
                continue;
            }
            let Some(l) = &mut e.wild else {
                continue;
            };
            l.quiet += DT;
            if l.quiet >= wl.give_up && living.iter().all(|p| p.distance(e.pos) > wl.leash) {
                l.returning = true;
                l.quiet = 0.0;
                l.goal = None;
                e.engaged = false;
            }
        }
    }

    /// Where each enemy that is not chasing walks this step, in [`World::enemies`]' order:
    /// home for one that gave up (healed on arrival), a patrol's next point, or a wanderer's
    /// spot; `None` stands. Empty in the arena.
    pub(super) fn idle_goals(&mut self) -> Vec<Option<V2>> {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return Vec::new();
        };
        let mut goals = Vec::with_capacity(self.enemies.len());
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            let Some(link) = e.wild.clone().filter(|_| !e.engaged) else {
                goals.push(None);
                continue;
            };
            let spawn = &def.spawns[self.wild[link.slot].spawn];
            let (home, pos) = (spawn.at(), e.pos);
            let goal = if link.returning {
                if pos.distance(home) <= ARRIVE {
                    let e = &mut self.enemies[i];
                    e.hp = e.max_hp;
                    if let Some(l) = &mut e.wild {
                        l.returning = false;
                    }
                    None
                } else {
                    Some(home)
                }
            } else if !spawn.path.is_empty() {
                let point = |k: usize| V2::new(spawn.path[k][0], spawn.path[k][1]);
                let mut next = link.path_at % spawn.path.len();
                if pos.distance(point(next)) <= ARRIVE {
                    next = (next + 1) % spawn.path.len();
                }
                if let Some(l) = &mut self.enemies[i].wild {
                    l.path_at = next;
                }
                Some(point(next))
            } else if spawn.wander > 0.0 {
                match link.goal {
                    Some(g) if pos.distance(g) > ARRIVE => Some(g),
                    _ => {
                        let angle = self.rng.range(0.0, std::f32::consts::TAU);
                        let g = home + V2::from_angle(angle, self.rng.range(0.0, spawn.wander));
                        if let Some(l) = &mut self.enemies[i].wild {
                            l.goal = Some(g);
                        }
                        Some(g)
                    }
                }
            } else {
                None
            };
            goals.push(goal);
        }
        goals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::tests::test_content;
    use crate::extraction::HotZoneDef;
    use crate::world::extraction::tests::{at, secs, state_mut};

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

    #[test]
    fn a_creature_notices_a_hero_who_comes_close_and_its_pack_comes_too() {
        let mut w = forest(|d| d.heat.notice = 2.0);
        at(&mut w, 0, 138.0, 0.0);
        secs(&mut w, 0.5);
        assert!(w.enemies.iter().all(|e| !e.engaged), "12 yd: out of notice");
        let heat = w.heat_of(0);
        at(&mut w, 0, 142.0, 0.0);
        secs(&mut w, 0.2);
        let engaged: Vec<V2> = w
            .enemies
            .iter()
            .filter(|e| e.engaged)
            .map(|e| e.pos)
            .collect();
        assert_eq!(
            engaged.len(),
            2,
            "the wolf and the one beside it: {engaged:?}"
        );
        assert!(
            w.heat_of(0) >= heat + 4.0 - 0.01,
            "two noticed: {}",
            w.heat_of(0)
        );
    }

    #[test]
    fn a_kobold_calls_its_kin_from_further_off() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 0.0, -142.0);
        secs(&mut w, 0.5);
        assert_eq!(w.enemies.iter().filter(|e| e.engaged).count(), 2);
        assert!(w
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::CalledHelp { .. })));
    }

    #[test]
    fn a_neutral_creature_never_notices_but_fights_back() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 0.0, 195.0);
        secs(&mut w, 2.0);
        let boar = creature(&w, 0.0, 200.0).unwrap();
        assert!(!boar.engaged);
        let id = boar.id;
        w.damage_enemy(0, 0, id, 1.0, false);
        assert!(w.enemy(id).unwrap().engaged);
    }

    #[test]
    fn a_hero_who_walks_into_a_neutral_creature_starts_the_fight() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 0.0, 199.2);
        secs(&mut w, 0.5);
        let boar = creature(&w, 0.0, 200.0).unwrap();
        assert!(boar.engaged);
    }

    #[test]
    fn heat_and_level_change_how_far_a_creature_notices() {
        // At 5 skulls a level-1 wolf notices from 10 × 1.75 = 17.5 yd.
        let mut w = forest(|_| {});
        at(&mut w, 0, 135.0, 0.0);
        state_mut(&mut w).heat = 1000.0;
        secs(&mut w, 0.5);
        assert!(w.enemies.iter().any(|e| e.engaged && e.is_wild()));
        // A party nine levels above it: 10 − 9 = 1, held at the minimum of 5 yd.
        let mut w = forest(|_| {});
        w.level = 10;
        at(&mut w, 0, 143.0, 0.0);
        secs(&mut w, 0.5);
        assert!(!w.enemies.iter().any(|e| e.engaged && e.is_wild()));
    }

    #[test]
    fn an_engaged_creature_never_sleeps() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.damage_enemy(0, 0, id, 1.0, false);
        w.heroes[0].abilities.clear();
        at(&mut w, 0, -250.0, 0.0);
        secs(&mut w, 1.0);
        assert!(w.enemy(id).is_some_and(|e| e.engaged));
    }

    #[test]
    fn a_creature_left_behind_gives_up_walks_home_and_heals() {
        let mut w = forest(|_| {});
        w.heroes[0].abilities.clear();
        at(&mut w, 0, 142.0, 0.0);
        // Creatures wake on the half second.
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0)
            .filter(|e| e.engaged)
            .expect("noticed")
            .id;
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 1.0;
        // The hero runs off faster than a wolf can follow.
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                ..Default::default()
            },
        );
        secs(&mut w, 40.0);
        let wolf = w.enemy(id).expect("awake still");
        assert!(!wolf.engaged);
        assert!(
            wolf.pos.distance(V2::new(150.0, 0.0)) < 2.0,
            "{:?}",
            wolf.pos
        );
        assert_eq!(wolf.hp, wolf.max_hp);
    }

    #[test]
    fn a_patrolling_creature_walks_its_path() {
        let mut w = forest(|_| {});
        at(&mut w, 0, -150.0, 60.0);
        secs(&mut w, 0.5);
        let start = creature(&w, -200.0, 0.0).unwrap().pos;
        secs(&mut w, 8.0);
        let now = w.enemies.iter().find(|e| e.is_wild()).unwrap().pos;
        assert!(now.y > start.y + 5.0, "{start:?} to {now:?}");
    }

    #[test]
    fn a_mirror_draws_the_awake_forest() {
        let mut host = forest(|_| {});
        at(&mut host, 0, 100.0, 0.0);
        secs(&mut host, 0.5);
        let mut mirror = forest(|_| {});
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        let awake = host.enemies.iter().filter(|e| e.is_wild()).count();
        assert!(awake > 0);
        assert_eq!(mirror.enemies.len(), host.enemies.len());
        // What sleeps on the host goes from the mirror too.
        at(&mut host, 0, -100.0, 0.0);
        secs(&mut host, 0.5);
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        assert_eq!(mirror.enemies.len(), host.enemies.len());
    }
}
