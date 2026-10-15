//! The real `content/` files at the repository root: they parse, every key resolves, and a run on
//! each zone with each class plays out.

use std::sync::Arc;

use together_sim::{
    Content, ContentSources, Event, InteractableKind, Item, Mode, PlayerInput, World, TICK_HZ, V2,
};

fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/classes.ron"),
        abilities: include_str!("../../../content/abilities.ron"),
        enemies: include_str!("../../../content/enemies.ron"),
        zones: include_str!("../../../content/zones.ron"),
    })
    .and_then(|c| c.with_rules(include_str!("../../../content/rules.ron")))
    .and_then(|c| c.with_extractions(include_str!("../../../content/extraction.ron")))
    .and_then(|c| c.with_spawns(include_str!("../../../content/spawns.ron")))
    .unwrap_or_else(|e| panic!("content/ is invalid: {e}"))
}

#[test]
fn shipped_content_is_valid() {
    let c = shipped();
    assert!(!c.classes.is_empty() && !c.zones.is_empty());
}

#[test]
fn every_item_that_strikes_has_its_ability() {
    let c = shipped();
    for item in Item::ALL {
        if let Some(key) = item.proc() {
            assert!(c.ability(key).is_some(), "{key} is missing from abilities.ron");
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
        assert_eq!(w.interactables.len() as u32, zone.shrines_at_once, "{}", zone.key);
        for i in &w.interactables {
            assert!(i.pos.distance(center) <= zone.leash, "{} {:?}", zone.key, i);
        }
    }
}

#[test]
fn every_class_survives_a_minute_in_every_zone_and_kills_something() {
    let c = Arc::new(shipped());
    // Extraction zones start their heroes in a safe ring where nothing spawns; they have their
    // own run below.
    for zone in c.zones.iter().filter(|z| z.extraction_ix.is_none()) {
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
fn an_extraction_run_in_elwynn_heats_up_out_in_the_forest_and_kills_something() {
    let c = Arc::new(shipped());
    let zone = c.zone("elwynn_extraction").expect("the zone");
    let map = c.zones[zone].extraction_ix.expect("its map");
    assert_eq!(c.extractions[map].key, "elwynn");
    for class in &c.classes {
        let mut w = World::new(c.clone(), "elwynn_extraction", 9).unwrap();
        w.add_hero(0, &class.key).unwrap();
        assert!(matches!(w.mode, Mode::Extraction(_)));
        // North out of Goldshire for 20 seconds, then circle out in the forest.
        for step in 0..(TICK_HZ * 80) {
            let a = step as f32 * 0.01;
            let move_dir = if step < TICK_HZ * 20 {
                V2::new(1.0, 0.0)
            } else {
                V2::new(a.cos(), a.sin())
            };
            w.set_input(
                0,
                PlayerInput {
                    move_dir,
                    pick: Some(0),
                    ..Default::default()
                },
            );
            w.step();
        }
        assert!(w.kills > 0, "{} killed nothing", class.key);
        assert!(w.heat_of(0) > 0.0, "{} stayed cool", class.key);
    }
}

#[test]
fn elwynns_forest_is_full_but_goldshire_is_clear() {
    let c = shipped();
    let map = &c.extractions[0];
    assert!(map.spawns.len() > 600, "{}", map.spawns.len());
    let bounds = map.bounds();
    for s in &map.spawns {
        assert!(bounds.inside(s.at(), 0.0), "outside the outline: {:?}", s.pos);
        assert!(!map.in_margin(s.at()), "in Goldshire's margin: {:?}", s.pos);
    }
}
