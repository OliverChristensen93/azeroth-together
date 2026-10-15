# Extraction Mode, Core Loop: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A playable extraction run in Elwynn: the whole forest open, Goldshire as a safe ring, heat that climbs outside and drives the horde, hunters and rewards, and timed exits where a flight takes whoever stands in its ring.

**Architecture:** `together-sim` gains a `Mode` (`Arena` or `Extraction`) chosen by the zone. An extraction zone in `zones.ron` points at a map in the new `content/extraction.ron`. Extraction rules live in a new `world/extraction.rs` (an `impl World` block, as `world/arena.rs` already is) and run at a few seams in `World::step`. Its state travels in every `Snapshot`, so co-op needs no new messages. The client reads the new state to draw rings, beams and flights, and to fill the HUD.

**Tech Stack:** Rust (GNU toolchain on Windows), `serde` + `ron` for content, `postcard` for snapshots, Bevy 0.18 for the client.

**Spec:** `docs/superpowers/specs/2026-10-15-extraction-mode-design.md`. This plan covers the spec's build steps 1–3 (foundations, core loop, extraction). Steps 4–6 (elite circles and rares, hold circles, Goldshire merchant and tuning) get their own plan after playtesting this one.

## Global Constraints

- Work only in the worktree `D:\Games\azeroth-together-extraction` on branch `extraction-mode`. Never switch branches in `D:\Games\azeroth-together`: another session works there.
- `D:` is exFAT, which records no file owners, so git refuses the worktree as "dubious". Run git there as `git -c safe.directory=D:/Games/azeroth-together-extraction …`, written `$G …` below:
  `G="git -c safe.directory=D:/Games/azeroth-together-extraction"`.
- Every build needs MinGW on `PATH` first:
  `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`.
- Fast tests: `cargo test -p together-sim -p together-net`. The client builds with `cargo build --release -p azeroth-together`, which takes about 17 minutes the first time in this worktree and about 6 after. Run it in the background.
- Game data is not in the worktree (`WoW/Data` is gitignored). Every run from here sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data`.
- Never launch a normal (foreground, fullscreen) run. Test runs always set `WOW_BG=1 WOW_WIN=1280x720`.
- **The arena must not change.** Every existing test in `together-sim` passes. The only edits to existing tests: adding `extracted: false` where a `Hero` is built literally, and skipping extraction zones in the shipped-content loops that fight in every zone.
- Every tuning number lives in `content/extraction.ron`. Consts in Rust only for drawing (rune displays, serials).
- Touch no engine crate (`crates/benilla-*`) outside `crates/benilla-app/src/together/`.
- Match the code's style: doc comments in plain sentences, small functions, no comment noise.
- End every commit message with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **An extracted hero at a later level-up** must get no cards and must not hold the run. Test in Task 6: `an_extracted_hero_gets_no_cards_and_never_holds_the_run`.
2. **`solo_extract: false` with a hero who falls while the flight waits:** the flight leaves once every *living* hero is aboard. Test in Task 6: `a_waiting_flight_leaves_when_the_rest_are_aboard_or_down`.
3. **A co-op client** sees the host's heat, exits and flights, and a client with a different `extraction.ron` is refused. Tests: `a_mirror_gets_the_extraction_state` (Task 3); the content hash covers `extraction.ron` (Task 8, step 2).
4. **A hero pushed back onto the outline's edge** can walk straight back in. Test in Task 3: `a_pose_past_the_outline_is_held_on_it_and_one_inside_is_taken`.
5. **The arena behaves exactly as before.** All existing world and shipped tests pass; Task 3 adds `an_arena_zone_plays_the_arena`.

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/together-sim/src/extraction.rs` | create | Extraction content types (`ExtractionDef` and parts), run state (`Mode`, `Extraction`, `ExitState`), `Bounds` geometry. Test fixtures for maps. |
| `crates/together-sim/src/content.rs` | modify | `Content::extractions`, `Content::with_extractions`, `ZoneDef::extraction_ix`. |
| `crates/together-sim/src/lib.rs` | modify | Declare and re-export the module. |
| `crates/together-sim/src/world.rs` | modify | `World::mode`, `Snapshot::mode`, `Hero::extracted`, new `Event`s, the seams in `step`, `move_heroes`, `place`, `reap_enemies`, `deal`, `revive_heroes`, `level_up`. |
| `crates/together-sim/src/world/extraction.rs` | create | The extraction rules: bounds and placing, heat, spawning by heat, hunters, recycling, exits and flights. With their tests. |
| `crates/together-sim/tests/shipped_content.rs` | modify | Load `extraction.ron`; a headless extraction run. |
| `content/zones.ron`, `content/extraction.ron` | modify, create | The `elwynn_extraction` zone and its map. |
| `crates/together-net/src/lib.rs` | modify | `PROTOCOL` 3 → 4. |
| `crates/benilla-app/src/together/mod.rs` | modify | Load `extraction.ron` and hash it; bounds in `feed_pose`; `TOGETHER_TRACE_POS`; autoplay toward exits; showcase opens exits. |
| `crates/benilla-app/src/together/view.rs` | modify | Safe ring, exit rings and beams, flights, hidden extracted heroes, event text. |
| `crates/benilla-app/src/together/hud.rs` | modify | Heat and exit lines, the extracted and end screens. |
| `CLAUDE.md` | modify | Where extraction lives and how to test it. |

---

### Task 1: Bring in the invite-code commit

`megabonk-systems` branched before `4ca19bc` (join by invite code). A trial merge of `phase-1-sim-core` into `extraction-mode` is clean.

**Files:** none edited by hand.

**Interfaces:** consumes nothing; later tasks assume `phase-1-sim-core`'s co-op code is present.

- [ ] **Step 1: Merge**

```bash
cd /d/Games/azeroth-together-extraction
G="git -c safe.directory=D:/Games/azeroth-together-extraction"
$G merge --no-ff phase-1-sim-core -m "Merge phase-1-sim-core: join by invite code

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

Expected: `Merge made by the 'ort' strategy.` If it reports conflicts, stop and report them; don't resolve them by guessing.

- [ ] **Step 2: Check the protocol number survived**

Run: `grep -n "PROTOCOL: u32" crates/together-net/src/lib.rs`
Expected: `pub const PROTOCOL: u32 = 3;`

- [ ] **Step 3: Run the fast tests**

```bash
export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo test -p together-sim -p together-net
```

Expected: all pass. The ignored online invite test stays ignored.

---

### Task 2: Extraction content types, parsing and bounds

**Files:**
- Create: `crates/together-sim/src/extraction.rs`
- Modify: `crates/together-sim/src/content.rs` (`ZoneDef`, `Content`, `Content::from_ron`, new `Content::with_extractions`)
- Modify: `crates/together-sim/src/lib.rs`

**Interfaces:**
- Produces, all `pub` and re-exported from the crate root:
  - `ExtractionDef { key, zone, boundary: Vec<[f32; 2]>, safe: SafeDef, recycle: f32, heat: HeatDef, exits: Vec<ExitDef>, flight_display: u32, settings: ExtractionSettings, zone_ix: usize }`
  - `SafeDef { center: [f32; 2], radius, margin, cool }`
  - `HeatDef { outside, kill, flight, floor_from, floor_per_min, reward_luck, quiet_range, quiet_secs, skulls: Vec<SkullDef>, hunters: HunterDef }`
  - `SkullDef { at, spawns, tier: usize, toughness, hunters_every, reward }`
  - `HunterDef { pack: (u32, u32), ring: (f32, f32), cone, speed }`
  - `ExitDef { name, pos: [f32; 2], radius, opens_at, call, flight, recall }`
  - `ExtractionSettings { heat_shared, lose_them, solo_extract, timed_boss }`
  - Methods on `ExtractionDef`: `safe_center() -> V2`, `in_safe(V2) -> bool`, `in_margin(V2) -> bool`, `exit_pos(usize) -> V2`, `bounds() -> Bounds`, `skull_index(f32) -> usize`, `skull(f32) -> &SkullDef`, `floor(f32) -> f32`.
  - `Bounds::{Circle { center, radius }, Polygon(Vec<V2>)}` with `clamp(V2) -> V2` and `inside(V2, margin: f32) -> bool`.
  - `Mode::{Arena, Extraction(Extraction)}`; `Extraction { def, heat, heat_by: Vec<(PlayerId, f32)>, floor, peak, told: Vec<(Option<PlayerId>, u8)>, quiet, hunt, exits: Vec<ExitState> }` with `Extraction::new(usize, &ExtractionDef)`; `ExitState { open, calling, flight: Option<f32>, landed, cooldown }`.
  - `Content::extractions: Vec<ExtractionDef>`, `Content::with_extractions(self, &str) -> Result<Content, ContentError>`, `ZoneDef::extraction_ix: Option<usize>`.
  - Test fixtures in `extraction::tests` (crate-visible): `TEST_ZONES`, `TEST_MAPS`, `test_content(settings: &str) -> Content`.

- [ ] **Step 1: Write the module with its tests, types first**

Create `crates/together-sim/src/extraction.rs`:

```rust
//! Extraction mode's data and state.
//!
//! The whole of a zone is open, inside an outline drawn round it. A safe ring (Goldshire) spawns
//! nothing and lets heat cool; out in the forest heat climbs with time and kills, and with it the
//! horde, the hunters sent to cut heroes off from home, and the rewards. Exits open on a timer: a
//! hero standing in an open one calls a flight, which takes whoever stands in its ring when it
//! lands. The rules that use all this are in `world/extraction.rs`.

use serde::{Deserialize, Serialize};

use crate::math::V2;
use crate::world::PlayerId;

/// One extraction map, as `content/extraction.ron` describes it.
#[derive(Clone, Debug, Deserialize)]
pub struct ExtractionDef {
    pub key: String,
    /// The zone it is played in, a key into `zones.ron`: its enemies, shrines, centre and map.
    pub zone: String,
    /// The playable area's outline in WoW `(x, y)`: a convex polygon, corners in order.
    pub boundary: Vec<[f32; 2]>,
    pub safe: SafeDef,
    /// Ordinary enemies further than this from every living hero are let go.
    pub recycle: f32,
    pub heat: HeatDef,
    pub exits: Vec<ExitDef>,
    /// The creature display a flight comes down as.
    pub flight_display: u32,
    #[serde(default)]
    pub settings: ExtractionSettings,
    /// [`ExtractionDef::zone`] resolved to an index into [`crate::Content::zones`].
    #[serde(skip)]
    pub zone_ix: usize,
}

/// The safe ring: nothing new spawns near it, and heat cools inside it.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SafeDef {
    pub center: [f32; 2],
    pub radius: f32,
    /// Nothing spawns within `radius + margin` of the centre.
    pub margin: f32,
    /// Heat lost per second while it may cool.
    pub cool: f32,
}

/// How heat is gained, what floor it keeps, and what each skull of it does.
#[derive(Clone, Debug, Deserialize)]
pub struct HeatDef {
    /// Gained each second outside the safe ring.
    pub outside: f32,
    /// Gained per kill.
    pub kill: f32,
    /// Gained each second a flight is on its way.
    pub flight: f32,
    /// The nightfall floor starts rising this many seconds in...
    pub floor_from: f32,
    /// ...by this much a minute.
    pub floor_per_min: f32,
    /// Luck added for each point of reward multiplier over 1.
    pub reward_luck: f32,
    /// `lose_them`: no enemy within this many yards of any living hero...
    pub quiet_range: f32,
    /// ...for this many seconds drops a skull.
    pub quiet_secs: f32,
    /// The skulls from 0 up, each starting at its `at` heat.
    pub skulls: Vec<SkullDef>,
    pub hunters: HunterDef,
}

/// What one skull of heat does.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct SkullDef {
    /// The heat it starts at.
    pub at: f32,
    /// The horde's size, times the arena's.
    pub spawns: f32,
    /// Roster tiers added to what the clock brings.
    pub tier: usize,
    /// Enemy health and damage, times.
    pub toughness: f32,
    /// Seconds between hunter packs; 0 for none.
    pub hunters_every: f32,
    /// XP and gold from kills, times.
    pub reward: f32,
}

/// The packs sent to cut a hot hero off from home.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct HunterDef {
    /// Enemies in a pack, fewest and most.
    pub pack: (u32, u32),
    /// How far from the hero they come, nearest and furthest.
    pub ring: (f32, f32),
    /// The cone around the way home they come from, in degrees.
    pub cone: f32,
    /// Their speed, times.
    pub speed: f32,
}

/// A place to leave from.
#[derive(Clone, Debug, Deserialize)]
pub struct ExitDef {
    pub name: String,
    pub pos: [f32; 2],
    pub radius: f32,
    /// Seconds into the run it opens.
    pub opens_at: f32,
    /// Seconds a hero stands in it to call a flight.
    pub call: f32,
    /// Seconds the flight takes to come.
    pub flight: f32,
    /// Seconds after a flight leaves before another can be called.
    pub recall: f32,
}

/// Rules to playtest both ways.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ExtractionSettings {
    /// One heat for the party, or one per hero.
    #[serde(default = "yes")]
    pub heat_shared: bool,
    /// A quiet spell with no enemy near drops a skull.
    #[serde(default)]
    pub lose_them: bool,
    /// A flight takes whoever is in its ring, or waits for every living hero.
    #[serde(default = "yes")]
    pub solo_extract: bool,
    /// The zone's boss comes on its arena timer.
    #[serde(default)]
    pub timed_boss: bool,
}

fn yes() -> bool {
    true
}

impl Default for ExtractionSettings {
    fn default() -> Self {
        Self {
            heat_shared: true,
            lose_them: false,
            solo_extract: true,
            timed_boss: false,
        }
    }
}

