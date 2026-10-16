//! The world of a run: its heroes, its enemies and where they stand. A hero's position comes from
//! its player's client (WoW's controller moves it there, as a WoW client reports its moves to a
//! server); the world keeps it inside the map. Enemies are the host's: once the baked height map
//! is attached ([`World::attach_map`]) they are placed from the content and walk its floors
//! ([`crate::enemies`]). A client's world mirrors its host's snapshots and has no map.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::enemies::{self, Brain, Enemy};
use crate::heightmap::{HeightMap, Navigator};
use crate::{Content, V2};

/// A player, numbered from 0 (the host).
pub type PlayerId = u8;

/// Steps a second.
pub const TICK_HZ: u32 = 30;
/// Seconds a step.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// How far apart heroes who start at the same entry stand, yards.
const SPAWN_SPACING: f32 = 2.0;

/// Where a player's client put its hero, and the way it faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroPose {
    pub pos: V2,
    pub facing: V2,
}

/// What a player sends a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub pose: Option<HeroPose>,
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
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Running speed, yards a second.
    pub speed: f32,
}

/// Everything a client needs to draw the world as the host has it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub elapsed: f32,
    pub heroes: Vec<Hero>,
    /// The enemies; a player's own snapshot has those near their hero.
    pub enemies: Vec<Enemy>,
}

#[derive(Debug, PartialEq)]
pub enum WorldError {
    NoSuchClass(String),
    PlayerTaken(PlayerId),
}

pub struct World {
    content: Arc<Content>,
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
    /// Which enemy chooses its way first next step, so the path budget goes round.
    plan_cursor: usize,
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
            plan_cursor: 0,
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
        let k = self.heroes.len();
        let pos = if k == 0 {
            entry
        } else {
            let angle = (k - 1) as f32 * std::f32::consts::TAU / 4.0;
            map.bounds.clamp(entry + V2::from_angle(angle, SPAWN_SPACING))
        };
        let def = &self.content.classes[class];
        self.heroes.push(Hero {
            player,
            class,
            name: name.to_owned(),
            race,
            sex,
            pos,
            facing: V2::new(1.0, 0.0),
            hp: def.base_hp,
            max_hp: def.base_hp,
            speed: def.speed,
        });
        Ok(())
    }

    pub fn remove_hero(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.inputs.retain(|(p, _)| *p != player);
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
    }

    /// The height map, on the host once attached.
    pub fn map(&self) -> Option<&Arc<HeightMap>> {
        self.nav.as_ref().map(Navigator::map)
    }

    /// What the content asked that the map could not give, for the log.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// `player`'s input for the next step, replacing any earlier one since the last step.
    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        match self.inputs.iter_mut().find(|(p, _)| *p == player) {
            Some(slot) => slot.1 = input,
            None => self.inputs.push((player, input)),
        }
    }

    /// One step: each hero takes its player's pose, kept inside the map (a pose that is not a
    /// number is dropped, so a broken client cannot poison the world); then, once a map is
    /// attached, every enemy goes about its business.
    pub fn step(&mut self) {
        let bounds = &self.content.map.bounds;
        for (player, input) in self.inputs.drain(..) {
            let Some(pose) = input.pose else { continue };
            if !pose.pos.is_finite() || !pose.facing.is_finite() {
                continue;
            }
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.pos = bounds.clamp(pose.pos);
                let facing = pose.facing.normalize_or_zero();
                if facing != V2::ZERO {
                    hero.facing = facing;
                }
            }
        }
        if let Some(nav) = &mut self.nav {
            enemies::step(
                &mut self.enemies,
                &mut self.brains,
                nav,
                &self.content,
                &mut self.warnings,
                &mut self.plan_cursor,
            );
        }
        self.elapsed += DT;
    }

    /// The whole world, as the host has it.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            elapsed: self.elapsed,
            heroes: self.heroes.clone(),
            enemies: self.enemies.clone(),
        }
    }

    /// A client's mirror takes the host's world.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.elapsed = snapshot.elapsed;
        self.heroes = snapshot.heroes;
        self.enemies = snapshot.enemies;
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
            })
            .unwrap(),
        )
    }

    fn pose(x: f32, y: f32) -> PlayerInput {
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing: V2::new(0.0, 2.0),
            }),
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
            }),
        };
        w.set_input(0, bad_facing);
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.pos, V2::new(50.0, 50.0));
        assert!(h.facing.is_finite());
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
}
