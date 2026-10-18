//! Bakes the shipped Duskwood bounds and says how long it took and how big the map is: the check
//! that a first launch's bake stays under a minute (spec §4.1). Writes nothing.
//! `cargo run --release -p nightfall-bake --example bake_duskwood` (with `WOW_DATA` set if the
//! data is not beside the project).

use std::cell::Cell;
use std::time::Instant;

use nightfall_bake::{bake, BakeRequest};
use nightfall_sim::{Content, ContentSources};

fn main() {
    let content = Content::from_ron(ContentSources {
        classes: include_str!("../../../content/nightfall/classes.ron"),
        map: include_str!("../../../content/nightfall/map.ron"),
        rules: include_str!("../../../content/nightfall/rules.ron"),
        enemies: include_str!("../../../content/nightfall/enemies.ron"),
        spawns: include_str!("../../../content/nightfall/spawns.ron"),
        abilities: include_str!("../../../content/nightfall/abilities.ron"),
        items: include_str!("../../../content/nightfall/items.ron"),
        loot: include_str!("../../../content/nightfall/loot.ron"),
    })
    .expect("the shipped content");
    let Some(data) = benilla_formats::wow_data() else {
        eprintln!("no WoW data folder: set WOW_DATA");
        std::process::exit(1);
    };
    let req = BakeRequest {
        data_dir: &data,
        map_id: content.map.map,
        key: &content.map.key,
        bounds: &content.map.bounds,
        props: &[],
    };
    let started = Instant::now();
    let shown = Cell::new(-1);
    let made = bake(&req, &|f| {
        let tenth = (f * 10.0) as i32;
        if tenth != shown.get() {
            shown.set(tenth);
            eprintln!("{:>4.0}%  {:.1?}", f * 100.0, started.elapsed());
        }
    })
    .unwrap_or_else(|e| {
        eprintln!("{e:#}");
        std::process::exit(1);
    });
    for f in &made.failures {
        eprintln!("unreadable: {f}");
    }
    let map = made.map;
    println!(
        "{:?}: baked in {:.1?}; {} columns hold more than terrain; {} MB as bytes",
        map.grid(),
        started.elapsed(),
        map.complex_columns(),
        map.to_bytes().len() / 1_000_000
    );
}
