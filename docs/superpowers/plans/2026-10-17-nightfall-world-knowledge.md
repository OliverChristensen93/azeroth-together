# Nightfall Sub-project 2 (World Knowledge) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The host knows Duskwood's walls, floors and cover from a height map baked from the player's own WoW data, and enemies placed from content stand at posts, walk patrol routes and roam areas on paths that go around houses, not through them, drawn by every client.

**Architecture:** `nightfall-sim` gains the layered height map (columns of solid spans; floors; indoors), line of sight, A* paths with a cache, enemies with three behaviours, and each player's set of seen enemies; it stays free of Bevy and WoW's formats and is tested on small hand-made maps. A new crate `nightfall-bake` reads terrain tiles, building (WMO) collision and doodad (M2) collision hulls through `benilla-formats`, rasterizes them into the sim's columns, and caches the result in `benilla-config/nightfall/`. The client bakes (or reads the cache) on a thread of its own when Nightfall's menu first shows, hands the map to the host's world, and draws the enemies as WoW creatures.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content), postcard (wire), flate2 (the cache), `benilla-formats` (ADT, WMO and M2 parsing).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md` (§4 in full; §7.1 for the enemies sub-project 2 places; §9.3 for props; §12 for tests). Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-world-knowledge-rulings.md`. This plan starts after every task of `docs/superpowers/plans/2026-10-16-nightfall-foundation.md` is done, and edits the client files that plan wrote.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git config --global --add safe.directory D:/Games/azeroth-together-stealth` once, or `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`.
- The worktree has no `WoW/Data`. Everything that reads WoW data (the bake test, the examples, the game) runs with `WOW_DATA=D:/Games/azeroth-together/WoW/Data`. A test that needs WoW data skips without it (`benilla_formats::wow_data_or_skip!`).
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats` (spec §3.1): its height map is built by hand in tests and by `nightfall-bake` in the game.
- `nightfall-bake` depends on `benilla-formats` (without default features, forwarding `dev`, as `benilla-formats`'s manifest asks of every dependent) and `nightfall-sim`, never on Bevy.
- "Baked on the host at first launch from the player's own WoW data, then cached in `benilla-config/nightfall/<map>-<hash>.bin` (the hash covers the data files and the prop list). Nothing Blizzard-derived ships in the zip." (spec §4.1). Nothing Blizzard-derived goes into `content/` or a commit.
- "Baking runs off the main thread with a progress line on the loading screen; a target of under a minute for Duskwood." (spec §4.1). Only the run's bounds are baked.
- No engine crate changes in this plan (`benilla-world`, `benilla-formats`, `benilla-assets` and the rest stay as they are).
- The client decides nothing about the game (spec §3.1): enemies move on the host; clients draw what snapshots say.
- Senses (spec §5) and combat (spec §6) are later sub-projects: enemies here notice nothing and fight nothing.
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A ray or a path that reaches past the baked region** (an enemy near the map's edge, a hero clamped to the bounds, a patrol point by the edge). Expect no panic: off-map columns are open air for sight and closed for walking, so no path leaves the map. Pinned in Task 2 (`rays_leaving_the_map_see_open_air_outside`) and Task 3 (`a_path_never_leaves_the_map`).
2. **A cache file that is stale or broken** (another WoW install, a different bounds or prop list, a write cut short by a crash). Expect a new key or a refusal, then a fresh bake; never a crash and never a wrong map. Pinned in Task 4 (`a_cut_short_or_foreign_file_is_refused`, `a_header_claiming_billions_of_columns_is_refused_before_allocating`) and Task 7 (`the_key_follows_the_data_the_bounds_and_the_props`, `a_corrupt_cache_file_is_not_trusted`).
3. **A patrol point or post the map cannot reach** (inside a closed house, on a roof, in a hole). Expect the point skipped and named in a warning, the enemy still walking its other points, and the host never searching for it again. Pinned in Task 8 (`an_unreachable_patrol_point_is_skipped_and_named`, `a_spawn_with_no_floor_is_left_out_and_named`).
4. **A hero pressed against a wall**, whose half-yard column holds the wall itself. Expect them still to see (and be seen) away from the wall, and not through a wall thicker than their column. Pinned in Task 2 (`a_hero_against_a_wall_sees_away_from_it_but_not_through_it`).
5. **Many path searches at once, or one that cannot succeed** (wanderers all choosing a new spot in one step, a goal in a sealed room). Expect each search to stop at its budget and no more searches a step than `paths_per_step`, so the host never hitches. Pinned in Task 3 (`an_unreachable_goal_gives_up_at_the_budget`) and Task 8 (`no_more_paths_a_step_than_the_rules_allow`).

**Not in this plan** (later sub-projects, see the rulings file): the client's `Concealed` marker that hides unseen enemies from Tab, nameplates and the minimap (sub-project 3; this plan computes, sends and keeps the seen set it needs); spawning `props.ron` models in the client (the bake already takes props); tracing Duskwood's real edge for the bounds; sleepers, trackers, casters, elites and ravens (§7.1, sub-projects 3, 4 and 6); moving the enemy groups from near the entry to their points of interest, which needs measuring in the game.

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/math.rs` | `V3` beside `V2` |
| `crates/nightfall-sim/src/heightmap/mod.rs` | `Grid`, `Span`, `HeightMap`: columns, floors, indoors, blocked columns |
| `crates/nightfall-sim/src/heightmap/build.rs` | `HeightMapBuilder`: terrain and spans in any order, merged at the end |
| `crates/nightfall-sim/src/heightmap/sight.rs` | `HeightMap::line_of_sight` |
| `crates/nightfall-sim/src/heightmap/path.rs` | A*, smoothing, `walk_line`, `Navigator` and its cache |
| `crates/nightfall-sim/src/heightmap/codec.rs` | `HeightMap::to_bytes` / `from_bytes` |
| `crates/nightfall-sim/src/heightmap/testmap.rs` | Hand-made maps for tests: flat ground, a house |
| `crates/nightfall-sim/src/content.rs` | `EnemyDef`, `SpawnDef`, `Behaviour`, `WorldRules` |
| `crates/nightfall-sim/src/enemies.rs` | `Enemy`; placing them; sentries, patrols, wanderers |
| `crates/nightfall-sim/src/sight.rs` | What each hero sees; each player's snapshot |
| `crates/nightfall-sim/src/world.rs` | Enemies, the attached map, the hero's height |
| `crates/nightfall-sim/src/messages.rs` | `State` carries the seen set; protocol 2 |
| `content/nightfall/{enemies,spawns}.ron`, `rules.ron` | Enemy kinds, their placements, the world's rules |
| `crates/nightfall-bake/src/place.rs` | A placed model's point in the world |
| `crates/nightfall-bake/src/raster.rs` | `Footprint`: triangles into columns |
| `crates/nightfall-bake/src/bake.rs` | `bake`, `buildings_near`: WoW files into a `HeightMap` |
| `crates/nightfall-bake/src/cache.rs` | `load_or_bake`, the cache key and file |
| `crates/nightfall-bake/tests/duskwood.rs` | Darkshire baked from real data (skipped without it) |
| `crates/nightfall-bake/examples/{bake_duskwood,buildings}.rs` | The bake's timing; buildings near a point for placing enemies |
| `crates/benilla-app/src/nightfall/terrain.rs` | The bake thread, its progress line, the map for the host |
| `crates/benilla-app/src/nightfall/enemies.rs` | Enemies drawn as WoW creatures |
| `crates/benilla-app/src/nightfall/{mod,session,run,units}.rs` | Content texts, per-player states, the seen set, the map attached, creature parts |

---

### Task 1: The height map: columns, floors, indoors

**Files:**
- Create: `crates/nightfall-sim/src/heightmap/mod.rs`, `crates/nightfall-sim/src/heightmap/build.rs`, `crates/nightfall-sim/src/heightmap/testmap.rs`
- Modify: `crates/nightfall-sim/src/math.rs`, `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Produces:
  - `nightfall_sim::V3 { x: f32, y: f32, z: f32 }` with `new(f32, f32, f32)`, `at(V2, f32)`, `ground(self) -> V2`, `is_finite(self) -> bool` (derives `Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize`).
  - Constants `CELL = 0.5`, `TERRAIN_DEPTH = 2.0`, `MERGE_GAP = 0.2`, `HEADROOM = 2.0`, `MAX_STEP = 0.7`, `MAX_FLOORS = 8`.
  - `Span { lo: f32, hi: f32 }`; `Grid { origin: V2, cell: f32, nx: u32, ny: u32 }` with `columns() -> usize`, `column_of(V2) -> Option<(u32, u32)>`, `index(u32, u32) -> usize`, `cell_of(usize) -> (u32, u32)`, `center(u32, u32) -> V2`.
  - `HeightMapBuilder` with `new(Grid)`, `flat(Grid, f32)`, `grid() -> Grid`, `set_ground(usize, f32)`, `add_span(usize, f32, f32)`, `block(usize)`, `columns_in(V2, V2) -> Vec<usize>`, `add_box(V2, V2, f32, f32)`, `block_box(V2, V2)`, `finish(self) -> HeightMap`.
  - `HeightMap` with `grid() -> Grid`, `complex_columns() -> usize`, `spans_at(usize) -> SpanList<'_>`, `floors_at(usize) -> Floors`, `is_blocked(usize) -> bool`, `ground_at(V2) -> Option<f32>`, `floor_within(V2, f32, f32) -> Option<f32>`, `floor_near(V2, f32) -> Option<f32>`, `floor_below(V2, f32) -> Option<f32>`, `is_indoors(V2, f32) -> bool`; crate-only `level_within(usize, f32, f32) -> Option<(usize, f32)>`.
  - `SpanList::as_slice() -> &[Span]`, `Floors::as_slice() -> &[f32]`.
  - Test-only `crate::heightmap::testmap::{grid(f32) -> Grid, flat(f32) -> HeightMapBuilder, house(&mut HeightMapBuilder, V2, V2, Option<f32>, bool)}`.

- [ ] **Step 1: Add `V3` with its test**

In `crates/nightfall-sim/src/math.rs`, below `impl Mul<f32> for V2 { … }`:

```rust
/// A point in WoW's world: `x` north, `y` west, `z` up, in yards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl V3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// The point `z` yards up over the ground point `p`.
    pub const fn at(p: V2, z: f32) -> Self {
        Self { x: p.x, y: p.y, z }
    }

    /// Where it stands on the ground plane.
    pub const fn ground(self) -> V2 {
        V2::new(self.x, self.y)
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}
```

In its `tests` module add:

```rust
    #[test]
    fn a_point_stands_on_its_ground_point() {
        let p = V3::at(V2::new(1.0, 2.0), 3.0);
        assert_eq!(p, V3::new(1.0, 2.0, 3.0));
        assert_eq!(p.ground(), V2::new(1.0, 2.0));
        assert!(p.is_finite());
        assert!(!V3::new(0.0, 0.0, f32::NAN).is_finite());
    }
```

- [ ] **Step 2: Write the test maps and the failing tests**

`crates/nightfall-sim/src/heightmap/testmap.rs`:

```rust
//! Small hand-made height maps for tests: flat ground and a house, in yards from the origin. No
//! WoW data is needed to test sight and paths on them (spec §12).

use super::{Grid, HeightMapBuilder, CELL};
use crate::V2;

/// A square grid `size` yards across, its corner at the origin.
pub(crate) fn grid(size: f32) -> Grid {
    let n = (size / CELL) as u32;
    Grid {
        origin: V2::ZERO,
        cell: CELL,
        nx: n,
        ny: n,
    }
}

/// Flat ground at height 0, `size` yards across.
pub(crate) fn flat(size: f32) -> HeightMapBuilder {
    HeightMapBuilder::flat(grid(size), 0.0)
}

/// A house over `[min, max]` on ground at height 0: walls half a yard thick and 3 yards tall, a
/// door 2.5 yards wide in the south wall (the one at `min.x`) centred at `door_y` when given, and
/// a roof slab on top when `roof`.
pub(crate) fn house(b: &mut HeightMapBuilder, min: V2, max: V2, door_y: Option<f32>, roof: bool) {
    const THICK: f32 = 0.5;
    const TALL: f32 = 3.0;
    // North, east and west walls.
    b.add_box(V2::new(max.x - THICK, min.y), max, 0.0, TALL);
    b.add_box(min, V2::new(max.x, min.y + THICK), 0.0, TALL);
    b.add_box(V2::new(min.x, max.y - THICK), max, 0.0, TALL);
    // The south wall, around its door.
    match door_y {
        None => b.add_box(min, V2::new(min.x + THICK, max.y), 0.0, TALL),
        Some(d) => {
            b.add_box(min, V2::new(min.x + THICK, d - 1.25), 0.0, TALL);
            b.add_box(V2::new(min.x, d + 1.25), V2::new(min.x + THICK, max.y), 0.0, TALL);
        }
    }
    if roof {
        b.add_box(min, max, TALL, TALL + 0.3);
    }
}
```

`crates/nightfall-sim/src/heightmap/mod.rs` (Step 4 adds the code above the tests):

```rust
mod build;
#[cfg(test)]
pub(crate) mod testmap;

#[cfg(test)]
mod tests {
    use super::testmap::{flat, house};
    use super::*;

    fn col(m: &HeightMap, x: f32, y: f32) -> usize {
        let g = m.grid();
        let (ix, iy) = g.column_of(V2::new(x, y)).unwrap();
        g.index(ix, iy)
    }

    #[test]
    fn a_flat_map_has_one_floor_everywhere() {
        let m = flat(20.0).finish();
        let p = V2::new(5.0, 5.0);
        assert_eq!(m.ground_at(p), Some(0.0));
        assert_eq!(m.floor_near(p, 7.0), Some(0.0));
        assert_eq!(m.floor_below(p, 0.0), Some(0.0));
        assert_eq!(m.floor_below(p, -1.0), None);
        assert_eq!(m.floors_at(col(&m, 5.0, 5.0)).as_slice(), &[0.0]);
        assert_eq!(m.complex_columns(), 0);
        assert!(!m.is_indoors(p, 0.0));
    }

    #[test]
    fn a_slab_overhead_makes_a_second_storey() {
        let mut b = flat(20.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(10.0, 10.0), 3.0, 3.2);
        let m = b.finish();
        let p = V2::new(7.0, 7.0);
        assert_eq!(m.floors_at(col(&m, 7.0, 7.0)).as_slice(), &[0.0, 3.2]);
        assert_eq!(m.floor_near(p, 3.0), Some(3.2));
        assert_eq!(m.floor_below(p, 2.0), Some(0.0));
        assert_eq!(m.floor_within(p, 1.8, 1.0), None);
        assert!(m.is_indoors(p, 0.0));
        assert!(!m.is_indoors(p, 3.2));
    }

    #[test]
    fn a_low_ceiling_leaves_no_room_to_stand_under_it() {
        let mut b = flat(20.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(10.0, 10.0), 1.0, 1.2);
        let m = b.finish();
        assert_eq!(m.floors_at(col(&m, 7.0, 7.0)).as_slice(), &[1.2]);
    }

    #[test]
    fn touching_solids_merge_into_one() {
        let mut b = flat(20.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(5.5, 5.5), 0.0, 1.0);
        b.add_box(V2::new(5.0, 5.0), V2::new(5.5, 5.5), 1.1, 2.0);
        let m = b.finish();
        let c = col(&m, 5.25, 5.25);
        assert_eq!(
            m.spans_at(c).as_slice(),
            &[Span {
                lo: -TERRAIN_DEPTH,
                hi: 2.0
            }]
        );
        assert_eq!(m.floors_at(c).as_slice(), &[2.0]);
    }

    #[test]
    fn the_order_things_are_added_in_does_not_matter() {
        let mut a = flat(10.0);
        a.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 3.0, 3.2);
        a.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 0.0, 1.0);
        let mut b = flat(10.0);
        b.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 0.0, 1.0);
        b.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 3.0, 3.2);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn heights_are_kept_to_an_eighth_of_a_yard() {
        let mut b = flat(4.0);
        b.set_ground(0, 1.03);
        let m = b.finish();
        let g = m.ground_at(V2::new(0.25, 0.25)).unwrap();
        assert!((g - 1.03).abs() <= 1.0 / 16.0, "{g}");
    }

    #[test]
    fn a_roofed_house_is_indoors_and_its_yard_is_not() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), Some(25.0), true);
        let m = b.finish();
        assert!(m.is_indoors(V2::new(25.0, 25.0), 0.0));
        assert!(!m.is_indoors(V2::new(10.0, 10.0), 0.0));
    }

    #[test]
    fn off_the_map_there_is_nothing_to_stand_on() {
        let m = flat(20.0).finish();
        assert_eq!(m.floor_near(V2::new(-1.0, 5.0), 0.0), None);
        assert_eq!(m.ground_at(V2::new(5.0, 25.0)), None);
        assert!(!m.is_indoors(V2::new(-1.0, 5.0), 0.0));
        assert!(m.is_blocked(m.grid().columns()));
        assert_eq!(m.floor_near(V2::new(f32::NAN, 5.0), 0.0), None);
        assert_eq!(m.floor_near(V2::new(5.0, 5.0), f32::NAN), None);
    }

    #[test]
    fn blocked_columns_are_remembered() {
        let mut b = flat(20.0);
        b.block_box(V2::new(0.0, 0.0), V2::new(1.0, 1.0));
        let m = b.finish();
        assert!(m.is_blocked(col(&m, 0.25, 0.75)));
        assert!(!m.is_blocked(col(&m, 1.25, 0.25)));
    }

    #[test]
    fn a_hole_in_the_ground_has_only_what_lies_under_it() {
        let mut b = flat(20.0);
        let c = b.grid().index(4, 4);
        b.set_ground(c, f32::NAN);
        b.add_span(c, -6.0, -5.5);
        let m = b.finish();
        assert_eq!(m.floors_at(c).as_slice(), &[-5.5]);
        assert_eq!(m.ground_at(V2::new(2.25, 2.25)), None);
    }

    #[test]
    fn a_box_covers_the_columns_whose_centres_it_holds() {
        let b = flat(10.0);
        let cols = b.columns_in(V2::new(1.0, 1.0), V2::new(2.0, 1.5));
        let g = b.grid();
        assert_eq!(cols, vec![g.index(2, 2), g.index(3, 2)]);
        assert!(b.columns_in(V2::new(-5.0, -5.0), V2::new(-1.0, -1.0)).is_empty());
    }
}
```

Add to `crates/nightfall-sim/src/lib.rs`, beside `mod content;`: `mod heightmap;`.

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim heightmap`
Expected: FAIL to compile (`file not found for module build`; `HeightMap`, `HeightMapBuilder` and the rest do not exist yet).

- [ ] **Step 4: Implement the height map and its builder**

Put this at the top of `crates/nightfall-sim/src/heightmap/mod.rs`, replacing its first three lines (the `mod` lines and the `#[cfg(test)]` before `testmap`), and keeping the `tests` module below:

