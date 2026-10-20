//! Rivals in whole worlds on small hand-made maps: their content, the fight between heroes and
//! rivals, their minds, their bots, their arrivals and whole raids with them.

use std::sync::Arc;

use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Act, ActPress, Alert, CastPress, Content, ContentSources, ControlKind, Event, Gait, HeroPose,
    Holder, KitDef, Life, PlayerId, PlayerInput, RaidRoll, Refusal, RivalOut, RoutePlace, Side,
    UnitRef, World, RIVAL_BASE, TICK_HZ, V2,
};

pub(super) const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0,
     swing: Some((min: 30.0, max: 30.0, every: 2.0)),
     bar: [(1, "stealth"), (2, "strike"), (3, "finish"), (74, "ambush")]),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 800.0,
     speed: 7.0, power_max: 1000.0, bar: [(1, "bolt"), (4, "nova")]),
]"#;
/// 120 yards square: two points of interest east of the entry, three exits.
pub(super) const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (120.0, 0.0), (120.0, 120.0), (0.0, 120.0)],
    entries: [(5.0, 60.0, 0.0)],
    exits: [(key: "east", name: "The East Gate", at: (115.0, 60.0)),
            (key: "north", name: "The North Gate", at: (60.0, 115.0)),
            (key: "cart", name: "The Cart", at: (60.0, 5.0), late: true)])"#;
pub(super) const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 600.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 30.0, max: 30.0, every: 2.0))),
]"#;
pub(super) const ABILITIES: &str = r#"[
    (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0, keeps_stealth: true,
     effects: [Stealth], tags: [Utility]),
    (key: "strike", name: "Strike", spell: 1752, aim: Enemy, range: 5.0, cost: 40.0, gcd: 1.0,
     facing: Front, attack: true, effects: [Weapon(percent: 100.0, bonus: 20.0), Combo(1)],
     tags: [Builder]),
    (key: "finish", name: "Finish", spell: 6762, aim: Enemy, range: 5.0, cost: 35.0, gcd: 1.0,
     facing: Front, needs: (combo: true),
     effects: [Damage(min: 50.0, max: 50.0, per_combo: 60.0)], tags: [Finisher]),
    (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     needs: (stealth: true, unaware: true), effects: [Weapon(percent: 250.0, bonus: 70.0)],
     tags: [Opener]),
    (key: "bolt", name: "Bolt", spell: 116, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
     gcd: 1.5, moving: Slowed, facing: Front, effects: [Damage(min: 100.0, max: 100.0)],
     tags: [Nuke]),
    (key: "nova", name: "Nova", spell: 122, aim: Me, radius: 10.0, cost: 50.0, cooldown: 25.0,
     gcd: 1.5, effects: [Damage(min: 10.0, max: 10.0), Control(kind: Root, secs: 8.0)],
     tags: [Control]),
    (key: "hack", name: "Hack", spell: 285, aim: Enemy, range: 5.0, cost: 30.0, gcd: 1.0,
     facing: Front, attack: true, effects: [Weapon(percent: 100.0, bonus: 30.0)], tags: [Nuke]),
    (key: "dirt", name: "Dirt", spell: 2094, aim: Enemy, range: 8.0, cost: 20.0, cooldown: 20.0,
     gcd: 1.0, sound: Quiet, effects: [Control(kind: Incapacitate, secs: 4.0)], tags: [Control]),
    (key: "bandage", name: "Bandage", spell: 3276, aim: Me, channel: 3.0, ticks: 3,
     cooldown: 30.0, moving: Rooted, sound: Quiet, effects: [Heal(min: 60.0, max: 60.0)],
     tags: [Heal]),
    (key: "dig", name: "Dig", spell: 6343, aim: Me, radius: 6.0, cost: 30.0, cooldown: 20.0,
     gcd: 1.0, moving: Rooted, sound: Loud,
     telegraph: Some((shape: Circle(radius: 6.0), windup: 1.2, on_self: true)),
     effects: [Damage(min: 150.0, max: 150.0)], tags: [Nuke]),
    (key: "shot", name: "Shot", spell: 3018, aim: Enemy, range: 30.0, cost: 20.0, cast: 1.0,
     gcd: 1.0, moving: Slowed, facing: Front, effects: [Damage(min: 60.0, max: 60.0)],
     tags: [Nuke]),
    (key: "net", name: "Net", spell: 6533, aim: Enemy, range: 15.0, cost: 20.0, cooldown: 25.0,
     gcd: 1.0, effects: [Control(kind: Root, secs: 5.0)], tags: [Control]),
    (key: "bleed", name: "Bleed", spell: 8632, aim: Enemy, range: 5.0, cost: 20.0, gcd: 1.0,
     sound: Silent, effects: [Dot(amount: 10.0, every: 1.0, secs: 5.0)], tags: [Nuke]),
    (key: "slow", name: "Slow", spell: 6136, aim: Enemy, range: 20.0, cost: 20.0, gcd: 1.0,
     effects: [Slow(by: 0.5, secs: 6.0)], tags: [Control]),
    (key: "dash", name: "Dash", spell: 2983, aim: Me, cooldown: 60.0, keeps_stealth: true,
     effects: [Sprint(by: 0.7, secs: 8.0)], tags: [Escape]),
]"#;
pub(super) const ITEMS: &str = r#"[
    (key: "shovel", name: "Shovel", kind: Weapon, icon: "INV_Pick_02", weight: 3.0,
     slot: Some(MainHand)),
    (key: "tunic", name: "Tunic", kind: Armour, icon: "INV_Chest_Cloth_17", weight: 2.0,
     slot: Some(Chest)),
    (key: "sack", name: "Sack", kind: Bag, icon: "INV_Misc_Bag_09", weight: 0.5, bag_slots: 4),
    (key: "gem", name: "Gem", kind: Valuable, icon: "INV_Misc_Gem_Ruby_01", weight: 1.0,
     stack: 10, value: 40),
    (key: "conjured_water", name: "Water", kind: Drink, icon: "a", weight: 0.1, stack: 20),
]"#;
/// A crate at each point of interest; a rival's carried find is one gem.
pub(super) const LOOT: &str = r#"(
    kinds: [(key: "crate", name: "Crate", display: 1387, search: 1.0, sound: Open, slots: 6)],
    tables: [(key: "one", rolls: (1, 1), items: [(item: "gem", weight: 1.0)]),
             (key: "found", rolls: (1, 1), items: [(item: "gem", weight: 1.0)])],
    containers: [(kind: "crate", table: "one", at: (60.0, 60.0)),
                 (kind: "crate", table: "one", at: (90.0, 30.0))],
    bodies: [],
)"#;
/// Three kinds (a melee digger, a shooter, and a blade with the whole rogue kit) in three
/// parties; two points of interest; one entry for rivals in the west. No routes: a party
/// searches straight to its goal on a map this small.
pub(super) const RIVALS: &str = r#"(
    kinds: [
        (key: "digger", name: "Digger", names: ["Silas", "Mott"], base: "rogue", class_id: Some(1),
         race: 1, sex: 0, hp: Some(900.0), kit: Abilities(["hack", "dirt", "bandage"]),
         big: Some("dig"), gear: [("shovel", 1), ("tunic", 1), ("sack", 1)], carry: Some("found")),
        (key: "shooter", name: "Shooter", base: "rogue", class_id: Some(1), race: 1, sex: 1,
         kit: Abilities(["shot", "net", "bandage"]), stand_off: 20.0, gear: [("tunic", 1)]),
        (key: "blade", name: "Blade", base: "rogue", race: 1, sex: 0, kit: Class("rogue"),
         sneaks: true),
    ],
    parties: [
        (key: "diggers", name: "Diggers", members: ["digger", "digger"], aggression: (0.1, 0.2),
         odds: 1.0),
        (key: "watch", name: "The Watch", members: ["shooter", "digger"], aggression: (0.8, 0.9),
         odds: 1.0),
        (key: "blades", name: "Blades", members: ["blade", "blade", "blade"],
         aggression: (0.5, 0.5), odds: 0.0),
    ],
    pois: [(key: "yard", name: "The Yard", at: (60.0, 60.0), radius: 6.0, worth: 2.0),
           (key: "barn", name: "The Barn", at: (90.0, 30.0), radius: 6.0)],
    entries: [(10.0, 60.0)],
)"#;
/// [`RIVALS`] with a route from its entry to each point of interest and from each of those to
/// each exit, none of a leg over the default 80 yards.
pub(super) fn with_routes() -> String {
    RIVALS.replace(
        "entries: [(10.0, 60.0)],",
        r#"entries: [(10.0, 60.0)],
    routes: [
        (from: Entry(0), to: Poi("yard"), via: []),
        (from: Entry(0), to: Poi("barn"), via: [(50.0, 45.0)]),
        (from: Poi("yard"), to: Exit("east"), via: []),
        (from: Poi("yard"), to: Exit("north"), via: []),
        (from: Exit("cart"), to: Poi("yard"), via: []),
        (from: Poi("barn"), to: Exit("east"), via: []),
        (from: Poi("barn"), to: Exit("north"), via: [(75.0, 70.0)]),
        (from: Poi("barn"), to: Exit("cart"), via: []),
    ],"#,
    )
}

/// The tests' rules with `rivals` inside `rivals: ( … )` (the defaults when empty).
pub(super) fn rules(rivals: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 200.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0),
        rivals: ({rivals}))"#
    )
}

/// The tests' content with these rivals' rules and `rivals.ron`; `spawns` are RON list items,
/// each ending in a comma.
pub(super) fn content_with(rivals_rules: &str, rivals: &str, spawns: &str) -> Content {
    Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(rivals_rules),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// [`content_with`] the tests' `rivals.ron`.
pub(super) fn content(rivals_rules: &str, spawns: &str) -> Content {
    content_with(rivals_rules, RIVALS, spawns)
}

/// Whether these rules and this `rivals.ron` are refused.
fn refused(rivals_rules: &str, rivals: &str) -> bool {
    Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(rivals_rules),
        enemies: ENEMIES,
        spawns: "[]",
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals,
    })
    .is_err()
}

#[test]
fn every_kind_becomes_a_rival_class_after_the_heroes() {
    let c = content("", "");
    assert_eq!(c.classes.len(), 5);
    assert_eq!(c.playable().map(|(i, _)| i).collect::<Vec<_>>(), vec![0, 1]);
    let digger = &c.rivals.kinds[c.rival_kind("digger").unwrap()];
    let class = &c.classes[digger.class];
    assert!(class.rival);
    assert_eq!(class.key, "rival:digger");
    assert_eq!(class.name, "Digger");
    assert_eq!((class.class_id, class.race, class.sex), (1, 1, 0));
    assert_eq!(class.base_hp, 900.0);
    // The base's numbers stay: energy, its swing.
    assert_eq!(class.power_max, 100.0);
    assert!(class.swing.is_some());
    // The simplified kit from action 1, the big move put after it.
    let keys: Vec<(u8, &str)> = class
        .kit
        .iter()
        .map(|&(s, a)| (s, c.abilities[a].key.as_str()))
        .collect();
    assert_eq!(
        keys,
        vec![(1, "hack"), (2, "dirt"), (3, "bandage"), (4, "dig")]
    );
    assert_eq!(digger.big_ability, c.ability("dig"));
    assert_eq!(
        class.free_items,
        vec![
            (c.item("shovel").unwrap(), 1),
            (c.item("tunic").unwrap(), 1),
            (c.item("sack").unwrap(), 1)
        ]
    );
    assert_eq!(digger.carry_table, Some(1));
    assert_eq!(c.rivals.entries, vec![V2::new(10.0, 60.0)]);
    let watch = &c.rivals.parties[c.party("watch").unwrap()];
    assert_eq!(
        watch.kinds,
        vec![
            c.rival_kind("shooter").unwrap(),
            c.rival_kind("digger").unwrap()
        ]
    );
}

#[test]
fn a_full_class_kit_takes_the_whole_bar() {
    let c = content("", "");
    let blade = &c.rivals.kinds[c.rival_kind("blade").unwrap()];
    assert!(matches!(blade.kit, KitDef::Class(_)));
    assert_eq!(c.classes[blade.class].kit, c.classes[0].kit);
    assert!(c.classes[blade.class].stealth);
    assert_eq!(blade.big_ability, None);
}

#[test]
fn no_rivals_file_is_no_rivals() {
    let c = content_with("", "", "");
    assert!(c.rivals.kinds.is_empty() && c.rivals.parties.is_empty());
    assert!(c.rivals.routes.is_empty());
    assert_eq!(c.classes.len(), 2);
}

