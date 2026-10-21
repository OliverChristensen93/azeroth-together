//! The host's eyes for the sim's [`together_sim::Obstacles`]: the arena scanned cell by cell
//! against the world's collision (trees, rocks, fences, walls and steep ground), so the horde
//! walks round them instead of through. A few hundred cells a frame, nearest the arena's centre
//! first; scanned again, more slowly, while the world's colliders keep streaming in. Joined
//! clients mirror the host's world and scan nothing.

use avian3d::prelude::SpatialQuery;
use benilla_world::collision::{ColliderEpoch, WorldCollision};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use together_sim::{Obstacles, V2};

use super::{session, units, Sim};

/// The grid's cell, in yards, and how far past the leash it reaches (spawns stand up to ten
/// yards outside it).
const CELL: f32 = 1.0;
const MARGIN: f32 = 12.0;
/// Cells probed a frame.
const PER_FRAME: usize = 300;
/// Probe heights above the ground, in yards: a trunk, a fence, a wall; ground steeper than about
/// 40 degrees hits too.
const PROBE_HEIGHTS: [f32; 2] = [0.9, 1.6];
/// Passes over the whole grid at full speed: the first, and more while colliders keep streaming
/// in. After these it goes on at a sixth of the pace for as long as the world keeps changing
/// (the hero walks into terrain and trees not loaded before).
const FAST_PASSES: u32 = 5;
/// Seconds the scan keeps going at least, whatever it has found.
const MIN_SECS: f32 = 15.0;
/// Where there is no terrain, the floor is looked for this far above and below the zone's height.
const FLOOR_PROBE: f32 = 40.0;

#[derive(Default)]
pub(super) struct Scan {
    /// Cells by distance from the centre, as (x, y).
    order: Vec<(u16, u16)>,
    next: usize,
    pass: u32,
    /// The collider epoch the pass began at, and whether any cell had no terrain yet.
    epoch: u64,
    missing: bool,
    /// Passes in a row that found nothing new, and the scan is finished.
    quiet: u32,
    done: bool,
    /// The blocked count last logged, and when the scan began.
    logged: usize,
    started: f32,
}