impl ExtractionDef {
    pub fn safe_center(&self) -> V2 {
        V2::new(self.safe.center[0], self.safe.center[1])
    }

    /// Inside the safe ring.
    pub fn in_safe(&self, p: V2) -> bool {
        p.distance(self.safe_center()) <= self.safe.radius
    }

    /// Too near the safe ring for anything to spawn.
    pub fn in_margin(&self, p: V2) -> bool {
        p.distance(self.safe_center()) < self.safe.radius + self.safe.margin
    }

    pub fn exit_pos(&self, exit: usize) -> V2 {
        let p = self.exits[exit].pos;
        V2::new(p[0], p[1])
    }

    pub fn bounds(&self) -> Bounds {
        Bounds::Polygon(self.boundary.iter().map(|p| V2::new(p[0], p[1])).collect())
    }

    /// The skull `heat` stands at: the last whose `at` it has reached.
    pub fn skull_index(&self, heat: f32) -> usize {
        self.heat.skulls.iter().rposition(|s| heat >= s.at).unwrap_or(0)
    }

    pub fn skull(&self, heat: f32) -> &SkullDef {
        &self.heat.skulls[self.skull_index(heat)]
    }

    /// The nightfall floor `elapsed` seconds in.
    pub fn floor(&self, elapsed: f32) -> f32 {
        ((elapsed - self.heat.floor_from) / 60.0 * self.heat.floor_per_min).max(0.0)
    }
}

/// Where heroes may go: the arena's circle, or an extraction map's outline.
#[derive(Clone, Debug, PartialEq)]
pub enum Bounds {
    Circle { center: V2, radius: f32 },
    /// A convex polygon, corners in order.
    Polygon(Vec<V2>),
}

impl Bounds {
    /// `p`, or the nearest point inside when it is outside.
    pub fn clamp(&self, p: V2) -> V2 {
        match self {
            Bounds::Circle { center, radius } => {
                let off = p - *center;
                if off.length() > *radius {
                    *center + off.normalize_or_zero() * *radius
                } else {
                    p
                }
            }
            Bounds::Polygon(poly) => {
                if contains(poly, p) {
                    p
                } else {
                    nearest_on_outline(poly, p)
                }
            }
        }
    }

    /// Whether `p` is inside with at least `margin` yards to spare.
    pub fn inside(&self, p: V2, margin: f32) -> bool {
        match self {
            Bounds::Circle { center, radius } => p.distance(*center) <= radius - margin,
            Bounds::Polygon(poly) => {
                contains(poly, p) && nearest_on_outline(poly, p).distance(p) >= margin
            }
        }
    }
}

/// Whether `p` lies inside `poly`, by the even-odd rule.
fn contains(poly: &[V2], p: V2) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The point on `poly`'s outline nearest `p`.
fn nearest_on_outline(poly: &[V2], p: V2) -> V2 {
    let mut best = poly[0];
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let ab = b - a;
        let ap = p - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let t = if len2 > 0.0 {
            ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = a + ab * t;
        if q.distance(p) < best.distance(p) {
            best = q;
        }
    }
    best
}

/// Which rules a run plays by.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    /// A leash round the zone's centre, the horde on the clock, a boss every few minutes.
    Arena,
    Extraction(Extraction),
}

/// An extraction run's state. It is in every snapshot, so a co-op client's HUD reads it too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extraction {
    /// Index into [`crate::Content::extractions`].
    pub def: usize,
    /// The party's heat, when `heat_shared`.
    pub heat: f32,
    /// Each hero's heat, when not.
    pub heat_by: Vec<(PlayerId, f32)>,
    /// The nightfall floor now.
    pub floor: f32,
    /// The most heat anyone has had this run.
    pub peak: f32,
    /// The skulls last told of (`None` the party's, else a hero's), so a change is told once.
    pub told: Vec<(Option<PlayerId>, u8)>,
    /// Seconds no enemy has come near any living hero (`lose_them`).
    pub quiet: f32,
    /// Seconds until the next hunter pack may come.
    pub hunt: f32,
    pub exits: Vec<ExitState>,
}

/// How one exit stands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExitState {
    pub open: bool,
    /// How far a hero standing in it has called a flight, 0 to 1.
    pub calling: f32,
    /// Seconds until the flight on its way lands.
    pub flight: Option<f32>,
    /// A flight has landed and waits for everyone (`solo_extract` off).
    pub landed: bool,
    /// Seconds until another flight can be called.
    pub cooldown: f32,
}

impl Extraction {
    pub fn new(def_ix: usize, def: &ExtractionDef) -> Self {
        Self {
            def: def_ix,
            heat: 0.0,
            heat_by: Vec::new(),
            floor: 0.0,
            peak: 0.0,
            told: Vec::new(),
            quiet: 0.0,
            hunt: 0.0,
            exits: vec![ExitState::default(); def.exits.len()],
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::content::tests::{ABILITIES, CLASSES, ENEMIES};
    use crate::content::{Content, ContentError, ContentSources};

    /// A small square world: the safe ring at the origin, exits east and south.
    pub(crate) const TEST_ZONES: &str = r#"[
        (key: "goldshire", name: "Goldshire", map: 0, center: (0.0, 0.0, 0.0), leash: 1000.0,
         enemies: ["wolf"], boss: "hogger",
         interactables: [(kind: Chest, count: 1)], shrines_at_once: 1, pots: 0),
    ]"#;

    /// `SETTINGS` is replaced by a test's settings, e.g. `heat_shared: false`.
    pub(crate) const TEST_MAPS: &str = r#"[
        (key: "goldshire", zone: "goldshire",
         boundary: [(-300.0, -300.0), (300.0, -300.0), (300.0, 300.0), (-300.0, 300.0)],
         safe: (center: (0.0, 0.0), radius: 40.0, margin: 50.0, cool: 15.0),
         recycle: 120.0,
         heat: (outside: 1.0, kill: 0.5, flight: 4.0, floor_from: 180.0, floor_per_min: 40.0,
                reward_luck: 0.25, quiet_range: 40.0, quiet_secs: 10.0,
                skulls: [
                    (at: 0.0, spawns: 1.0, tier: 0, toughness: 1.0, hunters_every: 0.0, reward: 1.0),
                    (at: 100.0, spawns: 1.3, tier: 1, toughness: 1.15, hunters_every: 0.0, reward: 1.5),
                    (at: 250.0, spawns: 1.6, tier: 1, toughness: 1.3, hunters_every: 0.0, reward: 2.0),
                    (at: 450.0, spawns: 2.0, tier: 2, toughness: 1.5, hunters_every: 20.0, reward: 2.75),
                    (at: 700.0, spawns: 2.5, tier: 3, toughness: 1.75, hunters_every: 15.0, reward: 3.5),
                    (at: 1000.0, spawns: 3.0, tier: 4, toughness: 2.0, hunters_every: 10.0, reward: 5.0),
                ],
                hunters: (pack: (6, 10), ring: (50.0, 70.0), cone: 60.0, speed: 1.2)),
         exits: [
            (name: "East Gate", pos: (200.0, 0.0), radius: 15.0, opens_at: 60.0, call: 3.0,
             flight: 45.0, recall: 30.0),
            (name: "South Camp", pos: (0.0, -250.0), radius: 15.0, opens_at: 120.0, call: 3.0,
             flight: 45.0, recall: 30.0),
         ],
         flight_display: 1566,
         settings: (timed_boss: false, SETTINGS)),
    ]"#;

    pub(crate) fn test_content(settings: &str) -> Content {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: ENEMIES,
            zones: TEST_ZONES,
        })
        .and_then(|c| c.with_extractions(&TEST_MAPS.replace("SETTINGS", settings)))
        .unwrap_or_else(|e| panic!("test content: {e}"))
    }

    fn maps_with(from: &str, to: &str) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            abilities: ABILITIES,
            enemies: ENEMIES,
            zones: TEST_ZONES,
        })?
        .with_extractions(&TEST_MAPS.replace("SETTINGS", "").replace(from, to))
    }

    #[test]
    fn a_map_parses_and_is_tied_to_its_zone() {
        let c = test_content("lose_them: true");
        assert_eq!(c.extractions.len(), 1);
        let map = &c.extractions[0];
        assert_eq!(map.zone_ix, 0);
        assert_eq!(c.zones[0].extraction_ix, Some(0));
        assert!(map.settings.heat_shared && map.settings.lose_them && map.settings.solo_extract);
        assert_eq!(map.exits[1].name, "South Camp");
    }

    #[test]
    fn a_map_for_a_missing_zone_is_refused() {
        let err = maps_with(r#"zone: "goldshire""#, r#"zone: "nowhere""#).unwrap_err();
        assert!(matches!(err, ContentError::UnknownKey { .. }), "{err}");
    }

    #[test]
    fn an_exit_outside_the_outline_is_refused() {
        let err = maps_with("pos: (200.0, 0.0)", "pos: (900.0, 0.0)").unwrap_err();
        assert!(
            matches!(err, ContentError::Invalid { why: "has an exit outside its boundary", .. }),
            "{err}"
        );
    }

    #[test]
    fn skulls_must_start_at_zero_and_rise() {
        let err = maps_with("(at: 250.0,", "(at: 50.0,").unwrap_err();
        assert!(matches!(err, ContentError::Invalid { .. }), "{err}");
    }

    #[test]
    fn skulls_and_the_floor_follow_heat_and_time() {
        let c = test_content("");
        let map = &c.extractions[0];
        assert_eq!(map.skull_index(0.0), 0);
        assert_eq!(map.skull_index(99.9), 0);
        assert_eq!(map.skull_index(100.0), 1);
        assert_eq!(map.skull_index(5000.0), 5);
        assert_eq!(map.skull(460.0).hunters_every, 20.0);
        assert_eq!(map.floor(100.0), 0.0);
        assert!((map.floor(240.0) - 40.0).abs() < 1e-4);
    }

    #[test]
    fn an_outline_holds_a_point_on_its_nearest_edge() {
        let square = Bounds::Polygon(vec![
            V2::new(-10.0, -10.0),
            V2::new(10.0, -10.0),
            V2::new(10.0, 10.0),
            V2::new(-10.0, 10.0),
        ]);
        assert_eq!(square.clamp(V2::new(3.0, 4.0)), V2::new(3.0, 4.0));
        assert_eq!(square.clamp(V2::new(25.0, 4.0)), V2::new(10.0, 4.0));
        assert_eq!(square.clamp(V2::new(-30.0, -30.0)), V2::new(-10.0, -10.0));
        assert!(square.inside(V2::new(0.0, 0.0), 5.0));
        assert!(!square.inside(V2::new(8.0, 0.0), 5.0));
        assert!(!square.inside(V2::new(11.0, 0.0), 0.0));
    }

    #[test]
    fn a_circle_holds_a_point_on_its_rim_as_the_arena_leash_does() {
        let ring = Bounds::Circle {
            center: V2::new(100.0, 0.0),
            radius: 10.0,
        };
        assert_eq!(ring.clamp(V2::new(130.0, 0.0)), V2::new(110.0, 0.0));
        assert_eq!(ring.clamp(V2::new(105.0, 0.0)), V2::new(105.0, 0.0));
        assert!(ring.inside(V2::new(104.0, 0.0), 3.0));
        assert!(!ring.inside(V2::new(108.0, 0.0), 3.0));
    }
}
```

- [ ] **Step 2: Wire the module into the crate**

In `crates/together-sim/src/lib.rs`, add `mod extraction;` after `mod content;`, and after the `pub use content::{…};` block add:

```rust
pub use extraction::{
    Bounds, ExitDef, ExitState, Extraction, ExtractionDef, ExtractionSettings, HeatDef, HunterDef,
    Mode, SafeDef, SkullDef,
};
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p together-sim extraction::tests`
Expected: compile errors: no method `with_extractions` on `Content`, no field `extraction_ix` on `ZoneDef`, no field `extractions` on `Content`.

- [ ] **Step 4: Add the content side**

In `crates/together-sim/src/content.rs`:

1. Add the import at the top with the other `use` lines: `use crate::extraction::ExtractionDef;`
2. In `ZoneDef`, after `enemy_ix`, add:

```rust
    /// The extraction map played in this zone (an index into [`Content::extractions`]), set by
    /// [`Content::with_extractions`]; `None` for an arena.
    #[serde(skip)]
    pub extraction_ix: Option<usize>,
```

3. In `Content`, after `rules`, add:

```rust
    /// Extraction maps: none until [`Content::with_extractions`] reads a file.
    pub extractions: Vec<ExtractionDef>,
```

4. In `Content::from_ron`'s final `Ok(Self { … })`, add `extractions: Vec::new(),` after `rules: Rules::default(),`.
5. After `with_rules`, add:

```rust
    /// These contents with the extraction maps in `text` (`extraction.ron`), each tied to its zone.
    pub fn with_extractions(mut self, text: &str) -> Result<Self, ContentError> {
        const FILE: &str = "extraction.ron";
        let invalid = |owner: &str, why: &'static str| ContentError::Invalid {
            file: FILE,
            owner: owner.to_owned(),
            why,
        };
        let mut maps: Vec<ExtractionDef> = parse(FILE, text)?;
        index(FILE, maps.iter().map(|m| m.key.as_str()))?;
        let zones = index("zones.ron", self.zones.iter().map(|z| z.key.as_str()))?;
        for (i, map) in maps.iter_mut().enumerate() {
            map.zone_ix = resolve(&zones, FILE, &map.key, &map.zone)?;
            if map.boundary.len() < 3 {
                return Err(invalid(&map.key, "has fewer than three boundary corners"));
            }
            let skulls = &map.heat.skulls;
            if skulls.first().map(|s| s.at) != Some(0.0)
                || skulls.windows(2).any(|w| w[1].at <= w[0].at)
            {
                return Err(invalid(&map.key, "needs skulls that start at 0 heat and rise"));
            }
            let bounds = map.bounds();
            if !bounds.inside(map.safe_center(), 0.0) {
                return Err(invalid(&map.key, "has its safe ring outside its boundary"));
            }
            if (0..map.exits.len()).any(|e| !bounds.inside(map.exit_pos(e), 0.0)) {
                return Err(invalid(&map.key, "has an exit outside its boundary"));
            }
            self.zones[map.zone_ix].extraction_ix = Some(i);
        }
        self.extractions = maps;
        Ok(self)
    }
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p together-sim`
Expected: every test passes, including the seven new `extraction::tests`.

- [ ] **Step 6: Commit**

```bash
$G add crates/together-sim/src/extraction.rs crates/together-sim/src/content.rs crates/together-sim/src/lib.rs
$G commit -m "Sim: extraction maps in content, with bounds and run state

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The `Mode` seam in `World`

