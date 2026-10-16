//! The senses in whole worlds on small hand-made maps: heroes walking, running and sneaking;
//! enemies seeing, hearing, checking, chasing, shouting and searching.

use std::sync::Arc;

use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Alert, Content, ContentSources, Gait, HeroPose, Loudness, Mark, PlayerId, PlayerInput, Stance,
    World, V2, V3,
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
pub(super) fn sentry(x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "skeleton", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// Flat ground 50 yards across.
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

#[test]
fn a_walking_hero_in_front_of_a_sentry_is_spotted_then_chased() {
    let mut w = world(&sentry(25.0, 25.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 37.0, 25.0, Gait::Walk, 0);
    let mut was_suspicious = false;
    for _ in 0..300 {
        w.step();
        was_suspicious |= w.marks_for(0)
            == [Mark {
                enemy: 1,
                alert: Alert::Suspicious,
            }];
    }
    assert!(was_suspicious);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Alerted
        }]
    );
    let e = &w.enemies[0];
    let to_hero = V2::new(37.0, 25.0) - e.pos;
    assert!(to_hero.length() <= 2.3, "{:?}", e.pos);
    assert!(
        e.facing.dot(to_hero.normalize_or_zero()) > 0.9,
        "{:?}",
        e.facing
    );
}

#[test]
fn a_sneaking_rogue_behind_a_sentry_goes_unseen_and_a_walker_does_not() {
    let marks = |presses: u8| {
        let mut w = world(&sentry(25.0, 25.0, 0.0));
        w.attach_map(open());
        put(&mut w, 0, 20.5, 25.0, Gait::Walk, 0);
        w.step();
        put(&mut w, 0, 20.5, 25.0, Gait::Walk, presses);
        steps(&mut w, 300);
        (w.marks_for(0), w.enemies[0].pos)
    };
    let (sneaking, at) = marks(1);
    assert!(sneaking.is_empty(), "{sneaking:?}");
    assert_eq!(at, V2::new(25.0, 25.0));
    let (walking, _) = marks(0);
    assert!(!walking.is_empty());
}

#[test]
fn through_a_wall_running_is_heard_at_half_the_distance() {
    let run_behind_wall = |x: f32| {
        let mut w = world(&sentry(20.0, 25.0, 180.0));
        let mut b = flat(50.0);
        b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
        w.attach_map(Arc::new(b.finish()));
        put(&mut w, 0, x, 25.0, Gait::Run, 0);
        steps(&mut w, 45);
        (w.marks_for(0), w.enemies[0].pos)
    };
    let (near, at) = run_behind_wall(25.5);
    assert!(!near.is_empty());
    assert!(at.x < 24.0, "it never walks through the wall: {at:?}");
    let (far, _) = run_behind_wall(29.0);
    assert!(far.is_empty(), "{far:?}");
}

#[test]
fn an_enemy_goes_to_check_the_noise_not_the_hero() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 45.0, 45.0, Gait::Still, 0);
    w.step();
    let at = V3::new(14.0, 14.0, 0.0);
    w.make_noise(0, at, Loudness::Normal);
    w.step();
    w.make_noise(0, at, Loudness::Normal);
    steps(&mut w, 75);
    let e = &w.enemies[0];
    assert!(e.pos.distance(V2::new(14.0, 14.0)) <= 1.0, "{:?}", e.pos);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Suspicious
        }]
    );
}

#[test]
fn a_shout_brings_allies_within_reach_and_not_beyond() {
    // A sees the hero; B is 12 yards from A, C 22 from A but only 10 from B.
    let spawns = format!(
        "{}{}{}",
        sentry(10.0, 25.0, 0.0),
        sentry(10.0, 37.0, 180.0),
        sentry(10.0, 47.0, 180.0)
    );
    let mut w = world(&spawns);
    w.attach_map(open());
    put(&mut w, 0, 16.0, 25.0, Gait::Walk, 0);
    let alerted = |w: &World, enemy: u32| {
        w.marks_for(0).contains(&Mark {
            enemy,
            alert: Alert::Alerted,
        })
    };
    let mut a = false;
    for _ in 0..300 {
        w.step();
        if alerted(&w, 1) {
            a = true;
            break;
        }
    }
    assert!(a);
    assert!(alerted(&w, 2), "{:?}", w.marks_for(0));
    assert!(
        !w.marks_for(0).iter().any(|m| m.enemy == 3),
        "a shout does not chain: {:?}",
        w.marks_for(0)
    );
    // Nor a step later, when B has had its turn.
    steps(&mut w, 2);
    assert!(
        !w.marks_for(0).iter().any(|m| m.enemy == 3),
        "{:?}",
        w.marks_for(0)
    );
}

