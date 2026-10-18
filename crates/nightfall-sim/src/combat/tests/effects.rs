//! What abilities do: damage, ambushes, combo points, kills, areas, Vanish, Blink, Distract and
//! the meals, and the blows that must never make a number go wrong.

use super::*;

#[test]
fn damage_lands_with_the_ambush_on_an_unaware_enemy_once() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let bolt = ability(&w, "bolt");
    w.land(0, bolt, Some(0), None);
    w.land(0, bolt, Some(0), None);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, foe(&w, 0).unwrap()), vec![150.0, 100.0]);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Damage { ambush: true, .. })));
    assert_eq!(w.enemies[0].hp, 750.0);
}

#[test]
fn an_enemy_alerted_to_a_friend_is_still_caught_unaware_by_you() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    join(&mut w, 1, "mage", 20.0, 35.0);
    let id = w.enemies[0].id;
    w.alert(id, 1);
    w.land(0, ability(&w, "bolt"), Some(0), None);
    assert_eq!(
        damage_to(&w.drain_events(), UnitRef::Enemy(id)),
        vec![150.0]
    );
}

#[test]
fn sap_holds_without_alerting() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let id = w.enemies[0].id;
    w.land(0, ability(&w, "sap"), Some(0), None);
    assert_eq!(
        w.combat.enemies[0].held.map(|h| h.kind),
        Some(ControlKind::Sap)
    );
    assert!(w.opener_allowed(id, 0));
    assert!(w.marks_for(0).is_empty());
}

#[test]
fn a_blow_frees_a_sheep_and_a_sapped_enemy_but_not_a_stunned_one() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    for (key, freed) in [("sap", true), ("sheep", true), ("cheap", false)] {
        w.combat.enemies[0].held = None;
        w.land(0, ability(&w, key), Some(0), None);
        w.strike(0, 0, 10.0, false, false, false);
        assert_eq!(w.combat.enemies[0].held.is_none(), freed, "{key}");
    }
    // Gouge's own damage comes before its hold, so it holds.
    w.combat.enemies[0].held = None;
    w.land(0, ability(&w, "gouge"), Some(0), None);
    assert_eq!(
        w.combat.enemies[0].held.map(|h| h.kind),
        Some(ControlKind::Incapacitate)
    );
}

#[test]
fn combo_points_build_on_one_target_and_a_finisher_spends_them() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("brute", 30.0, 40.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let (strike, finish) = (ability(&w, "strike"), ability(&w, "finish"));
    let (a, b) = (w.enemies[0].id, w.enemies[1].id);
    w.land(0, strike, Some(0), None);
    w.land(0, strike, Some(0), None);
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.combo, f.combo_on), (2, Some(a)));
    w.drain_events();
    w.land(0, finish, Some(0), None);
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Enemy(a)), vec![70.0]);
    assert_eq!(w.hero(0).unwrap().fight.combo, 0);
    w.land(0, strike, Some(0), None);
    w.land(0, strike, Some(1), None);
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.combo, f.combo_on), (1, Some(b)));
}

#[test]
fn an_area_ability_reaches_only_within_its_radius() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 38.0, 0.0),
        post("brute", 30.0, 45.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 30.0, 30.0);
    let hits = w.land(0, ability(&w, "nova"), None, None);
    assert_eq!(hits, vec![UnitRef::Enemy(w.enemies[0].id)]);
    assert_eq!(
        w.combat.enemies[0].held.map(|h| h.kind),
        Some(ControlKind::Root)
    );
    assert!(w.combat.enemies[1].held.is_none());
}

#[test]
fn a_kill_is_heard_and_ends_attacks_and_combos_on_the_dead() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let id = w.enemies[0].id;
    w.land(0, ability(&w, "strike"), Some(0), None);
    assert_eq!(w.hero(0).unwrap().fight.attacking, Some(id));
    w.strike(0, 0, 5000.0, false, false, false);
    assert_eq!(w.enemies[0].hp, 0.0);
    assert!(w
        .drain_events()
        .contains(&Event::EnemyDied { id, by: Some(0) }));
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.attacking, f.combo, f.combo_on), (None, 0, None));
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 15.0);
}

#[test]
fn vanish_turns_a_chase_into_a_search() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 20.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: id,
            alert: Alert::Alerted
        }]
    );
    w.land(0, ability(&w, "vanish"), None, None);
    // At once, with no step between: stealthed, as visible as a stealthed hero standing still,
    // and as fast as stealth lets the class go.
    let r = &w.content().rules.senses;
    let (sneak_still, still) = (r.visibility.sneak_still, r.visibility.still);
    let speed = w.content().classes[w.hero(0).unwrap().class].speed * r.stealth_speed;
    let hero = w.hero(0).unwrap();
    assert!(hero.stealthed);
    assert_ne!(sneak_still, still);
    assert_eq!((hero.visibility, hero.speed), (sneak_still, speed));
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: id,
            alert: Alert::Suspicious
        }]
    );
}