An extraction zone's world plays by its outline. Interactables stay clear of the safe ring. Snapshots carry the mode, and heroes can be marked extracted. The six new events exist but aren't sent yet.

**Files:**
- Create: `crates/together-sim/src/world/extraction.rs`
- Modify: `crates/together-sim/src/world.rs`
- Modify: `crates/together-net/src/lib.rs` (`PROTOCOL`)
- Modify: `crates/benilla-app/src/together/view.rs` (`apply_events`: one no-op arm, so the client still compiles)

**Interfaces:**
- Consumes (Task 2): `Mode`, `Extraction::new`, `ExtractionDef::{bounds, in_margin, safe_center}`, `Bounds`, `Content::extractions`, `ZoneDef::extraction_ix`.
- Produces:
  - `World::mode: Mode` (pub field); `Snapshot::mode: Mode`; `Hero::extracted: bool`.
  - `World::bounds(&self) -> Bounds` (pub).
  - `World::map<'c>(&self, &'c Content) -> Option<&'c ExtractionDef>` and `World::run_mut(&mut self) -> Option<&mut Extraction>` (private to `world`).
  - `World::placeable(&self, V2) -> V2` (`pub(super)`), used by `World::place`.
  - New `Event` variants: `HeatChanged { player: Option<PlayerId>, skulls: u8 }`, `HuntersCame { near: PlayerId }`, `ExitOpened { exit: u8 }`, `FlightCalled { exit: u8, by: PlayerId }`, `FlightLanded { exit: u8 }`, `HeroExtracted { player: PlayerId, exit: u8 }`.
  - Test helpers in `world::extraction::tests`: `run(settings, heroes) -> World`, `hush`, `at`, `secs`, `state`, `state_mut`.

- [ ] **Step 1: Write the failing tests**

Create `crates/together-sim/src/world/extraction.rs` with only its tests for now:

```rust
//! Extraction mode's rules ([`crate::extraction`]): bounds and where things may stand, heat,
//! spawning by heat, hunters, letting far enemies go, and the exits and their flights. Each runs
//! only in an extraction run; an arena never reaches them.

use super::*;
use crate::extraction::ExtractionDef;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::tests::test_content;

    /// An extraction run on the test map with `heroes` mages, settings as `settings` says.
    pub(super) fn run(settings: &str, heroes: u8) -> World {
        let mut w = World::new(Arc::new(test_content(settings)), "goldshire", 7).unwrap();
        for p in 0..heroes {
            w.add_hero(p, "mage").unwrap();
        }
        w
    }

    /// No horde: only what a test puts there.
    pub(super) fn hush(w: &mut World) {
        w.spawn_timer = f32::MAX;
    }

    pub(super) fn at(w: &mut World, player: PlayerId, x: f32, y: f32) {
        w.heroes.iter_mut().find(|h| h.player == player).unwrap().pos = V2::new(x, y);
    }

    pub(super) fn secs(w: &mut World, s: f32) {
        for _ in 0..(s * TICK_HZ as f32).round() as usize {
            w.step();
        }
    }

    pub(super) fn state(w: &World) -> &Extraction {
        match &w.mode {
            Mode::Extraction(x) => x,
            Mode::Arena => panic!("not an extraction run"),
        }
    }

    pub(super) fn state_mut(w: &mut World) -> &mut Extraction {
        match &mut w.mode {
            Mode::Extraction(x) => x,
            Mode::Arena => panic!("not an extraction run"),
        }
    }

    #[test]
    fn an_extraction_zone_plays_extraction_and_starts_with_its_exits_shut() {
        let w = run("", 1);
        let x = state(&w);
        assert_eq!(x.exits.len(), 2);
        assert!(x.exits.iter().all(|e| !e.open));
        assert_eq!(w.bounds(), w.content().extractions[0].bounds());
    }

    #[test]
    fn an_arena_zone_plays_the_arena() {
        let w = World::new(Arc::new(crate::content::tests::sample()), "elwynn", 1).unwrap();
        assert_eq!(w.mode, Mode::Arena);
        assert_eq!(
            w.bounds(),
            Bounds::Circle {
                center: w.center(),
                radius: 150.0
            }
        );
    }

    #[test]
    fn a_pose_past_the_outline_is_held_on_it_and_one_inside_is_taken() {
        let mut w = run("", 1);
        hush(&mut w);
        let pose = |x: f32| PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, 0.0),
                facing: V2::new(1.0, 0.0),
            }),
            ..Default::default()
        };
        w.set_input(0, pose(500.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(300.0, 0.0));
        w.set_input(0, pose(290.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(290.0, 0.0));
    }

    #[test]
    fn nothing_is_placed_in_the_safe_ring_or_past_the_outline() {
        let mut w = run("", 1);
        let near = w.place(InteractableKind::Chest, V2::new(10.0, 0.0));
        let at = w.interactable(near).unwrap().pos;
        assert!((at.distance(V2::ZERO) - 90.0).abs() < 1e-3, "{at:?}");
        let far = w.place(InteractableKind::Chest, V2::new(0.0, 900.0));
        assert_eq!(w.interactable(far).unwrap().pos, V2::new(0.0, 300.0));
    }

    #[test]
    fn a_mirror_gets_the_extraction_state() {
        let mut host = run("", 1);
        state_mut(&mut host).heat = 123.0;
        host.step();
        let mut mirror = run("", 0);
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        assert_eq!(mirror.mode, host.mode);
        assert!(!mirror.hero(0).unwrap().extracted);
    }
}
```

In `crates/together-sim/src/world.rs`, change `mod arena;` to:

```rust
mod arena;
mod extraction;
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p together-sim world::extraction`
Expected: compile errors: no field `mode` on `World`, no method `bounds`, no field `extracted` on `Hero`.

- [ ] **Step 3: Add the types and fields to `world.rs`**

1. Imports: after `use crate::content::{AbilityKind, Content};` add `use crate::extraction::{Bounds, Extraction, Mode};`
2. In `enum Event`, after `PickupTaken { … },` add:

```rust
    /// Extraction: the party's heat (`player: None`) or one hero's reached `skulls`.
    HeatChanged {
        player: Option<PlayerId>,
        skulls: u8,
    },
    /// Extraction: a hunter pack was sent to cut `near` off from home.
    HuntersCame {
        near: PlayerId,
    },
    /// Extraction: exit `exit` (an index into the map's exits) opened.
    ExitOpened {
        exit: u8,
    },
    /// Extraction: `by` called a flight to exit `exit`.
    FlightCalled {
        exit: u8,
        by: PlayerId,
    },
    /// Extraction: the flight to exit `exit` landed.
    FlightLanded {
        exit: u8,
    },
    /// Extraction: `player` left on the flight from exit `exit`.
    HeroExtracted {
        player: PlayerId,
        exit: u8,
    },
```

3. In `struct Hero`, after `pub alive: bool,` add:

```rust
    /// Left on a flight (extraction): out of the world for good, so not alive, and never raised.
    pub extracted: bool,
```

   In `add_hero_as`'s `Hero { … }`, add `extracted: false,` after `alive: true,`. Then find every other literal `Hero {` in the crate (`rg -n "damage_by: Vec::new\(\)" crates/together-sim`) and add `extracted: false,` to each.
4. In `struct Snapshot`, after `pickups`, add `pub mode: Mode,`.
5. In `struct World`, after `next_boss`, add:

```rust
    /// The rules this run plays by, chosen by its zone.
    pub mode: Mode,
```

6. In `World::new`, just before `let mut world = Self {`, add:

```rust
        let mode = match content.zones[zone].extraction_ix {
            Some(map) => Mode::Extraction(Extraction::new(map, &content.extractions[map])),
            None => Mode::Arena,
        };
```

   and add `mode,` as the last field of `Self { … }`.
7. In `place`, make the first line of the body `let pos = self.placeable(pos);`.
8. Replace `move_interactable`'s body with:

```rust
        let pos = self.bounds().clamp(pos);
        if let Some(it) = self.interactables.iter_mut().find(|i| i.id == id) {
            it.pos = pos;
        }
```

9. In `snapshot()`, add `mode: self.mode.clone(),` after `pickups`. In `apply_snapshot`, add `self.mode = snapshot.mode;` after `self.pickups = snapshot.pickups;`.
10. Replace `move_heroes` with this version. The arena's circle clamp is the same arithmetic as before:

```rust
    fn move_heroes(&mut self) {
        let bounds = self.bounds();
        for hero in self.heroes.iter_mut().filter(|h| h.alive) {
            let input = self.inputs.get_mut(&hero.player);
            if let Some(pose) = input.as_ref().and_then(|i| i.pose) {
                let pos = bounds.clamp(pose.pos);
                hero.moving = pos.distance(hero.pos) > 1e-3;
                hero.pos = pos;
                if pose.facing != V2::ZERO {
                    hero.facing = pose.facing.normalize_or_zero();
                }
                continue;
            }
            let jump = input.as_ref().is_some_and(|i| i.jump);
            let dir = input.map_or(V2::ZERO, |i| {
                i.jump = false;
                i.move_dir
            });
            if jump && hero.air == 0.0 && hero.vy == 0.0 {
                hero.vy = JUMP_SPEED;
            }
            if hero.vy != 0.0 || hero.air > 0.0 {
                hero.vy -= GRAVITY * DT;
                hero.air += hero.vy * DT;
                if hero.air <= 0.0 {
                    hero.air = 0.0;
                    hero.vy = 0.0;
                }
            }
            let dir = dir.normalize_or_zero();
            hero.moving = dir != V2::ZERO;
            if !hero.moving {
                continue;
            }
            hero.facing = dir;
            hero.pos = bounds.clamp(hero.pos + dir * (hero.speed * DT));
        }
    }
```

- [ ] **Step 4: Add the seam's helpers**

In `crates/together-sim/src/world/extraction.rs`, above `#[cfg(test)]`, add:

```rust
impl World {
    /// Where heroes may go: the arena's leash, or the extraction map's outline.
    pub fn bounds(&self) -> Bounds {
        match self.map(&self.content) {
            Some(def) => def.bounds(),
            None => Bounds::Circle {
                center: self.center(),
                radius: self.zone().leash,
            },
        }
    }

    /// The extraction map this run plays, read from `content` (the world's own, or a clone of it
    /// held while the world changes); `None` in the arena.
    fn map<'c>(&self, content: &'c Content) -> Option<&'c ExtractionDef> {
        match &self.mode {
            Mode::Extraction(x) => content.extractions.get(x.def),
            Mode::Arena => None,
        }
    }

    fn run_mut(&mut self) -> Option<&mut Extraction> {
        match &mut self.mode {
            Mode::Extraction(x) => Some(x),
            Mode::Arena => None,
        }
    }

    /// Where an interactable put at `pos` stands: there, in the arena; in an extraction run,
    /// inside the outline and out past the safe ring's margin.
    pub(super) fn placeable(&self, pos: V2) -> V2 {
        let Some(def) = self.map(&self.content) else {
            return pos;
        };
        let bounds = def.bounds();
        let at = bounds.clamp(pos);
        if !def.in_margin(at) {
            return at;
        }
        let home = def.safe_center();
        let out = (at - home).normalize_or_zero();
        let out = if out == V2::ZERO { V2::new(1.0, 0.0) } else { out };
        bounds.clamp(home + out * (def.safe.radius + def.safe.margin))
    }
}
```

- [ ] **Step 5: Bump the protocol and keep the client compiling**

In `crates/together-net/src/lib.rs`: `pub const PROTOCOL: u32 = 4;` (snapshots and heroes changed shape).

In `crates/benilla-app/src/together/view.rs`, in `apply_events`, add this arm after the `Event::Interacted { … } => { … }` arm. Task 9 replaces it:

```rust
            // Extraction: drawn from the world's state; Task 9 adds their text.
            Event::HeatChanged { .. }
            | Event::HuntersCame { .. }
            | Event::ExitOpened { .. }
            | Event::FlightCalled { .. }
            | Event::FlightLanded { .. }
            | Event::HeroExtracted { .. } => {}
```

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p together-sim -p together-net`
Expected: all pass, including the five new `world::extraction::tests`.

- [ ] **Step 7: Commit**

```bash
$G add crates/together-sim crates/together-net/src/lib.rs crates/benilla-app/src/together/view.rs
$G commit -m "Sim: a run's mode, chosen by its zone; extraction bounds and snapshots

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Heat

Heat climbs outside the safe ring and with kills. Shared heat cools only while every living hero is home; per-hero heat cools for whoever is home. The nightfall floor holds heat up, skull changes are announced once, and `lose_them` drops a skull after a quiet spell. Kills pay out by heat, and so does luck.

**Files:**
- Modify: `crates/together-sim/src/world/extraction.rs`
- Modify: `crates/together-sim/src/world.rs` (`step`, `reap_enemies`, `deal`)