```rust
//! The baked height map: what the host knows of the world's solid shape, in the style of
//! Recast's heightfield (spec §4.1). The ground plane is cut into square columns ([`CELL`]
//! yards); each column holds its solid spans, bottom first. The top of a span with [`HEADROOM`]
//! of air over it is a floor, so a column can hold several: a house's two storeys, a bridge over
//! a road, a crypt under a hill. Line of sight and paths run on it (later modules).
//!
//! The terrain is a shell [`TERRAIN_DEPTH`] yards thick under its surface rather than solid to
//! the bottom of the world, so a cave dug under a hillside keeps its air. Most columns are
//! terrain alone and are stored as one quantized height; the few with anything else on them
//! (walls, floors, trunks, rocks) keep their whole span list. `nightfall-bake` builds a map from
//! the player's WoW data; tests build small ones by hand with [`HeightMapBuilder`].

mod build;
#[cfg(test)]
pub(crate) mod testmap;

use std::fmt;

pub use build::HeightMapBuilder;

use crate::V2;

/// Yards across a column: a doorway is four or five of them, and Duskwood's bounds are 17
/// million.
pub const CELL: f32 = 0.5;
/// How deep under its surface the terrain is solid. Not to the bottom of the world: a crypt or a
/// mine dug under a hillside keeps its air, and no ray reaches this deep except through a hole.
pub const TERRAIN_DEPTH: f32 = 2.0;
/// Solids closer than this (yards) are one: a floor laid on the ground, a wall standing on it.
pub const MERGE_GAP: f32 = 0.2;
/// Air a unit needs over a floor to stand on it, yards.
pub const HEADROOM: f32 = 2.0;
/// The highest step a unit takes from one column to the next, yards: stairs and slopes up to
/// about 55°, never a ledge or a table.
pub const MAX_STEP: f32 = 0.7;
/// The most floors a column reports; more is a stack of thin slabs, not storeys.
pub const MAX_FLOORS: usize = 8;

/// Terrain heights are kept in eighths of a yard as `i16`: two bytes a column instead of eight,
/// across 17 million columns.
const Z_SCALE: f32 = 8.0;
/// A column with no terrain: a hole (a cave's mouth), or off every tile.
const NO_GROUND: i16 = i16::MIN;

/// Solid from `lo` to `hi`, yards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub lo: f32,
    pub hi: f32,
}

/// Where the columns lie: column `(ix, iy)` covers `x` from `origin.x + ix·cell` and `y` from
/// `origin.y + iy·cell`, one `cell` each way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub origin: V2,
    pub cell: f32,
    pub nx: u32,
    pub ny: u32,
}

impl Grid {
    pub fn columns(&self) -> usize {
        self.nx as usize * self.ny as usize
    }

    /// The column holding ground point `p`, if any.
    pub fn column_of(&self, p: V2) -> Option<(u32, u32)> {
        if !p.is_finite() {
            return None;
        }
        let fx = ((p.x - self.origin.x) / self.cell).floor();
        let fy = ((p.y - self.origin.y) / self.cell).floor();
        if fx < 0.0 || fy < 0.0 || fx >= self.nx as f32 || fy >= self.ny as f32 {
            return None;
        }
        Some((fx as u32, fy as u32))
    }

    /// Column `(ix, iy)`'s index, rows of `nx` columns.
    pub fn index(&self, ix: u32, iy: u32) -> usize {
        iy as usize * self.nx as usize + ix as usize
    }

    /// The column at an index, as `(ix, iy)`.
    pub fn cell_of(&self, col: usize) -> (u32, u32) {
        let nx = self.nx as usize;
        ((col % nx) as u32, (col / nx) as u32)
    }

    /// The middle of column `(ix, iy)`.
    pub fn center(&self, ix: u32, iy: u32) -> V2 {
        V2::new(
            self.origin.x + (ix as f32 + 0.5) * self.cell,
            self.origin.y + (iy as f32 + 0.5) * self.cell,
        )
    }
}

/// A terrain height as stored; not a number is no terrain.
fn quantize(z: f32) -> i16 {
    if z.is_finite() {
        (z * Z_SCALE).round().clamp(-32767.0, 32767.0) as i16
    } else {
        NO_GROUND
    }
}

fn dequantize(q: i16) -> Option<f32> {
    (q != NO_GROUND).then(|| f32::from(q) / Z_SCALE)
}

/// A column's solid spans, bottom first: its stored list, or the terrain's shell alone.
pub struct SpanList<'a> {
    one: [Span; 1],
    many: Option<&'a [Span]>,
    has_one: bool,
}

impl SpanList<'_> {
    pub fn as_slice(&self) -> &[Span] {
        match self.many {
            Some(many) => many,
            None if self.has_one => &self.one,
            None => &[],
        }
    }
}

/// A column's floors, bottom first: the tops of its spans with [`HEADROOM`] over them.
#[derive(Clone, Copy, Debug, Default)]
pub struct Floors {
    len: usize,
    z: [f32; MAX_FLOORS],
}

impl Floors {
    pub fn as_slice(&self) -> &[f32] {
        &self.z[..self.len]
    }

    fn push(&mut self, z: f32) {
        if self.len < MAX_FLOORS {
            self.z[self.len] = z;
            self.len += 1;
        }
    }
}

/// The world's solid shape over a [`Grid`].
#[derive(PartialEq)]
pub struct HeightMap {
    grid: Grid,
    /// Each column's terrain height, quantized ([`quantize`]).
    ground: Vec<i16>,
    /// The columns with more than terrain, ascending, and where each one's spans start in
    /// `spans` (one more entry than `cols`, the last the end).
    cols: Vec<u32>,
    starts: Vec<u32>,
    spans: Vec<Span>,
    /// One bit a column: no unit may walk into it.
    blocked: Vec<u64>,
}

// Written out: a derived `Debug` would print 17 million heights into a failing test's output.
impl fmt::Debug for HeightMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeightMap")
            .field("grid", &self.grid)
            .field("complex_columns", &self.cols.len())
            .field("spans", &self.spans.len())
            .finish()
    }
}

impl HeightMap {
    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// How many columns hold more than terrain.
    pub fn complex_columns(&self) -> usize {
        self.cols.len()
    }

    fn col_at(&self, p: V2) -> Option<usize> {
        let (ix, iy) = self.grid.column_of(p)?;
        Some(self.grid.index(ix, iy))
    }

    /// Column `col`'s solid spans, bottom first; none off the grid.
    pub fn spans_at(&self, col: usize) -> SpanList<'_> {
        let none = Span { lo: 0.0, hi: 0.0 };
        if let Ok(k) = self.cols.binary_search(&(col as u32)) {
            let (a, b) = (self.starts[k] as usize, self.starts[k + 1] as usize);
            return SpanList {
                one: [none],
                many: Some(&self.spans[a..b]),
                has_one: false,
            };
        }
        match self.ground.get(col).copied().and_then(dequantize) {
            Some(g) => SpanList {
                one: [Span {
                    lo: g - TERRAIN_DEPTH,
                    hi: g,
                }],
                many: None,
                has_one: true,
            },
            None => SpanList {
                one: [none],
                many: None,
                has_one: false,
            },
        }
    }

    /// Column `col`'s floors, bottom first.
    pub fn floors_at(&self, col: usize) -> Floors {
        let list = self.spans_at(col);
        let spans = list.as_slice();
        let mut floors = Floors::default();
        for (i, s) in spans.iter().enumerate() {
            let room = spans.get(i + 1).map_or(f32::INFINITY, |above| above.lo - s.hi);
            if room >= HEADROOM {
                floors.push(s.hi);
            }
        }
        floors
    }

    /// Whether no unit may walk into column `col`; every column off the grid is.
    pub fn is_blocked(&self, col: usize) -> bool {
        col >= self.grid.columns() || self.blocked[col / 64] >> (col % 64) & 1 == 1
    }

    /// The terrain's height under `p` (not what stands on it); `None` off the map or in a hole.
    pub fn ground_at(&self, p: V2) -> Option<f32> {
        dequantize(self.ground[self.col_at(p)?])
    }

    /// The floor of column `col` nearest `z` and within `reach` of it, as (which floor, bottom
    /// first; its height). A tie goes to the lower floor.
    pub(crate) fn level_within(&self, col: usize, z: f32, reach: f32) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (level, &f) in self.floors_at(col).as_slice().iter().enumerate() {
            let d = (f - z).abs();
            if d <= reach && best.is_none_or(|(_, b)| d < (b - z).abs()) {
                best = Some((level, f));
            }
        }
        best
    }

    /// The floor under `p` nearest height `z`, if one is within `reach`.
    pub fn floor_within(&self, p: V2, z: f32, reach: f32) -> Option<f32> {
        if !z.is_finite() {
            return None;
        }
        self.level_within(self.col_at(p)?, z, reach).map(|(_, f)| f)
    }

    /// The floor under `p` nearest height `z`: for placing a unit.
    pub fn floor_near(&self, p: V2, z: f32) -> Option<f32> {
        self.floor_within(p, z, f32::INFINITY)
    }

    /// The highest floor under `p` that a unit at height `z` stands on or over (a step up
    /// included).
    pub fn floor_below(&self, p: V2, z: f32) -> Option<f32> {
        let col = self.col_at(p)?;
        self.floors_at(col)
            .as_slice()
            .iter()
            .copied()
            .filter(|&f| f <= z + MAX_STEP)
            .last()
    }

    /// Whether something solid stands over the floor a unit at `p`, height `z`, stands on: a
    /// roof, an upper storey, a bridge (spec §4.2; noise through walls and rain will use it).
    pub fn is_indoors(&self, p: V2, z: f32) -> bool {
        let (Some(col), Some(floor)) = (self.col_at(p), self.floor_below(p, z)) else {
            return false;
        };
        self.spans_at(col).as_slice().iter().any(|s| s.lo > floor)
    }
}
```

`crates/nightfall-sim/src/heightmap/build.rs`:

```rust
//! Building a [`HeightMap`]: terrain heights and solid spans in any order, merged at the end.

use super::{dequantize, quantize, Grid, HeightMap, Span, MERGE_GAP, TERRAIN_DEPTH};
use crate::V2;

/// A height map under construction. Spans are kept as they come (a bake adds millions, many over
/// the same column) and merged per column in [`HeightMapBuilder::finish`], so the order things
/// are added in never changes the map.
pub struct HeightMapBuilder {
    grid: Grid,
    /// Each column's terrain height; not a number for none.
    ground: Vec<f32>,
    raw: Vec<(u32, Span)>,
    blocked: Vec<u64>,
}

impl HeightMapBuilder {
    /// A grid with no terrain and nothing on it.
    pub fn new(grid: Grid) -> Self {
        let n = grid.columns();
        Self {
            grid,
            ground: vec![f32::NAN; n],
            raw: Vec::new(),
            blocked: vec![0; n.div_ceil(64)],
        }
    }

    /// A grid of flat terrain at height `z`.
    pub fn flat(grid: Grid, z: f32) -> Self {
        let mut b = Self::new(grid);
        b.ground.fill(z);
        b
    }

    pub fn grid(&self) -> Grid {
        self.grid
    }

    /// Column `col`'s terrain height; not a number takes its terrain away (a hole).
    pub fn set_ground(&mut self, col: usize, z: f32) {
        if let Some(g) = self.ground.get_mut(col) {
            *g = z;
        }
    }

    /// Solid from `lo` to `hi` (either order) in column `col`. A span that is not a number, or a
    /// column off the grid, is dropped: a broken model must not break the map.
    pub fn add_span(&mut self, col: usize, lo: f32, hi: f32) {
        if col >= self.ground.len() || !lo.is_finite() || !hi.is_finite() {
            return;
        }
        let span = Span {
            lo: lo.min(hi),
            hi: lo.max(hi),
        };
        self.raw.push((col as u32, span));
    }

    /// No unit may walk into column `col` (an impassable chunk, outside the run's bounds). It
    /// blocks nothing else: sight passes.
    pub fn block(&mut self, col: usize) {
        if let Some(w) = self.blocked.get_mut(col / 64) {
            *w |= 1 << (col % 64);
        }
    }

    /// The columns whose centres lie in `[min, max)`.
    pub fn columns_in(&self, min: V2, max: V2) -> Vec<usize> {
        let g = self.grid;
        let range = |lo: f32, hi: f32, origin: f32, n: u32| {
            let first = ((lo - origin) / g.cell - 0.5).ceil().max(0.0) as i64;
            let last = (((hi - origin) / g.cell - 0.5).ceil() as i64 - 1).min(n as i64 - 1);
            first..=last
        };
        let mut out = Vec::new();
        for iy in range(min.y, max.y, g.origin.y, g.ny) {
            for ix in range(min.x, max.x, g.origin.x, g.nx) {
                out.push(g.index(ix as u32, iy as u32));
            }
        }
        out
    }

    /// Solid from `lo` to `hi` over every column whose centre is in `[min, max)`.
    pub fn add_box(&mut self, min: V2, max: V2, lo: f32, hi: f32) {
        for col in self.columns_in(min, max) {
            self.add_span(col, lo, hi);
        }
    }

    /// Every column whose centre is in `[min, max)` blocked.
    pub fn block_box(&mut self, min: V2, max: V2) {
        for col in self.columns_in(min, max) {
            self.block(col);
        }
    }

    /// The map: each column's terrain shell and spans sorted and merged where they touch
    /// ([`MERGE_GAP`]); a column that is terrain alone keeps only its height.
    pub fn finish(mut self) -> HeightMap {
        let ground: Vec<i16> = self.ground.iter().map(|&z| quantize(z)).collect();
        self.raw
            .sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.lo.total_cmp(&b.1.lo)));
        let (mut cols, mut starts, mut spans) = (Vec::new(), Vec::new(), Vec::new());
        let mut list: Vec<Span> = Vec::new();
        let mut i = 0;
        while i < self.raw.len() {
            let col = self.raw[i].0;
            list.clear();
            // The shell as stored, so a column's terrain is the same height whichever way it is
            // kept.
            if let Some(g) = dequantize(ground[col as usize]) {
                list.push(Span {
                    lo: g - TERRAIN_DEPTH,
                    hi: g,
                });
            }
            while i < self.raw.len() && self.raw[i].0 == col {
                list.push(self.raw[i].1);
                i += 1;
            }
            list.sort_unstable_by(|a, b| a.lo.total_cmp(&b.lo));
            cols.push(col);
            starts.push(spans.len() as u32);
            let mut cur = list[0];
            for s in &list[1..] {
                if s.lo <= cur.hi + MERGE_GAP {
                    cur.hi = cur.hi.max(s.hi);
                } else {
                    spans.push(cur);
                    cur = *s;
                }
            }
            spans.push(cur);
        }
        starts.push(spans.len() as u32);
        HeightMap {
            grid: self.grid,
            ground,
            cols,
            starts,
            spans,
            blocked: self.blocked,
        }
    }
}
```

In `crates/nightfall-sim/src/lib.rs` add, beside the other `pub use` lines:

```rust
pub use heightmap::{
    Floors, Grid, HeightMap, HeightMapBuilder, Span, SpanList, CELL, HEADROOM, MAX_FLOORS,
    MAX_STEP, MERGE_GAP, TERRAIN_DEPTH,
};
```

and change `pub use math::V2;` to `pub use math::{V2, V3};`.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim heightmap math`
Expected: PASS (11 height map tests, 3 math tests).

- [ ] **Step 6: Commit**

```bash
git add crates/nightfall-sim/src/heightmap crates/nightfall-sim/src/math.rs crates/nightfall-sim/src/lib.rs
git commit -m "nightfall-sim: the layered height map, its floors and indoors

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Line of sight

**Files:**
- Create: `crates/nightfall-sim/src/heightmap/sight.rs`
- Modify: `crates/nightfall-sim/src/heightmap/mod.rs` (`mod sight;`, `Grid::at`)

**Interfaces:**
- Consumes: `HeightMap`, `Grid`, `Span`, `V3`, `testmap` (Task 1).
- Produces: `HeightMap::line_of_sight(&self, a: V3, b: V3) -> bool`; crate-only `Grid::at(&self, ix: i64, iy: i64) -> Option<usize>`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/heightmap/sight.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::super::testmap::{flat, house};
    use crate::{V2, V3};

    fn eye(x: f32, y: f32) -> V3 {
        V3::new(x, y, 1.7)
    }

    #[test]
    fn open_ground_is_seen_across() {
        let m = flat(50.0).finish();
        assert!(m.line_of_sight(eye(10.0, 10.0), eye(15.0, 40.0)));
    }

    #[test]
    fn a_house_stands_in_the_way() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
        let m = b.finish();
        assert!(!m.line_of_sight(eye(10.0, 25.0), eye(40.0, 25.0)));
        // Past its corner it does not.
        assert!(m.line_of_sight(eye(10.0, 10.0), eye(15.0, 40.0)));
    }

    #[test]
    fn a_door_is_seen_through() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), Some(25.0), false);
        let m = b.finish();
        assert!(m.line_of_sight(eye(10.0, 25.0), eye(25.0, 25.0)));
    }

    #[test]
    fn a_low_wall_is_seen_over_and_a_tall_one_is_not() {
        let mut low = flat(50.0);
        low.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 0.8);
        assert!(low.finish().line_of_sight(eye(20.0, 10.0), eye(30.0, 10.0)));
        let mut tall = flat(50.0);
        tall.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 2.5);
        assert!(!tall.finish().line_of_sight(eye(20.0, 10.0), eye(30.0, 10.0)));
    }

    #[test]
    fn a_hill_hides_what_is_behind_it() {
        let mut b = flat(50.0);
        b.add_box(V2::new(24.0, 5.0), V2::new(26.0, 15.0), 0.0, 3.0);
        assert!(!b.finish().line_of_sight(eye(20.0, 10.0), eye(30.0, 10.0)));
    }

    #[test]
    fn nobody_sees_through_a_roof() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), Some(25.0), true);
        let m = b.finish();
        assert!(!m.line_of_sight(eye(25.0, 25.0), V3::new(25.1, 25.1, 10.0)));
    }

    #[test]
    fn a_hero_against_a_wall_sees_away_from_it_but_not_through_it() {
        // The hero's eye is in the wall's own column.
        let at = eye(24.3, 10.0);
        let mut thin = flat(50.0);
        thin.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
        assert!(thin.finish().line_of_sight(at, eye(10.0, 10.0)));
        let mut thick = flat(50.0);
        thick.add_box(V2::new(24.0, 0.0), V2::new(25.5, 50.0), 0.0, 3.0);
        let m = thick.finish();
        assert!(m.line_of_sight(at, eye(10.0, 10.0)));
        assert!(!m.line_of_sight(at, eye(40.0, 10.0)));
    }

    #[test]
    fn rays_leaving_the_map_see_open_air_outside() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
        let m = b.finish();
        assert!(m.line_of_sight(eye(40.0, 40.0), eye(60.0, 60.0)));
        assert!(m.line_of_sight(eye(-10.0, -10.0), eye(-5.0, -5.0)));
        assert!(!m.line_of_sight(eye(-5.0, 25.0), eye(55.0, 25.0)));
    }

    #[test]
    fn the_same_column_and_broken_points() {
        let m = flat(50.0).finish();
        assert!(m.line_of_sight(eye(5.1, 5.1), V3::new(5.2, 5.2, 1.0)));
        assert!(!m.line_of_sight(V3::new(f32::NAN, 5.0, 1.7), eye(10.0, 10.0)));
        assert!(!m.line_of_sight(eye(5.0, 5.0), V3::new(10.0, 10.0, f32::INFINITY)));
    }

    #[test]
    fn under_the_ground_is_not_seen_through() {
        let m = flat(50.0).finish();
        assert!(!m.line_of_sight(V3::new(10.0, 10.0, 1.0), V3::new(20.0, 10.0, -1.0)));
    }
}
```

In `heightmap/mod.rs`, add `mod sight;` under `mod build;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim heightmap::sight`
Expected: FAIL to compile (`no method named line_of_sight found for struct HeightMap`).

- [ ] **Step 3: Implement line of sight**

In `heightmap/mod.rs`, inside `impl Grid`, after `center`:

```rust
    /// Column `(ix, iy)`'s index, or `None` off the grid.
    pub(crate) fn at(&self, ix: i64, iy: i64) -> Option<usize> {
        (ix >= 0 && iy >= 0 && ix < self.nx as i64 && iy < self.ny as i64)
            .then(|| self.index(ix as u32, iy as u32))
    }
```

At the top of `heightmap/sight.rs`, above the tests:

```rust
//! Line of sight through the height map's columns (spec §4.2): a ray walks from column to column
//! along its ground line (Amanatides and Woo's grid walk), and in each it is blocked by any span
//! across the heights the ray passes there. Columns off the map are open air: outside a run's
//! bounds the bake knows of nothing.
//!
//! An end's own column is coarse (half a yard), so a hero pressed against a wall can share a
//! column with it. There, a span holding the end itself is passed over: the hero is beside the
//! wall, not inside it. A wall wholly inside that one column is the cost; walls are thicker than
//! that, or straddle a column edge, nearly everywhere.

use super::{HeightMap, Span};
use crate::V3;

impl HeightMap {
    /// Whether nothing solid stands between `a` and `b`. An end that is not a number sees
    /// nothing.
    pub fn line_of_sight(&self, a: V3, b: V3) -> bool {
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let g = self.grid;
        let (ax, ay) = ((a.x - g.origin.x) / g.cell, (a.y - g.origin.y) / g.cell);
        let (bx, by) = ((b.x - g.origin.x) / g.cell, (b.y - g.origin.y) / g.cell);
        let (dx, dy) = (bx - ax, by - ay);
        let start = (ax.floor() as i64, ay.floor() as i64);
        let end = (bx.floor() as i64, by.floor() as i64);
        let step_x: i64 = if dx > 0.0 { 1 } else { -1 };
        let step_y: i64 = if dy > 0.0 { 1 } else { -1 };
        let (mut next_x, delta_x) = edges(ax, dx);
        let (mut next_y, delta_y) = edges(ay, dy);
        let (mut ix, mut iy) = start;
        let mut t = 0.0f32;
        // One more than the columns the walk can visit, and two to spare against rounding.
        let cells = (end.0 - start.0).abs() + (end.1 - start.1).abs() + 3;
        for _ in 0..cells {
            let t_out = next_x.min(next_y).min(1.0);
            let za = a.z + (b.z - a.z) * t;
            let zb = a.z + (b.z - a.z) * t_out;
            let mut ends = [f32::NAN; 2];
            if (ix, iy) == start {
                ends[0] = a.z;
            }
            if (ix, iy) == end {
                ends[1] = b.z;
            }
            if self.solid_across(ix, iy, za.min(zb), za.max(zb), ends) {
                return false;
            }
            if (ix, iy) == end || t_out >= 1.0 {
                break;
            }
            if next_x < next_y {
                ix += step_x;
                t = next_x;
                next_x += delta_x;
            } else {
                iy += step_y;
                t = next_y;
                next_y += delta_y;
            }
        }
        true
    }

    /// Whether column `(ix, iy)` is solid anywhere from `lo` to `hi`, passing over a span that
    /// holds one of `ends` (not a number for none).
    fn solid_across(&self, ix: i64, iy: i64, lo: f32, hi: f32, ends: [f32; 2]) -> bool {
        let Some(col) = self.grid.at(ix, iy) else {
            return false;
        };
        let holds = |s: &Span, z: f32| s.lo <= z && z <= s.hi;
        self.spans_at(col)
            .as_slice()
            .iter()
            .any(|s| s.lo <= hi && s.hi >= lo && !holds(s, ends[0]) && !holds(s, ends[1]))
    }
}

/// How far along the ray (0..1) the first column edge past `from` lies, moving `d` columns over
/// the whole ray, and how far apart edges are; infinite when the ray does not move that way.
fn edges(from: f32, d: f32) -> (f32, f32) {
    if d > 0.0 {
        ((from.floor() + 1.0 - from) / d, 1.0 / d)
    } else if d < 0.0 {
        ((from.floor() - from) / d, -1.0 / d)
    } else {
        (f32::INFINITY, f32::INFINITY)
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim heightmap`
Expected: PASS (11 from Task 1, 10 sight tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim/src/heightmap
git commit -m "nightfall-sim: line of sight through the height map's columns

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Paths

**Files:**
- Create: `crates/nightfall-sim/src/heightmap/path.rs`
- Modify: `crates/nightfall-sim/src/heightmap/mod.rs` (`mod path;`, `pub use path::Navigator;`), `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `HeightMap`, `Grid::at`, `level_within`, `MAX_STEP`, `V2`, `V3` (Tasks 1–2).
- Produces:
  - `nightfall_sim::Navigator` with `new(Arc<HeightMap>) -> Self`, `map(&self) -> &Arc<HeightMap>`, `cached(&self) -> usize`, `find_path(&mut self, from: V3, to: V3, cache: bool, budget: usize) -> Option<Arc<[V3]>>` (the start left out, the goal's floor last).
  - `HeightMap::walk_line(&self, a: V3, b: V3) -> bool`; crate-only `HeightMap::walkable(&self, ix: i64, iy: i64, z: f32) -> bool`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/heightmap/path.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::testmap::{flat, house};
    use super::super::HeightMapBuilder;
    use super::*;

    const BIG: usize = 40_000;

    fn nav(b: HeightMapBuilder) -> Navigator {
        Navigator::new(Arc::new(b.finish()))
    }

    /// Every quarter yard along the path, from `from`.
    fn samples(from: V3, path: &[V3]) -> Vec<V2> {
        let mut out = vec![from.ground()];
        let mut at = from.ground();
        for p in path {
            let d = p.ground() - at;
            let n = ((d.length() / 0.25).ceil() as usize).max(1);
            for k in 1..=n {
                out.push(at + d * (k as f32 / n as f32));
            }
            at = p.ground();
        }
        out
    }

    fn length(from: V3, path: &[V3]) -> f32 {
        let mut at = from.ground();
        let mut len = 0.0;
        for p in path {
            len += at.distance(p.ground());
            at = p.ground();
        }
        len
    }

    fn inside(p: V2, min: V2, max: V2) -> bool {
        p.x > min.x && p.x < max.x && p.y > min.y && p.y < max.y
    }

    #[test]
    fn open_ground_is_a_few_straight_legs() {
        let mut n = nav(flat(50.0));
        let from = V3::new(5.0, 5.0, 0.0);
        let path = n.find_path(from, V3::new(40.0, 30.0, 0.0), false, BIG).unwrap();
        assert!(path.len() <= 4, "{} legs", path.len());
        // No longer than the grid's own octile path (45.4 yd), and near the straight 43.
        assert!(length(from, &path) <= 46.0);
        assert!(path.last().unwrap().ground().distance(V2::new(40.0, 30.0)) < 0.5);
        assert!(path.windows(2).all(|w| n.map().walk_line(w[0], w[1])));
    }

    #[test]
    fn a_path_goes_around_a_house_not_through_it() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
        let mut n = nav(b);
        let from = V3::new(25.0, 10.0, 0.0);
        let path = n.find_path(from, V3::new(25.0, 40.0, 0.0), false, BIG).unwrap();
        for s in samples(from, &path) {
            assert!(!inside(s, V2::new(20.0, 20.0), V2::new(30.0, 30.0)), "{s:?}");
        }
        assert!(length(from, &path) > 31.0);
    }

    #[test]
    fn a_path_into_a_house_goes_through_its_door() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), Some(25.0), false);
        let mut n = nav(b);
        let from = V3::new(10.0, 25.0, 0.0);
        let path = n.find_path(from, V3::new(25.0, 25.0, 0.0), false, BIG).unwrap();
        let crossing: Vec<V2> = samples(from, &path)
            .into_iter()
            .filter(|s| s.x >= 20.0 && s.x <= 20.5)
            .collect();
        assert!(!crossing.is_empty());
        assert!(crossing.iter().all(|s| s.y > 23.75 && s.y < 26.25), "{crossing:?}");
    }

    #[test]
    fn a_closed_house_cannot_be_entered() {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
        let mut n = nav(b);
        assert!(n
            .find_path(V3::new(10.0, 25.0, 0.0), V3::new(25.0, 25.0, 0.0), false, BIG)
            .is_none());
    }

    #[test]
    fn stairs_lead_to_the_upper_floor() {
        let mut b = flat(30.0);
        for k in 0..6 {
            let x = 10.0 + 0.5 * k as f32;
            b.add_box(V2::new(x, 10.0), V2::new(x + 0.5, 12.0), 0.0, 0.5 * (k + 1) as f32);
        }
        b.add_box(V2::new(13.0, 10.0), V2::new(16.0, 16.0), 2.8, 3.0);
        let mut n = nav(b);
        let path = n
            .find_path(V3::new(5.0, 11.0, 0.0), V3::new(15.0, 14.0, 3.0), false, BIG)
            .unwrap();
        assert!((path.last().unwrap().z - 3.0).abs() < 0.01);
    }

    #[test]
    fn a_ledge_too_high_is_not_climbed() {
        let mut b = flat(50.0);
        b.add_box(V2::new(30.0, 30.0), V2::new(35.0, 35.0), 0.0, 1.0);
        let mut n = nav(b);
        assert!(n
            .find_path(V3::new(20.0, 20.0, 0.0), V3::new(32.0, 32.0, 1.0), false, BIG)
            .is_none());
    }

    #[test]
    fn an_unreachable_goal_gives_up_at_the_budget() {
        let mut b = flat(400.0);
        house(&mut b, V2::new(200.0, 200.0), V2::new(210.0, 210.0), None, false);
        let mut n = nav(b);
        assert!(n
            .find_path(V3::new(10.0, 10.0, 0.0), V3::new(205.0, 205.0, 0.0), false, 5_000)
            .is_none());
    }

    #[test]
    fn a_far_goal_needs_a_big_enough_budget() {
        let mut n = nav(flat(400.0));
        let (from, to) = (V3::new(10.0, 10.0, 0.0), V3::new(390.0, 390.0, 0.0));
        assert!(n.find_path(from, to, false, 50).is_none());
        assert!(n.find_path(from, to, false, BIG).is_some());
    }

    #[test]
    fn a_path_never_leaves_the_map() {
        let mut n = nav(flat(50.0));
        let from = V3::new(0.75, 5.0, 0.0);
        let path = n.find_path(from, V3::new(0.75, 45.0, 0.0), false, BIG).unwrap();
        assert!(samples(from, &path).iter().skip(1).all(|s| s.x >= 0.5));
        assert!(n.find_path(from, V3::new(60.0, 25.0, 0.0), false, BIG).is_none());
        assert!(n.find_path(V3::new(-3.0, 25.0, 0.0), from, false, BIG).is_none());
    }

    #[test]
    fn a_unit_keeps_a_cell_from_a_wall() {
        let mut b = flat(50.0);
        b.add_box(V2::new(20.0, 0.0), V2::new(20.5, 50.0), 0.0, 3.0);
        let m = b.finish();
        assert!(!m.walkable(39, 10, 0.0));
        assert!(m.walkable(38, 10, 0.0));
    }

    #[test]
    fn the_cache_hands_out_the_same_path() {
        let mut n = nav(flat(50.0));
        let (from, to) = (V3::new(5.0, 5.0, 0.0), V3::new(40.0, 30.0, 0.0));
        let a = n.find_path(from, to, true, BIG).unwrap();
        let b = n.find_path(from, to, true, BIG).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(n.cached(), 1);
        n.find_path(from, V3::new(30.0, 40.0, 0.0), false, BIG).unwrap();
        assert_eq!(n.cached(), 1);
    }

    #[test]
    fn the_same_search_gives_the_same_path() {
        let build = || {
            let mut b = flat(50.0);
            house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
            nav(b)
        };
        let (from, to) = (V3::new(25.0, 10.0, 0.0), V3::new(25.0, 40.0, 0.0));
        assert_eq!(
            build().find_path(from, to, false, BIG),
            build().find_path(from, to, false, BIG)
        );
    }
}
```

In `heightmap/mod.rs`: add `mod path;` under `mod build;`, and `pub use path::Navigator;` under `pub use build::HeightMapBuilder;`. In `lib.rs` add `Navigator` to the `pub use heightmap::{…}` list.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim heightmap::path`
Expected: FAIL to compile (`cannot find type Navigator`).