#[test]
fn blink_stops_short_of_a_wall() {
    let mut w = world("");
    let mut b = flat(60.0);
    house(&mut b, V2::new(19.0, 0.0), V2::new(30.0, 60.0), None, false);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "blink"), None, None);
    let to = w
        .drain_events()
        .into_iter()
        .find_map(|e| match e {
            Event::Blinked { player: 0, to } => Some(to),
            _ => None,
        })
        .unwrap();
    assert!(to.x > 14.0 && to.x < 19.0, "{to:?}");
    assert!((to.y - 30.0).abs() < 1e-3);
    // In the open it goes the whole way.
    let mut w = world("");
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "blink"), None, None);
    assert!(w.drain_events().contains(&Event::Blinked {
        player: 0,
        to: V3::new(30.0, 30.0, 0.0)
    }));
}

#[test]
fn distract_turns_only_the_unalerted_within_reach() {
    let spawns = format!(
        "{}{}{}",
        post("brute", 40.0, 30.0, 0.0),
        post("brute", 44.0, 33.0, 0.0),
        post("brute", 55.0, 30.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 10.0, 30.0);
    let alerted = w.enemies[1].id;
    w.alert(alerted, 0);
    w.land(
        0,
        ability(&w, "distract"),
        None,
        Some(V3::new(42.0, 30.0, 0.0)),
    );
    let d = |i: usize| w.combat.enemies[i].distracted.map(|(at, _)| at);
    assert_eq!(d(0), Some(V2::new(42.0, 30.0)));
    assert_eq!(d(1), None);
    assert_eq!(d(2), None);
}

#[test]
fn a_blow_of_no_number_does_nothing_and_never_makes_health_nan() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.combat.enemies[0].held = Some(super::super::Held {
        kind: ControlKind::Sap,
        left: 10.0,
        by: 0,
        spell: 1,
    });
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        w.strike(0, 0, bad, false, false, true);
        assert_eq!(w.enemies[0].hp, 1000.0, "{bad}");
    }
    assert!(w.drain_events().is_empty());
    assert!(w.combat.enemies[0].held.is_some());
    // A negative blow heals nothing, and an enormous one kills without overflowing into NaN.
    w.strike(0, 0, -50.0, false, false, false);
    assert_eq!(w.enemies[0].hp, 1000.0);
    w.strike(0, 0, f32::MAX, false, false, true);
    assert_eq!(w.enemies[0].hp, 0.0);
}

#[test]
fn a_crit_multiplies_the_blow_and_a_huge_crit_stays_finite() {
    let mut w = world_with("crit_times: 2.0", &post("brute", 30.0, 30.0, 0.0));
    let mut c = (*w.content).clone();
    c.rules.combat.crit_chance = 1.0;
    w.content = Arc::new(c);
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.strike(0, 0, 100.0, false, true, true);
    assert_eq!(w.enemies[0].hp, 800.0);
    assert!(w
        .drain_events()
        .iter()
        .any(|e| matches!(e, Event::Damage { crit: true, amount, .. } if *amount == 200.0)));
    w.strike(0, 0, f32::MAX, false, true, true);
    assert_eq!(w.enemies[0].hp, 0.0);
}

#[test]
fn a_hold_with_no_number_for_its_time_is_not_applied() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let mut c = (*w.content).clone();
    for a in &mut c.abilities {
        if a.key == "sap" {
            a.effects = vec![crate::Effect::Control {
                kind: ControlKind::Sap,
                secs: f32::NAN,
            }];
        }
        if a.key == "blink" {
            a.effects = vec![crate::Effect::Blink {
                distance: f32::INFINITY,
            }];
        }
    }
    w.content = Arc::new(c);
    w.land(0, ability(&w, "sap"), Some(0), None);
    assert!(w.combat.enemies[0].held.is_none());
    // A Blink of no distance ends where the hero stands, and does not loop for ever.
    w.land(0, ability(&w, "blink"), None, None);
    assert!(w
        .drain_events()
        .iter()
        .any(|e| matches!(e, Event::Blinked { to, .. } if to.ground() == V2::new(28.0, 30.0))));
}

#[test]
fn a_killed_alerted_enemy_keeps_no_mark_and_its_shout_reaches_no_ally() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 180.0),
        post("brute", 30.0, 34.0, 180.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 10.0, 30.0);
    let dead = w.enemies[0].id;
    // Alerting queues a shout for the next step; the kill comes before it.
    w.alert(dead, 0);
    assert_eq!(w.marks_for(0).len(), 1);
    w.strike(0, 0, 5000.0, false, true, false);
    assert!(w.marks_for(0).is_empty());
    w.step();
    assert!(w.marks_for(0).is_empty(), "{:?}", w.marks_for(0));
    assert!(!w.senses.alerted(1));
}

