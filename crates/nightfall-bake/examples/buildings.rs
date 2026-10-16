//! Lists the buildings around a point, nearest first, with where each stands: for placing
//! Nightfall's enemies at real houses (`content/nightfall/spawns.ron`).
//! `cargo run -p nightfall-bake --example buildings -- <x> <y> [radius]`

use nightfall_bake::buildings_near;
use nightfall_sim::V2;

fn main() {
    let args: Vec<f32> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let (Some(&x), Some(&y)) = (args.first(), args.get(1)) else {
        eprintln!("usage: buildings <x> <y> [radius]");
        std::process::exit(2);
    };
    let radius = args.get(2).copied().unwrap_or(200.0);
    let Some(data) = benilla_formats::wow_data() else {
        eprintln!("no WoW data folder: set WOW_DATA");
        std::process::exit(1);
    };
    let found = buildings_near(&data, 0, V2::new(x, y), radius).unwrap_or_else(|e| {
        eprintln!("{e:#}");
        std::process::exit(1);
    });
    for (d, w) in found {
        println!(
            "{d:>6.1} yd  ({:.1}, {:.1}, {:.1})  {}",
            w.position[0], w.position[1], w.position[2], w.model
        );
    }
}