- [ ] **Step 3: Implement paths**

At the top of `heightmap/path.rs`, above the tests:

```rust
//! Paths over the height map's floors (spec §4.2): A* from floor to floor, a step to a
//! neighbouring column allowed when its floor is within [`MAX_STEP`] of this one, then smoothed
//! into straight legs a unit can walk ([`HeightMap::walk_line`]). A unit keeps one column clear
//! of anything it cannot step onto, so it walks beside a wall rather than half inside it, and
//! never through a doorway narrower than three columns.
//!
//! Every search has a budget of floors to look at: a goal shut away (a sealed house, a cave the
//! map has no way into) costs at most that, never a frozen host. [`Navigator`] keeps the paths
//! asked for again and again (a patrol's legs) so each is searched once.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use super::{HeightMap, MAX_STEP};
use crate::{V2, V3};

/// A floor: its column, and which of the column's floors (bottom first).
type Node = u64;

/// The eight neighbours, the four straight ones first.
const NEIGHBOURS: [(i64, i64); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];
/// The cost of a straight and a diagonal step (√2 ≈ 1.4), in tenths of a column: whole numbers,
/// so the same search always ends the same way.
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;
/// How far (columns) a start or goal off a walkable floor looks for one.
const SNAP_REACH: i64 = 4;
/// How far up or down (yards) a start's or goal's floor may be from the height asked.
const SNAP_HEIGHT: f32 = 1.5;
/// The most columns one straight leg of a smoothed path spans: it bounds the smoothing's work.
const LEG_CELLS: usize = 40;
/// Paths kept before the cache starts over.
const CACHE_CAP: usize = 1024;

fn node(col: usize, level: usize) -> Node {
    (col as u64) << 8 | level as u64
}

fn node_col(n: Node) -> usize {
    (n >> 8) as usize
}

fn node_level(n: Node) -> usize {
    (n & 0xFF) as usize
}

/// The octile distance between two columns in [`STRAIGHT`] units: never more than a path's cost.
fn octile(dx: i64, dy: i64) -> u32 {
    let (a, b) = (dx.unsigned_abs(), dy.unsigned_abs());
    let (lo, hi) = (a.min(b), a.max(b));
    (u64::from(DIAGONAL) * lo + u64::from(STRAIGHT) * (hi - lo)) as u32
}

impl HeightMap {
    /// Column `(ix, iy)`'s floor nearest `z` within `reach`, as (its level, its height); `None`
    /// off the map, on a blocked column, or with no floor in reach.
    fn level_near(&self, ix: i64, iy: i64, z: f32, reach: f32) -> Option<(usize, f32)> {
        let col = self.grid.at(ix, iy)?;
        if self.is_blocked(col) {
            return None;
        }
        self.level_within(col, z, reach)
    }

    /// Whether a unit can stand on the floor at `z` in column `(ix, iy)`: it is there, and every
    /// neighbour has a floor it could step to, so nothing it cannot stand on is within a column.
    pub(crate) fn walkable(&self, ix: i64, iy: i64, z: f32) -> bool {
        self.level_near(ix, iy, z, MAX_STEP).is_some()
            && NEIGHBOURS
                .iter()
                .all(|&(dx, dy)| self.level_near(ix + dx, iy + dy, z, MAX_STEP).is_some())
    }

    fn node_cell(&self, n: Node) -> (i64, i64) {
        let (ix, iy) = self.grid.cell_of(node_col(n));
        (i64::from(ix), i64::from(iy))
    }

    fn node_z(&self, n: Node) -> f32 {
        self.floors_at(node_col(n)).as_slice()[node_level(n)]
    }

    fn node_point(&self, n: Node) -> V3 {
        let (ix, iy) = self.grid.cell_of(node_col(n));
        V3::at(self.grid.center(ix, iy), self.node_z(n))
    }

    /// The walkable floor nearest `p`: its own column's if a unit can stand there, else the
    /// nearest within [`SNAP_REACH`] columns.
    fn snap(&self, p: V3) -> Option<Node> {
        if !p.is_finite() {
            return None;
        }
        let (cx, cy) = self.grid.column_of(p.ground())?;
        for r in 0..=SNAP_REACH {
            let mut best: Option<(f32, Node)> = None;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue;
                    }
                    let (ix, iy) = (i64::from(cx) + dx, i64::from(cy) + dy);
                    let Some((level, z)) = self.level_near(ix, iy, p.z, SNAP_HEIGHT) else {
                        continue;
                    };
                    if !self.walkable(ix, iy, z) {
                        continue;
                    }
                    let d = self.grid.center(ix as u32, iy as u32).distance(p.ground());
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, node(self.grid.index(ix as u32, iy as u32), level)));
                    }
                }
            }
            if let Some((_, n)) = best {
                return Some(n);
            }
        }
        None
    }

    /// A* from `start` to `goal`, looking at no more than `budget` floors. Ties go to the floor
    /// farther along, which keeps a search over open ground near its straight line.
    fn search(&self, start: Node, goal: Node, budget: usize) -> Option<Vec<Node>> {
        let (gx, gy) = self.node_cell(goal);
        let (sx, sy) = self.node_cell(start);
        let mut open = BinaryHeap::new();
        let mut best: HashMap<Node, (u32, Node)> = HashMap::new();
        let mut stand: HashMap<Node, bool> = HashMap::new();
        best.insert(start, (0, start));
        open.push(Reverse((octile(sx - gx, sy - gy), u32::MAX, start)));
        let mut looked = 0usize;
        while let Some(Reverse((_, inv_g, here))) = open.pop() {
            let g = u32::MAX - inv_g;
            if here == goal {
                return Some(walk_back(&best, start, goal));
            }
            if best.get(&here).is_some_and(|&(bg, _)| bg < g) {
                continue;
            }
            looked += 1;
            if looked > budget {
                return None;
            }
            let (ix, iy) = self.node_cell(here);
            let z = self.node_z(here);
            for &(dx, dy) in &NEIGHBOURS {
                let (nx, ny) = (ix + dx, iy + dy);
                let Some((level, nz)) = self.level_near(nx, ny, z, MAX_STEP) else {
                    continue;
                };
                let diagonal = dx != 0 && dy != 0;
                // No cutting a corner: both straight neighbours must be steppable too.
                if diagonal
                    && (self.level_near(ix + dx, iy, z, MAX_STEP).is_none()
                        || self.level_near(ix, iy + dy, z, MAX_STEP).is_none())
                {
                    continue;
                }
                let next = node(self.grid.index(nx as u32, ny as u32), level);
                if !*stand
                    .entry(next)
                    .or_insert_with(|| self.walkable(nx, ny, nz))
                {
                    continue;
                }
                let ng = g + if diagonal { DIAGONAL } else { STRAIGHT };
                if best.get(&next).is_none_or(|&(bg, _)| ng < bg) {
                    best.insert(next, (ng, here));
                    open.push(Reverse((ng + octile(nx - gx, ny - gy), u32::MAX - ng, next)));
                }
            }
        }
        None
    }

    /// The floors' points, straightened: from each point, the farthest later one (within
    /// [`LEG_CELLS`]) a unit can walk to in a straight line. The first point is left out.
    fn smooth(&self, nodes: &[Node]) -> Vec<V3> {
        let points: Vec<V3> = nodes.iter().map(|&n| self.node_point(n)).collect();
        if points.len() == 1 {
            return points;
        }
        let mut out = Vec::new();
        let mut anchor = 0;
        while anchor + 1 < points.len() {
            let mut reach = anchor + 1;
            while reach + 1 < points.len()
                && reach + 1 - anchor <= LEG_CELLS
                && self.walk_line(points[anchor], points[reach + 1])
            {
                reach += 1;
            }
            out.push(points[reach]);
            anchor = reach;
        }
        out
    }

    /// Whether a unit can walk straight from `a` to `b`: every quarter yard of the way has a
    /// floor it can stand on within [`MAX_STEP`] of the last, and it arrives at `b`'s height.
    pub fn walk_line(&self, a: V3, b: V3) -> bool {
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let d = b.ground() - a.ground();
        let steps = ((d.length() / (self.grid.cell * 0.5)).ceil() as usize).max(1);
        let mut z = a.z;
        for k in 1..=steps {
            let p = a.ground() + d * (k as f32 / steps as f32);
            let Some((ix, iy)) = self.grid.column_of(p) else {
                return false;
            };
            let (ix, iy) = (i64::from(ix), i64::from(iy));
            let Some((_, fz)) = self.level_near(ix, iy, z, MAX_STEP) else {
                return false;
            };
            if !self.walkable(ix, iy, fz) {
                return false;
            }
            z = fz;
        }
        (z - b.z).abs() <= MAX_STEP
    }
}

/// The floors from `start` to `goal` by the parents A* left.
fn walk_back(best: &HashMap<Node, (u32, Node)>, start: Node, goal: Node) -> Vec<Node> {
    let mut out = vec![goal];
    let mut at = goal;
    while at != start {
        at = best[&at].1;
        out.push(at);
    }
    out.reverse();
    out
}

/// Paths over one height map, with the ones asked for again kept.
pub struct Navigator {
    map: Arc<HeightMap>,
    cache: HashMap<(Node, Node), Arc<[V3]>>,
}

impl Navigator {
    pub fn new(map: Arc<HeightMap>) -> Self {
        Self {
            map,
            cache: HashMap::new(),
        }
    }

    pub fn map(&self) -> &Arc<HeightMap> {
        &self.map
    }

    /// How many paths are kept.
    pub fn cached(&self) -> usize {
        self.cache.len()
    }

    /// A path from `from` to `to`: the points to walk through, the start left out and the goal's
    /// floor last; `None` when either end has no walkable floor near it or no path is found
    /// within `budget` floors. With `cache`, the path is kept and handed out again for the same
    /// start and goal floors (spec §4.2: a patrol's legs).
    pub fn find_path(&mut self, from: V3, to: V3, cache: bool, budget: usize) -> Option<Arc<[V3]>> {
        let map = self.map.clone();
        let start = map.snap(from)?;
        let goal = map.snap(to)?;
        if cache {
            if let Some(path) = self.cache.get(&(start, goal)) {
                return Some(path.clone());
            }
        }
        let nodes = map.search(start, goal, budget)?;
        let path: Arc<[V3]> = map.smooth(&nodes).into();
        if cache {
            if self.cache.len() >= CACHE_CAP {
                self.cache.clear();
            }
            self.cache.insert((start, goal), path.clone());
        }
        Some(path)
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim heightmap`
Expected: PASS (21 from Tasks 1–2, 12 path tests). `an_unreachable_goal_gives_up_at_the_budget` and `a_far_goal_needs_a_big_enough_budget` each finish in well under a second.

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim/src/heightmap crates/nightfall-sim/src/lib.rs
git commit -m "nightfall-sim: paths over the height map's floors, smoothed and cached

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The height map as bytes

**Files:**
- Create: `crates/nightfall-sim/src/heightmap/codec.rs`
- Modify: `crates/nightfall-sim/src/heightmap/mod.rs` (`mod codec;`)

**Interfaces:**
- Consumes: `HeightMap`, `Grid`, `Span` (Task 1).
- Produces: `HeightMap::to_bytes(&self) -> Vec<u8>` and `HeightMap::from_bytes(&[u8]) -> Result<HeightMap, String>`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/heightmap/codec.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::super::testmap::{flat, house};
    use super::super::HeightMap;
    use crate::V2;

    fn town() -> HeightMap {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), Some(25.0), true);
        b.block_box(V2::new(0.0, 0.0), V2::new(3.0, 3.0));
        b.set_ground(7, f32::NAN);
        b.finish()
    }

    #[test]
    fn a_map_reads_back_as_it_was_written() {
        let m = town();
        assert_eq!(HeightMap::from_bytes(&m.to_bytes()).unwrap(), m);
    }

    #[test]
    fn a_cut_short_or_foreign_file_is_refused() {
        let bytes = town().to_bytes();
        assert!(HeightMap::from_bytes(&bytes[..bytes.len() - 1]).is_err());
        assert!(HeightMap::from_bytes(&[]).is_err());
        let mut foreign = bytes.clone();
        foreign[0] = b'X';
        assert!(HeightMap::from_bytes(&foreign).is_err());
        let mut newer = bytes.clone();
        newer[4] = 99;
        assert!(HeightMap::from_bytes(&newer).is_err());
        let mut longer = bytes;
        longer.push(0);
        assert!(HeightMap::from_bytes(&longer).is_err());
    }

    #[test]
    fn a_header_claiming_billions_of_columns_is_refused_before_allocating() {
        let mut bytes = town().to_bytes();
        // `nx` follows the magic, the version, the origin and the cell: bytes 20..24.
        bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(HeightMap::from_bytes(&bytes).is_err());
    }

    #[test]
    fn a_span_that_is_not_a_number_is_refused() {
        let m = town();
        let mut bytes = m.to_bytes();
        // The last span's `hi` is the file's last four bytes.
        let n = bytes.len();
        bytes[n - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(HeightMap::from_bytes(&bytes).is_err());
    }
}
```

Add `mod codec;` under `mod build;` in `heightmap/mod.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim heightmap::codec`
Expected: FAIL to compile (`no method named to_bytes found`).

- [ ] **Step 3: Implement the codec**

At the top of `heightmap/codec.rs`:

```rust
//! The height map as bytes, for the bake's cache: a header, the terrain heights, the blocked
//! bits and the span lists of the columns that have them, all little-endian. Reading checks
//! everything it can before it allocates, so a cut-short or foreign file is refused, never
//! trusted.

use super::{Grid, HeightMap, Span};
use crate::V2;

const MAGIC: &[u8; 4] = b"NFHM";
/// Bumped whenever the layout changes.
const VERSION: u32 = 1;
/// No map comes near this many columns (Duskwood's bounds are 17 million); a header past it is
/// not ours.
const MAX_COLUMNS: u64 = 200_000_000;

impl HeightMap {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            32 + self.ground.len() * 2
                + self.blocked.len() * 8
                + self.cols.len() * 8
                + self.spans.len() * 8,
        );
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        for v in [self.grid.origin.x, self.grid.origin.y, self.grid.cell] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.grid.nx.to_le_bytes());
        out.extend_from_slice(&self.grid.ny.to_le_bytes());
        for g in &self.ground {
            out.extend_from_slice(&g.to_le_bytes());
        }
        for w in &self.blocked {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(&(self.cols.len() as u32).to_le_bytes());
        for c in &self.cols {
            out.extend_from_slice(&c.to_le_bytes());
        }
        for s in &self.starts {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.extend_from_slice(&(self.spans.len() as u32).to_le_bytes());
        for s in &self.spans {
            out.extend_from_slice(&s.lo.to_le_bytes());
            out.extend_from_slice(&s.hi.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<HeightMap, String> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err("not a Nightfall height map".into());
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(format!("height map version {version}, not {VERSION}"));
        }
        let origin = V2::new(r.f32()?, r.f32()?);
        let cell = r.f32()?;
        let (nx, ny) = (r.u32()?, r.u32()?);
        if !origin.is_finite() || !(cell > 0.0 && cell.is_finite()) || nx == 0 || ny == 0 {
            return Err("the height map's header is broken".into());
        }
        let n = u64::from(nx) * u64::from(ny);
        if n > MAX_COLUMNS {
            return Err(format!("a height map of {n} columns is not ours"));
        }
        let n = n as usize;
        let ground = r.vec(n, 2, |b| i16::from_le_bytes([b[0], b[1]]))?;
        let blocked = r.vec(n.div_ceil(64), 8, |b| {
            u64::from_le_bytes(b.try_into().expect("eight bytes"))
        })?;
        let count = r.u32()? as usize;
        if count > n {
            return Err("more columns with spans than columns".into());
        }
        let cols = r.vec(count, 4, le_u32)?;
        if cols.windows(2).any(|w| w[0] >= w[1]) || cols.last().is_some_and(|&c| c as usize >= n)
        {
            return Err("the height map's columns are out of order".into());
        }
        let starts = r.vec(count + 1, 4, le_u32)?;
        let span_count = r.u32()? as usize;
        if starts[0] != 0
            || starts[count] as usize != span_count
            || starts.windows(2).any(|w| w[0] >= w[1])
        {
            return Err("the height map's span lists are out of order".into());
        }
        let spans = r.vec(span_count, 8, |b| Span {
            lo: f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            hi: f32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        })?;
        if spans
            .iter()
            .any(|s| !s.lo.is_finite() || !s.hi.is_finite() || s.lo > s.hi)
        {
            return Err("a span in the height map is not a number".into());
        }
        if r.at != bytes.len() {
            return Err("bytes left over after the height map".into());
        }
        Ok(HeightMap {
            grid: Grid {
                origin,
                cell,
                nx,
                ny,
            },
            ground,
            cols,
            starts,
            spans,
            blocked,
        })
    }
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Bytes read in order; every read checks there are enough first.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or("the height map is cut short")?;
        let s = &self.bytes[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.take(4).map(le_u32)
    }

    fn f32(&mut self) -> Result<f32, String> {
        self.u32().map(f32::from_bits)
    }

    /// `count` items of `size` bytes each; the bytes are checked to be there before the vector
    /// is made, so a lying count costs nothing.
    fn vec<T>(&mut self, count: usize, size: usize, read: impl Fn(&[u8]) -> T) -> Result<Vec<T>, String> {
        let len = count.checked_mul(size).ok_or("the height map is cut short")?;
        Ok(self.take(len)?.chunks_exact(size).map(read).collect())
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim heightmap`
Expected: PASS (33 from Tasks 1–3, 4 codec tests).

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim/src/heightmap
git commit -m "nightfall-sim: the height map as bytes, checked as it is read back

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Content: enemy kinds, where they go, the world's rules

**Files:**
- Create: `content/nightfall/enemies.ron`, `content/nightfall/spawns.ron`
- Modify: `content/nightfall/rules.ron`, `crates/nightfall-sim/src/content.rs`, `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/src/world.rs` (test fixture), `crates/nightfall-sim/src/messages.rs` (test fixture), `crates/nightfall-sim/tests/shipped_content.rs`
- Modify (so the client still builds): `crates/benilla-app/src/nightfall/mod.rs` (`prepare`, `content_texts`), `crates/benilla-app/src/nightfall/session.rs` (`our_content_hash`)

**Interfaces:**
- Consumes: `Bounds`, `V2`, `Content`, `ContentSources`, `ContentError` (foundation Task 3).
- Produces:
  - `EnemyDef { key: String, name: String, display: u32, scale: f32, level: u32, hp: f32, walk_speed: f32, run_speed: f32 }`.
  - `Behaviour::{Sentry { at: (f32, f32), facing: f32 }, Patrol { route: Vec<(f32, f32)>, pause: f32 }, Wander { center: (f32, f32), radius: f32 }}`.
  - `SpawnDef { place: String, kind: String, count: u32, z: Option<f32>, behaviour: Behaviour }`.
  - `WorldRules { share_sight_within: f32, paths_per_step: u32, wander_pause: (f32, f32) }` and `Rules::world` (10.0, 4, (3.0, 8.0) when absent).
  - `ContentSources` gains `enemies: &str` and `spawns: &str`; `Content` gains `enemies: Vec<EnemyDef>`, `spawns: Vec<SpawnDef>` and `enemy(&self, key: &str) -> Option<usize>`.
  - Client: `nightfall::content_texts() -> &'static [String; 5]` (classes, map, rules, enemies, spawns).

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/content.rs`'s `tests` module, add below `RULES`:

```rust
    const ENEMIES: &str = r#"[
        (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
        (key: "spider", name: "Spider", display: 955, scale: 0.9, level: 26, hp: 800.0, walk_speed: 2.2, run_speed: 7.0),
    ]"#;
    const SPAWNS: &str = r#"[
        (place: "Gate", kind: "skeleton", behaviour: Sentry(at: (60.0, 50.0), facing: 90.0)),
        (place: "Road", kind: "skeleton", behaviour: Patrol(route: [(20.0, 20.0), (80.0, 20.0)])),
        (place: "Nest", kind: "spider", count: 3, z: Some(1.0), behaviour: Wander(center: (30.0, 70.0), radius: 10.0)),
    ]"#;
```

Replace the `load` helper with:

```rust
    fn load(classes: &str, map: &str, rules: &str) -> Result<Content, ContentError> {
        load_all(classes, map, rules, ENEMIES, SPAWNS)
    }

    fn load_all(
        classes: &str,
        map: &str,
        rules: &str,
        enemies: &str,
        spawns: &str,
    ) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes,
            map,
            rules,
            enemies,
            spawns,
        })
    }
