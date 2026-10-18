//! Presses and casts: what the host refuses and why, casts and channels, the pace they allow,
//! auto-attack, Stealth's cooldown, and the walls and cover a blow needs a clear line through.

use super::*;

/// The last thing the host turned `player` 0 down for, if anything, taking the events.
fn refusal(w: &mut World) -> Option<Refusal> {
    w.drain_events().into_iter().rev().find_map(|e| match e {
        Event::Refused { player: 0, why } => Some(why),
        _ => None,
    })
}

/// Player 0 presses `slot` as a new press, the world steps, and what was refused (if anything).
fn attempt(
    w: &mut World,
    seq: &mut u8,
    slot: u8,
    target: Option<UnitRef>,
    at: Option<V3>,
) -> Option<Refusal> {
    *seq = seq.wrapping_add(1);
    press(w, 0, *seq, slot, target, at);
    w.step();
    refusal(w)
}

/// Player 0's next input: standing where they are, Stealth pressed `presses` times so far.
fn sneak(w: &mut World, presses: u8) {
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
            stealth_presses: presses,
            ..Default::default()
        },
    );
}

#[test]
fn a_press_repeated_in_every_input_casts_once() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let target = foe(&w, 0);
    for _ in 0..6 {
        press(&mut w, 0, 1, 3, target, None);
        w.step();
    }
    let gos = w
        .drain_events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::CastGo {
                    by: UnitRef::Hero(0),
                    ..
                }
            )
        })
        .count();
    assert_eq!(gos, 1);
    assert_eq!(w.hero(0).unwrap().fight.cooldowns.len(), 1);
    assert_eq!(w.enemies[0].hp, 850.0);
    assert_eq!(w.hero(0).unwrap().fight.power, 950.0);
}

#[test]
fn a_new_run_does_not_replay_the_last_press() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    w.add_hero(0, "mage", "Hero", 1, 0).unwrap();
    let target = foe(&w, 0);
    // The first input this host sees still carries a press from before: a baseline.
    press(&mut w, 0, 9, 3, target, None);
    w.step();
    press(&mut w, 0, 9, 3, target, None);
    w.step();
    assert!(w.drain_events().is_empty());
    stand(
        &mut w,
        0,
        V2::new(20.0, 30.0),
        V2::new(1.0, 0.0),
        Gait::Still,
    );
    w.step();
    press(&mut w, 0, 10, 3, target, None);
    w.step();
    assert!(w.drain_events().iter().any(|e| matches!(
        e,
        Event::CastGo {
            by: UnitRef::Hero(0),
            ..
        }
    )));
}

/// A bolt (2 s, slowed) begun 20 yards from a brute, the hero walking sideways at `speed`.
fn bolt_while_moving(speed: f32) -> Vec<Event> {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    for k in 1..=75 {
        let at = V2::new(10.0, 30.0 + speed * k as f32 * DT);
        stand(&mut w, 0, at, V2::new(1.0, 0.0), Gait::Run);
        w.step();
    }
    w.drain_events()
}

#[test]
fn a_slowed_cast_allows_the_slowed_pace_and_breaks_beyond_it() {
    // 3 yards a second stays under the slowed pace (3.5 after the grace): the bolt goes off.
    let slow = bolt_while_moving(3.0);
    assert!(slow.iter().any(|e| matches!(
        e,
        Event::CastGo {
            by: UnitRef::Hero(0),
            ..
        }
    )));
    // 5 yards a second outruns it about a second in.
    let fast = bolt_while_moving(5.0);
    assert!(fast.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::Moved
    }));
    assert!(!fast.iter().any(|e| matches!(e, Event::CastGo { .. })));
}

#[test]
fn a_rooted_cast_cannot_begin_on_the_move_and_breaks_when_the_hero_moves() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(10.0, 30.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            gait: Gait::Run,
            cast: Some(CastPress {
                seq: 1,
                slot: 2,
                target,
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(refusal(&mut w), Some(Refusal::Moving));
    press(&mut w, 0, 2, 2, target, None);
    w.step();
    assert!(w.hero(0).unwrap().fight.rooted);
    stand(&mut w, 0, V2::new(10.0, 31.0), V2::new(1.0, 0.0), Gait::Run);
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 1,
        why: Stop::Moved
    }));
    assert!(!w.hero(0).unwrap().fight.rooted);
}

