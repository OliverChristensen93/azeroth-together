//! The senses in whole worlds on small hand-made maps: heroes walking, running and sneaking;
//! enemies seeing, hearing, checking, chasing, shouting and searching.

use std::sync::Arc;

use crate::heightmap::testmap::flat;
use crate::heightmap::HeightMap;
use crate::{
    Content, ContentSources, Gait, HeroPose, Loudness, PlayerId, PlayerInput, Stance, World, V2, V3,
};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0, stealth: true),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 120.0, speed: 7.0),
]"#;
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
    entries: [(5.0, 5.0, 0.0)],
    lit: [(40.0, 40.0, 5.0)])"#;
const ENEMIES: &str = r#"[
    (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
]"#;
const RULES: &str = r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
    fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
    hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#;

/// A world on [`MAP`] (fog at 40 yd, a lit circle at (40, 40)) with `spawns`, each a RON list
/// item ending in a comma; the senses take the spec's numbers.
pub(super) fn world(spawns: &str) -> World {
    let spawns = format!("[{spawns}]");
    let content = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: RULES,
        enemies: ENEMIES,
        spawns: &spawns,
    })
    .unwrap();
    World::new(Arc::new(content))
}

/// A skeleton sentry at `(x, y)` facing `facing` degrees (0 north, 90 west), as a spawn line.
#[allow(dead_code)] // The enemies' tests of the next tasks use it.
pub(super) fn sentry(x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "skeleton", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// Flat ground 50 yards across.
#[allow(dead_code)] // The enemies' tests of the next tasks use it.
pub(super) fn open() -> Arc<HeightMap> {
    Arc::new(flat(50.0).finish())
}

/// `player`'s hero (a rogue, made if need be) at `(x, y)` on the ground, moving at `gait`, its
/// Stealth pressed `presses` times so far: the input for the next step.
pub(super) fn put(w: &mut World, player: PlayerId, x: f32, y: f32, gait: Gait, presses: u8) {
    if w.hero(player).is_none() {
        w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
    }
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            gait,
            stealth_presses: presses,
        },
    );
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

#[test]
fn each_gait_has_its_stance_and_visibility() {
    let mut w = world("");
    for (gait, stance, vis) in [
        (Gait::Still, Stance::Still, 35.0),
        (Gait::Walk, Stance::Walking, 55.0),
        (Gait::Run, Stance::Running, 100.0),
    ] {
        put(&mut w, 0, 10.0, 10.0, gait, 0);
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!((h.stance, h.visibility), (stance, vis));
    }
}

#[test]
fn a_rogue_stealths_on_a_press_and_a_mage_cannot() {
    let mut w = world("");
    w.add_hero(1, "mage", "Jaina", 1, 1).unwrap();
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 0);
    put(&mut w, 1, 20.0, 10.0, Gait::Walk, 0);
    w.step();
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 1);
    put(&mut w, 1, 20.0, 10.0, Gait::Walk, 1);
    w.step();
    let rogue = w.hero(0).unwrap();
    assert!(rogue.stealthed);
    assert_eq!(rogue.stance, Stance::Sneaking);
    assert_eq!(rogue.visibility, 20.0);
    assert!((rogue.speed - 4.9).abs() < 1e-4, "{}", rogue.speed);
    let mage = w.hero(1).unwrap();
    assert!(!mage.stealthed);
    assert_eq!(mage.speed, 7.0);
    // Still in stealth: the least visible of all.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 1);
    w.step();
    assert_eq!(w.hero(0).unwrap().visibility, 8.0);
}

#[test]
fn the_first_count_is_taken_as_it_stands() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 5);
    w.step();
    assert!(!w.hero(0).unwrap().stealthed);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 6);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
}

#[test]
fn two_presses_between_steps_cancel_and_one_is_never_lost() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    // Two inputs before a step: the second replaces the first, and its count holds both presses.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 1);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 2);
    w.step();
    assert!(!w.hero(0).unwrap().stealthed);
    // A press whose input a later one replaced is still in the later one's count.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 3);
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 3);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
    // The count goes round past 255: 3 to 255 is an even number of presses, 255 to 0 is one.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 255);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    assert!(!w.hero(0).unwrap().stealthed);
}

#[test]
fn breaking_stealth_gives_the_speed_back() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 1);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
    w.break_stealth(0);
    w.step();
    let h = w.hero(0).unwrap();
    assert!(!h.stealthed);
    assert_eq!(h.speed, 7.0);
    assert_eq!(h.stance, Stance::Still);
}

#[test]
fn a_lit_area_and_a_flash_raise_visibility() {
    let mut w = world("");
    put(&mut w, 0, 40.0, 41.0, Gait::Walk, 0);
    w.step();
    assert_eq!(w.hero(0).unwrap().visibility, 82.5);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    w.flash(0);
    w.step();
    assert_eq!(w.hero(0).unwrap().visibility, 70.0);
    steps(&mut w, 30);
    assert_eq!(w.hero(0).unwrap().visibility, 35.0);
}

#[test]
fn footsteps_are_as_loud_as_the_stance_and_the_rings_remember_a_second() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Run, 0);
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 12.0);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    steps(&mut w, 29);
    assert_eq!(w.hero(0).unwrap().noise, 12.0);
    steps(&mut w, 2);
    assert_eq!(w.hero(0).unwrap().noise, 0.0);
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 0);
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 4.0);
}

#[test]
fn a_sneaking_rogue_makes_no_footstep_noise() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    put(&mut w, 0, 10.0, 10.0, Gait::Run, 1);
    steps(&mut w, 60);
    let h = w.hero(0).unwrap();
    assert_eq!(h.stance, Stance::Sneaking);
    assert_eq!(h.noise, 0.0);
}

#[test]
fn a_made_noise_counts_for_its_maker() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    w.make_noise(0, V3::new(12.0, 10.0, 0.0), Loudness::Loud);
    // A player without a hero makes no noise.
    w.make_noise(7, V3::new(12.0, 10.0, 0.0), Loudness::Loud);
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 40.0);
}
