//! The bake itself: WoW's files in, a [`HeightMap`] out. Every tile under the bounds gives its
//! terrain (impassable chunks blocked); every building and doodad placed on those tiles gives its
//! collision, each model read once however often it is placed. Columns outside the bounds are
//! blocked, so no enemy walks out of the run.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use benilla_formats::{
    load_m2_collision_hull, load_map_catalog, load_tile_mesh, load_wmo_collision_tris, Chain,
    CollisionMesh, Doodad, TileMesh, WmoInstance, CHUNK_SIZE, TILE_SIZE,
};
use nightfall_sim::{Bounds, Grid, HeightMap, HeightMapBuilder, CELL, V2};

use crate::place::placement_point;
use crate::raster::Footprint;

/// How far outside the bounds a building's origin may stand and still reach in: Duskwood's
/// largest buildings span less than this.
const WMO_REACH: f32 = 250.0;
/// The same for a doodad (a tree, a cart, a rock).
const DOODAD_REACH: f32 = 30.0;

/// A model placed by Nightfall's own content (spec §9.3) rather than by the map.
#[derive(Clone, Debug, PartialEq)]
pub struct PropPlacement {
    /// The M2 path, as the map's doodads name theirs.
    pub model: String,
    /// WoW `(x, y, z)`.
    pub position: [f32; 3],
    /// MDDF-style Euler rotation, degrees.
    pub rotation: [f32; 3],
    pub scale: f32,
}

/// What to bake.
pub struct BakeRequest<'a> {
    /// WoW's `Data` folder.
    pub data_dir: &'a Path,
    /// `Map.dbc` id.
    pub map_id: u32,
    /// The map's content key (`"duskwood"`), which names its cache file.
    pub key: &'a str,
    /// The run's edge: the bake covers its bounding rectangle and blocks the columns outside it.
    pub bounds: &'a Bounds,
    pub props: &'a [PropPlacement],
}