**Interfaces:**
- Consumes (Task 3): `World::map`, `World::run_mut`, `Mode`, `Event::HeatChanged`.
- Produces:
  - `World::heat_of(&self, PlayerId) -> f32` (pub): the party's heat when shared, else the hero's own; 0 in the arena.
  - `World::skulls_of(&self, PlayerId) -> usize` (pub).
  - `World::extraction_settings(&self) -> Option<ExtractionSettings>` (pub).
  - `World::reward_of(&self, Option<PlayerId>) -> f32` and `World::reward_luck(&self, PlayerId) -> f32` (private to `world`).
  - `World::add_heat(&mut self, Option<PlayerId>, f32)`.
  - `World::tend_extraction(&mut self)` (`pub(super)`), called by `step` after `tend_arena`. Tasks 5 and 6 add calls to it.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `world/extraction.rs`:

```rust
    fn heat_told(w: &mut World) -> Vec<Event> {
        w.drain_events()
            .into_iter()
            .filter(|e| matches!(e, Event::HeatChanged { .. }))
            .collect()
    }

    #[test]
    fn heat_rises_outside_and_cools_only_while_everyone_is_home() {
        let mut w = run("", 2);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 2.0);
        assert!((w.heat_of(0) - 2.0).abs() < 0.1, "{}", w.heat_of(0));
        assert_eq!(w.heat_of(0), w.heat_of(1), "one heat for the party");
        at(&mut w, 0, 10.0, 0.0);
        state_mut(&mut w).heat = 50.0;
        secs(&mut w, 1.0);
        assert!((w.heat_of(0) - 35.0).abs() < 0.1, "{}", w.heat_of(0));
        at(&mut w, 1, 100.0, 0.0);
        secs(&mut w, 1.0);
        assert!((w.heat_of(0) - 36.0).abs() < 0.1, "one out: no cooling");
    }

    #[test]
    fn each_hero_has_their_own_heat_when_it_is_not_shared() {
        let mut w = run("heat_shared: false", 2);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 2.0);
        assert!((w.heat_of(0) - 2.0).abs() < 0.1, "{}", w.heat_of(0));
        assert_eq!(w.heat_of(1), 0.0);
    }

    #[test]
    fn the_night_floor_rises_and_heat_never_cools_below_it() {
        let mut w = run("", 1);
        hush(&mut w);
        w.elapsed = 300.0;
        secs(&mut w, 1.0);
        let floor = state(&w).floor;
        assert!((floor - 80.67).abs() < 0.1, "{floor}");
        assert!((w.heat_of(0) - floor).abs() < 1e-3);
    }

    #[test]
    fn reaching_a_skull_is_told_once() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        state_mut(&mut w).heat = 99.5;
        secs(&mut w, 1.0);
        assert_eq!(
            heat_told(&mut w),
            vec![Event::HeatChanged {
                player: None,
                skulls: 1
            }]
        );
        secs(&mut w, 1.0);
        assert!(heat_told(&mut w).is_empty());
    }

    #[test]
    fn with_lose_them_a_quiet_spell_drops_a_skull() {
        let mut w = run("lose_them: true", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        state_mut(&mut w).heat = 300.0;
        secs(&mut w, 10.5);
        assert_eq!(w.skulls_of(0), 1, "{}", w.heat_of(0));
    }

    #[test]
    fn without_lose_them_heat_holds_through_a_quiet_spell() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        state_mut(&mut w).heat = 300.0;
        secs(&mut w, 10.5);
        assert_eq!(w.skulls_of(0), 2);
    }

    #[test]
    fn hotter_kills_are_worth_more_and_heat_the_party() {
        for (heat, xp) in [(0.0, 1), (1000.0, 5)] {
            let mut w = run("", 1);
            hush(&mut w);
            state_mut(&mut w).heat = heat;
            let id = w.spawn_enemy(0, V2::new(5.0, 0.0), V2::ZERO, false);
            w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
            w.step();
            let dropped = w.drain_events().into_iter().find_map(|e| match e {
                Event::GemDropped { xp, .. } => Some(xp),
                _ => None,
            });
            assert_eq!(dropped, Some(xp), "at {heat} heat");
        }
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        let id = w.spawn_enemy(0, V2::new(105.0, 0.0), V2::ZERO, false);
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        assert!((w.heat_of(0) - (0.5 + 1.0 / 30.0)).abs() < 1e-3, "{}", w.heat_of(0));
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p together-sim world::extraction`
Expected: compile errors: no method `heat_of`, `skulls_of`.

- [ ] **Step 3: Implement heat**

In `world/extraction.rs`, change the import line to:

```rust
use crate::extraction::{ExtractionDef, ExtractionSettings};
```

Add these methods inside `impl World`:

```rust
    /// The rules this extraction run tries, or `None` in the arena.
    pub fn extraction_settings(&self) -> Option<ExtractionSettings> {
        self.map(&self.content).map(|d| d.settings)
    }

    /// The heat `player` plays under: the party's when shared, else their own; 0 in the arena.
    pub fn heat_of(&self, player: PlayerId) -> f32 {
        let (Mode::Extraction(x), Some(def)) = (&self.mode, self.map(&self.content)) else {
            return 0.0;
        };
        if def.settings.heat_shared {
            return x.heat;
        }
        x.heat_by
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(x.floor, |&(_, h)| h)
    }

    /// The skull `player` is at: 0 cool, up to the map's hottest.
    pub fn skulls_of(&self, player: PlayerId) -> usize {
        self.map(&self.content)
            .map_or(0, |d| d.skull_index(self.heat_of(player)))
    }

    /// The party's heat: shared, or the hottest hero's.
    fn party_heat(&self) -> f32 {
        match &self.mode {
            Mode::Extraction(x) => x.heat_by.iter().map(|&(_, h)| h).fold(x.heat, f32::max),
            Mode::Arena => 0.0,
        }
    }

    /// XP and gold from a kill by `killer`, times: by the killer's heat, or the party's for a
    /// kill nobody made. 1 in the arena.
    pub(super) fn reward_of(&self, killer: Option<PlayerId>) -> f32 {
        let Some(def) = self.map(&self.content) else {
            return 1.0;
        };
        let heat = killer.map_or_else(|| self.party_heat(), |p| self.heat_of(p));
        def.skull(heat).reward
    }

    /// Luck heat lends `player`'s hands of cards. 0 in the arena.
    pub(super) fn reward_luck(&self, player: PlayerId) -> f32 {
        let Some(def) = self.map(&self.content) else {
            return 0.0;
        };
        (def.skull(self.heat_of(player)).reward - 1.0) * def.heat.reward_luck
    }

    /// `amount` heat: to the party when shared; else to `player`, or with `None` to every
    /// living hero.
    fn add_heat(&mut self, player: Option<PlayerId>, amount: f32) {
        let Some(settings) = self.extraction_settings() else {
            return;
        };
        let living: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| h.player)
            .collect();
        let Some(x) = self.run_mut() else {
            return;
        };
        if settings.heat_shared {
            x.heat += amount;
            return;
        }
        for p in living.into_iter().filter(|p| player.is_none_or(|q| q == *p)) {
            hero_heat(x, p).1 += amount;
        }
    }

    /// A kill's heat, for `killer` (or, shared, the party).
    pub(super) fn heat_from_kill(&mut self, killer: Option<PlayerId>) {
        let content = self.content.clone();
        if let Some(def) = self.map(&content) {
            self.add_heat(killer, def.heat.kill);
        }
    }

    /// Each step's extraction upkeep. Nothing in the arena.
    pub(super) fn tend_extraction(&mut self) {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        self.tend_heat(def);
        self.lose_them(def);
        self.tell_skulls(def);
    }

    /// The floor rises with the night; heat climbs while anyone (shared) or the hero (not
    /// shared) is outside the safe ring, and cools while everyone (shared) or the hero is home.
    fn tend_heat(&mut self, def: &ExtractionDef) {
        let floor = def.floor(self.elapsed);
        let living: Vec<(PlayerId, bool)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, def.in_safe(h.pos)))
            .collect();
        let Some(x) = self.run_mut() else {
            return;
        };
        x.floor = floor;
        let (outside, cool) = (def.heat.outside * DT, def.safe.cool * DT);
        if def.settings.heat_shared {
            if living.iter().any(|&(_, home)| !home) {
                x.heat += outside;
            } else if !living.is_empty() {
                x.heat -= cool;
            }
            x.heat = x.heat.max(floor);
        } else {
            for &(p, home) in &living {
                let h = &mut hero_heat(x, p).1;
                let next = if home { *h - cool } else { *h + outside };
                *h = next.max(floor);
            }
            for (_, h) in &mut x.heat_by {
                *h = h.max(floor);
            }
        }
        x.peak = x.heat_by.iter().map(|&(_, h)| h).fold(x.heat.max(x.peak), f32::max);
    }

    /// `lose_them`: once no enemy has been near any living hero for the map's quiet spell, every
    /// heat drops to the start of the skull below it (never under the floor).
    fn lose_them(&mut self, def: &ExtractionDef) {
        if !def.settings.lose_them {
            return;
        }
        let living: Vec<V2> = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).collect();
        let range = def.heat.quiet_range;
        let near = self
            .enemies
            .iter()
            .any(|e| living.iter().any(|p| p.distance(e.pos) <= range));
        let Some(x) = self.run_mut() else {
            return;
        };
        x.quiet = if near { 0.0 } else { x.quiet + DT };
        if x.quiet < def.heat.quiet_secs {
            return;
        }
        x.quiet = 0.0;
        let floor = x.floor;
        let drop = |h: f32| match def.skull_index(h) {
            0 => h,
            s => def.heat.skulls[s - 1].at.max(floor),
        };
        x.heat = drop(x.heat);
        for (_, h) in &mut x.heat_by {
            *h = drop(*h);
        }
    }

    /// A [`Event::HeatChanged`] for each heat whose skull changed since last told.
    fn tell_skulls(&mut self, def: &ExtractionDef) {
        let now: Vec<(Option<PlayerId>, u8)> = match &self.mode {
            Mode::Extraction(x) if def.settings.heat_shared => {
                vec![(None, def.skull_index(x.heat) as u8)]
            }
            Mode::Extraction(x) => x
                .heat_by
                .iter()
                .map(|&(p, h)| (Some(p), def.skull_index(h) as u8))
                .collect(),
            Mode::Arena => return,
        };
        let Some(x) = self.run_mut() else {
            return;
        };
        let changed: Vec<Event> = now
            .iter()
            .filter(|&&(who, skulls)| {
                let before = x.told.iter().find(|(w, _)| *w == who).map_or(0, |&(_, s)| s);
                skulls != before
            })
            .map(|&(player, skulls)| Event::HeatChanged { player, skulls })
            .collect();
        x.told = now;
        self.events.extend(changed);
    }
```

Below the `impl World` block, outside it, add:

```rust
/// `player`'s entry in a per-hero heat list, made at the floor if they have none yet.
fn hero_heat(x: &mut Extraction, player: PlayerId) -> &mut (PlayerId, f32) {
    if !x.heat_by.iter().any(|(p, _)| *p == player) {
        x.heat_by.push((player, x.floor));
    }
    x.heat_by
        .iter_mut()
        .find(|(p, _)| *p == player)
        .expect("just made")
}
```

- [ ] **Step 4: Hook heat into `world.rs`**

1. In `step()`, after `self.tend_arena();` add `self.tend_extraction();`.
2. In `reap_enemies`, replace the line `self.drop_gem(pos, worth * reward, gold);` with:

```rust
            let paid = self.reward_of(killer);
            let scale = |n: u32| (n as f32 * paid).round() as u32;
            self.drop_gem(pos, scale(worth * reward), scale(gold));
            self.heat_from_kill(killer);
```

   In the arena `paid` is 1, and `round(n as f32)` is `n` for every gem, so arena gems don't change.
3. In `deal`, replace `let luck = self.heroes[h].stats.get(Stat::Luck);` with:

```rust
        let luck = self.heroes[h].stats.get(Stat::Luck) + self.reward_luck(self.heroes[h].player);
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p together-sim`
Expected: all pass, including the seven new heat tests and every existing arena test.

- [ ] **Step 6: Commit**