#[test]
fn a_hit_alert_shouts_through_the_world_to_calm_allies_only() {
    // Sentries 5 yd apart at y 10, 15 and 20, one far off at 45; both heroes far from all.
    let spawns = format!(
        "{}{}{}{}",
        sentry(10.0, 10.0, 0.0),
        sentry(10.0, 15.0, 0.0),
        sentry(10.0, 20.0, 0.0),
        sentry(10.0, 45.0, 0.0)
    );
    let alerted_to = |w: &World, p: u8| -> Vec<u32> {
        w.marks_for(p)
            .into_iter()
            .filter(|m| m.alert == Alert::Alerted)
            .map(|m| m.enemy)
            .collect()
    };
    let start = || {
        let mut w = world(&spawns);
        w.attach_map(open());
        put(&mut w, 0, 45.0, 45.0, Gait::Still, 0);
        put(&mut w, 1, 46.0, 46.0, Gait::Still, 0);
        w.step();
        w
    };
    // The first is hit by the first hero: it is alerted at once, its allies within 15 yd at the
    // next step (the alert itself shouts nothing), the one beyond not at all.
    let mut w = start();
    w.alert(1, 0);
    assert_eq!(alerted_to(&w, 0), vec![1]);
    w.step();
    assert_eq!(alerted_to(&w, 0), vec![1, 2, 3]);
    steps(&mut w, 3);
    assert_eq!(alerted_to(&w, 0), vec![1, 2, 3]);
    // An ally already after the other hero stays after them, and so does the shout's maker.
    let mut w = start();
    w.alert(2, 1);
    w.alert(1, 0);
    w.step();
    assert_eq!(alerted_to(&w, 0), vec![1]);
    assert_eq!(alerted_to(&w, 1), vec![2, 3]);
    // An enemy or a hero that is not there changes nothing.
    w.alert(9, 0);
    w.alert(1, 7);
    steps(&mut w, 3);
    assert_eq!(alerted_to(&w, 0), vec![1]);
    assert_eq!(alerted_to(&w, 1), vec![2, 3]);
}

#[test]
fn a_lost_hero_is_searched_for_then_the_sentry_goes_home() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    let mut b = flat(50.0);
    house(&mut b, V2::new(30.0, 30.0), V2::new(40.0, 40.0), None, true);
    w.attach_map(Arc::new(b.finish()));
    put(&mut w, 0, 20.0, 10.0, Gait::Run, 0);
    let alerted = [Mark {
        enemy: 1,
        alert: Alert::Alerted,
    }];
    let mut was_alerted = false;
    for _ in 0..150 {
        w.step();
        if w.marks_for(0) == alerted {
            was_alerted = true;
            break;
        }
    }
    assert!(was_alerted);
    // The hero slips into a closed, roofed house: out of every sight and every path.
    put(&mut w, 0, 35.0, 35.0, Gait::Still, 0);
    let searching = [Mark {
        enemy: 1,
        alert: Alert::Suspicious,
    }];
    let mut searched_there = false;
    for _ in 0..900 {
        w.step();
        searched_there |=
            w.marks_for(0) == searching && w.enemies[0].pos.distance(V2::new(20.0, 10.0)) <= 1.0;
    }
    assert!(searched_there);
    assert!(w.marks_for(0).is_empty());
    let e = &w.enemies[0];
    assert!(e.pos.distance(V2::new(10.0, 10.0)) <= 0.6, "{:?}", e.pos);
    assert!(e.facing.x > 0.99, "{:?}", e.facing);
}

