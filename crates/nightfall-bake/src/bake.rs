//! The bake itself: WoW's files in, a [`HeightMap`] out. Every tile under the bounds gives its
//! terrain (impassable chunks blocked); every building and doodad placed on those tiles gives its
//! collision, each model read once however often it is placed. Columns outside the bounds are
//! blocked, so no enemy walks out of the run. Every chunk's liquid deeper than a wade marks its
//! columns as water, where a scent trail breaks (spec §7.3).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use benilla_formats::{
    load_m2_collision_hull, load_map_catalog, load_tile_mesh, load_wmo_collision_tris, Chain,
    CollisionMesh, Doodad, LiquidMesh, TileMesh, WmoInstance, CHUNK_SIZE, TILE_SIZE,
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

/// What a bake made.
pub struct BakeOutput {
    pub map: HeightMap,
    /// Every file the data has but could not be read or parsed (a model a tile places is simply
    /// absent from a trimmed install; that is not listed). A map with any of these is missing
    /// terrain or collision a retry might find, so the cache does not keep it.
    pub failures: Vec<String>,
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

/// Every tile under the grid that the data has. A tile the data does not have (a zone the trimmed
/// data leaves out) is skipped; one it has but cannot read is skipped too and listed in
/// `failures`. No tile at all is an error.
fn load_tiles(
    chain: &mut Chain,
    dir: &str,
    g: &Grid,
    failures: &mut Vec<String>,
) -> Result<Vec<TileMesh>> {
    let mut tiles = Vec::new();
    for (tx, ty) in tiles_under(g) {
        if !chain.contains(&format!("World\\Maps\\{dir}\\{dir}_{tx}_{ty}.adt")) {
            continue;
        }
        match load_tile_mesh(chain, dir, tx, ty) {
            Ok(tile) => tiles.push(tile),
            Err(e) => failures.push(format!("tile {dir} {tx},{ty}: {e:#}")),
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

/// Liquid at least this deep over the terrain is water a trail breaks in, yards; shallower is a
/// puddle anyone wades through.
const WADE: f32 = 0.3;

/// The columns a liquid covers [`WADE`] deep or more are water (spec §7.3): each wet cell of its
/// grid marks the columns within its corners, its surface their mean height. Pools inside
/// buildings (WMO liquids) are not read: no trail crosses an indoor pool.
fn add_water(b: &mut HeightMapBuilder, liquid: &LiquidMesh) {
    let [cols, rows] = liquid.grid;
    let (cols, rows) = (cols as usize, rows as usize);
    if cols < 2 || rows < 2 || liquid.positions.len() < cols * rows {
        return;
    }
    for cy in 0..rows - 1 {
        for cx in 0..cols - 1 {
            if !liquid
                .wet
                .get(cy * (cols - 1) + cx)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            let corners = [
                cy * cols + cx,
                cy * cols + cx + 1,
                (cy + 1) * cols + cx,
                (cy + 1) * cols + cx + 1,
            ]
            .map(|i| liquid.positions[i]);
            let mut lo = V2::new(f32::MAX, f32::MAX);
            let mut hi = V2::new(f32::MIN, f32::MIN);
            let mut surface = 0.0;
            for p in corners {
                lo = V2::new(lo.x.min(p[0]), lo.y.min(p[1]));
                hi = V2::new(hi.x.max(p[0]), hi.y.max(p[1]));
                surface += p[2] / 4.0;
            }
            for col in b.columns_in(lo, hi) {
                if b.ground_of(col).is_none_or(|g| surface - g >= WADE) {
                    b.wet(col);
                }
            }
        }
    }
}

/// Every chunk's liquids on a tile; after its terrain, since the depth needs the ground.
fn add_tile_water(b: &mut HeightMapBuilder, tile: &TileMesh) {
    for chunk in &tile.chunks {
        for liquid in &chunk.liquids {
            add_water(b, liquid);
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
    for t in mesh.indices.as_chunks::<3>().0 {
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

/// A model's file name as the chain keys it: lowercase, `.mdx`/`.mdl` as `.m2` (the same
/// normalization `benilla-formats` applies when it reads one).
fn m2_path(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    match lower
        .strip_suffix(".mdx")
        .or_else(|| lower.strip_suffix(".mdl"))
    {
        Some(stem) => format!("{stem}.m2"),
        None => lower,
    }
}

/// What reading one model's collision gave.
enum Loaded {
    /// The data has no such file: a trimmed install, not an error for the map's own models.
    Absent,
    /// The file is there and could not be read (the failure is in the bake's list).
    Failed,
    /// The model has no collision geometry.
    Bare,
    Mesh(CollisionMesh),
}

/// Read one model once: `path` is its file in the chain, `load` the `benilla-formats` reader. A
/// file that is there but unreadable is added to `failures`.
fn read_model(
    chain: &mut Chain,
    path: &str,
    failures: &mut Vec<String>,
    load: impl FnOnce(&mut Chain) -> Result<CollisionMesh>,
) -> Loaded {
    if !chain.contains(path) {
        return Loaded::Absent;
    }
    match load(chain) {
        Ok(mesh) if mesh.is_empty() => Loaded::Bare,
        Ok(mesh) => Loaded::Mesh(mesh),
        Err(e) => {
            failures.push(format!("model {path}: {e:#}"));
            Loaded::Failed
        }
    }
}

/// Bake the request's map from the data, telling `progress` how far it has got (0 to 1). A
/// prop whose model the data lacks is an error; any other file the data has but cannot be read
/// is listed in the output's `failures`.
pub fn bake(req: &BakeRequest, progress: &dyn Fn(f32)) -> Result<BakeOutput> {
    progress(0.0);
    let mut chain = Chain::open(req.data_dir)
        .with_context(|| format!("opening WoW data at {}", req.data_dir.display()))?;
    for p in req.props {
        if !chain.contains(&m2_path(&p.model)) {
            bail!("prop model {} is not in the WoW data", p.model);
        }
    }
    let dir = map_directory(&mut chain, req.map_id)?;
    let grid = grid_for(req.bounds);
    let mut b = HeightMapBuilder::new(grid);
    let mut failures = Vec::new();
    let tiles = load_tiles(&mut chain, &dir, &grid, &mut failures)?;
    progress(0.1);
    for t in &tiles {
        add_terrain(&mut b, t);
        add_tile_water(&mut b, t);
    }
    progress(0.25);
    let (wmos, doodads) = placements(&tiles, &grid);
    drop(tiles);

    let mut fp = Footprint::new(grid);
    let mut buildings: HashMap<String, Loaded> = HashMap::new();
    for (k, w) in wmos.iter().enumerate() {
        let path = w.model.to_ascii_lowercase();
        if !buildings.contains_key(&path) {
            let loaded = read_model(&mut chain, &path, &mut failures, |c| {
                load_wmo_collision_tris(c, &path)
            });
            buildings.insert(path.clone(), loaded);
        }
        if let Loaded::Mesh(mesh) = &buildings[&path] {
            add_model(&mut fp, mesh, w.position, w.rotation, 1.0);
            fp.flush_into(&mut b, false);
        }
        progress(0.25 + 0.3 * (k + 1) as f32 / wmos.len().max(1) as f32);
    }

    let mut hulls: HashMap<String, Loaded> = HashMap::new();
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
        let path = m2_path(model);
        if !hulls.contains_key(&path) {
            let loaded = read_model(&mut chain, &path, &mut failures, |c| {
                load_m2_collision_hull(c, model)
            });
            hulls.insert(path.clone(), loaded);
        }
        if let Loaded::Mesh(hull) = &hulls[&path] {
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
    Ok(BakeOutput { map, failures })
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
    let mut failures = Vec::new();
    let tiles = load_tiles(&mut chain, &dir, &grid, &mut failures)?;
    if !failures.is_empty() {
        bail!(
            "{} tiles unreadable: {}",
            failures.len(),
            failures.join("; ")
        );
    }
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
    use benilla_formats::LiquidKind;

    /// A liquid of two cells side by side, 2 yards each, from (1, 1) to (5, 3), its surface at
    /// `z`, each cell wet as `wet` says.
    fn pool(z: f32, wet: [bool; 2]) -> LiquidMesh {
        let mut positions = Vec::new();
        for y in [1.0f32, 3.0] {
            for x in [1.0f32, 3.0, 5.0] {
                positions.push([x, y, z]);
            }
        }
        LiquidMesh {
            grid: [3, 2],
            wet: wet.to_vec(),
            shared: vec![false; 2],
            positions,
            uvs: vec![[0.0, 0.0]; 6],
            depths: vec![0.0; 6],
            indices: Vec::new(),
            sound_nibble: 0,
            material_id: None,
            kind: LiquidKind::Still,
        }
    }

    #[test]
    fn deep_liquid_is_water_and_a_puddle_is_not() {
        let grid = Grid {
            origin: V2::new(0.0, 0.0),
            cell: 0.5,
            nx: 12,
            ny: 12,
        };
        let mut b = HeightMapBuilder::flat(grid, 0.0);
        add_water(&mut b, &pool(1.0, [true, false]));
        let m = b.finish();
        assert!(m.is_water(V2::new(2.0, 2.0)));
        assert!(!m.is_water(V2::new(4.2, 2.0)));
        let mut b = HeightMapBuilder::flat(grid, 0.0);
        add_water(&mut b, &pool(0.1, [true, true]));
        assert_eq!(b.finish().water_columns(), 0);
    }

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
