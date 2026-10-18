//! Heroes falling: downed and crawling, revived by a teammate, dead, and what the enemies make of it.

use super::*;

#[test]
fn a_hero_at_zero_is_downed_and_crawls() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.heroes[0].stealthed = true;
    w.hurt_hero(0, 5000.0);
    let h = w.hero(0).unwrap();
    assert!(matches!(h.fight.life, Life::Downed { .. }));
    assert_eq!(h.hp, 0.0);
    assert!(!h.stealthed);
    assert!(w.drain_events().contains(&Event::Downed { player: 0 }));
    w.step();
    assert!((w.heroes[0].speed - 7.0 * 0.3).abs() < 1e-4);
}

#[test]
fn a_teammate_standing_by_revives_in_five_seconds_with_a_noise() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 12.0, 30.0);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 2);
    assert_eq!(w.hero(1).unwrap().noise, 20.0);
    steps(&mut w, 149);
    let h = w.hero(0).unwrap();
    assert!(h.fight.up());
    assert!((h.hp - 1000.0 * 0.3).abs() < 1e-3);
    assert!(w
        .drain_events()
        .contains(&Event::Revived { player: 0, by: 1 }));
}

#[test]
fn walking_away_starts_the_revive_over() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 12.0, 30.0);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 120);
    stand(&mut w, 1, V2::new(30.0, 30.0), V2::new(1.0, 0.0), Gait::Run);
    w.step();
    assert!(matches!(
        w.hero(0).unwrap().fight.life,
        Life::Downed { revive, .. } if revive == 0.0
    ));
    stand(&mut w, 1, V2::new(12.0, 30.0), V2::new(1.0, 0.0), Gait::Run);
    steps(&mut w, 120);
    assert!(!w.hero(0).unwrap().fight.up());
    steps(&mut w, 40);
    assert!(w.hero(0).unwrap().fight.up());
}

#[test]
fn unrevived_for_45_seconds_is_dead_and_cannot_press() {
    let mut w = world(&post("brute", 50.0, 50.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.hurt_hero(0, 5000.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 2, target, None);
    w.step();
    assert!(w.drain_events().contains(&Event::Refused {
        player: 0,
        why: Refusal::Down
    }));
    steps(&mut w, 45 * 30);
    assert_eq!(w.hero(0).unwrap().fight.life, Life::Dead);
    assert!(w.drain_events().contains(&Event::Died { player: 0 }));
    assert!(w.hero(0).unwrap().fight.rooted);
    assert_eq!(w.hero(0).unwrap().speed, 0.0);
    w.hurt_hero(0, 10.0);
    assert!(w.drain_events().is_empty());
}

#[test]
fn an_enemy_leaves_a_downed_hero_alone() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    steps(&mut w, 5);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 6 * 30);
    let alerted = Mark {
        enemy: id,
        alert: Alert::Alerted,
    };
    assert!(!w.marks_for(0).contains(&alerted));
    steps(&mut w, 25 * 30);
    assert!(w.marks_for(0).is_empty());
}

#[test]
fn a_blow_that_is_not_a_positive_number_does_not_hurt_a_hero() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    for amount in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -5.0, 0.0] {
        w.hurt_hero(0, amount);
    }
    w.hurt_hero(7, 10.0);
    assert_eq!(w.hero(0).unwrap().hp, 1000.0);
    assert!(w.hero(0).unwrap().fight.up());
    assert!(w.drain_events().is_empty());
    w.hurt_hero(0, 100.0);
    assert_eq!(w.hero(0).unwrap().hp, 900.0);
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Hero(0)), vec![100.0]);
}

#[test]
fn a_hero_who_goes_down_stops_casting_eating_and_swinging() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    assert!(w.heroes[0].fight.casting.is_some());
    w.heroes[0].fight.attacking = Some(w.enemies[0].id);
    w.heroes[0].fight.eating = 10.0;
    w.drain_events();
    w.hurt_hero(0, 5000.0);
    let f = &w.hero(0).unwrap().fight;
    assert!(f.casting.is_none());
    assert_eq!((f.attacking, f.eating), (None, 0.0));
    assert!(w.drain_events().iter().any(|e| matches!(
        e,
        Event::CastStop {
            by: UnitRef::Hero(0),
            why: Stop::Interrupted,
            ..
        }
    )));
    // Nothing goes off for the cast that was under way, and it cannot be begun again.
    steps(&mut w, 90);
    assert_eq!(w.enemies[0].hp, 1000.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 2, 1, target, None);
    w.step();
    assert!(w.drain_events().contains(&Event::Refused {
        player: 0,
        why: Refusal::Down
    }));
}

