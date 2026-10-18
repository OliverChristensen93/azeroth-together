//! Enemies held, slowed, burning, turned or distracted, and what a dead one leaves behind.

use super::*;

#[test]
fn a_stunned_enemy_stands_and_a_rooted_one_turns_but_stays() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 20.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    w.land(0, ability(&w, "cheap"), Some(0), None);
    w.heroes[0].fight.attacking = None;
    steps(&mut w, 30);
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
    assert_eq!(w.enemies[0].fight.control, Some(ControlKind::Stun));
    steps(&mut w, 100);
    assert_eq!(w.enemies[0].fight.control, None);
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 30.0, 22.0);
    w.alert(w.enemies[0].id, 0);
    w.land(0, ability(&w, "nova"), None, None);
    steps(&mut w, 30);
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
    let f = w.enemies[0].facing;
    assert!(f.y < -0.9, "{f:?}");
}

#[test]
fn a_sapped_enemy_neither_sees_nor_hears_until_it_wakes() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.land(0, ability(&w, "sap"), Some(0), None);
    // Running loudly about in front of it.
    let run_in_front = |w: &mut World, k: usize| {
        let at = V2::new(35.0 + (k % 20) as f32 * 0.2, 30.0);
        stand(w, 0, at, V2::new(1.0, 0.0), Gait::Run);
        w.step();
    };
    for k in 0..300 {
        run_in_front(&mut w, k);
    }
    assert!(w.marks_for(0).is_empty());
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
    for k in 0..700 {
        run_in_front(&mut w, k);
    }
    assert!(!w.marks_for(0).is_empty());
}

#[test]
fn bleeds_and_burns_tick_their_count_and_a_silent_one_is_not_heard() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.land(0, ability(&w, "garrote"), Some(0), None);
    w.heroes[0].fight.attacking = None;
    steps(&mut w, 18 * 30 + 5);
    let ticks = damage_to(&w.drain_events(), foe(&w, 0).unwrap());
    assert_eq!(ticks, vec![30.0; 6]);
    assert_eq!(w.enemies[0].hp, 1000.0 - 180.0);
    assert_eq!(w.hero(0).unwrap().noise, 0.0);
    assert!(w.enemies[0].fight.auras.is_empty());
}

#[test]
fn a_sheep_wanders_near_where_it_was_turned() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "sheep"), Some(0), None);
    let start = w.enemies[0].pos;
    let mut furthest: f32 = 0.0;
    for _ in 0..300 {
        w.step();
        furthest = furthest.max(w.enemies[0].pos.distance(start));
    }
    assert!(furthest > 0.5 && furthest <= 4.5, "{furthest}");
    assert_eq!(w.enemies[0].fight.control, Some(ControlKind::Polymorph));
    assert_eq!(w.enemies[0].fight.auras, vec![118]);
}

#[test]
fn a_slowed_enemy_chases_slower() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    w.combat.enemies[0].slow = Some(Slowed {
        by: 0.4,
        left: 7.0,
        spell: 116,
    });
    steps(&mut w, 10);
    assert!(
        (w.enemies[0].speed - 7.0 * 0.6).abs() < 0.05,
        "{}",
        w.enemies[0].speed
    );
    assert!(w.enemies[0].fight.slowed);
    assert_eq!(w.enemies[0].fight.auras, vec![116]);
}

#[test]
fn a_distracted_enemy_faces_the_spot() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.land(
        0,
        ability(&w, "distract"),
        None,
        Some(V3::new(30.0, 38.0, 0.0)),
    );
    steps(&mut w, 5);
    let f = w.enemies[0].facing;
    assert!(f.y > 0.9, "{f:?}");
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
}

