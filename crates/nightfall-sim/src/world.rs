//! The world of a run: its heroes, its enemies and where they stand. A hero's position comes from
//! its player's client (WoW's controller moves it there, as a WoW client reports its moves to a
//! server); the world keeps it inside the map. Enemies are the host's: once the baked height map
//! is attached ([`World::attach_map`]) they are placed from the content and walk its floors
//! ([`crate::enemies`]). A client's world mirrors its host's snapshots and has no map.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::combat::{CastPress, Combat, Engage, Event, HeroFight, Telegraph};
use crate::enemies::{self, Brain, Enemy, Planner};
use crate::heightmap::{HeightMap, Navigator};
use crate::senses::{self, Gait, Senses, Stance};
use crate::{Content, V2};

/// Where the host keeps a hero its client reports at height `z` over `pos` (`was` its height so
/// far): on the floor nearest `z` if one is within [`HERO_FLOOR_REACH`], so a hero on a stair
/// sees from where they stand; else `z` itself (a jump, a fall), kept between [`BELOW_FLOORS`]
/// under the column's lowest floor and [`ABOVE_FLOORS`] over its highest. A column with no floor
/// at all (off every tile) keeps `was`.
fn hero_height(map: &HeightMap, pos: V2, z: f32, was: f32) -> f32 {
    if let Some(f) = map.floor_within(pos, z, HERO_FLOOR_REACH) {
        return f;
    }
    let floors = map.floors_under(pos);
    match (floors.as_slice().first(), floors.as_slice().last()) {
        (Some(&lo), Some(&hi)) => z.clamp(lo - BELOW_FLOORS, hi + ABOVE_FLOORS),
        _ => was,
    }
}

/// A player, numbered from 0 (the host).
pub type PlayerId = u8;

/// Steps a second.
pub const TICK_HZ: u32 = 30;
/// Seconds a step.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// How far apart heroes who start at the same entry stand, yards.
const SPAWN_SPACING: f32 = 2.0;
/// How far (yards) a reported height may be from a floor and still stand on it.
const HERO_FLOOR_REACH: f32 = 1.0;
/// How far under its column's lowest floor and over its highest a hero's height is kept, yards:
/// room for a fall and a jump, never a height sight could be fooled with (a client reporting
/// a hero far under the ground or over the clouds).
const BELOW_FLOORS: f32 = 2.0;
const ABOVE_FLOORS: f32 = 10.0;

/// Where a player's client put its hero, and the way it faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroPose {
    pub pos: V2,
    pub facing: V2,
    /// The height of its feet as the client's controller has it: which floor of a house the hero
    /// is on, for what it sees.
    pub z: f32,
}

/// What a player sends a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub pose: Option<HeroPose>,
    /// How the hero moves: WoW's walk toggle, and whether they move at all (spec §5.1).
    pub gait: Gait,
    /// How many times the player has pressed Stealth, counting round past 255: a count rather
    /// than a flag, so a press survives an input replaced before the host steps.
    pub stealth_presses: u8,
    /// The latest ability press, repeated until the next (spec §6.1).
    pub cast: Option<CastPress>,
    /// How many times the player has pressed dodge, counting round past 255.
    pub dodge_presses: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    pub player: PlayerId,
    /// Index into [`Content::classes`].
    pub class: usize,
    pub name: String,
    pub race: u8,
    pub sex: u8,
    pub pos: V2,
    /// The floor it stands on (the reported height while no floor is near it, a jump, kept within
    /// 2 yd under and 10 yd over its column's floors).
    pub z: f32,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Running speed, yards a second.
    pub speed: f32,
    /// How they stand and move (spec §5.1).
    pub stance: Stance,
    /// In stealth (spec §6.3).
    pub stealthed: bool,
    /// How visible they are, 0..100 (spec §5.1).
    pub visibility: f32,
    /// The loudest noise they made in the last `noise_memory` seconds, yards: the rings by the
    /// player frame (spec §5.5).
    pub noise: f32,
    /// Their side of the fight (spec §6).
    pub fight: HeroFight,
}