#[test]
fn rivals_content_mistakes_are_refused() {
    assert!(refused(
        "",
        &RIVALS.replace(r#"["hack", "dirt""#, r#"["hacks", "dirt""#)
    ));
    assert!(refused(
        "",
        &RIVALS.replace(r#"big: Some("dig")"#, r#"big: Some("hack")"#)
    ));
    assert!(refused(
        "",
        &RIVALS.replace(r#"["digger", "digger"]"#, r#"["digger"]"#)
    ));
    assert!(refused("", &RIVALS.replace("(0.1, 0.2)", "(0.1, 1.5)")));
    assert!(refused(
        "",
        &RIVALS.replace("(90.0, 30.0), radius", "(190.0, 30.0), radius")
    ));
    // A mage base cannot stealth, and the blade sneaks.
    assert!(refused(
        "",
        &RIVALS.replace(
            r#"base: "rogue", race: 1, sex: 0, kit: Class"#,
            r#"base: "mage", race: 1, sex: 0, kit: Class"#
        )
    ));
    assert!(refused(
        "",
        &RIVALS.replace(r#"("shovel", 1)"#, r#"("spade", 1)"#)
    ));
    assert!(refused(
        "",
        &RIVALS.replace("entries: [(10.0, 60.0)]", "entries: []")
    ));
    assert!(refused(r#"leave_by: Some("dawn")"#, RIVALS));
    assert!(refused("engage_at: 1.5", RIVALS));
    // The rules want more points of interest than the file has.
    assert!(refused("pois: (3, 4)", RIVALS));
}

#[test]
fn numbers_that_are_not_finite_are_refused() {
    assert!(refused("arrive: (0.0, inf)", RIVALS));
    assert!(refused("hunt_for: inf", RIVALS));
    assert!(refused("route_leg: inf", RIVALS));
    assert!(refused("stuck_for: NaN", RIVALS));
    assert!(refused(
        "",
        &RIVALS.replace("radius: 6.0, worth: 2.0", "radius: inf, worth: 2.0")
    ));
    assert!(refused(
        "",
        &RIVALS.replace("radius: 6.0, worth: 2.0", "radius: 6.0, worth: inf")
    ));
    assert!(refused(
        "",
        &RIVALS.replace("hp: Some(900.0)", "hp: Some(inf)")
    ));
    assert!(refused(
        "",
        &RIVALS.replace("(10.0, 60.0)]", "(inf, 60.0)]")
    ));
    let heal = ABILITIES.replace("Heal(min: 60.0, max: 60.0)", "Heal(min: 60.0, max: inf)");
    let c = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(""),
        enemies: ENEMIES,
        spawns: "[]",
        abilities: &heal,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals: "",
    });
    assert!(c.is_err(), "a heal of infinity is refused");
}

#[test]
fn a_rival_dodges_no_earlier_than_its_immunity_lasts() {
    // The default immunity is 0.3 s: the dodge happens no earlier than that before the blow.
    let c = content("", "");
    assert!(c.rules.rivals.dodge_at <= c.rules.combat.dodge.immune);
    assert!(
        c.rules.rivals.leave_by.is_none(),
        "a fixture with its own stages needs no \"hunt\""
    );
    assert!(!refused("dodge_at: 0.3", RIVALS));
    assert!(refused("dodge_at: 0.6", RIVALS));
}

#[test]
fn routes_run_either_way_from_their_places() {
    let c = content_with("", &with_routes(), "");
    assert_eq!(c.rivals.routes.len(), 8);
    let (yard, barn) = (RoutePlace::Poi(0), RoutePlace::Poi(1));
    let there = c.rivals.route(RoutePlace::Entry(0), barn).unwrap();
    assert_eq!(
        there,
        vec![
            V2::new(10.0, 60.0),
            V2::new(50.0, 45.0),
            V2::new(90.0, 30.0)
        ]
    );
    let back = c.rivals.route(barn, RoutePlace::Entry(0)).unwrap();
    assert_eq!(back, there.iter().rev().copied().collect::<Vec<_>>());
    // Listed from the exit, kept and walked from the yard to it.
    let cart = c.map.exits.iter().position(|e| e.key == "cart").unwrap();
    assert_eq!(
        c.rivals.route(yard, RoutePlace::Exit(cart)).unwrap(),
        vec![V2::new(60.0, 60.0), V2::new(60.0, 5.0)]
    );
    assert!(c
        .rivals
        .route(RoutePlace::Entry(0), RoutePlace::Exit(cart))
        .is_none());
    // Without routes in the file there are none.
    assert!(content("", "").rivals.routes.is_empty());
}

#[test]
fn route_mistakes_are_refused() {
    let routes = with_routes();
    assert!(!refused("", &routes));
    // A route left out: the barn has none to the cart.
    assert!(refused(
        "",
        &routes.replace(
            r#"        (from: Poi("barn"), to: Exit("cart"), via: []),
"#,
            ""
        )
    ));
    // A leg longer than route_leg: the barn is 85 yards from the entry without its waypoint.
    assert!(refused(
        "",
        &routes.replace("via: [(50.0, 45.0)]", "via: []")
    ));
    assert!(!refused(
        "route_leg: 90.0",
        &routes.replace("via: [(50.0, 45.0)]", "via: []")
    ));
    // A waypoint outside the map, one not a number, a place that is not there.
    assert!(refused(
        "",
        &routes.replace("(50.0, 45.0)", "(50.0, 145.0)")
    ));
    assert!(refused("", &routes.replace("(50.0, 45.0)", "(inf, 45.0)")));
    assert!(refused(
        "",
        &routes.replace(r#"Poi("barn"), via: [(50"#, r#"Poi("silo"), via: [(50"#)
    ));
    assert!(refused(
        "",
        &routes.replace(
            r#"Exit("cart"), to: Poi("yard")"#,
            r#"Exit("moon"), to: Poi("yard")"#
        )
    ));
    assert!(refused(
        "",
        &routes.replace("Entry(0), to: Poi(\"yard\")", "Entry(1), to: Poi(\"yard\")")
    ));
    // A route that joins the wrong kinds, and one listed twice (here the other way round).
    assert!(refused(
        "",
        &routes.replace(
            r#"(from: Poi("yard"), to: Exit("east"), via: [])"#,
            r#"(from: Entry(0), to: Exit("east"), via: [(50.0, 60.0), (100.0, 60.0)])"#
        )
    ));
    assert!(refused(
        "",
        &routes.replace(
            r#"(from: Poi("yard"), to: Exit("north"), via: [])"#,
            r#"(from: Exit("east"), to: Poi("yard"), via: [])"#
        )
    ));
}

#[test]
fn the_kit_leaves_stealth_out_and_marks_the_big_move() {
    use super::kit::kit_of;
    use crate::Tag;

    let c = content("", "");
    let digger = &c.rivals.kinds[c.rival_kind("digger").unwrap()];
    let kit = kit_of(digger.class, digger.big_ability, &c);
    assert_eq!(
        kit.iter().map(|k| k.slot).collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert!(kit[3].big && kit[3].tags.contains(&Tag::Nuke));
    assert_eq!(
        kit[3].reach, 6.0,
        "an on-self telegraph reaches its own size"
    );
    assert!(
        kit[0].sight && kit[0].reach == 5.0,
        "the fight wants a clear line at every range"
    );
    let blade = &c.rivals.kinds[c.rival_kind("blade").unwrap()];
    let kit = kit_of(blade.class, blade.big_ability, &c);
    let keys: Vec<&str> = kit
        .iter()
        .map(|k| c.abilities[k.ability].key.as_str())
        .collect();
    assert_eq!(
        keys,
        vec!["strike", "finish", "ambush"],
        "Stealth is pressed by count"
    );
    assert!(kit[2].opener);
}

/// Flat ground 120 yards across.
pub(super) fn open() -> HeightMap {
    flat(120.0).finish()
}

/// A world on `map` with the tests' content, its raid begun from `roll`, then its map attached.
pub(super) fn world_on(
    map: HeightMap,
    rivals_rules: &str,
    spawns: &str,
    roll: impl FnOnce(&Content) -> RaidRoll,
) -> World {
    let c = content(rivals_rules, spawns);
    let r = roll(&c);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(r).unwrap();
    w.attach_map(Arc::new(map));
    w
}

/// [`world_on`] open ground, the standard roll (no rivals come by themselves).
pub(super) fn world(rivals_rules: &str, spawns: &str) -> World {
    world_on(open(), rivals_rules, spawns, |c| RaidRoll::standard(c, 1))
}

/// `player`'s next input: their hero at `(x, y)` facing `facing`, moving at `gait`.
pub(super) fn put(w: &mut World, player: PlayerId, x: f32, y: f32, facing: V2, gait: Gait) {
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing,
                z: 0.0,
            }),
            gait,
            ..Default::default()
        },
    );
}

/// `player`'s rogue standing at `(x, y)`, facing +x, its first input taken.
pub(super) fn join(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
    put(w, player, x, y, V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

/// A rival of `kind` in party `party` at `(x, y)`, numbered `RIVAL_BASE + n`, its first input
/// (a baseline) taken. No bot drives it: it is in no party's list, so a test moves it.
pub(super) fn rival(w: &mut World, n: u8, kind: &str, party: u8, x: f32, y: f32) -> PlayerId {
    let player = RIVAL_BASE + n;
    let k = w.content().rival_kind(kind).unwrap();
    w.add_rival_hero(player, k, Side::Rival(party), V2::new(x, y), kind)
        .unwrap();
    w.step();
    player
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

/// Steps in `s` seconds.
pub(super) fn secs(s: f32) -> usize {
    (s * TICK_HZ as f32).round() as usize
}

/// The events since the last drain that `pick` keeps.
pub(super) fn events_of<T>(w: &mut World, pick: impl Fn(&Event) -> Option<T>) -> Vec<T> {
    w.drain_events().iter().filter_map(pick).collect()
}

/// A sentry of the tests' brute at `(x, y)` facing south (-y), as a spawn line.
pub(super) fn sentry_line(x: f32, y: f32) -> String {
    format!(
        r#"(place: "Post", kind: "brute", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: 270.0)),"#
    )
}

/// Every stack in the world's containers and in the packs of `players`, by item and count: what
/// the raid holds, to show nothing is copied or lost.
fn everything_held(w: &World, players: &[PlayerId]) -> Vec<(usize, u16)> {
    let mut all: Vec<(usize, u16)> = Vec::new();
    for c in &w.raid.containers {
        all.extend(
            c.stacks
                .iter()
                .flatten()
                .map(|s| (usize::from(s.item), s.count)),
        );
    }
    for &p in players {
        if let Some(inv) = w.pack(p) {
            all.extend(
                inv.equip
                    .iter()
                    .chain(inv.bags.iter())
                    .chain(inv.pack.iter())
                    .chain(inv.safe.iter())
                    .flatten()
                    .map(|s| (usize::from(s.item), s.count)),
            );
        }
    }
    all.sort_unstable();
    all
}

#[test]
fn a_rival_is_a_hero_of_its_own_side_in_its_own_gear() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    join(&mut w, 1, 22.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    let b = rival(&mut w, 1, "shooter", 1, 50.0, 60.0);
    let h = w.hero(a).unwrap();
    assert_eq!(h.side, Side::Rival(0));
    assert_eq!(w.content().classes[h.class].key, "rival:digger");
    assert_eq!(h.pos, V2::new(40.0, 60.0));
    assert_eq!(h.hp, 900.0);
    assert!(w.hostile(0, a) && w.hostile(a, 0));
    assert!(!w.hostile(0, 1), "the party are friends");
    assert!(!w.hostile(a, b), "rival parties leave one another be");
    assert!(!w.hostile(0, 200), "no hero, no fight");
    assert_eq!(w.side_of(0), Side::Party);
    assert_eq!(
        w.side_of(200),
        Side::Party,
        "a player with no hero is the party's"
    );
    // It wears its gear, its sack sits in a bag slot.
    let pack = w.pack(a).unwrap();
    let c = w.content().clone();
    assert!(pack
        .equip
        .iter()
        .flatten()
        .any(|s| usize::from(s.item) == c.item("shovel").unwrap()));
    assert_eq!(
        pack.bags[0].map(|s| usize::from(s.item)),
        Some(c.item("sack").unwrap())
    );
    assert!(w.rivals.body(a).is_some());
}

#[test]
fn the_raid_ends_with_the_party_whatever_the_rivals_do() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    // The hero dies; the rival is up and well: the raid is over, with one outcome.
    w.hurt_hero(0, 5000.0);
    let down = w.content().rules.combat.downed_for;
    steps(&mut w, secs(down + 1.0));
    assert!(w.is_over());
    let outcomes = w.take_outcomes();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].player, 0);
    assert!(w.hero(a).is_some_and(|h| h.fight.up()));
}

#[test]
fn a_raid_of_rivals_alone_is_not_begun_and_a_slain_rival_does_not_end_it() {
    let mut w = world("", "");
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    steps(&mut w, secs(2.0));
    assert!(!w.is_over(), "no hero of the party has been in it");
    join(&mut w, 0, 20.0, 60.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    steps(&mut w, secs(2.0));
    assert!(
        !w.is_over(),
        "the party is up: the raid goes on without its rival"
    );
    assert!(w.take_outcomes().is_empty());
}

#[test]
fn abandoning_a_raid_settles_the_party_and_no_rival() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 41.0, 60.0);
    w.wound(b, 5000.0, Some(UnitRef::Hero(0)));
    w.abandon();
    let outcomes = w.take_outcomes();
    assert_eq!(
        outcomes.iter().map(|o| o.player).collect::<Vec<_>>(),
        vec![0]
    );
    assert!(w.hero(a).is_some() && w.hero(b).is_some());
}

#[test]
fn a_rival_out_by_an_exit_leaves_no_outcome() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 115.0, 60.0);
    w.drain_events();
    let extract = w.content().rules.run.extract_for;
    steps(&mut w, secs(extract + 0.5));
    assert!(w.hero(a).is_none(), "out and gone");
    assert!(w.take_outcomes().is_empty());
    assert!(!w.is_over(), "the party is still in");
    let out = events_of(&mut w, |e| match e {
        Event::RivalOut { player, exit } => Some((*player, *exit)),
        _ => None,
    });
    assert_eq!(out, vec![(a, 0)]);
    assert_eq!(w.rivals.out, vec![a], "counted out");
    assert!(!w.is_parted(a), "no client watches from a rival");
}

#[test]
fn a_rival_out_by_an_exit_takes_what_it_carried_and_drops_nothing() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 115.0, 60.0);
    let carried = everything_held(&w, &[a]);
    assert!(!carried.is_empty());
    let before = w.raid.containers.len();
    let extract = w.content().rules.run.extract_for;
    steps(&mut w, secs(extract + 0.5));
    assert!(w.hero(a).is_none());
    assert!(w.pack(a).is_none(), "its pack went with it, not to a stash");
    assert_eq!(
        w.raid.containers.len(),
        before,
        "no pile at the exit, no body: nothing was dropped for a friend to take"
    );
    assert!(!w.raid.containers.iter().any(|c| c.holder == Holder::Pile));
}

#[test]
fn rivals_never_revive_heroes_nor_lend_them_their_eyes() {
    // A sentry at (60, 100): 37 yards from (60, 63), inside the 40-yard fog; 45 from (60, 55),
    // outside it. The two spots are 8 yards apart, inside the 10 yards sight is shared over.
    let mut w = world("", &format!("{}\n", sentry_line(60.0, 100.0)));
    join(&mut w, 0, 60.0, 55.0);
    join(&mut w, 1, 60.0, 63.0);
    assert!(!w.seen_by(0).is_empty(), "a teammate's eyes count");
    put(&mut w, 1, 20.0, 20.0, V2::new(1.0, 0.0), Gait::Still);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 63.0);
    put(&mut w, 0, 60.0, 55.0, V2::new(0.0, 1.0), Gait::Still);
    w.step();
    assert!(
        w.seen_by(0).is_empty(),
        "a rival's eyes are not a teammate's"
    );
    // Downed with an up rival beside them, the hero is never revived.
    put(&mut w, a, 61.0, 55.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    w.hurt_hero(0, 5000.0);
    let revive = w.content().rules.combat.revive_for;
    steps(&mut w, secs(revive + 1.0));
    assert!(matches!(w.hero(0).unwrap().fight.life, Life::Downed { .. }));
    assert!(events_of(&mut w, |e| matches!(e, Event::Revived { .. }).then_some(())).is_empty());
}

#[test]
fn a_slain_rival_drops_everything_and_counts_as_a_kill() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let before = everything_held(&w, &[0, a]);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    assert_eq!(
        w.hero(a).unwrap().fight.life,
        Life::Dead,
        "rivals die at once"
    );
    w.step();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(a))
        .expect("its body holds what it had");
    let held: Vec<usize> = body
        .stacks
        .iter()
        .flatten()
        .map(|s| usize::from(s.item))
        .collect();
    let c = w.content().clone();
    for item in ["shovel", "tunic", "sack", "gem"] {
        assert!(held.contains(&c.item(item).unwrap()), "{item} dropped");
    }
    assert_eq!(w.raid.kills_of(0), 1);
    assert!(
        w.raid.slain_of(0).is_empty(),
        "a rival is no kind of enemy slain"
    );
    let died = events_of(&mut w, |e| match e {
        Event::RivalDied { player, by } => Some((*player, *by)),
        _ => None,
    });
    assert_eq!(died, vec![(a, Some(0))]);
    // Nothing is copied or lost: the body holds exactly what the rival had.
    assert_eq!(everything_held(&w, &[0, a]), before);
}