#[test]
fn a_finisher_gets_the_bonus_only_from_points_on_its_own_target() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("brute", 30.0, 40.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let (strike, finish) = (ability(&w, "strike"), ability(&w, "finish"));
    let b = w.enemies[1].id;
    w.land(0, strike, Some(0), None);
    w.land(0, strike, Some(0), None);
    w.drain_events();
    w.land(0, finish, Some(1), None);
    // Finish is 10 plus 30 a point, times 1.5 for B being caught unaware: B has no points of its
    // own, so it gets 15 (not 105), and the points
    // built on A are spent all the same.
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Enemy(b)), vec![15.0]);
    assert_eq!(w.hero(0).unwrap().fight.combo, 0);
}

#[test]
fn a_blow_that_does_no_harm_is_no_blow() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.land(0, ability(&w, "sap"), Some(0), None);
    w.drain_events();
    for amount in [0.0, -50.0, f32::NAN] {
        w.strike(0, 0, amount, false, false, true);
    }
    assert!(w.drain_events().is_empty());
    assert!(w.combat.enemies[0].held.is_some());
    assert_eq!(w.combat.hero(0).unwrap().combat_for, 0.0);
}

#[test]
fn an_ambush_too_big_for_a_float_still_kills() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let mut c = (*w.content).clone();
    c.rules.senses.ambush = f32::MAX;
    w.content = Arc::new(c);
    w.land(0, ability(&w, "bolt"), Some(0), None);
    assert_eq!(w.enemies[0].hp, 0.0);
    assert_eq!(
        w.drain_events()
            .iter()
            .filter(|e| matches!(e, Event::EnemyDied { .. }))
            .count(),
        1
    );
}

#[test]
fn a_dead_enemy_dies_once() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.strike(0, 0, 5000.0, false, true, false);
    w.strike(0, 0, 5000.0, false, true, false);
    w.kill_enemy(0, Some(0));
    w.kill_enemy(99, Some(0));
    let died = |e: &&Event| matches!(e, Event::EnemyDied { .. });
    assert_eq!(w.drain_events().iter().filter(died).count(), 1);
}

#[test]
fn an_area_ability_hits_each_living_enemy_in_it_once_and_not_the_dead() {
    let spawns = format!(
        "{}{}{}",
        post("brute", 30.0, 35.0, 0.0),
        post("brute", 30.0, 36.0, 0.0),
        post("brute", 33.0, 30.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 30.0, 30.0);
    w.strike(0, 1, 5000.0, false, true, false);
    w.drain_events();
    let hits = w.land(0, ability(&w, "nova"), None, None);
    let (a, c) = (w.enemies[0].id, w.enemies[2].id);
    assert_eq!(hits, vec![UnitRef::Enemy(a), UnitRef::Enemy(c)]);
    let events = w.drain_events();
    for id in [a, c] {
        assert_eq!(damage_to(&events, UnitRef::Enemy(id)).len(), 1);
    }
    assert!(damage_to(&events, UnitRef::Enemy(w.enemies[1].id)).is_empty());
}

#[test]
fn an_aimed_ability_ignores_a_dead_target() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    w.strike(0, 0, 5000.0, false, true, false);
    w.drain_events();
    assert!(w.land(0, ability(&w, "bolt"), Some(0), None).is_empty());
    assert!(w.drain_events().is_empty());
}

#[test]
fn a_blink_longer_than_the_map_is_cut_to_it_and_a_blocked_one_stays_put() {
    let mut w = world("");
    join(&mut w, 0, "mage", 10.0, 30.0);
    let mut c = (*w.content).clone();
    for a in &mut c.abilities {
        if a.key == "blink" {
            a.effects = vec![crate::Effect::Blink { distance: 1.0e30 }];
        }
    }
    w.content = Arc::new(c);
    w.land(0, ability(&w, "blink"), None, None);
    let to = w
        .drain_events()
        .into_iter()
        .find_map(|e| match e {
            Event::Blinked { to, .. } => Some(to),
            _ => None,
        })
        .unwrap();
    // The map's edge is the furthest east it goes.
    assert!(to.x > 10.0 && to.x <= 60.0 + 1e-3, "{to:?}");
    // Walled in on every side the Blink still happens, where the hero stands.
    let mut w = world("");
    let mut b = flat(60.0);
    house(&mut b, V2::new(11.0, 0.0), V2::new(13.0, 60.0), None, false);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "blink"), None, None);
    let events = w.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Blinked { to, .. } if to.x < 11.0)));
}