#[test]
fn a_caster_keeps_its_distance_while_it_sees_its_hero() {
    let mut w = world(&post("caster", 50.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 150);
    let d = w.enemies[0].pos.distance(V2::new(10.0, 30.0));
    assert!((27.0..=29.0).contains(&d), "{d}");
}

#[test]
fn a_dead_enemy_stays_where_it_fell() {
    let patrol = r#"(place: "Road", kind: "brute", behaviour: Patrol(route: [(20.0, 20.0), (40.0, 20.0)])),"#;
    let mut w = world(patrol);
    join(&mut w, 0, "mage", 5.0, 50.0);
    steps(&mut w, 30);
    w.kill_enemy(0, Some(0));
    let at = w.enemies[0].pos;
    steps(&mut w, 60);
    assert_eq!(w.enemies[0].pos, at);
    assert_eq!(w.enemies[0].speed, 0.0);
}

#[test]
fn a_corpse_with_a_hero_in_its_cone_stays_calm() {
    // A brute facing the hero 3 yd away, who runs about loudly in front of it.
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 33.0, 30.0);
    w.enemies[0].facing = V2::new(1.0, 0.0);
    w.kill_enemy(0, None);
    w.drain_events();
    for k in 0..90 {
        let at = V2::new(33.0 + (k % 4) as f32 * 0.2, 30.0);
        stand(&mut w, 0, at, V2::new(-1.0, 0.0), Gait::Run);
        w.step();
    }
    assert!(!w.senses.alerted(0), "the corpse re-alerted");
    assert!(w.marks_for(0).is_empty());
    assert_eq!(w.enemies[0].speed, 0.0);
}

#[test]
fn a_shout_does_not_wake_a_corpse() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("brute", 30.0, 36.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.kill_enemy(1, None);
    // The first is alerted by a blow and shouts at the next step; the second lies dead beside it.
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 5);
    assert!(w.senses.alerted(0));
    assert!(!w.senses.alerted(1), "the corpse heard the shout");
    assert_eq!(w.marks_for(0).len(), 1);
}

#[test]
fn a_distraction_ends_by_timeout() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.land(
        0,
        ability(&w, "distract"),
        None,
        Some(V3::new(30.0, 38.0, 0.0)),
    );
    steps(&mut w, 5);
    assert!(w.combat.enemies[0].distracted.is_some());
    // Its ten seconds are up: the enemy faces the spot no longer, and the wait is over.
    steps(&mut w, 10 * 30);
    assert!(w.combat.enemies[0].distracted.is_none());
    assert_eq!(w.combat.enemies[0].engage.hold, Hold::Free);
}

#[test]
fn a_distraction_ends_when_the_enemy_is_alerted() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.land(
        0,
        ability(&w, "distract"),
        None,
        Some(V3::new(30.0, 38.0, 0.0)),
    );
    steps(&mut w, 5);
    assert!(w.combat.enemies[0].distracted.is_some());
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 2);
    assert!(w.combat.enemies[0].distracted.is_none());
    assert_eq!(w.combat.enemies[0].engage.hold, Hold::Free);
}

#[test]
fn a_sheep_beside_a_wall_stays_on_its_side() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    let mut b = flat(60.0);
    // A wall the whole length of the map, 1 yard east of the sheep: no way round it.
    b.add_box(V2::new(31.0, 0.0), V2::new(31.5, 60.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "sheep"), Some(0), None);
    let (mut furthest, mut east): (f32, f32) = (0.0, 0.0);
    for _ in 0..500 {
        w.step();
        furthest = furthest.max(w.enemies[0].pos.distance(V2::new(30.0, 30.0)));
        east = east.max(w.enemies[0].pos.x);
    }
    assert_eq!(w.enemies[0].fight.control, Some(ControlKind::Polymorph));
    assert!(furthest > 0.5, "the sheep did not wander: {furthest}");
    assert!(east < 31.0, "the sheep went through the wall: {east}");
}