/// Everything a client needs to draw the world as the host has it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub elapsed: f32,
    pub heroes: Vec<Hero>,
    /// The enemies; a player's own snapshot has those near their hero.
    pub enemies: Vec<Enemy>,
    /// The telegraphed blows about to land; a player's own snapshot has those near their hero.
    pub telegraphs: Vec<Telegraph>,
}

#[derive(Debug, PartialEq)]
pub enum WorldError {
    NoSuchClass(String),
    PlayerTaken(PlayerId),
}

pub struct World {
    pub(crate) content: Arc<Content>,
    pub heroes: Vec<Hero>,
    /// The host's, placed when a map is attached; a client's, from its host's snapshots.
    pub enemies: Vec<Enemy>,
    /// Seconds since the run began.
    pub elapsed: f32,
    /// Each player's latest input since the last step.
    inputs: Vec<(PlayerId, PlayerInput)>,
    /// What each enemy means to do, beside it in `enemies`: the host's alone, never sent.
    brains: Vec<Brain>,
    /// The height map and its paths, once attached: the host's alone.
    nav: Option<Navigator>,
    /// What the content asked that the map could not give (a patrol point no path reaches).
    warnings: Vec<String>,
    /// What the enemies share in choosing their ways (the path budget, the points found sealed).
    planner: Planner,
    /// Each hero's senses and each enemy's mind: the host's alone, never sent.
    pub(crate) senses: Senses,
    /// The telegraphed blows about to land (spec §6.1), as every client draws them.
    pub telegraphs: Vec<Telegraph>,
    /// What happened in the steps since they were last drained (host) or pushed (mirror).
    pub(crate) events: Vec<Event>,
    /// The fight's host-only state.
    pub(crate) combat: Combat,
}

impl World {
    /// An empty run on the content's map.
    pub fn new(content: Arc<Content>) -> Self {
        Self {
            content,
            heroes: Vec::new(),
            enemies: Vec::new(),
            elapsed: 0.0,
            inputs: Vec::new(),
            brains: Vec::new(),
            nav: None,
            warnings: Vec::new(),
            planner: Planner::default(),
            senses: Senses::default(),
            telegraphs: Vec::new(),
            events: Vec::new(),
            combat: Combat::default(),
        }
    }

    pub fn content(&self) -> &Arc<Content> {
        &self.content
    }

    /// `player`'s hero, of the class with `class_key`, at the first entry; each later hero stands
    /// [`SPAWN_SPACING`] yards from it, around a circle.
    pub fn add_hero(
        &mut self,
        player: PlayerId,
        class_key: &str,
        name: &str,
        race: u8,
        sex: u8,
    ) -> Result<(), WorldError> {
        let class = self
            .content
            .class(class_key)
            .ok_or_else(|| WorldError::NoSuchClass(class_key.to_owned()))?;
        if self.hero(player).is_some() {
            return Err(WorldError::PlayerTaken(player));
        }
        let map = &self.content.map;
        let entry = V2::new(map.entries[0][0], map.entries[0][1]);
        let z = map.entries[0][2];
        let k = self.heroes.len();
        let pos = if k == 0 {
            entry
        } else {
            let angle = (k - 1) as f32 * std::f32::consts::TAU / 4.0;
            map.bounds
                .clamp(entry + V2::from_angle(angle, SPAWN_SPACING))
        };
        let def = &self.content.classes[class];
        self.heroes.push(Hero {
            player,
            class,
            name: name.to_owned(),
            race,
            sex,
            pos,
            z,
            facing: V2::new(1.0, 0.0),
            hp: def.base_hp,
            max_hp: def.base_hp,
            speed: def.speed,
            stance: Stance::Still,
            stealthed: false,
            visibility: self.content.rules.senses.visibility.still,
            noise: 0.0,
            fight: HeroFight::new(def, &self.content.rules.combat),
        });
        Ok(())
    }

    pub fn remove_hero(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.inputs.retain(|(p, _)| *p != player);
        self.senses.forget(player);
        self.combat.forget(player);
    }