#[test]
fn a_rogues_presses_are_refused_for_what_is_wrong() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("bones", 30.0, 33.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 27.0, 30.0);
    let (brute, bones) = (foe(&w, 0), foe(&w, 1));
    let north = V2::new(1.0, 0.0);
    let mut seq = 0;
    assert_eq!(
        attempt(&mut w, &mut seq, 2, None, None),
        Some(Refusal::NoTarget)
    );
    assert_eq!(
        attempt(&mut w, &mut seq, 3, brute, None),
        Some(Refusal::NeedCombo)
    );
    assert_eq!(
        attempt(&mut w, &mut seq, 74, brute, None),
        Some(Refusal::NeedStealth)
    );
    assert_eq!(
        attempt(&mut w, &mut seq, 11, None, None),
        Some(Refusal::NoTarget)
    );
    let far = Some(V3::new(27.0, 59.0, 0.0));
    assert_eq!(
        attempt(&mut w, &mut seq, 11, None, far),
        Some(Refusal::OutOfRange)
    );
    w.heroes[0].stealthed = true;
    assert_eq!(
        attempt(&mut w, &mut seq, 77, bones, None),
        Some(Refusal::BadTarget)
    );
    stand(&mut w, 0, V2::new(27.0, 30.0), north * -1.0, Gait::Still);
    w.step();
    assert_eq!(
        attempt(&mut w, &mut seq, 75, brute, None),
        Some(Refusal::Behind)
    );
    assert_eq!(
        attempt(&mut w, &mut seq, 2, brute, None),
        Some(Refusal::Facing)
    );
    stand(&mut w, 0, V2::new(10.0, 30.0), north, Gait::Still);
    w.step();
    assert_eq!(
        attempt(&mut w, &mut seq, 2, brute, None),
        Some(Refusal::OutOfRange)
    );
    seq += 1;
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(27.0, 30.0),
                facing: north,
                z: 0.0,
            }),
            gait: Gait::Run,
            cast: Some(CastPress {
                seq,
                slot: 12,
                target: None,
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(refusal(&mut w), Some(Refusal::Moving));
    // A blow that lands, then another inside the global cooldown.
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), None);
    assert_eq!(
        attempt(&mut w, &mut seq, 2, brute, None),
        Some(Refusal::NotReady)
    );
    steps(&mut w, 40);
    assert_eq!(
        attempt(&mut w, &mut seq, 12, None, None),
        Some(Refusal::InCombat)
    );
    w.heroes[0].stealthed = true;
    assert_eq!(
        attempt(&mut w, &mut seq, 74, brute, None),
        Some(Refusal::Aware)
    );
    w.heroes[0].fight.power = 0.0;
    assert_eq!(
        attempt(&mut w, &mut seq, 2, brute, None),
        Some(Refusal::NoPower)
    );
}

#[test]
fn a_mages_presses_are_refused_for_what_is_wrong() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("beast", 30.0, 50.0, 0.0)
    );
    let mut w = world(&spawns);
    let mut b = flat(60.0);
    house(&mut b, V2::new(15.0, 45.0), V2::new(25.0, 55.0), None, true);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 50.0);
    let (brute, beast) = (foe(&w, 0), foe(&w, 1));
    let mut seq = 0;
    assert_eq!(
        attempt(&mut w, &mut seq, 1, beast, None),
        Some(Refusal::NotInSight)
    );
    stand(
        &mut w,
        0,
        V2::new(10.0, 30.0),
        V2::new(1.0, 0.0),
        Gait::Still,
    );
    w.step();
    assert_eq!(attempt(&mut w, &mut seq, 1, brute, None), None);
    assert_eq!(
        attempt(&mut w, &mut seq, 3, brute, None),
        Some(Refusal::Busy)
    );
    steps(&mut w, 70);
    let me = Some(UnitRef::Hero(0));
    assert_eq!(
        attempt(&mut w, &mut seq, 6, me, None),
        Some(Refusal::BadTarget)
    );
}

#[test]
fn a_channel_pays_up_front_and_ticks() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 5, target, None);
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.power, 950.0);
    steps(&mut w, 95);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, target.unwrap()), vec![15.0, 10.0, 10.0]);
    assert!(events.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 3,
        why: Stop::Finished
    }));
    assert!(w.hero(0).unwrap().fight.casting.is_none());
}