#[test]
fn a_slain_rival_drops_everything_even_where_a_hero_would_keep_what_it_brought() {
    let rules = rules("").replace("combat:", "run: (death_loss: found_only), combat:");
    let c = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules,
        enemies: ENEMIES,
        spawns: "[]",
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals: RIVALS,
    })
    .unwrap();
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    w.step();
    let shovel = w.content().item("shovel").unwrap();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(a))
        .expect("a body");
    assert!(body
        .stacks
        .iter()
        .flatten()
        .any(|s| usize::from(s.item) == shovel));
}

#[test]
fn a_rival_dying_is_told_once_and_its_killer_is_the_last_hero_to_strike_it() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    join(&mut w, 1, 22.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let b = rival(&mut w, 1, "digger", 1, 31.0, 60.0);
    // A hero struck it, then the world finished it: the hero slew it.
    w.wound(a, 10.0, Some(UnitRef::Hero(1)));
    w.wound(a, 10.0, Some(UnitRef::Hero(0)));
    w.hurt_hero(a, 5000.0);
    // A rival of another party striking it is no slayer; nor is nobody.
    w.wound(b, 10.0, Some(UnitRef::Hero(a)));
    w.hurt_hero(b, 5000.0);
    steps(&mut w, 3);
    let mut died = events_of(&mut w, |e| match e {
        Event::RivalDied { player, by } => Some((*player, *by)),
        _ => None,
    });
    died.sort();
    assert_eq!(died, vec![(a, Some(0)), (b, None)], "one each, one killer");
    assert_eq!(w.raid.kills_of(0), 1);
    assert_eq!(w.raid.kills_of(1), 0);
}

#[test]
fn a_kill_of_a_rival_and_of_an_enemy_each_count_once() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    w.events.push(Event::EnemyDied {
        id: 777,
        by: Some(0),
    });
    w.events.push(Event::RivalDied {
        player: RIVAL_BASE,
        by: Some(0),
    });
    w.events.push(Event::RivalDied {
        player: RIVAL_BASE + 1,
        by: None,
    });
    w.tally_kills();
    w.tally_kills();
    assert_eq!(w.raid.kills_of(0), 2);
}

#[test]
fn with_downed_set_a_rival_goes_down_and_its_friend_revives_it() {
    let mut w = world("downed: true", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 31.0, 60.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    assert!(matches!(w.hero(a).unwrap().fight.life, Life::Downed { .. }));
    let revive = w.content().rules.combat.revive_for;
    steps(&mut w, secs(revive + 0.5));
    assert!(w.hero(a).unwrap().fight.up(), "its own party revives it");
    assert!(w.hero(b).unwrap().fight.up());
}

#[test]
fn a_downed_rival_is_not_revived_by_a_hero() {
    let mut w = world("downed: true", "");
    join(&mut w, 0, 30.0, 61.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    // The hero stands beside it for longer than a revive takes.
    let revive = w.content().rules.combat.revive_for;
    steps(&mut w, secs(revive + 1.0));
    assert!(
        matches!(
            w.hero(a).unwrap().fight.life,
            Life::Downed { .. } | Life::Dead
        ),
        "the other side never gets it up"
    );
    assert!(events_of(&mut w, |e| matches!(e, Event::Revived { .. }).then_some(())).is_empty());
}

#[test]
fn a_rival_slips_out_unseen_only_once_no_hero_sees_it() {
    let mut w = world("", "");
    join(&mut w, 0, 60.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 70.0, 60.0);
    let carried = everything_held(&w, &[a]);
    assert!(!carried.is_empty());
    let before = w.raid.containers.len();
    assert!(!w.rival_leaves_unseen(a), "the hero sees it 10 yards off");
    assert!(w.hero(a).is_some() && w.rivals.out.is_empty());
    // A friend of its own sees nothing for the party: only the party's eyes count.
    let c = rival(&mut w, 1, "digger", 0, 70.0, 61.0);
    put(&mut w, 0, 5.0, 5.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    assert!(
        w.rival_leaves_unseen(a),
        "the hero is far off: nobody sees it"
    );
    assert!(w.hero(a).is_none() && w.pack(a).is_none());
    assert_eq!(w.rivals.out, vec![a], "counted out");
    assert_eq!(w.raid.containers.len(), before, "no pile, no body");
    assert!(w.take_outcomes().is_empty());
    assert!(!w.is_parted(a));
    assert!(
        events_of(&mut w, |e| matches!(e, Event::RivalOut { .. })
            .then_some(()))
        .is_empty(),
        "nobody saw it go"
    );
    // It is not a hero, a dead rival, nor a rival gone already.
    assert!(
        !w.rival_leaves_unseen(0),
        "a hero of the party never slips away"
    );
    assert!(w.hero(0).is_some());
    assert!(!w.rival_leaves_unseen(a));
    w.hurt_hero(c, 5000.0);
    assert!(!w.rival_leaves_unseen(c), "the dead stay as bodies");
    assert_eq!(w.rivals.out, vec![a]);
}

#[test]
fn a_downed_hero_does_not_see_a_rival_leave() {
    let mut w = world("", "");
    join(&mut w, 0, 60.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 66.0, 60.0);
    // A hero downed does not watch.
    w.hurt_hero(0, 5000.0);
    assert!(w.rival_leaves_unseen(a));
    assert!(w.hero(a).is_none());
}

#[test]
fn rivals_roll_their_carried_find_with_dice_of_their_own() {
    use super::carry_dice;
    // Twin seeds and twin players all roll apart; a rival never rolls what a chest did.
    let first = |seed: u64, player: PlayerId| carry_dice(seed, player).unit();
    let mut same_seed_twins = 0;
    let mut same_player_twins = 0;
    for s in 0..100u64 {
        let s = s * 2 + 1000;
        if first(s, RIVAL_BASE) == first(s ^ 1, RIVAL_BASE) {
            same_seed_twins += 1;
        }
        if first(s, RIVAL_BASE) == first(s, RIVAL_BASE + 1) {
            same_player_twins += 1;
        }
    }
    assert_eq!(same_seed_twins, 0, "twin seeds roll alike");
    assert_eq!(same_player_twins, 0, "twin rivals roll alike");
}

#[test]
fn a_new_raid_starts_without_the_last_raids_rivals() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 115.0, 60.0);
    w.rivals.next = 3;
    w.rivals.out.push(a);
    let again = RaidRoll::standard(&w.content().clone(), 2);
    w.begin_raid(again).unwrap();
    assert!(w.rivals.bodies.is_empty() && w.rivals.out.is_empty());
    assert_eq!(w.rivals.next, 0);
}

/// `player` presses action `slot` at `target`, standing still; numbered `seq` (a new press
/// needs a new number, as from a client).
pub(super) fn press(w: &mut World, player: PlayerId, seq: u8, slot: u8, target: Option<UnitRef>) {
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
                at: None,
            }),
            ..Default::default()
        },
    );
}

/// The damage `to` took in these events.
pub(super) fn damage_to(events: &[Event], to: UnitRef) -> Vec<f32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Damage { to: t, amount, .. } if *t == to => Some(*amount),
            _ => None,
        })
        .collect()
}

/// A hero at (20, 60) facing +x and a digger 3 yards in front of it facing the hero.
fn duel() -> (World, PlayerId) {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 23.0, 60.0);
    put(&mut w, a, 23.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    w.drain_events();
    (w, a)
}

#[test]
fn a_hero_strikes_a_rival_and_the_rival_strikes_back() {
    let (mut w, a) = duel();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let events = w.drain_events();
    // The press and the auto-attack it starts, in the same step (as against an enemy).
    let hit = damage_to(&events, UnitRef::Hero(a));
    assert_eq!(hit.len(), 2, "{events:?}");
    assert!(w.hero(a).unwrap().hp < 900.0);
    assert_eq!(
        w.hero(0).unwrap().fight.combo_hero,
        Some(a),
        "a combo point on the rival"
    );
    press(&mut w, a, 1, 1, Some(UnitRef::Hero(0)));
    w.step();
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(0)).len(), 2, "{events:?}");
    assert!(w.hero(0).unwrap().hp < 1000.0);
}

#[test]
fn friends_and_rivals_of_another_party_cannot_be_targeted() {
    let (mut w, a) = duel();
    join(&mut w, 1, 21.0, 61.0);
    let b = rival(&mut w, 1, "digger", 1, 24.0, 61.0);
    w.drain_events();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(1)));
    press(&mut w, a, 1, 1, Some(UnitRef::Hero(b)));
    w.step();
    let refused = events_of(&mut w, |e| match e {
        Event::Refused { player, why } => Some((*player, *why)),
        _ => None,
    });
    assert!(refused.contains(&(0, Refusal::BadTarget)));
    assert!(refused.contains(&(a, Refusal::BadTarget)));
}

#[test]
fn a_hold_roots_the_hero_and_refuses_presses_until_a_blow_frees_it() {
    let (mut w, a) = duel();
    press(&mut w, a, 1, 2, Some(UnitRef::Hero(0)));
    w.step();
    let h = w.hero(0).unwrap();
    assert_eq!(h.fight.held, Some(ControlKind::Incapacitate));
    assert!(h.fight.rooted, "held feet");
    w.drain_events();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let refused = events_of(&mut w, |e| match e {
        Event::Refused { player: 0, why } => Some(*why),
        _ => None,
    });
    assert_eq!(refused, vec![Refusal::Held]);
    steps(&mut w, secs(1.0));
    press(&mut w, a, 2, 1, Some(UnitRef::Hero(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.held, None);
    assert!(!w.hero(0).unwrap().fight.rooted);
}

#[test]
fn a_hold_runs_out_by_itself() {
    let (mut w, a) = duel();
    press(&mut w, a, 1, 2, Some(UnitRef::Hero(0)));
    w.step();
    assert!(w.hero(0).unwrap().fight.held.is_some());
    steps(&mut w, secs(4.2));
    assert_eq!(w.hero(0).unwrap().fight.held, None);
}

#[test]
fn an_area_ability_reaches_the_hostile_heroes_in_it_and_spares_friends() {
    let mut w = world("", "");
    w.add_hero(0, "mage", "Mage", 1, 1).unwrap();
    put(&mut w, 0, 20.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    join(&mut w, 1, 22.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 25.0, 60.0);
    let far = rival(&mut w, 1, "digger", 0, 40.0, 60.0);
    w.drain_events();
    press(&mut w, 0, 1, 4, None);
    w.step();
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(a)).len(), 1);
    assert!(
        damage_to(&events, UnitRef::Hero(1)).is_empty(),
        "a friend is spared"
    );
    assert!(
        damage_to(&events, UnitRef::Hero(far)).is_empty(),
        "out of reach"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::CastGo { hits, .. } if hits.contains(&UnitRef::Hero(a)))));
    assert_eq!(w.hero(a).unwrap().fight.held, Some(ControlKind::Root));
}

#[test]
fn a_bandage_heals_its_rival_tick_by_tick() {
    let (mut w, a) = duel();
    w.wound(a, 400.0, Some(UnitRef::Hero(0)));
    let before = w.hero(a).unwrap().hp;
    press(&mut w, a, 1, 3, None);
    steps(&mut w, secs(3.2));
    let healed: f32 = events_of(&mut w, |e| match e {
        Event::Healed { player, amount } if *player == a => Some(*amount),
        _ => None,
    })
    .iter()
    .sum();
    assert!((healed - 180.0).abs() < 1e-3, "three ticks of 60: {healed}");
    assert!((w.hero(a).unwrap().hp - (before + 180.0)).abs() < 1e-3);
}

#[test]
fn a_slow_and_a_bleed_on_a_hero_run_their_course() {
    let (mut w, a) = duel();
    let speed = w.hero(0).unwrap().speed;
    let ci = w.combat.hero_index(0);
    w.combat.heroes[ci].slow = Some(crate::combat::Slowed {
        by: 0.5,
        left: 1.0,
        spell: 1,
    });
    w.combat.heroes[ci].burns.push(crate::combat::Burn {
        by: a,
        amount: 10.0,
        every: 1.0,
        next: 1.0,
        left: 3.0,
        spell: 2,
        silent: true,
    });
    w.step();
    assert!(w.hero(0).unwrap().speed < speed * 0.6, "slowed by half");
    steps(&mut w, secs(3.2));
    let ticks = damage_to(&w.drain_events(), UnitRef::Hero(0));
    assert_eq!(ticks, vec![10.0, 10.0, 10.0]);
    assert!(
        (w.hero(0).unwrap().speed - speed).abs() < 1e-3,
        "the slow is gone"
    );
}

#[test]
fn an_area_ability_does_not_reach_a_rival_behind_a_wall() {
    let mut b = flat(120.0);
    crate::heightmap::testmap::house(&mut b, V2::new(26.0, 50.0), V2::new(36.0, 70.0), None, true);
    let mut w = world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1));
    w.add_hero(0, "mage", "Mage", 1, 1).unwrap();
    put(&mut w, 0, 20.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let clear = rival(&mut w, 1, "digger", 0, 20.0, 66.0);
    w.drain_events();
    press(&mut w, 0, 1, 4, None);
    w.step();
    let events = w.drain_events();
    assert!(
        damage_to(&events, UnitRef::Hero(a)).is_empty(),
        "{events:?}"
    );
    assert_eq!(w.hero(a).unwrap().fight.held, None);
    assert_eq!(
        damage_to(&events, UnitRef::Hero(clear)).len(),
        1,
        "in the open it lands"
    );
}

#[test]
fn a_press_at_a_rival_behind_a_wall_is_refused_for_sight() {
    let mut b = flat(120.0);
    crate::heightmap::testmap::house(&mut b, V2::new(26.0, 50.0), V2::new(36.0, 70.0), None, true);
    let mut w = world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1));
    w.add_hero(0, "mage", "Mage", 1, 1).unwrap();
    put(&mut w, 0, 20.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.drain_events();
    press(&mut w, 0, 1, 1, Some(UnitRef::Hero(a)));
    w.step();
    let refused = events_of(&mut w, |e| match e {
        Event::Refused { player: 0, why } => Some(*why),
        _ => None,
    });
    assert_eq!(refused, vec![Refusal::NotInSight]);
}

/// The damage of a hack from a digger at the hero, with the shovel's bonus as given.
fn hack_with_shovel(bonus: &str) -> f32 {
    let items = ITEMS.replace(
        r#"slot: Some(MainHand))"#,
        &format!("slot: Some(MainHand), bonus: (damage: {bonus}))"),
    );
    let c = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(""),
        enemies: ENEMIES,
        spawns: "[]",
        abilities: ABILITIES,
        items: &items,
        loot: LOOT,
        monsters: "[]",
        rivals: RIVALS,
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 23.0, 60.0);
    put(&mut w, a, 23.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    w.drain_events();
    press(&mut w, a, 1, 1, Some(UnitRef::Hero(0)));
    w.step();
    damage_to(&w.drain_events(), UnitRef::Hero(0))[0]
}

