//! The fight in whole worlds on small hand-made maps: what abilities do, presses and casts,
//! enemies held and hurt, heroes falling, and enemies striking back at heroes who dodge.

use std::sync::Arc;

use super::Slowed;
use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Alert, CastPress, Content, ContentSources, ControlKind, Event, Gait, HeroPose, Mark, PlayerId,
    PlayerInput, Refusal, Stop, UnitRef, World, DT, V2, V3,
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