#[test]
fn a_cast_whose_target_dies_stops_and_costs_nothing() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    w.kill_enemy(0, None);
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::TargetLost
    }));
    assert_eq!(w.hero(0).unwrap().fight.power, 1000.0);
}

#[test]
fn auto_attack_follows_a_melee_blow_and_stops_in_stealth() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 27.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 2, target, None);
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.attacking, Some(w.enemies[0].id));
    steps(&mut w, 70);
    let swings = w
        .drain_events()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Swing {
                    by: UnitRef::Hero(0),
                    ..
                }
            )
        })
        .count();
    assert!(swings >= 2, "{swings}");
    w.heroes[0].stealthed = true;
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.attacking, None);
}

#[test]
fn nothing_comes_back_in_combat_but_energy_does() {
    let mut w = world_with("regen: (10.0, 10.0)", &post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.heroes[0].hp = 500.0;
    w.heroes[0].fight.power = 0.0;
    steps(&mut w, 30);
    assert!((w.heroes[0].hp - 510.0).abs() < 0.5, "{}", w.heroes[0].hp);
    assert!((w.heroes[0].fight.power - 10.0).abs() < 0.5);
    w.alert(w.enemies[0].id, 0);
    w.step();
    let (hp, power) = (w.heroes[0].hp, w.heroes[0].fight.power);
    steps(&mut w, 30);
    assert!(w.heroes[0].hp <= hp);
    assert!(w.heroes[0].fight.power > power + 9.0);
    assert!(w.hero(0).unwrap().fight.in_combat);
}

#[test]
fn eating_heals_while_still_and_a_step_ends_it() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.heroes[0].hp = 400.0;
    press(&mut w, 0, 1, 12, None, None);
    w.step();
    steps(&mut w, 29);
    let healed = w.heroes[0].hp;
    assert!((healed - (400.0 + 1000.0 / 15.0)).abs() < 2.0, "{healed}");
    stand(
        &mut w,
        0,
        V2::new(11.0, 30.0),
        V2::new(1.0, 0.0),
        Gait::Walk,
    );
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.eating, 0.0);
    let after = w.heroes[0].hp;
    steps(&mut w, 30);
    assert_eq!(w.heroes[0].hp, after);
}

#[test]
fn sprint_and_a_slowed_cast_set_the_pace() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    press(&mut w, 0, 1, 5, None, None);
    w.step();
    assert!((w.heroes[0].speed - 10.5).abs() < 1e-4);
    steps(&mut w, 241);
    assert!((w.heroes[0].speed - 7.0).abs() < 1e-4);
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    assert!((w.heroes[0].speed - 3.5).abs() < 1e-4);
}

#[test]
fn stealth_cools_down_once_it_ends() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    sneak(&mut w, 1);
    w.step();
    assert!(w.heroes[0].stealthed);
    w.break_stealth(0);
    w.step();
    assert!(!w.heroes[0].stealthed);
    assert!(!w.may_stealth(0));
    sneak(&mut w, 2);
    w.step();
    assert!(!w.heroes[0].stealthed);
    steps(&mut w, 300);
    sneak(&mut w, 3);
    w.step();
    assert!(w.heroes[0].stealthed);
}

/// A rogue 3 yards from a brute with a thin wall between them (a house's west wall, the brute
/// inside), or in the open.
fn rogue_by_a_brute(walled: bool) -> World {
    let mut w = world(&post("brute", 12.0, 30.0, 0.0));
    if walled {
        let mut b = flat(60.0);
        b.add_box(V2::new(10.0, 25.0), V2::new(10.5, 35.0), 0.0, 3.0);
        w.attach_map(Arc::new(b.finish()));
    }
    join(&mut w, 0, "rogue", 9.0, 30.0);
    w
}

#[test]
fn a_melee_blow_needs_a_clear_line_at_point_blank() {
    let mut w = rogue_by_a_brute(true);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 2, target, None);
    w.step();
    assert_eq!(refusal(&mut w), Some(Refusal::NotInSight));
    assert_eq!(w.enemies[0].hp, 1000.0);
    // In the open, the same blow lands.
    let mut w = rogue_by_a_brute(false);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 2, target, None);
    w.step();
    assert_eq!(refusal(&mut w), None);
    assert!(w.enemies[0].hp < 1000.0);
}

