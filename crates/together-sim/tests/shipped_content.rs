//! The real `content/` files at the repository root: they parse, every key resolves, and a run on
//! each zone with each class plays out.

use std::sync::Arc;

use together_sim::{
    describe, CastSpec, Content, ContentSources, Event, InteractableKind, Item, PlayerInput, World,
    TICK_HZ, V2,
};

fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/classes.ron"),
        abilities: include_str!("../../../content/abilities.ron"),
        enemies: include_str!("../../../content/enemies.ron"),
        zones: include_str!("../../../content/zones.ron"),
    })
    .and_then(|c| c.with_rules(include_str!("../../../content/rules.ron")))
    .and_then(|c| c.with_races(include_str!("../../../content/races.ron")))
    .unwrap_or_else(|e| panic!("content/ is invalid: {e}"))
}

#[test]
fn every_classic_race_has_its_passives_and_a_dwarf_finds_more_gold() {
    let c = shipped();
    for race in 1..=8u8 {
        let def = c.race(race).unwrap_or_else(|| panic!("race {race} has no passives"));
        assert!(!def.passives.is_empty(), "{}", def.name);
    }
    assert!(c.race(3).unwrap().treasure());
    let mut w = World::new(Arc::new(c), "elwynn", 1).unwrap();
    w.add_hero_as(0, "warrior", "Dwarf", 3, 0).unwrap();
    let hero = w.hero(0).unwrap();
    assert!(hero.stats.get(together_sim::Stat::GoldGain) > 0.1);
    assert_eq!(hero.hp, hero.max_hp);
}

#[test]
fn shipped_content_is_valid() {
    let c = shipped();
    assert!(!c.classes.is_empty() && !c.zones.is_empty());
}

#[test]
fn every_description_fills_all_its_numbers() {
    let c = shipped();
    for (i, def) in c.abilities.iter().enumerate() {
        let text = describe(def, &CastSpec::base(i, def), 100.0);
        assert!(
            !text.contains('{') && !text.contains('}'),
            "{}: a placeholder is left in {text:?}",
            def.key
        );
    }
}

#[test]
fn every_item_that_strikes_has_its_ability() {
    let c = shipped();
    for item in Item::ALL {
        if let Some(key) = item.proc() {
            assert!(
                c.ability(key).is_some(),
                "{key} is missing from abilities.ron"
            );
        }
    }
}

#[test]
fn every_zone_starts_with_its_few_shrines_inside_its_arena() {
    let c = Arc::new(shipped());
    for zone in &c.zones {
        assert!(
            zone.interactables
                .iter()
                .any(|p| p.kind == InteractableKind::Chest),
            "{} has no chests",
            zone.key
        );
        let w = World::new(c.clone(), &zone.key, 11).unwrap();
        let center = w.center();
        let pots = w
            .interactables
            .iter()
            .filter(|i| i.kind == InteractableKind::Pot)
            .count() as u32;
        assert_eq!(pots, zone.pots, "{}", zone.key);
        assert_eq!(
            w.interactables.len() as u32 - pots,
            zone.shrines_at_once,
            "{}",
            zone.key
        );
        for i in &w.interactables {
            assert!(i.pos.distance(center) <= zone.leash, "{} {:?}", zone.key, i);
        }
    }
}

#[test]
fn every_class_survives_a_minute_in_every_zone_and_kills_something() {
    let c = Arc::new(shipped());
    for zone in &c.zones {
        for class in &c.classes {
            let mut w = World::new(c.clone(), &zone.key, 3).unwrap();
            w.add_hero(0, &class.key).unwrap();
            // Circle slowly and always take the first card, like a lazy player.
            for step in 0..(TICK_HZ * 60) {
                let a = step as f32 * 0.01;
                w.set_input(
                    0,
                    PlayerInput {
                        move_dir: V2::new(a.cos(), a.sin()),
                        pick: Some(0),
                        jump: false,
                        pose: None,
                        ..Default::default()
                    },
                );
                w.step();
            }
            let events = w.drain_events();
            assert!(w.kills > 0, "{} in {} killed nothing", class.key, zone.key);
            assert!(events.iter().any(|e| matches!(e, Event::Cast { .. })));
        }
    }
}

#[test]
fn every_class_has_twelve_spells_with_capstones_and_a_defensive_button() {
    let c = shipped();
    for class in &c.classes {
        assert_eq!(class.pool.len(), 12, "{}", class.key);
        assert!(
            class.defensive_ix.is_some(),
            "{} has no defensive",
            class.key
        );
        for key in &class.pool {
            assert_eq!(
                together_sim::Augment::capstones_of(key).len(),
                3,
                "{}: {key}",
                class.key
            );
        }
    }
}