#[test]
fn a_rivals_worn_gear_counts_in_its_blows_as_a_heros_does() {
    let plain = hack_with_shovel("0.0");
    let sharp = hack_with_shovel("1.0");
    assert!(plain > 0.0);
    assert!((sharp - 2.0 * plain).abs() < 1e-3, "{plain} -> {sharp}");
}

#[test]
fn a_hero_swings_at_one_target_at_a_time() {
    let (mut w, a) = duel();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let ci = w.combat.hero_index(0);
    assert_eq!(w.combat.heroes[ci].swing_at, Some(a));
    assert_eq!(w.hero(0).unwrap().fight.attacking, None);
}

#[test]
fn the_first_blow_at_an_unaware_rival_is_an_ambush_and_says_so() {
    let (mut w, a) = duel();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let ambush: Vec<bool> = events_of(&mut w, |e| match e {
        Event::Damage { to, ambush, .. } if *to == UnitRef::Hero(a) => Some(*ambush),
        _ => None,
    });
    assert_eq!(ambush.first(), Some(&true), "{ambush:?}");
}

#[test]
fn a_rival_stays_while_any_one_hero_of_the_party_sees_it() {
    let mut w = world("", "");
    join(&mut w, 0, 60.0, 60.0);
    join(&mut w, 1, 5.0, 5.0);
    // Far from one hero, near the other.
    let a = rival(&mut w, 0, "digger", 0, 66.0, 60.0);
    assert!(!w.rival_leaves_unseen(a), "the near hero sees it");
    assert!(w.hero(a).is_some());
    // Both far: now it may go.
    put(&mut w, 0, 5.0, 8.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    assert!(w.rival_leaves_unseen(a));
}

#[test]
fn a_wall_between_a_hero_and_a_rival_hides_it_from_the_hero() {
    let mut b = flat(120.0);
    crate::heightmap::testmap::house(&mut b, V2::new(60.0, 50.0), V2::new(80.0, 70.0), None, true);
    let mut w = world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1));
    join(&mut w, 0, 50.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 70.0, 60.0);
    assert!(
        w.rival_leaves_unseen(a),
        "20 yards off behind a roofed house: unseen"
    );
    // The same distance in the open is seen.
    let mut w = world("", "");
    join(&mut w, 0, 50.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 70.0, 60.0);
    assert!(!w.rival_leaves_unseen(a));
}

#[test]
fn a_rivals_big_move_shows_first_then_lands_on_its_foes_inside() {
    let mut w = world("", &format!("{}\n", sentry_line(43.0, 60.0)));
    join(&mut w, 0, 37.0, 60.0);
    join(&mut w, 1, 30.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    let friend = rival(&mut w, 1, "digger", 0, 41.0, 61.0);
    w.drain_events();
    let sentry_hp = w.enemies[0].hp;
    // The shovel smash (action 4): a 6-yard circle around the digger, 1.2 s to land.
    press(&mut w, a, 1, 4, None);
    w.step();
    let t = *w
        .telegraphs
        .iter()
        .find(|t| t.by == Some(a))
        .expect("shown on the ground");
    assert!((t.total - 1.2).abs() < 1e-4);
    assert_eq!(t.enemy, 0);
    assert!(
        w.drain_events()
            .iter()
            .all(|e| !matches!(e, Event::Damage { .. })),
        "nothing lands while it shows"
    );
    steps(&mut w, secs(1.3));
    let events = w.drain_events();
    assert_eq!(
        damage_to(&events, UnitRef::Hero(0)).len(),
        1,
        "inside: {events:?}"
    );
    assert!(
        damage_to(&events, UnitRef::Hero(1)).is_empty(),
        "10 yards off: outside"
    );
    assert!(
        damage_to(&events, UnitRef::Hero(friend)).is_empty(),
        "its friend is spared"
    );
    assert!(
        w.enemies[0].hp < sentry_hp,
        "a monster inside is struck too"
    );
    assert!(w.telegraphs.iter().all(|t| t.by != Some(a)));
}

#[test]
fn a_hero_who_dodges_sees_the_big_move_miss() {
    let mut w = world("", "");
    join(&mut w, 0, 37.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    w.drain_events();
    press(&mut w, a, 1, 4, None);
    w.step();
    steps(&mut w, secs(1.0));
    // The dodge, as a client counts it, just before the blow lands.
    w.set_input(
        0,
        PlayerInput {
            dodge_presses: 1,
            ..Default::default()
        },
    );
    steps(&mut w, secs(0.4));
    let events = w.drain_events();
    assert!(
        damage_to(&events, UnitRef::Hero(0)).is_empty(),
        "{events:?}"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Evaded { player: 0, .. })));
}

#[test]
fn a_big_move_cut_short_takes_its_telegraph_with_it() {
    let mut w = world("", "");
    join(&mut w, 0, 37.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    w.drain_events();
    press(&mut w, a, 1, 4, None);
    w.step();
    assert!(w.telegraphs.iter().any(|t| t.by == Some(a)));
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    assert!(w.telegraphs.iter().all(|t| t.by != Some(a)));
    steps(&mut w, secs(1.5));
    assert!(damage_to(&w.drain_events(), UnitRef::Hero(0)).is_empty());
}

#[test]
fn a_wall_shields_a_hero_from_a_rivals_big_move() {
    let mut b = flat(120.0);
    crate::heightmap::testmap::house(&mut b, V2::new(60.0, 50.0), V2::new(80.0, 70.0), None, true);
    let mut w = world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1));
    // The hero stands inside the house, 4 yards from the digger outside its west wall.
    join(&mut w, 0, 63.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 59.0, 60.0);
    w.drain_events();
    press(&mut w, a, 1, 4, None);
    w.step();
    steps(&mut w, secs(1.4));
    let events = w.drain_events();
    assert!(
        damage_to(&events, UnitRef::Hero(0)).is_empty(),
        "{events:?}"
    );
    // The same two in the open: struck.
    let mut w = world("", "");
    join(&mut w, 0, 63.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 59.0, 60.0);
    w.drain_events();
    press(&mut w, a, 1, 4, None);
    w.step();
    steps(&mut w, secs(1.4));
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Hero(0)).len(), 1);
}

#[test]
fn a_big_move_is_lost_when_its_caster_steps_away_or_dodges() {
    for dodge in [false, true] {
        let (mut w, a) = duel();
        press(&mut w, a, 1, 4, None);
        w.step();
        assert!(w.telegraphs.iter().any(|t| t.by == Some(a)));
        if dodge {
            w.set_input(
                a,
                PlayerInput {
                    dodge_presses: 1,
                    ..Default::default()
                },
            );
        } else {
            // Far past what a rooted cast allows.
            put(&mut w, a, 28.0, 60.0, V2::new(-1.0, 0.0), Gait::Run);
        }
        w.step();
        assert!(
            w.telegraphs.iter().all(|t| t.by != Some(a)),
            "dodge {dodge}: its telegraph goes with the cast"
        );
        w.drain_events();
        steps(&mut w, secs(1.5));
        assert!(damage_to(&w.drain_events(), UnitRef::Hero(0)).is_empty());
    }
}

/// Walk `player` from `from` to `to` over `n` steps at `gait`, facing the way it goes, with
/// `stealth_presses` in every input.
fn walk(w: &mut World, player: PlayerId, from: V2, to: V2, n: usize, gait: Gait, presses: u8) {
    let way = (to - from).normalize_or_zero();
    for i in 1..=n {
        let at = from + (to - from) * (i as f32 / n as f32);
        w.set_input(
            player,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: at,
                    facing: way,
                    z: 0.0,
                }),
                gait,
                stealth_presses: presses,
                ..Default::default()
            },
        );
        w.step();
    }
}

#[test]
fn a_sneaking_rogue_passes_behind_a_rival_and_a_running_one_is_heard() {
    let mut w = world("", "");
    join(&mut w, 0, 54.0, 40.0);
    join(&mut w, 1, 54.0, 39.0);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 60.0);
    put(&mut w, a, 60.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    // Hero 0 steps into stealth, then sneaks past 6 yards behind the rival.
    walk(
        &mut w,
        0,
        V2::new(54.0, 40.0),
        V2::new(54.0, 41.0),
        3,
        Gait::Still,
        1,
    );
    assert!(w.hero(0).unwrap().stealthed);
    walk(
        &mut w,
        0,
        V2::new(54.0, 41.0),
        V2::new(54.0, 80.0),
        secs(16.0),
        Gait::Walk,
        1,
    );
    let r = w.content().rules.senses.awareness.clone();
    assert!(
        w.rival_mind(a).unwrap().level(0) < r.suspicious,
        "unnoticed and unheard"
    );
    // Hero 1 runs the same way: its footsteps carry.
    walk(
        &mut w,
        1,
        V2::new(54.0, 39.0),
        V2::new(54.0, 80.0),
        secs(6.0),
        Gait::Run,
        0,
    );
    assert!(
        w.rival_mind(a).unwrap().level(1) >= r.suspicious,
        "heard running past"
    );
}

/// A roofed house over (60, 50)-(80, 70) on a flat 120-yard map.
fn walled_world() -> World {
    let mut b = flat(120.0);
    house(&mut b, V2::new(60.0, 50.0), V2::new(80.0, 70.0), None, true);
    world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1))
}

/// Hero 0 runs up and down a yard at `x`, `y` for about two seconds.
fn pace(w: &mut World, x: f32, y: f32) {
    for _ in 0..5 {
        walk(w, 0, V2::new(x, y), V2::new(x + 1.0, y), 6, Gait::Run, 0);
        walk(w, 0, V2::new(x + 1.0, y), V2::new(x, y), 6, Gait::Run, 0);
    }
}

