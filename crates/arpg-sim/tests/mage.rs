//! The mage's kit against the shipped content: every ability goes where it is aimed, and nothing
//! goes off without a key held.

use std::path::Path;

use arpg_sim::{Action, Content, Event, PlayerInput, World, DT, V2};

fn content() -> Content {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/arpg");
    Content::from_dir(&dir).expect("content/arpg parses")
}

/// A world with one mage at the start and no spawner; returns it and the start.
fn world() -> (World, V2) {
    let mut w = World::new(content(), "elwynn", 7).unwrap();
    w.set_spawning(false);
    w.add_hero(0, "mage").unwrap();
    let start = w.start();
    (w, start)
}

fn slot(w: &World, key: &str) -> u8 {
    let ab = w.content().ability(key).unwrap();
    let hero = &w.heroes()[0];
    w.content()
        .slots(hero.class)
        .iter()
        .position(|&a| a == ab)
        .unwrap() as u8
}

fn kind(w: &World, key: &str) -> usize {
    w.content().enemy(key).unwrap()
}

fn hp(w: &World, id: u32) -> f32 {
    w.enemies().iter().find(|e| e.id == id).map_or(0.0, |e| e.hp)
}

fn run(w: &mut World, secs: f32) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..(secs / DT).round() as usize {
        w.step();
        events.extend(w.drain_events());
    }
    events
}

/// Holds `slot` aimed at `aim` until it has gone off once, then lets go.
fn cast(w: &mut World, slot: u8, aim: V2) -> Vec<Event> {
    let mut events = Vec::new();
    w.set_input(
        0,
        PlayerInput {
            aim,
            cast: Some(slot),
            ..Default::default()
        },
    );
    for _ in 0..90 {
        w.step();
        let step = w.drain_events();
        let went = step.iter().any(|e| matches!(e, Event::Cast { .. }));
        events.extend(step);
        if went {
            break;
        }
    }
    w.set_input(
        0,
        PlayerInput {
            aim,
            ..Default::default()
        },
    );
    events
}

#[test]
fn nothing_casts_without_a_key() {
    let (mut w, start) = world();
    let wolf = kind(&w, "young_wolf");
    w.spawn_enemy(wolf, start + V2::new(6.0, 0.0));
    w.set_input(
        0,
        PlayerInput {
            aim: start + V2::new(6.0, 0.0),
            ..Default::default()
        },
    );
    let events = run(&mut w, 3.0);
    assert!(!events.iter().any(|e| matches!(e, Event::Cast { .. })));
    assert!(w.projectiles().is_empty());
}

#[test]
fn frostbolt_hits_what_is_on_its_line_and_misses_what_is_not() {
    let (mut w, start) = world();
    let k = kind(&w, "kobold_miner");
    let on = w.spawn_enemy(k, start + V2::new(15.0, 0.0));
    let off = w.spawn_enemy(k, start + V2::new(10.0, 8.0));
    let s = slot(&w, "frostbolt");
    cast(&mut w, s, start + V2::new(20.0, 0.0));
    assert_eq!(w.projectiles().len(), 1, "the bolt is in flight");
    run(&mut w, 1.0);
    let full = w.content().enemies[k].hp;
    assert!(hp(&w, on) < full, "the one on the line is hit");
    assert_eq!(hp(&w, off), full, "the one off it is not");
    assert!(w.enemies().iter().find(|e| e.id == on).unwrap().is_chilled(w.time()));
}

#[test]
fn a_bolt_aimed_at_nothing_flies_out() {
    let (mut w, start) = world();
    let s = slot(&w, "frostbolt");
    cast(&mut w, s, start + V2::new(0.0, -5.0));
    let events = run(&mut w, 2.0);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::MissileGone { struck: false, .. })));
    assert!(w.projectiles().is_empty());
}

#[test]
fn casting_frostbolt_roots_and_instant_blink_moves_toward_the_aim() {
    let (mut w, start) = world();
    let s = slot(&w, "frostbolt");
    w.set_input(
        0,
        PlayerInput {
            move_dir: V2::new(1.0, 0.0),
            aim: start + V2::new(0.0, 10.0),
            cast: Some(s),
            restart: false,
        },
    );
    w.step();
    assert!(matches!(w.heroes()[0].action, Action::Casting { .. }));
    let at = w.heroes()[0].pos;
    w.step();
    assert_eq!(w.heroes()[0].pos, at, "standing still while casting");

    let (mut w, start) = world();
    let b = slot(&w, "blink");
    let events = cast(&mut w, b, start + V2::new(0.0, 50.0));
    let pos = w.heroes()[0].pos;
    assert!((pos.distance(start) - 12.0).abs() < 0.01, "blinks its full range");
    assert!(pos.y > start.y + 11.9, "toward the aim");
    assert!(events.iter().any(|e| matches!(e, Event::Blinked { .. })));
}

