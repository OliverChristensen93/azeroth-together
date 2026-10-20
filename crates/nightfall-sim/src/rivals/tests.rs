//! Rivals in whole worlds on small hand-made maps: their content, the fight between heroes and
//! rivals, their minds, their bots, their arrivals and whole raids with them.

use std::sync::Arc;

use crate::heightmap::testmap::flat;
use crate::heightmap::HeightMap;
use crate::{
    CastPress, Content, ContentSources, ControlKind, Event, Gait, HeroPose, Holder, KitDef, Life, PlayerId, PlayerInput, Refusal,
    RaidRoll, RoutePlace, Side, UnitRef, World, RIVAL_BASE, TICK_HZ, V2,
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
    assert_eq!(w.hero(0).unwrap().fight.combo_hero, Some(a), "a combo point on the rival");
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
    assert!(damage_to(&events, UnitRef::Hero(1)).is_empty(), "a friend is spared");
    assert!(damage_to(&events, UnitRef::Hero(far)).is_empty(), "out of reach");
    assert!(events.iter().any(|e| matches!(e, Event::CastGo { hits, .. } if hits.contains(&UnitRef::Hero(a)))));
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
    assert!((w.hero(0).unwrap().speed - speed).abs() < 1e-3, "the slow is gone");
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
    assert!(damage_to(&events, UnitRef::Hero(a)).is_empty(), "{events:?}");
    assert_eq!(w.hero(a).unwrap().fight.held, None);
    assert_eq!(damage_to(&events, UnitRef::Hero(clear)).len(), 1, "in the open it lands");
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
