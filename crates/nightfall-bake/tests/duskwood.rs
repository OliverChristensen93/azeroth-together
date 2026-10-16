//! Darkshire, Duskwood's town, baked from the real data: its buildings' walls block sight and its
//! streets join up. Skipped without a WoW install (set `WOW_DATA`).

use std::sync::Arc;

use benilla_formats::{load_wmo_collision_tris, Chain};
use nightfall_bake::{bake, buildings_near, placement_point, BakeRequest, Footprint};
use nightfall_sim::{Bounds, HeightMapBuilder, Navigator, V2, V3};

/// Darkshire, near its middle: houses close together on flat ground.
const DARKSHIRE: V2 = V2::new(-10560.0, -1180.0);

#[test]
fn darkshire_bakes_with_walls_that_block_sight_and_streets_that_join() {
    let data = benilla_formats::wow_data_or_skip!();
    let near = buildings_near(&data, 0, DARKSHIRE, 300.0).expect("Duskwood's tiles");
    // A house, not a tent or a wagon the map files as a WMO, so a wall is what stands there.
    let (distance, building) = near
        .iter()
        .find(|(_, w)| w.model.to_ascii_lowercase().contains("\\buildings\\"))
        .expect("a house within 300 yd of Darkshire");
    eprintln!("the building: {} at {distance:.0} yd", building.model);
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
    let made = bake(&req, &|_| {}).expect("the bake");
    assert!(
        made.failures.is_empty(),
        "unreadable files: {:?}",
        made.failures
    );
    let map = made.map;
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

    // The building's own collision is in the map. The same model placed alone on the same grid is
    // the reference: every column where it stands a wall (2.5 yd or more of solid) must hold that
    // solid in the baked map too. Trees and rocks cannot account for it, since the reference has
    // none of them and they would have to coincide with the walls column for column.
    let mut chain = Chain::open(&data).expect("the data");
    let mesh =
        load_wmo_collision_tris(&mut chain, &building.model).expect("the building's collision");
    let mut fp = Footprint::new(g);
    for t in mesh.indices.as_chunks::<3>().0 {
        let at = |i: u32| {
            placement_point(
                building.position,
                building.rotation,
                1.0,
                mesh.positions[i as usize],
            )
        };
        fp.add_triangle([at(t[0]), at(t[1]), at(t[2])]);
    }
    let mut alone = HeightMapBuilder::new(g);
    fp.flush_into(&mut alone, false);
    let alone = alone.finish();
    let mut walls = Vec::new();
    for (ix, iy) in columns() {
        let col = g.index(ix, iy);
        for s in alone.spans_at(col).as_slice() {
            if s.hi - s.lo >= 2.5 {
                walls.push((col, *s));
            }
        }
    }
    assert!(
        walls.len() >= 50,
        "{} has only {} wall columns in the reference",
        building.model,
        walls.len()
    );
    let held = walls
        .iter()
        .filter(|(col, w)| {
            map.spans_at(*col)
                .as_slice()
                .iter()
                .any(|s| s.lo <= w.lo + 0.05 && s.hi >= w.hi - 0.05)
        })
        .count();
    eprintln!(
        "{held} of {} wall columns of the building are in the map",
        walls.len()
    );
    assert!(
        held * 100 >= walls.len() * 98,
        "only {held} of {} wall columns are in the baked map",
        walls.len()
    );

    // Sight through a wall column, at the height of its solid, is blocked.
    let (col, wall) = walls[walls.len() / 2];
    let w = g.center(col as u32 % g.nx, col as u32 / g.nx);
    let z = (wall.lo + wall.hi) / 2.0;
    let blocked = [V2::new(1.0, 0.0), V2::new(0.0, 1.0)]
        .into_iter()
        .any(|d| !map.line_of_sight(V3::at(w - d * 3.0, z), V3::at(w + d * 3.0, z)));
    assert!(blocked, "sight passes straight through a wall at {w:?}");

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

#[test]
fn a_prop_whose_model_the_data_lacks_is_an_error() {
    let data = benilla_formats::wow_data_or_skip!();
    let bounds = Bounds::new(vec![
        DARKSHIRE + V2::new(-20.0, -20.0),
        DARKSHIRE + V2::new(20.0, -20.0),
        DARKSHIRE + V2::new(20.0, 20.0),
        DARKSHIRE + V2::new(-20.0, 20.0),
    ])
    .unwrap();
    let props = [nightfall_bake::PropPlacement {
        model: "Nightfall/NoSuchProp.mdx".into(),
        position: [DARKSHIRE.x, DARKSHIRE.y, 0.0],
        rotation: [0.0; 3],
        scale: 1.0,
    }];
    let req = BakeRequest {
        data_dir: &data,
        map_id: 0,
        key: "prop-test",
        bounds: &bounds,
        props: &props,
    };
    let err = bake(&req, &|_| {})
        .err()
        .expect("a missing prop model must fail the bake");
    assert!(format!("{err:#}").contains("NoSuchProp"), "{err:#}");
}