```bash
$G add crates/together-sim
$G commit -m "Sim: heat for extraction runs, with its floor, skulls, lose_them and rewards

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Spawning by heat, hunters and recycling

In an extraction run the horde comes only around heroes outside the safe ring, never within its margin or past the outline. It's sized, ranked and toughened by skull. At three skulls and up, hunter packs come from the direction of home. Ordinary enemies far from everyone are let go. The timed boss stays off unless `timed_boss` is set.

**Files:**
- Modify: `crates/together-sim/src/world/extraction.rs`
- Modify: `crates/together-sim/src/world.rs` (`step`)

**Interfaces:**
- Consumes (Task 4): `heat_of`, `extraction_settings`, `tend_extraction`; (existing) `spawn_enemy(def_ix, pos, toward, elite) -> u32`, `tiers() -> (usize, usize)`, `target_alive(elapsed, heroes)`, `SPAWN_RING`, `SPAWN_EVERY`, `ELITES_FROM`, `ELITE_CHANCE`.
- Produces: `World::spawn_in_extraction(&mut self)` (`pub(super)`), called by `step` instead of `spawn_enemies` and `spawn_bosses` in extraction runs; `send_hunters` and `recycle`, called from `tend_extraction`; `toughen(id, by, speed)`.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `world/extraction.rs`:

```rust
    #[test]
    fn nothing_spawns_while_everyone_is_home_or_near_the_safe_ring() {
        let mut w = run("", 1);
        secs(&mut w, 10.0);
        assert!(w.enemies.is_empty(), "home: no horde");
        at(&mut w, 0, 60.0, 0.0);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..300 {
            w.step();
            for e in &w.enemies {
                // Seen a step after spawning, so up to one step's walk nearer.
                if seen.insert(e.id) {
                    assert!(e.pos.distance(V2::ZERO) >= 89.5, "{:?}", e.pos);
                }
            }
        }
        assert!(!seen.is_empty(), "outside: a horde");
    }

    #[test]
    fn more_skulls_bring_a_bigger_tougher_horde() {
        let mut cold = run("", 1);
        let mut hot = run("", 1);
        at(&mut cold, 0, 200.0, 0.0);
        at(&mut hot, 0, 200.0, 0.0);
        state_mut(&mut hot).heat = 900.0;
        secs(&mut cold, 3.0);
        secs(&mut hot, 3.0);
        assert!(hot.enemies.len() > cold.enemies.len());
        let wolf = hot.content().enemies[0].hp;
        assert!(hot.enemies.iter().all(|e| e.max_hp >= wolf * 1.7));
        assert!(cold.enemies.iter().all(|e| e.max_hp < wolf * 1.1));
    }

    #[test]
    fn hunters_come_between_a_hot_hero_and_home() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 200.0, 0.0);
        state_mut(&mut w).heat = 450.0;
        w.step();
        let hero = V2::new(200.0, 0.0);
        let hunters: Vec<&Enemy> = w.enemies.iter().filter(|e| e.speed > 5.5).collect();
        assert!((6..=10).contains(&hunters.len()), "{}", hunters.len());
        let cone = 30f32.to_radians().cos();
        for e in hunters {
            let to = e.pos - hero;
            let d = to.length();
            assert!((50.0..=70.0).contains(&d), "{d}");
            assert!(-to.x / d >= cone - 1e-3, "{:?} is not on the way home", e.pos);
        }
        assert!(w
            .drain_events()
            .contains(&Event::HuntersCame { near: 0 }));
    }

    #[test]
    fn far_enemies_are_let_go_but_never_elites() {
        let mut w = run("", 1);
        hush(&mut w);
        let near = w.spawn_enemy(0, V2::new(50.0, 0.0), V2::ZERO, false);
        let far = w.spawn_enemy(0, V2::new(250.0, 0.0), V2::ZERO, false);
        let elite = w.spawn_enemy(0, V2::new(-250.0, 0.0), V2::ZERO, true);
        w.step();
        assert!(w.enemy(near).is_some());
        assert!(w.enemy(far).is_none());
        assert!(w.enemy(elite).is_some());
    }

    #[test]
    fn the_timed_boss_comes_only_when_asked_for() {
        for (settings, comes) in [("", false), ("timed_boss: true", true)] {
            let mut w = run(settings, 1);
            hush(&mut w);
            w.next_boss = 0.0;
            w.step();
            assert_eq!(w.enemies.iter().any(|e| e.boss), comes, "{settings}");
        }
    }
```

Note on the hunter test: the safe ring is due west of the hero (`-x`), so a hunter on the way home has a direction whose `-x` share is at least `cos(30°)`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p together-sim world::extraction`
Expected: `nothing_spawns…`, `more_skulls…`, `hunters_come…`, `far_enemies…` and `the_timed_boss…` fail. Spawning still follows the arena, and there are no hunters or recycling yet.

- [ ] **Step 3: Implement**

In `world/extraction.rs`, change the import line to:

```rust
use crate::extraction::{ExtractionDef, ExtractionSettings, SkullDef};
```

Add to `impl World`:

```rust
    /// The horde, in an extraction run: around heroes outside the safe ring only, never within
    /// its margin or past the outline; as big, as high in the roster and as tough as the heat of
    /// the hero it comes for says.
    pub(super) fn spawn_in_extraction(&mut self) {
        self.spawn_timer -= DT;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = SPAWN_EVERY;
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        let outside: Vec<(PlayerId, V2, SkullDef)> = self
            .heroes
            .iter()
            .filter(|h| h.alive && !def.in_safe(h.pos))
            .map(|h| (h.player, h.pos, *def.skull(self.heat_of(h.player))))
            .collect();
        if outside.is_empty() {
            return;
        }
        let busiest = outside.iter().map(|o| o.2.spawns).fold(1.0, f32::max);
        let horde = 1.0 + self.difficulty.horde;
        let target = (target_alive(self.elapsed, outside.len()) as f32 * busiest * horde) as usize;
        let alive = self.enemies.iter().filter(|e| !e.elite && !e.boss).count();
        if alive >= target {
            return;
        }
        let batch = ((target - alive) as f32 / 6.0).ceil().min(4.0) as usize;
        let (low, top) = self.tiers();
        let roster = self.zone().enemy_ix.clone();
        let bounds = def.bounds();
        for _ in 0..batch {
            let (_, near, skull) = outside[self.rng.index(outside.len())];
            let Some(pos) = self.spawn_spot(def, &bounds, near) else {
                continue;
            };
            let tier = (low + self.rng.index(top - low + 1) + skull.tier).min(roster.len() - 1);
            let elite = self.elapsed >= ELITES_FROM && self.rng.f32() < ELITE_CHANCE;
            let id = self.spawn_enemy(roster[tier], pos, near, elite);
            self.toughen(id, skull.toughness, 1.0);
        }
    }

    /// A spot on the spawn ring round `near`, inside the outline and clear of the safe ring's
    /// margin; `None` after a few tries.
    fn spawn_spot(&mut self, def: &ExtractionDef, bounds: &Bounds, near: V2) -> Option<V2> {
        for _ in 0..8 {
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let at = near + V2::from_angle(angle, self.rng.range(SPAWN_RING.0, SPAWN_RING.1));
            if bounds.inside(at, 0.0) && !def.in_margin(at) {
                return Some(at);
            }
        }
        None
    }

    /// Enemy `id`'s health and damage times `by`, and its speed times `speed`.
    fn toughen(&mut self, id: u32, by: f32, speed: f32) {
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
            e.hp *= by;
            e.max_hp *= by;
            e.damage *= by;
            e.speed *= speed;
        }
    }

    /// When the hunt timer has run out, a hunter pack for a hero outside whose skull sends them:
    /// the strongest enemies in play, faster than the rest, coming from the way home.
    fn send_hunters(&mut self, def: &ExtractionDef) {
        if let Some(x) = self.run_mut() {
            x.hunt -= DT;
            if x.hunt > 0.0 {
                return;
            }
        }
        let hunted: Vec<(PlayerId, V2, SkullDef)> = self
            .heroes
            .iter()
            .filter(|h| h.alive && !def.in_safe(h.pos))
            .map(|h| (h.player, h.pos, *def.skull(self.heat_of(h.player))))
            .filter(|(_, _, s)| s.hunters_every > 0.0)
            .collect();
        if hunted.is_empty() {
            return;
        }
        let (player, from, skull) = hunted[self.rng.index(hunted.len())];
        let roster = self.zone().enemy_ix.clone();
        let (_, top) = self.tiers();
        let kind = roster[(top + skull.tier).min(roster.len() - 1)];
        let pack = def.heat.hunters;
        let count = pack.pack.0 + self.rng.index((pack.pack.1 - pack.pack.0 + 1) as usize) as u32;
        let home = (def.safe_center() - from).normalize_or_zero();
        let half = pack.cone.to_radians() / 2.0;
        let bounds = def.bounds();
        for _ in 0..count {
            let (sin, cos) = self.rng.range(-half, half).sin_cos();
            let way = V2::new(home.x * cos - home.y * sin, home.x * sin + home.y * cos);
            let at = from + way * self.rng.range(pack.ring.0, pack.ring.1);
            if !bounds.inside(at, 0.0) || def.in_margin(at) {
                continue;
            }
            let id = self.spawn_enemy(kind, at, from, false);
            self.toughen(id, skull.toughness, pack.speed);
        }
        if let Some(x) = self.run_mut() {
            x.hunt = skull.hunters_every;
        }
        self.events.push(Event::HuntersCame { near: player });
    }

    /// Ordinary enemies further than the map's reach from every living hero are let go: a party
    /// spread over the forest drags no tail of stragglers. Elites and bosses stay.
    fn recycle(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).collect();
        if living.is_empty() {
            return;
        }
        let reach = def.recycle;
        self.enemies
            .retain(|e| e.elite || e.boss || living.iter().any(|p| p.distance(e.pos) <= reach));
    }
```

In `tend_extraction`, after `self.tell_skulls(def);` add:

```rust
        self.send_hunters(def);
        self.recycle(def);
```

In `world.rs` `step()`, replace the two lines `self.spawn_enemies();` and `self.spawn_bosses();` with:

```rust
        if matches!(self.mode, Mode::Arena) {
            self.spawn_enemies();
            self.spawn_bosses();
        } else {
            self.spawn_in_extraction();
            if self.extraction_settings().is_some_and(|s| s.timed_boss) {
                self.spawn_bosses();
            }
        }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p together-sim`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
$G add crates/together-sim
$G commit -m "Sim: the extraction horde by heat, hunters from the way home, far enemies let go

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Exits, flights and extraction

Exits open on their timers. Standing in an open exit for its `call` seconds calls a flight, which heats the party while it comes. A landed flight takes every living hero in its ring (`solo_extract`), or waits until every living hero is in it. Extracted heroes are out for good: not alive, never raised, dealt no cards. The run ends when nobody is left alive.

**Files:**
- Modify: `crates/together-sim/src/world/extraction.rs`
- Modify: `crates/together-sim/src/world.rs` (`revive_heroes`, `level_up`)

**Interfaces:**
- Consumes (Tasks 3–5): `add_heat`, `tend_extraction`, `Event::{ExitOpened, FlightCalled, FlightLanded, HeroExtracted}`, `Hero::extracted`.
- Produces: `World::tend_exits(&mut self, &ExtractionDef)`, called from `tend_extraction`. `World::game_over()` is unchanged and now also means "everyone extracted or down".

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `world/extraction.rs`:

```rust
    #[test]
    fn exits_open_on_their_own_time() {
        let mut w = run("", 1);
        hush(&mut w);
        w.elapsed = 59.0;
        secs(&mut w, 2.0);
        let x = state(&w);
        assert!(x.exits[0].open && !x.exits[1].open);
        let opened: Vec<Event> = w
            .drain_events()
            .into_iter()
            .filter(|e| matches!(e, Event::ExitOpened { .. }))
            .collect();
        assert_eq!(opened, vec![Event::ExitOpened { exit: 0 }]);
    }

    #[test]
    fn a_shut_exit_calls_nothing() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 0.0, -250.0);
        w.elapsed = 61.0;
        secs(&mut w, 5.0);
        let x = state(&w);
        assert!(!x.exits[1].open && x.exits[1].flight.is_none());
    }

    #[test]
    fn standing_in_an_open_exit_calls_a_flight_that_takes_whoever_is_in_it() {
        let mut w = run("", 2);
        hush(&mut w);
        w.elapsed = 61.0;
        at(&mut w, 0, 200.0, 0.0);
        secs(&mut w, 3.2);
        assert!(state(&w).exits[0].flight.is_some(), "called");
        let before = w.heat_of(0);
        secs(&mut w, 45.5);
        let gained = w.heat_of(0) - before;
        assert!(gained > 45.0 * 4.0, "a flight on its way heats the party: {gained}");
        let (out, home) = (w.hero(0).unwrap(), w.hero(1).unwrap());
        assert!(out.extracted && !out.alive);
        assert!(home.alive && !home.extracted);
        assert!(!w.game_over());
        assert!(w
            .drain_events()
            .contains(&Event::HeroExtracted { player: 0, exit: 0 }));
    }

    #[test]
    fn without_solo_extract_the_flight_waits_for_everyone() {
        let mut w = run("solo_extract: false", 2);
        hush(&mut w);
        w.elapsed = 61.0;
        at(&mut w, 0, 200.0, 0.0);
        secs(&mut w, 48.5);
        assert!(state(&w).exits[0].landed);
        assert!(w.hero(0).unwrap().alive, "still waiting");
        at(&mut w, 1, 201.0, 2.0);
        w.step();
        assert!(w.heroes.iter().all(|h| h.extracted));
        assert!(w.game_over());
    }

    #[test]
    fn a_waiting_flight_leaves_when_the_rest_are_aboard_or_down() {
        let mut w = run("solo_extract: false", 2);
        hush(&mut w);
        w.elapsed = 61.0;
        at(&mut w, 0, 200.0, 0.0);
        secs(&mut w, 48.5);
        assert!(state(&w).exits[0].landed);
        let other = w.heroes.iter_mut().find(|h| h.player == 1).unwrap();
        other.alive = false;
        other.hp = 0.0;
        w.step();
        assert!(w.hero(0).unwrap().extracted);
        assert!(!w.hero(1).unwrap().extracted);
        assert!(w.game_over());
    }

    #[test]
    fn the_last_one_out_ends_the_run_and_leaves_the_fallen() {
        let mut w = run("", 2);
        hush(&mut w);
        w.elapsed = 61.0;
        at(&mut w, 0, 200.0, 0.0);
        let fallen = w.heroes.iter_mut().find(|h| h.player == 1).unwrap();
        fallen.alive = false;
        fallen.hp = 0.0;
        secs(&mut w, 48.5);
        assert!(w.hero(0).unwrap().extracted);
        assert!(!w.hero(1).unwrap().extracted, "the fallen are left behind");
        assert!(w.game_over());
    }

    #[test]
    fn an_extracted_hero_is_never_raised() {
        let mut w = run("", 2);
        hush(&mut w);
        let gone = w.heroes.iter_mut().find(|h| h.player == 0).unwrap();
        gone.alive = false;
        gone.extracted = true;
        secs(&mut w, 5.0);
        assert!(!w.hero(0).unwrap().alive);
    }

    #[test]
    fn an_extracted_hero_gets_no_cards_and_never_holds_the_run() {
        let mut w = run("", 2);
        hush(&mut w);
        let gone = w.heroes.iter_mut().find(|h| h.player == 0).unwrap();
        gone.alive = false;
        gone.extracted = true;
        w.xp = xp_to_next(w.level);
        w.step();
        assert!(w.hero(0).unwrap().offers.is_empty());
        assert!(!w.hero(1).unwrap().offers.is_empty());
        w.set_input(
            1,
            PlayerInput {
                pick: Some(0),
                ..Default::default()
            },
        );
        w.step();
        assert!(!w.paused());
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p together-sim world::extraction`
Expected: the exit tests fail (exits never open). `an_extracted_hero_is_never_raised` fails because the friend beside them raises them. `an_extracted_hero_gets_no_cards…` fails because extracted heroes still get hands.

