//! The fight in whole worlds on small hand-made maps: what abilities do, presses and casts,
//! enemies held and hurt, heroes falling, and enemies striking back at heroes who dodge.

use std::sync::Arc;

use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Alert, CastPress, Content, ContentSources, ControlKind, Event, Gait, HeroPose, Mark, PlayerId,
    PlayerInput, UnitRef, World, V2, V3,
};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0,
     swing: Some((min: 30.0, max: 30.0, every: 2.0)),
     bar: [(1, "stealth"), (2, "strike"), (3, "finish"), (4, "gouge"), (5, "sprint"),
           (6, "distract"), (7, "vanish"), (11, "stone"), (12, "eat"), (74, "ambush"),
           (75, "garrote"), (76, "cheap"), (77, "sap")]),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 800.0,
     speed: 7.0, power_max: 1000.0,
     bar: [(1, "bolt"), (2, "nuke"), (3, "blast"), (4, "nova"), (5, "missiles"), (6, "sheep"),
           (7, "blink"), (8, "water"), (11, "stone"), (12, "eat")]),
]"#;
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)],
    entries: [(5.0, 5.0, 0.0)])"#;
const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 1000.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 40.0, max: 40.0, every: 2.0))),
    (key: "beast", name: "Beast", display: 955, level: 26, hp: 800.0, walk_speed: 2.2,
     run_speed: 7.0, creature: Beast),
    (key: "bones", name: "Bones", display: 200, level: 28, hp: 900.0, walk_speed: 2.5,
     run_speed: 7.0, creature: Undead),
    (key: "caster", name: "Caster", display: 203, level: 28, hp: 900.0, walk_speed: 2.5,
     run_speed: 7.0, moves: [(ability: "smite", every: 5.0)]),
    (key: "elite", name: "Elite", display: 657, level: 31, hp: 3000.0, walk_speed: 2.5,
     run_speed: 8.0, elite: true, moves: [(ability: "cleave", every: 8.0)]),
    (key: "stomper", name: "Stomper", display: 955, level: 30, hp: 3000.0, walk_speed: 2.2,
     run_speed: 7.0, elite: true, moves: [(ability: "stomp", every: 8.0)]),
]"#;
const ABILITIES: &str = r#"[
    (key: "bolt", name: "Bolt", spell: 116, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
     gcd: 1.5, moving: Slowed, facing: Front, effects: [Damage(min: 100.0, max: 100.0)]),
    (key: "nuke", name: "Nuke", spell: 133, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
     gcd: 1.5, moving: Rooted, sound: Loud, flash: true, facing: Front,
     effects: [Damage(min: 100.0, max: 100.0)]),
    (key: "blast", name: "Blast", spell: 2136, aim: Enemy, range: 20.0, cost: 50.0,
     cooldown: 8.0, gcd: 1.5, sound: Loud, effects: [Damage(min: 100.0, max: 100.0)]),
    (key: "missiles", name: "Missiles", spell: 5143, aim: Enemy, range: 30.0, cost: 50.0,
     channel: 3.0, ticks: 3, gcd: 1.5, moving: Rooted,
     effects: [Damage(min: 10.0, max: 10.0)]),
    (key: "nova", name: "Nova", spell: 122, aim: Me, radius: 10.0, cost: 50.0, cooldown: 25.0,
     gcd: 1.5, effects: [Damage(min: 10.0, max: 10.0), Control(kind: Root, secs: 8.0)]),
    (key: "sheep", name: "Sheep", spell: 118, aim: Enemy, range: 30.0, cost: 50.0, cast: 1.5,
     gcd: 1.5, moving: Slowed, needs: (creatures: [Humanoid, Beast]),
     effects: [Control(kind: Polymorph, secs: 20.0)]),
    (key: "blink", name: "Blink", spell: 1953, aim: Me, cooldown: 15.0, gcd: 1.5,
     effects: [Blink(distance: 20.0)]),
    (key: "water", name: "Water", spell: 6127, aim: Me, cost: 50.0, cast: 3.0, gcd: 1.5,
     moving: Rooted, needs: (out_of_combat: true), effects: [ConjureWater(count: 2)]),
    (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0, sound: Silent,
     keeps_stealth: true, needs: (out_of_combat: true), effects: [Stealth]),
    (key: "sap", name: "Sap", spell: 2070, aim: Enemy, range: 5.0, cost: 65.0, gcd: 1.0,
     sound: Quiet, alerts: false, keeps_stealth: true,
     needs: (stealth: true, unaware: true, creatures: [Humanoid]),
     effects: [Control(kind: Sap, secs: 30.0)]),
    (key: "cheap", name: "Cheap", spell: 1833, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     attack: true, needs: (stealth: true, unaware: true),
     effects: [Control(kind: Stun, secs: 4.0), Combo(2)]),
    (key: "garrote", name: "Garrote", spell: 8632, aim: Enemy, range: 5.0, cost: 50.0, gcd: 1.0,
     sound: Silent, facing: Behind, attack: true, needs: (stealth: true, unaware: true),
     effects: [Dot(amount: 30.0, every: 3.0, secs: 18.0), Combo(1)]),
    (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     facing: Behind, attack: true, needs: (stealth: true, unaware: true),
     effects: [Weapon(percent: 250.0, bonus: 40.0), Combo(1)]),
    (key: "strike", name: "Strike", spell: 1760, aim: Enemy, range: 5.0, cost: 45.0, gcd: 1.0,
     facing: Front, attack: true, effects: [Weapon(bonus: 20.0), Combo(1)]),
    (key: "finish", name: "Finish", spell: 6762, aim: Enemy, range: 5.0, cost: 35.0, gcd: 1.0,
     facing: Front, needs: (combo: true),
     effects: [Damage(min: 10.0, max: 10.0, per_combo: 30.0)]),
    (key: "gouge", name: "Gouge", spell: 1777, aim: Enemy, range: 5.0, cost: 45.0,
     cooldown: 10.0, gcd: 1.0, facing: Front,
     effects: [Damage(min: 20.0, max: 20.0), Control(kind: Incapacitate, secs: 4.0), Combo(1)]),
    (key: "sprint", name: "Sprint", spell: 2983, aim: Me, cooldown: 300.0, sound: Silent,
     keeps_stealth: true, effects: [Sprint(by: 0.5, secs: 8.0)]),
    (key: "distract", name: "Distract", spell: 1725, aim: Ground, range: 30.0, radius: 10.0,
     cost: 30.0, cooldown: 30.0, gcd: 1.0, sound: Loud, alerts: false, keeps_stealth: true,
     effects: [Distract(secs: 10.0)]),
    (key: "vanish", name: "Vanish", spell: 1856, aim: Me, cooldown: 300.0, sound: Quiet,
     keeps_stealth: true, effects: [Vanish]),
    (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0, cooldown: 20.0,
     alerts: false, keeps_stealth: true, effects: []),
    (key: "eat", name: "Eat", spell: 2639, aim: Me, sound: Quiet,
     needs: (out_of_combat: true, still: true), effects: [Eat(secs: 15.0)]),
    (key: "smite", name: "Smite", spell: 1106, aim: Enemy, range: 30.0, cast: 2.0,
     facing: Front, effects: [Damage(min: 50.0, max: 50.0)]),
    (key: "cleave", name: "Cleave", spell: 845, aim: Enemy, range: 8.0, sound: Loud,
     telegraph: Some((shape: Cone(length: 8.0, angle: 120.0), windup: 1.2)),
     effects: [Damage(min: 200.0, max: 200.0)]),
    (key: "stomp", name: "Stomp", spell: 45, aim: Me, radius: 8.0,
     telegraph: Some((shape: Circle(radius: 8.0), windup: 1.5, on_self: true)),
     effects: [Damage(min: 150.0, max: 150.0)]),
]"#;