```

and add these tests:

```rust
    #[test]
    fn enemies_and_their_places_load() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        assert_eq!(c.enemy("spider"), Some(1));
        assert_eq!(c.enemy("worgen"), None);
        assert_eq!(c.enemies[0].scale, 1.0);
        assert_eq!(c.spawns.len(), 3);
        assert_eq!(c.spawns[0].count, 1);
        assert_eq!(c.spawns[2].z, Some(1.0));
        match &c.spawns[1].behaviour {
            Behaviour::Patrol { route, pause } => {
                assert_eq!(route.len(), 2);
                assert_eq!(*pause, 2.0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_world_rules_default_when_absent_and_are_checked_when_given() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        assert_eq!(c.rules.world.share_sight_within, 10.0);
        assert_eq!(c.rules.world.paths_per_step, 4);
        let rules = RULES.replacen(
            "(hero_level: 30,",
            "(hero_level: 30, world: (share_sight_within: 10.0, paths_per_step: 0, wander_pause: (3.0, 8.0)),",
            1,
        );
        assert!(matches!(
            load(CLASSES, MAP, &rules),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_spawn_of_an_unknown_kind_is_refused() {
        let spawns = SPAWNS.replace("kind: \"spider\"", "kind: \"wolf\"");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &spawns),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_spawn_outside_the_bounds_is_refused() {
        let post = SPAWNS.replace("at: (60.0, 50.0)", "at: (160.0, 50.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &post),
            Err(ContentError::Invalid(_))
        ));
        let route = SPAWNS.replace("(80.0, 20.0)", "(80.0, -20.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &route),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_patrol_needs_two_points_and_a_wander_a_radius() {
        let one = SPAWNS.replace("[(20.0, 20.0), (80.0, 20.0)]", "[(20.0, 20.0)]");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &one),
            Err(ContentError::Invalid(_))
        ));
        let none = SPAWNS.replace("radius: 10.0", "radius: 0.0");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &none),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn an_enemy_kind_twice_or_without_health_is_refused() {
        let twice = ENEMIES.replace("\"spider\", name", "\"skeleton\", name");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, &twice, SPAWNS),
            Err(ContentError::Invalid(_))
        ));
        let dead = ENEMIES.replace("hp: 900.0", "hp: 0.0");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, &dead, SPAWNS),
            Err(ContentError::Invalid(_))
        ));
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim content`
Expected: FAIL to compile (`struct ContentSources has no field named enemies`, `cannot find type Behaviour`).

- [ ] **Step 3: Implement the content**

In `crates/nightfall-sim/src/content.rs`, change the module doc's first line to `//! Nightfall's content: its classes, its map, its enemies and where they go, and its rules, read`. Replace `pub struct Rules { … }` with the version below and add the new types after it:

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct Rules {
    /// Every hero's level: fixed, there is no levelling in a run.
    pub hero_level: u32,
    pub atmosphere: Atmosphere,
    /// The world's knowledge (spec §4); its defaults when `rules.ron` leaves it out.
    #[serde(default)]
    pub world: WorldRules,
}

/// How the host uses what it knows of the world.
#[derive(Clone, Debug, Deserialize)]
pub struct WorldRules {
    /// A teammate this near (yards) shares what they see (spec §4.3).
    pub share_sight_within: f32,
    /// The most path searches the host makes a step, so many enemies choosing a way at once
    /// never stall it.
    pub paths_per_step: u32,
    /// How long a wanderer rests between walks, seconds (shortest, longest).
    pub wander_pause: (f32, f32),
}

impl Default for WorldRules {
    fn default() -> Self {
        Self {
            share_sight_within: 10.0,
            paths_per_step: 4,
            wander_pause: (3.0, 8.0),
        }
    }
}

/// A kind of enemy: one of Duskwood's creatures (spec §7.1).
#[derive(Clone, Debug, Deserialize)]
pub struct EnemyDef {
    pub key: String,
    pub name: String,
    /// `CreatureDisplayInfo.dbc` id: the model.
    pub display: u32,
    #[serde(default = "one_f32")]
    pub scale: f32,
    pub level: u32,
    pub hp: f32,
    /// Walking (patrols, wanderers), yards a second.
    pub walk_speed: f32,
    /// Running, for the chases of later sub-projects, yards a second.
    pub run_speed: f32,
}

fn one_f32() -> f32 {
    1.0
}

fn one_u32() -> u32 {
    1
}

fn two_f32() -> f32 {
    2.0
}

/// What a group of enemies does (spec §7.1). Points are WoW `(x, y)` yards.
#[derive(Clone, Debug, Deserialize)]
pub enum Behaviour {
    /// Stands at a post, facing `facing` degrees (0 north, 90 west).
    Sentry { at: (f32, f32), facing: f32 },
    /// Walks its route point by point and back to the first, resting `pause` seconds at each.
    Patrol {
        route: Vec<(f32, f32)>,
        #[serde(default = "two_f32")]
        pause: f32,
    },
    /// Roams within `radius` yards of `center`, resting between walks.
    Wander { center: (f32, f32), radius: f32 },
}

/// A group of enemies at a place.
#[derive(Clone, Debug, Deserialize)]
pub struct SpawnDef {
    /// The point of interest, for the content's author and the host's warnings.
    pub place: String,
    /// An [`EnemyDef::key`].
    pub kind: String,
    #[serde(default = "one_u32")]
    pub count: u32,
    /// The height to look for a floor near (a house's upper storey, a crypt); without it, the
    /// floor nearest the terrain.
    #[serde(default)]
    pub z: Option<f32>,
    pub behaviour: Behaviour,
}

/// The most enemies one spawn line places.
const MAX_SPAWN_COUNT: u32 = 20;
```

Change `ContentSources` and `Content` to:

```rust
/// The text of each content file.
pub struct ContentSources<'a> {
    pub classes: &'a str,
    pub map: &'a str,
    pub rules: &'a str,
    pub enemies: &'a str,
    pub spawns: &'a str,
}

#[derive(Clone, Debug)]
pub struct Content {
    pub classes: Vec<ClassDef>,
    pub map: MapDef,
    pub rules: Rules,
    pub enemies: Vec<EnemyDef>,
    pub spawns: Vec<SpawnDef>,
}
```

In `Content::from_ron`, after `let rules: Rules = parse("rules.ron", src.rules)?;` add:

```rust
        let enemies: Vec<EnemyDef> = parse("enemies.ron", src.enemies)?;
        let spawns: Vec<SpawnDef> = parse("spawns.ron", src.spawns)?;
```

and just before `Ok(Self {` add:

```rust
        for (i, e) in enemies.iter().enumerate() {
            if enemies[..i].iter().any(|d| d.key == e.key) {
                return invalid(format!("enemy \"{}\" is listed twice", e.key));
            }
            if !(e.hp > 0.0 && e.walk_speed > 0.0 && e.run_speed >= e.walk_speed && e.scale > 0.0) {
                return invalid(format!(
                    "enemies.ron: \"{}\" needs hp, walk_speed and scale above 0 and run_speed at least walk_speed",
                    e.key
                ));
            }
        }
        let inside = |(x, y): (f32, f32)| bounds.contains(V2::new(x, y));
        for s in &spawns {
            let who = format!("spawns.ron: \"{}\"", s.place);
            if !enemies.iter().any(|e| e.key == s.kind) {
                return invalid(format!("{who}: no enemy \"{}\" in enemies.ron", s.kind));
            }
            if s.count == 0 || s.count > MAX_SPAWN_COUNT {
                return invalid(format!("{who}: count must be 1 to {MAX_SPAWN_COUNT}"));
            }
            match &s.behaviour {
                Behaviour::Sentry { at, .. } => {
                    if !inside(*at) {
                        return invalid(format!("{who}: its post {at:?} is outside the bounds"));
                    }
                }
                Behaviour::Patrol { route, pause } => {
                    if route.len() < 2 {
                        return invalid(format!("{who}: a patrol needs 2 points or more"));
                    }
                    if let Some(p) = route.iter().find(|p| !inside(**p)) {
                        return invalid(format!("{who}: route point {p:?} is outside the bounds"));
                    }
                    if !(*pause >= 0.0) {
                        return invalid(format!("{who}: pause must be 0 or more"));
                    }
                }
                Behaviour::Wander { center, radius } => {
                    if !inside(*center) {
                        return invalid(format!("{who}: its centre {center:?} is outside the bounds"));
                    }
                    if !(*radius > 0.0) {
                        return invalid(format!("{who}: radius must be above 0"));
                    }
                }
            }
        }
        let w = &rules.world;
        if !(w.share_sight_within >= 0.0)
            || w.paths_per_step == 0
            || !(w.wander_pause.0 >= 0.0 && w.wander_pause.0 <= w.wander_pause.1)
        {
            return invalid(
                "rules.ron: world needs share_sight_within of 0 or more, paths_per_step of 1 or more and wander_pause (shortest, longest) from 0 up"
                    .into(),
            );
        }
```

Add `enemies,` and `spawns,` to the `Ok(Self { … })` literal after `rules,`. Add to `impl Content`:

```rust
    /// The enemy kind with this key, by index.
    pub fn enemy(&self, key: &str) -> Option<usize> {
        self.enemies.iter().position(|e| e.key == key)
    }
```

In `lib.rs`, change the content `pub use` to:

```rust
pub use content::{
    Atmosphere, Behaviour, ClassDef, Content, ContentError, ContentSources, EnemyDef, HeroLight,
    MapDef, Rules, SpawnDef, WeaponDef, WorldRules,
};
```

In `world.rs`'s tests, `content()`'s `ContentSources { … }` gains, after `rules: …,`:

```rust
                enemies: "[]",
                spawns: "[]",
```

In `messages.rs`'s test `a_full_partys_state_is_small`, the `ContentSources` gains:

```rust
                enemies: include_str!("../../../content/nightfall/enemies.ron"),
                spawns: include_str!("../../../content/nightfall/spawns.ron"),
```

- [ ] **Step 4: Write the shipped content**

`content/nightfall/enemies.ron`:

```ron
// Nightfall's enemy kinds: Duskwood's own creatures (spec §7.1). `display` is the
// CreatureDisplayInfo id, the same models the survivors content uses for them. `hp`, `walk_speed`
// and `run_speed` are starting values that combat (sub-project 4) tunes; speeds are yards a
// second (WoW's creatures walk at about 2.5 and run at 7).
[
    (key: "skeletal_warrior", name: "Skeletal Warrior", display: 200, scale: 1.2, level: 28,
     hp: 980.0, walk_speed: 2.5, run_speed: 7.0),
    (key: "nightbane_worgen", name: "Nightbane Worgen", display: 657, level: 27,
     hp: 940.0, walk_speed: 2.5, run_speed: 8.0),
    (key: "nightbane_shadow_weaver", name: "Nightbane Shadow Weaver", display: 203, level: 28,
     hp: 900.0, walk_speed: 2.5, run_speed: 7.5),
    (key: "venom_web_spider", name: "Venom Web Spider", display: 955, level: 26,
     hp: 860.0, walk_speed: 2.2, run_speed: 7.0),
    (key: "black_ravager", name: "Black Ravager", display: 741, level: 25,
     hp: 820.0, walk_speed: 3.0, run_speed: 8.0),
    (key: "fetid_corpse", name: "Fetid Corpse", display: 559, scale: 1.3, level: 30,
     hp: 1100.0, walk_speed: 1.8, run_speed: 5.0),
]
```

`content/nightfall/spawns.ron`:

```ron
// Where Nightfall's enemies stand and walk (spec §7.1): a sentry holds a post, a patrol walks its
// route round and round, wanderers roam an area. Points are WoW (x, y) yards; `facing` is degrees
// from north toward west; `z` (optional) picks the floor near that height, for an upper storey.
//
// Every group sits near the first entry for now, so a run meets them at once. Each is marked
// `measure:` with the place it belongs; move it there once measured in the game
// (`NIGHTFALL_TRACE_POS=1` logs where the hero stands, and
// `cargo run -p nightfall-bake --example buildings -- <x> <y> <radius>` lists the buildings
// around a point).
[
    // measure: Raven Hill cemetery's gate.
    (place: "Near the entry: a sentry", kind: "skeletal_warrior",
     behaviour: Sentry(at: (-10040.0, -800.0), facing: 180.0)),
    // measure: the road past Brightwood Grove.
    (place: "Near the entry: a patrol", kind: "nightbane_worgen",
     behaviour: Patrol(route: [(-10035.0, -775.0), (-10035.0, -815.0), (-10070.0, -815.0),
                               (-10070.0, -775.0)], pause: 2.0)),
    // measure: Twilight Grove's webs.
    (place: "Near the entry: spiders", kind: "venom_web_spider", count: 3,
     behaviour: Wander(center: (-10085.0, -790.0), radius: 18.0)),
    // measure: the Rotting Orchard.
    (place: "Near the entry: ravagers", kind: "black_ravager", count: 2,
     behaviour: Wander(center: (-10020.0, -760.0), radius: 20.0)),
]
```

In `content/nightfall/rules.ron`, after the `atmosphere: ( … ),` block and before the closing `)`:

```ron
    // What the host does with what it knows of the world (spec §4): how near a teammate shares
    // what they see (yards), how many path searches the host makes a step, and how long a
    // wanderer rests between walks (seconds, shortest and longest).
    world: (
        share_sight_within: 10.0,
        paths_per_step: 4,
        wander_pause: (3.0, 8.0),
    ),
```

In `crates/nightfall-sim/tests/shipped_content.rs`, replace `shipped()` and add a test:

```rust
pub fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/nightfall/classes.ron"),
        map: include_str!("../../../content/nightfall/map.ron"),
        rules: include_str!("../../../content/nightfall/rules.ron"),
        enemies: include_str!("../../../content/nightfall/enemies.ron"),
        spawns: include_str!("../../../content/nightfall/spawns.ron"),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn duskwoods_creatures_are_placed_inside_it() {
    let c = shipped();
    assert!(c.enemy("nightbane_worgen").is_some());
    assert!(!c.spawns.is_empty());
    assert_eq!(c.rules.world.share_sight_within, 10.0);
}
```

- [ ] **Step 5: Keep the client reading every file**

In `crates/benilla-app/src/nightfall/mod.rs` (as the foundation plan's Task 8 wrote it), replace the `CONTENT_TEXTS` static and `content_texts` with:

```rust
/// The content files' text (classes, map, rules, enemies, spawns), for the hash a client and host
/// compare.
static CONTENT_TEXTS: OnceLock<[String; 5]> = OnceLock::new();

pub(super) fn content_texts() -> &'static [String; 5] {
    CONTENT_TEXTS.get().expect("nightfall::prepare ran")
}
```

In `prepare`, after the `let rules = read(…)?;` statement add:

```rust
    let enemies = read(
        "enemies.ron",
        include_str!("../../../../content/nightfall/enemies.ron"),
    )?;
    let spawns = read(
        "spawns.ron",
        include_str!("../../../../content/nightfall/spawns.ron"),
    )?;
```

change the `ContentSources { … }` literal to:

```rust
    let content = Content::from_ron(ContentSources {
        classes: &classes,
        map: &map,
        rules: &rules,
        enemies: &enemies,
        spawns: &spawns,
    })
    .map_err(|e| e.to_string())?;
```

and the `CONTENT_TEXTS.set(…)` line to `let _ = CONTENT_TEXTS.set([classes, map, rules, enemies, spawns]);`.

In `crates/benilla-app/src/nightfall/session.rs`, replace `our_content_hash` with:

```rust
/// The content files' hash, which a client and its host must share.
fn our_content_hash() -> u64 {
    let texts: Vec<&str> = super::content_texts().iter().map(String::as_str).collect();
    content_hash(&texts)
}
```

- [ ] **Step 6: Run the tests and check the client builds**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test, 6 new content tests and `duskwoods_creatures_are_placed_inside_it`).
Run: `cargo check -p benilla-app`
Expected: builds.

- [ ] **Step 7: Commit**

```bash
git add content/nightfall crates/nightfall-sim crates/benilla-app/src/nightfall/mod.rs crates/benilla-app/src/nightfall/session.rs
git commit -m "Nightfall content: Duskwood's creatures, where they stand and walk, the world's rules

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `nightfall-bake`: placing models and rasterizing triangles

**Files:**
- Create: `crates/nightfall-bake/Cargo.toml`, `crates/nightfall-bake/src/lib.rs`, `crates/nightfall-bake/src/place.rs`, `crates/nightfall-bake/src/raster.rs`

**Interfaces:**
- Consumes: `Grid`, `HeightMapBuilder`, `Span`, `MERGE_GAP`, `CELL`, `V2`, `V3`, `HeightMap::line_of_sight` (Tasks 1–2).
- Produces:
  - `nightfall_bake::placement_point(position: [f32; 3], rotation_deg: [f32; 3], scale: f32, local: [f32; 3]) -> [f32; 3]`.
  - `nightfall_bake::Footprint` with `new(Grid) -> Self`, `add_triangle(&mut self, [[f32; 3]; 3])`, `len(&self) -> usize`, `is_empty(&self) -> bool`, `flush_into(&mut self, b: &mut HeightMapBuilder, solid: bool)`.

- [ ] **Step 1: Create the crate**

`crates/nightfall-bake/Cargo.toml`:

```toml
[package]
name = "nightfall-bake"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Bakes Nightfall's height map from the player's own WoW data: terrain, buildings and doodad hulls into nightfall-sim's columns, cached on disk"

[lints]
workspace = true

[features]
# `benilla-formats`'s `dev` (its project-folder fallback for finding WoW data), forwarded: every
# dependent takes `benilla-formats` without default features and forwards `dev`, or the player
# build would carry the source tree's path.
default = ["dev"]
dev = ["benilla-formats/dev"]

[dependencies]
anyhow.workspace = true
benilla-formats = { path = "../benilla-formats", default-features = false }
# The cache is deflated: most of Duskwood's columns are a handful of nearby heights.
flate2 = "1.1"
nightfall-sim = { path = "../nightfall-sim" }
```

`crates/nightfall-bake/src/lib.rs`:

```rust
//! Nightfall's height map, baked from the player's own WoW data (spec §4.1): the terrain of every
//! tile under a run's bounds, its buildings' (WMO) walking collision and its doodads' collision
//! hulls (a tree's trunk, never its canopy), rasterized into `nightfall-sim`'s columns. Only the
//! host needs the map; it is baked once and kept in a cache keyed by the data files, the bounds
//! and the props, so nothing Blizzard-derived ever ships.

mod place;

pub use place::placement_point;
```

