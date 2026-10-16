//! A busy run's world still fits the wire: what one snapshot of a crowded late run costs.

use std::sync::Arc;

use together_sim::messages::ServerMsg;
use together_sim::{Content, ContentSources, PlayerInput, World, TICK_HZ, V2};

fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/classes.ron"),
        abilities: include_str!("../../../content/abilities.ron"),
        enemies: include_str!("../../../content/enemies.ron"),
        zones: include_str!("../../../content/zones.ron"),
    })
    .unwrap()
}

#[test]
fn a_crowded_snapshot_is_small() {
    let c = Arc::new(shipped());
    let mut w = World::new(c, "elwynn", 7).unwrap();
    for p in 0..4 {
        w.add_hero(p, "mage").unwrap();
    }
    for step in 0..(TICK_HZ * 300) {
        for h in &mut w.heroes {
            h.hp = h.max_hp;
        }
        for p in 0..4 {
            let a = step as f32 * 0.01 + f32::from(p);
            w.set_input(
                p,
                PlayerInput {
                    move_dir: V2::new(a.cos(), a.sin()),
                    pick: Some(0),
                    ..Default::default()
                },
            );
        }
        w.step();
        w.drain_events();
    }
    let msg = ServerMsg::State {
        snapshot: w.snapshot(),
        events: Vec::new(),
    };
    let bytes = postcard::to_stdvec(&msg).unwrap().len();
    eprintln!(
        "{} enemies, {} gems: {bytes} bytes",
        w.enemies.len(),
        w.gems.len()
    );
    // About 20 bytes an enemy, so a busy run stays well under a megabit at 15 snapshots a second.
    assert!(
        bytes < 30 * w.enemies.len() + 2000,
        "{bytes} bytes for {} enemies",
        w.enemies.len()
    );
}