/// The tests' rules: crits off so damage is exact, with `combat` (more `combat:` fields) on top.
fn rules(combat: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0, {combat}))"#
    )
}

/// A world on flat ground 60 yards across, its map attached, with `spawns` (RON list items, each
/// ending in a comma) and `combat` rules on top of the tests' own.
pub(super) fn world_with(combat: &str, spawns: &str) -> World {
    let content = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(combat),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
    })
    .unwrap();
    let mut w = World::new(Arc::new(content));
    w.attach_map(open());
    w
}

pub(super) fn world(spawns: &str) -> World {
    world_with("", spawns)
}

/// Flat ground 60 yards across.
pub(super) fn open() -> Arc<HeightMap> {
    Arc::new(flat(60.0).finish())
}

/// A `kind` sentry at `(x, y)` facing `facing` degrees (0 north, 90 west), as a spawn line.
pub(super) fn post(kind: &str, x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "{kind}", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// `player`'s hero of `class` at `(x, y)` facing north and standing, its first input (the
/// presses' baseline) taken.
pub(super) fn join(w: &mut World, player: PlayerId, class: &str, x: f32, y: f32) {
    w.add_hero(player, class, "Hero", 1, 0).unwrap();
    stand(w, player, V2::new(x, y), V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

/// `player`'s next input: their hero at `at`, facing `facing`, moving at `gait`.
pub(super) fn stand(w: &mut World, player: PlayerId, at: V2, facing: V2, gait: Gait) {
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: at,
                facing,
                z: 0.0,
            }),
            gait,
            ..Default::default()
        },
    );
}