#[test]
fn a_place_no_path_reaches_is_searched_from_where_it_stands() {
    let mut w = world(&sentry(20.0, 25.0, 0.0));
    let mut b = flat(50.0);
    b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    put(&mut w, 0, 30.0, 25.0, Gait::Still, 0);
    w.step();
    w.alert(1, 0);
    w.step();
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Alerted
        }]
    );
    // Five seconds unseen, fifteen looking around where it stands, then home.
    steps(&mut w, 900);
    assert!(w.marks_for(0).is_empty());
    let e = &w.enemies[0];
    assert!(e.pos.x < 24.0, "{:?}", e.pos);
    assert!(e.pos.distance(V2::new(20.0, 25.0)) <= 0.6, "{:?}", e.pos);
}

#[test]
fn a_sleeper_sleeps_through_a_hero_in_front_and_wakes_to_noise() {
    let mut w = world(
        r#"(place: "Crypt", kind: "skeleton", behaviour: Sleeper(at: (10.0, 10.0), facing: 0.0)),"#,
    );
    w.attach_map(open());
    put(&mut w, 0, 13.0, 10.0, Gait::Still, 0);
    steps(&mut w, 90);
    assert!(w.marks_for(0).is_empty());
    assert!(w.enemies[0].asleep);
    for _ in 0..3 {
        w.make_noise(0, V3::new(13.0, 10.0, 0.0), Loudness::Normal);
        w.step();
    }
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Alerted
        }]
    );
    assert!(!w.enemies[0].asleep);
}

#[test]
fn a_hero_who_leaves_mid_chase_is_forgotten() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 20.0, 10.0, Gait::Still, 0);
    put(&mut w, 1, 5.0, 40.0, Gait::Still, 0);
    w.step();
    w.alert(1, 0);
    steps(&mut w, 10);
    w.remove_hero(0);
    steps(&mut w, 300);
    assert!(w.marks_for(0).is_empty());
    assert!(w.marks_for(1).is_empty());
    let e = &w.enemies[0];
    assert!(e.pos.distance(V2::new(10.0, 10.0)) <= 0.6, "{:?}", e.pos);
}

#[test]
fn the_same_run_twice_is_the_same_with_the_senses() {
    let run = || {
        let spawns = format!(
            "{}{}",
            sentry(25.0, 25.0, 0.0),
            r#"(place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (15.0, 35.0), radius: 6.0)),"#
        );
        let mut w = world(&spawns);
        w.attach_map(open());
        for k in 0..300 {
            put(&mut w, 0, 30.0 + (k % 60) as f32 * 0.1, 25.0, Gait::Run, 0);
            w.step();
        }
        (w.snapshot(), w.marks_for(0))
    };
    assert_eq!(run(), run());
}

#[test]
fn an_enemy_whose_facing_is_not_a_number_looks_from_the_side() {
    // A still hero (visibility 35) 10 yd in front of a sentry: its notice range is 12.25 yd in
    // front and 7.35 to the side. The sentry's facing is wrecked before every step (its post
    // would put it right again after it).
    let marks = |wreck: bool| {
        let mut w = world(&sentry(25.0, 25.0, 0.0));
        w.attach_map(open());
        put(&mut w, 0, 35.0, 25.0, Gait::Still, 0);
        for _ in 0..120 {
            if wreck {
                w.enemies[0].facing = V2::new(f32::NAN, f32::NAN);
            }
            w.step();
        }
        (w.marks_for(0), w.enemies[0].facing)
    };
    let (front, _) = marks(false);
    assert!(!front.is_empty(), "the control sees the hero");
    let (wrecked, facing) = marks(true);
    assert!(wrecked.is_empty(), "{wrecked:?}");
    assert!(facing.is_finite());
}

