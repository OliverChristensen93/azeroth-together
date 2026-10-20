//! Enemies striking back at heroes: swings, casts, telegraphs, the dodge that makes them miss, and
//! the noise a blow makes.

use super::*;

#[test]
fn a_brute_in_reach_swings_every_two_seconds_and_breaks_stealth() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.heroes[0].stealthed = true;
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 125);
    assert_eq!(
        damage_to(&w.drain_events(), UnitRef::Hero(0)),
        vec![40.0, 40.0, 40.0]
    );
    assert!(!w.hero(0).unwrap().stealthed);
}

#[test]
fn a_caster_casts_from_range_and_its_bolt_lands() {
    let mut w = world(&post("caster", 35.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    steps(&mut w, 150);
    let events = w.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::CastStart { by: UnitRef::Enemy(i), .. } if *i == id)));
    assert!(events.contains(&Event::CastGo {
        by: UnitRef::Enemy(id),
        ability: ability(&w, "smite") as u16,
        hits: vec![UnitRef::Hero(0)],
        at: None
    }));
    assert_eq!(w.hero(0).unwrap().hp, 750.0);
}

#[test]
fn an_enemys_cast_stops_when_its_hero_falls() {
    let mut w = world(&post("caster", 35.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    for _ in 0..150 {
        if w.enemies[0].fight.casting.is_some() {
            break;
        }
        w.step();
    }
    assert!(w.enemies[0].fight.casting.is_some());
    w.hurt_hero(0, 5000.0);
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Enemy(id),
        ability: ability(&w, "smite") as u16,
        why: Stop::TargetLost
    }));
}

#[test]
fn a_telegraph_shows_then_lands_on_whoever_stands_in_it() {
    let mut w = world(&post("stomper", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    join(&mut w, 1, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    for _ in 0..200 {
        if !w.telegraphs.is_empty() {
            break;
        }
        w.step();
    }
    let t = w.telegraphs[0];
    assert_eq!(t.shape, Shape::Circle { radius: 8.0 });
    assert_eq!(t.at.ground(), w.enemies[0].pos);
    assert!((t.total - 1.5).abs() < 1e-4);
    assert!(w.combat.enemies[0].cast.is_some());
    steps(&mut w, 50);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(0)), vec![150.0]);
    assert!(damage_to(&events, UnitRef::Hero(1)).is_empty());
    assert!(w.telegraphs.is_empty());
}

/// An elite's stomp put on the ground under player 0 to land in `steps_to_land` steps, the hero
/// pressing dodge in the first of them.
fn stomped_after_a_dodge(steps_to_land: usize) -> Vec<Event> {
    let mut w = world(&post("elite", 40.0, 40.0, 0.0));
    join(&mut w, 0, "rogue", 20.0, 30.0);
    w.telegraphs.push(Telegraph {
        id: 99,
        enemy: w.enemies[0].id,
        by: None,
        ability: ability(&w, "stomp") as u16,
        shape: Shape::Circle { radius: 8.0 },
        at: V3::new(20.0, 30.0, 0.0),
        facing: V2::new(1.0, 0.0),
        left: steps_to_land as f32 * DT,
        total: 1.5,
    });
    dodging(&mut w, 1);
    steps(&mut w, steps_to_land);
    w.drain_events()
}

#[test]
fn a_dodge_makes_a_telegraph_miss_only_inside_its_window() {
    // Landing in the press's step and the 8 after it, 0.3 s: a miss.
    let inside = stomped_after_a_dodge(9);
    assert!(inside.contains(&Event::Evaded {
        player: 0,
        from: UnitRef::Enemy(1)
    }));
    assert!(damage_to(&inside, UnitRef::Hero(0)).is_empty());
    // One step later: a hit.
    let late = stomped_after_a_dodge(10);
    assert_eq!(damage_to(&late, UnitRef::Hero(0)), vec![150.0]);
}

/// A step with player 0 at (20, 30) and `presses` dodge presses counted so far.
fn dodging(w: &mut World, presses: u8) {
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(20.0, 30.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            dodge_presses: presses,
            ..Default::default()
        },
    );
    w.step();
}

