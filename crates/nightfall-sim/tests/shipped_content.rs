//! The content Nightfall ships (`content/nightfall/`) loads and holds what the game expects.

use nightfall_sim::{Content, ContentSources, V2};

pub fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/nightfall/classes.ron"),
        map: include_str!("../../../content/nightfall/map.ron"),
        rules: include_str!("../../../content/nightfall/rules.ron"),
        enemies: include_str!("../../../content/nightfall/enemies.ron"),
        spawns: include_str!("../../../content/nightfall/spawns.ron"),
        abilities: include_str!("../../../content/nightfall/abilities.ron"),
        items: include_str!("../../../content/nightfall/items.ron"),
        loot: include_str!("../../../content/nightfall/loot.ron"),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_shipped_content_loads_with_rogue_and_mage_in_duskwood() {
    let c = shipped();
    assert!(c.class("rogue").is_some());
    assert!(c.class("mage").is_some());
    assert_eq!(c.map.key, "duskwood");
    assert_eq!(c.map.map, 0);
    for e in &c.map.entries {
        assert!(c.map.bounds.contains(V2::new(e[0], e[1])));
    }
}

#[test]
fn duskwoods_creatures_are_placed_inside_it() {
    let c = shipped();
    assert!(c.enemy("nightbane_worgen").is_some());
    assert!(!c.spawns.is_empty());
    assert_eq!(c.rules.world.share_sight_within, 10.0);
}

#[test]
fn both_classes_play_in_duskwood() {
    use std::sync::Arc;

    use nightfall_sim::{HeroPose, PlayerInput, World};

    let content = Arc::new(shipped());
    let mut w = World::new(content.clone());
    w.add_hero(0, "rogue", "Valeera", 1, 0).unwrap();
    w.add_hero(1, "mage", "Jaina", 1, 1).unwrap();
    let far = V2::new(0.0, 0.0);
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: far,
                facing: V2::new(1.0, 0.0),
                z: 36.5,
            }),
            ..Default::default()
        },
    );
    w.step();
    let h = w.hero(0).unwrap();
    assert!(content.map.bounds.contains(h.pos) || content.map.bounds.clamp(h.pos) == h.pos);
    assert_ne!(h.pos, far, "a pose far off the map was kept inside it");
}

#[test]
fn the_shipped_senses_are_the_specs_and_the_rogue_stealths() {
    let c = shipped();
    let s = &c.rules.senses;
    assert_eq!(s.notice_range, 35.0);
    assert_eq!(s.stealth_speed, 0.7);
    assert_eq!(s.visibility.sneak_still, 8.0);
    assert_eq!(s.awareness.lose_after, 5.0);
    assert!(c.classes[c.class("rogue").unwrap()].stealth);
    assert!(!c.classes[c.class("mage").unwrap()].stealth);
    assert!(c
        .spawns
        .iter()
        .any(|sp| matches!(sp.behaviour, nightfall_sim::Behaviour::Sleeper { .. })));
}

#[test]
fn the_shipped_kits_are_on_their_bars_and_enemies_fight_back() {
    use nightfall_sim::{Aim, ClassDef, Effect, Moving};

    let c = shipped();
    let rogue = &c.classes[c.class("rogue").unwrap()];
    let mage = &c.classes[c.class("mage").unwrap()];
    let on_bar =
        |class: &ClassDef, key: &str| class.kit.iter().any(|&(_, a)| c.abilities[a].key == key);
    for key in [
        "stealth",
        "sap",
        "cheap_shot",
        "garrote",
        "ambush",
        "sinister_strike",
        "eviscerate",
        "gouge",
        "sprint",
        "distract",
        "vanish",
        "throw_stone",
        "eat_and_drink",
    ] {
        assert!(on_bar(rogue, key), "rogue: {key}");
    }
    for key in [
        "frostbolt",
        "fireball",
        "fire_blast",
        "frost_nova",
        "arcane_missiles",
        "polymorph",
        "blink",
        "conjure_water",
        "throw_stone",
        "eat_and_drink",
    ] {
        assert!(on_bar(mage, key), "mage: {key}");
    }
    let a = |key: &str| &c.abilities[c.ability(key).unwrap()];
    assert_eq!(a("frostbolt").cast, 2.5);
    assert_eq!(a("frostbolt").moving, Moving::Slowed);
    assert_eq!(a("fireball").moving, Moving::Rooted);
    assert_eq!(a("arcane_missiles").moving, Moving::Rooted);
    assert_eq!(a("blink").cooldown, 15.0);
    assert_eq!(a("throw_stone").range, 25.0);
    assert_eq!(a("throw_stone").cooldown, 20.0);
    assert_eq!(a("throw_stone").aim, Aim::Ground);
    assert!(a("vanish").has(|e| matches!(e, Effect::Vanish)));
    assert_eq!(c.rules.combat.downed_for, 45.0);
    assert_eq!(rogue.power_max, 100.0);
    assert!(rogue.swing.is_some());
    assert!(mage.power_max > 0.0);
    let weaver = &c.enemies[c.enemy("nightbane_shadow_weaver").unwrap()];
    assert!(!weaver.kit.is_empty());
    assert!(c.enemies.iter().filter(|e| e.elite).count() >= 2);
    for e in c.enemies.iter().filter(|e| e.elite) {
        assert!(
            e.kit
                .iter()
                .any(|&(m, _)| c.abilities[m].telegraph.is_some()),
            "{}",
            e.key
        );
    }
}

#[test]
fn the_shipped_content_has_its_items_loot_exits_and_kits() {
    let c = shipped();
    assert!(c.items.len() >= 20);
    assert!(c.conjured.is_some());
    assert!(c.map.exits.len() >= 3);
    assert!(c.map.exits.iter().any(|e| e.late));
    assert!(!c.loot.containers.is_empty());
    assert!(!c.loot.bodies.is_empty());
    for class in &c.classes {
        assert!(!class.free_items.is_empty(), "{} has a free kit", class.key);
    }
}
