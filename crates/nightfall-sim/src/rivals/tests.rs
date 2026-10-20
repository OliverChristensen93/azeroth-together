//! Rivals in whole worlds on small hand-made maps: their content, the fight between heroes and
//! rivals, their minds, their bots, their arrivals and whole raids with them.

use crate::{Content, ContentSources, KitDef, RoutePlace, V2};

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