(Step 3 adds the rasterizer's two lines.)

- [ ] **Step 2: Write `place.rs` with its tests**

`crates/nightfall-bake/src/place.rs`:

```rust
//! Where a point of a placed model lands in the world, in WoW's frame (`x` north, `y` west, `z`
//! up). The rotation is the map's MDDF/MODF Euler triple in degrees, composed as the engine
//! composes it (the `in_wow` half of `benilla_assets::coords::placement_rotation`, wowdev.wiki's
//! `createPlacementMatrix`): `Rx(90°)·Ry(ry−180°)·Rz(−rx)·Rx(rz−90°)`, applied right to left.
//! Written out here because the engine's version is in Bevy's types, which the bake does not use.

/// The world point of model-local point `local` for a model at `position`, turned by
/// `rotation_deg` and scaled by `scale`.
pub fn placement_point(
    position: [f32; 3],
    rotation_deg: [f32; 3],
    scale: f32,
    local: [f32; 3],
) -> [f32; 3] {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (rx, ry, rz) = (
        rotation_deg[0].to_radians(),
        rotation_deg[1].to_radians(),
        rotation_deg[2].to_radians(),
    );
    let rot_x = |p: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
    };
    let rot_y = |p: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
    };
    let rot_z = |p: [f32; 3], a: f32| {
        let (s, c) = a.sin_cos();
        [p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]]
    };
    let p = [local[0] * scale, local[1] * scale, local[2] * scale];
    let p = rot_x(p, rz - FRAC_PI_2);
    let p = rot_z(p, -rx);
    let p = rot_y(p, ry - PI);
    let p = rot_x(p, FRAC_PI_2);
    [position[0] + p[0], position[1] + p[1], position[2] + p[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    #[test]
    fn a_heading_of_180_leaves_the_model_as_it_is() {
        let p = placement_point([10.0, 20.0, 30.0], [0.0, 180.0, 0.0], 1.0, [1.0, 2.0, 3.0]);
        assert!(close(p, [11.0, 22.0, 33.0]), "{p:?}");
    }

    #[test]
    fn a_heading_turns_about_up_and_keeps_up_up() {
        let turned = placement_point([0.0; 3], [0.0, 270.0, 0.0], 1.0, [1.0, 0.0, 0.0]);
        assert!(close(turned, [0.0, 1.0, 0.0]), "{turned:?}");
        let up = placement_point([0.0; 3], [0.0, 270.0, 0.0], 1.0, [0.0, 0.0, 1.0]);
        assert!(close(up, [0.0, 0.0, 1.0]), "{up:?}");
    }

    #[test]
    fn scale_grows_the_model_about_its_origin() {
        let p = placement_point([5.0, 0.0, 0.0], [0.0, 180.0, 0.0], 2.0, [1.0, 0.0, 0.0]);
        assert!(close(p, [7.0, 0.0, 0.0]), "{p:?}");
    }
}
```

Run: `cargo test -p nightfall-bake place`
Expected: PASS (3 tests).

- [ ] **Step 3: Write the rasterizer's failing tests**

In `lib.rs` add `mod raster;` under `mod place;` and `pub use raster::Footprint;` under `pub use place::placement_point;`. Create `crates/nightfall-bake/src/raster.rs` with its tests (Step 5 adds the code above them):

```rust
#[cfg(test)]
mod tests {
    use nightfall_sim::{Grid, HeightMapBuilder, Span, CELL, V2, V3};

    use super::*;

    /// 20 yards across.
    fn grid() -> Grid {
        Grid {
            origin: V2::ZERO,
            cell: CELL,
            nx: 40,
            ny: 40,
        }
    }

    /// A box's twelve triangles.
    fn cuboid(lo: [f32; 3], hi: [f32; 3]) -> Vec<[[f32; 3]; 3]> {
        let c = |i: usize| {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        };
        let faces = [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ];
        faces
            .iter()
            .flat_map(|f| [[c(f[0]), c(f[1]), c(f[2])], [c(f[0]), c(f[2]), c(f[3])]])
            .collect()
    }

    #[test]
    fn a_wall_stands_in_the_columns_it_passes_through() {
        let mut b = HeightMapBuilder::flat(grid(), 0.0);
        let mut fp = Footprint::new(grid());
        // A wall along x = 10.1 from y = 5 to y = 15, 3 yards tall.
        fp.add_triangle([[10.1, 5.0, 0.0], [10.1, 15.0, 0.0], [10.1, 15.0, 3.0]]);
        fp.add_triangle([[10.1, 5.0, 0.0], [10.1, 15.0, 3.0], [10.1, 5.0, 3.0]]);
        fp.flush_into(&mut b, false);
        assert!(fp.is_empty());
        let m = b.finish();
        let g = m.grid();
        assert_eq!(m.floors_at(g.index(20, 20)).as_slice(), &[3.0]);
        assert_eq!(m.floors_at(g.index(18, 20)).as_slice(), &[0.0]);
        assert!(!m.line_of_sight(V3::new(8.0, 10.0, 1.7), V3::new(12.0, 10.0, 1.7)));
        assert!(m.line_of_sight(V3::new(8.0, 2.0, 1.7), V3::new(12.0, 2.0, 1.7)));
    }

    #[test]
    fn a_floor_overhead_is_a_second_storey() {
        let mut b = HeightMapBuilder::flat(grid(), 0.0);
        let mut fp = Footprint::new(grid());
        fp.add_triangle([[2.0, 2.0, 5.0], [6.0, 2.0, 5.0], [6.0, 6.0, 5.0]]);
        fp.add_triangle([[2.0, 2.0, 5.0], [6.0, 6.0, 5.0], [2.0, 6.0, 5.0]]);
        fp.flush_into(&mut b, false);
        let m = b.finish();
        assert_eq!(m.floors_at(m.grid().index(8, 8)).as_slice(), &[0.0, 5.0]);
        assert!(m.is_indoors(V2::new(4.25, 4.25), 0.0));
    }

    #[test]
    fn a_hull_is_solid_through_and_a_building_is_hollow() {
        let col = grid().index(7, 7);
        let mut fp = Footprint::new(grid());
        let mut solid = HeightMapBuilder::new(grid());
        for t in cuboid([3.0, 3.0, 0.0], [5.0, 5.0, 2.0]) {
            fp.add_triangle(t);
        }
        fp.flush_into(&mut solid, true);
        assert_eq!(
            solid.finish().spans_at(col).as_slice(),
            &[Span { lo: 0.0, hi: 2.0 }]
        );
        let mut hollow = HeightMapBuilder::new(grid());
        for t in cuboid([3.0, 3.0, 0.0], [5.0, 5.0, 2.0]) {
            fp.add_triangle(t);
        }
        fp.flush_into(&mut hollow, false);
        assert_eq!(
            hollow.finish().spans_at(col).as_slice(),
            &[Span { lo: 0.0, hi: 0.0 }, Span { lo: 2.0, hi: 2.0 }]
        );
    }

    #[test]
    fn a_triangle_off_the_grid_or_not_a_number_adds_nothing() {
        let mut fp = Footprint::new(grid());
        fp.add_triangle([[30.0, 30.0, 0.0], [35.0, 30.0, 0.0], [35.0, 35.0, 0.0]]);
        fp.add_triangle([[-9.0, -9.0, 0.0], [-5.0, -9.0, 0.0], [-5.0, -5.0, 0.0]]);
        fp.add_triangle([[1.0, 1.0, f32::NAN], [2.0, 1.0, 0.0], [2.0, 2.0, 0.0]]);
        assert_eq!(fp.len(), 0);
    }
}
```

- [ ] **Step 4: Run them to see them fail**

Run: `cargo test -p nightfall-bake raster`
Expected: FAIL to compile (`cannot find type Footprint`).

- [ ] **Step 5: Implement the rasterizer**

At the top of `crates/nightfall-bake/src/raster.rs`, above the tests:

```rust
//! Solid geometry into columns, as Recast's heightfield rasterizer does it: each triangle is
//! clipped to every column its footprint touches, and the part inside a column is solid from its
//! lowest to its highest point there. One model's triangles gather in a [`Footprint`] first, so
//! its many faces over a column reach the builder as one or two spans, not dozens.

use nightfall_sim::{Grid, HeightMapBuilder, Span, MERGE_GAP};

/// One model's triangles, column by column, until [`Footprint::flush_into`] hands them on.
pub struct Footprint {
    grid: Grid,
    spans: Vec<(u32, Span)>,
}

impl Footprint {
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            spans: Vec::new(),
        }
    }

    /// How many column pieces are gathered.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// A triangle (WoW world points), clipped into every column it touches. One with a point
    /// that is not a number is dropped, and the parts off the grid are too.
    pub fn add_triangle(&mut self, tri: [[f32; 3]; 3]) {
        if tri.iter().flatten().any(|v| !v.is_finite()) {
            return;
        }
        let g = self.grid;
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for p in &tri {
            for a in 0..2 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        let first = |v: f32, origin: f32| ((v - origin) / g.cell).floor().max(0.0) as i64;
        let last = |v: f32, origin: f32, n: u32| {
            ((v - origin) / g.cell).floor().min(n as f32 - 1.0) as i64
        };
        let (ix0, ix1) = (first(lo[0], g.origin.x), last(hi[0], g.origin.x, g.nx));
        let (iy0, iy1) = (first(lo[1], g.origin.y), last(hi[1], g.origin.y, g.ny));
        let mut a = [[0.0f32; 3]; 8];
        let mut b = [[0.0f32; 3]; 8];
        for iy in iy0..=iy1 {
            for ix in ix0..=ix1 {
                let x0 = g.origin.x + ix as f32 * g.cell;
                let y0 = g.origin.y + iy as f32 * g.cell;
                a[..3].copy_from_slice(&tri);
                let n = clip(&a[..3], &mut b, 0, x0, true);
                let n = clip(&b[..n], &mut a, 0, x0 + g.cell, false);
                let n = clip(&a[..n], &mut b, 1, y0, true);
                let n = clip(&b[..n], &mut a, 1, y0 + g.cell, false);
                if n == 0 {
                    continue;
                }
                let (zlo, zhi) = a[..n]
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), p| {
                        (l.min(p[2]), h.max(p[2]))
                    });
                let col = g.index(ix as u32, iy as u32) as u32;
                self.spans.push((col, Span { lo: zlo, hi: zhi }));
            }
        }
    }

    /// Everything gathered into the builder, then forgotten. `solid` (a doodad's collision hull,
    /// a closed solid) fills each column from the model's lowest to its highest point there; a
    /// building's spans only merge where they touch, so its rooms keep their air.
    pub fn flush_into(&mut self, b: &mut HeightMapBuilder, solid: bool) {
        self.spans
            .sort_unstable_by(|x, y| x.0.cmp(&y.0).then(x.1.lo.total_cmp(&y.1.lo)));
        let mut i = 0;
        while i < self.spans.len() {
            let (col, mut cur) = self.spans[i];
            i += 1;
            while i < self.spans.len() && self.spans[i].0 == col {
                let s = self.spans[i].1;
                if solid || s.lo <= cur.hi + MERGE_GAP {
                    cur.hi = cur.hi.max(s.hi);
                } else {
                    b.add_span(col as usize, cur.lo, cur.hi);
                    cur = s;
                }
                i += 1;
            }
            b.add_span(col as usize, cur.lo, cur.hi);
        }
        self.spans.clear();
    }
}

/// Sutherland–Hodgman against one axis-aligned plane: the part of `poly` whose coordinate `axis`
/// (0 is x, 1 is y) is at or above `at` when `keep_above`, at or below it otherwise, written to
/// `out`; returns its point count. A triangle clipped by four planes has at most seven points.
fn clip(poly: &[[f32; 3]], out: &mut [[f32; 3]; 8], axis: usize, at: f32, keep_above: bool) -> usize {
    let inside = |p: &[f32; 3]| if keep_above { p[axis] >= at } else { p[axis] <= at };
    let mut n = 0;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let (ia, ib) = (inside(&a), inside(&b));
        if ia && n < out.len() {
            out[n] = a;
            n += 1;
        }
        if ia != ib && n < out.len() {
            let t = (at - a[axis]) / (b[axis] - a[axis]);
            out[n] = [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ];
            n += 1;
        }
    }
    n
}
```

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p nightfall-bake`
Expected: PASS (3 place tests, 4 raster tests).

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-bake Cargo.lock
git commit -m "nightfall-bake: the crate, placed models' points, triangles into columns

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Baking from WoW data, and the cache

**Files:**
- Create: `crates/nightfall-bake/src/bake.rs`, `crates/nightfall-bake/src/cache.rs`, `crates/nightfall-bake/tests/duskwood.rs`, `crates/nightfall-bake/examples/bake_duskwood.rs`, `crates/nightfall-bake/examples/buildings.rs`
- Modify: `crates/nightfall-bake/src/lib.rs`

**Interfaces:**
- Consumes:
  - `Footprint`, `placement_point` (Task 6); `HeightMap::{to_bytes, from_bytes}` (Task 4); `Bounds`, `Grid`, `HeightMapBuilder`, `CELL`, `Navigator` (Tasks 1–3); `Content`, `ContentSources` (Task 5).
  - From `benilla-formats`: `Chain::open(&Path) -> Result<Chain>`; `load_map_catalog(&mut Chain) -> Result<MapCatalog>` and `MapCatalog::directory(u32) -> Option<&str>`; `load_tile_mesh(&mut Chain, map: &str, tile_x: u32, tile_y: u32) -> Result<TileMesh>`; `TileMesh { chunks: Vec<ChunkMesh>, doodads: Vec<Doodad>, wmos: Vec<WmoInstance> }`; `ChunkMesh { positions: Vec<[f32; 3]>, impassable: bool, .. }` and `ChunkMesh::height_at([f32; 3]) -> Option<f32>`; `Doodad { model: String, position: [f32; 3], rotation: [f32; 3], scale: f32, unique_id: u32 }`; `WmoInstance { model: String, position: [f32; 3], rotation: [f32; 3], unique_id: u32, .. }`; `load_wmo_collision_tris(&mut Chain, &str) -> Result<CollisionMesh>`; `load_m2_collision_hull(&mut Chain, &str) -> Result<CollisionMesh>`; `CollisionMesh { positions: Vec<[f32; 3]>, indices: Vec<u32> }` with `is_empty()`; `TILE_SIZE`, `CHUNK_SIZE`; `wow_data() -> Option<PathBuf>`; `wow_data_or_skip!`.
- Produces:
  - `PropPlacement { model: String, position: [f32; 3], rotation: [f32; 3], scale: f32 }`.
  - `BakeRequest<'a> { data_dir: &'a Path, map_id: u32, key: &'a str, bounds: &'a Bounds, props: &'a [PropPlacement] }`.
  - `bake(req: &BakeRequest, progress: &dyn Fn(f32)) -> anyhow::Result<HeightMap>` (progress from 0 to 1).
  - `buildings_near(data_dir: &Path, map_id: u32, at: V2, radius: f32) -> anyhow::Result<Vec<(f32, WmoInstance)>>`, nearest first.
  - `BAKE_VERSION: u32 = 1`, `Baked { map: HeightMap, from_cache: bool }`, `cache_key(&BakeRequest) -> anyhow::Result<u64>`, `cache_file(dir: &Path, &BakeRequest) -> anyhow::Result<PathBuf>`, `load_or_bake(req: &BakeRequest, cache_dir: Option<&Path>, progress: &dyn Fn(f32)) -> anyhow::Result<Baked>`.

- [ ] **Step 1: Write the bake, with its data-free tests**

`crates/nightfall-bake/src/bake.rs`:

```rust
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
    let tile = |w: f32| ((32.0 * TILE_SIZE - w) / TILE_SIZE).floor().clamp(0.0, 63.0) as u32;
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
fn add_model(fp: &mut Footprint, mesh: &CollisionMesh, position: [f32; 3], rotation: [f32; 3], scale: f32) {
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
pub fn buildings_near(data_dir: &Path, map_id: u32, at: V2, radius: f32) -> Result<Vec<(f32, WmoInstance)>> {
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
```

- [ ] **Step 2: Write the cache, with its data-free tests**

`crates/nightfall-bake/src/cache.rs`:

```rust
//! The bake's cache: `<dir>/<key>-<hash>.bin`, deflated. The hash covers what the bake reads
//! (every MPQ's name, size and time, so a patched or swapped install bakes again), what it is
//! asked (the map, the bounds, the props) and [`BAKE_VERSION`]. A file that does not read back
//! whole is baked again; older files for the same map are removed once a new one is written.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use nightfall_sim::{HeightMap, CELL};

use crate::bake::{bake, BakeRequest};

/// Bumped whenever the bake would make a different map from the same data.
pub const BAKE_VERSION: u32 = 1;

/// A height map, and whether it came from the cache.
pub struct Baked {
    pub map: HeightMap,
    pub from_cache: bool,
}

/// FNV-1a over the parts, each ended by a zero byte, as `together_net::content_hash` hashes.
fn fnv(parts: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.bytes().chain(std::iter::once(0)) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// Every MPQ in the data folder as `name:size:seconds`, sorted: what changes when the install
/// does, without reading gigabytes.
fn data_fingerprint(dir: &Path) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with(".mpq") {
            continue;
        }
        let meta = entry.metadata()?;
        let secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        parts.push(format!("{name}:{}:{secs}", meta.len()));
    }
    parts.sort();
    Ok(parts)
}

/// The cache's key for a request.
pub fn cache_key(req: &BakeRequest) -> Result<u64> {
    let mut parts = vec![
        format!("version {BAKE_VERSION}"),
        format!("cell {CELL}"),
        format!("map {} {}", req.key, req.map_id),
    ];
    parts.extend(
        req.bounds
            .points()
            .iter()
            .map(|p| format!("bound {} {}", p.x.to_bits(), p.y.to_bits())),
    );
    parts.extend(req.props.iter().map(|p| {
        format!(
            "prop {} {:?} {:?} {}",
            p.model.to_ascii_lowercase(),
            p.position,
            p.rotation,
            p.scale
        )
    }));
    parts.extend(data_fingerprint(req.data_dir)?);
    Ok(fnv(&parts))
}

/// Where a request's map is cached in `dir`.
pub fn cache_file(dir: &Path, req: &BakeRequest) -> Result<PathBuf> {
    Ok(dir.join(format!("{}-{:016x}.bin", req.key, cache_key(req)?)))
}

/// The map in a cache file, if it reads back whole.
pub(crate) fn read_cache(path: &Path) -> Option<HeightMap> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    DeflateDecoder::new(BufReader::new(file))
        .read_to_end(&mut bytes)
        .ok()?;
    HeightMap::from_bytes(&bytes).ok()
}

/// Write the map to `path` (through a temporary file, so a crash never leaves half a map under
/// the real name), then remove this map's older cache files in `dir`.
pub(crate) fn write_cache(dir: &Path, key: &str, path: &Path, map: &HeightMap) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    {
        let file = std::fs::File::create(&tmp)?;
        let mut out = DeflateEncoder::new(BufWriter::new(file), Compression::fast());
        out.write_all(&map.to_bytes())?;
        out.finish()?.flush()?;
    }
    std::fs::rename(&tmp, path)?;
    let prefix = format!("{key}-");
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path() != path && name.starts_with(&prefix) && name.ends_with(".bin") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// The request's map from the cache in `cache_dir` when it is there and whole, else baked and
/// cached (a cache that cannot be written still plays).
pub fn load_or_bake(req: &BakeRequest, cache_dir: Option<&Path>, progress: &dyn Fn(f32)) -> Result<Baked> {
    let path = match cache_dir {
        Some(dir) => Some(cache_file(dir, req)?),
        None => None,
    };
    if let Some(map) = path.as_deref().and_then(read_cache) {
        progress(1.0);
        return Ok(Baked {
            map,
            from_cache: true,
        });
    }
    let map = bake(req, progress)?;
    if let (Some(dir), Some(path)) = (cache_dir, &path) {
        let _ = write_cache(dir, req.key, path, &map);
    }
    Ok(Baked {
        map,
        from_cache: false,
    })
}

#[cfg(test)]
mod tests {
    use nightfall_sim::{Bounds, Grid, HeightMapBuilder, V2};

    use super::*;
    use crate::bake::PropPlacement;

    /// A fresh, empty folder of this test's own.
    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-bake-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn square(size: f32) -> Bounds {
        Bounds::new(vec![
            V2::new(0.0, 0.0),
            V2::new(size, 0.0),
            V2::new(size, size),
            V2::new(0.0, size),
        ])
        .unwrap()
    }

    fn small_map() -> HeightMap {
        let grid = Grid {
            origin: V2::ZERO,
            cell: CELL,
            nx: 20,
            ny: 20,
        };
        let mut b = HeightMapBuilder::flat(grid, 1.0);
        b.add_box(V2::new(2.0, 2.0), V2::new(4.0, 4.0), 1.0, 3.0);
        b.finish()
    }

    #[test]
    fn the_key_follows_the_data_the_bounds_and_the_props() {
        let data = scratch("key");
        std::fs::write(data.join("patch.MPQ"), b"one").unwrap();
        let (b10, b20) = (square(10.0), square(20.0));
        let prop = [PropPlacement {
            model: "World\\Crate.mdx".into(),
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 90.0, 0.0],
            scale: 1.0,
        }];
        let key = |bounds: &Bounds, props: &[PropPlacement]| {
            cache_key(&BakeRequest {
                data_dir: &data,
                map_id: 0,
                key: "test",
                bounds,
                props,
            })
            .unwrap()
        };
        let base = key(&b10, &[]);
        assert_eq!(base, key(&b10, &[]));
        assert_ne!(base, key(&b20, &[]));
        assert_ne!(base, key(&b10, &prop));
        std::fs::write(data.join("readme.txt"), b"not an archive").unwrap();
        assert_eq!(base, key(&b10, &[]));
        std::fs::write(data.join("patch.MPQ"), b"three!").unwrap();
        assert_ne!(base, key(&b10, &[]));
    }

    #[test]
    fn a_cache_reads_back_what_was_written_and_old_files_go() {
        let dir = scratch("roundtrip");
        let stale = dir.join("test-0000000000000001.bin");
        std::fs::write(&stale, b"old").unwrap();
        let other = dir.join("elwynn-0000000000000001.bin");
        std::fs::write(&other, b"another map").unwrap();
        let path = dir.join("test-00000000000000ff.bin");
        let map = small_map();
        write_cache(&dir, "test", &path, &map).unwrap();
        assert_eq!(read_cache(&path).unwrap(), map);
        assert!(!stale.exists());
        assert!(other.exists());
    }

    #[test]
    fn a_corrupt_cache_file_is_not_trusted() {
        let dir = scratch("corrupt");
        let garbage = dir.join("test-1.bin");
        std::fs::write(&garbage, b"this is not deflate").unwrap();
        assert!(read_cache(&garbage).is_none());
        let cut = dir.join("test-2.bin");
        write_cache(&dir, "test", &cut, &small_map()).unwrap();
        let bytes = std::fs::read(&cut).unwrap();
        std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        assert!(read_cache(&cut).is_none());
        assert!(read_cache(&dir.join("missing.bin")).is_none());
    }

    #[test]
    fn no_data_is_an_error_not_a_panic() {
        let data = scratch("nodata");
        let cache = scratch("nodata-cache");
        let bounds = square(10.0);
        let req = BakeRequest {
            data_dir: &data,
            map_id: 0,
            key: "test",
            bounds: &bounds,
            props: &[],
        };
        assert!(load_or_bake(&req, Some(&cache), &|_| {}).is_err());
    }
}
```

Replace the `mod`/`pub use` lines of `crates/nightfall-bake/src/lib.rs` with:

```rust
mod bake;
mod cache;
mod place;
mod raster;

pub use bake::{bake, buildings_near, BakeRequest, PropPlacement};
pub use cache::{cache_file, cache_key, load_or_bake, Baked, BAKE_VERSION};
pub use place::placement_point;
pub use raster::Footprint;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-bake --lib`
Expected: PASS (7 from Task 6, 2 bake tests, 4 cache tests).

- [ ] **Step 4: Write the bake test over a corner of Duskwood**

`crates/nightfall-bake/tests/duskwood.rs`:

```rust
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
```

Run: `WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo test --release -p nightfall-bake --test duskwood -- --nocapture`
Expected: PASS, printing the grid (320×320 columns) and how long the bake took (seconds).
Run: `cargo test -p nightfall-bake --test duskwood -- --nocapture` (no `WOW_DATA`)
Expected: PASS, printing `skipping: no WoW install found — looked in [...]`.

If the first run fails at `a building within 300 yd of Darkshire` or `a wall near the building`, print `near`, check `DARKSHIRE` in the game (stand in Darkshire with `NIGHTFALL_TRACE_POS=1`) and correct the constant; keep the assertions.

- [ ] **Step 5: Write the examples**

`crates/nightfall-bake/examples/bake_duskwood.rs`:

```rust
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
    let map = bake(&req, &|f| {
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
    println!(
        "{:?}: baked in {:.1?}; {} columns hold more than terrain; {} MB as bytes",
        map.grid(),
        started.elapsed(),
        map.complex_columns(),
        map.to_bytes().len() / 1_000_000
    );
}
```

`crates/nightfall-bake/examples/buildings.rs`:

```rust
//! Lists the buildings around a point, nearest first, with where each stands: for placing
//! Nightfall's enemies at real houses (`content/nightfall/spawns.ron`).
//! `cargo run -p nightfall-bake --example buildings -- <x> <y> [radius]`

use nightfall_bake::buildings_near;
use nightfall_sim::V2;

fn main() {
    let args: Vec<f32> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
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
```

- [ ] **Step 6: Time the whole of Duskwood**

Run: `WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo run --release -p nightfall-bake --example bake_duskwood`
Expected: a progress line each tenth, then `Grid { … nx: 3240, ny: 5300 }: baked in …` under 60 s. Over a minute: the slowest tenth shows which pass to look at (the doodads' is the likeliest); report it with the numbers rather than tuning here.
Run: `WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo run --release -p nightfall-bake --example buildings -- -10051.8 -792.5 400`
Expected: buildings listed nearest first with distance, position and WMO path; an empty list means the entry is far from any (then try 800).

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-bake
git commit -m "nightfall-bake: Duskwood's terrain, buildings and hulls into the height map, cached

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

Report Step 6's time and size with the task, so the review sees whether the minute holds.

---

### Task 8: Enemies: placed from content, walking the map

**Files:**
- Create: `crates/nightfall-sim/src/enemies.rs`
- Modify: `crates/nightfall-sim/src/world.rs`, `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `Navigator`, `HeightMap`, `MAX_STEP`, `testmap` (Tasks 1–3); `Behaviour`, `Content::{enemies, spawns, enemy}`, `WorldRules` (Task 5).
- Produces:
  - `nightfall_sim::Enemy { id: u32, kind: usize, pos: V2, z: f32, facing: V2, hp: f32, max_hp: f32, speed: f32 }` (`Clone, Debug, PartialEq, Serialize, Deserialize`).
  - `PATROL_BUDGET: usize = 40_000`, `WANDER_BUDGET: usize = 4_000`.
  - `Snapshot` gains `enemies: Vec<Enemy>`.
  - `World` gains `pub enemies: Vec<Enemy>`, `attach_map(&mut self, map: Arc<HeightMap>)`, `map(&self) -> Option<&Arc<HeightMap>>`, `enemy(&self, id: u32) -> Option<&Enemy>`, `warnings(&self) -> &[String]`; `step` moves enemies once a map is attached; `apply_snapshot` takes them.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/enemies.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::heightmap::testmap::{flat, house};
    use crate::{Content, ContentSources, HeightMap, World, V2};

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
        entries: [(5.0, 5.0, 0.0)])"#;
    const ENEMIES: &str = r#"[
        (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
    ]"#;

    fn world(spawns: &str, paths_per_step: u32) -> World {
        let rules = format!(
            r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
                fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                world: (share_sight_within: 10.0, paths_per_step: {paths_per_step}, wander_pause: (1.0, 2.0)))"#
        );
        let content = Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: &rules,
            enemies: ENEMIES,
            spawns,
        })
        .unwrap();
        World::new(Arc::new(content))
    }

    /// Flat ground 50 yd across with a closed house (no door, no roof) over [20, 30]².
    fn town() -> Arc<HeightMap> {
        let mut b = flat(50.0);
        house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
        Arc::new(b.finish())
    }

    fn in_house(p: V2) -> bool {
        p.x > 20.0 && p.x < 30.0 && p.y > 20.0 && p.y < 30.0
    }

    const GATE: &str =
        r#"[(place: "Gate", kind: "skeleton", behaviour: Sentry(at: (10.0, 10.0), facing: 90.0))]"#;

    #[test]
    fn a_world_without_a_map_has_no_enemies() {
        let mut w = world(GATE, 4);
        w.step();
        assert!(w.enemies.is_empty());
        assert!(w.map().is_none());
    }

    #[test]
    fn a_sentry_holds_its_post_facing_its_way() {
        let mut w = world(GATE, 4);
        w.attach_map(town());
        for _ in 0..30 {
            w.step();
        }
        let e = &w.enemies[0];
        assert_eq!(e.id, 1);
        assert_eq!(e.pos, V2::new(10.0, 10.0));
        assert_eq!(e.z, 0.0);
        assert!(e.facing.y > 0.99, "{:?}", e.facing);
        assert_eq!(e.speed, 0.0);
        assert_eq!(e.hp, 900.0);
        assert!(w.warnings().is_empty());
    }

    #[test]
    fn a_patrol_walks_around_a_house_not_through_it() {
        let mut w = world(
            r#"[(place: "Road", kind: "skeleton", behaviour: Patrol(route: [(25.0, 10.0), (25.0, 40.0)], pause: 0.0))]"#,
            4,
        );
        w.attach_map(town());
        let mut reached = false;
        for _ in 0..900 {
            w.step();
            let e = &w.enemies[0];
            assert!(!in_house(e.pos), "{:?}", e.pos);
            reached |= e.pos.distance(V2::new(25.0, 40.0)) <= 0.6;
        }
        assert!(reached);
    }

    #[test]
    fn wanderers_stay_in_their_area() {
        let mut w = world(
            r#"[(place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (10.0, 40.0), radius: 6.0))]"#,
            4,
        );
        w.attach_map(town());
        assert_eq!(w.enemies.len(), 3);
        let start: Vec<V2> = w.enemies.iter().map(|e| e.pos).collect();
        for _ in 0..1800 {
            w.step();
            for e in &w.enemies {
                assert!(e.pos.distance(V2::new(10.0, 40.0)) <= 6.75, "{:?}", e.pos);
            }
        }
        assert!(w.enemies.iter().zip(&start).any(|(e, s)| e.pos != *s));
    }

    #[test]
    fn an_unreachable_patrol_point_is_skipped_and_named() {
        let mut w = world(
            r#"[(place: "Road", kind: "skeleton", behaviour: Patrol(route: [(10.0, 10.0), (25.0, 25.0), (40.0, 10.0)], pause: 0.0))]"#,
            4,
        );
        w.attach_map(town());
        let mut reached = false;
        for _ in 0..1200 {
            w.step();
            reached |= w.enemies[0].pos.distance(V2::new(40.0, 10.0)) <= 0.6;
        }
        assert!(reached);
        let named = w
            .warnings()
            .iter()
            .filter(|m| m.contains("\"Road\"") && m.contains("patrol point 2"))
            .count();
        assert_eq!(named, 1, "{:?}", w.warnings());
    }

    #[test]
    fn a_spawn_with_no_floor_is_left_out_and_named() {
        let mut b = flat(50.0);
        for col in b.columns_in(V2::new(8.0, 8.0), V2::new(12.0, 12.0)) {
            b.set_ground(col, f32::NAN);
        }
        let mut w = world(GATE, 4);
        w.attach_map(Arc::new(b.finish()));
        assert!(w.enemies.is_empty());
        assert!(w.warnings()[0].contains("\"Gate\""));
    }

    #[test]
    fn no_more_paths_a_step_than_the_rules_allow() {
        let mut w = world(
            r#"[(place: "Pack", kind: "skeleton", count: 10, behaviour: Wander(center: (10.0, 10.0), radius: 5.0))]"#,
            2,
        );
        w.attach_map(town());
        w.step();
        assert_eq!(w.enemies.iter().filter(|e| e.speed > 0.0).count(), 2);
    }

    #[test]
    fn the_same_run_twice_is_the_same() {
        let run = || {
            let mut w = world(
                r#"[
                    (place: "Road", kind: "skeleton", behaviour: Patrol(route: [(25.0, 10.0), (25.0, 40.0)])),
                    (place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (10.0, 40.0), radius: 6.0)),
                ]"#,
                4,
            );
            w.attach_map(town());
            for _ in 0..300 {
                w.step();
            }
            w.snapshot()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_mirror_takes_the_hosts_enemies() {
        let mut host = world(GATE, 4);
        host.attach_map(town());
        host.step();
        let mut mirror = world("[]", 4);
        mirror.apply_snapshot(host.snapshot());
        assert_eq!(mirror.enemies, host.enemies);
        assert!(mirror.enemy(1).is_some());
        mirror.step();
        assert_eq!(mirror.enemies, host.enemies);
    }
}
```

Add `mod enemies;` to `lib.rs` beside `mod content;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim enemies`
Expected: FAIL to compile (`no method named attach_map`, `no field enemies on type World`).

- [ ] **Step 3: Implement the enemies**

At the top of `crates/nightfall-sim/src/enemies.rs`:

```rust
//! Nightfall's enemies as the host runs them (spec §7.1): placed from the content's spawns on the
//! height map's floors, then each about its business. A sentry holds its post, a patrol walks its
//! route round and round, a wanderer roams its area and rests between walks. They go where the
//! map's paths go, so around houses rather than through them. What they notice (sub-project 3)
//! and how they fight (sub-project 4) come later; an enemy's [`Brain`] is where both plug in.

use std::f32::consts::TAU;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::content::Behaviour;
use crate::heightmap::{HeightMap, Navigator, MAX_STEP};
use crate::{Content, DT, V2, V3};

/// Floors a patrol's or sentry's path search may look at: its legs can be long, and each is
/// searched once (the [`Navigator`] keeps them).
pub const PATROL_BUDGET: usize = 40_000;
/// Floors a wanderer's search may look at: its walks are short, and a spot it cannot reach (in a
/// closed house) must be given up on cheaply.
pub const WANDER_BUDGET: usize = 4_000;
/// Within this many yards of a point, an enemy is there.
const ARRIVE: f32 = 0.6;
/// How far apart a group of sentries stands around its post.
const GROUP_SPACING: f32 = 1.5;
/// Seconds before a sentry tries again to reach a post it found no path to.
const RETRY: f32 = 5.0;
/// Failed walks in a row before a wanderer's area is reported.
const WANDER_GIVE_UP: u32 = 10;

/// An enemy as every client sees it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enemy {
    pub id: u32,
    /// Index into [`Content::enemies`].
    pub kind: usize,
    pub pos: V2,
    /// The floor it stands on.
    pub z: f32,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// How fast it moved this step, yards a second (0 standing).
    pub speed: f32,
}

/// A small random number generator (xorshift64*), seeded per enemy, so a run plays the same every
/// time with the same inputs.
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..1`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// What an enemy is set to do.
pub(crate) enum Order {
    Sentry { post: V3, facing: V2 },
    /// `None` marks a point with no floor, or one no path reaches: it is skipped.
    Patrol {
        route: Vec<Option<V3>>,
        target: usize,
        pause: f32,
    },
    Wander { center: V3, radius: f32 },
}

/// An enemy's mind, the host's alone: never sent.
pub(crate) struct Brain {
    /// Index into [`Content::spawns`], for warnings.
    spawn: usize,
    order: Order,
    /// The path being walked, and the next point of it.
    path: Option<Arc<[V3]>>,
    next: usize,
    /// Seconds to stand before moving on.
    wait: f32,
    /// Failed searches in a row.
    failures: u32,
    /// A warning about it has been given.
    warned: bool,
    rng: Rng,
}

/// How a warning names a spawn.
fn who(content: &Content, spawn: usize) -> String {
    let s = &content.spawns[spawn];
    format!("spawns.ron: \"{}\" ({})", s.place, s.kind)
}

/// `d` as a facing; north when it is (nearly) nothing.
fn heading(d: V2) -> V2 {
    let n = d.normalize_or_zero();
    if n == V2::ZERO {
        V2::new(1.0, 0.0)
    } else {
        n
    }
}

/// The next point of the route after `from` that is not skipped; `from` itself when none is.
fn next_target(route: &[Option<V3>], from: usize) -> usize {
    (1..=route.len())
        .map(|d| (from + d) % route.len())
        .find(|&i| route[i].is_some())
        .unwrap_or(from)
}

/// Every enemy the content's spawns place, on the map's floors, with a warning for each that
/// could not be placed. Ids count from 1 in the content's order.
pub(crate) fn spawn_all(content: &Content, map: &HeightMap) -> (Vec<Enemy>, Vec<Brain>, Vec<String>) {
    let mut enemies = Vec::new();
    let mut brains = Vec::new();
    let mut warnings = Vec::new();
    let mut id = 1u32;
    for (si, s) in content.spawns.iter().enumerate() {
        let Some(kind) = content.enemy(&s.kind) else {
            continue;
        };
        let def = &content.enemies[kind];
        // The floor a point stands on: near the height the content gives, else near the terrain.
        let floor = |p: V2| -> Option<V3> {
            let hint = s.z.or_else(|| map.ground_at(p)).unwrap_or(0.0);
            map.floor_near(p, hint).map(|z| V3::at(p, z))
        };
        for k in 0..s.count as usize {
            let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15 ^ u64::from(id).wrapping_mul(0xBF58_476D_1CE4_E5B9));
            let placed = match &s.behaviour {
                Behaviour::Sentry { at, facing } => {
                    let post = V2::new(at.0, at.1);
                    let stand = if k == 0 {
                        post
                    } else {
                        post + V2::from_angle(k as f32 * TAU / s.count as f32, GROUP_SPACING)
                    };
                    let face = V2::from_angle(facing.to_radians(), 1.0);
                    floor(stand).map(|p| (p, Order::Sentry { post: p, facing: face }, face))
                }
                Behaviour::Patrol { route, pause } => {
                    let points: Vec<Option<V3>> =
                        route.iter().map(|&(x, y)| floor(V2::new(x, y))).collect();
                    if k == 0 {
                        for (i, p) in points.iter().enumerate() {
                            if p.is_none() {
                                warnings.push(format!(
                                    "{}: patrol point {} has no floor; it is skipped",
                                    who(content, si),
                                    i + 1
                                ));
                            }
                        }
                    }
                    let first = k % points.len();
                    points[first].map(|p| {
                        let target = next_target(&points, first);
                        let facing = points[target]
                            .map_or(V2::new(1.0, 0.0), |t| heading(t.ground() - p.ground()));
                        let order = Order::Patrol {
                            route: points.clone(),
                            target,
                            pause: *pause,
                        };
                        (p, order, facing)
                    })
                }
                Behaviour::Wander { center, radius } => {
                    floor(V2::new(center.0, center.1)).map(|c| {
                        let spot = c.ground()
                            + V2::from_angle(rng.next_f32() * TAU, radius * rng.next_f32().sqrt());
                        let p = map.floor_near(spot, c.z).map_or(c, |z| V3::at(spot, z));
                        let facing = V2::from_angle(rng.next_f32() * TAU, 1.0);
                        (p, Order::Wander { center: c, radius: *radius }, facing)
                    })
                }
            };
            let Some((at, order, facing)) = placed else {
                warnings.push(format!("{}: no floor to stand on; left out", who(content, si)));
                continue;
            };
            enemies.push(Enemy {
                id,
                kind,
                pos: at.ground(),
                z: at.z,
                facing,
                hp: def.hp,
                max_hp: def.hp,
                speed: 0.0,
            });
            brains.push(Brain {
                spawn: si,
                order,
                path: None,
                next: 0,
                wait: 0.0,
                failures: 0,
                warned: false,
                rng,
            });
            id += 1;
        }
    }
    (enemies, brains, warnings)
}

/// One step for every enemy: those with nowhere to go choose a way (at most `paths_per_step`
/// searches, starting one enemy further along each step so every enemy gets its turn), then all
/// walk.
pub(crate) fn step(
    enemies: &mut [Enemy],
    brains: &mut [Brain],
    nav: &mut Navigator,
    content: &Content,
    warnings: &mut Vec<String>,
    cursor: &mut usize,
) {
    let n = enemies.len();
    if n == 0 {
        return;
    }
    let mut budget = content.rules.world.paths_per_step;
    let first = *cursor % n;
    *cursor = cursor.wrapping_add(1);
    for i in 0..n {
        let k = (first + i) % n;
        plan(&enemies[k], &mut brains[k], nav, content, warnings, &mut budget);
    }
    let map = nav.map();
    for (e, b) in enemies.iter_mut().zip(brains.iter_mut()) {
        walk(e, b, map, content);
    }
}

/// An idle enemy chooses where to go next and searches its path, if the step's budget allows.
fn plan(
    e: &Enemy,
    b: &mut Brain,
    nav: &mut Navigator,
    content: &Content,
    warnings: &mut Vec<String>,
    budget: &mut u32,
) {
    if b.wait > 0.0 || b.path.is_some() || *budget == 0 {
        return;
    }
    let here = V3::at(e.pos, e.z);
    let (goal, cache, limit) = match &mut b.order {
        Order::Sentry { post, .. } => {
            if here.ground().distance(post.ground()) <= ARRIVE {
                return;
            }
            (*post, true, PATROL_BUDGET)
        }
        Order::Patrol { route, target, pause } => {
            let Some(goal) = route[*target] else {
                *target = next_target(route, *target);
                return;
            };
            if here.ground().distance(goal.ground()) <= ARRIVE {
                *target = next_target(route, *target);
                b.wait = *pause;
                return;
            }
            (goal, true, PATROL_BUDGET)
        }
        Order::Wander { center, radius } => {
            let spot = center.ground()
                + V2::from_angle(b.rng.next_f32() * TAU, *radius * b.rng.next_f32().sqrt());
            let Some(z) = nav.map().floor_near(spot, center.z) else {
                b.wait = 1.0;
                return;
            };
            (V3::at(spot, z), false, WANDER_BUDGET)
        }
    };
    *budget -= 1;
    if let Some(path) = nav.find_path(here, goal, cache, limit) {
        b.path = Some(path);
        b.next = 0;
        b.failures = 0;
        return;
    }
    match &mut b.order {
        Order::Sentry { .. } => {
            b.wait = RETRY;
            if !b.warned {
                b.warned = true;
                warnings.push(format!("{}: no path to its post", who(content, b.spawn)));
            }
        }
        Order::Patrol { route, target, .. } => {
            warnings.push(format!(
                "{}: no path to patrol point {} ({:.1}, {:.1}); it is skipped",
                who(content, b.spawn),
                *target + 1,
                goal.x,
                goal.y
            ));
            route[*target] = None;
            *target = next_target(route, *target);
        }
        Order::Wander { .. } => {
            b.failures += 1;
            b.wait = 1.0;
            if b.failures >= WANDER_GIVE_UP && !b.warned {
                b.warned = true;
                warnings.push(format!(
                    "{}: finds no ground to walk to in its area",
                    who(content, b.spawn)
                ));
            }
        }
    }
}

/// An enemy walks its path for a step at its walking speed, on the floor under it.
fn walk(e: &mut Enemy, b: &mut Brain, map: &HeightMap, content: &Content) {
    if b.wait > 0.0 {
        b.wait = (b.wait - DT).max(0.0);
        e.speed = 0.0;
        return;
    }
    let Some(path) = b.path.clone() else {
        e.speed = 0.0;
        if let Order::Sentry { facing, .. } = &b.order {
            e.facing = *facing;
        }
        return;
    };
    let speed = content.enemies[e.kind].walk_speed;
    let from = e.pos;
    let mut left = speed * DT;
    while left > 1e-6 && b.next < path.len() {
        let to = path[b.next];
        let d = e.pos.distance(to.ground());
        if d <= left {
            e.pos = to.ground();
            e.z = to.z;
            left -= d;
            b.next += 1;
        } else {
            e.pos += (to.ground() - e.pos) * (left / d);
            left = 0.0;
        }
    }
    e.z = map.floor_within(e.pos, e.z, MAX_STEP).unwrap_or(e.z);
    let moved = e.pos - from;
    if moved.length() > 1e-4 {
        e.facing = moved.normalize_or_zero();
        e.speed = speed;
    } else {
        e.speed = 0.0;
    }
    if b.next >= path.len() {
        b.path = None;
        arrive(e, b, content);
    }
}

/// At the end of a path: a sentry turns to its watch, a patrol rests and takes its next point, a
/// wanderer rests a while.
fn arrive(e: &mut Enemy, b: &mut Brain, content: &Content) {
    match &mut b.order {
        Order::Sentry { facing, .. } => e.facing = *facing,
        Order::Patrol { route, target, pause } => {
            *target = next_target(route, *target);
            b.wait = *pause;
        }
        Order::Wander { .. } => {
            let (lo, hi) = content.rules.world.wander_pause;
            b.wait = lo + (hi - lo) * b.rng.next_f32();
        }
    }
}
```

Replace everything above `#[cfg(test)]` in `crates/nightfall-sim/src/world.rs` with:

```rust
//! The world of a run: its heroes, its enemies and where they stand. A hero's position comes from
//! its player's client (WoW's controller moves it there, as a WoW client reports its moves to a
//! server); the world keeps it inside the map. Enemies are the host's: once the baked height map
//! is attached ([`World::attach_map`]) they are placed from the content and walk its floors
//! ([`crate::enemies`]). A client's world mirrors its host's snapshots and has no map.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::enemies::{self, Brain, Enemy};
use crate::heightmap::{HeightMap, Navigator};
use crate::{Content, V2};

/// A player, numbered from 0 (the host).
pub type PlayerId = u8;

/// Steps a second.
pub const TICK_HZ: u32 = 30;
/// Seconds a step.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// How far apart heroes who start at the same entry stand, yards.
const SPAWN_SPACING: f32 = 2.0;

/// Where a player's client put its hero, and the way it faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroPose {
    pub pos: V2,
    pub facing: V2,
}

/// What a player sends a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub pose: Option<HeroPose>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    pub player: PlayerId,
    /// Index into [`Content::classes`].
    pub class: usize,
    pub name: String,
    pub race: u8,
    pub sex: u8,
    pub pos: V2,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Running speed, yards a second.
    pub speed: f32,
}

/// Everything a client needs to draw the world as the host has it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub elapsed: f32,
    pub heroes: Vec<Hero>,
    /// The enemies; a player's own snapshot has those near their hero.
    pub enemies: Vec<Enemy>,
}

#[derive(Debug, PartialEq)]
pub enum WorldError {
    NoSuchClass(String),
    PlayerTaken(PlayerId),
}

pub struct World {
    content: Arc<Content>,
    pub heroes: Vec<Hero>,
    /// The host's, placed when a map is attached; a client's, from its host's snapshots.
    pub enemies: Vec<Enemy>,
    /// Seconds since the run began.
    pub elapsed: f32,
    /// Each player's latest input since the last step.
    inputs: Vec<(PlayerId, PlayerInput)>,
    /// What each enemy means to do, beside it in `enemies`: the host's alone, never sent.
    brains: Vec<Brain>,
    /// The height map and its paths, once attached: the host's alone.
    nav: Option<Navigator>,
    /// What the content asked that the map could not give (a patrol point no path reaches).
    warnings: Vec<String>,
    /// Which enemy chooses its way first next step, so the path budget goes round.
    plan_cursor: usize,
}

impl World {
    /// An empty run on the content's map.
    pub fn new(content: Arc<Content>) -> Self {
        Self {
            content,
            heroes: Vec::new(),
            enemies: Vec::new(),
            elapsed: 0.0,
            inputs: Vec::new(),
            brains: Vec::new(),
            nav: None,
            warnings: Vec::new(),
            plan_cursor: 0,
        }
    }

    pub fn content(&self) -> &Arc<Content> {
        &self.content
    }

    /// `player`'s hero, of the class with `class_key`, at the first entry; each later hero stands
    /// [`SPAWN_SPACING`] yards from it, around a circle.
    pub fn add_hero(
        &mut self,
        player: PlayerId,
        class_key: &str,
        name: &str,
        race: u8,
        sex: u8,
    ) -> Result<(), WorldError> {
        let class = self
            .content
            .class(class_key)
            .ok_or_else(|| WorldError::NoSuchClass(class_key.to_owned()))?;
        if self.hero(player).is_some() {
            return Err(WorldError::PlayerTaken(player));
        }
        let map = &self.content.map;
        let entry = V2::new(map.entries[0][0], map.entries[0][1]);
        let k = self.heroes.len();
        let pos = if k == 0 {
            entry
        } else {
            let angle = (k - 1) as f32 * std::f32::consts::TAU / 4.0;
            map.bounds.clamp(entry + V2::from_angle(angle, SPAWN_SPACING))
        };
        let def = &self.content.classes[class];
        self.heroes.push(Hero {
            player,
            class,
            name: name.to_owned(),
            race,
            sex,
            pos,
            facing: V2::new(1.0, 0.0),
            hp: def.base_hp,
            max_hp: def.base_hp,
            speed: def.speed,
        });
        Ok(())
    }

    pub fn remove_hero(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.inputs.retain(|(p, _)| *p != player);
    }

    pub fn hero(&self, player: PlayerId) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.player == player)
    }

    pub fn enemy(&self, id: u32) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.id == id)
    }

    /// The host's height map: from now on the world knows walls and floors, and the content's
    /// enemies are placed on them (any placed before start over).
    pub fn attach_map(&mut self, map: Arc<HeightMap>) {
        let (enemies, brains, warnings) = enemies::spawn_all(&self.content, &map);
        self.enemies = enemies;
        self.brains = brains;
        self.warnings.extend(warnings);
        self.nav = Some(Navigator::new(map));
    }

    /// The height map, on the host once attached.
    pub fn map(&self) -> Option<&Arc<HeightMap>> {
        self.nav.as_ref().map(Navigator::map)
    }

    /// What the content asked that the map could not give, for the log.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// `player`'s input for the next step, replacing any earlier one since the last step.
    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        match self.inputs.iter_mut().find(|(p, _)| *p == player) {
            Some(slot) => slot.1 = input,
            None => self.inputs.push((player, input)),
        }
    }

    /// One step: each hero takes its player's pose, kept inside the map (a pose that is not a
    /// number is dropped, so a broken client cannot poison the world); then, once a map is
    /// attached, every enemy goes about its business.
    pub fn step(&mut self) {
        let bounds = &self.content.map.bounds;
        for (player, input) in self.inputs.drain(..) {
            let Some(pose) = input.pose else { continue };
            if !pose.pos.is_finite() || !pose.facing.is_finite() {
                continue;
            }
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.pos = bounds.clamp(pose.pos);
                let facing = pose.facing.normalize_or_zero();
                if facing != V2::ZERO {
                    hero.facing = facing;
                }
            }
        }
        if let Some(nav) = &mut self.nav {
            enemies::step(
                &mut self.enemies,
                &mut self.brains,
                nav,
                &self.content,
                &mut self.warnings,
                &mut self.plan_cursor,
            );
        }
        self.elapsed += DT;
    }

    /// The whole world, as the host has it.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            elapsed: self.elapsed,
            heroes: self.heroes.clone(),
            enemies: self.enemies.clone(),
        }
    }

    /// A client's mirror takes the host's world.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.elapsed = snapshot.elapsed;
        self.heroes = snapshot.heroes;
        self.enemies = snapshot.enemies;
    }
}
```

In `lib.rs`, add `pub use enemies::{Enemy, PATROL_BUDGET, WANDER_BUDGET};`, and replace the module doc's sentence that begins `Sub-project 1 has heroes` (to the end of the doc) with: `Sub-project 1 has heroes, their poses and the map's edge; sub-project 2 adds the baked height map (walls, floors, sight and paths) and enemies that walk it; senses, combat and loot come in later sub-projects (docs/superpowers/specs/2026-10-16-nightfall-design.md).`

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (everything before, 9 enemy tests). The longest, `an_unreachable_patrol_point_is_skipped_and_named`, takes a second or two in a debug build.

- [ ] **Step 5: Check the client still builds**

Run: `cargo check -p benilla-app`
Expected: builds (the client reads `Snapshot` but builds none).

- [ ] **Step 6: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: enemies placed from content, standing, patrolling and wandering on paths

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: What each hero sees, and each player's snapshot

**Files:**
- Create: `crates/nightfall-sim/src/sight.rs`
- Modify: `crates/nightfall-sim/src/world.rs` (the hero's height), `crates/nightfall-sim/src/messages.rs`, `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`
- Modify (the wire changed): `crates/benilla-app/src/nightfall/session.rs` (`host_send`, `Heard`, `client_exchange`), `crates/benilla-app/src/nightfall/run.rs` (`Sim::seen`, the inbox, `feed_pose`, `step_sim`)

**Interfaces:**
- Consumes: `World`, `Enemy`, `Hero`, `Snapshot`, `World::map` (Task 8); `HeightMap::line_of_sight` (Task 2); `WorldRules::share_sight_within`, `Atmosphere::{fog_end, view_distance}` (Task 5).
- Produces:
  - `HeroPose` gains `z: f32` (the client's height for its hero's feet); `Hero` gains `z: f32` (the floor it stands on).
  - `EYE_HEIGHT = 1.7`, `CHEST_HEIGHT = 1.2`.
  - `World::seen_by(&self, player: PlayerId) -> Vec<u32>` and `World::snapshot_for(&self, player: PlayerId) -> Snapshot`.
  - `ServerMsg::State { snapshot: Snapshot, seen: Vec<u32> }`; `PROTOCOL = 2`.
  - Client: `run::Sim` gains `pub seen: Vec<u32>`; `session::Heard::State(Snapshot, Vec<u32>)`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/sight.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::heightmap::testmap::{flat, house};
    use crate::{
        Content, ContentSources, Enemy, HeroPose, PlayerId, PlayerInput, World, V2,
    };

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
        entries: [(5.0, 5.0, 0.0)])"#;
    const ENEMIES: &str = r#"[
        (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
    ]"#;

    /// A sentry at each point, ids 1, 2, … in order; fog at 40 yd, drawn to `view` yd; with
    /// `map`, flat ground with a closed house over [20, 30]².
    fn world(posts: &[(f32, f32)], view: f32, map: bool) -> World {
        let spawns: Vec<String> = posts
            .iter()
            .map(|(x, y)| {
                format!(
                    r#"(place: "Post", kind: "skeleton", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: 0.0))"#
                )
            })
            .collect();
        let spawns = format!("[{}]", spawns.join(", "));
        let rules = format!(
            r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
                fog_color: (0.1, 0.1, 0.1), view_distance: {view:.1},
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#
        );
        let content = Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: &rules,
            enemies: ENEMIES,
            spawns: &spawns,
        })
        .unwrap();
        let mut w = World::new(Arc::new(content));
        if map {
            let mut b = flat(50.0);
            house(&mut b, V2::new(20.0, 20.0), V2::new(30.0, 30.0), None, false);
            w.attach_map(Arc::new(b.finish()));
        }
        w
    }

    /// `player`'s hero (made if need be) at `(x, y)` on the ground, from the next step.
    fn put(w: &mut World, player: PlayerId, x: f32, y: f32) {
        if w.hero(player).is_none() {
            w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
        }
        w.set_input(
            player,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: V2::new(x, y),
                    facing: V2::new(1.0, 0.0),
                    z: 0.0,
                }),
            },
        );
    }

    #[test]
    fn an_enemy_behind_a_house_is_not_seen_and_one_in_the_open_is() {
        let mut w = world(&[(38.0, 25.0), (12.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        assert_eq!(w.seen_by(0), vec![2]);
    }

    #[test]
    fn a_teammate_close_by_shares_what_they_see() {
        let mut w = world(&[(38.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        put(&mut w, 1, 12.0, 34.0);
        w.step();
        assert_eq!(w.seen_by(0), vec![1]);
    }

    #[test]
    fn a_teammate_far_away_does_not() {
        let mut w = world(&[(38.0, 40.0)], 120.0, true);
        put(&mut w, 0, 12.0, 25.0);
        put(&mut w, 1, 12.0, 44.0);
        w.step();
        assert!(w.seen_by(0).is_empty());
        assert_eq!(w.seen_by(1), vec![1]);
    }

    #[test]
    fn nothing_past_the_fog_is_seen() {
        let mut w = world(&[(45.0, 45.0)], 120.0, true);
        put(&mut w, 0, 5.0, 5.0);
        w.step();
        assert!(w.seen_by(0).is_empty());
    }

    #[test]
    fn without_a_map_only_distance_counts() {
        let mut w = world(&[], 120.0, false);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        let mut s = w.snapshot();
        s.enemies.push(Enemy {
            id: 9,
            kind: 0,
            pos: V2::new(38.0, 25.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 900.0,
            max_hp: 900.0,
            speed: 0.0,
        });
        w.apply_snapshot(s);
        assert_eq!(w.seen_by(0), vec![9]);
    }

    #[test]
    fn a_players_snapshot_leaves_out_enemies_past_the_view() {
        let mut w = world(&[(38.0, 25.0), (12.0, 40.0)], 20.0, true);
        put(&mut w, 0, 12.0, 25.0);
        w.step();
        let ids: Vec<u32> = w.snapshot_for(0).enemies.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![2]);
        assert_eq!(w.snapshot().enemies.len(), 2);
        assert_eq!(w.snapshot_for(7).enemies.len(), 2);
    }

    #[test]
    fn a_player_without_a_hero_sees_nothing() {
        let mut w = world(&[(12.0, 40.0)], 120.0, true);
        w.step();
        assert!(w.seen_by(7).is_empty());
    }
}
```

In `world.rs`'s tests, give `pose` and `bad_facing` a height: in `pose`, `HeroPose { pos: V2::new(x, y), facing: V2::new(0.0, 2.0), z: 10.0 }`; in `bad_facing`, add `z: 10.0,`. Add:

```rust
    #[test]
    fn a_hero_stands_on_the_floor_near_the_height_its_client_reports() {
        let mut w = World::new(content());
        w.attach_map(Arc::new(crate::heightmap::testmap::flat(100.0).finish()));
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        assert_eq!(w.hero(0).unwrap().z, 10.0);
        let at = |z: f32| PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(60.0, 40.0),
                facing: V2::new(1.0, 0.0),
                z,
            }),
        };
        w.set_input(0, at(0.3));
        w.step();
        assert_eq!(w.hero(0).unwrap().z, 0.0);
        // Mid-jump, no floor within a yard: the reported height stands.
        w.set_input(0, at(5.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().z, 5.0);
        w.set_input(0, at(f32::NAN));
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.z, 5.0);
        assert_eq!(h.pos, V2::new(60.0, 40.0));
    }

    #[test]
    fn without_a_map_a_hero_takes_the_reported_height() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(
            0,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: V2::new(60.0, 40.0),
                    facing: V2::new(1.0, 0.0),
                    z: 12.5,
                }),
            },
        );
        w.step();
        assert_eq!(w.hero(0).unwrap().z, 12.5);
    }
```

In `tests/shipped_content.rs`, the `HeroPose { pos: far, facing: V2::new(1.0, 0.0) }` gains `z: 36.5,`.

In `messages.rs`'s `a_full_partys_state_is_small`, replace everything from `let msg = ServerMsg::State {` to the end of the test with:

```rust
        let mut snapshot = w.snapshot();
        snapshot.enemies = (1..=20)
            .map(|id| Enemy {
                id,
                kind: 0,
                pos: V2::new(-10051.8, -792.5),
                z: 36.5,
                facing: V2::new(1.0, 0.0),
                hp: 900.0,
                max_hp: 900.0,
                speed: 2.5,
            })
            .collect();
        let msg = ServerMsg::State {
            snapshot,
            seen: (1..=20).collect(),
        };
        let bytes = postcard::to_stdvec(&msg).unwrap().len();
        // Four heroes and the twenty enemies near one of them, 15 times a second, stay near
        // 20 KB a second.
        assert!(bytes < 1400, "{bytes} bytes");
```

and change that test module's `use crate::{Content, ContentSources, World};` to `use crate::{Content, ContentSources, Enemy, World, V2};`.

Add `mod sight;` to `lib.rs` beside `mod world;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`struct HeroPose has no field named z`, `no method named seen_by`).

- [ ] **Step 3: Implement the hero's height, sight and the per-player snapshot**

In `world.rs`:
- `HeroPose` gains, after `facing`:

```rust
    /// The height of its feet as the client's controller has it: which floor of a house the hero
    /// is on, for what it sees.
    pub z: f32,
```

- `Hero` gains, after `pos`:

```rust
    /// The floor it stands on (the reported height while no floor is near it: a jump).
    pub z: f32,
```

- Below `SPAWN_SPACING`, add:

```rust
/// How far (yards) a reported height may be from a floor and still stand on it.
const HERO_FLOOR_REACH: f32 = 1.0;
```

- In `add_hero`, after `let entry = …;` add `let z = map.entries[0][2];`, and add `z,` after `pos,` in the `Hero { … }` literal.
- In `step`, replace `hero.pos = bounds.clamp(pose.pos);` with:

```rust
                hero.pos = bounds.clamp(pose.pos);
                if pose.z.is_finite() {
                    // On the host, the floor nearest the reported height, so a hero on a stair
                    // sees from where they stand; a height with no floor near it (a jump) is
                    // kept as it is.
                    let pos = hero.pos;
                    hero.z = self
                        .nav
                        .as_ref()
                        .and_then(|nav| nav.map().floor_within(pos, pose.z, HERO_FLOOR_REACH))
                        .unwrap_or(pose.z);
                }
```

At the top of `crates/nightfall-sim/src/sight.rs`:

```rust
//! What each hero sees (spec §4.3): the enemies within the fog and in line of sight from the
//! hero's eyes to their chests, or from a teammate's within `share_sight_within` yards (a friend
//! at your shoulder calls out what they see). The host sends each player theirs, with a snapshot
//! that holds only the enemies near their own hero. Sub-project 3 hides the others from the
//! interface (the `Concealed` marker), and enemies' sight of heroes will use the same query.

use crate::{Enemy, Hero, PlayerId, Snapshot, World, V3};

/// How high a unit's eyes are over its floor, yards.
pub const EYE_HEIGHT: f32 = 1.7;
/// How high the point seen of a unit is over its floor: its chest.
pub const CHEST_HEIGHT: f32 = 1.2;

impl World {
    /// The ids of the enemies `player`'s hero sees, in the world's order; none for a player
    /// without a hero.
    pub fn seen_by(&self, player: PlayerId) -> Vec<u32> {
        let Some(me) = self.hero(player) else {
            return Vec::new();
        };
        let rules = &self.content().rules;
        let share = rules.world.share_sight_within;
        let fog = rules.atmosphere.fog_end;
        let watchers: Vec<&Hero> = self
            .heroes
            .iter()
            .filter(|h| h.player == player || h.pos.distance(me.pos) <= share)
            .collect();
        self.enemies
            .iter()
            .filter(|e| watchers.iter().any(|h| self.sees(h, e, fog)))
            .map(|e| e.id)
            .collect()
    }

    /// `player`'s snapshot: every hero, and the enemies within the view distance of their hero
    /// (all of them for a player without one), so a big map's enemies never crowd a line.
    pub fn snapshot_for(&self, player: PlayerId) -> Snapshot {
        let near = self.hero(player).map(|h| h.pos);
        let reach = self.content().rules.atmosphere.view_distance;
        Snapshot {
            elapsed: self.elapsed,
            heroes: self.heroes.clone(),
            enemies: self
                .enemies
                .iter()
                .filter(|e| near.is_none_or(|p| p.distance(e.pos) <= reach))
                .cloned()
                .collect(),
        }
    }

    /// Whether `hero` sees `enemy`: within `range`, and nothing solid between the hero's eyes and
    /// its chest. Without a map (a client's mirror, a bake that failed) nothing is known to stand
    /// between them.
    fn sees(&self, hero: &Hero, enemy: &Enemy, range: f32) -> bool {
        if hero.pos.distance(enemy.pos) > range {
            return false;
        }
        match self.map() {
            None => true,
            Some(map) => map.line_of_sight(
                V3::at(hero.pos, hero.z + EYE_HEIGHT),
                V3::at(enemy.pos, enemy.z + CHEST_HEIGHT),
            ),
        }
    }
}
```

In `lib.rs` add `pub use sight::{CHEST_HEIGHT, EYE_HEIGHT};`.

In `messages.rs`: `pub const PROTOCOL: u32 = 2;`, and replace the `State` variant with:

```rust
    /// The world after a step, as this player gets it: the enemies near their hero, and which of
    /// them their hero sees (spec §4.3).
    State { snapshot: Snapshot, seen: Vec<u32> },
```

- [ ] **Step 4: Run the sim's tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (everything before, 7 sight tests, 2 hero height tests).

- [ ] **Step 5: Carry the seen set through the client's session and run**

In `crates/benilla-app/src/nightfall/session.rs` (as the foundation plan's Task 9 wrote it):
- In `host_send`, replace everything from `let snapshot = world.snapshot();` to the end of the function with:

```rust
    // Each player gets the enemies near their own hero and which of them they see (spec §4.3).
    let host = h.host.lock().unwrap();
    for p in &h.peers {
        if host.backlog(p.conn) > MAX_BACKLOG {
            continue;
        }
        host.send(
            p.conn,
            &ServerMsg::State {
                snapshot: world.snapshot_for(p.player),
                seen: world.seen_by(p.player),
            },
        );
    }
}
```

- Replace `Heard`'s `State(Snapshot),` with:

```rust
    /// A state, and which enemies in it our hero sees.
    State(Snapshot, Vec<u32>),