#[test]
fn flamestrike_lands_at_the_cursor_pulled_in_to_its_range() {
    let (mut w, start) = world();
    let k = kind(&w, "kobold_miner");
    let s = slot(&w, "flamestrike");
    let range = w.content().abilities[w.content().ability("flamestrike").unwrap()].range;
    let far = w.spawn_enemy(k, start + V2::new(range, 0.0));
    let near = w.spawn_enemy(k, start + V2::new(8.0, 0.0));
    cast(&mut w, s, start + V2::new(range * 3.0, 0.0));
    let area = &w.areas()[0];
    assert!((area.pos.distance(start) - range).abs() < 0.01);
    run(&mut w, 1.0);
    let full = w.content().enemies[k].hp;
    assert!(hp(&w, far) < full, "the one under the clamped spot burns");
    assert_eq!(hp(&w, near), full);
}

#[test]
fn cone_of_cold_points_at_the_cursor() {
    let (mut w, start) = world();
    let k = kind(&w, "kobold_miner");
    let ahead = w.spawn_enemy(k, start + V2::new(0.0, 6.0));
    let behind = w.spawn_enemy(k, start + V2::new(0.0, -6.0));
    let s = slot(&w, "cone_of_cold");
    cast(&mut w, s, start + V2::new(0.0, 20.0));
    let full = w.content().enemies[k].hp;
    assert!(hp(&w, ahead) < full);
    assert_eq!(hp(&w, behind), full);
}

#[test]
fn frost_nova_freezes_and_frozen_enemies_take_crits() {
    let (mut w, start) = world();
    let k = kind(&w, "kobold_miner");
    let id = w.spawn_enemy(k, start + V2::new(5.0, 0.0));
    let s = slot(&w, "frost_nova");
    let events = cast(&mut w, s, start);
    assert!(events.iter().any(|e| matches!(e, Event::Frozen { .. })));
    // The nova's own hit is not a Shatter.
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Damage { crit: false, .. })));
    let b = slot(&w, "frostbolt");
    let mut events = cast(&mut w, b, start + V2::new(5.0, 0.0));
    events.extend(run(&mut w, 0.5));
    let shatter = events.iter().any(|e| {
        matches!(e, Event::Damage { target: arpg_sim::Target::Enemy(t), crit: true, .. } if *t == id)
    });
    assert!(shatter, "the bolt on the frozen enemy crits");
}

#[test]
fn blizzard_ticks_while_channelled_and_stops_when_the_mage_walks() {
    let (mut w, start) = world();
    let k = kind(&w, "kobold_miner");
    let id = w.spawn_enemy(k, start + V2::new(15.0, 0.0));
    let s = slot(&w, "blizzard");
    let aim = start + V2::new(15.0, 0.0);
    w.set_input(
        0,
        PlayerInput {
            aim,
            cast: Some(s),
            ..Default::default()
        },
    );
    run(&mut w, 2.6);
    assert!(matches!(w.heroes()[0].action, Action::Channeling { .. }));
    assert_eq!(w.areas().len(), 1);
    let after_two = hp(&w, id);
    assert!(after_two < w.content().enemies[k].hp);
    w.set_input(
        0,
        PlayerInput {
            aim,
            move_dir: V2::new(-1.0, 0.0),
            cast: Some(s),
            restart: false,
        },
    );
    run(&mut w, 2.0);
    assert!(w.areas().is_empty(), "walking ends the channel and its area");
    assert_eq!(hp(&w, id), after_two);
}

#[test]
fn ice_block_turns_away_every_blow() {
    let (mut w, start) = world();
    let k = kind(&w, "defias_thug");
    w.spawn_enemy(k, start + V2::new(2.0, 0.0));
    let s = slot(&w, "ice_block");
    cast(&mut w, s, start);
    let full = w.heroes()[0].max_hp;
    run(&mut w, 3.5);
    assert_eq!(w.heroes()[0].hp, full);
    run(&mut w, 4.0);
    assert!(w.heroes()[0].hp < full, "once it melts the thug lands its swings");
}

#[test]
fn stepping_out_of_a_telegraph_dodges_it() {
    let (mut w, start) = world();
    let k = kind(&w, "kobold_miner");
    let id = w.spawn_enemy(k, start + V2::new(3.0, 0.0));
    // Wait for the windup to begin.
    for _ in 0..30 {
        w.step();
        if w.drain_events().iter().any(|e| matches!(e, Event::EnemyWindup { .. })) {
            break;
        }
    }
    let enemy = w.enemies().iter().find(|e| e.id == id).unwrap();
    assert!(w.telegraph(enemy).is_some());
    w.set_input(
        0,
        PlayerInput {
            move_dir: V2::new(-1.0, 0.0),
            ..Default::default()
        },
    );
    run(&mut w, 1.4);
    assert_eq!(w.heroes()[0].hp, w.heroes()[0].max_hp, "out of the circle in time");
}

#[test]
fn the_spawner_fills_the_field_and_a_restart_clears_it() {
    let mut w = World::new(content(), "elwynn", 3).unwrap();
    w.add_hero(0, "mage").unwrap();
    w.step();
    let zone = &w.content().zones[0];
    assert_eq!(w.enemies().len(), zone.alive_start as usize);
    for e in w.enemies() {
        let d = e.pos.distance(w.start());
        assert!(d >= zone.spawn_ring.0 - 0.01 && d <= zone.spawn_ring.1 + 0.01);
    }
    w.restart();
    assert!(w.enemies().is_empty());
}