#[test]
fn a_rival_hears_a_hero_behind_a_wall_it_cannot_see_through() {
    // M14: rivals listen before the enemies take the step's noises; with them listening last,
    // this hero (out of sight, behind a roofed house's wall) would be heard by nobody.
    let mut w = walled_world();
    join(&mut w, 0, 63.0, 60.0);
    // Outside the west wall, looking away from the house.
    let a = rival(&mut w, 0, "digger", 0, 58.0, 60.0);
    put(&mut w, a, 58.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    assert_eq!(w.rival_mind(a).unwrap().level(0), 0.0);
    pace(&mut w, 63.0, 60.0);
    let m = w.rival_mind(a).unwrap();
    assert!(
        m.level(0) > 0.0 || m.is_alerted(),
        "the rival heard the footsteps through the wall: {:?}",
        m.state
    );
    // The same run far out of hearing leaves it calm.
    let mut w = walled_world();
    join(&mut w, 0, 75.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 58.0, 60.0);
    put(&mut w, a, 58.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    pace(&mut w, 75.0, 60.0);
    assert_eq!(w.rival_mind(a).unwrap().level(0), 0.0);
}

#[test]
fn a_sentry_hears_a_rival_walk_past_and_strikes_it_once_it_is_found() {
    let mut w = world("", &format!("{}\n", sentry_line(60.0, 60.0)));
    join(&mut w, 0, 10.0, 10.0);
    // The sentry faces south; the rival passes 3 yards behind it, inside a walking step's
    // hearing.
    let a = rival(&mut w, 0, "digger", 0, 57.0, 63.0);
    walk(
        &mut w,
        a,
        V2::new(57.0, 63.0),
        V2::new(63.0, 63.0),
        secs(3.0),
        Gait::Walk,
        0,
    );
    assert!(
        !w.marks_for(a).is_empty(),
        "the rival's footsteps were heard"
    );
    // In front of it, the sentry finds the rival and goes for it as for a hero.
    walk(
        &mut w,
        a,
        V2::new(63.0, 63.0),
        V2::new(60.0, 56.0),
        secs(2.0),
        Gait::Walk,
        0,
    );
    steps(&mut w, secs(8.0));
    assert!(w.marks_for(a).iter().any(|m| m.alert == Alert::Alerted));
    let events = w.drain_events();
    assert!(
        !damage_to(&events, UnitRef::Hero(a)).is_empty(),
        "struck: {events:?}"
    );
}

#[test]
fn an_unaware_rival_takes_an_ambush_and_its_party_hears_of_it() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 23.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 30.0, 60.0);
    put(&mut w, a, 23.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    w.drain_events();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let first = w.drain_events();
    let hit = first.iter().find_map(|e| match e {
        Event::Damage {
            to, amount, ambush, ..
        } if *to == UnitRef::Hero(a) => Some((*amount, *ambush)),
        _ => None,
    });
    assert!(hit.is_some_and(|(_, ambush)| ambush), "{first:?}");
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    assert!(
        w.rival_mind(b).unwrap().alerted_to(0),
        "the shout reached its friend"
    );
    // Alerted now: the next blow is no ambush.
    steps(&mut w, secs(1.1));
    press(&mut w, 0, 2, 2, Some(UnitRef::Hero(a)));
    w.step();
    let second = w.drain_events();
    assert!(second
        .iter()
        .any(|e| matches!(e, Event::Damage { to, ambush: false, .. } if *to == UnitRef::Hero(a))));
}

#[test]
fn an_opener_is_refused_on_a_rival_that_has_found_the_hero() {
    let (mut w, a) = duel();
    // Stealthed first (the rival loses sight of the hero), then told of them again.
    walk(
        &mut w,
        0,
        V2::new(20.0, 60.0),
        V2::new(20.0, 60.0),
        3,
        Gait::Still,
        1,
    );
    assert!(w.hero(0).unwrap().stealthed);
    // An unaware rival may be opened on; one alerted to the hero may not.
    assert!(w.foe_unaware(a, 0));
    w.alert_rival(a, 0);
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    assert!(!w.foe_unaware(a, 0));
    w.drain_events();
    w.set_input(
        0,
        PlayerInput {
            stealth_presses: 1,
            cast: Some(CastPress {
                seq: 1,
                slot: 74,
                target: Some(UnitRef::Hero(a)),
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    let refused = events_of(&mut w, |e| match e {
        Event::Refused { player: 0, why } => Some(*why),
        _ => None,
    });
    assert_eq!(refused, vec![Refusal::Aware]);
}

#[test]
fn the_hero_sees_rivals_in_the_open_not_behind_walls_nor_far_in_stealth() {
    let mut b = flat(120.0);
    house(
        &mut b,
        V2::new(30.0, 70.0),
        V2::new(35.0, 90.0),
        None,
        false,
    );
    let mut w = world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1));
    join(&mut w, 0, 20.0, 60.0);
    let open = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    // The line from (20, 60) to (40, 85) crosses the house's west wall.
    let walled = rival(&mut w, 1, "digger", 0, 40.0, 85.0);
    let sneak = rival(&mut w, 2, "blade", 0, 30.0, 50.0);
    walk(
        &mut w,
        sneak,
        V2::new(30.0, 50.0),
        V2::new(30.0, 50.0),
        3,
        Gait::Still,
        1,
    );
    assert!(w.hero(sneak).unwrap().stealthed);
    put(&mut w, 0, 20.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    let seen = w.snapshot_for(0).rivals.seen;
    assert!(seen.contains(&open));
    assert!(!seen.contains(&walled), "behind the house");
    assert!(
        !seen.contains(&sneak),
        "a still blade in stealth, 14 yards off"
    );
    put(&mut w, 0, 29.0, 51.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    assert!(
        w.snapshot_for(0).rivals.seen.contains(&sneak),
        "close enough to make it out"
    );
}

#[test]
fn a_rival_marks_the_hero_it_suspects_then_is_alerted_to() {
    let mut w = world("", "");
    join(&mut w, 0, 40.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 60.0);
    put(&mut w, a, 60.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    // Running toward it, in its front cone (M15: two seconds fill the suspicion past the mark).
    walk(
        &mut w,
        0,
        V2::new(40.0, 60.0),
        V2::new(48.0, 60.0),
        secs(2.0),
        Gait::Run,
        0,
    );
    let marks = w.rival_view(0).marks;
    assert!(marks.iter().any(|m| m.rival == a), "{marks:?}");
    walk(
        &mut w,
        0,
        V2::new(48.0, 60.0),
        V2::new(50.0, 60.0),
        secs(4.0),
        Gait::Run,
        0,
    );
    assert!(w.rival_view(0).marks.contains(&crate::RivalMark {
        rival: a,
        alert: Alert::Alerted
    }));
}

#[test]
fn a_vanish_makes_rivals_lose_the_rogue() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.alert_rival(a, 0);
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    w.rivals_lose(0);
    assert!(!w.rival_mind(a).unwrap().is_alerted());
}

#[test]
fn a_hero_a_rival_is_alerted_to_is_in_combat_until_it_loses_them() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 70.0, 60.0);
    w.step();
    assert!(!w.hero(0).unwrap().fight.in_combat);
    w.alert_rival(a, 0);
    w.step();
    assert!(w.hero(0).unwrap().fight.in_combat, "hunted by a rival");
    w.rivals_lose(0);
    steps(&mut w, secs(1.0));
    assert!(!w.hero(0).unwrap().fight.in_combat);
}

#[test]
fn a_mirror_takes_the_rival_view_its_host_sent() {
    let mut host = world("", "");
    join(&mut host, 0, 20.0, 60.0);
    let a = rival(&mut host, 0, "digger", 0, 30.0, 60.0);
    host.alert_rival(a, 0);
    host.step();
    let snap = host.snapshot_for(0);
    assert!(snap.rivals.seen.contains(&a));
    let mut mirror = World::new(Arc::new(content("", "")));
    mirror.begin_mirror();
    mirror.apply_snapshot(snap.clone());
    assert_eq!(mirror.rival_view(0), snap.rivals);
}
/// A world on open ground whose rogue has a Vanish on action 5 and the tests' Slow on action 6.
fn rogue_world() -> World {
    let abilities = ABILITIES.replace(
        r#"(key: "ambush""#,
        r#"(key: "vanish", name: "Vanish", spell: 1856, aim: Me, cooldown: 60.0, keeps_stealth: true,
     effects: [Vanish], tags: [Escape]),
    (key: "ambush""#,
    );
    let classes = CLASSES.replace(
        r#"(74, "ambush")"#,
        r#"(74, "ambush"), (5, "vanish"), (6, "slow")"#,
    );
    let c = Content::from_ron(ContentSources {
        classes: &classes,
        map: MAP,
        rules: &rules(""),
        enemies: ENEMIES,
        spawns: "[]",
        abilities: &abilities,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals: RIVALS,
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    w
}

#[test]
fn pressing_vanish_makes_rivals_lose_the_rogue() {
    // The effect itself, not `rivals_lose`: a rogue with a Vanish on its bar presses it.
    let mut w = rogue_world();
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.alert_rival(a, 0);
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    press(&mut w, 0, 1, 5, None);
    w.step();
    assert!(
        !w.rival_mind(a).unwrap().is_alerted(),
        "a Vanish pressed loses it"
    );
}

#[test]
fn a_blow_that_does_no_damage_still_alerts_the_rival_and_its_party() {
    // Slow lands no damage: only the ability's own alert (D13) tells the rival and its friend.
    let mut w = rogue_world();
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 33.0, 60.0);
    put(&mut w, a, 30.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    put(&mut w, b, 33.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    assert!(!w.rival_mind(a).unwrap().alerted_to(0));
    w.drain_events();
    press(&mut w, 0, 1, 6, Some(UnitRef::Hero(a)));
    w.step();
    assert!(damage_to(&w.drain_events(), UnitRef::Hero(a)).is_empty());
    assert!(w.rival_mind(a).unwrap().alerted_to(0), "the slowed rival");
    assert!(w.rival_mind(b).unwrap().alerted_to(0), "and its friend");
}

#[test]
fn any_wound_from_a_hero_alerts_the_rival_it_lands_on() {
    // A bleed's tick or any damage that is no ability's landing: `rival_struck` alerts.
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 70.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 73.0, 60.0);
    assert!(!w.rival_mind(a).unwrap().is_alerted());
    w.wound(a, 10.0, Some(UnitRef::Hero(0)));
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    assert!(w.rival_mind(b).unwrap().alerted_to(0), "its party is told");
}

#[test]
fn a_rival_held_out_of_the_fight_neither_looks_nor_thinks() {
    let mut w = world("", "");
    join(&mut w, 0, 40.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 60.0);
    put(&mut w, a, 60.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    let ci = w.combat.hero_index(a);
    w.combat.heroes[ci].held = Some(crate::combat::Held {
        kind: ControlKind::Incapacitate,
        left: 30.0,
        by: 0,
        spell: 0,
    });
    // Running at it in plain view, in its front cone: the control in the marks test fills it.
    walk(
        &mut w,
        0,
        V2::new(40.0, 60.0),
        V2::new(48.0, 60.0),
        secs(2.0),
        Gait::Run,
        0,
    );
    assert_eq!(w.rival_mind(a).unwrap().level(0), 0.0);
    assert!(w.rival_view(0).marks.is_empty());
}

/// Hero `player` held by `kind` for 30 s, as a rival's Cheap Shot would hold them.
fn hold_hero(w: &mut World, player: PlayerId, kind: ControlKind) {
    let ci = w.combat.hero_index(player);
    w.combat.heroes[ci].held = Some(crate::combat::Held {
        kind,
        left: 30.0,
        by: RIVAL_BASE,
        spell: 0,
    });
    let hi = w.hero_slot(player).unwrap();
    w.heroes[hi].fight.held = Some(kind);
}

#[test]
fn a_hero_held_by_a_rival_stops_auto_attacking_its_enemy() {
    let mut w = world("", &format!("{}\n", sentry_line(24.0, 60.0)));
    join(&mut w, 0, 20.0, 60.0);
    let brute = w.enemies[0].id;
    press(&mut w, 0, 1, 2, Some(UnitRef::Enemy(brute)));
    w.step();
    steps(&mut w, secs(4.5));
    let swings = |w: &mut World| {
        events_of(w, |e| match e {
            Event::Swing {
                by: UnitRef::Hero(0),
                ..
            } => Some(()),
            _ => None,
        })
        .len()
    };
    assert!(swings(&mut w) >= 2, "it swings at the brute while free");
    hold_hero(&mut w, 0, ControlKind::Stun);
    steps(&mut w, secs(5.0));
    assert_eq!(swings(&mut w), 0, "a stunned hero lands no blow");
}

/// A mage (player 1) at `(x, y)` facing +x, its first input taken.
fn join_mage(w: &mut World, x: f32, y: f32) {
    w.add_hero(1, "mage", "Mage", 1, 1).unwrap();
    put(w, 1, x, y, V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

#[test]
fn a_cast_spell_at_a_rival_lands_when_the_cast_ends() {
    let mut w = world("", "");
    join_mage(&mut w, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.drain_events();
    press(&mut w, 1, 1, 1, Some(UnitRef::Hero(a)));
    w.step();
    steps(&mut w, secs(1.5));
    assert!(
        damage_to(&w.drain_events(), UnitRef::Hero(a)).is_empty(),
        "still casting"
    );
    steps(&mut w, secs(0.8));
    let events = w.drain_events();
    assert_eq!(
        damage_to(&events, UnitRef::Hero(a)),
        vec![150.0],
        "100 x 1.5: unaware, an ambush: {events:?}"
    );
    assert_eq!(w.hero(a).unwrap().hp, 750.0);
}

#[test]
fn a_cast_at_a_rival_stops_when_the_rival_falls_or_leaves() {
    for leaves in [false, true] {
        let mut w = world("", "");
        join_mage(&mut w, 20.0, 60.0);
        let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
        w.drain_events();
        press(&mut w, 1, 1, 1, Some(UnitRef::Hero(a)));
        w.step();
        steps(&mut w, secs(0.5));
        if leaves {
            w.remove_hero(a);
        } else {
            w.hurt_hero(a, 5000.0);
        }
        w.step();
        let events = w.drain_events();
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::CastStop {
                    by: UnitRef::Hero(1),
                    why: crate::Stop::TargetLost,
                    ..
                }
            )),
            "leaves {leaves}: {events:?}"
        );
        steps(&mut w, secs(2.5));
        assert!(damage_to(&w.drain_events(), UnitRef::Hero(a)).is_empty());
    }
}

#[test]
fn a_slow_and_a_hold_on_a_hero_survive_the_senses_working_speed_out_again() {
    // M12: whatever re-derives a hero's speed between steps (a stealth press, a blow) goes
    // through `set_pace`, which lays the foes' slow and hold on last.
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    w.rederive(0);
    let free = w.hero(0).unwrap().speed;
    let ci = w.combat.hero_index(0);
    w.combat.heroes[ci].slow = Some(crate::combat::Slowed {
        by: 0.5,
        left: 5.0,
        spell: 0,
    });
    w.rederive(0);
    assert!((w.hero(0).unwrap().speed - free * 0.5).abs() < 1e-4);
    w.combat.heroes[ci].slow = None;
    hold_hero(&mut w, 0, ControlKind::Root);
    w.rederive(0);
    assert!(w.hero(0).unwrap().fight.rooted, "feet held");
}

#[test]
fn a_mirror_keeps_the_hold_its_host_sent() {
    // A client's mirror runs no steps, and no hold of its own to run down: the snapshot's
    // `fight.held` stays through whatever sets its pace.
    let mut host = world("", "");
    join(&mut host, 0, 20.0, 60.0);
    hold_hero(&mut host, 0, ControlKind::Stun);
    let snap = host.snapshot_for(0);
    let mut mirror = World::new(Arc::new(content("", "")));
    mirror.begin_mirror();
    mirror.apply_snapshot(snap);
    assert_eq!(mirror.hero(0).unwrap().fight.held, Some(ControlKind::Stun));
    let hi = mirror.hero_slot(0).unwrap();
    mirror.set_pace(hi);
    assert_eq!(mirror.hero(0).unwrap().fight.held, Some(ControlKind::Stun));
}

/// Party kind `key` brought in at `(x, y)` as party 0, at `aggression`, with dice `seed`.
pub(super) fn party(
    w: &mut World,
    key: &str,
    aggression: f32,
    x: f32,
    y: f32,
    seed: u64,
) -> Vec<PlayerId> {
    let def = w.content().party(key).unwrap();
    let members = w.add_party(def, 0, aggression, V2::new(x, y), seed);
    w.step();
    members
}

/// Whether the crate at `at` has been searched and emptied.
pub(super) fn emptied(w: &World, at: V2) -> bool {
    w.raid
        .containers
        .iter()
        .find(|c| c.at.distance(at) < 0.5 && matches!(c.holder, Holder::Placed(_)))
        .is_some_and(|c| c.searched && c.stacks.iter().all(Option::is_none))
}

#[test]
fn a_party_walks_around_a_house_to_its_point_and_loots_the_crate() {
    // A closed house stands between the rivals' entry and the yard.
    let mut b = flat(120.0);
    house(
        &mut b,
        V2::new(40.0, 50.0),
        V2::new(50.0, 70.0),
        None,
        false,
    );
    let mut w = world_on(b.finish(), "pois: (1, 1)", "", |c| RaidRoll::standard(c, 1));
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    assert_eq!(members.len(), 2);
    let mut inside = false;
    for _ in 0..secs(80.0) {
        w.step();
        for &m in &members {
            if let Some(h) = w.hero(m) {
                inside |= (40.0..=50.0).contains(&h.pos.x) && (50.0..=70.0).contains(&h.pos.y);
            }
        }
    }
    assert!(!inside, "nobody walked through the house");
    let p = &w.rivals.parties[0];
    assert_eq!(p.visited.len(), 1);
    let poi = &w.content().rivals.pois[p.visited[0]];
    assert!(
        emptied(&w, V2::new(poi.at.0, poi.at.1)),
        "its crate searched and emptied"
    );
}

#[test]
fn a_party_done_looting_walks_out_by_the_nearest_open_exit() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.drain_events();
    steps(&mut w, secs(120.0));
    let out = events_of(&mut w, |e| match e {
        Event::RivalOut { player, exit } => Some((*player, *exit)),
        _ => None,
    });
    assert_eq!(out.len(), 2, "{out:?}");
    for (p, _) in &out {
        assert!(members.contains(p));
    }
    assert!(members.iter().all(|&m| w.hero(m).is_none()));
}

#[test]
fn a_rival_in_a_fight_presses_once_per_choice() {
    let mut w = world(
        "pois: (1, 1)",
        &format!(
            "{}\n",
            r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (30.0, 60.0), facing: 180.0)),"#
        ),
    );
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.drain_events();
    steps(&mut w, secs(40.0));
    let events = w.drain_events();
    let refused: Vec<Refusal> = events
        .iter()
        .filter_map(|e| match e {
            Event::Refused { player, why } if members.contains(player) => Some(*why),
            _ => None,
        })
        .collect();
    let mashing = [
        Refusal::Busy,
        Refusal::NotReady,
        Refusal::NoPower,
        Refusal::Moving,
    ];
    assert!(refused.iter().all(|r| !mashing.contains(r)), "{refused:?}");
    assert!(refused.len() <= 2, "{refused:?}");
    let blows = events
        .iter()
        .filter(|e| {
            matches!(e, Event::Damage { to: UnitRef::Enemy(_), by: Some(UnitRef::Hero(p)), .. } if members.contains(p))
        })
        .count();
    assert!(blows >= 4, "they fought the sentry on their way: {blows}");
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::EnemyDied { by: Some(p), .. } if members.contains(p))));
}