```

- In `client_exchange`, replace the `State` arm with `ClientEvent::Message(ServerMsg::State { snapshot, seen }) => heard.push(Heard::State(snapshot, seen)),`.

In `crates/benilla-app/src/nightfall/run.rs`:
- In `Sim`, replace the `inbox` field and add `seen` after `stepped`:

```rust
    /// Joined: the host's snapshots, each with what our hero sees in it, not yet played, oldest
    /// first.
    inbox: VecDeque<(Snapshot, Vec<u32>)>,
```

```rust
    /// The enemies our hero sees (spec §4.3): worked out here alone or hosting, the host's word
    /// when joined. Sub-project 3 hides the others from the interface.
    pub seen: Vec<u32>,
```

- In `Sim::hosted` and `Sim::mirror`, add `seen: Vec::new(),` to the literal.
- Replace `apply_next` with:

```rust
    fn apply_next(&mut self) {
        if let Some((snapshot, seen)) = self.inbox.pop_front() {
            self.world.apply_snapshot(snapshot);
            self.seen = seen;
            self.stepped = true;
        }
    }
```

- In `feed_pose`, the `HeroPose { … }` literal gains `z: tf.translation.y,` (Bevy's up is WoW's `z`).
- In `step_sim`, the client loop's `if let Heard::State(snapshot) = h { sim.inbox.push_back(snapshot); }` becomes `if let Heard::State(snapshot, seen) = h { sim.inbox.push_back((snapshot, seen)); }`, and in the host's loop add `sim.seen = sim.world.seen_by(sim.me);` right after `sim.world.step();`.

- [ ] **Step 6: Run the client's tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo check -p benilla-app`
Expected: builds.

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-sim crates/benilla-app/src/nightfall/session.rs crates/benilla-app/src/nightfall/run.rs
git commit -m "Nightfall: what each hero sees, sent with a snapshot of the enemies near them

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The client bakes, and hands the map to the host's world