/// How many dodges the events since the last drain hold.
fn dodged(w: &mut World) -> usize {
    w.drain_events()
        .iter()
        .filter(|e| matches!(e, Event::Dodged { player: 0 }))
        .count()
}

#[test]
fn dodges_come_back_one_at_a_time_and_are_heard() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 20.0, 30.0);
    for n in 1..=3 {
        dodging(&mut w, n);
    }
    assert_eq!(dodged(&mut w), 2);
    assert_eq!(w.hero(0).unwrap().fight.dodge.have, 0);
    assert_eq!(w.hero(0).unwrap().noise, 20.0);
    steps(&mut w, 8 * 30);
    assert_eq!(w.hero(0).unwrap().fight.dodge.have, 1);
    // A count that went round past 255 is one press.
    let mut w = world("");
    w.add_hero(0, "rogue", "Hero", 1, 0).unwrap();
    dodging(&mut w, 255);
    dodging(&mut w, 0);
    assert_eq!(dodged(&mut w), 1);
}

#[test]
fn the_first_dodge_count_is_a_baseline_not_a_press() {
    let mut w = world("");
    w.add_hero(0, "rogue", "Hero", 1, 0).unwrap();
    dodging(&mut w, 7);
    assert_eq!(dodged(&mut w), 0);
    assert_eq!(w.hero(0).unwrap().fight.dodge.have, 2);
    dodging(&mut w, 8);
    assert_eq!(dodged(&mut w), 1);
}

#[test]
fn a_dodge_while_downed_is_refused() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 20.0, 30.0);
    w.hurt_hero(0, 5000.0);
    w.drain_events();
    dodging(&mut w, 1);
    let events = w.drain_events();
    assert!(events.contains(&Event::Refused {
        player: 0,
        why: Refusal::Down
    }));
    assert!(!events.iter().any(|e| matches!(e, Event::Dodged { .. })));
    assert_eq!(w.hero(0).unwrap().fight.dodge.have, 2);
}

#[test]
fn a_dodge_stops_a_cast() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(10.0, 30.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            dodge_presses: 1,
            ..Default::default()
        },
    );
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::Cancelled
    }));
}

#[test]
fn a_dodge_breaks_a_rooted_channel_and_the_hero_is_free_to_evade_a_stomp() {
    // Arcane Missiles roots its channeller; the dodge must still reach the cast, or a mage beside a
    // winding-up stomp is locked in.
    let mut w = world(&post("elite", 40.0, 40.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 5, target, None);
    w.step();
    assert!(w.hero(0).unwrap().fight.casting.is_some());
    assert!(w.hero(0).unwrap().fight.rooted, "the channel roots them");
    w.drain_events();

    w.telegraphs.push(Telegraph {
        id: 99,
        enemy: w.enemies[0].id,
        by: None,
        ability: ability(&w, "stomp") as u16,
        shape: Shape::Circle { radius: 8.0 },
        at: V3::new(20.0, 30.0, 0.0),
        facing: V2::new(1.0, 0.0),
        left: 9.0 * DT,
        total: 1.5,
    });
    dodging(&mut w, 1);
    let events = w.drain_events();
    assert!(events.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: ability(&w, "missiles") as u16,
        why: Stop::Cancelled
    }));
    assert!(
        !w.hero(0).unwrap().fight.rooted,
        "the root lifts with the cast"
    );
    assert!(w.hero(0).unwrap().fight.casting.is_none());

    // The stomp lands inside the dodge's window: a miss.
    steps(&mut w, 9);
    let events = w.drain_events();
    assert!(events.contains(&Event::Evaded {
        player: 0,
        from: UnitRef::Enemy(1)
    }));
    assert!(damage_to(&events, UnitRef::Hero(0)).is_empty());
}

/// A brute at (11.5, 30) after a rogue at (9.5, 30), 2 yards apart, with a thin wall (a cell
/// wide, aligned to the grid) between them or not.
fn brute_across_a_wall(walled: bool) -> World {
    let mut w = world(&post("brute", 11.5, 30.0, 90.0));
    if walled {
        let mut b = flat(60.0);
        b.add_box(V2::new(10.0, 25.0), V2::new(10.5, 35.0), 0.0, 3.0);
        w.attach_map(Arc::new(b.finish()));
    }
    join(&mut w, 0, "rogue", 9.5, 30.0);
    w.alert(w.enemies[0].id, 0);
    w
}

