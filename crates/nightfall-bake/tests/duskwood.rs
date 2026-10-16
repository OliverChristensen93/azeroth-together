//! Darkshire, Duskwood's town, baked from the real data: its buildings' walls block sight and its
//! streets join up. Skipped without a WoW install (set `WOW_DATA`).

use std::sync::Arc;

use nightfall_bake::{bake, buildings_near, BakeRequest};
use nightfall_sim::{Bounds, Navigator, V2, V3};

/// Darkshire, near its middle: houses close together on flat ground.
const DARKSHIRE: V2 = V2::new(-10560.0, -1180.0);

#[test]
fn darkshire_bakes_with_walls_that_block_sight_and_streets_that_join() {
    let data = benilla_formats::wow_data_or_skip!();
    let near = buildings_near(&data, 0, DARKSHIRE, 300.0).expect("Duskwood's tiles");
    let (_, building) = near.first().expect("a building within 300 yd of Darkshire");
    let c = V2::new(building.position[0], building.position[1]);
    let half = 80.0;
    let bounds = Bounds::new(vec![
        c + V2::new(-half, -half),
        c + V2::new(half, -half),
        c + V2::new(half, half),
        c + V2::new(-half, half),
    ])
    .unwrap();
    let req = BakeRequest {
        data_dir: &data,
        map_id: 0,
        key: "darkshire-test",
        bounds: &bounds,
        props: &[],
    };
    let started = std::time::Instant::now();
    let map = bake(&req, &|_| {}).expect("the bake");
    eprintln!(
        "baked {:?} in {:.1?}: {} columns hold more than terrain",
        map.grid(),
        started.elapsed(),
        map.complex_columns()
    );
    let g = map.grid();
    let columns = || (0..g.ny).flat_map(move |iy| (0..g.nx).map(move |ix| (ix, iy)));

    // Terrain under nearly all of it.
    let with_ground = columns()
        .filter(|&(ix, iy)| map.ground_at(g.center(ix, iy)).is_some())
        .count();
    assert!(
        with_ground * 100 >= g.columns() * 95,
        "{with_ground} of {} columns have ground",
        g.columns()
    );

    // The building rises out of the ground: a wall at least 2.5 yd tall within 30 yd of its
    // origin, and sight across its foot is blocked.
    let wall = columns()
        .map(|(ix, iy)| (g.index(ix, iy), g.center(ix, iy)))
        .filter(|(_, p)| p.distance(c) <= 30.0)
        .find(|&(col, p)| {
            let Some(ground) = map.ground_at(p) else {
                return false;
            };
            map.spans_at(col)
                .as_slice()
                .iter()
                .any(|s| s.lo <= ground + 0.5 && s.hi >= ground + 2.5)
        });
    let (_, w) = wall.expect("a wall near the building");
    let foot = map.ground_at(w).unwrap() + 1.5;
    assert!(!map.line_of_sight(
        V3::at(w + V2::new(-3.0, 0.0), foot),
        V3::at(w + V2::new(3.0, 0.0), foot)
    ));

    // The streets join: from some side of the building to the opposite one there is a path,
    // every leg of it walkable.
    let mut nav = Navigator::new(Arc::new(map));
    let mut joined = false;
    for k in 0..36 {
        let angle = (k as f32 * 10.0).to_radians();
        let a = c + V2::from_angle(angle, 30.0);
        let b = c + V2::from_angle(angle + std::f32::consts::PI, 30.0);
        let (Some(za), Some(zb)) = (nav.map().ground_at(a), nav.map().ground_at(b)) else {
            continue;
        };
        if let Some(path) = nav.find_path(V3::at(a, za), V3::at(b, zb), false, 40_000) {
            assert!(path.windows(2).all(|l| nav.map().walk_line(l[0], l[1])));
            joined = true;
            break;
        }
    }
    assert!(joined, "no path across the building from any side");
}