**Files:**
- Create: `crates/benilla-app/src/nightfall/terrain.rs`
- Modify: `crates/benilla-app/Cargo.toml`, `crates/benilla-app/src/nightfall/mod.rs`, `crates/benilla-app/src/nightfall/run.rs` (`step_sim`)

**Interfaces:**
- Consumes: `load_or_bake`, `BakeRequest`, `Baked` (Task 7); `World::{attach_map, map, warnings}` (Task 8); `nightfall::content()`, `NightfallSet`, `GameMode` (foundation Tasks 6, 8); `crate::local_state::home() -> Option<PathBuf>`; `benilla_formats::wow_data() -> Option<PathBuf>`; `crate::char_select::wow_font(&AssetServer) -> Handle<Font>`.
- Produces:
  - `terrain::Terrain` (a `Resource`) with `ready(&self) -> Option<&Arc<HeightMap>>`.
  - `terrain::Progress { Idle, Running(f32), Ready, Failed(String) }` and `terrain::line(map_name: &str, progress: &Progress) -> Option<String>`.
  - Systems `terrain::{begin, poll, spawn_line, show_line}`; the `TerrainLine` component.

- [ ] **Step 1: Add the dependency**

In `crates/benilla-app/Cargo.toml`, under the `nightfall-sim` line:

```toml
# Nightfall's height map, baked from the player's own WoW data on the host.
nightfall-bake = { path = "../nightfall-bake", default-features = false }
```

and add `"nightfall-bake/dev"` to the `dev` feature's list (after `"benilla-formats/dev"`).

- [ ] **Step 2: Write the line's test first**

`crates/benilla-app/src/nightfall/terrain.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_shows_while_baking_and_after_a_failure_only() {
        assert_eq!(line("Duskwood", &Progress::Idle), None);
        assert_eq!(
            line("Duskwood", &Progress::Running(0.42)).as_deref(),
            Some("Learning Duskwood's walls… 42%")
        );
        assert_eq!(line("Duskwood", &Progress::Ready), None);
        let failed = line("Duskwood", &Progress::Failed("no WoW data folder".into())).unwrap();
        assert!(failed.contains("no WoW data folder") && failed.contains("no enemies"));
    }
}
```

Add `mod terrain;` to `nightfall/mod.rs` beside the other `mod` lines.

- [ ] **Step 3: Write the bake's thread and its line**

At the top of `terrain.rs`:

```rust
//! Duskwood's walls for the host's rules: the baked height map (`nightfall-bake`, spec §4.1),
//! read from `benilla-config/nightfall/` or baked on a thread of its own the first time
//! Nightfall's menu shows, so neither the menu nor a run waits on it. A line at the bottom of the
//! screen, over the loading screen, says how far it has got. Only a host needs the map, but every
//! player bakes it once, so whoever hosts next starts at once. A run that starts before it is
//! ready plays without enemies until it is; a bake that fails plays without them and says why.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use nightfall_bake::{load_or_bake, BakeRequest, Baked};
use nightfall_sim::HeightMap;
use web_time::Instant;

use super::content;
use crate::game_pick::GameMode;

/// What the bake's thread hands back: the map and the seconds it took, or why there is none.
type Outcome = Result<(Baked, f32), String>;

#[derive(Default)]
enum Bake {
    #[default]
    Idle,
    /// On its thread: how far (thousandths) and, once done, what came of it.
    Running {
        progress: Arc<AtomicU32>,
        outcome: Arc<Mutex<Option<Outcome>>>,
    },
    Ready(Arc<HeightMap>),
    Failed(String),
}

/// The height map for the host's world, once there is one.
#[derive(Resource, Default)]
pub(super) struct Terrain {
    bake: Bake,
}

/// How the bake stands, for the line.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Progress {
    Idle,
    Running(f32),
    Ready,
    Failed(String),
}

impl Terrain {
    pub fn ready(&self) -> Option<&Arc<HeightMap>> {
        match &self.bake {
            Bake::Ready(map) => Some(map),
            _ => None,
        }
    }

    fn progress(&self) -> Progress {
        match &self.bake {
            Bake::Idle => Progress::Idle,
            Bake::Running { progress, .. } => {
                Progress::Running(progress.load(Ordering::Relaxed) as f32 / 1000.0)
            }
            Bake::Ready(_) => Progress::Ready,
            Bake::Failed(why) => Progress::Failed(why.clone()),
        }
    }
}

/// The line at the bottom of the screen: while baking, and after a failure; nothing otherwise.
pub(super) fn line(map_name: &str, progress: &Progress) -> Option<String> {
    match progress {
        Progress::Idle | Progress::Ready => None,
        Progress::Running(f) => Some(format!("Learning {map_name}'s walls… {:.0}%", f * 100.0)),
        Progress::Failed(why) => Some(format!(
            "{map_name}'s walls are unknown ({why}): no enemies this run"
        )),
    }
}

/// The first frame Nightfall is up: read the cache, or bake, on a thread of its own.
pub(super) fn begin(mut terrain: ResMut<Terrain>) {
    if !matches!(terrain.bake, Bake::Idle) {
        return;
    }
    let Some(data) = benilla_formats::wow_data() else {
        terrain.bake = Bake::Failed("no WoW data folder".into());
        return;
    };
    let cache = crate::local_state::home().map(|h| h.join("nightfall"));
    let progress = Arc::new(AtomicU32::new(0));
    let outcome: Arc<Mutex<Option<Outcome>>> = Arc::new(Mutex::new(None));
    let (to_progress, to_outcome) = (progress.clone(), outcome.clone());
    let content = content();
    let spawned = std::thread::Builder::new()
        .name("nightfall-bake".into())
        .spawn(move || {
            let started = Instant::now();
            let map = &content.map;
            let req = BakeRequest {
                data_dir: &data,
                map_id: map.map,
                key: &map.key,
                bounds: &map.bounds,
                props: &[],
            };
            let report =
                |f: f32| to_progress.store((f.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
            let result = load_or_bake(&req, cache.as_deref(), &report)
                .map(|baked| (baked, started.elapsed().as_secs_f32()))
                .map_err(|e| format!("{e:#}"));
            *to_outcome.lock().unwrap() = Some(result);
        });
    terrain.bake = match spawned {
        Ok(_) => Bake::Running { progress, outcome },
        Err(e) => Bake::Failed(format!("no thread for the bake: {e}")),
    };
}

/// The bake's result, once its thread is done.
pub(super) fn poll(mut terrain: ResMut<Terrain>) {
    let Bake::Running { outcome, .. } = &terrain.bake else {
        return;
    };
    let Some(result) = outcome.lock().unwrap().take() else {
        return;
    };
    terrain.bake = match result {
        Ok((baked, secs)) => {
            info!(
                "nightfall: height map {} in {secs:.1} s ({} columns hold more than terrain)",
                if baked.from_cache {
                    "read from the cache"
                } else {
                    "baked"
                },
                baked.map.complex_columns()
            );
            Bake::Ready(Arc::new(baked.map))
        }
        Err(why) => {
            warn!("nightfall: no height map: {why}");
            Bake::Failed(why)
        }
    };
}

/// The line's node, hidden until there is something to say.
#[derive(Component)]
pub(super) struct TerrainLine;

pub(super) fn spawn_line(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Over the loading screen (1000), which covers the start of a run.
            GlobalZIndex(1001),
            Visibility::Hidden,
            TerrainLine,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font,
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.85, 0.9)),
                TextShadow::default(),
            ));
        });
}

/// The line follows the bake while Nightfall is the game picked, and hides otherwise.
pub(super) fn show_line(
    mode: Res<State<GameMode>>,
    terrain: Res<Terrain>,
    mut roots: Query<(&mut Visibility, &Children), With<TerrainLine>>,
    mut texts: Query<&mut Text>,
) {
    let said = if *mode.get() == GameMode::Nightfall {
        line(&content().map.name, &terrain.progress())
    } else {
        None
    };
    for (mut visibility, children) in &mut roots {
        let want = if said.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
        let Some(said) = &said else { continue };
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != *said {
                    text.0 = said.clone();
                }
            }
        }
    }
}
```