#[test]
fn a_brute_does_not_swing_through_a_thin_wall() {
    let mut w = brute_across_a_wall(true);
    steps(&mut w, 60);
    assert!(damage_to(&w.drain_events(), UnitRef::Hero(0)).is_empty());
    assert_eq!(w.hero(0).unwrap().hp, 1000.0);
    // The same two yards in the open: it swings.
    let mut w = brute_across_a_wall(false);
    steps(&mut w, 60);
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Hero(0)), vec![40.0]);
}

/// A stomper at (30, 30) and a rogue 7.5 yards west of it, a thin wall between them or not.
fn stomper_across_a_wall(walled: bool) -> World {
    let mut w = world(&post("stomper", 30.0, 30.0, 180.0));
    if walled {
        let mut b = flat(60.0);
        b.add_box(V2::new(26.0, 0.0), V2::new(26.5, 60.0), 0.0, 3.0);
        w.attach_map(Arc::new(b.finish()));
    }
    join(&mut w, 0, "rogue", 22.5, 30.0);
    w
}

#[test]
fn a_telegraph_does_not_reach_through_a_wall() {
    let land = |walled: bool| {
        let mut w = stomper_across_a_wall(walled);
        w.telegraphs.push(Telegraph {
            id: 99,
            enemy: w.enemies[0].id,
            by: None,
            ability: ability(&w, "stomp") as u16,
            shape: Shape::Circle { radius: 8.0 },
            at: V3::new(30.0, 30.0, 0.0),
            facing: V2::new(-1.0, 0.0),
            left: DT,
            total: 1.5,
        });
        w.step();
        w.drain_events()
    };
    assert!(damage_to(&land(true), UnitRef::Hero(0)).is_empty());
    assert_eq!(damage_to(&land(false), UnitRef::Hero(0)), vec![150.0]);
}

#[test]
fn an_enemy_does_not_begin_a_telegraphed_move_at_a_hero_behind_a_wall() {
    let mut w = stomper_across_a_wall(true);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 120);
    assert!(w.telegraphs.is_empty());
    assert!(w.combat.enemies[0].cast.is_none());
}

#[test]
fn a_blow_does_not_cost_a_sprinting_hero_her_speed_in_that_step() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    press(&mut w, 0, 1, 5, None, None);
    w.step();
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Hero(0)), vec![40.0]);
    assert!(
        (w.heroes[0].speed - 10.5).abs() < 1e-4,
        "{}",
        w.heroes[0].speed
    );
}

#[test]
fn a_hero_downed_by_a_blow_crawls_in_that_step() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.heroes[0].hp = 30.0;
    w.alert(w.enemies[0].id, 0);
    w.step();
    assert!(w.drain_events().contains(&Event::Downed { player: 0 }));
    assert!(
        (w.heroes[0].speed - 7.0 * 0.3).abs() < 1e-4,
        "{}",
        w.heroes[0].speed
    );
}

#[test]
fn what_enemies_roll_is_the_same_every_run() {
    let run = || {
        let mut w = world(&format!(
            "{}{}",
            post("stomper", 30.0, 30.0, 180.0),
            post("brute", 25.0, 36.0, 180.0)
        ));
        join(&mut w, 0, "rogue", 24.0, 30.0);
        w.alert(w.enemies[0].id, 0);
        w.alert(w.enemies[1].id, 0);
        steps(&mut w, 300);
        w.drain_events()
    };
    let events = run();
    assert!(events.iter().any(|e| matches!(e, Event::Swing { .. })));
    assert_eq!(events, run());
}

/// A brute at (30, 30) after a rogue at (28, 30), and a calm brute 12 yards west of the rogue,
/// facing away. The first is alerted (its shout dropped, so the calm one is not woken by it) when
/// `fight` is set. One step; the calm one's suspicion of the rogue.
fn suspicion_after_a_step(fight: bool) -> f32 {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 180.0),
        post("brute", 16.0, 30.0, 90.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 28.0, 30.0);
    if fight {
        w.alert(w.enemies[0].id, 0);
        w.senses.drop_shouts();
    }
    w.step();
    assert_eq!(
        damage_to(&w.drain_events(), UnitRef::Hero(0)).len(),
        usize::from(fight)
    );
    w.senses.suspicion_of(1, 0)
}