/// The grid over the bounds' rectangle, its corner on a multiple of [`CELL`].
pub(crate) fn grid_for(bounds: &Bounds) -> Grid {
    let (mut lo, mut hi) = (
        V2::new(f32::INFINITY, f32::INFINITY),
        V2::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    );
    for p in bounds.points() {
        lo = V2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = V2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let origin = V2::new((lo.x / CELL).floor() * CELL, (lo.y / CELL).floor() * CELL);
    Grid {
        origin,
        cell: CELL,
        nx: ((hi.x - origin.x) / CELL).ceil().max(1.0) as u32,
        ny: ((hi.y - origin.y) / CELL).ceil().max(1.0) as u32,
    }
}

/// The far corner of a grid.
fn far_corner(g: &Grid) -> V2 {
    V2::new(
        g.origin.x + g.nx as f32 * g.cell,
        g.origin.y + g.ny as f32 * g.cell,
    )
}

/// The tiles `(tile_x, tile_y)` under the grid, as `load_tile_mesh` names them: `tile_x` counts
/// along `y` and `tile_y` along `x`, both from the map's north-west corner.
pub(crate) fn tiles_under(g: &Grid) -> Vec<(u32, u32)> {
    let far = far_corner(g);
    let tile = |w: f32| {
        ((32.0 * TILE_SIZE - w) / TILE_SIZE)
            .floor()
            .clamp(0.0, 63.0) as u32
    };
    let (tx0, tx1) = (tile(far.y), tile(g.origin.y));
    let (ty0, ty1) = (tile(far.x), tile(g.origin.x));
    (ty0..=ty1)
        .flat_map(|ty| (tx0..=tx1).map(move |tx| (tx, ty)))
        .collect()
}

fn map_directory(chain: &mut Chain, map_id: u32) -> Result<String> {
    let catalog = load_map_catalog(chain).context("reading Map.dbc")?;
    catalog
        .directory(map_id)
        .map(str::to_owned)
        .with_context(|| format!("Map.dbc has no map {map_id}"))
}

/// Every tile under the grid that the data has; a missing tile (a zone the trimmed data leaves
/// out) is skipped, and none at all is an error.
fn load_tiles(chain: &mut Chain, dir: &str, g: &Grid) -> Result<Vec<TileMesh>> {
    let mut tiles = Vec::new();
    for (tx, ty) in tiles_under(g) {
        if let Ok(tile) = load_tile_mesh(chain, dir, tx, ty) {
            tiles.push(tile);
        }
    }
    if tiles.is_empty() {
        bail!("no terrain of {dir} under the run's bounds");
    }
    Ok(tiles)
}

/// Each column's terrain height from the tile, and every column of an impassable chunk blocked:
/// the engine fences those chunks off, and here a unit simply never enters one.
fn add_terrain(b: &mut HeightMapBuilder, tile: &TileMesh) {
    let g = b.grid();
    for chunk in &tile.chunks {
        let Some(&nw) = chunk.positions.first() else {
            continue;
        };
        // A chunk runs south (−x) and east (−y) from its north-west corner.
        let range = |north: f32, origin: f32, n: u32| {
            let first = ((north - CHUNK_SIZE - origin) / g.cell).floor().max(0.0) as i64;
            let last = ((north - origin) / g.cell).floor().min(n as f32 - 1.0) as i64;
            first..=last
        };
        for iy in range(nw[1], g.origin.y, g.ny) {
            for ix in range(nw[0], g.origin.x, g.nx) {
                let c = g.center(ix as u32, iy as u32);
                let Some(z) = chunk.height_at([c.x, c.y, 0.0]) else {
                    continue;
                };
                let col = g.index(ix as u32, iy as u32);
                b.set_ground(col, z);
                if chunk.impassable {
                    b.block(col);
                }
            }
        }
    }
}

/// The buildings and doodads that can reach the grid, each once (a model on a tile seam is
/// listed by both tiles), in a fixed order.
fn placements(tiles: &[TileMesh], g: &Grid) -> (Vec<WmoInstance>, Vec<Doodad>) {
    let far = far_corner(g);
    let near = |p: [f32; 3], reach: f32| {
        p[0] >= g.origin.x - reach
            && p[0] <= far.x + reach
            && p[1] >= g.origin.y - reach
            && p[1] <= far.y + reach
    };
    let mut seen = HashSet::new();
    let mut wmos: Vec<WmoInstance> = Vec::new();
    for t in tiles {
        for w in &t.wmos {
            if near(w.position, WMO_REACH) && seen.insert(w.unique_id) {
                wmos.push(w.clone());
            }
        }
    }
    let mut seen = HashSet::new();
    let mut doodads: Vec<Doodad> = Vec::new();
    for t in tiles {
        for d in &t.doodads {
            if near(d.position, DOODAD_REACH) && seen.insert(d.unique_id) {
                doodads.push(d.clone());
            }
        }
    }
    wmos.sort_by_key(|w| w.unique_id);
    doodads.sort_by_key(|d| d.unique_id);
    (wmos, doodads)
}

/// A placed model's collision triangles into the footprint.
fn add_model(
    fp: &mut Footprint,
    mesh: &CollisionMesh,
    position: [f32; 3],
    rotation: [f32; 3],
    scale: f32,
) {
    for t in mesh.indices.chunks_exact(3) {
        let v = |i: u32| {
            mesh.positions
                .get(i as usize)
                .map(|&p| placement_point(position, rotation, scale, p))
        };
        let (Some(a), Some(b), Some(c)) = (v(t[0]), v(t[1]), v(t[2])) else {
            continue;
        };
        fp.add_triangle([a, b, c]);
    }
}

/// Bake the request's map from the data, telling `progress` how far it has got (0 to 1).
pub fn bake(req: &BakeRequest, progress: &dyn Fn(f32)) -> Result<HeightMap> {
    progress(0.0);
    let mut chain = Chain::open(req.data_dir)
        .with_context(|| format!("opening WoW data at {}", req.data_dir.display()))?;
    let dir = map_directory(&mut chain, req.map_id)?;
    let grid = grid_for(req.bounds);
    let mut b = HeightMapBuilder::new(grid);
    let tiles = load_tiles(&mut chain, &dir, &grid)?;
    progress(0.1);
    for t in &tiles {
        add_terrain(&mut b, t);
    }
    progress(0.25);
    let (wmos, doodads) = placements(&tiles, &grid);
    drop(tiles);

    let mut fp = Footprint::new(grid);
    let mut buildings: HashMap<String, Option<CollisionMesh>> = HashMap::new();
    for (k, w) in wmos.iter().enumerate() {
        let mesh = buildings
            .entry(w.model.to_ascii_lowercase())
            .or_insert_with(|| {
                load_wmo_collision_tris(&mut chain, &w.model)
                    .ok()
                    .filter(|m| !m.is_empty())
            });
        if let Some(mesh) = mesh {
            add_model(&mut fp, mesh, w.position, w.rotation, 1.0);
            fp.flush_into(&mut b, false);
        }
        progress(0.25 + 0.3 * (k + 1) as f32 / wmos.len().max(1) as f32);
    }

    let mut hulls: HashMap<String, Option<CollisionMesh>> = HashMap::new();
    let total = (doodads.len() + req.props.len()).max(1);
    let models = doodads
        .iter()
        .map(|d| (&d.model, d.position, d.rotation, d.scale))
        .chain(
            req.props
                .iter()
                .map(|p| (&p.model, p.position, p.rotation, p.scale)),
        );
    for (k, (model, position, rotation, scale)) in models.enumerate() {
        let hull = hulls.entry(model.to_ascii_lowercase()).or_insert_with(|| {
            load_m2_collision_hull(&mut chain, model)
                .ok()
                .filter(|m| !m.is_empty())
        });
        if let Some(hull) = hull {
            add_model(&mut fp, hull, position, rotation, scale);
            fp.flush_into(&mut b, true);
        }
        if k % 256 == 0 {
            progress(0.55 + 0.35 * k as f32 / total as f32);
        }
    }

    for iy in 0..grid.ny {
        for ix in 0..grid.nx {
            if !req.bounds.contains(grid.center(ix, iy)) {
                b.block(grid.index(ix, iy));
            }
        }
    }
    progress(0.95);
    let map = b.finish();
    progress(1.0);
    Ok(map)
}

/// Every building (WMO) whose origin stands within `radius` yards of `at`, nearest first, with
/// its distance: for placing enemies at real houses (the `buildings` example) and for the bake's
/// own test.
pub fn buildings_near(
    data_dir: &Path,
    map_id: u32,
    at: V2,
    radius: f32,
) -> Result<Vec<(f32, WmoInstance)>> {
    let mut chain = Chain::open(data_dir)
        .with_context(|| format!("opening WoW data at {}", data_dir.display()))?;
    let dir = map_directory(&mut chain, map_id)?;
    let side = (2.0 * radius / CELL).ceil().max(1.0) as u32;
    let grid = Grid {
        origin: V2::new(at.x - radius, at.y - radius),
        cell: CELL,
        nx: side,
        ny: side,
    };
    let tiles = load_tiles(&mut chain, &dir, &grid)?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for t in &tiles {
        for w in &t.wmos {
            let d = V2::new(w.position[0], w.position[1]).distance(at);
            if d <= radius && seen.insert(w.unique_id) {
                out.push((d, w.clone()));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.unique_id.cmp(&b.1.unique_id)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duskwood() -> Bounds {
        Bounds::new(vec![
            V2::new(-9880.0, 800.0),
            V2::new(-9880.0, -1850.0),
            V2::new(-11500.0, -1850.0),
            V2::new(-11500.0, 800.0),
        ])
        .unwrap()
    }

    #[test]
    fn the_grid_covers_the_bounds_in_half_yards() {
        let g = grid_for(&duskwood());
        assert_eq!(g.origin, V2::new(-11500.0, -1850.0));
        assert_eq!((g.nx, g.ny), (3240, 5300));
    }

    #[test]
    fn duskwood_lies_on_twenty_four_tiles() {
        let tiles = tiles_under(&grid_for(&duskwood()));
        assert_eq!(tiles.len(), 24);
        // The tile of the first entry, (-10051.8, -792.5).
        assert!(tiles.contains(&(33, 50)));
    }
}