    pub fn hero(&self, player: PlayerId) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.player == player)
    }

    pub fn enemy(&self, id: u32) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.id == id)
    }

    /// The host's height map: from now on the world knows walls and floors, and the content's
    /// enemies are placed on them (any placed before start over).
    pub fn attach_map(&mut self, map: Arc<HeightMap>) {
        let (enemies, brains, warnings) = enemies::spawn_all(&self.content, &map);
        self.enemies = enemies;
        self.brains = brains;
        self.warnings.extend(warnings);
        self.nav = Some(Navigator::new(map));
        // Every enemy starts calm, with the eyes its behaviour gives it.
        self.senses
            .place(&self.content, self.brains.iter().map(Brain::spawn));
        self.combat.place(&self.content, &self.enemies);
    }

    /// The height map, on the host once attached.
    pub fn map(&self) -> Option<&Arc<HeightMap>> {
        self.nav.as_ref().map(Navigator::map)
    }

    /// What the content asked that the map could not give, for the log.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// How many path searches have found nothing so far (a sealed patrol point or post), for the
    /// log and for tests: a content mistake must cost a few searches, not one a step.
    pub fn failed_searches(&self) -> u64 {
        self.planner.failed_searches()
    }

    /// Floors every path search has looked at so far, for the log and for tests: what path
    /// finding costs, counted rather than timed.
    pub fn floors_looked_at(&self) -> u64 {
        self.nav.as_ref().map_or(0, Navigator::floors_looked_at)
    }

    /// `player`'s input for the next step, replacing any earlier one since the last step.
    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        match self.inputs.iter_mut().find(|(p, _)| *p == player) {
            Some(slot) => slot.1 = input,
            None => self.inputs.push((player, input)),
        }
    }

    /// One step: each hero takes its player's input (the gait and Stealth presses first, then the
    /// pose, kept inside the map; a pose that is not a number is dropped, so a broken client
    /// cannot poison the world), then the presses of the fight; every hero's senses are worked out
    /// and their fight moves on; once a map is attached, every enemy looks, listens, thinks and
    /// goes about its business.
    pub fn step(&mut self) {
        let bounds = &self.content.map.bounds;
        let inputs = std::mem::take(&mut self.inputs);
        for (player, input) in &inputs {
            let may_stealth = self.may_stealth(*player);
            let why = self.stealth_refusal(*player);
            let Some(hero) = self.heroes.iter_mut().find(|h| h.player == *player) else {
                continue;
            };
            if senses::take_input(&mut self.senses, hero, input, &self.content, may_stealth) {
                self.events.push(crate::Event::Refused {
                    player: *player,
                    why,
                });
            }
            let Some(pose) = input.pose else { continue };
            if !pose.pos.is_finite() || !pose.facing.is_finite() {
                continue;
            }
            hero.pos = bounds.clamp(pose.pos);
            if pose.z.is_finite() {
                let map = self.nav.as_ref().map(Navigator::map);
                hero.z = map.map_or(pose.z, |map| hero_height(map, hero.pos, pose.z, hero.z));
            }
            let facing = pose.facing.normalize_or_zero();
            if facing != V2::ZERO {
                hero.facing = facing;
            }
        }
        for (player, input) in &inputs {
            self.take_presses(*player, input);
        }
        senses::step_heroes(&mut self.senses, &mut self.heroes, &self.content);
        self.step_fighters();
        // Holds, slows and burns tick before the senses read what each enemy may do (a client's
        // mirror has no map, and no fight to run).
        if self.nav.is_some() {
            self.hold_enemies();
            // Before the senses listen, so the noise of these blows is heard this step.
            self.strike_back();
        }
        // Their pace last: a blow just struck broke stealth, which works the senses' speed out
        // afresh, so the Sprint, the slowed cast and the crawl are laid on after it.
        for hi in 0..self.heroes.len() {
            self.set_pace(hi);
        }
        // Enemies and brains are kept side by side; were they not (a snapshot taken by a world
        // with a map, caught in debug builds), the enemies stand still rather than index wrongly.
        if let Some(nav) = self
            .nav
            .as_mut()
            .filter(|_| self.brains.len() == self.enemies.len())
        {
            // One budget of floors for the whole step: the senses' searches (chasing, checking,
            // searching) come first, the orders' (posts, patrols, walks) take what is left.
            nav.begin_step(enemies::STEP_FLOORS);
            let engage: Vec<Engage> = self.combat.enemies.iter().map(|c| c.engage).collect();
            senses::step_enemies(
                &mut self.senses,
                &mut self.enemies,
                &mut self.brains,
                &self.heroes,
                nav,
                &self.content,
                &engage,
            );
            enemies::step_planned(
                &mut self.enemies,
                &mut self.brains,
                nav,
                &self.content,
                &mut self.warnings,
                &mut self.planner,
            );
        }
        // Noises are heard the step they are made, or the next; none waits longer.
        self.senses.noises.clear();
        self.elapsed += DT;
    }

    /// The whole world, as the host has it.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            elapsed: self.elapsed,
            heroes: self.heroes.clone(),
            enemies: self.enemies.clone(),
            telegraphs: self.telegraphs.clone(),
        }
    }

    /// A client's mirror takes the host's world. A mirror must not have a map attached: the
    /// snapshot's enemies would have no brains beside them.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        debug_assert!(
            self.nav.is_none(),
            "a world with a map is a host; it takes no snapshots"
        );
        self.elapsed = snapshot.elapsed;
        self.heroes = snapshot.heroes;
        self.enemies = snapshot.enemies;
        self.telegraphs = snapshot.telegraphs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentSources;

    fn content() -> Arc<Content> {
        Arc::new(
            Content::from_ron(ContentSources {
                classes: r#"[
                    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
                    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 120.0, speed: 7.0),
                ]"#,
                map: r#"(key: "test", name: "Test", map: 0,
                    bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                    entries: [(50.0, 50.0, 10.0)])"#,
                rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                    fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                    hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
                enemies: "[]",
                spawns: "[]",
                abilities: "[]",
            })
            .unwrap(),
        )
    }

    fn pose(x: f32, y: f32) -> PlayerInput {
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing: V2::new(0.0, 2.0),
                z: 10.0,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn heroes_start_at_the_entry_a_little_apart() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "Valeera", 1, 0).unwrap();
        w.add_hero(1, "mage", "Jaina", 1, 1).unwrap();
        let a = w.hero(0).unwrap();
        let b = w.hero(1).unwrap();
        assert_eq!(a.pos, V2::new(50.0, 50.0));
        assert!((a.pos.distance(b.pos) - SPAWN_SPACING).abs() < 1e-4);
        assert_eq!(a.hp, 135.0);
        assert_eq!(b.max_hp, 120.0);
        assert_eq!(b.name, "Jaina");
    }

    #[test]
    fn an_unknown_class_or_a_taken_player_is_refused() {
        let mut w = World::new(content());
        assert_eq!(
            w.add_hero(0, "warlock", "X", 1, 0),
            Err(WorldError::NoSuchClass("warlock".into()))
        );
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        assert_eq!(
            w.add_hero(0, "mage", "B", 1, 1),
            Err(WorldError::PlayerTaken(0))
        );
    }

    #[test]
    fn a_step_takes_the_pose_and_normalizes_the_facing() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(60.0, 40.0));
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.pos, V2::new(60.0, 40.0));
        assert_eq!(h.facing, V2::new(0.0, 1.0));
        assert!((w.elapsed - DT).abs() < 1e-6);
    }

    #[test]
    fn a_pose_outside_the_map_is_kept_inside() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(130.0, 40.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(100.0, 40.0));
    }

    #[test]
    fn a_pose_that_is_not_a_number_is_ignored() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(f32::NAN, 40.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(50.0, 50.0));
        let bad_facing = PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(55.0, 50.0),
                facing: V2::new(f32::INFINITY, 0.0),
                z: 10.0,
            }),
            ..Default::default()
        };
        w.set_input(0, bad_facing);
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.pos, V2::new(50.0, 50.0));
        assert!(h.facing.is_finite());
    }

    #[test]
    fn a_hero_stands_on_the_floor_near_the_height_its_client_reports() {
        let mut w = World::new(content());
        w.attach_map(Arc::new(crate::heightmap::testmap::flat(100.0).finish()));
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        assert_eq!(w.hero(0).unwrap().z, 10.0);
        let at = |z: f32| PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(60.0, 40.0),
                facing: V2::new(1.0, 0.0),
                z,
            }),
            ..Default::default()
        };
        w.set_input(0, at(0.3));
        w.step();
        assert_eq!(w.hero(0).unwrap().z, 0.0);
        // Mid-jump, no floor within a yard: the reported height stands.
        w.set_input(0, at(5.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().z, 5.0);
        w.set_input(0, at(f32::NAN));
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.z, 5.0);
        assert_eq!(h.pos, V2::new(60.0, 40.0));
    }

    #[test]
    fn without_a_map_a_hero_takes_the_reported_height() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(
            0,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: V2::new(60.0, 40.0),
                    facing: V2::new(1.0, 0.0),
                    z: 12.5,
                }),
                ..Default::default()
            },
        );
        w.step();
        assert_eq!(w.hero(0).unwrap().z, 12.5);
    }

    #[test]
    fn no_input_leaves_a_hero_where_it_stands() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(60.0, 40.0));
        w.step();
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(60.0, 40.0));
    }

    #[test]
    fn the_latest_input_before_a_step_wins() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(60.0, 40.0));
        w.set_input(0, pose(70.0, 40.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(70.0, 40.0));
    }

    #[test]
    fn input_for_a_player_without_a_hero_does_nothing() {
        let mut w = World::new(content());
        w.set_input(3, pose(60.0, 40.0));
        w.step();
        assert!(w.heroes.is_empty());
    }

    #[test]
    fn a_snapshot_rebuilds_the_world() {
        let mut host = World::new(content());
        host.add_hero(0, "rogue", "A", 1, 0).unwrap();
        host.add_hero(1, "mage", "B", 1, 1).unwrap();
        host.set_input(1, pose(20.0, 30.0));
        host.step();
        let mut mirror = World::new(content());
        mirror.apply_snapshot(host.snapshot());
        assert_eq!(mirror.snapshot(), host.snapshot());
    }

    #[test]
    fn the_same_inputs_give_the_same_world() {
        let run = || {
            let mut w = World::new(content());
            w.add_hero(0, "rogue", "A", 1, 0).unwrap();
            for i in 0..90 {
                w.set_input(0, pose(50.0 + i as f32, 50.0));
                w.step();
            }
            w.snapshot()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_hero_who_leaves_is_gone() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.add_hero(1, "mage", "B", 1, 1).unwrap();
        w.remove_hero(1);
        assert!(w.hero(1).is_none());
        assert!(w.hero(0).is_some());
    }

    #[test]
    fn a_hero_reported_far_under_or_over_the_ground_is_kept_near_its_floors() {
        let mut b = crate::heightmap::testmap::flat(100.0);
        // A second storey over part of the map: floors at 0 and 3.2.
        b.add_box(V2::new(70.0, 30.0), V2::new(80.0, 50.0), 3.0, 3.2);
        let mut w = World::new(content());
        w.attach_map(Arc::new(b.finish()));
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        let at = |x: f32, z: f32| PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, 40.0),
                facing: V2::new(1.0, 0.0),
                z,
            }),
            ..Default::default()
        };
        for (x, z, kept) in [
            (60.0, -500.0, -2.0),
            (60.0, 1e30, 10.0),
            (60.0, f32::MAX, 10.0),
            (60.0, 6.0, 6.0),
            (75.0, -500.0, -2.0),
            (75.0, 1e30, 13.2),
            (75.0, 12.0, 12.0),
        ] {
            w.set_input(0, at(x, z));
            w.step();
            assert_eq!(w.hero(0).unwrap().z, kept, "reported {z} at x {x}");
        }
    }
}
