//! The shipped map's ways out can be walked to: from every entry, a path over the real baked
//! ground reaches each exit's ring. Skipped without a WoW install (set `WOW_DATA`).
//!
//! The bake is read from the cache the game keeps (`target/nf-home/nightfall/` of the workspace,
//! or `NIGHTFALL_BAKE_CACHE`), and made, once, if it is not there: a first run takes minutes.

use std::path::PathBuf;
use std::sync::Arc;

use nightfall_bake::{load_or_bake, BakeRequest};
use nightfall_sim::{Content, ContentSources, Navigator, V2, V3};

/// Floors one search may look at: the whole map, if it must.
const BUDGET: usize = 50_000_000;

fn shipped() -> Content {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/nightfall");
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let (classes, map, rules) = (read("classes.ron"), read("map.ron"), read("rules.ron"));
    let (enemies, spawns, abilities) = (
        read("enemies.ron"),
        read("spawns.ron"),
        read("abilities.ron"),
    );
    let (items, loot, monsters) = (read("items.ron"), read("loot.ron"), read("monsters.ron"));
    Content::from_ron(ContentSources {
        classes: &classes,
        map: &map,
        rules: &rules,
        enemies: &enemies,
        spawns: &spawns,
        abilities: &abilities,
        items: &items,
        loot: &loot,
        monsters: &monsters,
    })
    .expect("the shipped content parses")
}

#[test]
fn every_exit_can_be_walked_to_from_every_entry() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = &content.map;
    let cache = std::env::var_os("NIGHTFALL_BAKE_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/nf-home/nightfall")
        });
    let req = BakeRequest {
        data_dir: &data,
        map_id: map.map,
        key: &map.key,
        bounds: &map.bounds,
        props: &[],
    };
    let baked = load_or_bake(&req, Some(&cache), &|_| {}).expect("the bake");
    // A map missing files, or one that could not be cached, would make the paths below mean less.
    assert!(
        baked.failures.is_empty(),
        "the bake could not read: {:#?}",
        baked.failures
    );
    assert!(
        baked.cache_error.is_none(),
        "the bake could not be cached: {:?}",
        baked.cache_error
    );
    eprintln!(
        "the map came {} ({} columns hold more than terrain)",
        if baked.from_cache {
            "from the cache"
        } else {
            "from a bake"
        },
        baked.map.complex_columns()
    );
    let mut nav = Navigator::new(Arc::new(baked.map));
    let mut unreachable = Vec::new();
    for exit in &map.exits {
        // The ring's middle stands on baked ground, or no ring could be drawn there.
        let Some(z) = nav.map().ground_at(exit.at) else {
            unreachable.push(format!("{}: no ground at {:?}", exit.name, exit.at));
            continue;
        };
        let goal = V3::at(exit.at, z);
        for (i, e) in map.entries.iter().enumerate() {
            let from = V2::new(e[0], e[1]);
            let Some(fz) = nav.map().ground_at(from) else {
                unreachable.push(format!("entry {i}: no ground at {from:?}"));
                continue;
            };
            match nav.find_path(V3::at(from, fz), goal, false, BUDGET) {
                Some(path) => {
                    let end = path.last().expect("a path has an end");
                    let short = V2::new(end.x, end.y).distance(exit.at);
                    eprintln!(
                        "entry {i} to {}: {} points, ends {short:.1} yd from its middle",
                        exit.name,
                        path.len()
                    );
                    if short > exit.radius {
                        unreachable.push(format!(
                            "{}: the path from entry {i} ends {short:.1} yd short of the ring",
                            exit.name
                        ));
                    }
                }
                None => unreachable.push(format!("{}: no path from entry {i}", exit.name)),
            }
        }
    }
    assert!(
        unreachable.is_empty(),
        "unreachable exits: {unreachable:#?}"
    );
}