#[test]
fn a_reviver_who_goes_down_starts_the_revive_over() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 12.0, 30.0);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 120);
    w.hurt_hero(1, 5000.0);
    steps(&mut w, 60);
    // Two heroes down side by side: neither revives the other, and both run out of time.
    assert!(matches!(
        w.hero(0).unwrap().fight.life,
        Life::Downed { revive, .. } if revive == 0.0
    ));
    steps(&mut w, 45 * 30);
    assert_eq!(w.hero(0).unwrap().fight.life, Life::Dead);
    assert_eq!(w.hero(1).unwrap().fight.life, Life::Dead);
}

#[test]
fn a_reviver_out_of_reach_does_not_begin_a_revive() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 14.0, 30.0);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 200);
    assert!(!w.hero(0).unwrap().fight.up());
    assert_eq!(w.hero(1).unwrap().noise, 0.0);
}

#[test]
fn an_enemy_after_a_hero_loses_them_the_moment_they_go_down() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    steps(&mut w, 5);
    assert!(w.senses.alerted(0));
    w.hurt_hero(0, 5000.0);
    // Not alerted any more, with no step between: it would otherwise run on for `lose_after`.
    assert!(!w.senses.alerted(0));
    assert!(!w.senses.hunted(0));
}

#[test]
fn a_downed_hero_makes_no_footsteps_even_when_the_client_says_run() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 22.0, 30.0);
    w.hurt_hero(0, 5000.0);
    for _ in 0..120 {
        stand(&mut w, 0, V2::new(22.0, 30.0), V2::new(1.0, 0.0), Gait::Run);
        w.step();
        assert_eq!(w.hero(0).unwrap().noise, 0.0);
    }
    assert_eq!(w.hero(0).unwrap().stance, crate::Stance::Still);
    assert!(w.marks_for(0).is_empty());
    // The same footsteps of a hero who is up are heard from the same spot.
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 22.0, 30.0);
    stand(&mut w, 0, V2::new(22.0, 30.0), V2::new(1.0, 0.0), Gait::Run);
    w.step();
    assert!(w.hero(0).unwrap().noise > 0.0);
}

/// A rogue at (10, 30), downed if `downed`, and a mage at (50, 30), a calm brute 15 yards from
/// the rogue (and facing away from both); the mage walks to the rogue at `at` seconds. Returns
/// the world with that input set.
fn mage_arrives_beside_a_sentry(at: f32, downed: bool) -> World {
    let mut w = world(&post("brute", 25.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 50.0, 30.0);
    if downed {
        w.hurt_hero(0, 5000.0);
    }
    steps(&mut w, (at * 30.0) as usize);
    stand(
        &mut w,
        1,
        V2::new(12.0, 30.0),
        V2::new(1.0, 0.0),
        Gait::Still,
    );
    w
}

fn revive_beside_a_sentry(at: f32) -> World {
    mage_arrives_beside_a_sentry(at, true)
}

#[test]
fn a_revive_is_heard_by_a_calm_enemy_within_twenty_yards() {
    let mut w = revive_beside_a_sentry(0.0);
    let noise = w.content().rules.senses.awareness.noise;
    assert_eq!(w.senses.suspicion_of(0, 1), 0.0);
    w.step();
    assert_eq!(w.senses.suspicion_of(0, 1), noise);
    // The same jump of the mage to an up rogue, nobody to revive, is not noticed at all.
    let mut w = mage_arrives_beside_a_sentry(0.0, false);
    steps(&mut w, 10);
    assert_eq!(w.senses.suspicion_of(0, 1), 0.0);
}

#[test]
fn a_reviver_arriving_at_forty_seconds_is_in_time_and_one_at_forty_one_is_late() {
    let mut w = revive_beside_a_sentry(40.0);
    steps(&mut w, 200);
    assert!(w.hero(0).unwrap().fight.up());
    assert!(w
        .drain_events()
        .contains(&Event::Revived { player: 0, by: 1 }));
    let mut w = revive_beside_a_sentry(41.0);
    steps(&mut w, 200);
    assert_eq!(w.hero(0).unwrap().fight.life, Life::Dead);
    let events = w.drain_events();
    assert!(events.contains(&Event::Died { player: 0 }));
    assert!(!events.iter().any(|e| matches!(e, Event::Revived { .. })));
}

#[test]
fn a_shout_raised_for_a_hero_downed_in_the_same_step_sends_no_ally_to_the_body() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("brute", 30.0, 36.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 28.0, 30.0);
    // A blow of the hero's alerts the first, which shouts at the next step; the hero is downed
    // before the shout is given out.
    w.alert(w.enemies[0].id, 0);
    w.hurt_hero(0, 5000.0);
    assert!(!w.hero(0).unwrap().fight.up());
    steps(&mut w, 5);
    assert!(!w.senses.alerted(1), "the ally went to the fallen hero");
    // The same shout for a hero still up does wake the ally.
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 28.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 5);
    assert!(w.senses.alerted(1));
}
