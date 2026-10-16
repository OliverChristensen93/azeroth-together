//! A busy run's world still fits the wire: what one snapshot of a crowded late run costs.

use std::sync::Arc;

use together_net::ServerMsg;
use together_sim::{Content, ContentSources, Mode, PlayerInput, World, TICK_HZ, V2};

fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/classes.ron"),
        abilities: include_str!("../../../content/abilities.ron"),
        enemies: include_str!("../../../content/enemies.ron"),
        zones: include_str!("../../../content/zones.ron"),
    })
    .and_then(|c| c.with_extractions(include_str!("../../../content/extraction.ron")))
    .and_then(|c| c.with_spawns(include_str!("../../../content/spawns.ron")))
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

#[test]
fn a_hot_extraction_snapshot_keeps_to_the_horde_cap() {
    let c = Arc::new(shipped());
    let map = &c.extractions[0];
    let cap = map.max_enemies_per_hero as usize * 4;
    let home = map.safe_center();
    let mut w = World::new(c.clone(), "elwynn_extraction", 7).unwrap();
    for p in 0..4 {
        w.add_hero(p, "mage").unwrap();
    }
    // Ten minutes in, four heroes out in the forest at the hottest heat, casting nothing, so the
    // horde fills up to its cap.
    w.elapsed = 600.0;
    for h in &mut w.heroes {
        h.abilities.clear();
    }
    let spots = [(200.0, 0.0), (-200.0, 0.0), (0.0, 200.0), (0.0, -200.0)];
    for step in 0..(TICK_HZ * 25) {
        if let Mode::Extraction(x) = &mut w.mode {
            x.heat = 1000.0;
        }
        for (h, (dx, dy)) in w.heroes.iter_mut().zip(spots) {
            // A full horde could fell a hero within one step: these never fall.
            h.max_hp = 1e9;
            h.hp = 1e9;
            h.pos = home + V2::new(dx, dy);
        }
        for p in 0..4 {
            w.set_input(
                p,
                PlayerInput {
                    pick: Some(0),
                    ..Default::default()
                },
            );
        }
        w.step();
        w.drain_events();
        let horde = w
            .enemies
            .iter()
            .filter(|e| !e.elite && !e.boss && !e.is_wild())
            .count();
        // Hunter packs come on top of the cap, a few at a time.
        assert!(horde <= cap + 40, "step {step}: {horde} enemies, cap {cap}");
    }
    let msg = ServerMsg::State {
        snapshot: w.snapshot(),
        events: Vec::new(),
    };
    let bytes = postcard::to_stdvec(&msg).unwrap().len();
    eprintln!(
        "extraction at 5 skulls: {} enemies ({} creatures): {bytes} bytes",
        w.enemies.len(),
        w.enemies.iter().filter(|e| e.is_wild()).count()
    );
    assert!(w.enemies.len() > cap / 2, "the horde filled up: {}", w.enemies.len());
    assert!(
        bytes < 30 * w.enemies.len() + 2000,
        "{bytes} bytes for {} enemies",
        w.enemies.len()
    );
}