#[test]
fn a_hero_off_the_maps_grid_is_neither_seen_nor_heard_within_the_fog() {
    // The map's grid is 30 yd across but the world's bounds reach 50: a hero at (35, 25) stands
    // off it, 10 yd from a sentry that faces them. Rays are not cast to a place off the grid
    // ("open air" would show it plain), and a noise there is not heard.
    let run = |hero_x: f32| {
        let mut w = world(&sentry(25.0, 25.0, 0.0));
        w.attach_map(Arc::new(flat(30.0).finish()));
        put(&mut w, 0, hero_x, 25.0, Gait::Walk, 0);
        for _ in 0..120 {
            w.make_noise(0, V3::new(hero_x, 25.0, 0.0), Loudness::Normal);
            w.step();
        }
        w.marks_for(0)
    };
    assert!(
        !run(28.0).is_empty(),
        "the control, on the grid, is noticed"
    );
    assert!(run(35.0).is_empty());
}

#[test]
fn an_enemy_off_the_grid_sees_and_hears_nothing() {
    let mut w = world(&sentry(25.0, 25.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 30.0, 25.0, Gait::Run, 0);
    w.step();
    w.enemies[0].pos = V2::new(f32::MAX, 25.0);
    steps(&mut w, 5);
    assert!(w.marks_for(0).is_empty(), "{:?}", w.marks_for(0));
}

#[test]
fn a_new_place_to_check_is_not_made_to_wait_for_the_last_ones_failure() {
    // A sentry hears two noises inside a sealed house: no way there, so it looks from where it
    // stands. Within a second a noise on the far side of a short wall: that place it walks to.
    let mut w = world(&sentry(28.0, 35.0, 0.0));
    let mut b = flat(50.0);
    house(&mut b, V2::new(30.0, 30.0), V2::new(40.0, 40.0), None, true);
    b.add_box(V2::new(24.0, 33.0), V2::new(24.5, 37.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    put(&mut w, 0, 45.0, 5.0, Gait::Still, 0);
    w.step();
    let inside = V3::new(33.0, 35.0, 0.0);
    w.make_noise(0, inside, Loudness::Normal);
    w.step();
    w.make_noise(0, inside, Loudness::Normal);
    steps(&mut w, 20);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Suspicious
        }]
    );
    assert!(w.enemies[0].pos.distance(V2::new(28.0, 35.0)) <= 0.1);
    // 20 steps (0.67 s) on, suspicion has drained a little: this noise brings it to check again,
    // short of alert.
    let beyond_the_wall = V3::new(20.0, 35.0, 0.0);
    w.make_noise(0, beyond_the_wall, Loudness::Normal);
    let mut got_there = false;
    for _ in 0..300 {
        w.step();
        got_there |= w.enemies[0].pos.distance(V2::new(20.0, 35.0)) <= 1.0;
    }
    assert!(got_there, "{:?}", w.enemies[0].pos);
}

#[test]
fn a_crowd_chasing_round_a_wall_stays_within_the_steps_floors() {
    // Nine enemies alerted to a hero behind a long wall: every step each wants a path round it, or
    // a fresh one when the hero has moved. The searches of the senses and of the orders share the
    // step's floors, whatever the crowd.
    let spawns: String = (0..9)
        .map(|k| sentry(10.0 + k as f32 * 0.5, 25.0, 0.0))
        .collect();
    let mut w = world(&spawns);
    let mut b = flat(50.0);
    b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 34.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    put(&mut w, 0, 30.0, 25.0, Gait::Still, 0);
    w.step();
    for k in 1..=9 {
        w.alert(k, 0);
    }
    let mut worst = 0;
    for k in 0..300 {
        // The hero paces about, so goals move and paths are asked for again.
        put(
            &mut w,
            0,
            30.0 + (k % 40) as f32 * 0.25,
            25.0,
            Gait::Still,
            0,
        );
        let before = w.floors_looked_at();
        w.step();
        worst = worst.max(w.floors_looked_at() - before);
    }
    assert!(
        worst <= crate::STEP_FLOORS as u64,
        "{worst} floors in one step"
    );
    // They found their way round the wall's end.
    assert!(
        w.enemies.iter().any(|e| e.pos.x > 24.5),
        "{:?}",
        w.enemies.iter().map(|e| e.pos).collect::<Vec<_>>()
    );
}