#[test]
fn a_party_with_no_way_in_gives_the_point_up_and_leaves() {
    // The only point of interest is inside a sealed, roofed house.
    let rivals = RIVALS.replace(
        r#"(key: "barn", name: "The Barn", at: (90.0, 30.0), radius: 6.0)"#,
        r#"(key: "barn", name: "The Barn", at: (60.0, 60.0), radius: 3.0)"#,
    );
    let mut b = flat(120.0);
    house(&mut b, V2::new(55.0, 55.0), V2::new(65.0, 65.0), None, true);
    let c = content_with("pois: (2, 2), give_up: 5.0", &rivals, "");
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.drain_events();
    steps(&mut w, secs(90.0));
    assert_eq!(w.rivals.parties[0].visited.len(), 2, "both points given up");
    let out = events_of(&mut w, |e| {
        matches!(e, Event::RivalOut { .. }).then_some(())
    });
    assert_eq!(out.len(), members.len(), "and out by an exit");
}

#[test]
fn followers_keep_behind_their_leader() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "watch", 0.8, 10.0, 60.0, 3);
    steps(&mut w, secs(3.0));
    let start = w.hero(members[0]).unwrap().pos;
    for _ in 0..secs(10.0) {
        w.step();
        let (Some(lead), Some(follow)) = (w.hero(members[0]), w.hero(members[1])) else {
            panic!("both walk");
        };
        assert!(
            lead.pos.distance(follow.pos) <= 6.0,
            "{:?} {:?}",
            lead.pos,
            follow.pos
        );
    }
    assert!(
        w.hero(members[0]).unwrap().pos.distance(start) > 10.0,
        "the leader walked, the others after it"
    );
}

#[test]
fn a_sneaking_kind_goes_in_stealth_on_its_way() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "blades", 0.5, 10.0, 60.0, 5);
    assert_eq!(members.len(), 3);
    steps(&mut w, secs(3.0));
    assert!(members.iter().all(|&m| w.hero(m).unwrap().stealthed));
}

/// A monster with a telegraphed smash on itself, for the dodge.
const SMASHER: &str = r#"
    (key: "smasher", name: "Smasher", display: 200, level: 30, hp: 3000.0, walk_speed: 2.5,
     run_speed: 6.0, swing: Some((min: 10.0, max: 10.0, every: 2.0)),
     moves: [(ability: "dig", every: 6.0)]),
]"#;

/// A smasher at (32, 60) alerted to the first of a digger party standing at (30, 60).
fn smash_world(rivals_rules: &str, party_x: f32) -> (World, Vec<PlayerId>) {
    let enemies = ENEMIES.trim_end().trim_end_matches(']').to_string() + SMASHER;
    let c = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(rivals_rules),
        enemies: &enemies,
        spawns: r#"[(place: "Post", kind: "smasher", behaviour: Sentry(at: (32.0, 60.0), facing: 180.0)),]"#,
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals: RIVALS,
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, party_x, 60.0, 7);
    let id = w.enemies[0].id;
    w.alert(id, members[0]);
    w.drain_events();
    (w, members)
}

#[test]
fn a_rival_dodges_a_telegraph_it_stands_in() {
    let (mut w, members) = smash_world("dodge: 1.0, pois: (1, 1)", 30.0);
    steps(&mut w, secs(8.0));
    let events = w.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Dodged { player } if members.contains(player))));
    for &m in &members {
        assert!(
            damage_to(&events, UnitRef::Hero(m))
                .iter()
                .all(|&d| d < 100.0),
            "the smash missed: {:?}",
            damage_to(&events, UnitRef::Hero(m))
        );
    }
}

#[test]
fn a_rival_stays_out_of_a_telegraph_it_was_not_in_until_it_has_landed() {
    // A blow is kept about to land round the yard, 8 yards out, while a digger party walks to
    // the yard: they stop at its edge and walk in once it is gone.
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    let down = |w: &mut World| {
        w.telegraphs.clear();
        w.telegraphs.push(crate::Telegraph {
            id: 9_999,
            enemy: 0,
            by: None,
            ability: 0,
            shape: crate::Shape::Circle { radius: 8.0 },
            at: crate::V3::new(60.0, 60.0, 0.0),
            facing: V2::new(1.0, 0.0),
            left: 1.0,
            total: 1.2,
        });
    };
    let mut nearest = f32::MAX;
    for _ in 0..secs(60.0) {
        down(&mut w);
        w.step();
        for &m in &members {
            let d = w.hero(m).unwrap().pos.distance(V2::new(60.0, 60.0));
            nearest = nearest.min(d);
        }
    }
    assert!(
        nearest > 7.9,
        "one walked in to {nearest} yards of its centre"
    );
    assert!(nearest < 12.0, "they came up to its edge: {nearest}");
    // It has landed (or gone): they go in and loot.
    w.telegraphs.clear();
    steps(&mut w, secs(30.0));
    let p = &w.rivals.parties[0];
    assert!(
        p.looting.is_some() || !p.visited.is_empty(),
        "and then walk in: {:?}",
        p.goal
    );
}

/// [`world_on`] with `rivals.ron` of the routes' kind ([`with_routes`]), open ground.
fn world_routes(rivals_rules: &str, spawns: &str) -> World {
    let c = content_with(rivals_rules, &with_routes(), spawns);
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    w
}

#[test]
fn a_party_walks_the_routes_of_the_content_and_leaves_by_the_exit_they_end_at() {
    let mut w = world_routes("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    // It starts at the entry and has its route to the yard.
    steps(&mut w, secs(2.0));
    let p = &w.rivals.parties[0];
    assert_eq!(p.entry, Some(0));
    assert_eq!(p.journey.as_ref().map(|j| j.to), Some(RoutePlace::Poi(0)));
    w.drain_events();
    steps(&mut w, secs(150.0));
    let p = &w.rivals.parties[0];
    assert_eq!(p.place, Some(RoutePlace::Poi(0)), "it was at the yard");
    assert_eq!(p.visited, vec![0]);
    let out = events_of(&mut w, |e| match e {
        Event::RivalOut { player, exit } => Some((*player, *exit)),
        _ => None,
    });
    assert_eq!(out.len(), members.len(), "{out:?}");
    // The yard's routes end at the east and north gates and the cart; the east is the shortest.
    assert!(out.iter().all(|&(_, exit)| exit == 0), "{out:?}");
}

#[test]
fn a_party_goes_from_one_point_to_another_by_way_of_its_entry() {
    // The content has a route from the entry to each point and from each point to each exit,
    // none between two points: the party walks back by the entry. It is found out by where it
    // is when its first point is done.
    let mut w = world_routes("pois: (2, 2), give_up: 40.0", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    let mut at_yard = false;
    let mut nearest_entry_after = f32::MAX;
    for _ in 0..secs(200.0) {
        w.step();
        let p = &w.rivals.parties[0];
        at_yard |= !p.visited.is_empty();
        if at_yard && p.visited.len() == 1 && p.looting.is_none() {
            if let Some(h) = w.hero(members[0]) {
                nearest_entry_after = nearest_entry_after.min(h.pos.distance(V2::new(10.0, 60.0)));
            }
        }
    }
    assert!(at_yard, "the first point was done");
    assert!(
        nearest_entry_after < 8.0,
        "on its way to the second it passed its entry: {nearest_entry_after}"
    );
}

#[test]
fn a_route_between_two_points_is_made_of_the_routes_to_their_entry_and_a_way_out_from_an_entry_of_the_best_point(
) {
    let mut w = world_routes("pois: (2, 2)", "");
    join(&mut w, 0, 110.0, 110.0);
    party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    let rc = w.content().rivals.clone();
    let len = |p: &[V2]| -> f32 { p.windows(2).map(|s| s[0].distance(s[1])).sum() };
    // Entry to an exit: by the point with the shortest whole way.
    let exit = RoutePlace::Exit(0);
    let whole = w.route_points(0, exit).unwrap();
    let best = (0..rc.pois.len())
        .filter_map(|q| {
            let a = rc.route(RoutePlace::Entry(0), RoutePlace::Poi(q))?;
            let b = rc.route(RoutePlace::Poi(q), exit)?;
            Some(len(&a) + len(&b))
        })
        .fold(f32::MAX, f32::min);
    assert!(
        (len(&whole) - best).abs() < 1e-3,
        "{} against {best}",
        len(&whole)
    );
    assert_eq!(whole.first(), Some(&V2::new(10.0, 60.0)));
    assert_eq!(whole.last(), Some(&V2::new(115.0, 60.0)));
    // A point to another: by the entry, the joint once.
    w.rivals.parties[0].place = Some(RoutePlace::Poi(0));
    let across = w.route_points(0, RoutePlace::Poi(1)).unwrap();
    assert_eq!(across.first(), Some(&V2::new(60.0, 60.0)));
    assert_eq!(across.last(), Some(&V2::new(90.0, 30.0)));
    assert_eq!(
        across.iter().filter(|&&p| p == V2::new(10.0, 60.0)).count(),
        1
    );
    // The place it stands at has no route to itself.
    assert_eq!(w.route_points(0, RoutePlace::Poi(0)), None);
}

#[test]
fn rivals_search_inside_the_pool_the_step_leaves_them() {
    // Many parties walking round a long wall at once, each wanting its own searches.
    let mut b = flat(120.0);
    b.add_box(V2::new(30.0, 0.0), V2::new(31.0, 100.0), 0.0, 10.0);
    let mut w = world_on(b.finish(), "pois: (1, 1), paths_per_step: 40", "", |c| {
        RaidRoll::standard(c, 1)
    });
    join(&mut w, 0, 110.0, 110.0);
    for number in 0..6u8 {
        let def = w.content().party("diggers").unwrap();
        w.add_party(
            def,
            number,
            0.1,
            V2::new(20.0, 10.0 + f32::from(number) * 12.0),
            u64::from(number),
        );
    }
    w.step();
    let mut worst = 0;
    let mut searched = 0;
    for _ in 0..secs(30.0) {
        let before = w.nav.as_ref().unwrap().floors_looked_at();
        w.step();
        let spent = (w.nav.as_ref().unwrap().floors_looked_at() - before) as usize;
        worst = worst.max(spent);
        searched += spent;
    }
    assert!(searched > 0, "they searched");
    assert!(
        worst <= crate::RIVAL_POOL,
        "one step looked at {worst} floors"
    );
}

#[test]
fn a_party_sealed_in_slips_away_unseen_when_no_hero_sees_it() {
    // A roofed room with no door: nothing in it can go anywhere.
    let mut b = flat(120.0);
    house(&mut b, V2::new(50.0, 50.0), V2::new(62.0, 62.0), None, true);
    let mut w = world_on(
        b.finish(),
        "pois: (1, 1), stuck_for: 10.0, give_up: 3.0",
        "",
        |c| RaidRoll::standard(c, 1),
    );
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 55.0, 55.0, 7);
    assert_eq!(members.len(), 2);
    w.drain_events();
    steps(&mut w, secs(60.0));
    assert!(
        members.iter().all(|&m| w.hero(m).is_none()),
        "they are gone"
    );
    assert_eq!(w.rivals.out.len(), 2, "counted out");
    let events = w.drain_events();
    assert!(
        !events.iter().any(|e| matches!(e, Event::RivalOut { .. })),
        "nobody saw them go"
    );
}

#[test]
fn a_party_stays_while_a_hero_watches_it_stuck() {
    // A pen of walls too high to climb and low enough to see over: the hero outside sees in.
    let mut b = flat(120.0);
    for (min, max) in [
        (V2::new(50.0, 50.0), V2::new(62.0, 50.5)),
        (V2::new(50.0, 61.5), V2::new(62.0, 62.0)),
        (V2::new(50.0, 50.0), V2::new(50.5, 62.0)),
        (V2::new(61.5, 50.0), V2::new(62.0, 62.0)),
    ] {
        b.add_box(min, max, 0.0, 1.0);
    }
    let mut w = world_on(
        b.finish(),
        "pois: (1, 1), stuck_for: 10.0, give_up: 3.0",
        "",
        |c| RaidRoll::standard(c, 1),
    );
    join(&mut w, 0, 70.0, 56.0);
    let members = party(&mut w, "diggers", 0.1, 55.0, 55.0, 7);
    steps(&mut w, secs(30.0));
    assert!(
        members.iter().any(|&m| w.hero(m).is_some()),
        "in plain sight they stay, whatever the backstop says"
    );
}

#[test]
fn every_party_leaves_or_dies_within_the_night_on_the_small_map() {
    let mut w = world_routes(
        "pois: (1, 2)",
        &format!(
            "{}\n{}\n",
            r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (60.0, 62.0), facing: 180.0)),"#,
            r#"(place: "Post2", kind: "brute", behaviour: Sentry(at: (92.0, 30.0), facing: 180.0)),"#
        ),
    );
    join(&mut w, 0, 110.0, 110.0);
    let mut all = Vec::new();
    for (number, (key, aggression)) in [
        ("diggers", 0.1),
        ("watch", 0.9),
        ("blades", 0.5),
        ("diggers", 0.6),
    ]
    .into_iter()
    .enumerate()
    {
        let def = w.content().party(key).unwrap();
        let at = V2::new(10.0, 56.0 + number as f32 * 3.0);
        all.extend(w.add_party(def, number as u8, aggression, at, number as u64 + 1));
    }
    w.step();
    steps(&mut w, secs(900.0));
    let left: Vec<PlayerId> = all
        .iter()
        .copied()
        .filter(|&m| w.hero(m).is_some_and(|h| h.fight.up()))
        .collect();
    assert!(left.is_empty(), "still about after the night: {left:?}");
}

#[test]
fn twin_seeds_bring_parties_with_different_dice() {
    let same = (0..100u64)
        .filter(|&s| {
            let mut a = world("", "");
            let mut b = world("", "");
            let def = a.content().party("diggers").unwrap();
            let ma = a.add_party(def, 0, 0.1, V2::new(10.0, 60.0), s);
            let mb = b.add_party(def, 0, 0.1, V2::new(10.0, 60.0), s ^ 1);
            let names = |w: &World, m: &[PlayerId]| -> Vec<String> {
                m.iter().map(|&p| w.hero(p).unwrap().name.clone()).collect()
            };
            names(&a, &ma) == names(&b, &mb)
                && a.rivals.parties[0].pois_left == b.rivals.parties[0].pois_left
        })
        .count();
    assert!(same < 40, "{same} of 100 twin seeds rolled the same party");
}

#[test]
fn rival_bots_do_not_run_on_a_mirror() {
    // A mirror has no map: its rivals are drawn from the host's snapshots, never moved. Even
    // handed the host's bodies and parties, it moves nobody.
    let mut host = world("pois: (1, 1)", "");
    join(&mut host, 0, 110.0, 110.0);
    party(&mut host, "diggers", 0.1, 10.0, 60.0, 7);
    steps(&mut host, secs(5.0));
    let snap = host.snapshot_for(0);
    let mut mirror = World::new(Arc::new(content("pois: (1, 1)", "")));
    mirror.begin_mirror();
    mirror.apply_snapshot(snap);
    mirror.rivals.bodies = host.rivals.bodies.clone();
    mirror.rivals.parties = host.rivals.parties.clone();
    let at: Vec<V2> = mirror.heroes.iter().map(|h| h.pos).collect();
    assert!(mirror.heroes.iter().any(|h| h.side.is_rival()));
    steps(&mut mirror, secs(5.0));
    assert_eq!(
        mirror.heroes.iter().map(|h| h.pos).collect::<Vec<_>>(),
        at,
        "nobody moved by themselves"
    );
}