#[test]
fn a_blow_landing_on_a_hero_is_heard_fifteen_yards_around() {
    let noise = world("").content().rules.senses.awareness.noise;
    // It sees a little of the rogue either way; only the blow adds a noise's worth.
    let (quiet, struck) = (suspicion_after_a_step(false), suspicion_after_a_step(true));
    assert!(
        (struck - quiet - noise).abs() < 1e-3,
        "{quiet} then {struck}"
    );
}

#[test]
fn a_telegraphed_blow_makes_its_own_noise_as_it_lands() {
    let mut w = world(&post("elite", 40.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 36.0, 30.0);
    w.telegraphs.push(Telegraph {
        id: 99,
        enemy: w.enemies[0].id,
        by: None,
        ability: ability(&w, "cleave") as u16,
        shape: Shape::Cone {
            length: 8.0,
            angle: 120.0,
        },
        at: V3::new(40.0, 30.0, 0.0),
        facing: V2::new(-1.0, 0.0),
        left: DT,
        total: 1.2,
    });
    steps(&mut w, 2);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(0)), vec![200.0]);
    // The cleave's own Loud (40 yards), not just the hit's 15.
    assert_eq!(w.hero(0).unwrap().noise, 40.0);
}

#[test]
fn a_blow_that_misses_or_does_no_harm_is_not_heard() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.blow_lands(w.enemies[0].id, 0, f32::NAN);
    w.hurt_hero(0, 5000.0);
    w.blow_lands(w.enemies[0].id, 0, 40.0);
    steps(&mut w, 2);
    assert_eq!(w.hero(0).unwrap().noise, 0.0);
}

#[test]
fn an_enemys_fight_in_the_state_matches_the_events_of_its_step() {
    let mut w = world(&post("caster", 35.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    let (mut began, mut landed) = (false, false);
    for _ in 0..200 {
        w.step();
        for e in w.drain_events() {
            match e {
                Event::CastStart {
                    by: UnitRef::Enemy(_),
                    ..
                } => {
                    began = true;
                    assert!(w.enemies[0].fight.casting.is_some());
                    assert_eq!(w.combat.enemies[0].engage.hold, Hold::Still);
                }
                Event::CastGo {
                    by: UnitRef::Enemy(_),
                    ..
                } => {
                    landed = true;
                    assert!(w.enemies[0].fight.casting.is_none());
                    assert_ne!(w.combat.enemies[0].engage.hold, Hold::Still);
                }
                _ => {}
            }
        }
    }
    assert!(began && landed);
}

#[test]
fn a_telegraph_landing_frees_its_caster_in_the_same_step() {
    let mut w = world(&post("stomper", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    let mut landed = false;
    for _ in 0..200 {
        w.step();
        if w.drain_events().iter().any(|e| {
            matches!(
                e,
                Event::CastGo {
                    by: UnitRef::Enemy(_),
                    ..
                }
            )
        }) {
            landed = true;
            assert!(w.enemies[0].fight.casting.is_none());
            assert_ne!(w.combat.enemies[0].engage.hold, Hold::Still);
            break;
        }
    }
    assert!(landed);
}

/// A stomper at (30, 30) after a rogue 5 yards west, alerted, run to the step its telegraph shows.
fn stomper_winding_up() -> World {
    let mut w = world(&post("stomper", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    for _ in 0..200 {
        if !w.telegraphs.is_empty() {
            break;
        }
        w.step();
    }
    assert!(!w.telegraphs.is_empty());
    w.drain_events();
    w
}

#[test]
fn a_caster_killed_in_the_windup_takes_its_telegraph_with_it() {
    let mut w = stomper_winding_up();
    w.kill_enemy(0, Some(0));
    steps(&mut w, 60);
    assert!(w.telegraphs.is_empty());
    assert!(w.combat.enemies[0].cast.is_none());
    assert!(w.enemies[0].fight.casting.is_none());
    let events = w.drain_events();
    assert!(events.contains(&Event::EnemyDied {
        id: w.enemies[0].id,
        by: Some(0)
    }));
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::CastGo {
            by: UnitRef::Enemy(_),
            ..
        }
    )));
    assert_eq!(w.hero(0).unwrap().hp, 1000.0);
}

#[test]
fn a_caster_stunned_or_sapped_in_the_windup_loses_its_telegraph() {
    for key in ["cheap", "sap"] {
        let mut w = stomper_winding_up();
        w.land(0, ability(&w, key), Some(0), None);
        steps(&mut w, 60);
        assert!(w.telegraphs.is_empty(), "{key}");
        assert!(w.combat.enemies[0].cast.is_none(), "{key}");
        let events = w.drain_events();
        assert!(
            events.contains(&Event::CastStop {
                by: UnitRef::Enemy(w.enemies[0].id),
                ability: ability(&w, "stomp") as u16,
                why: Stop::Interrupted
            }),
            "{key}"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                Event::CastGo {
                    by: UnitRef::Enemy(_),
                    ..
                }
            )),
            "{key}"
        );
        assert_eq!(w.hero(0).unwrap().hp, 1000.0, "{key}");
    }
}

#[test]
fn a_cast_waiting_for_a_telegraph_that_is_gone_is_dropped() {
    let mut w = stomper_winding_up();
    w.telegraphs.clear();
    w.step();
    assert!(w.combat.enemies[0].cast.is_none());
    assert!(w.enemies[0].fight.casting.is_none());
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Enemy(w.enemies[0].id),
        ability: ability(&w, "stomp") as u16,
        why: Stop::Interrupted
    }));
}