#[test]
fn auto_attack_does_not_swing_through_a_wall() {
    for (walled, swings) in [(true, false), (false, true)] {
        let mut w = rogue_by_a_brute(walled);
        let id = w.enemies[0].id;
        w.heroes[0].fight.attacking = Some(id);
        steps(&mut w, 70);
        let swung = w.drain_events().iter().any(|e| {
            matches!(
                e,
                Event::Swing {
                    by: UnitRef::Hero(0),
                    ..
                }
            )
        });
        assert_eq!(swung, swings, "walled: {walled}");
    }
}

#[test]
fn a_press_with_a_target_that_is_not_there_or_a_spot_that_is_not_a_number_is_refused() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let mut seq = 0;
    let nowhere = Some(UnitRef::Enemy(9999));
    assert_eq!(
        attempt(&mut w, &mut seq, 1, nowhere, None),
        Some(Refusal::NoTarget)
    );
    let nan = Some(V3::new(f32::NAN, 1.0, 0.0));
    let mut w = world("");
    join(&mut w, 0, "rogue", 20.0, 30.0);
    assert_eq!(
        attempt(&mut w, &mut seq, 11, None, nan),
        Some(Refusal::NoTarget)
    );
    let inf = Some(V3::new(20.0, f32::INFINITY, 0.0));
    assert_eq!(
        attempt(&mut w, &mut seq, 11, None, inf),
        Some(Refusal::NoTarget)
    );
}

#[test]
fn a_finisher_aimed_at_another_enemy_is_refused() {
    let spawns = format!(
        "{}{}",
        post("brute", 30.0, 30.0, 0.0),
        post("bones", 30.0, 33.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 27.0, 30.0);
    let (brute, bones) = (foe(&w, 0), foe(&w, 1));
    let mut seq = 0;
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), None);
    assert!(w.hero(0).unwrap().fight.combo > 0);
    steps(&mut w, 40);
    assert_eq!(
        attempt(&mut w, &mut seq, 3, bones, None),
        Some(Refusal::NeedCombo)
    );
}

#[test]
fn an_ability_breaks_stealth_unless_it_keeps_it() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 27.0, 30.0);
    w.heroes[0].stealthed = true;
    // Sprint keeps stealth; Strike does not.
    press(&mut w, 0, 1, 5, None, None);
    w.step();
    assert!(w.heroes[0].stealthed);
    let target = foe(&w, 0);
    press(&mut w, 0, 2, 2, target, None);
    w.step();
    assert!(w.drain_events().iter().any(|e| matches!(
        e,
        Event::CastGo {
            by: UnitRef::Hero(0),
            ..
        }
    )));
    assert!(!w.heroes[0].stealthed);
}

/// A world with a thin, cell-aligned wall standing from (20, 35) north to the map's edge, and
/// the hero's class `class` at (10, 30) with the spawns in the open.
fn world_with_a_wall(spawns: &str, class: &str) -> World {
    let mut w = world(spawns);
    let mut b = flat(60.0);
    b.add_box(V2::new(20.0, 35.0), V2::new(20.5, 60.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, class, 10.0, 30.0);
    w
}

#[test]
fn a_channel_breaks_when_its_target_steps_behind_cover() {
    let mut w = world_with_a_wall(&post("brute", 30.0, 30.0, 0.0), "mage");
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 5, target, None);
    steps(&mut w, 35);
    let first = w.drain_events();
    assert_eq!(damage_to(&first, target.unwrap()).len(), 1);
    // The brute is moved behind the wall (30, 50 is within range; the line crosses x 20 at y 40).
    w.enemies[0].pos = V2::new(30.0, 50.0);
    steps(&mut w, 70);
    let rest = w.drain_events();
    assert!(damage_to(&rest, target.unwrap()).is_empty());
    assert!(rest.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 3,
        why: Stop::TargetLost
    }));
    assert!(w.hero(0).unwrap().fight.casting.is_none());
}

#[test]
fn a_cast_whose_target_steps_behind_cover_stops_and_costs_nothing() {
    let mut w = world_with_a_wall(&post("brute", 30.0, 30.0, 0.0), "mage");
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    w.enemies[0].pos = V2::new(30.0, 50.0);
    steps(&mut w, 70);
    let events = w.drain_events();
    assert!(events.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::TargetLost
    }));
    assert!(!events.iter().any(|e| matches!(e, Event::CastGo { .. })));
    assert_eq!(w.hero(0).unwrap().fight.power, 1000.0);
}