- [ ] **Step 3: Implement**

Add to `impl World` in `world/extraction.rs`:

```rust
    /// Exits open on their timers. A hero standing in an open one calls a flight; while it comes
    /// it heats the party (shared) or whoever waits in its ring. A landed flight takes every
    /// living hero in its ring, or with `solo_extract` off waits until every living hero is in it.
    fn tend_exits(&mut self, def: &ExtractionDef) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        let elapsed = self.elapsed;
        let mut events = Vec::new();
        let mut heat: Vec<(Option<PlayerId>, f32)> = Vec::new();
        let mut taken: Vec<(PlayerId, u8)> = Vec::new();
        let Some(x) = self.run_mut() else {
            return;
        };
        for (i, (exit, state)) in def.exits.iter().zip(x.exits.iter_mut()).enumerate() {
            let tag = i as u8;
            let at = V2::new(exit.pos[0], exit.pos[1]);
            let inside: Vec<PlayerId> = living
                .iter()
                .filter(|(_, p)| p.distance(at) <= exit.radius)
                .map(|&(p, _)| p)
                .collect();
            if !state.open {
                if elapsed < exit.opens_at {
                    continue;
                }
                state.open = true;
                events.push(Event::ExitOpened { exit: tag });
            }
            state.cooldown = (state.cooldown - DT).max(0.0);
            if let Some(left) = state.flight {
                let gain = def.heat.flight * DT;
                if def.settings.heat_shared {
                    heat.push((None, gain));
                } else {
                    heat.extend(inside.iter().map(|&p| (Some(p), gain)));
                }
                let left = left - DT;
                if left <= 0.0 {
                    state.flight = None;
                    state.landed = true;
                    events.push(Event::FlightLanded { exit: tag });
                } else {
                    state.flight = Some(left);
                }
            } else if !state.landed && state.cooldown <= 0.0 {
                let step = DT / exit.call;
                state.calling = if inside.is_empty() {
                    (state.calling - step).max(0.0)
                } else {
                    state.calling + step
                };
                if state.calling >= 1.0 {
                    state.calling = 0.0;
                    state.flight = Some(exit.flight);
                    events.push(Event::FlightCalled {
                        exit: tag,
                        by: inside[0],
                    });
                }
            }
            if state.landed {
                let everyone = !living.is_empty() && inside.len() == living.len();
                if def.settings.solo_extract || everyone {
                    state.landed = false;
                    state.cooldown = exit.recall;
                    taken.extend(inside.iter().map(|&p| (p, tag)));
                }
            }
        }
        for (who, amount) in heat {
            self.add_heat(who, amount);
        }
        for (player, exit) in taken {
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.alive = false;
                hero.extracted = true;
                hero.moving = false;
            }
            events.push(Event::HeroExtracted { player, exit });
        }
        self.events.extend(events);
    }
```

In `tend_extraction`, after `self.recycle(def);` add `self.tend_exits(def);`.

In `world.rs`:

1. In `revive_heroes`, make the loop over `&mut self.heroes` start with:

```rust
            // Gone on a flight: nothing to raise.
            if hero.extracted {
                continue;
            }
```

2. In `level_up`, change the loop body so extracted heroes get no hands:

```rust
            for h in 0..self.heroes.len() {
                if !self.heroes[h].extracted {
                    self.offer(h, Source::LevelUp);
                }
            }
```

   and update the comment above it to: `// The fallen too (their hands wait until they are raised); not those who left.`

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p together-sim -p together-net`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
$G add crates/together-sim
$G commit -m "Sim: exits on timers, flights called by standing in them, extraction

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Elwynn's extraction map in the shipped content

**Files:**
- Modify: `content/zones.ron` (a new zone after `elwynn`)
- Create: `content/extraction.ron`
- Modify: `crates/together-sim/tests/shipped_content.rs`

**Interfaces:**
- Consumes: `Content::with_extractions`, `ZoneDef::extraction_ix`, `World::{heat_of, mode}`.
- Produces: zone key `elwynn_extraction` and map key `elwynn`. Later client tasks use `TOGETHER_ZONE=elwynn_extraction`.

Coordinates come from the game's own tables, read with `mpqx`:
- `AreaPOI.dbc`: Goldshire (-9480.1, 63.5, 56.2); Stormwind (-9153.8, 364.1).
- `WorldMapArea.dbc`: Elwynn spans x -10254 to -7940, y -1935 to 1535.
- Eastvale Logging Camp is read off the world map at about 81, 66, which gives (-9467, -1276). Task 11 checks it in game.
- The gryphon model isn't in the trimmed data; `Creature\FelBat\BatTaxi.m2` (display 1566) is.

- [ ] **Step 1: Write the failing test**

In `crates/together-sim/tests/shipped_content.rs`:

1. In `shipped()`, after `.and_then(|c| c.with_rules(include_str!("../../../content/rules.ron")))` add:

```rust
    .and_then(|c| c.with_extractions(include_str!("../../../content/extraction.ron")))
