//! The real `content/` files at the repository root: they parse, every key resolves, and a run on
//! each zone with each class plays out.

use std::sync::Arc;

use together_sim::{
    Content, ContentSources, Event, InteractableKind, PlayerInput, World, TICK_HZ, V2,
};

fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/classes.ron"),
        abilities: include_str!("../../../content/abilities.ron"),
        enemies: include_str!("../../../content/enemies.ron"),
        zones: include_str!("../../../content/zones.ron"),
    })
    .unwrap_or_else(|e| panic!("content/ is invalid: {e}"))
}

#[test]
fn shipped_content_is_valid() {
    let c = shipped();
    assert!(!c.classes.is_empty() && !c.zones.is_empty());
}

#[test]
fn every_zone_scatters_shrines_and_chests_inside_its_arena() {
    let c = Arc::new(shipped());
    for zone in &c.zones {
        let w = World::new(c.clone(), &zone.key, 11).unwrap();
        let center = w.center();
        assert!(
            w.interactables
                .iter()
                .any(|i| i.kind == InteractableKind::Chest),
            "{} has no chest",
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