#[test]
fn a_caster_that_sees_its_hero_over_a_low_wall_does_not_ask_for_a_path_again_and_again() {
    // A knee-high wall: nothing can walk over it, but a caster sees its hero across it, stops short
    // of them at its reach, and the hero shifting a little then makes it step and stop in turn.
    let mut w = world(&post("caster", 50.0, 30.0, 180.0));
    let mut b = flat(60.0);
    b.add_box(V2::new(25.0, 20.0), V2::new(25.5, 40.0), 0.0, 1.0);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    for k in 0..300 {
        let x = 10.0 + (k % 2) as f32 * 0.5;
        stand(&mut w, 0, V2::new(x, 30.0), V2::new(1.0, 0.0), Gait::Still);
        w.step();
    }
    // One search at the start; not one every other step (150 of them).
    let asked = w.senses.paths_asked(0);
    assert!((1..=2).contains(&asked), "{asked} paths asked for in 10 s");
    let d = w.enemies[0].pos.distance(V2::new(10.0, 30.0));
    assert!(d < 29.0, "it never closed in: {d}");
}

#[test]
fn a_slow_with_no_number_for_its_strength_slows_nothing() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    w.combat.enemies[0].slow = Some(Slowed {
        by: f32::NAN,
        left: 7.0,
        spell: 116,
    });
    steps(&mut w, 10);
    assert_eq!(w.combat.enemies[0].engage.speed, 1.0);
    assert!(
        (w.enemies[0].speed - 7.0).abs() < 0.05,
        "{}",
        w.enemies[0].speed
    );
}

#[test]
fn an_enemy_added_mid_run_leaves_the_others_as_they_were_and_acts_on_its_own() {
    use crate::enemies::Order;

    let nest =
        r#"(place: "Nest", kind: "brute", behaviour: Wander(center: (10.0, 50.0), radius: 5.0)),"#;
    let mut w = world(&format!(
        "{}{}{nest}",
        post("brute", 30.0, 30.0, 0.0),
        post("brute", 45.0, 30.0, 0.0)
    ));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let (alerted, sapped) = (w.enemies[0].id, w.enemies[1].id);
    w.alert(alerted, 0);
    w.land(0, ability(&w, "sap"), Some(1), None);
    steps(&mut w, 5);
    let marks = w.marks_for(0);
    assert!(marks.iter().any(|m| m.enemy == alerted));
    assert!(
        w.opener_allowed(sapped, 0),
        "the sapped one was never alerted"
    );
    let held = |w: &World| w.combat.enemies[1].held.map(|h| h.kind);
    assert_eq!(held(&w), Some(ControlKind::Sap));

    // A fourth enemy arrives on the nest's spawn line, far from the fight.
    let at = V3::new(50.0, 10.0, 0.0);
    let order = Order::Wander {
        center: at,
        radius: 5.0,
        height: None,
    };
    let id = w.add_enemy(2, at, V2::new(1.0, 0.0), order).unwrap();
    assert_eq!((w.enemies.len(), id), (4, 4));
    assert_eq!(w.enemies[3].hp, 1000.0);

    // The two already there keep their minds and their hold.
    assert!(w.senses.alerted(0));
    assert!(!w.senses.alerted(1));
    assert_eq!(held(&w), Some(ControlKind::Sap));
    assert_eq!(w.marks_for(0), marks);
    // The new one is calm, and goes about its business: wandering.
    assert!(!w.senses.alerted(3));
    assert!(w.marks_for(0).iter().all(|m| m.enemy != id));
    let start = w.enemies[3].pos;
    steps(&mut w, 600);
    assert!(w.enemies[3].pos.distance(start) > 0.1, "it never moved");
    assert!(w.senses.alerted(0), "the fight went on beside it");

    // It can be killed like any other.
    w.drain_events();
    w.kill_enemy(3, Some(0));
    assert_eq!(w.enemies[3].hp, 0.0);
    assert!(w
        .drain_events()
        .contains(&Event::EnemyDied { id, by: Some(0) }));

    // Without a map (a client's mirror) there is nothing to add to.
    let mut mirror = World::new(w.content().clone());
    assert!(mirror
        .add_enemy(
            0,
            at,
            V2::new(1.0, 0.0),
            Order::Sentry {
                post: at,
                facing: V2::new(1.0, 0.0)
            }
        )
        .is_none());
}
