//! The fight in whole worlds on small hand-made maps: what abilities do, presses and casts,
//! enemies held and hurt, heroes falling, and enemies striking back at heroes who dodge. This file
//! holds the worlds and helpers they share; the tests are by subject in the files beside it.

use std::sync::Arc;

use super::{Hold, Slowed, Telegraph};
use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Alert, CastPress, Content, ContentSources, ControlKind, Event, Gait, HeroPose, Life, Mark,
    PlayerId, PlayerInput, Refusal, Shape, Stop, UnitRef, World, DT, V2, V3,
};

mod effects;
mod falling;
mod holds;
mod presses;
mod strikes;

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
    (key: "shocker", name: "Shocker", display: 203, level: 28, hp: 900.0, walk_speed: 2.5,
     run_speed: 7.0, moves: [(ability: "shockwave", every: 5.0)]),
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
    (key: "shockwave", name: "Shockwave", spell: 45, aim: Me, radius: 6.0, cast: 1.0,
     effects: [Damage(min: 50.0, max: 50.0)]),
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