#[test]
fn the_party_remembers_the_loudest_noise_a_hero_made_lately() {
    let mut w = world("pois: (1, 1)", "");
    rival(&mut w, 0, "digger", 0, 10.0, 10.0);
    join(&mut w, 0, 100.0, 100.0);
    // Running is loud: the noise is kept with where it came from.
    put(&mut w, 0, 100.0, 100.0, V2::new(1.0, 0.0), Gait::Run);
    w.step();
    put(&mut w, 0, 101.0, 100.0, V2::new(1.0, 0.0), Gait::Run);
    w.step();
    let (at, radius, _) = w.rivals.rumour.expect("a noise was kept");
    assert!(at.ground().distance(V2::new(101.0, 100.0)) < 3.0 && radius > 0.0);
    // Forgotten after the hunt's memory.
    let memory = w.content().rules.night.hunt.memory;
    put(&mut w, 0, 101.0, 100.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(memory + 2.0));
    assert!(w.rivals.rumour.is_none());
}

#[test]
fn a_bold_party_with_nothing_left_to_loot_hunts_the_noise_it_heard() {
    let mut w = world("pois: (1, 1), hunt_at: 0.7", "");
    join(&mut w, 0, 100.0, 20.0);
    let members = party(&mut w, "watch", 0.9, 10.0, 60.0, 3);
    let mut hunted = false;
    let mut nearest = f32::MAX;
    for i in 0..secs(240.0) {
        // The rogue runs on the spot far off, keeping a noise going.
        let x = 100.0 + (i % 2) as f32 * 0.1;
        put(&mut w, 0, x, 20.0, V2::new(1.0, 0.0), Gait::Run);
        w.step();
        hunted |= matches!(w.rivals.parties[0].goal, crate::rivals::Goal::Hunt { .. });
        for &m in &members {
            if let Some(h) = w.hero(m) {
                nearest = nearest.min(h.pos.distance(V2::new(100.0, 20.0)));
            }
        }
    }
    assert!(hunted, "it went after the noise");
    assert!(w.rivals.parties[0].hunted);
    // And it came nearer the noise than the entry it started from (98 yards off).
    assert!(nearest < 60.0, "{nearest} yards from the noise");
}

#[test]
fn the_trace_has_a_line_for_each_party() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    let lines = w.rival_debug();
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("Diggers") && lines[0].contains("aggression 0.10"),
        "{lines:?}"
    );
}

#[test]
fn a_client_is_sent_only_the_rivals_within_its_heros_reach() {
    // A short view, so the 120-yard map has a far corner.
    let c = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules("pois: (1, 1)").replace("view_distance: 200.0", "view_distance: 45.0"),
        enemies: ENEMIES,
        spawns: "[]",
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals: RIVALS,
    })
    .unwrap_or_else(|e| panic!("{e}"));
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    join(&mut w, 0, 20.0, 60.0);
    let near = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let far = rival(&mut w, 1, "digger", 0, 119.0, 119.0);
    let reach = w.reach();
    assert!(w.hero(far).unwrap().pos.distance(V2::new(20.0, 60.0)) > reach);
    let ids =
        |s: &crate::Snapshot| -> Vec<PlayerId> { s.heroes.iter().map(|h| h.player).collect() };
    let snap = w.snapshot_for(0);
    assert!(ids(&snap).contains(&0), "its own hero");
    assert!(ids(&snap).contains(&near));
    assert!(!ids(&snap).contains(&far), "{:?}", ids(&snap));
    // The party is sent whole: a friend far off is still in it, and a rival beside the friend is
    // not, being out of the first hero's reach.
    w.add_hero(1, "rogue", "Friend", 1, 0).unwrap();
    put(&mut w, 1, 119.0, 119.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    let snap = w.snapshot_for(0);
    assert!(ids(&snap).contains(&1), "the party whole: {:?}", ids(&snap));
    assert!(!ids(&snap).contains(&far));
    // The friend sees the rival beside them, and not the near one 100 yards off.
    let snap = w.snapshot_for(1);
    assert!(ids(&snap).contains(&far) && !ids(&snap).contains(&near));
    // A player who has left is sent no rival at all.
    w.remove_hero(1);
    let snap = w.snapshot_for(1);
    assert!(
        snap.heroes.iter().all(|h| !h.side.is_rival()),
        "{:?}",
        ids(&snap)
    );
}

#[test]
fn a_rival_that_could_not_move_for_stuck_for_seconds_slips_out_alone() {
    let mut w = world("pois: (1, 1), stuck_for: 20.0", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.rivals.body_mut(members[1]).unwrap().pinned = 20.0;
    w.step();
    assert!(w.hero(members[1]).is_none(), "the pinned one is gone");
    assert!(w.hero(members[0]).is_some(), "its friend walks on");
    assert_eq!(w.rivals.out, vec![members[1]]);
}

#[test]
fn the_way_from_one_point_to_another_is_not_walked_twice_where_the_ways_in_share_a_start() {
    // Both ways in leave the entry by the same waypoint: back from the yard only as far as it,
    // then on to the barn, never to the entry itself.
    let routes = with_routes()
        .replace(
            r#"(from: Entry(0), to: Poi("yard"), via: [])"#,
            r#"(from: Entry(0), to: Poi("yard"), via: [(30.0, 60.0)])"#,
        )
        .replace(
            r#"(from: Entry(0), to: Poi("barn"), via: [(50.0, 45.0)])"#,
            r#"(from: Entry(0), to: Poi("barn"), via: [(30.0, 60.0), (50.0, 45.0)])"#,
        );
    let c = content_with("pois: (2, 2)", &routes, "");
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(open()));
    join(&mut w, 0, 110.0, 110.0);
    party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.rivals.parties[0].place = Some(RoutePlace::Poi(0));
    let across = w.route_points(0, RoutePlace::Poi(1)).unwrap();
    assert_eq!(
        across,
        vec![
            V2::new(60.0, 60.0),
            V2::new(30.0, 60.0),
            V2::new(50.0, 45.0),
            V2::new(90.0, 30.0)
        ]
    );
}

#[test]
fn a_leader_waits_for_a_follower_that_falls_behind() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    // The follower is rooted where it stands; the leader may not walk off from it.
    hold_hero(&mut w, members[1], ControlKind::Root);
    let held_at = w.hero(members[1]).unwrap().pos;
    steps(&mut w, secs(20.0));
    let lead = w.hero(members[0]).unwrap().pos;
    assert!(
        lead.distance(held_at) <= 15.0,
        "the leader walked on to {lead:?} from {held_at:?}"
    );
    // Freed, they go on together.
    let hi = w.hero_slot(members[1]).unwrap();
    w.heroes[hi].fight.held = None;
    let ci = w.combat.hero_index(members[1]);
    w.combat.heroes[ci].held = None;
    steps(&mut w, secs(20.0));
    assert!(w.hero(members[0]).unwrap().pos.distance(held_at) > 25.0);
}

#[test]
fn the_same_party_walks_the_same_way_twice() {
    let run = |seed: u64| -> Vec<(i32, i32)> {
        let mut w = world_routes(
            "pois: (1, 2)",
            &format!(
                "{}\n",
                r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (60.0, 62.0), facing: 180.0)),"#
            ),
        );
        join(&mut w, 0, 110.0, 110.0);
        let members = party(&mut w, "watch", 0.8, 10.0, 60.0, seed);
        let mut trail = Vec::new();
        for i in 0..secs(90.0) {
            w.step();
            if i % 30 == 0 {
                for &m in &members {
                    if let Some(h) = w.hero(m) {
                        trail.push(((h.pos.x * 100.0) as i32, (h.pos.y * 100.0) as i32));
                    }
                }
            }
        }
        trail
    };
    assert_eq!(run(5), run(5), "the same dice walk the same way");
    assert_ne!(run(5), run(6), "other dice walk another way");
}

#[test]
fn the_same_seed_brings_the_same_rivals_and_seeds_differ() {
    let c = content("", "");
    assert_eq!(RaidRoll::roll(&c, 42).rivals, RaidRoll::roll(&c, 42).rivals);
    let rolls: Vec<Vec<RivalOut>> = (0..30).map(|s| RaidRoll::roll(&c, s).rivals).collect();
    assert!(rolls.iter().any(|r| r != &rolls[0]));
}

#[test]
fn raids_of_twin_seeds_bring_different_rivals() {
    // `Dice::new` sets bit 0, so a seed and its twin roll alike unless the seed is hashed first
    // (ruling 6); the dice of every party are hashed from the seed too.
    let c = content("", "");
    let key = |r: Vec<RivalOut>| -> Vec<(u8, u8, u32, u32)> {
        r.iter()
            .map(|o| (o.party, o.entry, o.at.to_bits(), o.aggression.to_bits()))
            .collect()
    };
    let same = (0..100u64)
        .filter(|&s| key(RaidRoll::roll(&c, s).rivals) == key(RaidRoll::roll(&c, s ^ 1).rivals))
        .count();
    assert!(same < 5, "{same} of 100 twin seeds rolled the same rivals");
    for s in 0..100u64 {
        let (a, b) = (
            RaidRoll::roll(&c, s).rivals,
            RaidRoll::roll(&c, s ^ 1).rivals,
        );
        for (x, y) in a.iter().zip(&b) {
            assert_ne!(
                x.seed, y.seed,
                "twin seeds {s}: the party's own dice differ"
            );
        }
    }
}

#[test]
fn the_rivals_do_not_disturb_the_rest_of_the_roll() {
    let with = content("", "");
    let without = content_with("", "", "");
    for seed in 0..20 {
        let (a, b) = (RaidRoll::roll(&with, seed), RaidRoll::roll(&without, seed));
        assert!(b.rivals.is_empty(), "no rivals.ron, no rivals");
        assert_eq!(
            (a.exits, a.weather, a.monsters, a.closes, a.entry),
            (b.exits, b.weather, b.monsters, b.closes, b.entry)
        );
    }
}

#[test]
fn parties_out_bounds_the_parties_and_a_party_at_odds_zero_never_comes() {
    let c = content("parties_out: (1, 2), arrive: (30.0, 90.0)", "");
    let blades = c.party("blades").unwrap() as u8;
    let mut counts = [0usize; 3];
    for seed in 0..200 {
        let out = RaidRoll::roll(&c, seed).rivals;
        assert!((1..=2).contains(&out.len()), "seed {seed}: {out:?}");
        counts[out.len()] += 1;
        for o in &out {
            assert_ne!(o.party, blades);
            assert!((30.0..=90.0).contains(&o.at));
            let (lo, hi) = c.rivals.parties[usize::from(o.party)].aggression;
            assert!((lo..=hi).contains(&o.aggression));
            assert!(usize::from(o.entry) < c.rivals.entries.len());
        }
        let mut parties: Vec<u8> = out.iter().map(|o| o.party).collect();
        parties.dedup();
        assert_eq!(parties.len(), out.len(), "a party comes once");
    }
    assert!(counts[2] > 0, "two come up when the dice allow: {counts:?}");
}

#[test]
fn a_party_at_odds_a_half_comes_about_half_the_raids_and_parties_out_tops_up() {
    let half = RIVALS.replace(
        "aggression: (0.8, 0.9),\n         odds: 1.0",
        "aggression: (0.8, 0.9),\n         odds: 0.5",
    );
    assert_ne!(half, RIVALS);
    let c = content_with("parties_out: (0, 2)", &half, "");
    let watch = c.party("watch").unwrap() as u8;
    let came = (0..400u64)
        .filter(|&s| {
            RaidRoll::roll(&c, s)
                .rivals
                .iter()
                .any(|o| o.party == watch)
        })
        .count();
    assert!((120..280).contains(&came), "{came} of 400");
    // With at least two wanted and only two at odds above zero, both are brought.
    let c = content("parties_out: (2, 2)", "");
    assert!((0..50u64).all(|s| RaidRoll::roll(&c, s).rivals.len() == 2));
}

#[test]
fn the_standard_roll_brings_none_and_a_roll_naming_a_missing_party_is_refused() {
    let c = Arc::new(content("", ""));
    assert!(RaidRoll::standard(&c, 1).rivals.is_empty());
    let mut bad = RaidRoll::standard(&c, 1);
    bad.rivals.push(RivalOut {
        party: 9,
        ..RivalOut::default()
    });
    assert!(World::new(c.clone()).begin_raid(bad).is_err());
    let mut far = RaidRoll::standard(&c, 1);
    far.rivals.push(RivalOut {
        entry: 5,
        ..RivalOut::default()
    });
    assert!(World::new(c.clone()).begin_raid(far).is_err());
    let mut nan = RaidRoll::standard(&c, 1);
    nan.rivals.push(RivalOut {
        at: f32::NAN,
        ..RivalOut::default()
    });
    assert!(World::new(c.clone()).begin_raid(nan).is_err());
    let mut ok = RaidRoll::standard(&c, 1);
    ok.rivals.push(RivalOut::default());
    assert!(World::new(c).begin_raid(ok).is_ok());
}

/// A world on open ground whose roll brings `plan`, a hero (player 0) at `(x, y)`.
fn world_with_plan(plan: RivalOut, x: f32, y: f32) -> World {
    let mut w = world_on(open(), "", "", |c| {
        let mut r = RaidRoll::standard(c, 1);
        r.rivals.push(plan);
        r
    });
    join(&mut w, 0, x, y);
    w
}

const PLAN: RivalOut = RivalOut {
    party: 0,
    entry: 0,
    at: 5.0,
    aggression: 0.15,
    seed: 3,
};

#[test]
fn rivals_arrive_late_and_unseen_with_numbers_of_their_own() {
    // A hero two yards from the rivals' entry: nobody comes while they stand there.
    let mut w = world_with_plan(PLAN, 12.0, 60.0);
    steps(&mut w, secs(10.0));
    assert!(
        w.heroes.iter().all(|h| h.side == Side::Party),
        "not in their sight"
    );
    put(&mut w, 0, 80.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(2.0));
    let rivals: Vec<&crate::Hero> = w.heroes.iter().filter(|h| h.side.is_rival()).collect();
    assert_eq!(rivals.len(), 2);
    assert!(rivals
        .iter()
        .all(|h| h.player >= RIVAL_BASE && h.side == Side::Rival(0)));
    assert_ne!(rivals[0].player, rivals[1].player);
    assert!(
        rivals.iter().all(|h| !h.carry.worn.is_empty()),
        "dressed in their gear"
    );
    let gem = w.content().item("gem").unwrap();
    assert!(rivals
        .iter()
        .all(|h| w.raid.pack(h.player).is_some_and(|p| p.count_of(gem) >= 1)));
    assert_eq!(w.rivals.parties[0].aggression, 0.15);
    // It comes once.
    steps(&mut w, secs(3.0));
    assert_eq!(w.heroes.iter().filter(|h| h.side.is_rival()).count(), 2);
}