pub(super) fn scan(
    mut sim: ResMut<Sim>,
    net: Res<session::Net>,
    world_point: WorldPoint,
    spatial: SpatialQuery,
    epoch: Res<ColliderEpoch>,
    time: Res<Time<Real>>,
    mut state: Local<Scan>,
) {
    if net.is_client() {
        return;
    }
    let world = &mut sim.world;
    // A new run's world has no grid yet: a fresh scan.
    if world.obstacles.is_none() {
        let center = world.center();
        let grid = Obstacles::new(center, world.zone().leash + MARGIN, CELL);
        let side = grid.side();
        let mid = (side as f32 - 1.0) * 0.5;
        let mut order: Vec<(u16, u16)> = (0..side)
            .flat_map(|y| (0..side).map(move |x| (x as u16, y as u16)))
            .collect();
        order.sort_by(|a, b| {
            let da = (a.0 as f32 - mid).powi(2) + (a.1 as f32 - mid).powi(2);
            let db = (b.0 as f32 - mid).powi(2) + (b.1 as f32 - mid).powi(2);
            da.total_cmp(&db)
        });
        // Only the round arena and its margin, not the square's corners.
        let reach = (world.zone().leash + MARGIN) / CELL;
        order
            .retain(|&(x, y)| (x as f32 - mid).powi(2) + (y as f32 - mid).powi(2) <= reach * reach);
        world.obstacles = Some(grid);
        *state = Scan {
            order,
            next: 0,
            pass: 0,
            epoch: epoch.get(),
            missing: false,
            quiet: 0,
            done: false,
            logged: 0,
            started: time.elapsed_secs(),
        };
    }
    if state.done {
        return;
    }
    let per_frame = if state.pass < FAST_PASSES {
        PER_FRAME
    } else {
        PER_FRAME / 6
    };
    let fallback = world.zone().center[2];
    let filter = WorldCollision::body_filter();
    let Some(grid) = world.obstacles.as_mut() else {
        return;
    };
    let mut hits = Vec::new();
    let end = (state.next + per_frame).min(state.order.len());
    let mut missing = false;
    for &(x, y) in &state.order[state.next..end] {
        let (x, y) = (x as usize, y as usize);
        let at = grid.cell_center(x, y);
        let probe = units::ground_to_bevy(at, fallback);
        // The terrain's height, or where there is none (a building's floor over a hole, as in
        // Gurubashi Arena) the first solid thing under a ray from well above.
        let ground = world_point.terrain_height_under(probe).or_else(|| {
            let top = Vec3::new(probe.x, fallback + FLOOR_PROBE, probe.z);
            spatial
                .cast_ray(top, Dir3::NEG_Y, FLOOR_PROBE * 2.0, true, &filter)
                .map(|hit| top.y - hit.distance)
        });
        let Some(ground) = ground else {
            missing = true;
            continue;
        };
        // Along the sim's +x and +y to the next cells' centres: whatever stops the ray blocks
        // the cell it is in.
        for (step, dir) in [
            (V2::new(CELL, 0.0), Dir3::NEG_Z),
            (V2::new(0.0, CELL), Dir3::NEG_X),
        ] {
            for lift in PROBE_HEIGHTS {
                let from = Vec3::new(probe.x, ground + lift, probe.z);
                if let Some(hit) = spatial.cast_ray(from, dir, CELL, true, &filter) {
                    let into = (hit.distance + 0.25).min(CELL) / CELL;
                    hits.push(at + step * into);
                }
            }
        }
    }
    for p in hits {
        if let Some((x, y)) = grid.cell_at(p) {
            grid.set(x, y, true);
        }
    }
    state.missing |= missing;
    state.next = end;
    if state.next >= state.order.len() {
        // The pass is done; another while the world was still filling in.
        state.pass += 1;
        state.next = 0;
        let blocked = grid.blocked_count();
        if blocked != state.logged || state.pass == 1 {
            info!(
                "together: obstacle scan pass {}: {} of {} cells blocked{}",
                state.pass,
                blocked,
                state.order.len(),
                if state.missing {
                    ", some ground not loaded yet"
                } else {
                    ""
                }
            );
            state.logged = blocked;
            if let Ok(path) = std::env::var("TOGETHER_OBSTACLE_DUMP") {
                dump(grid, &path);
            }
        }
        // A pass is quiet when it finds the world as it was: no new colliders, and (in the fast
        // passes) no ground still missing; ground that never loads (a hole under nothing) stops
        // counting after them. Two quiet passes in a row end the scan.
        let changed = epoch.get() != state.epoch;
        let waiting = state.missing && state.pass < FAST_PASSES;
        state.quiet = if changed || waiting { 0 } else { state.quiet + 1 };
        // Never before [`MIN_SECS`]: a building's collider can attach a while after its terrain.
        let long_enough = time.elapsed_secs() - state.started >= MIN_SECS;
        if long_enough && state.quiet >= 2 {
            state.done = true;
        }
        state.epoch = epoch.get();
        state.missing = false;
    }
}

/// `TOGETHER_OBSTACLE_DUMP=<file.pgm>`: the grid as a greyscale picture, north up, blocked
/// cells black, for checking what the scan found.
fn dump(grid: &Obstacles, path: &str) {
    let side = grid.side();
    let mut out = format!("P5 {side} {side} 255\n").into_bytes();
    // Image rows run north to south: the sim's +x is north, +y west.
    for x in (0..side).rev() {
        for y in (0..side).rev() {
            out.push(if grid.is_blocked(x, y) { 0 } else { 220 });
        }
    }
    if let Err(e) = std::fs::write(path, out) {
        warn!("together: could not write the obstacle dump to {path}: {e}");
    }
}