#[test]
fn auto_attack_swings_again_once_the_enemy_is_back_in_reach_and_sight() {
    let mut w = world_with_a_wall(&post("brute", 22.0, 40.0, 0.0), "rogue");
    let id = w.enemies[0].id;
    w.heroes[0].fight.attacking = Some(id);
    // The brute is held where it stands, so only the hero's place changes; the swings counted
    // are those of a stay of 70 steps.
    let stay = |w: &mut World, at: V2, facing: V2| {
        stand(w, 0, at, facing, Gait::Still);
        for _ in 0..70 {
            w.enemies[0].pos = V2::new(22.0, 40.0);
            w.step();
        }
        w.drain_events()
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Swing {
                        by: UnitRef::Hero(0),
                        ..
                    }
                )
            })
            .count()
    };
    let (west, east) = (V2::new(-1.0, 0.0), V2::new(1.0, 0.0));
    assert!(stay(&mut w, V2::new(24.0, 40.0), west) >= 2);
    // Out of reach, then through the wall: nothing swings, and the attack is kept.
    assert_eq!(stay(&mut w, V2::new(40.0, 40.0), west), 0);
    assert_eq!(stay(&mut w, V2::new(19.0, 40.0), east), 0);
    assert_eq!(w.hero(0).unwrap().fight.attacking, Some(id));
    // Back in reach and sight, it swings again.
    assert!(stay(&mut w, V2::new(24.0, 40.0), west) >= 1);
}

#[test]
fn stealth_cools_down_even_when_a_press_breaks_it_in_the_step_it_began() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    // Stealth and Eat (which breaks stealth) in one input.
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
            stealth_presses: 1,
            cast: Some(CastPress {
                seq: 1,
                slot: 12,
                target: None,
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert!(!w.heroes[0].stealthed);
    assert!(!w.may_stealth(0));
    sneak(&mut w, 2);
    w.step();
    assert!(!w.heroes[0].stealthed);
}

#[test]
fn a_refused_step_into_stealth_is_told_to_its_player() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    // On cooldown: stealth begins and ends, then is pressed again.
    sneak(&mut w, 1);
    w.step();
    assert!(w.heroes[0].stealthed);
    assert_eq!(refusal(&mut w), None);
    w.break_stealth(0);
    w.step();
    sneak(&mut w, 2);
    w.step();
    assert_eq!(refusal(&mut w), Some(Refusal::NotReady));
    // In combat.
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    w.step();
    sneak(&mut w, 1);
    w.step();
    assert!(!w.heroes[0].stealthed);
    assert_eq!(refusal(&mut w), Some(Refusal::InCombat));
    // Stepping out is never refused.
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    sneak(&mut w, 1);
    w.step();
    sneak(&mut w, 2);
    w.step();
    assert!(!w.heroes[0].stealthed);
    assert_eq!(refusal(&mut w), None);
}

/// A rogue sprinting (10.5 yards a second against 7), the press stepped.
fn sprinting_rogue() -> World {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    press(&mut w, 0, 1, 5, None, None);
    w.step();
    assert!((w.heroes[0].speed - 10.5).abs() < 1e-4);
    w
}

#[test]
fn setting_the_pace_again_changes_nothing() {
    let mut w = sprinting_rogue();
    w.set_pace(0);
    w.set_pace(0);
    assert!(
        (w.heroes[0].speed - 10.5).abs() < 1e-4,
        "{}",
        w.heroes[0].speed
    );
}

#[test]
fn a_sprinting_hero_hurt_between_steps_keeps_sprint_speed_in_the_next_snapshot() {
    let mut w = sprinting_rogue();
    w.hurt_hero(0, 10.0);
    // Hurt breaks stealth and works the senses' speed out afresh; the Sprint is laid on again.
    assert!((w.snapshot().heroes[0].speed - 10.5).abs() < 1e-4);
    // So do the other seams that work it out afresh.
    w.break_stealth(0);
    assert!((w.snapshot().heroes[0].speed - 10.5).abs() < 1e-4);
    w.flash(0);
    assert!((w.snapshot().heroes[0].speed - 10.5).abs() < 1e-4);
}

#[test]
fn a_hero_downed_between_steps_shows_crawl_speed() {
    let mut w = sprinting_rogue();
    w.hurt_hero(0, 5000.0);
    assert!(!w.hero(0).unwrap().fight.up());
    assert!((w.snapshot().heroes[0].speed - 7.0 * 0.3).abs() < 1e-4);
}