#[test]
fn a_party_does_not_come_before_its_time() {
    let late = RivalOut { at: 30.0, ..PLAN };
    let mut w = world_with_plan(late, 100.0, 100.0);
    steps(&mut w, secs(28.0));
    assert!(w.heroes.iter().all(|h| !h.side.is_rival()), "not yet");
    steps(&mut w, secs(4.0));
    assert_eq!(w.heroes.iter().filter(|h| h.side.is_rival()).count(), 2);
}

#[test]
fn a_hero_who_is_down_does_not_keep_a_party_waiting() {
    let mut w = world_with_plan(PLAN, 12.0, 60.0);
    steps(&mut w, secs(10.0));
    assert!(w.heroes.iter().all(|h| !h.side.is_rival()));
    w.wound(0, 1.0e6, None);
    w.step();
    assert!(!w.hero(0).unwrap().fight.up());
    steps(&mut w, secs(2.0));
    assert_eq!(w.heroes.iter().filter(|h| h.side.is_rival()).count(), 2);
}

#[test]
fn a_new_raid_brings_its_rivals_afresh() {
    let mut w = world_with_plan(PLAN, 100.0, 100.0);
    steps(&mut w, secs(8.0));
    assert_eq!(w.heroes.iter().filter(|h| h.side.is_rival()).count(), 2);
    let c = w.content().clone();
    let mut again = RaidRoll::standard(&c, 2);
    again.rivals.push(PLAN);
    w.begin_raid(again).unwrap();
    assert!(w.rivals.arrived.is_empty() && w.rivals.parties.is_empty());
    assert_eq!(w.rivals.next, 0, "numbers start over");
}

#[test]
fn a_mirror_brings_in_no_rivals_of_its_own() {
    // A client's world has the roll but no map: its rivals come only in the host's snapshots.
    let c = Arc::new(content("", ""));
    let mut roll = RaidRoll::standard(&c, 1);
    roll.rivals.push(PLAN);
    let mut mirror = World::new(c);
    mirror.begin_raid(roll).unwrap();
    mirror.begin_mirror();
    steps(&mut mirror, secs(10.0));
    assert!(mirror.heroes.iter().all(|h| !h.side.is_rival()));
    assert!(mirror.rivals.parties.is_empty());
}

#[test]
fn a_roll_with_rivals_crosses_the_wire() {
    let c = content("", "");
    let r = RaidRoll::roll(&c, 11);
    assert!(!r.rivals.is_empty());
    let bytes = postcard::to_stdvec(&r).unwrap();
    let back: RaidRoll = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(back, r);
}
#[test]
fn another_rival_at_the_entry_does_not_keep_a_party_from_coming() {
    // Only heroes of the players' party can see a party come in; a rival standing at the entry
    // (an earlier party that has not walked off yet) is no eyes.
    let mut w = world_with_plan(PLAN, 100.0, 100.0);
    rival(&mut w, 5, "digger", 1, 11.0, 60.0);
    steps(&mut w, secs(8.0));
    assert!(w.rivals.parties.len() == 1, "the roll's party came");
}

/// A world on open ground whose roll brings `plans`, the party's hero (player 0) far off at
/// (110, 110).
fn raid_with(rivals_rules: &str, spawns: &str, plans: &[RivalOut]) -> World {
    let mut w = world_on(open(), rivals_rules, spawns, |c| {
        let mut r = RaidRoll::standard(c, 1);
        r.rivals.extend_from_slice(plans);
        r
    });
    join(&mut w, 0, 110.0, 110.0);
    w
}

fn plan(party: u8, aggression: f32) -> RivalOut {
    RivalOut {
        party,
        entry: 0,
        at: 0.0,
        aggression,
        seed: 21,
    }
}

/// Hero 0 runs back and forth between `a` and `b` for `n` steps, a loud walk.
fn run_between(w: &mut World, a: V2, b: V2, n: usize) {
    for i in 0..n {
        let t = ((i as f32 / secs(3.0) as f32) % 2.0 - 1.0).abs();
        let at = a + (b - a) * t;
        put(w, 0, at.x, at.y, (b - a).normalize_or_zero(), Gait::Run);
        w.step();
    }
}

fn rivals_up(w: &World) -> usize {
    w.heroes
        .iter()
        .filter(|h| h.side.is_rival() && h.fight.up())
        .count()
}

#[test]
fn a_cautious_party_loots_its_points_and_extracts() {
    let mut w = raid_with("pois: (2, 2)", "", &[plan(0, 0.1)]);
    w.drain_events();
    steps(&mut w, secs(300.0));
    assert!(emptied(&w, V2::new(60.0, 60.0)) && emptied(&w, V2::new(90.0, 30.0)));
    let out = events_of(&mut w, |e| {
        matches!(e, Event::RivalOut { .. }).then_some(())
    });
    assert_eq!(out.len(), 2, "both out");
    assert_eq!(rivals_up(&w), 0);
    assert!(
        w.raid
            .containers
            .iter()
            .all(|c| !matches!(c.holder, Holder::Hero(_))),
        "no bodies"
    );
    assert!(w.take_outcomes().is_empty(), "rivals settle no outcome");
}

#[test]
fn a_bold_party_done_looting_hunts_a_noisy_hero_and_fights_it() {
    let mut w = raid_with("pois: (1, 1), hunt_for: 150.0", "", &[plan(1, 0.9)]);
    w.drain_events();
    // While they loot, the hero runs about 50 yards off, loud.
    run_between(
        &mut w,
        V2::new(100.0, 100.0),
        V2::new(100.0, 90.0),
        secs(240.0),
    );
    assert!(w.rivals.parties[0].hunted, "they went hunting");
    let events = w.drain_events();
    let hit = |e: &Event| matches!(e, Event::Damage { to: UnitRef::Hero(0), by: Some(UnitRef::Hero(r)), .. } if *r >= RIVAL_BASE);
    // They found the hero and struck it: a blow in the events, or health already lost.
    let h = w.hero(0).unwrap();
    assert!(
        events.iter().any(hit) || !h.fight.up() || h.hp < h.max_hp,
        "they found and fought the hero"
    );
}

#[test]
fn a_party_fights_the_monster_guarding_its_point_then_loots_it() {
    let mut w = raid_with(
        "pois: (1, 1)",
        &format!("{}\n", sentry_line(62.0, 60.0)),
        &[plan(0, 0.1)],
    );
    w.drain_events();
    steps(&mut w, secs(240.0));
    let events = w.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::EnemyDied { by: Some(p), .. } if *p >= RIVAL_BASE)));
    let poi = &w.content().rivals.pois[w.rivals.parties[0].visited[0]];
    assert!(emptied(&w, V2::new(poi.at.0, poi.at.1)));
}

#[test]
fn a_slain_rival_drops_its_bag_for_the_hero_to_take() {
    let mut w = raid_with("", "", &[]);
    let a = rival(&mut w, 0, "digger", 0, 105.0, 110.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    w.step();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(a))
        .map(|c| c.id)
        .expect("a body");
    put(&mut w, 0, 106.0, 110.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    let act = |w: &mut World, seq: u8, act: Act| {
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
                act: Some(ActPress { seq, act }),
                ..Default::default()
            },
        );
        w.step();
    };
    act(&mut w, 1, Act::Open(body));
    steps(&mut w, secs(10.0));
    act(&mut w, 2, Act::TakeAll);
    let sack = w.content().item("sack").unwrap();
    assert!(
        w.raid.pack(0).unwrap().count_of(sack) >= 1,
        "its bag is the hero's now"
    );
}

#[test]
fn every_party_leaves_or_dies_within_the_night() {
    for seed in [1, 2] {
        let c = content("arrive: (0.0, 60.0), pois: (1, 2)", "");
        let roll = RaidRoll::roll(&c, seed);
        assert!(!roll.rivals.is_empty());
        let mut w = World::new(Arc::new(c));
        w.begin_raid(roll).unwrap();
        w.attach_map(Arc::new(open()));
        join(&mut w, 0, 110.0, 110.0);
        steps(&mut w, secs(900.0));
        let left: Vec<String> = w
            .heroes
            .iter()
            .filter(|h| h.side.is_rival() && h.fight.up())
            .map(|h| format!("{} at ({:.0}, {:.0})", h.name, h.pos.x, h.pos.y))
            .collect();
        assert!(
            left.is_empty(),
            "seed {seed}: {left:?}\n{:#?}",
            w.rival_debug()
        );
        assert!(!w.rivals.out.is_empty() || w.heroes.iter().any(|h| h.side.is_rival()));
    }
}

#[test]
fn a_world_with_rivals_plays_the_same_twice() {
    let run = || {
        let mut w = raid_with(
            "pois: (1, 2)",
            &format!("{}\n", sentry_line(62.0, 60.0)),
            &[plan(0, 0.2), plan(1, 0.9)],
        );
        run_between(
            &mut w,
            V2::new(100.0, 100.0),
            V2::new(100.0, 80.0),
            secs(80.0),
        );
        w.snapshot()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_container_the_host_will_not_open_is_given_up_after_give_up_seconds() {
    // Both crates stand 4 yards above the rivals' floor: the host refuses every `Open` (it
    // reaches 3 yards up), so only the loot clock lets the party get on.
    let mut w = world("pois: (1, 1), give_up: 8.0", "");
    for c in &mut w.raid.containers {
        c.z = Some(4.0);
    }
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    steps(&mut w, secs(120.0));
    let p = &w.rivals.parties[0];
    assert_eq!(p.visited, vec![0], "the point is done: {:?}", p.goal);
    assert!(
        w.raid
            .containers
            .iter()
            .any(|c| c.at == V2::new(60.0, 60.0) && !c.searched),
        "and never opened"
    );
    assert!(members
        .iter()
        .all(|&m| w.hero(m).is_none_or(|h| h.fight.up())));
    // The party carried on to the exit: it is gone, out by the gate or unseen.
    assert!(w.rivals.out.len() == members.len(), "{:?}", w.rival_debug());
}

#[test]
fn a_loot_reach_the_host_would_refuse_is_refused_at_load() {
    assert!(refused("loot_reach: 50.0", RIVALS));
    assert!(!refused("loot_reach: 2.5", RIVALS));
}

#[test]
fn a_goal_given_up_for_the_next_keeps_the_idle_time_it_had() {
    // The backstop must see through a party cycling through points it cannot reach: giving one
    // point up for a nearer one is no headway (the call to `Progress::rebase`).
    let mut w = world("pois: (2, 2)", "");
    join(&mut w, 0, 110.0, 110.0);
    party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    let p = &mut w.rivals.parties[0];
    p.progress = super::travel::Progress::aged(10.0, 500.0);
    p.visited = vec![0];
    p.looting = None;
    p.goal = crate::rivals::Goal::Loot { poi: 0 };
    p.next_poi = Some(1);
    p.place = Some(RoutePlace::Entry(0));
    w.step();
    let p = &w.rivals.parties[0];
    assert!(
        matches!(p.goal, crate::rivals::Goal::Loot { poi: 1 }),
        "{:?}",
        p.goal
    );
    assert!(p.progress.idle() >= 10.0, "idle {}", p.progress.idle());
}

#[test]
fn a_leader_held_for_a_follower_is_not_idle_and_the_follower_alone_slips_away() {
    // A follower sealed in a room (no way to its leader) is `pinned`; its leader waits for it and
    // that wait is no idleness, so the party does not leave: only the follower does, after
    // `stuck_for`, and the leader goes on.
    let mut b = flat(120.0);
    house(&mut b, V2::new(12.0, 40.0), V2::new(20.0, 48.0), None, true);
    let mut w = world_on(b.finish(), "pois: (1, 1), stuck_for: 20.0", "", |c| {
        RaidRoll::standard(c, 1)
    });
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    let hi = w.hero_slot(members[1]).unwrap();
    w.heroes[hi].pos = V2::new(16.0, 44.0);
    let at = w.heroes[hi].pos;
    put(
        &mut w,
        members[1],
        at.x,
        at.y,
        V2::new(1.0, 0.0),
        Gait::Still,
    );
    steps(&mut w, secs(10.0));
    let pinned = w.rivals.body(members[1]).map_or(0.0, |b| b.pinned);
    assert!(
        pinned > 5.0,
        "the sealed follower counts its time: {pinned}"
    );
    assert!(!w.rivals.parties[0].leaving, "the wait is no idleness");
    steps(&mut w, secs(20.0));
    assert!(w.hero(members[1]).is_none(), "the sealed one slipped out");
    assert!(w.hero(members[0]).is_some(), "its leader did not");
    assert_eq!(w.rivals.out, vec![members[1]]);
}

#[test]
fn a_new_leader_searches_its_own_leg_and_does_not_walk_the_old_ones() {
    let mut w = world_routes("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    steps(&mut w, secs(6.0));
    let old = w.rivals.parties[0]
        .journey
        .as_ref()
        .and_then(|j| j.travel.path())
        .expect("the old leader walks a leg");
    w.wound(members[0], 5000.0, None);
    w.step();
    let new = w.rivals.parties[0]
        .journey
        .as_ref()
        .and_then(|j| j.travel.path())
        .expect("the new leader has a leg");
    assert!(
        !Arc::ptr_eq(&old, &new),
        "the new leader walked the old leader's leg"
    );
}

#[test]
fn the_parties_are_served_from_a_start_that_moves_on() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    for n in 0..3u8 {
        let def = w.content().party("diggers").unwrap();
        w.add_party(
            def,
            n,
            0.1,
            V2::new(10.0, 40.0 + f32::from(n) * 10.0),
            u64::from(n),
        );
    }
    let mut firsts = Vec::new();
    for _ in 0..6 {
        w.step();
        firsts.push(w.rivals.first);
    }
    assert_eq!(firsts, vec![1, 2, 0, 1, 2, 0]);
}

#[test]
fn a_rival_does_not_dodge_into_another_blow_about_to_land() {
    // A second blow covers every spot it could step to, 40 yards round: the dodge is not taken.
    let (mut w, members) = smash_world("dodge: 1.0, pois: (1, 1)", 30.0);
    let mut dodged = false;
    for i in 0..secs(8.0) {
        let at = w.hero(members[0]).map_or(V2::new(30.0, 60.0), |h| h.pos);
        w.telegraphs.retain(|t| t.id < 9_000);
        w.telegraphs.push(crate::Telegraph {
            id: 9_000 + i as u32,
            enemy: 0,
            by: None,
            ability: 0,
            shape: crate::Shape::Circle { radius: 40.0 },
            at: crate::V3::new(at.x, at.y, 0.0),
            facing: V2::new(1.0, 0.0),
            left: 5.0,
            total: 5.0,
        });
        w.step();
        dodged |= w
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::Dodged { player } if members.contains(player)));
    }
    assert!(!dodged, "it dodged into the second blow");
}