- [ ] **Step 4: Register it, and attach the map in the host's step**

In `NightfallPlugin::build` (`nightfall/mod.rs`):
- After `.init_resource::<view::Views>()` add `.init_resource::<terrain::Terrain>()`.
- Add `terrain::spawn_line` to the `Startup` systems: `.add_systems(Startup, (menu::spawn_roots, terrain::spawn_line))`.
- Add:

```rust
            // Ungated: hides the line whenever another game is up.
            .add_systems(Update, terrain::show_line)
            .add_systems(
                Update,
                (terrain::begin, terrain::poll).chain().in_set(NightfallSet),
            )
```

In `run.rs`, give `step_sim` the parameter `terrain: Res<super::terrain::Terrain>,` and after `session::host_receive(&mut net, &mut sim.world);` add:

```rust
    // The host's world learns the map's walls once the bake is done; its enemies come with them.
    if sim.world.map().is_none() {
        if let Some(map) = terrain.ready() {
            sim.world.attach_map(map.clone());
            for w in sim.world.warnings() {
                warn!("nightfall: {w}");
            }
        }
    }
```

- [ ] **Step 5: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS (`the_line_shows_while_baking_and_after_a_failure_only` with the rest).
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 6: See the bake in a background window**

The app keeps running after a live shot, so stop each run yourself once its line is in the log (close the window, or `taskkill //IM azeroth-together.exe //F` in Git Bash).

These runs keep their state in `target/nf-home` (`BENILLA_HOME`), so the cache's place is known and the user's own `benilla-config/` is left alone. First, with no cache (`rm -rf target/nf-home/nightfall`):

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 WOW_LIVE_SHOT=target/nightfall-bake.png WOW_LIVE_SHOT_AT=10 ./target/release/azeroth-together.exe 2>&1 | tee target/nightfall-bake.log
```

Expected:
- `target/nightfall-bake.png` shows Nightfall's menu with `Learning Duskwood's walls… N%` centred at the bottom.
- Within a minute the log has `nightfall: height map baked in N s (… columns hold more than terrain)`, N under 60.
- `target/nf-home/nightfall/duskwood-<16 hex digits>.bin` exists.

Then again, the same command: the log has `nightfall: height map read from the cache in …` within a few seconds, and the screenshot at 10 s shows no line.

- [ ] **Step 7: Commit**

```bash
git add crates/benilla-app/Cargo.toml Cargo.lock crates/benilla-app/src/nightfall
git commit -m "Nightfall: the host bakes Duskwood's walls on a thread, caches them, and its world learns them

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Enemies drawn as WoW creatures

**Files:**
- Create: `crates/benilla-app/src/nightfall/enemies.rs`
- Modify: `crates/benilla-app/src/nightfall/units.rs` (only if the foundation's trim removed the creature parts), `crates/benilla-app/src/nightfall/mod.rs`, `crates/benilla-app/src/nightfall/run.rs` (`back_to_menu`)

**Interfaces:**
- Consumes: `Sim { world, me, stepped, seen }`, `Sim::alpha()` (foundation Task 9, Task 9 here); `Enemy`, `World::{enemies, hero}` (Task 8); `units::{spawn_unit, UnitSpec, UnitKind, unit_guid, wow_to_bevy, yaw_of, set_vitals, ShownVitals, FACTION_MONSTER}`; `NameCache::insert_creature(u32, Option<CreatureRecord>)`; `GuidIndex`; `MovementState`, `move_flags::{FORWARD, WALK_MODE}`; `WorldPoint::terrain_height_under(Vec3) -> Option<f32>`.
- Produces: `enemies::EnemyViews { drawn: HashMap<u32, Entity> }` (a `Resource`); `enemies::EnemyPos` (a `Component`); systems `enemies::{sync, vitals, place, trace_seen}`; pure `keep_drawn(f32, bool, f32) -> bool`, `draw_height(f32, Option<f32>) -> f32`, `gait_flags(f32, f32) -> u32`.

- [ ] **Step 1: Make sure `units.rs` can spawn a creature**

Open `crates/benilla-app/src/nightfall/units.rs`. The foundation's trim deleted what the compiler called unused; a creature needs these, exactly as `crates/benilla-app/src/together/units.rs` has them. Restore any that are missing:

```rust
/// The guid's high word of a creature.
const HIGH_CREATURE: u64 = 0xF130 << 48;

/// Hostile to every player: a red name, attackable.
pub(super) const FACTION_MONSTER: u32 = 14;
```

```rust
pub(super) enum UnitKind {
    /// A creature, named by its template `entry` (see [`crate::names::NameCache`]).
    Creature { entry: u32 },
    /// A player: the local one is driven by WoW's own character controller.
    Player,
}
```

(with its `#[derive(Clone, Copy, PartialEq, Eq)]`), and in `unit_guid` and `spawn_unit` the creature arms:

```rust
        UnitKind::Creature { .. } => HIGH_CREATURE | serial,
```

```rust
        UnitKind::Creature { entry } => (0x9, entry, benilla_protocol::EntityKind::Unit),
```

and, after the spawn, `if spec.kind != UnitKind::Player { commands.entity(e).insert(crate::net::ScriptedFacing); }` with its comment `// We turn creatures ourselves; the controller turns the player.`

Run: `cargo check -p benilla-app`
Expected: builds (warnings that `Creature` and `FACTION_MONSTER` are unused go away in Step 3).

- [ ] **Step 2: Write the drawing's pure parts with their tests**

`crates/benilla-app/src/nightfall/enemies.rs`:

```rust
//! Nightfall's enemies, drawn: a WoW creature for each enemy near the local hero, gliding between
//! the host's states on the floor the host says it stands on (a house's upper storey included),
//! walking or standing as it moves. It draws what the world holds and decides nothing; which of
//! them the hero sees ([`Sim::seen`]) is what sub-project 3 hides the rest by.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{V2, V3};

use super::content;
use super::run::Sim;
use super::units::{self, ShownVitals, UnitKind, UnitSpec};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::{CreatureRecord, NameCache};
use crate::net::{GuidIndex, ObjectStore};

/// Yards past the view distance an enemy already drawn stays drawn, so one at the edge does not
/// blink in and out.
const KEEP_EXTRA: f32 = 15.0;
/// Within this many yards of the client's own terrain height the host's floor is taken to be the
/// terrain: the client's is exact, the bake's columns are not.
const ON_TERRAIN: f32 = 1.0;

/// Which entity draws which enemy, and the creature entries already named.
#[derive(Resource, Default)]
pub(super) struct EnemyViews {
    pub drawn: HashMap<u32, Entity>,
    named: HashSet<u32>,
}

/// Where an enemy was at the last two states, for gliding between them.
#[derive(Component)]
pub(super) struct EnemyPos {
    prev: V3,
    cur: V3,
    facing: V2,
    /// The host's speed for it, and its kind's walking speed, which pick its gait.
    speed: f32,
    walk: f32,
}

impl EnemyPos {
    fn new(at: V3, facing: V2, speed: f32, walk: f32) -> Self {
        Self {
            prev: at,
            cur: at,
            facing,
            speed,
            walk,
        }
    }

    /// A new state.
    fn advance(&mut self, at: V3, facing: V2, speed: f32) {
        self.prev = self.cur;
        self.cur = at;
        if facing != V2::ZERO {
            self.facing = facing;
        }
        self.speed = speed;
    }

    /// Where it is `alpha` (0..1) of the way from the last state to this one.
    fn at(&self, alpha: f32) -> V3 {
        V3::new(
            self.prev.x + (self.cur.x - self.prev.x) * alpha,
            self.prev.y + (self.cur.y - self.prev.y) * alpha,
            self.prev.z + (self.cur.z - self.prev.z) * alpha,
        )
    }
}

/// Whether an enemy `distance` yards from the local hero is drawn: within the view distance to
/// appear, and [`KEEP_EXTRA`] beyond it to go.
pub(super) fn keep_drawn(distance: f32, drawn: bool, view: f32) -> bool {
    distance <= if drawn { view + KEEP_EXTRA } else { view }
}

/// The height to draw an enemy at: the client's own terrain when the host's floor is that
/// terrain, else the host's floor (an upper storey, a bridge).
pub(super) fn draw_height(sim_z: f32, terrain: Option<f32>) -> f32 {
    match terrain {
        Some(t) if (t - sim_z).abs() <= ON_TERRAIN => t,
        _ => sim_z,
    }
}

/// The movement flags for an enemy moving at `speed` whose kind walks at `walk`: standing,
/// walking (the walk animation) or running.
pub(super) fn gait_flags(speed: f32, walk: f32) -> u32 {
    if speed <= 0.05 {
        0
    } else if speed <= walk + 0.5 {
        move_flags::FORWARD | move_flags::WALK_MODE
    } else {
        move_flags::FORWARD
    }
}

/// The creature entry of an enemy kind: its index plus one, so the name cache files each apart.
fn entry_of(kind: usize) -> u32 {
    kind as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_enemy_appears_inside_the_view_and_goes_a_little_past_it() {
        assert!(keep_drawn(100.0, false, 120.0));
        assert!(!keep_drawn(125.0, false, 120.0));
        assert!(keep_drawn(125.0, true, 120.0));
        assert!(!keep_drawn(140.0, true, 120.0));
    }

    #[test]
    fn on_the_ground_the_clients_terrain_wins_and_upstairs_the_hosts_floor() {
        assert_eq!(draw_height(36.4, Some(36.6)), 36.6);
        assert_eq!(draw_height(40.0, Some(36.6)), 40.0);
        assert_eq!(draw_height(40.0, None), 40.0);
    }

    #[test]
    fn standing_walking_and_running_have_their_gaits() {
        assert_eq!(gait_flags(0.0, 2.5), 0);
        assert_eq!(gait_flags(2.5, 2.5), move_flags::FORWARD | move_flags::WALK_MODE);
        assert_eq!(gait_flags(7.0, 2.5), move_flags::FORWARD);
    }

    #[test]
    fn gliding_goes_from_the_last_state_to_this_one() {
        let mut p = EnemyPos::new(V3::new(0.0, 0.0, 1.0), V2::new(1.0, 0.0), 0.0, 2.5);
        p.advance(V3::new(2.0, 4.0, 3.0), V2::ZERO, 2.5);
        assert_eq!(p.at(0.5), V3::new(1.0, 2.0, 2.0));
        assert_eq!(p.facing, V2::new(1.0, 0.0));
        assert_eq!(entry_of(0), 1);
    }
}
```

Add `mod enemies;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::enemies`
Expected: PASS (4 tests).

- [ ] **Step 3: Write the systems**

Append to `enemies.rs`, above the tests:

```rust
fn trace() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_TRACE_SEEN").is_some())
}

/// Every enemy near the local hero gets a creature body, filed in the [`GuidIndex`] and
/// [`NameCache`] as the net layer files a streamed one, so Tab, the target frame and nameplates
/// treat it as WoW's own; one that moves out of range, or out of the world, goes.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<EnemyViews>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut positions: Query<&mut EnemyPos>,
) {
    let world = &sim.world;
    let content = content();
    let view = content.rules.atmosphere.view_distance;
    let me = world.hero(sim.me).map(|h| h.pos);
    for enemy in &world.enemies {
        let drawn = views.drawn.get(&enemy.id).copied();
        let distance = me.map_or(f32::INFINITY, |m| m.distance(enemy.pos));
        if !keep_drawn(distance, drawn.is_some(), view) {
            if let Some(e) = drawn {
                index.0.retain(|_, v| *v != e);
                commands.entity(e).despawn();
                views.drawn.remove(&enemy.id);
            }
            continue;
        }
        let at = V3::at(enemy.pos, enemy.z);
        if let Some(e) = drawn {
            if sim.stepped {
                if let Ok(mut p) = positions.get_mut(e) {
                    p.advance(at, enemy.facing, enemy.speed);
                }
            }
            continue;
        }
        let def = &content.enemies[enemy.kind];
        let entry = entry_of(enemy.kind);
        if views.named.insert(entry) {
            names.insert_creature(
                entry,
                Some(CreatureRecord {
                    name: def.name.clone(),
                    subname: None,
                    creature_type: 0,
                    pet_family: 0,
                    rank: 0,
                    type_flags: 0,
                    civilian: false,
                    racial_leader: false,
                    display_id: def.display,
                }),
            );
        }
        let kind = UnitKind::Creature { entry };
        let serial = u64::from(enemy.id);
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                kind,
                serial,
                display: def.display,
                scale: def.scale,
                level: def.level,
                faction: units::FACTION_MONSTER,
                bytes0: 1 << 8,
                weapons: &[],
                sheath: 0,
                extra: &[],
            },
            units::wow_to_bevy([at.x, at.y, at.z]),
            units::yaw_of(enemy.facing),
        );
        commands
            .entity(e)
            .insert(EnemyPos::new(at, enemy.facing, enemy.speed, def.walk_speed));
        index.0.insert(units::unit_guid(kind, serial), e);
        views.drawn.insert(enemy.id, e);
    }
    // Gone from the world (a client's snapshot left it out, a new run): gone from the screen.
    let present: HashSet<u32> = world.enemies.iter().map(|e| e.id).collect();
    views.drawn.retain(|id, e| {
        let keep = present.contains(id);
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// Health and level into every drawn enemy's store, for WoW's target frame.
pub(super) fn vitals(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    mut bodies: Query<(&mut ObjectStore, &mut ShownVitals)>,
) {
    let content = content();
    for enemy in &sim.world.enemies {
        if let Some((mut store, mut shown)) = views
            .drawn
            .get(&enemy.id)
            .and_then(|&e| bodies.get_mut(e).ok())
        {
            let level = content.enemies[enemy.kind].level;
            units::set_vitals(&mut store, &mut shown, enemy.hp, enemy.max_hp, level, None);
        }
    }
}

/// Every drawn enemy between its last two states, at its height, turned the way it goes,
/// walking when it moves.
pub(super) fn place(
    sim: Res<Sim>,
    world_point: WorldPoint,
    time: Res<Time<Real>>,
    mut bodies: Query<(&EnemyPos, &mut Transform, &mut MovementState)>,
) {
    let alpha = sim.alpha();
    let turn = (time.delta_secs() * 12.0).min(1.0);
    for (p, mut tf, mut mv) in &mut bodies {
        let at = p.at(alpha);
        let mut pos = units::wow_to_bevy([at.x, at.y, at.z]);
        pos.y = draw_height(at.z, world_point.terrain_height_under(pos));
        if tf.translation != pos {
            tf.translation = pos;
        }
        if p.facing != V2::ZERO {
            let target = Quat::from_rotation_y(units::yaw_of(p.facing));
            tf.rotation = tf.rotation.slerp(target, turn);
        }
        let flags = gait_flags(p.speed, p.walk);
        if mv.speed != p.speed || mv.flags != flags {
            mv.speed = p.speed;
            mv.flags = flags;
        }
    }
}

/// `NIGHTFALL_TRACE_SEEN=1`: every 2 seconds, how many of the enemies within the fog our hero
/// sees.
pub(super) fn trace_seen(sim: Res<Sim>, real: Res<Time<Real>>, mut last: Local<f32>) {
    if !trace() || real.elapsed_secs() - *last < 2.0 {
        return;
    }
    *last = real.elapsed_secs();
    let Some(me) = sim.world.hero(sim.me) else {
        return;
    };
    let fog = content().rules.atmosphere.fog_end;
    let near = sim
        .world
        .enemies
        .iter()
        .filter(|e| e.pos.distance(me.pos) <= fog)
        .count();
    info!(
        "nightfall: seeing {} of the {near} enemies within {fog} yd",
        sim.seen.len()
    );
}
```

- [ ] **Step 4: Register them, and clear them when a run ends**

In `NightfallPlugin::build`:
- Add `.init_resource::<enemies::EnemyViews>()` after `.init_resource::<view::Views>()`.
- In the run's chain `(run::feed_pose, run::step_sim, view::sync, view::write_vitals, view::place)`, add after `view::place`: `enemies::sync, enemies::vitals, enemies::place, enemies::trace_seen`.

In `run.rs`'s `back_to_menu`, add the parameter `mut enemy_views: ResMut<super::enemies::EnemyViews>,` and, after `*views = Views::default();`, `*enemy_views = super::enemies::EnemyViews::default();` (the bodies are `NightfallEntity`s, which the function already despawns).

- [ ] **Step 5: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 6: See the enemies, solo**

With the cache from Task 10 in place:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_TRACE_SEEN=1 WOW_SHOT_REQUIRE="Nightbane Worgen" WOW_LIVE_SHOT=target/nightfall-enemies.png WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=3 WOW_LIVE_SHOT_AT=30 ./target/release/azeroth-together.exe 2>&1 | tee target/nightfall-enemies.log
```

Expected:
- The log has `nightfall: height map read from the cache …`, no `nightfall: spawns.ron …` warnings (or, if some, each names a group whose point near the entry lands in a building: note it for Task 12), and `nightfall: seeing N of the M enemies within 40 yd` lines with M of 1 or more.
- `target/nightfall-enemies-000.png` to `-003.png` (taken once the worgen is in view) show the Nightbane Worgen walking (the walk animation, not a run) along its square near the hero in the fog, and other creatures (the skeleton at its post, spiders and ravagers) where the hero passes.
Stop the run.

- [ ] **Step 7: See a patrol walk around a building**

Find the nearest building to the entry:
`WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo run --release -p nightfall-bake --example buildings -- -10051.8 -792.5 800`
Take the first line's position as `(X, Y, Z)`. Copy the content: `cp -r content/nightfall target/nf-house`. In `target/nf-house/spawns.ron`, set the patrol's route to the corners of a 50-yard square around the building: `[(X−25, Y−25), (X+25, Y−25), (X+25, Y+25), (X−25, Y+25)]` (the numbers worked out). In `target/nf-house/map.ron`, put `(X−30, Y, Z)` first among the entries. Then:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_CONTENT=target/nf-house WOW_SHOT_REQUIRE="Nightbane Worgen" WOW_LIVE_SHOT=target/nightfall-house.png WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=5 WOW_LIVE_SHOT_AT=30 ./target/release/azeroth-together.exe 2>&1 | tee target/nightfall-house.log
```

Expected: the shots show the worgen walking the square's sides past the building, never through its walls; any corner inside the building is named in a `no path to patrol point` warning and skipped. `target/nf-house` is a scratch copy: do not commit it.

- [ ] **Step 8: See the enemies in co-op on one PC**

Start the host in the background, and a few seconds later the client:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_NAME=Host WOW_LIVE_SHOT=target/host-enemies.png WOW_LIVE_SHOT_AT=50 ./target/release/azeroth-together.exe 2>&1 | tee target/host.log
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend NIGHTFALL_TRACE_SEEN=1 WOW_LIVE_SHOT=target/client-enemies.png WOW_LIVE_SHOT_AT=45 ./target/release/azeroth-together.exe 2>&1 | tee target/client.log
```

Expected: both screenshots show the same creatures in the same places (give or take the client's playback delay), and the client's log has `nightfall: seeing … enemies` lines, the seen set coming from the host. Stop both.

- [ ] **Step 9: Commit**

```bash
git add crates/benilla-app/src/nightfall
git commit -m "Nightfall: enemies drawn as WoW creatures, walking where the host's paths go

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Docs and the full check

**Files:**
- Modify: `CLAUDE.md`, `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line)

- [ ] **Step 1: Document world knowledge in `CLAUDE.md`**

Under "Where things live", after the `crates/nightfall-sim/` entry the foundation added, add:

```markdown
- Nightfall's world knowledge (sub-project 2): `nightfall-sim/src/heightmap/` is the layered
  height map (half-yard columns of solid spans; floors, indoors, line of sight, A* paths with a
  cache), tested on hand-made maps (`heightmap/testmap.rs`). `crates/nightfall-bake/` bakes it from
  the player's own WoW data (terrain, WMO walking collision, M2 collision hulls; impassable chunks
  and everything outside the bounds blocked) and caches it in
  `benilla-config/nightfall/<map>-<hash>.bin`; the client (`nightfall/terrain.rs`) bakes it on a
  thread when Nightfall's menu first shows and hands it to the host's world. Enemies
  (`nightfall-sim/src/enemies.rs`, placed by `content/nightfall/spawns.ron` from the kinds in
  `enemies.ron`) stand, patrol and wander on its paths; each player's snapshot carries the enemies
  near their hero and which they see (`World::seen_by`), kept in the client's `Sim::seen`.
```

Under "Build, test, run", add:

```markdown
- Nightfall's bake: `cargo test -p nightfall-bake` (its Darkshire test needs
  `WOW_DATA=D:/Games/azeroth-together/WoW/Data` in a worktree without `WoW/Data`, and skips without
  it); `cargo run --release -p nightfall-bake --example bake_duskwood` times the whole bake;
  `cargo run -p nightfall-bake --example buildings -- <x> <y> [radius]` lists the buildings around
  a point, for placing enemies. `NIGHTFALL_TRACE_SEEN=1` logs how many enemies the hero sees.
  Enemy groups in `spawns.ron` marked `measure:` still sit near the entry: move each to its place
  once measured in the game (`NIGHTFALL_TRACE_POS=1`); a point no path reaches is skipped and named
  in the log as `nightfall: spawns.ron: …`.
```

- [ ] **Step 2: Mark the spec**

In the spec's status line, after the sentence the foundation added (`Sub-project 1 built …`), add: `Sub-project 2 built (plan docs/superpowers/plans/2026-10-17-nightfall-world-knowledge.md).`

- [ ] **Step 3: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS (the Darkshire bake test skips without `WOW_DATA`).
Run: `WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo test --release -p nightfall-bake --test duskwood`
Expected: PASS.
Run: `cargo test -p benilla-app --lib nightfall game_pick`
Expected: PASS.
Run (only where `rustup target list --installed` lists `wasm32-unknown-unknown`): `cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu`
Expected: builds, as `check.yml` will check it.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md
git commit -m "Docs: Nightfall's height map, its bake, enemies and their test hooks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