#[test]
fn a_dodge_in_the_input_of_a_press_goes_first_and_the_press_then_begins() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    let h = w.hero(0).unwrap();
    let pose = HeroPose {
        pos: h.pos,
        facing: h.facing,
        z: h.z,
    };
    w.set_input(
        0,
        PlayerInput {
            pose: Some(pose),
            cast: Some(CastPress {
                seq: 1,
                slot: 1,
                target,
                at: None,
            }),
            dodge_presses: 1,
            ..Default::default()
        },
    );
    w.step();
    let events = w.drain_events();
    let dodge = events
        .iter()
        .position(|e| matches!(e, Event::Dodged { .. }))
        .unwrap();
    let cast = events
        .iter()
        .position(|e| matches!(e, Event::CastStart { .. }))
        .unwrap();
    assert!(dodge < cast);
    assert!(!events.iter().any(|e| matches!(e, Event::CastStop { .. })));
    assert!(w.hero(0).unwrap().fight.casting.is_some());
}

#[test]
fn an_enemy_cast_aimed_at_its_own_place_hits_every_hero_around_it_with_a_clear_line() {
    let mut w = world(&post("shocker", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 27.0, 30.0);
    join(&mut w, 1, "mage", 31.0, 30.0);
    join(&mut w, 2, "mage", 40.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    let mut hits = Vec::new();
    for _ in 0..300 {
        w.step();
        if let Some(Event::CastGo { hits: h, .. }) = w
            .drain_events()
            .into_iter()
            .find(|e| matches!(e, Event::CastGo { .. }))
        {
            hits = h;
            break;
        }
    }
    assert_eq!(hits, vec![UnitRef::Hero(0), UnitRef::Hero(1)]);
    assert_eq!(w.hero(0).unwrap().hp, 950.0);
    assert_eq!(w.hero(2).unwrap().hp, 800.0);
}

#[test]
fn an_enemy_cast_aimed_at_its_own_place_does_not_reach_through_a_wall() {
    let mut w = world(&post("shocker", 30.0, 30.0, 180.0));
    let mut b = flat(60.0);
    b.add_box(V2::new(27.0, 20.0), V2::new(27.5, 40.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "rogue", 31.0, 30.0);
    join(&mut w, 1, "mage", 24.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    let mut hits = None;
    for _ in 0..300 {
        w.step();
        if let Some(Event::CastGo { hits: h, .. }) = w
            .drain_events()
            .into_iter()
            .find(|e| matches!(e, Event::CastGo { .. }))
        {
            hits = Some(h);
            break;
        }
    }
    assert_eq!(hits, Some(vec![UnitRef::Hero(0)]));
    assert_eq!(w.hero(1).unwrap().hp, 800.0);
}