```

2. In `every_class_survives_a_minute_in_every_zone_and_kills_something`, change `for zone in &c.zones {` to:

```rust
    // Extraction zones start their heroes in a safe ring where nothing spawns; they have their
    // own run below.
    for zone in c.zones.iter().filter(|z| z.extraction_ix.is_none()) {
```

3. Add `use together_sim::Mode;` to the imports, and append:

```rust
#[test]
fn an_extraction_run_in_elwynn_heats_up_out_in_the_forest_and_kills_something() {
    let c = Arc::new(shipped());
    let zone = c.zone("elwynn_extraction").expect("the zone");
    let map = c.zones[zone].extraction_ix.expect("its map");
    assert_eq!(c.extractions[map].key, "elwynn");
    for class in &c.classes {
        let mut w = World::new(c.clone(), "elwynn_extraction", 9).unwrap();
        w.add_hero(0, &class.key).unwrap();
        assert!(matches!(w.mode, Mode::Extraction(_)));
        // North out of Goldshire for 20 seconds, then circle out in the forest.
        for step in 0..(TICK_HZ * 80) {
            let a = step as f32 * 0.01;
            let move_dir = if step < TICK_HZ * 20 {
                V2::new(1.0, 0.0)
            } else {
                V2::new(a.cos(), a.sin())
            };
            w.set_input(
                0,
                PlayerInput {
                    move_dir,
                    pick: Some(0),
                    ..Default::default()
                },
            );
            w.step();
        }
        assert!(w.kills > 0, "{} killed nothing", class.key);
        assert!(w.heat_of(0) > 0.0, "{} stayed cool", class.key);
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p together-sim --test shipped_content`
Expected: it fails to compile, because `content/extraction.ron` doesn't exist.

- [ ] **Step 3: Add the zone**

In `content/zones.ron`, insert after the closing `),` of the `elwynn` entry:

```ron
    // Elwynn for extraction runs (extraction.ron "elwynn"): centred on Goldshire, the whole
    // forest open. `leash` only bounds where the first shrines are scattered.
    (
        key: "elwynn_extraction",
        name: "Elwynn Forest: Extraction",
        continent: "Eastern Kingdoms",
        description: "All of Elwynn is open. Goldshire is safe; out in the forest the heat climbs. Get out by flight from Stormwind Gate or Eastvale.",
        map: 0,
        center: (-9480.1, 63.5, 56.2),
        leash: 1500.0,
        enemies: [
            "young_wolf", "timber_wolf", "defias_thug", "mangy_wolf", "stonetusk_boar",
            "kobold_tunneler", "defias_cutpurse", "forest_spider", "murloc", "kobold_miner",
            "murloc_streamrunner", "rockhide_boar", "gray_forest_wolf", "defias_bandit",
            "young_forest_bear", "defias_rogue_wizard",
        ],
        boss: "hogger",
        shrines_at_once: 4,
        pots: 6,
        interactables: [
            (kind: TitanShrine, count: 2, weight: 10),
            (kind: GoldenShrine, count: 1, weight: 2),
            (kind: CursedAltar, count: 1, weight: 6),
            (kind: Chest, count: 2, weight: 10),
            (kind: ChallengeIdol, count: 1, weight: 5),
            (kind: WarBanner, count: 1, weight: 6),
        ],
    ),
```

- [ ] **Step 4: Add the map**

Create `content/extraction.ron`:

```ron
// Extraction maps: a whole zone open, a safe ring where nothing spawns, heat that climbs out in
// the forest, and exits that open on a timer. Positions are WoW (x, y): x north, y west.
// `zone` is the zones.ron entry the map is played in. Every number is a starting value to tune.
[
    (
        key: "elwynn",
        zone: "elwynn_extraction",
        // Elwynn between Stormwind's walls (north), Westfall (west), Redridge (east) and
        // Duskwood (south). A convex outline, corners in order.
        boundary: [(-8850.0, 820.0), (-8850.0, -1480.0), (-9990.0, -1480.0), (-9990.0, 820.0)],
        // Goldshire's crossroads (AreaPOI "Goldshire").
        safe: (center: (-9480.1, 63.5), radius: 40.0, margin: 50.0, cool: 15.0),
        recycle: 120.0,
        heat: (
            outside: 1.0,
            kill: 0.5,
            flight: 4.0,
            floor_from: 180.0,
            floor_per_min: 40.0,
            reward_luck: 0.25,
            quiet_range: 40.0,
            quiet_secs: 10.0,
            skulls: [
                (at: 0.0, spawns: 1.0, tier: 0, toughness: 1.0, hunters_every: 0.0, reward: 1.0),
                (at: 100.0, spawns: 1.3, tier: 1, toughness: 1.15, hunters_every: 0.0, reward: 1.5),
                (at: 250.0, spawns: 1.6, tier: 1, toughness: 1.3, hunters_every: 0.0, reward: 2.0),
                (at: 450.0, spawns: 2.0, tier: 2, toughness: 1.5, hunters_every: 20.0, reward: 2.75),
                (at: 700.0, spawns: 2.5, tier: 3, toughness: 1.75, hunters_every: 15.0, reward: 3.5),
                (at: 1000.0, spawns: 3.0, tier: 4, toughness: 2.0, hunters_every: 10.0, reward: 5.0),
            ],
            hunters: (pack: (6, 10), ring: (50.0, 70.0), cone: 60.0, speed: 1.2),
        ),
        exits: [
            // By Stormwind's gate (AreaPOI "Stormwind").
            (name: "Stormwind Gate", pos: (-9150.0, 360.0), radius: 15.0, opens_at: 480.0,
             call: 3.0, flight: 45.0, recall: 30.0),
            // Eastvale Logging Camp, read off the world map: check it in game.
            (name: "Eastvale Logging Camp", pos: (-9467.0, -1276.0), radius: 15.0,
             opens_at: 840.0, call: 3.0, flight: 45.0, recall: 30.0),
        ],
        // `BatTaxi.m2`, the Undercity flight bat: the gryphon (display 6852) is not in the
        // trimmed data. Swap it when the full client data is in.
        flight_display: 1566,
        settings: (heat_shared: true, lose_them: false, solo_extract: true, timed_boss: false),
    ),
]
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p together-sim`
Expected: all pass, including `an_extraction_run_in_elwynn…` and `every_zone_starts_with_its_few_shrines_inside_its_arena` for the new zone.

- [ ] **Step 6: Commit**

```bash
$G add content/zones.ron content/extraction.ron crates/together-sim/tests/shipped_content.rs
$G commit -m "Content: Elwynn's extraction map, with Goldshire safe and two exits

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Client wiring, bounds and a position trace

The client loads `extraction.ron` and puts it in the co-op content hash. The local hero and shrine settling use `World::bounds`, and `TOGETHER_TRACE_POS=1` logs where the hero stands, for checking coordinates.

**Files:**
- Modify: `crates/benilla-app/src/together/mod.rs` (`prepare`, `feed_pose`, module docs)
- Modify: `crates/benilla-app/src/together/view.rs` (`settle_shrines`)

**Interfaces:**
- Consumes: `Content::with_extractions`, `World::bounds`, `Bounds::{clamp, inside}`.
- Produces: content hash part 4 = zones + rules + extraction; the env var `TOGETHER_TRACE_POS`.

- [ ] **Step 1: Load and hash `extraction.ron`**

In `prepare()` in `mod.rs`, after the `let rules = read(…)?;` line, add:

```rust
    let extraction = read(
        "extraction.ron",
        include_str!("../../../../content/extraction.ron"),
    )?;
```

After `.and_then(|c| c.with_rules(&rules))` add `.and_then(|c| c.with_extractions(&extraction))`. Change the `CONTENT_TEXTS.set` line to:

```rust
    let _ = CONTENT_TEXTS.set((
        classes,
        abilities,
        enemies,
        format!("{zones}\n{rules}\n{extraction}"),
    ));
```

and its doc comment (on `CONTENT_TEXTS`) to: `/// The content files' text, for the content hash a co-op client and host compare: classes, abilities, enemies, and the zones with the rules and extraction maps after them.`

- [ ] **Step 2: Confirm the hash covers it**

Run: `grep -n "content_hash(&\[" crates/benilla-app/src/together/session.rs`
Expected: `content_hash(&[&c.0, &c.1, &c.2, &c.3])`. Part 3 now includes `extraction.ron`, so a client whose `extraction.ron` differs is refused with the existing content-mismatch message.

- [ ] **Step 3: Bounds in `feed_pose` and `settle_shrines`, and the trace**

In `feed_pose` (`mod.rs`):
1. Add a parameter `mut traced: Local<f32>,` after `mut bodies: Query<&mut Transform>,`.
2. Delete `let center = sim.world.center();` and replace the leash block

```rust
    let leash = sim.world.zone().leash;
    let off = pos - center;
    if off.length() > leash {
        pos = center + off.normalize_or_zero() * leash;
    }
```

   with:

```rust
    pos = sim.world.bounds().clamp(pos);
    if trace_pos() {
        *traced += time.delta_secs();
        if *traced >= 2.0 {
            *traced = 0.0;
            println!("together: at ({:.1}, {:.1})", pos.x, pos.y);
        }
    }
```

3. Update `feed_pose`'s doc comment: `The arena's edge` → `The run's edge (the arena's leash, or an extraction map's outline)`.
4. After `fn autoplay()`, add:

```rust
/// `TOGETHER_TRACE_POS=1`: the local hero's position every two seconds, for reading off
/// coordinates for the content files.
fn trace_pos() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TOGETHER_TRACE_POS").is_some())
}
```

5. In the module doc's "Configuration" paragraph, add `TOGETHER_TRACE_POS` (logs the hero's position) after `TOGETHER_CONTENT (…)`.

In `settle_shrines` (`view.rs`), delete `let center = sim.world.center();` and `let leash = sim.world.zone().leash;`, add `let bounds = sim.world.bounds();`, and replace `.filter(|p| p.distance(center) <= leash - 3.0)` with `.filter(|&p| bounds.inside(p, 3.0))`.

- [ ] **Step 4: Build**

```bash
cd /d/Games/azeroth-together-extraction
export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo build --release -p azeroth-together 2>&1 | tail -5
```

Run it in the background. The first build in this worktree takes about 17 minutes.
Expected: `Finished release profile`. Warnings are fine; errors are not.

- [ ] **Step 5: Commit**

```bash
$G add crates/benilla-app/src/together/mod.rs crates/benilla-app/src/together/view.rs
$G commit -m "Client: load extraction maps, keep heroes inside the run's edge, trace positions

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Drawing the extraction map

The safe ring gets a green rune and each exit a blue one. An open exit gets a pillar of light visible from anywhere. A flight comes down over its last eight seconds and waits on the ground once landed. Extracted heroes vanish with a teleport. The new events get floating text.

**Files:**
- Modify: `crates/benilla-app/src/together/view.rs`

**Interfaces:**
- Consumes: `World::mode`, `Content::extractions`, `ExtractionDef::{safe_center, exit_pos, safe, exits, flight_display}`, `ExitState`, `Hero::extracted`, the six extraction events; existing `units::{spawn_object, spawn_unit, unit_guid, UnitKind, UnitSpec, FACTION_FRIENDLY}`, `Fx::{text, cast}`, `BEAM`, `BEAM_SCALE`, `NameCache::insert_creature`, `CreatureRecord`.
- Produces: `Views::extraction: Option<ExtractionView>`; `sync_extraction`; event text.

- [ ] **Step 1: State and constants**

In `view.rs`, add a field to `Views` after `pickups`:

```rust
    /// An extraction run's rings, beams and flights; built on its first frame.
    extraction: Option<ExtractionView>,
```

After `impl ShrineView`, add:

```rust
/// The bodies that draw an extraction map: the safe ring, and per exit its ring, its beam while
/// open, and its flight while one comes down or waits.
pub(super) struct ExtractionView {
    safe: Entity,
    rings: Vec<Entity>,
    beams: Vec<Option<Entity>>,
    flights: Vec<Option<Entity>>,
}
```

After `COLOR_POWER_UP`, add:

```rust
/// GameObject serials for an extraction map's rings and beams: the safe ring, then two per exit.
const EXTRACTION_SERIAL_BASE: u64 = 0x3000_0000;
/// Creature serials for the flights to the exits, and the entry that names them.
const FLIGHT_SERIAL_BASE: u64 = 0x3800_0000;
const FLIGHT_ENTRY: u32 = 0xF000;
/// `G_RuneGroundGreen01.mdx` for the safe ring and `G_RuneGroundBlue01.mdx` for an exit, each
/// with its radius at scale 1.
const SAFE_RUNE: (u32, f32) = (674, 2.8);
const EXIT_RUNE: (u32, f32) = (343, 1.0);
/// How high a flight starts down, and over its last how many seconds it comes.
const FLIGHT_HEIGHT: f32 = 40.0;
const FLIGHT_DESCENT: f32 = 8.0;
/// Teleport: Stormwind, whose visual plays as a hero leaves.
const TELEPORT: u32 = 3561;
/// The colours heat and exit news float up in, 0xAARRGGBB.
const COLOR_HEAT: u32 = 0xFFFF_6A00;
const COLOR_EXIT: u32 = 0xFF66_FF99;
```

- [ ] **Step 2: `sync_extraction`**

After `sync_shrines`, add:

```rust
/// An extraction run's map: the safe ring and each exit's ring, kept on the ground as the terrain
/// loads; a beam over each open exit, seen from anywhere; and each exit's flight, coming down over
/// its last seconds and standing in the ring once landed.
#[allow(clippy::too_many_arguments)]
fn sync_extraction(
    commands: &mut Commands,
    world: &together_sim::World,
    views: &mut Views,
    names: &mut NameCache,
    index: &mut GuidIndex,
    ground: &impl Fn(V2, f32) -> Vec3,
    fallback_y: f32,
    transforms: &mut Query<&mut Transform, Without<SimPos>>,
) {
    let together_sim::Mode::Extraction(x) = &world.mode else {
        return;
    };
    let def = &world.content().extractions[x.def];
    let lift = Vec3::Y * 0.05;
    let view = views.extraction.get_or_insert_with(|| {
        let (rune, r) = SAFE_RUNE;
        let safe = units::spawn_object(
            commands,
            EXTRACTION_SERIAL_BASE,
            rune,
            def.safe.radius / r,
            ground(def.safe_center(), fallback_y) + lift,
        );
        let rings = (0..def.exits.len())
            .map(|i| {
                let (rune, r) = EXIT_RUNE;
                units::spawn_object(
                    commands,
                    EXTRACTION_SERIAL_BASE + 1 + 2 * i as u64,
                    rune,
                    def.exits[i].radius / r,
                    ground(def.exit_pos(i), fallback_y) + lift,
                )
            })
            .collect();
        ExtractionView {
            safe,
            rings,
            beams: vec![None; def.exits.len()],
            flights: vec![None; def.exits.len()],
        }
    });
    // Rings spawned before the terrain under them loaded settle onto it.
    let seats = std::iter::once((view.safe, def.safe_center()))
        .chain(view.rings.iter().enumerate().map(|(i, &e)| (e, def.exit_pos(i))));
    for (e, at) in seats {
        if let Ok(mut tf) = transforms.get_mut(e) {
            let want = ground(at, tf.translation.y) + lift;
            if tf.translation != want {
                tf.translation = want;
            }
        }
    }
    for (i, state) in x.exits.iter().enumerate() {
        let at = def.exit_pos(i);
        match (state.open, view.beams[i]) {
            (true, None) => {
                let serial = EXTRACTION_SERIAL_BASE + 2 + 2 * i as u64;
                let e = units::spawn_object(commands, serial, BEAM, BEAM_SCALE * 2.0, ground(at, fallback_y));
                view.beams[i] = Some(e);
            }
            (false, Some(e)) => {
                commands.entity(e).despawn();
                view.beams[i] = None;
            }
            _ => {}
        }
        let height = match (state.flight, state.landed) {
            (Some(left), _) if left <= FLIGHT_DESCENT => Some(left / FLIGHT_DESCENT * FLIGHT_HEIGHT),
            (_, true) => Some(0.0),
            _ => None,
        };
        match (height, view.flights[i]) {
            (Some(h), None) => {
                if views.named.insert(FLIGHT_ENTRY) {
                    names.insert_creature(
                        FLIGHT_ENTRY,
                        Some(CreatureRecord {
                            name: "Flight".into(),
                            subname: None,
                            creature_type: 0,
                            pet_family: 0,
                            rank: 0,
                            type_flags: 0,
                            civilian: true,
                            racial_leader: false,
                            display_id: def.flight_display,
                        }),
                    );
                }
                let kind = UnitKind::Creature { entry: FLIGHT_ENTRY };
                let serial = FLIGHT_SERIAL_BASE + i as u64;
                let e = units::spawn_unit(
                    commands,
                    &UnitSpec {
                        kind,
                        serial,
                        display: def.flight_display,
                        scale: 1.0,
                        level: 60,
                        faction: units::FACTION_FRIENDLY,
                        bytes0: 1 << 8,
                        weapons: &[],
                        sheath: 0,
                        extra: &[],
                    },
                    ground(at, fallback_y) + Vec3::Y * h,
                    0.0,
                );
                index.0.insert(units::unit_guid(kind, serial), e);
                view.flights[i] = Some(e);
            }
            (Some(h), Some(e)) => {
                if let Ok(mut tf) = transforms.get_mut(e) {
                    let want = ground(at, fallback_y) + Vec3::Y * h;
                    if tf.translation != want {
                        tf.translation = want;
                    }
                }
            }
            (None, Some(e)) => {
                index.0.retain(|_, v| *v != e);
                commands.entity(e).despawn();
                view.flights[i] = None;
            }
            (None, None) => {}
        }
    }
}
```

The compiler should accept `views.named` while `view` borrows `views.extraction`, since they're different fields of the same struct. If it doesn't, rewrite as `let named_new = views.named.insert(FLIGHT_ENTRY);` before `let view = views.extraction.get_or_insert_with(…)`, and use `named_new` inside the loop.

- [ ] **Step 3: Call it, and hide extracted heroes**

In `sync`:
1. Add a parameter `mut visibility: Query<&mut Visibility>,` after `mut transforms: Query<&mut Transform, Without<SimPos>>,`.
2. After the line `sync_shrines(&mut commands, world, &mut views, &ground, fallback_y, me, &mut transforms);`, add:

```rust
    sync_extraction(
        &mut commands,
        world,
        &mut views,
        &mut names,
        &mut index,
        &ground,
        fallback_y,
        &mut transforms,
    );
    // A hero gone on a flight is out of the world: their body goes from sight.
    for hero in &world.heroes {
        let want = if hero.extracted {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if let Some(mut v) = views
            .heroes
            .get(&hero.player)
            .and_then(|&e| visibility.get_mut(e).ok())
        {
            if *v != want {
                *v = want;
            }
        }
    }
```

- [ ] **Step 4: Floating text for the events**

Replace the Task 3 no-op arm in `apply_events` with:

```rust
            Event::HeatChanged { player, skulls } => {
                if player.is_none_or(|p| p == sim.me) {
                    if let Some(&e) = views.heroes.get(&sim.me) {
                        let text = match skulls {
                            0 => "The heat is off".to_string(),
                            n => format!("Heat {n}!"),
                        };
                        fx.text(e, text, COLOR_HEAT);
                    }
                }
            }
            Event::HuntersCame { near } => {
                if near == sim.me {
                    if let Some(&e) = views.heroes.get(&near) {
                        fx.text(e, "Hunters are cutting you off!".into(), COLOR_HEAT);
                    }
                }
            }
            Event::ExitOpened { exit } => {
                if let Some(&e) = views.heroes.get(&sim.me) {
                    fx.text(e, format!("{} is open", exit_name(&sim.world, exit)), COLOR_EXIT);
                }
            }
            Event::FlightCalled { exit, .. } => {
                if let Some(&e) = views.heroes.get(&sim.me) {
                    let text = format!("A flight is coming to {}", exit_name(&sim.world, exit));
                    fx.text(e, text, COLOR_EXIT);
                }
            }
            // The flight is drawn from the world's state.
            Event::FlightLanded { .. } => {}
            Event::HeroExtracted { player, .. } => {
                if let Some(&e) = views.heroes.get(&player) {
                    fx.cast(e, TELEPORT, &[e], None);
                }
            }
```

And after `gem_look`, add:

```rust
/// Exit `exit`'s name on the map being played; empty in the arena.
fn exit_name(world: &together_sim::World, exit: u8) -> String {
    match &world.mode {
        together_sim::Mode::Extraction(x) => world.content().extractions[x.def]
            .exits
            .get(usize::from(exit))
            .map_or_else(String::new, |e| e.name.clone()),
        together_sim::Mode::Arena => String::new(),
    }
}
```

- [ ] **Step 5: Build**

Run: `cargo build --release -p azeroth-together 2>&1 | tail -5`
Expected: `Finished`. Fix any borrow error with the field-split fallback in Step 2.

- [ ] **Step 6: Commit**

```bash
$G add crates/benilla-app/src/together/view.rs
$G commit -m "Client: draw the safe ring, exits, beams and flights; hide who has left

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The extraction HUD

The status shows the heat, its skulls and multiplier, the night floor, Goldshire, and each exit's state with distance and direction. The center panel shows "You got out!" while the others are still out, and an extraction end screen.

**Files:**
- Modify: `crates/benilla-app/src/together/hud.rs`

**Interfaces:**
- Consumes: `World::{mode, heat_of, extraction_settings, elapsed}`, `ExtractionDef::{skull_index, heat, exits, exit_pos, in_safe}`, `Extraction::{floor, peak, exits}`, `Hero::extracted`; the existing `clock`.
- Produces: `extraction_status`, `extraction_end`, `compass`.

- [ ] **Step 1: The helpers**

In `hud.rs`, after `fn clock`, add:

```rust
/// The way `to` points, as a compass word (WoW's `x` is north, `y` west; the minimap is
/// north-up).
fn compass(to: together_sim::V2) -> &'static str {
    const WAYS: [&str; 8] = ["N", "NW", "W", "SW", "S", "SE", "E", "NE"];
    let turn = to.y.atan2(to.x) / std::f32::consts::FRAC_PI_4;
    WAYS[(turn.round() as i32).rem_euclid(8) as usize]
}

/// Extraction runs: the heat and what it pays, Goldshire when the hero is in it, and how each
/// exit stands and where it lies. `None` in the arena.
fn extraction_status(world: &together_sim::World, hero: &together_sim::Hero) -> Option<String> {
    let together_sim::Mode::Extraction(x) = &world.mode else {
        return None;
    };
    let def = &world.content().extractions[x.def];
    let heat = world.heat_of(hero.player);
    let skulls = def.skull_index(heat);
    let skull = &def.heat.skulls[skulls];
    let top = def.heat.skulls.len() - 1;
    let next = def.heat.skulls.get(skulls + 1).map_or(String::new(), |n| {
        let part = ((heat - skull.at) / (n.at - skull.at) * 100.0).clamp(0.0, 99.0);
        format!("   next {part:.0}%")
    });
    let mut s = format!(
        "\nHeat {skulls}/{top}   Rewards x{:.2}{next}   Night floor {:.0}",
        skull.reward, x.floor
    );
    if def.in_safe(hero.pos) {
        s.push_str("\nGoldshire: safe. Nothing new comes; heat cools while everyone is home.");
    }
    for (i, (exit, state)) in def.exits.iter().zip(&x.exits).enumerate() {
        let to = def.exit_pos(i) - hero.pos;
        let way = format!("{:.0} yd {}", to.length(), compass(to));
        let line = if !state.open {
            let left = (exit.opens_at - world.elapsed).max(0.0);
            format!("{} opens in {}", exit.name, clock(left))
        } else if let Some(left) = state.flight {
            format!("{}: flight lands in {}   {way}", exit.name, clock(left.max(0.0)))
        } else if state.landed {
            format!("{}: the flight waits for everyone   {way}", exit.name)
        } else if state.calling > 0.0 {
            format!("{}: calling a flight {:.0}%   {way}", exit.name, state.calling * 100.0)
        } else {
            format!("{}: open, stand in it to call a flight   {way}", exit.name)
        };
        s.push('\n');
        s.push_str(&line);
    }
    Some(s)
}

/// Extraction runs: what the center panel says once the hero has left (`over`: the whole run
/// has ended), with `next` telling how to go on. `None` in the arena.
fn extraction_end(
    world: &together_sim::World,
    hero: &together_sim::Hero,
    over: bool,
    next: &str,
) -> Option<String> {
    let together_sim::Mode::Extraction(x) = &world.mode else {
        return None;
    };
    let def = &world.content().extractions[x.def];
    if !over {
        let out: Vec<String> = world
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| format!("{} is still out there", h.name))
            .collect();
        return Some(format!("You got out!\n\n{}", out.join("\n")));
    }
    let title = if hero.extracted {
        "You got out!"
    } else {
        "You did not make it out."
    };
    let party: Vec<String> = world
        .heroes
        .iter()
        .map(|h| {
            let how = if h.extracted { "got out" } else { "fell" };
            format!("{}: {how}", h.name)
        })
        .collect();
    Some(format!(
        "{title}\n\n{}\n\n{}  ·  Level {}  ·  {} kills  ·  {} gold  ·  peak heat {}/{}\n\n{next}",
        party.join("\n"),
        clock(world.elapsed),
        world.level,
        world.kills,
        hero.gold,
        def.skull_index(x.peak),
        def.heat.skulls.len() - 1,
    ))
}
```

- [ ] **Step 2: Use them in `refresh`**

In `refresh`:
1. Replace `} else if let Some(boss) = world.zone().boss_ix {` with:

```rust
        } else if let Some(boss) = world
            .zone()
            .boss_ix
            .filter(|_| world.extraction_settings().is_none_or(|s| s.timed_boss))
        {
```

2. Before the `// Friends who have fallen` comment, add:

```rust
        if let Some(lines) = extraction_status(world, hero) {
            s.push_str(&lines);
        }
```

3. Change the fallen-friends filter `.filter(|h| !h.alive)` to `.filter(|h| !h.alive && !h.extracted)`.
4. In the center-panel chain, make the `world.game_over()` branch use the extraction end first:

```rust
    let center = if world.game_over() {
        let next = if net.is_client() {
            "The host takes everyone back to the menu"
        } else {
            "Press Enter to return to the menu"
        };
        extraction_end(world, hero, true, next).or_else(|| {
            let fallen = if world.heroes.len() > 1 {
                "The party has fallen."
            } else {
                "You have fallen."
            };
            Some(format!(
                "{fallen}\n\nSurvived {}  ·  Level {}  ·  {} kills  ·  {:.0} damage\n\n{next}",
                clock(world.elapsed),
                world.level,
                world.kills,
                hero.damage_done,
            ))
        })
    } else if hero.extracted {
        extraction_end(world, hero, false, "")
    } else if !hero.alive {
```

   Keep the rest of the chain as it is: the existing `!hero.alive` body follows.

- [ ] **Step 3: Build**

Run: `cargo build --release -p azeroth-together 2>&1 | tail -5`
Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
$G add crates/benilla-app/src/together/hud.rs
$G commit -m "Client: the extraction HUD, heat and exits, and the end of the run

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Autoplay, showcase, and seeing it run

**Files:**
- Modify: `crates/benilla-app/src/together/mod.rs` (`autowalk`, `showcase_for_tests`, new `extraction_goal`)
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: `World::{mode, skulls_of}`, `ExtractionDef::{exit_pos, safe_center}`.
- Produces: autoplay that roams, goes home at three skulls and extracts once an exit is open; `TOGETHER_SHOWCASE=1` opening every exit at once in an extraction run.

- [ ] **Step 1: Autoplay toward exits and home**

In `mod.rs`, after `autowalk`, add:

```rust
/// `TOGETHER_AUTOPLAY` in an extraction run: to the nearest open exit once one is open; home to
/// the safe ring at three skulls; else `None`, to roam to shrines as in the arena.
fn extraction_goal(world: &World, me: PlayerId, from: V2) -> Option<V2> {
    let together_sim::Mode::Extraction(x) = &world.mode else {
        return None;
    };
    let def = &world.content().extractions[x.def];
    let exit = (0..def.exits.len())
        .filter(|&i| x.exits[i].open)
        .map(|i| def.exit_pos(i))
        .min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)));
    exit.or_else(|| (world.skulls_of(me) >= 3).then(|| def.safe_center()))
}
```

In `autowalk`, right after `let hero = world.hero(me).filter(|h| h.alive && h.offers.is_empty())?;`, add:

```rust
    if let Some(goal) = extraction_goal(world, me, from) {
        let to = goal - from;
        let dist = to.length();
        if dist < 2.0 {
            return None;
        }
        return Some(from + to.normalize_or_zero() * (hero.speed * dt).min(dist));
    }
```

Update `autowalk`'s doc comment to start with: `TOGETHER_AUTOPLAY: where the local hero at from walks this frame: in an extraction run, to an open exit or home when hot ([extraction_goal]); else, with a banner, …`

- [ ] **Step 2: Showcase opens the exits**

At the end of `showcase_for_tests`, add:

```rust
    // An extraction run's exits open at once, so a short test run reaches a flight.
    if let together_sim::Mode::Extraction(x) = &mut world.mode {
        for exit in &mut x.exits {
            exit.open = true;
        }
    }
```

Update its doc comment to add `, and an extraction run's exits are open` after `the boss comes 8 seconds in`.

- [ ] **Step 3: Build**

Run: `cargo build --release -p azeroth-together 2>&1 | tail -5`
Expected: `Finished`.

- [ ] **Step 4: A background run with screenshots**

```bash
cd /d/Games/azeroth-together-extraction
mkdir -p shots
WOW_DATA=D:/Games/azeroth-together/WoW/Data WOW_BG=1 WOW_WIN=1280x720 \
TOGETHER_ZONE=elwynn_extraction TOGETHER_AUTOPLAY=1 TOGETHER_SHOWCASE=1 TOGETHER_TRACE_POS=1 \
WOW_LIVE_SHOT=shots/extraction.png WOW_LIVE_SHOT_AT=30 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=25 \
./target/release/azeroth-together.exe 2>&1 | tee shots/extraction.log | grep "together:"
```

Run it in the background and wait for it to exit (6 shots × 25 s after 30 s ≈ 3 minutes). Then check:
- The log has `together: content from …` and position lines that move from Goldshire, about (-9480, 63), toward Stormwind Gate (-9150, 360).
- In the screenshots: the green rune at Goldshire with the HUD's "Goldshire: safe" line; the heat line climbing once outside; "Stormwind Gate: open… yd NE"; a pillar of light at the gate; then "flight lands in…"; then a bat coming down and the "You got out!" end screen.
- If a shot shows the exit ring under a building or in a wall, read the gate's coordinates from the trace while standing by the gate, and correct `content/extraction.ron`.

Record what you see: which shots show what, and anything that looks wrong.

- [ ] **Step 5: Check Eastvale's coordinates**

Autoplay walks to the nearest open exit, which is Stormwind Gate. To reach Eastvale:
1. Temporarily delete the Stormwind Gate entry from `content/extraction.ron`. The game reads content from disk at startup, so this needs no rebuild.
2. Run Step 4's command with `WOW_LIVE_SHOT_AT=150 WOW_LIVE_SHOT_COUNT=5 WOW_LIVE_SHOT_EVERY=20`. The walk from Goldshire is about 1,340 yards.
3. Read the trace and screenshots near (-9467, -1276). If the ring is in the logging camp's open ground, leave the position alone. If not, set Eastvale's `pos` to a nearby open spot read from the trace.
4. Restore the Stormwind Gate entry, and check with `$G diff content/extraction.ron` that only a real coordinate change remains.

- [ ] **Step 6: A co-op run on this PC**

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data WOW_BG=1 WOW_WIN=1280x720 TOGETHER_ZONE=elwynn_extraction \
TOGETHER_HOST=1 TOGETHER_AUTOPLAY=1 TOGETHER_SHOWCASE=1 TOGETHER_NAME=Host \
WOW_LIVE_SHOT=shots/host.png WOW_LIVE_SHOT_AT=60 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=30 \
./target/release/azeroth-together.exe > shots/host.log 2>&1 &
sleep 5
WOW_DATA=D:/Games/azeroth-together/WoW/Data WOW_BG=1 WOW_WIN=1280x720 \
TOGETHER_JOIN=127.0.0.1 TOGETHER_AUTOPLAY=1 TOGETHER_NAME=Guest \
WOW_LIVE_SHOT=shots/guest.png WOW_LIVE_SHOT_AT=60 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=30 \
./target/release/azeroth-together.exe > shots/guest.log 2>&1 &
wait
```

Expected: the guest joins (no content-mismatch refusal in `shots/guest.log`). Both HUDs show the same heat and exit lines. Both heroes reach the open gate. The end screens list both heroes. Write down what the shots show.

- [ ] **Step 7: Document**

In `CLAUDE.md`, under "Where things live", after the `content/*.ron` bullet, add:

```markdown
- Extraction mode: `content/extraction.ron` (maps: outline, safe ring, heat, exits) played in a
  zone that points at it (`elwynn_extraction`). Rules in `together-sim/src/world/extraction.rs`,
  data and state in `together-sim/src/extraction.rs`. Spec and plans under `docs/superpowers/`.
```

In the testing bullet list, add:

```markdown
- Extraction: `TOGETHER_ZONE=elwynn_extraction`; `TOGETHER_SHOWCASE=1` opens the exits at once;
  `TOGETHER_TRACE_POS=1` logs the hero's position every 2 s, for reading off coordinates. From a
  second worktree, point `WOW_DATA` at the main checkout's `WoW/Data`.
```

- [ ] **Step 8: Commit**

```bash
$G add crates/benilla-app/src/together/mod.rs CLAUDE.md content/extraction.ron
$G commit -m "Client: autoplay extracts, showcase opens the exits; docs

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

Don't commit `shots/`; delete it, or leave it untracked.

---

## Not in this plan (the next plan, after playtesting)

- Elite circles (Hogger, Princess, Goldtooth) and roaming rares, with minimap markers (spec §3.4, §3.6, build step 4).
- Hold circles on the charge circle (spec §3.5, build step 5).
- The Goldshire merchant, fixed chest spots and a tuning pass (spec §3.2, §3.7, build step 6).
- Minimap and world-map landmarks for the exits. This plan points the way with the HUD's distance and compass line and a beam over each open exit.
- Spectating a teammate's camera. An extracted player's hero is hidden, and their controller can still look around (spec §3.8).
- WoW's centre-screen warning text for announcements. This plan floats the text over the hero.
- `reaper_at` (spec §4) and the gryphon model (not in the trimmed data; the bat stands in).
- A separate Arena/Extraction switch in the menu (spec §6). Extraction shows as its own row in the zone list, "Elwynn Forest: Extraction", so the menu, the lobby and the co-op start message need no change. The spec's `Gryphon…` events are named `Flight…` for the same stand-in reason.
