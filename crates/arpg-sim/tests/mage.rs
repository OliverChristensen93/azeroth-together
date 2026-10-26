//! The mage's kit against the shipped content: every ability goes where it is aimed, and nothing
//! goes off without a key held.

use std::path::Path;

use arpg_sim::{Action, Content, EnemyState, Event, PlayerInput, World, DT, V2};

fn content() -> Content {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/arpg");
    Content::from_dir(&dir).expect("content/arpg parses")
}

/// A world with one mage at the start and no spawner; returns it and the start.
fn world() -> (World, V2) {
    let mut w = World::new(content(), "northshire", 7).unwrap();
    w.set_camps(false);
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
    let k = kind(&w, "kobold_laborer");
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
fn casting_fireball_roots_and_instant_blink_moves_toward_the_aim() {
    let (mut w, start) = world();
    let s = slot(&w, "fireball");
    w.set_input(
        0,
        PlayerInput {
            move_dir: V2::new(1.0, 0.0),
            aim: start + V2::new(0.0, 10.0),
            cast: Some(s),
            ..Default::default()
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
    let k = kind(&w, "kobold_laborer");
    let s = slot(&w, "flamestrike");
    let range = w.content().abilities[w.content().ability("flamestrike").unwrap()].range;
    let far = w.spawn_idle(k, start + V2::new(range, 0.0));
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
    let k = kind(&w, "kobold_laborer");
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
    let k = kind(&w, "kobold_laborer");
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
    let k = kind(&w, "kobold_laborer");
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
            ..Default::default()
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
    let k = kind(&w, "kobold_laborer");
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
fn a_roll_carries_the_hero_and_breaks_off_a_cast() {
    let (mut w, start) = world();
    let s = slot(&w, "fireball");
    w.set_input(
        0,
        PlayerInput {
            aim: start + V2::new(10.0, 0.0),
            cast: Some(s),
            ..Default::default()
        },
    );
    w.step();
    assert!(matches!(w.heroes()[0].action, Action::Casting { .. }));
    // The press lands between ticks and is kept for the next.
    w.set_input(
        0,
        PlayerInput {
            move_dir: V2::new(0.0, 1.0),
            dodge: true,
            ..Default::default()
        },
    );
    w.set_input(
        0,
        PlayerInput {
            move_dir: V2::new(0.0, 1.0),
            ..Default::default()
        },
    );
    let mut events = run(&mut w, 0.4);
    let dodge = w.content().classes[0].dodge;
    let hero = &w.heroes()[0];
    assert!(events.iter().any(|e| matches!(e, Event::Dodged { .. })));
    assert!(events.iter().any(|e| matches!(e, Event::CastStopped { .. })));
    assert!(hero.pos.y - start.y >= dodge.distance - 0.01, "rolled its distance");
    assert_eq!(hero.action, Action::Idle);
    events.clear();
}

/// A world with the zone's camps, one mage at the start.
fn camped() -> (World, V2) {
    let mut w = World::new(content(), "northshire", 11).unwrap();
    w.add_hero(0, "mage").unwrap();
    let start = w.start();
    w.step();
    (w, start)
}

#[test]
fn camps_stand_idle_until_a_hero_comes_near() {
    let (mut w, start) = camped();
    let zone = &w.content().zones[0];
    let count: u32 = zone.camps.iter().map(|c| c.count).sum();
    assert_eq!(w.enemies().len(), count as usize);
    assert!(w.enemies().iter().all(|e| e.state == EnemyState::Idle));
    assert!(w.enemies().iter().all(|e| e.pos.distance(start) > 20.0), "none on the Abbey's steps");
    run(&mut w, 3.0);
    assert!(w.enemies().iter().all(|e| e.state == EnemyState::Idle));
    // Walk up to the nearest.
    let target = w
        .enemies()
        .iter()
        .min_by(|a, b| a.pos.distance(start).total_cmp(&b.pos.distance(start)))
        .unwrap()
        .id;
    for _ in 0..600 {
        let e = w.enemies().iter().find(|e| e.id == target).unwrap();
        if e.state != EnemyState::Idle {
            break;
        }
        let me = w.heroes()[0].pos;
        w.set_input(
            0,
            PlayerInput {
                move_dir: e.pos - me,
                ..Default::default()
            },
        );
        w.step();
    }
    let e = w.enemies().iter().find(|e| e.id == target).unwrap();
    assert_ne!(e.state, EnemyState::Idle, "it noticed the hero");
    assert!(e.pos.distance(w.heroes()[0].pos) <= w.content().enemies[e.kind].aggro + 0.5);
}

#[test]
fn a_hit_brings_the_camp_mates_too() {
    let (mut w, _) = camped();
    let k = kind(&w, "kobold_vermin");
    let a = w.spawn_idle(k, V2::new(0.0, 0.0));
    let b = w.spawn_idle(k, V2::new(3.0, 0.0));
    let far = w.spawn_idle(k, V2::new(40.0, 0.0));
    w.hurt_enemy(a, 5.0, 0);
    let state = |w: &World, id: u32| w.enemies().iter().find(|e| e.id == id).unwrap().state;
    assert_ne!(state(&w, a), EnemyState::Idle);
    assert_ne!(state(&w, b), EnemyState::Idle, "its neighbour comes too");
    assert_eq!(state(&w, far), EnemyState::Idle);
}

#[test]
fn pulled_past_its_leash_it_runs_home_whole() {
    let (mut w, start) = world();
    let k = kind(&w, "young_wolf");
    let id = w.spawn_enemy(k, start + V2::new(5.0, 0.0));
    let leash = w.content().enemies[k].leash;
    w.hurt_enemy(id, 10.0, 0);
    // Freeze it, then run.
    let nova = slot(&w, "frost_nova");
    cast(&mut w, nova, start);
    let mut saw_evade = false;
    for _ in 0..(12.0 / DT) as usize {
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(-1.0, 0.0),
                ..Default::default()
            },
        );
        w.step();
        if w.drain_events().iter().any(|e| matches!(e, Event::EnemyEvade { .. })) {
            saw_evade = true;
            break;
        }
    }
    assert!(saw_evade, "a leash of {leash} yards");
    let e = w.enemies().iter().find(|e| e.id == id).unwrap();
    assert_eq!(e.hp, e.max_hp, "whole again");
    // Untouchable on the way.
    w.hurt_enemy(id, 10.0, 0);
    let e = w.enemies().iter().find(|e| e.id == id).unwrap();
    assert_eq!(e.hp, e.max_hp);
    run(&mut w, 10.0);
    let e = w.enemies().iter().find(|e| e.id == id).unwrap();
    assert_eq!(e.state, EnemyState::Idle);
    assert!(e.pos.distance(start + V2::new(5.0, 0.0)) < 0.5, "back home");
}

#[test]
fn a_camp_s_dead_come_back() {
    let (mut w, _) = camped();
    let first = w.enemies()[0].clone();
    let respawn = w.content().zones[0].camps[0].respawn;
    w.hurt_enemy(first.id, 10_000.0, 0);
    w.step();
    assert!(w.enemies().iter().all(|e| e.id != first.id));
    let before = w.enemies().len();
    run(&mut w, respawn + 0.5);
    assert_eq!(w.enemies().len(), before + 1);
    assert!(w.enemies().iter().any(|e| e.pos == first.home && e.id != first.id));
}

#[test]
fn a_fallen_hero_wakes_at_the_abbey_and_the_enemies_go_home() {
    let (mut w, start) = world();
    let k = kind(&w, "defias_thug");
    let id = w.spawn_enemy(k, start + V2::new(2.0, 0.0));
    for _ in 0..(120.0 / DT) as usize {
        w.step();
        if w.is_over() {
            break;
        }
    }
    assert!(w.is_over());
    w.set_input(
        0,
        PlayerInput {
            restart: true,
            ..Default::default()
        },
    );
    w.step();
    let hero = &w.heroes()[0];
    assert!(hero.alive && hero.pos == start && hero.hp == hero.max_hp);
    let e = w.enemies().iter().find(|e| e.id == id).unwrap();
    assert_eq!(e.state, EnemyState::Idle);
    assert_eq!(e.hp, e.max_hp);
}

#[test]
fn a_caster_s_bolt_hits_a_hero_who_stands_and_misses_one_who_steps_aside() {
    for step_aside in [false, true] {
        let (mut w, start) = world();
        let k = kind(&w, "defias_rogue_wizard");
        w.spawn_enemy(k, start + V2::new(15.0, 0.0));
        // Wait for the throw's windup.
        for _ in 0..90 {
            w.step();
            if w.drain_events().iter().any(|e| matches!(e, Event::EnemyWindup { .. })) {
                break;
            }
        }
        if step_aside {
            w.set_input(
                0,
                PlayerInput {
                    move_dir: V2::new(0.0, 1.0),
                    ..Default::default()
                },
            );
        }
        let mut thrown = false;
        for _ in 0..(2.0 / DT) as usize {
            w.step();
            thrown |= w
                .drain_events()
                .iter()
                .any(|e| matches!(e, Event::MissileLaunched { .. }));
            if thrown && step_aside {
                // Out of the line: stand again.
                w.set_input(0, PlayerInput::default());
            }
        }
        assert!(thrown, "it threw");
        let hero = &w.heroes()[0];
        if step_aside {
            assert_eq!(hero.hp, hero.max_hp, "the bolt flew past");
        } else {
            assert!(hero.hp < hero.max_hp, "the bolt struck");
        }
    }
}

#[test]
fn frostbolt_casts_on_the_move() {
    let (mut w, start) = world();
    let s = slot(&w, "frostbolt");
    w.set_input(
        0,
        PlayerInput {
            move_dir: V2::new(1.0, 0.0),
            aim: start + V2::new(0.0, 10.0),
            cast: Some(s),
            ..Default::default()
        },
    );
    w.step();
    assert!(matches!(w.heroes()[0].action, Action::Casting { .. }));
    let at = w.heroes()[0].pos;
    w.step();
    assert!(w.heroes()[0].pos.x > at.x, "walking while it casts");
}

#[test]
fn holding_the_key_casts_once_per_global_cooldown_and_blink_is_off_it() {
    let (mut w, start) = world();
    let bolt = slot(&w, "frostbolt");
    w.set_input(
        0,
        PlayerInput {
            aim: start + V2::new(0.0, 20.0),
            cast: Some(bolt),
            ..Default::default()
        },
    );
    let events = run(&mut w, 3.0);
    let casts = events
        .iter()
        .filter(|e| matches!(e, Event::CastStart { .. }))
        .count();
    let gcd = w.content().classes[0].gcd;
    // Casts start at 0, gcd, 2 gcd...: 3 s holds four at a 0.9 s global cooldown.
    assert_eq!(casts, (3.0 / gcd).ceil() as usize, "one per {gcd} s");
    // Blink answers on the global cooldown's heels.
    let blink = slot(&w, "blink");
    w.set_input(
        0,
        PlayerInput {
            aim: start + V2::new(0.0, 20.0),
            cast: Some(bolt),
            ..Default::default()
        },
    );
    w.step();
    w.drain_events();
    assert!(w.heroes()[0].gcd > 0.0 || matches!(w.heroes()[0].action, Action::Casting { .. }));
    w.set_input(
        0,
        PlayerInput {
            aim: start + V2::new(0.0, 20.0),
            cast: Some(blink),
            ..Default::default()
        },
    );
    // Once the bolt in hand is out, Blink goes at once whatever the global cooldown says.
    let events = run(&mut w, 0.5);
    assert!(events.iter().any(|e| matches!(e, Event::Blinked { .. })));
}

#[test]
fn two_rolls_in_a_row_then_a_breather() {
    let (mut w, _) = world();
    let class = w.content().classes[0].clone();
    let roll = |w: &mut World| {
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                dodge: true,
                ..Default::default()
            },
        );
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                ..Default::default()
            },
        );
        // The roll and its pause.
        run(w, class.dodge.time + class.dodge.recover + 0.05)
            .iter()
            .any(|e| matches!(e, Event::Dodged { .. }))
    };
    assert!(roll(&mut w));
    assert!(roll(&mut w));
    assert!(!roll(&mut w), "out of stamina");
    run(&mut w, class.stamina_delay + class.dodge.cost / class.stamina_regen + 0.2);
    assert!(roll(&mut w), "back after a breather");
}