/// `player`'s next input presses action `slot` (press number `seq`) at `target` or the spot
/// `at`, the hero standing where they are. Bind the target first (`let target = foe(&w, 0);`):
/// `foe(&w, 0)` inside the call is a shared borrow of the world the call borrows mutably.
// Used from the presses' tests (Task 5) on.
#[allow(dead_code)]
pub(super) fn press(
    w: &mut World,
    player: PlayerId,
    seq: u8,
    slot: u8,
    target: Option<UnitRef>,
    at: Option<V3>,
) {
    let h = w.hero(player).unwrap();
    let pose = HeroPose {
        pos: h.pos,
        facing: h.facing,
        z: h.z,
    };
    w.set_input(
        player,
        PlayerInput {
            pose: Some(pose),
            cast: Some(CastPress {
                seq,
                slot,
                target,
                at,
            }),
            ..Default::default()
        },
    );
}

// Used from the presses' tests (Task 5) on.
#[allow(dead_code)]
pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

pub(super) fn ability(w: &World, key: &str) -> usize {
    w.content().ability(key).unwrap()
}

/// Enemy `i` as a target.
pub(super) fn foe(w: &World, i: usize) -> Option<UnitRef> {
    Some(UnitRef::Enemy(w.enemies[i].id))
}

/// The damage `to` took in `events`, in order.
pub(super) fn damage_to(events: &[Event], to: UnitRef) -> Vec<f32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Damage { to: t, amount, .. } if *t == to => Some(*amount),
            _ => None,
        })
        .collect()
}

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
    assert!(w.hero(0).unwrap().stealthed);
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
fn eating_needs_a_drink_for_mana_and_water_gives_two() {
    let mut w = world("");
    join(&mut w, 0, "mage", 10.0, 30.0);
    let eat = ability(&w, "eat");
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert!((meal.health - 800.0 / 15.0).abs() < 1e-3);
    assert_eq!(meal.mana, 0.0);
    w.land(0, ability(&w, "water"), None, None);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 2);
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert!((meal.mana - 1000.0 / 15.0).abs() < 1e-3);
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.drinks, f.eating), (1, 15.0));
}

#[test]
fn a_blow_of_no_number_does_nothing_and_never_makes_health_nan() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.combat.enemies[0].held = Some(super::Held {
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
