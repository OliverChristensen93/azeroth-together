# Nightfall Sub-project 6 (The Night's Threats) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raids that stay uncertain: a night clock whose stages (Dusk, the howling, Moonrise, the hunt, Stitches) are heard coming and change the raid when they land, with no timer on screen; signature monsters with one learnable behaviour each (Mor'Ladim who moves only unwatched, Nightbane trackers on a hero's scent, a banshee whose wail wakes everything within 60 yards, the Dark Riders who see far along the roads, Stitches walking the road at the end); risk zones with tougher enemies and richer loot; the ravens' alarm; and every raid rolling its exits (some late, one perhaps closing in the hunt), its weather (clear, mist, rain) and which signature monsters are out, shown on the inn's map table.

**Architecture:** `nightfall-sim` gains a `night` module: its content (`rules.rs`: stages, weather, the roll's numbers, scent, the hunt, ravens; `places.rs`: risk zones, roads, signature monsters), and the host's rules in files beside it: the raid's roll (`roll.rs`, filling 5a's `RaidRoll` seam), the clock (`clock.rs`), who is out and the newcomers each stage brings (`arrivals.rs`), the hunt (`hunt.rs`), scent and trackers (`scent.rs`), Mor'Ladim and the banshee's alarm (`signature.rs`), and the ravens (`ravens.rs`). The night works through the seams the earlier plans left: the fight's `Engage` (Mor'Ladim watched is out of the fight), an enemy's `Mind` (trackers sniff, Mor'Ladim and the wail alert), a `Brain` errand (the hunt), the senses' fog and notice range (Moonrise, the weather, the riders' roads), spawn lines filtered by the roll, and enemy abilities (the wail is an ability with an `Alarm` effect). The height map learns water from the ADT liquids, so trails break there. Every snapshot carries a small `NightView` (stage, weather, flocks), every client draws the fog and weather from it and plays the cues the host's events name. One engine seam, a new file: `sound::SoundCue`, a game's own sound at a place or everywhere.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content), postcard (wire), `benilla-formats` (ADT liquids, in the bake).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md`: §7.2 (the night clock, `night_stages`, exits that open late), §7.3 (signature monsters), §8.1's risk zones and "every raid a little different", §7.1's ravens and trackers (deferred here by the senses' rulings), §8.5's map table intel (5b left its seam), §10's "rain that shortens the fog", §11's `night_stages`, §12's tests. Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-night-threats-rulings.md`. This plan starts after every task of the foundation, world-knowledge, senses, combat, run-loop (5a) and inn (5b) plans is done, and edits files those plans wrote; every name it consumes from them is the one those plans give. Rivals (sub-project 7) stay out.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call. Other agents commit in this worktree: stage only the files a task names, never `git add -A`.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`. This machine runs out of memory on wide builds: build `benilla-app` and `azeroth-together` with `-j 2` (`-j 1` if a build is killed).
- The worktree has no `WoW/Data`: every game launch and every data-reading example sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data`, and game launches `BENILLA_HOME=target/nf-home` (a test home). The exe does not exit after its shots: start it in the background, wait for the last shot file, then `kill` its PID.
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats` (spec §3.1). The clock, the roll (seeded, deterministic), every monster's behaviour, scent, the hunt, zones and ravens are pure sim logic with unit tests on hand-made maps (`heightmap::testmap`).
- Every number is content: `content/nightfall/rules.ron` under `night:` (the stages with their times, the weathers and their weights, the roll's chances, scent, the hunt, the ravens; the spec's values as defaults when left out), `map.ron` (`zones`, `roads`, `ravens`), `enemies.ron` (the monsters' traits), `monsters.ron` (new: the signature monsters, their odds and spawn lines), `spawns.ron` (`stage:` on a line), `abilities.ron` (the wail, Stitches' blows).
- The host decides (spec §3.1): clients get the night in each snapshot (`NightView`) and in events, and draw and play what those say.
- Engine changes are only these: a new file `crates/benilla-app/src/sound/cue.rs` and three lines in `crates/benilla-app/src/sound/mod.rs`. Everything else is in Nightfall's own crates and files; edits to files earlier plans wrote are spelled out.
- Nothing Blizzard-derived goes into `content/` or a commit: content names WoW ids, sound file paths and icon names only. Display ids and sound paths not yet checked in the data are marked `measure:` (see the rulings).
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A raid with no way out, or a hunt that closes the last one** (every exit rolled shut, the only open exit picked to close, a map whose exits are all late). Expect at least one exit open from the start whenever one can be, at least `open_least` open at some time, and the hunt never closing the last. Pinned in Task 3 (`a_thousand_raids_each_have_a_way_out`, `the_hunt_never_closes_the_last_way_out`).
2. **Enemies added mid-raid falling out of step with their parallel lists** (the world's `enemies` and `brains`, the senses' `minds` and `ways`, the fight's `enemies`; a senses step that silently stops when their lengths differ), or a newcomer appearing in a hero's face, or ids colliding. Expect every list the same length after every arrival, ids unique, the newcomer thinking like the rest, and no arrival while a hero is within `arrive_unseen` yards of where it comes. Pinned in Task 5 (`newcomers_wait_unseen_and_keep_every_list_in_step`).
3. **Mor'Ladim at the edges of being watched** (one of two heroes watching, a watcher behind a wall, a downed watcher, a watcher standing next to him). Expect him frozen, never striking, while any up hero sees him in their view cone, and moving the step nobody does. Pinned in Task 8 (`one_watcher_holds_him_while_a_friend_moves`, `a_wall_or_a_fallen_watcher_does_not_hold_him`, `watched_he_never_strikes`).
4. **A scent trail that survives water, a road or time, or one that cannot be followed at all.** Expect a tracker to follow a sneaking rogue's fresh trail to them, and to lose it at a pond, at a road and where the crumb it needs has faded. Pinned in Task 7 (`a_tracker_follows_a_sneaking_rogue_by_scent`, `water_breaks_the_trail`, `a_road_breaks_the_trail`, `a_trail_too_old_is_lost`).
5. **The clock at its edges** (a cue given twice, a stage landing before its cue, a clock started late by a test hook, a joined client's fog not the host's). Expect each stage cued once, `warn` seconds before it lands; a late clock giving every passed stage's cue and landing once, in order; the mirror's fog equal to the host's. Pinned in Task 4 (`each_stage_is_cued_before_it_lands_and_only_once`, `a_clock_started_late_gives_every_passed_stage_once_in_order`, `the_mirror_draws_the_hosts_fog`).

**Not in this plan** (see the rulings file): rivals (sub-project 7); Darkshire's Night Watch guards fighting Stitches and keeping monsters out (no enemy-against-enemy fight exists; Stitches turns back at Darkshire's road end); weather that changes during a raid (spec §14); a rolled entry point; measuring the real positions of zones, roads, flocks and signature monsters in the game (they are `measure:` placeholders, as every earlier plan's placements are).

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/night/mod.rs` (new) | `Night` (host state), `NightView`, `Fog`, `fog_of`, the flock states; `World::{night_view, fog, notice_scale, step_night, night_holds}` |
| `crates/nightfall-sim/src/night/rules.rs` (new) | `NightRules` and its parts: `StageDef`, `WeatherDef`, `ExitRoll`, `ScentRules`, `HuntRules`, `RavenRules`, `CueSound`, `StepSound`; checks |
| `crates/nightfall-sim/src/night/places.rs` (new) | `ZoneDef`, `RoadDef`, `MonsterDef` and their files; `there_and_back` |
| `crates/nightfall-sim/src/night/roll.rs` (new) | `RaidRoll::roll`: exits, the hunt's closing, weather, monsters |
| `crates/nightfall-sim/src/night/clock.rs` (new) | `stage_at`; cues, stages landing, the hunt's exit closing |
| `crates/nightfall-sim/src/night/arrivals.rs` (new) | `place_enemies`, newcomers per stage placed out of sight, `might_of` (zones' blows) |
| `crates/nightfall-sim/src/night/hunt.rs` (new) | The loudest recent noise; hunters sent after it |
| `crates/nightfall-sim/src/night/scent.rs` (new) | `Trail`, `Crumb`; trails laid; trackers following |
| `crates/nightfall-sim/src/night/signature.rs` (new) | Mor'Ladim watched; the banshee's `alarm` |
| `crates/nightfall-sim/src/night/ravens.rs` (new) | Flocks rolled, taking off, coming back |
| `crates/nightfall-sim/src/night/tests.rs` (new) | The night in whole worlds on hand-made maps |
| `crates/nightfall-sim/src/{content,abilities,enemies,world,sight,lib,messages}.rs` | Content glue; `Effect::Alarm`; spawn lines filtered, zones' health, errands; the step's order; fog in sight; `Snapshot::night`; protocol 7 |
| `crates/nightfall-sim/src/senses/{mod,awareness,react}.rs` | `minds` crate-visible, `Senses::add`, `State` exported; `Mind::sniff`; fog, notice and the riders' road sight |
| `crates/nightfall-sim/src/combat/{mod,strikes,falling}.rs` | Night events; `EnemyCombat::fresh`, `Combat::add`; the wail's alarm; zones' damage |
| `crates/nightfall-sim/src/raid/{mod,extract,containers}.rs` | `RaidRoll` fields and `exit_open`; zones' loot |
| `crates/nightfall-sim/src/heightmap/{mod,build,codec,testmap}.rs` | Water columns; codec version 2; `pond` |
| `crates/nightfall-bake/src/{bake,cache}.rs`, `examples/bake_duskwood.rs` | ADT liquids baked as water; `BAKE_VERSION` 2 |
| `content/nightfall/monsters.ron` (new), `{rules,map,enemies,spawns,abilities,loot,contracts}.ron` | The night's content |
| `crates/benilla-app/src/sound/cue.rs` (new, engine), `sound/mod.rs` | `SoundCue`, `Cue` |
| `crates/benilla-app/src/nightfall/night.rs` (new) | Fog, weather, cues and lines, Stitches' tread, the planned raid and its test hooks, the map table's lines |
| `crates/benilla-app/src/nightfall/ravens.rs` (new) | Flocks drawn, their cry |
| `crates/benilla-app/src/nightfall/{mod,session,run,enemies,board,fight}.rs` | Registration, `monsters.ron` read, the roll planned, `NIGHTFALL_NIGHT_AT`, mounts, intel, new events |

---

### Task 1: Content: the night's rules, places, monsters and traits

**Files:**
- Create: `crates/nightfall-sim/src/night/mod.rs`, `crates/nightfall-sim/src/night/rules.rs`, `crates/nightfall-sim/src/night/places.rs`, `content/nightfall/monsters.ron`
- Modify: `crates/nightfall-sim/src/content.rs`, `crates/nightfall-sim/src/abilities.rs` (`Effect::Alarm`), `crates/nightfall-sim/src/enemies.rs` (the `Road` arm), `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`
- Modify (every `ContentSources { … }` literal gains `monsters`): the test fixtures in `crates/nightfall-sim/src/{content,world,enemies,sight,messages}.rs`, `senses/tests.rs`, `combat/{mod,tests}.rs`, `raid/tests.rs`, and any in `crates/benilla-app/src/nightfall` (`grep -rn "ContentSources {" crates/nightfall-sim crates/benilla-app/src/nightfall`)
- Modify: `content/nightfall/{rules,map,enemies,spawns,abilities,loot,contracts}.ron`
- Modify (so the client still builds): `crates/benilla-app/src/nightfall/mod.rs` (`prepare`, `CONTENT_TEXTS`, `content_texts`)

**Interfaces:**
- Consumes: `Content::from_ron`, `ContentSources`, `ContentError`, `Rules`, `EnemyDef`, `MoveDef`, `SpawnDef`, `Behaviour`, `MapDef`, `MapFile`, `Bounds`, `Effect`, the content tests' fixtures `CLASSES`, `ENEMIES`, `SPAWNS`, `RULES` (earlier plans).
- Produces:
  - `nightfall_sim::{NightRules, StageDef, WeatherDef, ExitRoll, ScentRules, HuntRules, RavenRules, CueSound, StepSound, ZoneDef, RoadDef, MonsterDef}`; `Rules::night: NightRules` (`#[serde(default)]`, the spec's five stages at 0, 480, 840, 1200 and 1560 s); `NightRules::check(&self) -> Result<(), String>` (crate-only).
  - `CueSound::{Kit(u32), Name(String), File(String)}`; `StepSound { sound: CueSound, every: f32, heard: f32 }`.
  - `StageDef { key, name, at, warn, fog, notice, hunt, cue: Option<CueSound>, line: Option<String> }`; `WeatherDef { key, name, weight, fog_start, fog_end, fog_color: Option<(f32, f32, f32)>, rain: f32, sound: u32 }`; `ExitRoll { shut, late, late_window: (f32, f32), cart_jitter, hunt_close, open_least: u8 }`; `ScentRules { every, lasts, pick }` and `HuntRules { every, memory, reach, rest }` (both `Copy`); `RavenRules { odds, run_within, fight_within, back_after, clear, birds: u8, display: u32, scale: f32, cry: Option<CueSound> }`; `NightRules { stages, weather, exits, monsters_out: (u8, u8), arrive_unseen, look_cone, scent, hunt, ravens }`.
  - `ZoneDef { key, name, area: Bounds, hp, damage, loot_rolls: u8 }`; `RoadDef { key, name, width, points: Vec<V2> }` with `holds(&self, V2) -> bool`; `MonsterDef { key, name, odds, always, lines: Vec<usize> }`; crate-only `there_and_back(&[V2]) -> Vec<(f32, f32)>`, `segment_distance(V2, V2, V2) -> f32`.
  - `MapDef::{zones: Vec<ZoneDef>, roads: Vec<RoadDef>, flocks: Vec<V2>}` with `zone_at(&self, V2) -> Option<&ZoneDef>` and `on_road(&self, V2) -> bool`.
  - `EnemyDef` gains `watched: Option<f32>`, `tracker: bool`, `hunter: bool`, `road_sight: f32`, `mount: u32`, `step_sound: Option<StepSound>`, `kit_first: Vec<f32>` (resolved); `MoveDef` gains `first: Option<f32>`.
  - `Behaviour::Road { road: String, pause: f32 }` (made a `Patrol` there and back as the content loads).
  - `SpawnDef` gains `stage: Option<String>`, `stage_at: Option<usize>` (resolved), `monster: Option<usize>` (resolved).
  - `Effect::Alarm { radius: f32 }`.
  - `ContentSources::monsters: &str`; `Content::monsters: Vec<MonsterDef>` (their spawn lines appended to `Content::spawns`); `Content::{monster(&self, &str) -> Option<usize>, stage(&self, &str) -> Option<usize>}`.
  - Client: `nightfall::content_texts() -> &'static [String; 12]` (`monsters.ron` last).

- [ ] **Step 1: Write the night's content types**

`crates/nightfall-sim/src/night/rules.rs`:

```rust
//! The night's numbers (spec §7.2, §7.3, §8.1, §11's `night_stages`): `rules.ron`'s `night:`.
//! Every number is a starting value to playtest; each one left out takes the spec's.

use serde::Deserialize;

/// A sound the client plays: a SoundEntries kit by id or by name (heard from where it is), or a
/// file by its path (heard everywhere alike). The host never plays one; it says when.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum CueSound {
    Kit(u32),
    Name(String),
    File(String),
}

/// A sound an enemy makes while it moves (Stitches' tread, spec §7.2's "loud"): the client's.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct StepSound {
    pub sound: CueSound,
    /// Seconds between two.
    pub every: f32,
    /// Heard within this many yards of the local hero.
    #[serde(default = "sixty")]
    pub heard: f32,
}

fn sixty() -> f32 {
    60.0
}

fn twenty() -> f32 {
    20.0
}

fn one() -> f32 {
    1.0
}

/// One stage of the night clock (spec §7.2).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct StageDef {
    pub key: String,
    pub name: String,
    /// Seconds into the raid it lands.
    pub at: f32,
    /// Seconds before it lands that its cue sounds.
    #[serde(default = "twenty")]
    pub warn: f32,
    /// While it lasts, the fog's distances are times this (Moonrise's 40 to 30 yards is 0.75).
    #[serde(default = "one")]
    pub fog: f32,
    /// While it lasts, enemies' notice range is times this (Moonrise's +20% is 1.2).
    #[serde(default = "one")]
    pub notice: f32,
    /// While it lasts, hunters go after the loudest recent noise (the hunt).
    #[serde(default)]
    pub hunt: bool,
    /// What the players hear `warn` seconds before it lands, and the line their chat shows then.
    #[serde(default)]
    pub cue: Option<CueSound>,
    #[serde(default)]
    pub line: Option<String>,
}

impl StageDef {
    /// A stage with the defaults a file leaves out.
    fn new(key: &str, name: &str, at: f32) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            at,
            warn: twenty(),
            fog: 1.0,
            notice: 1.0,
            hunt: false,
            cue: None,
            line: None,
        }
    }
}

/// One weather a raid may roll (spec §8.1: clear, mist, rain shortening the fog).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct WeatherDef {
    pub key: String,
    pub name: String,
    /// How often it is rolled, against the others' weights.
    pub weight: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    /// The fog's colour (sRGB 0..1); the atmosphere's when left out.
    #[serde(default)]
    pub fog_color: Option<(f32, f32, f32)>,
    /// How hard it rains, 0 (no rain) to 1: the engine's own rain at this grade.
    #[serde(default)]
    pub rain: f32,
    /// Its sound loop, a SoundEntries kit (8533 light rain, 8534 medium, 8535 heavy); 0 for none.
    #[serde(default)]
    pub sound: u32,
}

/// How a raid rolls its exits (spec §8.1, §7.2).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct ExitRoll {
    /// The chance each exit that is not late stays shut the whole raid.
    pub shut: f32,
    /// The chance each exit that is not late opens late, at a time within `late_window`.
    pub late: f32,
    pub late_window: (f32, f32),
    /// The late exits (Darkshire's cart) open within this many seconds of `cart_opens_at`.
    pub cart_jitter: f32,
    /// The chance one exit closes as the hunt lands (spec §7.2: "one exit may close").
    pub hunt_close: f32,
    /// However the dice fall, at least this many exits open in a raid, one of them from the start.
    pub open_least: u8,
}

impl Default for ExitRoll {
    fn default() -> Self {
        Self {
            shut: 0.15,
            late: 0.25,
            late_window: (900.0, 1320.0),
            cart_jitter: 120.0,
            hunt_close: 0.5,
            open_least: 2,
        }
    }
}

/// Scent trails (spec §7.1, §7.3: the trackers).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct ScentRules {
    /// A hero leaves a crumb every so many yards walked.
    pub every: f32,
    /// Seconds a crumb lasts (the spec's "trail of the last 30 s").
    pub lasts: f32,
    /// A tracker smells a crumb within this many yards.
    pub pick: f32,
}

impl Default for ScentRules {
    fn default() -> Self {
        Self {
            every: 2.0,
            lasts: 30.0,
            pick: 3.0,
        }
    }
}

/// The hunt (spec §7.2: "worgen packs hunt toward the loudest recent noise").
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct HuntRules {
    /// Seconds between two calls to the hunt.
    pub every: f32,
    /// Seconds a noise counts as recent.
    pub memory: f32,
    /// Hunters within this many yards of the noise go.
    pub reach: f32,
    /// Seconds a hunter looks about where the noise was before it goes home.
    pub rest: f32,
}

impl Default for HuntRules {
    fn default() -> Self {
        Self {
            every: 15.0,
            memory: 30.0,
            reach: 150.0,
            rest: 4.0,
        }
    }
}

/// The ravens (spec §7.1): an alarm on the ground.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct RavenRules {
    /// The chance each of the map's flocks is out in a raid.
    pub odds: f32,
    /// A flock takes off when a hero runs within this many yards, or fights within the next.
    pub run_within: f32,
    pub fight_within: f32,
    /// Seconds before a flock that took off settles again, once no hero is within `clear` yards.
    pub back_after: f32,
    pub clear: f32,
    /// What the client draws: so many birds of this `CreatureDisplayInfo` id at this scale, and
    /// the cry it plays as they take off.
    pub birds: u8,
    pub display: u32,
    pub scale: f32,
    pub cry: Option<CueSound>,
}

impl Default for RavenRules {
    fn default() -> Self {
        Self {
            odds: 0.7,
            run_within: 8.0,
            fight_within: 15.0,
            back_after: 120.0,
            clear: 20.0,
            birds: 3,
            display: 14562,
            scale: 0.3,
            cry: None,
        }
    }
}

/// The night (spec §7.2, §7.3, §8.1).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct NightRules {
    /// The clock's stages, the first at 0.
    pub stages: Vec<StageDef>,
    /// The weathers a raid may roll; none for the atmosphere's own fog every raid.
    pub weather: Vec<WeatherDef>,
    pub exits: ExitRoll,
    /// How many of the signature monsters that are not always out a raid rolls (fewest, most).
    pub monsters_out: (u8, u8),
    /// A stage's newcomers wait until no hero is this near where they come, yards.
    pub arrive_unseen: f32,
    /// Mor'Ladim counts as looked at within this many degrees of a hero's facing (the whole cone).
    pub look_cone: f32,
    pub scent: ScentRules,
    pub hunt: HuntRules,
    pub ravens: RavenRules,
}

impl Default for NightRules {
    fn default() -> Self {
        Self {
            stages: spec_stages(),
            weather: Vec::new(),
            exits: ExitRoll::default(),
            monsters_out: (1, 3),
            arrive_unseen: 50.0,
            look_cone: 90.0,
            scent: ScentRules::default(),
            hunt: HuntRules::default(),
            ravens: RavenRules::default(),
        }
    }
}

/// The spec's §7.2 table: Dusk at 0:00, the howling at 8:00, Moonrise at 14:00 (fog 40 to 30
/// yards, notice +20%), the hunt at 20:00, Stitches at 26:00; Moonrise's fog and eyes last.
fn spec_stages() -> Vec<StageDef> {
    let late = |key: &str, name: &str, at: f32, hunt: bool| StageDef {
        fog: 0.75,
        notice: 1.2,
        hunt,
        ..StageDef::new(key, name, at)
    };
    vec![
        StageDef::new("dusk", "Dusk", 0.0),
        StageDef::new("howling", "The howling", 480.0),
        late("moonrise", "Moonrise", 840.0, false),
        late("hunt", "The hunt", 1200.0, true),
        late("stitches", "Stitches", 1560.0, true),
    ]
}

impl NightRules {
    /// What is wrong with these numbers, if anything: each check is one the night relies on.
    pub(crate) fn check(&self) -> Result<(), String> {
        let unit = |x: f32| (0.0..=1.0).contains(&x);
        let Some(first) = self.stages.first() else {
            return Err("needs a stage".into());
        };
        if first.at != 0.0 {
            return Err(format!("the first stage, \"{}\", lands at 0", first.key));
        }
        if self.stages.len() > usize::from(u8::MAX) || self.weather.len() > usize::from(u8::MAX) {
            return Err("at most 255 stages and 255 weathers".into());
        }
        for (i, s) in self.stages.iter().enumerate() {
            let who = format!("stage \"{}\"", s.key);
            if self.stages[..i].iter().any(|t| t.key == s.key) {
                return Err(format!("{who} is listed twice"));
            }
            if !(s.at.is_finite()
                && s.warn >= 0.0
                && s.fog > 0.0
                && s.fog <= 2.0
                && s.notice > 0.0
                && s.notice <= 3.0)
            {
                return Err(format!(
                    "{who}: warn 0 or more, fog above 0 to 2, notice above 0 to 3"
                ));
            }
            if i > 0 && !(s.at > self.stages[i - 1].at && s.at - s.warn >= self.stages[i - 1].at) {
                return Err(format!(
                    "{who} lands after the stage before it, and is cued after that one lands"
                ));
            }
            // A line goes into Lua between long brackets.
            if s.line.as_deref().is_some_and(|l| l.contains("]]")) {
                return Err(format!("{who}: a line may not hold \"]]\""));
            }
        }
        for (i, w) in self.weather.iter().enumerate() {
            let who = format!("weather \"{}\"", w.key);
            if self.weather[..i].iter().any(|v| v.key == w.key) {
                return Err(format!("{who} is listed twice"));
            }
            if !(w.weight >= 0.0
                && w.weight.is_finite()
                && w.fog_start >= 0.0
                && w.fog_start < w.fog_end
                && w.fog_end.is_finite()
                && unit(w.rain))
            {
                return Err(format!(
                    "{who}: weight 0 or more, fog_start from 0 and below fog_end, rain 0 to 1"
                ));
            }
        }
        if !self.weather.is_empty() && !(self.weather.iter().map(|w| w.weight).sum::<f32>() > 0.0)
        {
            return Err("the weathers' weights add up to more than 0".into());
        }
        let x = &self.exits;
        if !(unit(x.shut)
            && unit(x.late)
            && unit(x.hunt_close)
            && x.cart_jitter >= 0.0
            && x.late_window.0 >= 0.0
            && x.late_window.0 <= x.late_window.1
            && x.open_least >= 1)
        {
            return Err(
                "exits: chances 0 to 1, a late_window from 0 up in order, cart_jitter 0 or more, open_least 1 or more"
                    .into(),
            );
        }
        if self.monsters_out.0 > self.monsters_out.1 {
            return Err("monsters_out is (fewest, most)".into());
        }
        if !(self.arrive_unseen >= 0.0 && self.look_cone > 0.0 && self.look_cone <= 360.0) {
            return Err("arrive_unseen 0 or more, look_cone above 0 to 360".into());
        }
        let (s, h, r) = (&self.scent, &self.hunt, &self.ravens);
        if !(s.every > 0.0 && s.lasts > 0.0 && s.pick > 0.0) {
            return Err("scent: every, lasts and pick above 0".into());
        }
        if !(h.every > 0.0 && h.memory > 0.0 && h.reach > 0.0 && h.rest >= 0.0) {
            return Err("hunt: every, memory and reach above 0, rest 0 or more".into());
        }
        if !(unit(r.odds)
            && r.run_within >= 0.0
            && r.fight_within >= 0.0
            && r.back_after > 0.0
            && r.clear >= 0.0
            && r.scale > 0.0)
        {
            return Err(
                "ravens: odds 0 to 1, distances 0 or more, back_after and scale above 0".into(),
            );
        }
        Ok(())
    }
}
```

`crates/nightfall-sim/src/night/places.rs`:

```rust
//! Places the night uses (spec §7.3, §8.1): risk zones, roads and the ravens' flocks from
//! `map.ron`, and the signature monsters of `monsters.ron`.

use serde::Deserialize;

use crate::{Bounds, SpawnDef, V2};

fn one() -> f32 {
    1.0
}

fn six() -> f32 {
    6.0
}

/// A risk zone, or the gentler outskirts (spec §8.1): enemies placed inside have their health and
/// their blows times these, and containers and bodies inside roll their tables `loot_rolls` more
/// times. Death works the same everywhere.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneDef {
    pub key: String,
    pub name: String,
    pub area: Bounds,
    pub hp: f32,
    pub damage: f32,
    pub loot_rolls: u8,
}

#[derive(Deserialize)]
pub(crate) struct ZoneFile {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) area: Vec<(f32, f32)>,
    #[serde(default = "one")]
    pub(crate) hp: f32,
    #[serde(default = "one")]
    pub(crate) damage: f32,
    #[serde(default)]
    pub(crate) loot_rolls: u8,
}

/// A road (spec §7.3): the Dark Riders ride it and see far along it, Stitches walks one, and a
/// scent trail across one is broken.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadDef {
    pub key: String,
    pub name: String,
    /// Yards across.
    pub width: f32,
    pub points: Vec<V2>,
}

#[derive(Deserialize)]
pub(crate) struct RoadFile {
    pub(crate) key: String,
    pub(crate) name: String,
    #[serde(default = "six")]
    pub(crate) width: f32,
    pub(crate) points: Vec<(f32, f32)>,
}

impl RoadDef {
    /// Whether `p` stands on the road: within half its width of its line.
    pub fn holds(&self, p: V2) -> bool {
        self.points
            .windows(2)
            .any(|w| segment_distance(p, w[0], w[1]) <= self.width * 0.5)
    }
}

/// How far `p` is from the segment from `a` to `b`.
pub(crate) fn segment_distance(p: V2, a: V2, b: V2) -> f32 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

/// A road's points from end to end and back, without walking either end twice: a patrol route,
/// which loops from its last point to its first.
pub(crate) fn there_and_back(points: &[V2]) -> Vec<(f32, f32)> {
    let mut route: Vec<(f32, f32)> = points.iter().map(|p| (p.x, p.y)).collect();
    route.extend(
        points
            .iter()
            .rev()
            .skip(1)
            .take(points.len().saturating_sub(2))
            .map(|p| (p.x, p.y)),
    );
    route
}

/// A signature monster (spec §7.3): its spawn lines (in `Content::spawns`) are placed only in the
/// raids that roll it out, and every raid for one `always` out.
#[derive(Clone, Debug, PartialEq)]
pub struct MonsterDef {
    pub key: String,
    pub name: String,
    /// The chance a raid rolls it out.
    pub odds: f32,
    pub always: bool,
    /// Its spawn lines, indices into `Content::spawns`.
    pub lines: Vec<usize>,
}

#[derive(Deserialize)]
pub(crate) struct MonsterFile {
    pub(crate) key: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) odds: f32,
    #[serde(default)]
    pub(crate) always: bool,
    pub(crate) spawns: Vec<SpawnDef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_road_holds_what_is_within_half_its_width() {
        let r = RoadDef {
            key: "r".into(),
            name: "R".into(),
            width: 6.0,
            points: vec![V2::new(0.0, 0.0), V2::new(10.0, 0.0), V2::new(10.0, 10.0)],
        };
        assert!(r.holds(V2::new(5.0, 2.9)));
        assert!(!r.holds(V2::new(5.0, 3.1)));
        assert!(r.holds(V2::new(12.5, 6.0)));
        assert!(!r.holds(V2::new(-3.5, 0.0)));
    }

    #[test]
    fn a_road_there_and_back_walks_each_end_once() {
        let p = |x: f32| V2::new(x, 0.0);
        assert_eq!(
            there_and_back(&[p(1.0), p(2.0), p(3.0), p(4.0)]),
            vec![(1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0), (3.0, 0.0), (2.0, 0.0)]
        );
        assert_eq!(there_and_back(&[p(1.0), p(2.0)]), vec![(1.0, 0.0), (2.0, 0.0)]);
    }
}
```

`crates/nightfall-sim/src/night/mod.rs`:

```rust
//! The night's threats (spec §7.2, §7.3, §8.1): the clock and its stages, the raid's roll,
//! signature monsters, risk zones, scent, the hunt and the ravens. This first part is their
//! content; the host's rules come in the files beside it.

mod places;
mod rules;

pub(crate) use places::{there_and_back, MonsterFile, RoadFile, ZoneFile};
pub use places::{MonsterDef, RoadDef, ZoneDef};
pub use rules::{
    CueSound, ExitRoll, HuntRules, NightRules, RavenRules, ScentRules, StageDef, StepSound,
    WeatherDef,
};
```

In `crates/nightfall-sim/src/lib.rs` add `mod night;` beside the other modules and:

```rust
pub use night::{
    CueSound, ExitRoll, HuntRules, MonsterDef, NightRules, RavenRules, RoadDef, ScentRules,
    StageDef, StepSound, WeatherDef, ZoneDef,
};
```

Run: `cargo test -p nightfall-sim night`
Expected: PASS (the two road tests).

- [ ] **Step 2: Write the failing content tests**

In `crates/nightfall-sim/src/content.rs`'s `tests` module, add:

```rust
    const NIGHT_MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
        entries: [(50.0, 50.0, 10.0)],
        zones: [(key: "hill", name: "The Hill", hp: 1.5, damage: 1.25, loot_rolls: 1,
                 area: [(60.0, 60.0), (90.0, 60.0), (90.0, 90.0), (60.0, 90.0)])],
        roads: [(key: "road", name: "The Road", points: [(10.0, 50.0), (50.0, 50.0), (90.0, 50.0)])],
        ravens: [(20.0, 20.0)])"#;
    const MONSTERS: &str = r#"[
        (key: "ghost", name: "The Ghost", odds: 0.5, spawns: [
            (place: "Hill", kind: "spider", behaviour: Wander(center: (75.0, 75.0), radius: 5.0))]),
        (key: "walker", name: "The Walker", always: true, spawns: [
            (place: "Road", kind: "skeleton", stage: Some("late"), behaviour: Road(road: "road"))]),
    ]"#;

    fn night_rules() -> String {
        RULES.replacen(
            "(hero_level: 30,",
            r#"(hero_level: 30, night: (stages: [(key: "dusk", name: "Dusk", at: 0.0),
                (key: "late", name: "Late", at: 60.0, warn: 10.0, fog: 0.75)]),"#,
            1,
        )
    }

    fn load_night(map: &str, rules: &str, monsters: &str) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            map,
            rules,
            enemies: ENEMIES,
            spawns: SPAWNS,
            abilities: "[]",
            items: "[]",
            loot: "()",
            monsters,
        })
    }

    #[test]
    fn the_night_defaults_to_the_spec() {
        let c = load_night(MAP, RULES, "[]").unwrap();
        let n = &c.rules.night;
        let at: Vec<f32> = n.stages.iter().map(|s| s.at).collect();
        assert_eq!(at, vec![0.0, 480.0, 840.0, 1200.0, 1560.0]);
        assert_eq!(n.stages[2].key, "moonrise");
        assert_eq!((n.stages[2].fog, n.stages[2].notice), (0.75, 1.2));
        assert!(n.stages[3].hunt && !n.stages[1].hunt);
        assert!(n.weather.is_empty());
        assert_eq!(n.scent.lasts, 30.0);
        assert!(c.monsters.is_empty() && c.map.zones.is_empty() && c.map.roads.is_empty());
    }

    #[test]
    fn places_and_monsters_load() {
        let c = load_night(NIGHT_MAP, &night_rules(), MONSTERS).unwrap();
        assert_eq!(c.map.zones[0].hp, 1.5);
        assert_eq!(c.map.zone_at(V2::new(70.0, 70.0)).map(|z| z.key.as_str()), Some("hill"));
        assert!(c.map.zone_at(V2::new(10.0, 10.0)).is_none());
        assert!(c.map.on_road(V2::new(30.0, 52.9)));
        assert!(!c.map.on_road(V2::new(30.0, 60.0)));
        assert_eq!(c.map.flocks, vec![V2::new(20.0, 20.0)]);
        assert_eq!(c.monster("walker"), Some(1));
        assert_eq!(c.stage("late"), Some(1));
        // The monsters' lines come after spawns.ron's three.
        assert_eq!(c.monsters[0].lines, vec![3]);
        assert_eq!(c.monsters[1].lines, vec![4]);
        assert!(c.monsters[1].always && !c.monsters[0].always);
        let walker = &c.spawns[4];
        assert_eq!((walker.monster, walker.stage_at), (Some(1), Some(1)));
        assert_eq!(c.spawns[0].monster, None);
        match &walker.behaviour {
            Behaviour::Patrol { route, pause } => {
                assert_eq!(route, &vec![(10.0, 50.0), (50.0, 50.0), (90.0, 50.0), (50.0, 50.0)]);
                assert_eq!(*pause, 0.0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn night_content_that_cannot_work_is_refused() {
        let bad_monsters = [
            MONSTERS.replace("Some(\"late\")", "Some(\"dawn\")"),
            MONSTERS.replace("road: \"road\"", "road: \"path\""),
            MONSTERS.replace("odds: 0.5", "odds: 1.5"),
            MONSTERS.replace("\"walker\", name", "\"ghost\", name"),
        ];
        for m in &bad_monsters {
            assert!(
                matches!(load_night(NIGHT_MAP, &night_rules(), m), Err(ContentError::Invalid(_))),
                "{m}"
            );
        }
        let bad_rules = [
            night_rules().replace("at: 60.0, warn: 10.0", "at: 60.0, warn: 61.0"),
            night_rules().replace("name: \"Dusk\", at: 0.0", "name: \"Dusk\", at: 5.0"),
            night_rules().replace("fog: 0.75", "fog: 0.0"),
            RULES.replacen(
                "(hero_level: 30,",
                "(hero_level: 30, night: (weather: [(key: \"m\", name: \"M\", weight: 1.0, fog_start: 30.0, fog_end: 20.0)]),",
                1,
            ),
            RULES.replacen("(hero_level: 30,", "(hero_level: 30, night: (monsters_out: (3, 1)),", 1),
        ];
        for r in &bad_rules {
            assert!(
                matches!(load_night(NIGHT_MAP, r, "[]"), Err(ContentError::Invalid(_))),
                "{r}"
            );
        }
        let bad_maps = [
            NIGHT_MAP.replace("(90.0, 50.0)])", "(190.0, 50.0)])"),
            NIGHT_MAP.replace("hp: 1.5", "hp: 0.0"),
            NIGHT_MAP.replace("ravens: [(20.0, 20.0)]", "ravens: [(-20.0, 20.0)]"),
        ];
        for m in &bad_maps {
            assert!(
                matches!(load_night(m, &night_rules(), MONSTERS), Err(ContentError::Invalid(_))),
                "{m}"
            );
        }
    }

    #[test]
    fn an_enemys_night_traits_load_and_are_checked() {
        let enemies = ENEMIES.replace(
            "(key: \"spider\",",
            "(key: \"spider\", watched: Some(90.0), tracker: true, hunter: true, road_sight: 70.0, mount: 2404,
              step_sound: Some((sound: File(\"a.wav\"), every: 1.5)),",
        );
        let c = Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: RULES,
            enemies: &enemies,
            spawns: SPAWNS,
            abilities: "[]",
            items: "[]",
            loot: "()",
            monsters: "[]",
        })
        .unwrap();
        let s = &c.enemies[1];
        assert_eq!(s.watched, Some(90.0));
        assert!(s.tracker && s.hunter);
        assert_eq!((s.road_sight, s.mount), (70.0, 2404));
        assert_eq!(s.step_sound.as_ref().map(|st| st.heard), Some(60.0));
        assert!(!c.enemies[0].tracker && c.enemies[0].watched.is_none());
        let bad = enemies.replace("watched: Some(90.0)", "watched: Some(0.0)");
        let r = Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: RULES,
            enemies: &bad,
            spawns: SPAWNS,
            abilities: "[]",
            items: "[]",
            loot: "()",
            monsters: "[]",
        });
        assert!(matches!(r, Err(ContentError::Invalid(_))));
    }
```

Every other `ContentSources { … }` literal (`grep -rn "ContentSources {" crates/nightfall-sim crates/benilla-app/src/nightfall`: the fixtures of `content.rs`'s `load_all` and `load_loot` and its other tests, `world.rs`, `enemies.rs`, `sight.rs`, `messages.rs`, `senses/tests.rs`, `combat/mod.rs`, `combat/tests.rs`, `raid/tests.rs`, and the client's test helpers) gains, after `loot`:

```rust
            monsters: "[]",
```

and the literals that read the shipped files (`tests/shipped_content.rs`'s `shipped`, `messages.rs`'s `a_full_partys_state_is_small`, and any client test that `include_str!`s them) gain `monsters: include_str!("<the same relative path>/content/nightfall/monsters.ron"),` beside their `loot` line.

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`struct ContentSources has no field named monsters`, `no method named zone_at`).

- [ ] **Step 3: Load and check them**

In `crates/nightfall-sim/src/content.rs`:

- Add to the `use` lines: `use crate::night::{there_and_back, MonsterDef, MonsterFile, NightRules, RoadDef, RoadFile, StepSound, ZoneDef, ZoneFile};`.
- `Rules` gains:

```rust
    /// The night (spec §7.2, §7.3, §8.1); the spec's stages when `rules.ron` leaves it out.
    #[serde(default)]
    pub night: NightRules,
```

- `EnemyDef` gains, after its last field:

```rust
    /// Mor'Ladim's way (spec §7.3): it stands while any hero looks at it, and otherwise goes
    /// after the nearest hero within this many yards, seen or not.
    #[serde(default)]
    pub watched: Option<f32>,
    /// A tracker (spec §7.1, §7.3): once suspicious of a hero, it follows their scent.
    #[serde(default)]
    pub tracker: bool,
    /// While a hunting stage lasts it goes after the loudest recent noise (spec §7.2).
    #[serde(default)]
    pub hunter: bool,
    /// It notices a hero standing on a road this many yards off at visibility 100, past the
    /// fog (the Dark Riders, spec §7.3); 0 for the usual notice range.
    #[serde(default)]
    pub road_sight: f32,
    /// The mount it rides, a `CreatureDisplayInfo` id, 0 for none: the client's look.
    #[serde(default)]
    pub mount: u32,
    /// A sound while it moves (Stitches' tread): the client's.
    #[serde(default)]
    pub step_sound: Option<StepSound>,
    /// When each of `kit`'s moves is first ready, seconds into its fight (each move's `first`,
    /// half its `every` when left out).
    #[serde(skip)]
    pub kit_first: Vec<f32>,
```

- `MoveDef` gains:

```rust
    /// Seconds into a fight before it is first ready; half `every` when left out, so nothing
    /// opens with its big move, unless the content wants it to (the banshee's wail, at once).
    #[serde(default)]
    pub first: Option<f32>,
```

- `Behaviour` gains, after its last variant:

```rust
    /// Walks a road of `map.ron` from end to end and back, resting `pause` seconds at each point
    /// (the Dark Riders, Stitches). Made a [`Behaviour::Patrol`] as the content loads.
    Road {
        road: String,
        #[serde(default)]
        pause: f32,
    },
```

- `SpawnDef` gains:

```rust
    /// The night stage (a `StageDef::key`) at which it comes, out of every hero's sight; from the
    /// start when left out.
    #[serde(default)]
    pub stage: Option<String>,
    /// `stage`, an index into the night's stages.
    #[serde(skip)]
    pub stage_at: Option<usize>,
    /// The signature monster it belongs to, an index into [`Content::monsters`].
    #[serde(skip)]
    pub monster: Option<usize>,
```

- `MapFile` gains:

```rust
    #[serde(default)]
    zones: Vec<ZoneFile>,
    #[serde(default)]
    roads: Vec<RoadFile>,
    #[serde(default)]
    ravens: Vec<(f32, f32)>,
```

and `MapDef` gains:

```rust
    /// Risk zones and outskirts, the first holding a point the one that counts (spec §8.1).
    pub zones: Vec<ZoneDef>,
    /// The roads (spec §7.3).
    pub roads: Vec<RoadDef>,
    /// Where flocks of ravens may settle (spec §7.1).
    pub flocks: Vec<V2>,
```

(any other `MapDef { … }` literal, `grep -rn "MapDef {" crates/nightfall-sim crates/benilla-app/src/nightfall`, gains `zones: Vec::new(), roads: Vec::new(), flocks: Vec::new(),`).

- Add:

```rust
impl MapDef {
    /// The zone that holds `p`: the first listed.
    pub fn zone_at(&self, p: V2) -> Option<&ZoneDef> {
        self.zones.iter().find(|z| z.area.contains(p))
    }

    /// Whether `p` stands on any road.
    pub fn on_road(&self, p: V2) -> bool {
        self.roads.iter().any(|r| r.holds(p))
    }
}

/// The night's places in `map.ron` (spec §7.3, §8.1): risk zones, roads and the ravens' flocks,
/// each checked against the bounds.
fn places_of(
    map: &MapFile,
    bounds: &Bounds,
) -> Result<(Vec<ZoneDef>, Vec<RoadDef>, Vec<V2>), String> {
    let mut zones: Vec<ZoneDef> = Vec::new();
    for z in &map.zones {
        let area = Bounds::new(z.area.iter().map(|&(x, y)| V2::new(x, y)).collect())
            .map_err(|why| format!("map.ron: zone \"{}\": {why}", z.key))?;
        if zones.iter().any(|o| o.key == z.key)
            || !(z.hp > 0.0 && z.damage > 0.0 && z.hp.is_finite() && z.damage.is_finite())
        {
            return Err(format!(
                "map.ron: zone \"{}\" is listed once, with hp and damage above 0",
                z.key
            ));
        }
        zones.push(ZoneDef {
            key: z.key.clone(),
            name: z.name.clone(),
            area,
            hp: z.hp,
            damage: z.damage,
            loot_rolls: z.loot_rolls,
        });
    }
    let mut roads: Vec<RoadDef> = Vec::new();
    for r in &map.roads {
        let points: Vec<V2> = r.points.iter().map(|&(x, y)| V2::new(x, y)).collect();
        if roads.iter().any(|o| o.key == r.key)
            || points.len() < 2
            || !(r.width > 0.0)
            || points.iter().any(|&p| !bounds.contains(p))
        {
            return Err(format!(
                "map.ron: road \"{}\" is listed once, with 2 points or more inside the bounds and a width above 0",
                r.key
            ));
        }
        roads.push(RoadDef {
            key: r.key.clone(),
            name: r.name.clone(),
            width: r.width,
            points,
        });
    }
    let flocks: Vec<V2> = map.ravens.iter().map(|&(x, y)| V2::new(x, y)).collect();
    if let Some(p) = flocks.iter().find(|p| !bounds.contains(**p)) {
        return Err(format!(
            "map.ron: a flock of ravens at ({}, {}) is outside the bounds",
            p.x, p.y
        ));
    }
    if flocks.len() > usize::from(u8::MAX) {
        return Err("map.ron: at most 255 flocks of ravens".into());
    }
    Ok((zones, roads, flocks))
}
```

- `ContentSources` gains `pub monsters: &'a str,`; `Content` gains:

```rust
    /// The signature monsters (spec §7.3); their spawn lines are in `spawns`, after spawns.ron's.
    pub monsters: Vec<MonsterDef>,
```

- In `Content::from_ron`:
  - After `let spawns: Vec<SpawnDef> = parse("spawns.ron", src.spawns)?;` add `let monster_files: Vec<MonsterFile> = parse("monsters.ron", src.monsters)?;`.
  - After the loop that checks the map's entries (the `bounds` local is made by then) add:

```rust
        // The night's places (spec §7.3, §8.1) and its numbers.
        let (zones, roads, flocks) = places_of(&map, &bounds).map_err(ContentError::Invalid)?;
        rules
            .night
            .check()
            .map_err(|why| ContentError::Invalid(format!("rules.ron: night: {why}")))?;
```

  - Immediately before the line `let inside = |(x, y): (f32, f32)| bounds.contains(V2::new(x, y));` add:

```rust
        // The signature monsters' lines go after spawns.ron's, each knowing its monster; then
        // every line's stage is looked up and every road walk made a patrol, so the checks below
        // see them all as spawns.ron's own.
        let mut spawns = spawns;
        let mut monsters: Vec<MonsterDef> = Vec::new();
        if monster_files.len() > usize::from(u8::MAX) {
            return invalid("monsters.ron: at most 255 monsters".into());
        }
        for (m, f) in monster_files.into_iter().enumerate() {
            if monsters.iter().any(|d| d.key == f.key) {
                return invalid(format!("monsters.ron: \"{}\" is listed twice", f.key));
            }
            if !(0.0..=1.0).contains(&f.odds) || f.spawns.is_empty() {
                return invalid(format!(
                    "monsters.ron: \"{}\" needs odds from 0 to 1 and a spawn",
                    f.key
                ));
            }
            let first = spawns.len();
            for mut s in f.spawns {
                s.monster = Some(m);
                spawns.push(s);
            }
            monsters.push(MonsterDef {
                key: f.key,
                name: f.name,
                odds: f.odds,
                always: f.always,
                lines: (first..spawns.len()).collect(),
            });
        }
        for s in &mut spawns {
            if let Some(key) = &s.stage {
                let Some(i) = rules.night.stages.iter().position(|st| st.key == *key) else {
                    return invalid(format!(
                        "spawns: \"{}\" comes at a stage \"{key}\" the night has not",
                        s.place
                    ));
                };
                s.stage_at = Some(i);
            }
            if let Behaviour::Road { road, pause } = &s.behaviour {
                let Some(r) = roads.iter().find(|r| r.key == *road) else {
                    return invalid(format!(
                        "spawns: \"{}\" walks a road \"{road}\" map.ron has not",
                        s.place
                    ));
                };
                s.behaviour = Behaviour::Patrol {
                    route: there_and_back(&r.points),
                    pause: *pause,
                };
            }
        }
```

  - In the spawn check's `match &s.behaviour { … }`, add the arm `Behaviour::Road { .. } => return invalid(format!("{who}: a road is made a patrol as it loads")),` (never reached: the block above made every one a patrol).
  - In the loop that checks each enemy kind (the one refusing a kind listed twice), add:

```rust
            if e.watched.is_some_and(|r| !(r > 0.0))
                || !(e.road_sight >= 0.0)
                || e.step_sound.as_ref().is_some_and(|s| !(s.every > 0.0 && s.heard > 0.0))
                || e.moves.iter().any(|m| m.first.is_some_and(|f| !(f >= 0.0)))
            {
                return invalid(format!(
                    "enemies.ron: \"{}\": watched above 0, road_sight 0 or more, a step sound's every and heard above 0, a move's first 0 or more",
                    e.key
                ));
            }
```

  - Where `from_ron` fills each enemy's `kit` from its `moves`, after that loop (the enemies are mutable there) add:

```rust
        for e in &mut enemies {
            e.kit_first = e
                .moves
                .iter()
                .map(|m| m.first.unwrap_or(m.every * 0.5))
                .collect();
        }
```

  - The `MapDef { … }` literal gains `zones, roads, flocks,`; the final `Ok(Self { … })` literal gains `monsters,`.
- Add to `impl Content`:

```rust
    /// The signature monster with this key, by index.
    pub fn monster(&self, key: &str) -> Option<usize> {
        self.monsters.iter().position(|m| m.key == key)
    }

    /// The night stage with this key, by index.
    pub fn stage(&self, key: &str) -> Option<usize> {
        self.rules.night.stages.iter().position(|s| s.key == key)
    }
```

In `crates/nightfall-sim/src/abilities.rs`, `Effect` gains, after its last variant:

```rust
    /// Every other enemy within `radius` yards that is in the fight is alerted to the target
    /// (the banshee's wail, spec §7.3). An enemy's alone: a hero's ability with it does nothing.
    Alarm { radius: f32 },
```

`Effect::on_enemy` answers `false` for it, and every other `match` on `Effect` the compiler now names as not exhaustive (a hero's `land`, the content's effect checks) takes `Effect::Alarm { .. }` into its arm that does nothing; the content's effect check refuses `Alarm` with a radius not above 0, beside the other checks there.

In `crates/nightfall-sim/src/enemies.rs`'s `spawn_all`, the `match &s.behaviour` gains the arm:

```rust
                // Never reached: the content makes every road walk a patrol as it loads.
                Behaviour::Road { .. } => None,
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test, the four new content tests and the two road tests).

- [ ] **Step 5: Write the shipped content**

`content/nightfall/monsters.ron`:

```ron
// The night's signature monsters (spec §7.3): each raid rolls which are out, each at its `odds`
// and between rules.ron's night.monsters_out of them; an `always` one comes every raid. Each
// one's `spawns` are spawns.ron lines; `stage` (a night stage's key) makes a line come when that
// stage lands, out of every hero's sight. measure: every place here, in the game with
// NIGHTFALL_TRACE_POS=1.
[
    (key: "morladim", name: "Mor'Ladim", odds: 0.5, spawns: [
        // He walks Raven Hill's cemetery, by his grave.
        (place: "Raven Hill cemetery", kind: "morladim",
         behaviour: Wander(center: (-10740.0, 330.0), radius: 30.0)),
    ]),
    (key: "trackers", name: "Nightbane trackers", odds: 0.6, spawns: [
        // They join the patrols as the howling begins (spec §7.2).
        (place: "The Brightwood road", kind: "nightbane_tracker", count: 2, stage: Some("howling"),
         behaviour: Patrol(route: [(-10280.0, -1250.0), (-10220.0, -1150.0), (-10160.0, -1050.0),
                                   (-10220.0, -1150.0)])),
        (place: "Near the entry", kind: "nightbane_tracker", stage: Some("howling"),
         behaviour: Patrol(route: [(-10035.0, -775.0), (-10035.0, -815.0), (-10070.0, -815.0),
                                   (-10070.0, -775.0)])),
    ]),
    (key: "banshee", name: "A banshee", odds: 0.5, spawns: [
        (place: "Raven Hill's crypt", kind: "wailing_banshee",
         behaviour: Sentry(at: (-10700.0, 390.0), facing: 90.0)),
    ]),
    (key: "dark_riders", name: "The Dark Riders", odds: 0.5, spawns: [
        (place: "The north road", kind: "dark_rider", count: 2,
         behaviour: Road(road: "north_road", pause: 3.0)),
    ]),
    (key: "stitches", name: "Stitches", always: true, spawns: [
        // The last stage of the clock: from Raven Hill to Darkshire and back (spec §7.2).
        (place: "The Raven Hill road", kind: "stitches", stage: Some("stitches"),
         behaviour: Road(road: "raven_hill_road", pause: 5.0)),
    ]),
]
```

In `content/nightfall/rules.ron`, before the closing `)`:

```ron
    // The night (spec §7.2, §7.3, §8.1). Each stage lands `at` seconds into the raid; its cue
    // sounds `warn` seconds before, with its line in the chat, and no timer shows anywhere. While
    // a stage lasts, the fog's distances are times `fog` and enemies' notice range times
    // `notice`; `hunt` sends the hunters toward the loudest noise of the last hunt.memory
    // seconds. A raid rolls one weather by weight (rain is the engine's own, `sound` its loop
    // kit), its exits (each not late is shut at `shut`, opens late within late_window at
    // `late`; the late ones open within cart_jitter of cart_opens_at; one may close as the hunt
    // lands at hunt_close; at least open_least open), and monsters_out of monsters.ron's.
    // Newcomers wait until no hero is within arrive_unseen yards; Mor'Ladim is looked at within
    // look_cone degrees of a hero's facing. measure: every cue's sound and the ravens' look,
    // with `cargo run -p benilla-formats --example list_chain -- <word> wav` (or `m2`).
    night: (
        stages: [
            (key: "dusk", name: "Dusk", at: 0.0),
            (key: "howling", name: "The howling", at: 480.0, warn: 20.0,
             cue: Some(File("Sound\\Creature\\Worgen\\WorgenAggro.wav")),
             line: Some("Somewhere in the woods, the worgen begin to howl.")),
            (key: "moonrise", name: "Moonrise", at: 840.0, warn: 20.0, fog: 0.75, notice: 1.2,
             cue: Some(File("Sound\\Doodad\\BellTollHuman.wav")),
             line: Some("A bell tolls in Darkshire. The moon is rising, and the mist with it.")),
            (key: "hunt", name: "The hunt", at: 1200.0, warn: 20.0, fog: 0.75, notice: 1.2, hunt: true,
             cue: Some(File("Sound\\Creature\\Worgen\\WorgenAggro.wav")),
             line: Some("The howls answer each other now. The packs are hunting.")),
            (key: "stitches", name: "Stitches", at: 1560.0, warn: 30.0, fog: 0.75, notice: 1.2, hunt: true,
             cue: Some(File("Sound\\Creature\\Abomination\\AbominationAggro.wav")),
             line: Some("Something heavy is walking the road from Raven Hill.")),
        ],
        weather: [
            (key: "clear", name: "Clear", weight: 5.0, fog_start: 10.0, fog_end: 40.0),
            (key: "mist", name: "Mist", weight: 3.0, fog_start: 4.0, fog_end: 28.0,
             fog_color: Some((0.10, 0.12, 0.13))),
            (key: "rain", name: "Rain", weight: 2.0, fog_start: 6.0, fog_end: 32.0, rain: 0.7,
             sound: 8534),
        ],
        exits: (shut: 0.15, late: 0.25, late_window: (900.0, 1320.0), cart_jitter: 120.0,
                hunt_close: 0.5, open_least: 2),
        monsters_out: (1, 3),
        arrive_unseen: 50.0,
        look_cone: 90.0,
        scent: (every: 2.0, lasts: 30.0, pick: 3.0),
        hunt: (every: 15.0, memory: 30.0, reach: 150.0, rest: 4.0),
        // measure: a crow's display and cry; a small bat stands in.
        ravens: (odds: 0.7, run_within: 8.0, fight_within: 15.0, back_after: 120.0, clear: 20.0,
                 birds: 3, display: 14562, scale: 0.3,
                 cry: Some(File("Sound\\Creature\\Bat\\BatAggro.wav"))),
    ),
```

In `content/nightfall/map.ron`, before the closing `)`:

```ron
    // The night's places (spec §7.3, §8.1). `zones`: the risk zones first (enemies placed inside
    // have their health and blows times hp and damage; containers and bodies inside roll their
    // tables loot_rolls more times), then the gentler outskirts; the first holding a point
    // counts. `roads` (6 yards wide when left out): the Dark Riders ride them and see far along
    // them, Stitches walks one, a scent trail across one is lost. `ravens`: where flocks may
    // settle. measure: every point here, in the game; road points no more than about 60 yards
    // apart, so a patrol's legs stay inside the path search's budget.
    zones: [
        (key: "raven_hill", name: "Raven Hill cemetery and its crypt", hp: 1.3, damage: 1.25,
         loot_rolls: 1,
         area: [(-10640.0, 230.0), (-10640.0, 430.0), (-10840.0, 430.0), (-10840.0, 230.0)]),
        (key: "vul_gol", name: "Vul'Gol Ogre Mound", hp: 1.3, damage: 1.25, loot_rolls: 1,
         area: [(-10980.0, -380.0), (-10980.0, -560.0), (-11160.0, -560.0), (-11160.0, -380.0)]),
        (key: "outskirts", name: "The northern outskirts", hp: 0.85, damage: 0.85,
         area: [(-9880.0, 800.0), (-9880.0, -1850.0), (-10150.0, -1850.0), (-10150.0, 800.0)]),
    ],
    roads: [
        (key: "north_road", name: "The road from the bridge",
         points: [(-9935.0, -480.0), (-10000.0, -560.0), (-10060.0, -650.0), (-10120.0, -760.0),
                  (-10200.0, -870.0), (-10300.0, -960.0), (-10420.0, -1060.0), (-10540.0, -1130.0)]),
        (key: "raven_hill_road", name: "The Raven Hill road",
         points: [(-10720.0, 330.0), (-10690.0, 180.0), (-10660.0, 20.0), (-10640.0, -140.0),
                  (-10620.0, -300.0), (-10600.0, -470.0), (-10590.0, -640.0), (-10580.0, -810.0),
                  (-10570.0, -980.0), (-10560.0, -1130.0)]),
        (key: "west_road", name: "The Westfall road",
         points: [(-10650.0, 760.0), (-10680.0, 600.0), (-10700.0, 450.0), (-10720.0, 330.0)]),
        (key: "east_road", name: "The Deadwind road",
         points: [(-10560.0, -1150.0), (-10650.0, -1330.0), (-10760.0, -1520.0),
                  (-10850.0, -1680.0), (-10900.0, -1800.0)]),
    ],
    ravens: [(-10030.0, -770.0), (-10075.0, -820.0), (-10735.0, 360.0), (-10300.0, -1230.0),
             (-10990.0, -470.0)],
```

In `content/nightfall/abilities.ron`, before the closing `]`:

```ron

    // --- The night's monsters (spec §7.3) ---
    // The banshee's wail: Psychic Scream's spell for its look and sound (measure: that it shows).
    (key: "banshee_wail", name: "Banshee Wail", spell: 8122, aim: Enemy, range: 40.0, cast: 2.0,
     sound: Loud, facing: Front, effects: [Alarm(radius: 60.0)], tags: [Control]),
    (key: "stitches_cleave", name: "Cleave", spell: 845, aim: Enemy, range: 7.0, sound: Loud,
     telegraph: Some((shape: Cone(length: 7.0, angle: 100.0), windup: 1.2)),
     effects: [Damage(min: 380.0, max: 460.0)], tags: [Nuke]),
    (key: "stitches_smash", name: "Smash", spell: 45, aim: Me, radius: 9.0, sound: Loud,
     telegraph: Some((shape: Circle(radius: 9.0), windup: 1.5, on_self: true)),
     effects: [Damage(min: 600.0, max: 750.0)], tags: [Nuke]),
```

In `content/nightfall/enemies.ron`: the `nightbane_worgen` and `fenros` lines each gain `hunter: true,` after their `run_speed`; and before the closing `]`:

```ron
    // The night's signature monsters (spec §7.3; monsters.ron says where and when). measure:
    // every display here is a stand-in from Duskwood's own creatures until the real one is
    // checked in the data (list_chain <name> m2, then its CreatureDisplayInfo id); the riders'
    // horse (`mount`) waits for that too.
    (key: "morladim", name: "Mor'Ladim", display: 200, scale: 1.5, level: 35, hp: 4200.0,
     walk_speed: 2.5, run_speed: 6.5, creature: Undead, elite: true, watched: Some(90.0),
     swing: Some((min: 150.0, max: 190.0, every: 2.0))),
    (key: "nightbane_tracker", name: "Nightbane Tracker", display: 736, level: 29, hp: 980.0,
     walk_speed: 4.5, run_speed: 8.5, tracker: true, hunter: true,
     swing: Some((min: 40.0, max: 55.0, every: 2.0))),
    (key: "wailing_banshee", name: "Wailing Banshee", display: 203, scale: 1.1, level: 30,
     hp: 700.0, walk_speed: 2.2, run_speed: 6.0, creature: Undead,
     swing: Some((min: 20.0, max: 28.0, every: 2.0)),
     moves: [(ability: "banshee_wail", every: 40.0, first: Some(0.0))]),
    (key: "dark_rider", name: "Dark Rider", display: 200, scale: 1.1, level: 31, hp: 1400.0,
     walk_speed: 10.0, run_speed: 12.0, creature: Undead, road_sight: 70.0,
     swing: Some((min: 60.0, max: 80.0, every: 2.0))),
    (key: "stitches", name: "Stitches", display: 559, scale: 2.4, level: 35, hp: 20000.0,
     walk_speed: 2.2, run_speed: 5.5, creature: Undead, elite: true, reach: 4.0,
     swing: Some((min: 250.0, max: 320.0, every: 2.5)),
     moves: [(ability: "stitches_cleave", every: 7.0), (ability: "stitches_smash", every: 13.0)],
     step_sound: Some((sound: File("Sound\\Creature\\Abomination\\AbominationFootstepA.wav"),
                       every: 1.4, heard: 60.0))),
```

In `content/nightfall/spawns.ron`, before the closing `]`:

```ron
    // The howling (spec §7.2) brings more wanderers. measure: Brightwood Grove, the Rotting
    // Orchard.
    (place: "Brightwood Grove: the howling", kind: "nightbane_worgen", count: 3,
     stage: Some("howling"), behaviour: Wander(center: (-10280.0, -1250.0), radius: 30.0)),
    (place: "Near the entry: the howling", kind: "nightbane_worgen", count: 2,
     stage: Some("howling"), behaviour: Wander(center: (-10090.0, -760.0), radius: 20.0)),
```

In `content/nightfall/loot.ron`'s `bodies`, before its closing `]`:

```ron
        (enemy: "morladim", table: "elite"),
        (enemy: "stitches", table: "elite"),
        (enemy: "dark_rider", table: "undead"),
        (enemy: "wailing_banshee", table: "caster"),
```

In `content/nightfall/contracts.ron`, replace the commented Stitches block (the two comment lines about sub-project 6 and the five commented lines of the contract) with:

```ron
    // Stitches walks the road once the night's clock reaches its last stage.
    (key: "stitches", giver: "ebonlock", name: "Stitches",
     text: "It walks from Raven Hill to Darkshire. Stop it.",
     objectives: [Kill(enemy: "stitches", count: 1)],
     reward: (gold: 500)),
```

In `crates/nightfall-sim/tests/shipped_content.rs` add:

```rust
#[test]
fn the_night_is_in_the_shipped_content() {
    let c = shipped();
    let n = &c.rules.night;
    assert_eq!(n.stages.len(), 5);
    assert!(n.stages[1..].iter().all(|s| s.cue.is_some() && s.line.is_some()));
    assert!(n.weather.len() >= 3);
    for key in ["morladim", "trackers", "banshee", "dark_riders", "stitches"] {
        assert!(c.monster(key).is_some(), "{key}");
    }
    assert!(c.monsters[c.monster("stitches").unwrap()].always);
    assert!(!c.map.roads.is_empty() && !c.map.zones.is_empty() && !c.map.flocks.is_empty());
    assert!(c.enemies[c.enemy("stitches").unwrap()].elite);
    assert!(c.enemies[c.enemy("morladim").unwrap()].watched.is_some());
    assert!(
        c.spawns.iter().any(|s| s.stage_at.is_some() && s.monster.is_none()),
        "the howling brings more wanderers"
    );
}
```

- [ ] **Step 6: Find the cue sounds in the data**

The sound paths above are names the 1.12 client is likely to have; check each in the trimmed data and change the content to one that is there:

```bash
export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
for w in worgen bell abomination bat crow raven banshee; do echo "== $w"; WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo run -q -p benilla-formats --example list_chain -- $w wav; done > target/nf-night-sounds.txt
for w in banshee crow raven abomination horse; do echo "== $w"; WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo run -q -p benilla-formats --example list_chain -- $w m2; done > target/nf-night-models.txt
```

Expected: two text files listing paths under each word (they are Blizzard data: never commit them). For each `File(…)` in `rules.ron` and `enemies.ron`, keep it when it is listed, else put in the listed path whose name fits best (an aggro or howl for the worgen, a toll for the bell, an aggro and a footstep for the abomination, a take-off or aggro for the ravens); a word with nothing listed keeps its line and gains `// measure: not in the trimmed data` above it. Note the model paths in the rulings' display line for the later check; the display ids stay the stand-ins.

- [ ] **Step 7: Keep the client reading every file**

In `crates/benilla-app/src/nightfall/mod.rs`: `CONTENT_TEXTS` and `content_texts()` hold `[String; 12]`, `monsters` last (after the three inn files); in `prepare`, after the `loot` read, add:

```rust
    let monsters = read(
        "monsters.ron",
        include_str!("../../../../content/nightfall/monsters.ron"),
    )?;
```

the `ContentSources { … }` literal gains `monsters: &monsters,`, and the `CONTENT_TEXTS.set([…])` array gains `monsters` at its end (after `stations`). The content hash covers it: a friend with other monsters is refused.

- [ ] **Step 8: Run every test and check the client**

Run: `cargo test -p nightfall-sim`
Expected: PASS, `the_night_is_in_the_shipped_content` and `the_shipped_inn_loads_against_the_shipped_raid` (the Stitches contract now names an enemy that exists) among them.
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 9: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim content/nightfall crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall content: the night's stages, weathers and roll, risk zones, roads, ravens, signature monsters

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The height map knows water; the bake marks the ADT's liquids

**Files:**
- Modify: `crates/nightfall-sim/src/heightmap/mod.rs` (`HeightMap::water`, `is_water`, `water_columns`), `crates/nightfall-sim/src/heightmap/build.rs` (`wet`, `ground_of`), `crates/nightfall-sim/src/heightmap/codec.rs` (version 2), `crates/nightfall-sim/src/heightmap/testmap.rs` (`pond`)
- Modify: `crates/nightfall-bake/src/bake.rs` (`add_water`, `add_tile_water`), `crates/nightfall-bake/src/cache.rs` (`BAKE_VERSION`), `crates/nightfall-bake/examples/bake_duskwood.rs`

**Interfaces:**
- Consumes: `HeightMap`, `HeightMapBuilder::{new, flat, columns_in, finish}`, `Grid`, the codec's `Reader` (world knowledge Tasks 1, 4); `bake`, `add_terrain`, `TileMesh`, `ChunkMesh::liquids`, `benilla_formats::{LiquidMesh, LiquidKind}` (world knowledge Task 7, `benilla-formats`).
- Produces:
  - `HeightMap::{is_water(&self, V2) -> bool, water_columns(&self) -> usize}`; `HeightMapBuilder::{wet(&mut self, usize), ground_of(&self, usize) -> Option<f32>}`.
  - The codec's `VERSION` 2 (water bits after the blocked bits); `BAKE_VERSION` 2, so every cache bakes again once.
  - Test-only `heightmap::testmap::pond(&mut HeightMapBuilder, V2, V2)`.
  - Crate-only in `nightfall-bake`: `WADE: f32 = 0.3`, `add_water(&mut HeightMapBuilder, &LiquidMesh)`, `add_tile_water(&mut HeightMapBuilder, &TileMesh)`.

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/heightmap/mod.rs`'s tests add:

```rust
    #[test]
    fn a_pond_is_water_and_the_bank_is_not() {
        let mut b = super::testmap::flat(20.0);
        super::testmap::pond(&mut b, V2::new(5.0, 5.0), V2::new(8.0, 8.0));
        let m = b.finish();
        assert!(m.is_water(V2::new(6.5, 6.5)));
        assert!(!m.is_water(V2::new(9.5, 6.5)));
        assert!(!m.is_water(V2::new(-5.0, 6.5)));
        assert!(m.water_columns() >= 36);
    }
```

In `crates/nightfall-sim/src/heightmap/codec.rs`'s tests add:

```rust
    #[test]
    fn water_comes_back_from_bytes_and_a_version_1_file_is_refused() {
        let mut b = flat(50.0);
        super::super::testmap::pond(&mut b, V2::new(10.0, 10.0), V2::new(14.0, 12.0));
        let m = b.finish();
        let back = HeightMap::from_bytes(&m.to_bytes()).unwrap();
        assert!(back.is_water(V2::new(12.0, 11.0)));
        assert_eq!(back.water_columns(), m.water_columns());
        let mut old = m.to_bytes();
        old[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            HeightMap::from_bytes(&old).unwrap_err(),
            "height map version 1, not 2"
        );
    }
```

In `crates/nightfall-bake/src/bake.rs`'s tests (the data-free ones) add:

```rust
    use benilla_formats::{LiquidKind, LiquidMesh};

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
```

Run: `cargo test -p nightfall-sim -p nightfall-bake`
Expected: FAIL to compile (`cannot find function pond`, `no method named is_water`).

- [ ] **Step 2: Water columns in the map and the builder**

In `crates/nightfall-sim/src/heightmap/mod.rs`, `HeightMap` gains, after `blocked`:

```rust
    /// One bit a column: water deeper than a wade (spec §7.3: a scent trail breaks there).
    water: Vec<u64>,
```

and `impl HeightMap` gains:

```rust
    /// Whether the column holding `p` is water (spec §7.3); off the map, it is not.
    pub fn is_water(&self, p: V2) -> bool {
        self.grid.column_of(p).is_some_and(|(x, y)| {
            let c = self.grid.index(x, y);
            self.water.get(c / 64).is_some_and(|w| (w >> (c % 64)) & 1 == 1)
        })
    }

    /// How many columns are water, for the bake's report.
    pub fn water_columns(&self) -> usize {
        self.water.iter().map(|w| w.count_ones() as usize).sum()
    }
```

In `crates/nightfall-sim/src/heightmap/build.rs`, `HeightMapBuilder` gains `water: Vec<u64>,` after `blocked`; its `new` literal gains `water` made the same way as `blocked` (one `u64` per 64 columns, all zero); `finish`'s `HeightMap { … }` literal gains `water: self.water,`; and `impl HeightMapBuilder` gains:

```rust
    /// Column `col` is water (an ADT liquid deep enough, spec §7.3).
    pub fn wet(&mut self, col: usize) {
        if let Some(w) = self.water.get_mut(col / 64) {
            *w |= 1 << (col % 64);
        }
    }

    /// Column `col`'s terrain height, when it has one.
    pub fn ground_of(&self, col: usize) -> Option<f32> {
        self.ground.get(col).copied().filter(|z| z.is_finite())
    }
```

In `crates/nightfall-sim/src/heightmap/testmap.rs` add:

```rust
/// Water over the columns from `min` to `max`: a pond a scent trail cannot cross.
pub(crate) fn pond(b: &mut HeightMapBuilder, min: V2, max: V2) {
    for c in b.columns_in(min, max) {
        b.wet(c);
    }
}
```

In `crates/nightfall-sim/src/heightmap/codec.rs`:

- The module doc's first sentence becomes `The height map as bytes, for the bake's cache: a header, the terrain heights, the blocked and water bits and the span lists of the columns that have them, all little-endian.`
- `const VERSION: u32 = 1;` becomes `const VERSION: u32 = 2;` (its doc unchanged).
- In `to_bytes`, the capacity adds `+ self.water.len() * 8`, and after the loop writing `self.blocked` add:

```rust
        for w in &self.water {
            out.extend_from_slice(&w.to_le_bytes());
        }
```

- In `from_bytes`, after `let blocked = r.vec(…)?;` add:

```rust
        let water = r.vec(n.div_ceil(64), 8, |b| {
            u64::from_le_bytes(b.try_into().expect("eight bytes"))
        })?;
```

and the `Ok(HeightMap { … })` literal gains `water,`.

- [ ] **Step 3: The bake marks liquids**

In `crates/nightfall-bake/src/bake.rs`:

- The `benilla_formats` import gains `LiquidMesh`.
- The module doc gains a sentence after its first: `Every chunk's liquid deeper than a wade marks its columns as water, where a scent trail breaks (spec §7.3).`
- Add, after `add_terrain`:

```rust
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
            if !liquid.wet.get(cy * (cols - 1) + cx).copied().unwrap_or(false) {
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
```

- In `bake`, `for t in &tiles { add_terrain(&mut b, t); }` becomes:

```rust
    for t in &tiles {
        add_terrain(&mut b, t);
        add_tile_water(&mut b, t);
    }
```

In `crates/nightfall-bake/src/cache.rs`, `pub const BAKE_VERSION: u32 = 1;` becomes `pub const BAKE_VERSION: u32 = 2;` (every cache bakes once more, now with water).

In `crates/nightfall-bake/examples/bake_duskwood.rs`, after the line that prints how long the bake took, print the water too: `println!("water columns: {}", <the baked map>.water_columns());`, naming the example's own binding for the map.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim -p nightfall-bake`
Expected: PASS (the three new tests; every earlier codec and bake test; the Darkshire bake test skips without `WOW_DATA`).
Run: `WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo test -p nightfall-bake --test duskwood`
Expected: PASS (it bakes again: the version changed).
Run: `WOW_DATA=D:/Games/azeroth-together/WoW/Data cargo run --release -p nightfall-bake --example bake_duskwood`
Expected: the bake's line as before, then `water columns: <n>` with `n` above 0 (the river under the north bridge is inside the bounds).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/heightmap crates/nightfall-bake
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall's height map knows water: the bake marks the ADT's liquids deeper than a wade

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The raid's roll: exits open, late and closing; the weather; the monsters out

**Files:**
- Create: `crates/nightfall-sim/src/night/roll.rs`, `crates/nightfall-sim/src/night/tests.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs`, `crates/nightfall-sim/src/raid/mod.rs` (`RaidRoll`, `RaidRoll::{standard, exit_open}`, `loot_view`, `begin_raid`), `crates/nightfall-sim/src/raid/extract.rs` (`exit_at`)
- Modify (literals): every `RaidRoll { … }` literal (`grep -rn "RaidRoll {" crates/nightfall-sim crates/benilla-app/src/nightfall`: the run loop's `raid/tests.rs`, the inn's `session.rs` and `board.rs` tests) gains `..Default::default()` before its `}`

**Interfaces:**
- Consumes: `RaidRoll { seed, entry, exits }`, `ExitPlan { open, opens_at }`, `ExitDef::late`, `RaidRoll::standard`, `World::{begin_raid, loot_view}`, `ExitMark`, `exit_at`, `RunRules::cart_opens_at` (run loop Tasks 1, 3, 6); `crate::combat::Dice::{new, unit, between, chance}` (combat Task 2); `NightRules`, `MonsterDef` (Task 1).
- Produces:
  - `RaidRoll` gains `weather: u8`, `monsters: Vec<u8>` (in order, indices into `Content::monsters`) and `closes: Option<(u8, f32)>` (an exit and the hunt's time).
  - `RaidRoll::standard` puts out the `always` monsters, weather 0, nothing closing.
  - `RaidRoll::roll(content: &Content, seed: u64) -> RaidRoll`; `RaidRoll::exit_open(&self, exit: usize, now: f32) -> bool`.
  - `begin_raid` refuses a roll naming a monster or weather the content has not.
  - Test-only `night::tests::{rules, content, standard, open, world_on, world, sentry, put, join, steps, secs, events_of}` and the fixtures `CLASSES`, `MAP`, `ENEMIES`, `ABILITIES`, `ITEMS`, `LOOT`.

- [ ] **Step 1: Write the test kit and the failing tests**

`crates/nightfall-sim/src/night/tests.rs`:

```rust
//! The night in whole worlds on small hand-made maps: the roll, the clock, newcomers and zones,
//! the hunt, scent, Mor'Ladim and the banshee, the riders and Stitches, the ravens.

use std::sync::Arc;

use crate::heightmap::testmap::{flat, pond};
use crate::heightmap::HeightMap;
use crate::{
    Content, ContentSources, Event, Gait, HeroPose, PlayerId, PlayerInput, RaidRoll, World, V2,
    V3, TICK_HZ,
};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0),
]"#;
/// 120 yards square: a road across it along y = 60, a risk zone in the far corner, two flocks
/// of ravens, three exits (the last one late).
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (120.0, 0.0), (120.0, 120.0), (0.0, 120.0)],
    entries: [(5.0, 5.0, 0.0)],
    exits: [(key: "north", name: "The North Gate", at: (115.0, 5.0)),
            (key: "west", name: "The West Gate", at: (5.0, 115.0)),
            (key: "cart", name: "The Cart", at: (115.0, 115.0), late: true)],
    zones: [(key: "hill", name: "The Hill", hp: 1.5, damage: 1.25, loot_rolls: 1,
             area: [(90.0, 90.0), (120.0, 90.0), (120.0, 120.0), (90.0, 120.0)])],
    roads: [(key: "road", name: "The Road", points: [(1.0, 60.0), (60.0, 60.0), (119.0, 60.0)])],
    ravens: [(30.0, 30.0), (90.0, 30.0)])"#;
const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 1000.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 40.0, max: 40.0, every: 2.0))),
    (key: "worgen", name: "Worgen", display: 657, level: 27, hp: 900.0, walk_speed: 2.5,
     run_speed: 8.0, hunter: true),
    (key: "tracker", name: "Tracker", display: 736, level: 29, hp: 900.0, walk_speed: 4.5,
     run_speed: 8.5, tracker: true),
    (key: "morladim", name: "Mor'Ladim", display: 200, level: 35, hp: 4000.0, walk_speed: 2.5,
     run_speed: 6.5, watched: Some(90.0), swing: Some((min: 100.0, max: 100.0, every: 2.0))),
    (key: "banshee", name: "Banshee", display: 203, level: 30, hp: 700.0, walk_speed: 2.2,
     run_speed: 6.0, moves: [(ability: "wail", every: 40.0, first: Some(0.0))]),
    (key: "rider", name: "Rider", display: 200, level: 31, hp: 1400.0, walk_speed: 10.0,
     run_speed: 12.0, road_sight: 70.0),
    (key: "stitches", name: "Stitches", display: 559, level: 35, hp: 20000.0, walk_speed: 2.2,
     run_speed: 5.5, elite: true, reach: 4.0,
     swing: Some((min: 250.0, max: 250.0, every: 2.5)), moves: [(ability: "smash", every: 6.0)]),
]"#;
const ABILITIES: &str = r#"[
    (key: "wail", name: "Wail", spell: 8122, aim: Enemy, range: 40.0, cast: 2.0, sound: Loud,
     effects: [Alarm(radius: 60.0)], tags: [Control]),
    (key: "smash", name: "Smash", spell: 45, aim: Me, radius: 8.0, sound: Loud,
     telegraph: Some((shape: Circle(radius: 8.0), windup: 1.5, on_self: true)),
     effects: [Damage(min: 500.0, max: 500.0)], tags: [Nuke]),
]"#;
const ITEMS: &str = r#"[
    (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10, value: 40),
]"#;
/// One chest outside the risk zone and one inside, each holding one roll of one gem.
const LOOT: &str = r#"(
    kinds: [(key: "chest", name: "Chest", display: 259, search: 2.0, sound: Open, slots: 8)],
    tables: [(key: "one", rolls: (1, 1), items: [(item: "gem", weight: 1.0)])],
    containers: [(kind: "chest", table: "one", at: (10.0, 10.0)),
                 (kind: "chest", table: "one", at: (100.0, 100.0))],
    bodies: [],
)"#;

/// The tests' rules with `night` inside `night: ( … )` (the spec's stages when empty).
pub(super) fn rules(night: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 200.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0),
        night: ({night}))"#
    )
}

/// The tests' content: `spawns` and `monsters` are RON list items, each ending in a comma.
pub(super) fn content(night: &str, spawns: &str, monsters: &str) -> Content {
    Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(night),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: &format!("[{monsters}]"),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// The roll every raid of the earlier plans had.
pub(super) fn standard(c: &Content) -> RaidRoll {
    RaidRoll::standard(c, 1)
}

/// Flat ground 120 yards across.
pub(super) fn open() -> HeightMap {
    flat(120.0).finish()
}

/// A world on `map` with this content, its raid begun from `roll`, then its map attached.
pub(super) fn world_on(
    map: HeightMap,
    night: &str,
    spawns: &str,
    monsters: &str,
    roll: impl FnOnce(&Content) -> RaidRoll,
) -> World {
    let c = content(night, spawns, monsters);
    let r = roll(&c);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(r).unwrap();
    w.attach_map(Arc::new(map));
    w
}

/// [`world_on`] open ground, no monsters, the standard roll.
pub(super) fn world(night: &str, spawns: &str) -> World {
    world_on(open(), night, spawns, "", standard)
}

/// A sentry spawn line of `kind` at `(x, y)` facing `facing` degrees (0 is +x).
pub(super) fn sentry(kind: &str, x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "{kind}", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// `player`'s next input: their hero at `(x, y)` facing `facing`, moving at `gait`.
pub(super) fn put(w: &mut World, player: PlayerId, x: f32, y: f32, facing: V2, gait: Gait) {
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing,
                z: 0.0,
            }),
            gait,
            ..Default::default()
        },
    );
}

/// `player`'s rogue standing at `(x, y)`, facing +x, its first input taken.
pub(super) fn join(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
    put(w, player, x, y, V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

/// Steps in `s` seconds.
pub(super) fn secs(s: f32) -> usize {
    (s * TICK_HZ as f32).round() as usize
}

/// The events since the last drain that `pick` keeps.
pub(super) fn events_of<T>(w: &mut World, pick: impl Fn(&Event) -> Option<T>) -> Vec<T> {
    w.drain_events().iter().filter_map(pick).collect()
}

const TWO_MONSTERS: &str = r#"
    (key: "ghost", name: "Ghost", odds: 0.5, spawns: [
        (place: "Ghost", kind: "brute", behaviour: Sentry(at: (50.0, 50.0), facing: 0.0))]),
    (key: "wraith", name: "Wraith", odds: 0.5, spawns: [
        (place: "Wraith", kind: "brute", behaviour: Sentry(at: (70.0, 50.0), facing: 0.0))]),
    (key: "walker", name: "Walker", always: true, spawns: [
        (place: "Walker", kind: "stitches", stage: Some("stitches"), behaviour: Road(road: "road"))]),
"#;

#[test]
fn the_same_seed_rolls_the_same_raid_and_seeds_differ() {
    let c = content(
        r#"weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
                     (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0)]"#,
        "",
        TWO_MONSTERS,
    );
    assert_eq!(RaidRoll::roll(&c, 42), RaidRoll::roll(&c, 42));
    let rolls: Vec<RaidRoll> = (0..40).map(|s| RaidRoll::roll(&c, s)).collect();
    assert!(rolls.iter().any(|r| r.weather != rolls[0].weather));
    assert!(rolls.iter().any(|r| r.monsters != rolls[0].monsters));
    assert!(rolls.iter().any(|r| r.exits != rolls[0].exits));
    // The loot's seed is the raid's own.
    assert!(rolls.iter().enumerate().all(|(s, r)| r.seed == s as u64));
}

#[test]
fn a_thousand_raids_each_have_a_way_out() {
    let c = content("exits: (shut: 0.9, late: 0.9, open_least: 2)", "", "");
    for seed in 0..1000 {
        let r = RaidRoll::roll(&c, seed);
        assert_eq!(r.exits.len(), 3);
        assert!(
            r.exits.iter().any(|p| p.open && p.opens_at <= 0.0),
            "seed {seed}: no way out from the start: {:?}",
            r.exits
        );
        assert!(r.exits.iter().filter(|p| p.open).count() >= 2, "seed {seed}");
        // The cart opens within its jitter of cart_opens_at (1080 s by default).
        assert!((960.0..=1200.0).contains(&r.exits[2].opens_at), "seed {seed}");
        for p in &r.exits[..2] {
            assert!(!p.open || p.opens_at <= 0.0 || (900.0..=1320.0).contains(&p.opens_at));
        }
    }
}

#[test]
fn the_hunt_never_closes_the_last_way_out() {
    let c = content("exits: (shut: 0.6, late: 0.0, hunt_close: 1.0, open_least: 1)", "", "");
    let hunt = c.rules.night.stages[3].at;
    let mut closed = 0;
    for seed in 0..1000 {
        let r = RaidRoll::roll(&c, seed);
        let Some((x, at)) = r.closes else { continue };
        closed += 1;
        assert_eq!(at, hunt);
        assert!(!c.map.exits[usize::from(x)].late, "seed {seed}: the cart never closes");
        assert!(r.exit_open(usize::from(x), at - 1.0) && !r.exit_open(usize::from(x), at));
        let after = (0..3).filter(|&i| i != usize::from(x) && r.exits[i].open).count();
        assert!(after >= 1, "seed {seed}: {r:?} closed its last way out");
    }
    assert!(closed > 100, "the hunt closes an exit when it can: {closed}");
}

#[test]
fn monsters_out_stay_within_the_rules_and_the_always_ones_come() {
    let c = content("monsters_out: (1, 1)", "", TWO_MONSTERS);
    for seed in 0..300 {
        let r = RaidRoll::roll(&c, seed);
        assert!(r.monsters.contains(&2), "seed {seed}: the walker always comes");
        assert_eq!(r.monsters.len(), 2, "seed {seed}: exactly one of the other two");
        assert!(r.monsters.windows(2).all(|w| w[0] < w[1]));
    }
    let both = (0..300).any(|s| RaidRoll::roll(&c, s).monsters.contains(&0))
        && (0..300).any(|s| RaidRoll::roll(&c, s).monsters.contains(&1));
    assert!(both);
    assert_eq!(RaidRoll::standard(&c, 7).monsters, vec![2]);
}

#[test]
fn the_weather_follows_its_weights() {
    let c = content(
        r#"weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
                     (key: "never", name: "Never", weight: 0.0, fog_start: 4.0, fog_end: 28.0),
                     (key: "rain", name: "Rain", weight: 3.0, fog_start: 6.0, fog_end: 32.0, rain: 0.7)]"#,
        "",
        "",
    );
    let counts = (0..1000).fold([0; 3], |mut n, s| {
        n[usize::from(RaidRoll::roll(&c, s).weather)] += 1;
        n
    });
    assert_eq!(counts[1], 0);
    assert!(counts[2] > counts[0] * 2, "{counts:?}");
    assert_eq!(RaidRoll::roll(&content("", "", ""), 3).weather, 0);
}

#[test]
fn a_closed_exit_takes_nobody_out_and_a_roll_must_fit_its_content() {
    let mut w = world_on(open(), "", "", TWO_MONSTERS, |c| RaidRoll {
        closes: Some((0, 1.0)),
        ..standard(c)
    });
    join(&mut w, 0, 115.0, 5.0);
    steps(&mut w, secs(12.0));
    assert!(w.hero(0).is_some(), "the north gate closed before the count ended");
    assert!(!w.loot_view(Some(0)).exits[0].open);
    assert_eq!(w.loot_view(Some(0)).exits[0].opens_in, None);
    let c = content("", "", TWO_MONSTERS);
    let mut w = World::new(Arc::new(c.clone()));
    assert!(w
        .begin_raid(RaidRoll {
            monsters: vec![3],
            ..standard(&c)
        })
        .is_err());
    assert!(w
        .begin_raid(RaidRoll {
            weather: 1,
            ..standard(&c)
        })
        .is_err());
}
```

The imports `pond` and `V3` are used by later tasks' tests: until Task 7 uses the last of them, put `#[allow(unused_imports)]` on the `use crate::heightmap::testmap::{flat, pond};` line and on the `use crate::{…}` line, and take both off in Task 7.

In `crates/nightfall-sim/src/night/mod.rs` add `mod roll;` below `mod rules;` and:

```rust
#[cfg(test)]
mod tests;
```

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL to compile (`no function or associated item named roll`, `struct RaidRoll has no field named closes`).

- [ ] **Step 2: The roll's fields and the open test**

In `crates/nightfall-sim/src/raid/mod.rs`:

- `RaidRoll`'s doc becomes `What makes this raid this raid (spec §8.1): the seed its loot is rolled from, the entry, which exits open when and which one closes as the hunt lands, the weather, and which signature monsters are out. [`RaidRoll::roll`] rolls them; [`RaidRoll::standard`] is every exit open, the first weather, and only the monsters that always come.` and it gains, after `exits`:

```rust
    /// The weather, an index into the night's weathers (0 when there are none).
    pub weather: u8,
    /// The signature monsters out, indices into `Content::monsters`, in order.
    pub monsters: Vec<u8>,
    /// The exit that closes as the hunt lands, and when (spec §7.2: "one exit may close").
    pub closes: Option<(u8, f32)>,
```

- In `RaidRoll::standard`'s literal add:

```rust
            weather: 0,
            monsters: content
                .monsters
                .iter()
                .enumerate()
                .filter(|(_, m)| m.always)
                .map(|(i, _)| i as u8)
                .collect(),
            closes: None,
```

- Add to `impl RaidRoll`:

```rust
    /// Whether exit `exit` is open `now` seconds into the raid: opened, and not closed by the hunt.
    pub fn exit_open(&self, exit: usize, now: f32) -> bool {
        self.exits
            .get(exit)
            .is_some_and(|p| p.open && now >= p.opens_at)
            && self
                .closes
                .is_none_or(|(x, at)| usize::from(x) != exit || now < at)
    }
```

- In `loot_view`, the `ExitMark { … }` the exits' map builds becomes:

```rust
                let closed = self
                    .raid
                    .roll
                    .closes
                    .is_some_and(|(x, at)| usize::from(x) == i && self.elapsed >= at);
                ExitMark {
                    exit: i as u8,
                    open: self.raid.roll.exit_open(i, self.elapsed),
                    opens_in: (plan.open && !closed)
                        .then(|| (plan.opens_at - self.elapsed).max(0.0)),
                }
```

- In `begin_raid`, after the check of the exits' count, add:

```rust
        if let Some(m) = roll
            .monsters
            .iter()
            .find(|&&m| usize::from(m) >= content.monsters.len())
        {
            return Err(format!("no monster {m} in monsters.ron"));
        }
        if usize::from(roll.weather) >= content.rules.night.weather.len().max(1) {
            return Err(format!("no weather {} in rules.ron", roll.weather));
        }
```

In `crates/nightfall-sim/src/raid/extract.rs`, `exit_at`'s condition `roll.exits.get(*i).is_some_and(|p| p.open && now >= p.opens_at) && pos.distance(e.at) <= e.radius` becomes `roll.exit_open(*i, now) && pos.distance(e.at) <= e.radius`, and its doc's first sentence `The exit (an index into the map's exits) whose ring `pos` stands in while it is open now (opened, and not closed by the hunt).`

Every other `RaidRoll { … }` literal (see Files) gains `..Default::default()`.

- [ ] **Step 3: The roll**

`crates/nightfall-sim/src/night/roll.rs`:

```rust
//! Every raid a little different (spec §8.1): from the raid's seed, which exits open and when,
//! which one may close as the hunt lands (§7.2), the weather, and which signature monsters are
//! out (§7.3). The map stays the same, so knowing it pays; the same seed rolls the same raid.

use crate::combat::Dice;
use crate::{Content, ExitDef, ExitPlan, RaidRoll};

/// Mixed into the raid's seed, so the roll's dice never follow the loot's.
const ROLL_SEED: u64 = 0x6E16_47F0_11ED_0005;

impl RaidRoll {
    /// This raid as the dice fall for `seed`. The late exits (Darkshire's cart) open within
    /// `cart_jitter` of `cart_opens_at`; every other one is shut at `shut`, else late (within
    /// `late_window`) at `late`, else open from the start; whatever falls, one exit is open from
    /// the start and `open_least` at some time. At `hunt_close`, one exit open by the hunt that is
    /// not late closes as it lands, never the last. The weather by its weights; the monsters
    /// each at its odds, between `monsters_out` of them, and every `always` one.
    pub fn roll(content: &Content, seed: u64) -> Self {
        let x = &content.rules.night.exits;
        let cart = content.rules.run.cart_opens_at;
        let mut dice = Dice::new(seed ^ ROLL_SEED);
        let mut roll = Self::standard(content, seed);
        for (plan, def) in roll.exits.iter_mut().zip(&content.map.exits) {
            *plan = if def.late {
                ExitPlan {
                    open: true,
                    opens_at: (cart + dice.between(-x.cart_jitter, x.cart_jitter)).max(0.0),
                }
            } else if dice.chance(x.shut) {
                ExitPlan {
                    open: false,
                    opens_at: 0.0,
                }
            } else if dice.chance(x.late) {
                ExitPlan {
                    open: true,
                    opens_at: dice.between(x.late_window.0, x.late_window.1),
                }
            } else {
                ExitPlan {
                    open: true,
                    opens_at: 0.0,
                }
            };
        }
        keep_enough_open(&mut roll.exits, &content.map.exits, usize::from(x.open_least));
        roll.closes = hunt_closing(&roll.exits, content, &mut dice);
        roll.weather = pick_weather(content, &mut dice);
        roll.monsters = pick_monsters(content, &mut dice);
        roll
    }
}

/// One exit open from the start (the first that is not late, when every such one fell shut or
/// late), and shut ones opened in order until `least` open at some time.
fn keep_enough_open(plans: &mut [ExitPlan], defs: &[ExitDef], least: usize) {
    let open_now = ExitPlan {
        open: true,
        opens_at: 0.0,
    };
    if !plans.iter().any(|p| p.open && p.opens_at <= 0.0) {
        if let Some(i) = defs.iter().position(|d| !d.late) {
            plans[i] = open_now;
        }
    }
    let least = least.min(plans.len());
    let mut open = plans.iter().filter(|p| p.open).count();
    for p in plans.iter_mut() {
        if open >= least {
            break;
        }
        if !p.open {
            *p = open_now;
            open += 1;
        }
    }
}

/// The exit the hunt closes as it lands, if the dice say one does: one open by then and not
/// late, while another is open at some time, so it is never the last way out.
fn hunt_closing(plans: &[ExitPlan], content: &Content, dice: &mut Dice) -> Option<(u8, f32)> {
    let n = &content.rules.night;
    let at = n.stages.iter().find(|s| s.hunt)?.at;
    if !dice.chance(n.exits.hunt_close) {
        return None;
    }
    let open = plans.iter().filter(|p| p.open).count();
    let can: Vec<usize> = plans
        .iter()
        .zip(&content.map.exits)
        .enumerate()
        .filter(|(_, (p, d))| p.open && p.opens_at <= at && !d.late)
        .map(|(i, _)| i)
        .collect();
    if open < 2 || can.is_empty() {
        return None;
    }
    let i = can[((dice.unit() * can.len() as f32) as usize).min(can.len() - 1)];
    Some((i as u8, at))
}

/// A weather by weight; 0 when the night has none (the atmosphere's own fog).
fn pick_weather(content: &Content, dice: &mut Dice) -> u8 {
    let w = &content.rules.night.weather;
    let total: f32 = w.iter().map(|w| w.weight).sum();
    let mut x = dice.unit() * total;
    for (i, def) in w.iter().enumerate() {
        if def.weight > 0.0 && x < def.weight {
            return i as u8;
        }
        x -= def.weight;
    }
    w.iter().rposition(|d| d.weight > 0.0).unwrap_or(0) as u8
}

/// The signature monsters out: every `always` one, and of the rest each whose roll comes up at
/// its odds, topped up or cut down to `monsters_out` (the likeliest kept), in order.
fn pick_monsters(content: &Content, dice: &mut Dice) -> Vec<u8> {
    let (lo, hi) = content.rules.night.monsters_out;
    // A key under 1 comes up at its odds; the lowest keys are the likeliest, so a raid short of
    // `lo` takes the next lowest and one over `hi` drops the highest.
    let mut keyed: Vec<(f32, usize)> = content
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.always && m.odds > 0.0)
        .map(|(i, m)| (dice.unit() / m.odds, i))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let rolled = keyed.iter().filter(|(k, _)| *k < 1.0).count();
    let take = rolled.clamp(usize::from(lo).min(keyed.len()), usize::from(hi));
    let mut out: Vec<u8> = content
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| m.always)
        .map(|(i, _)| i as u8)
        .collect();
    out.extend(keyed.iter().take(take).map(|&(_, i)| i as u8));
    out.sort_unstable();
    out
}
```

`ExitDef` and `ExitPlan` are already `nightfall_sim` exports (run loop Tasks 1, 3).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the six roll tests and every earlier one; the run loop's `RaidRoll` tests unchanged with `..Default::default()`).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (the inn's session and board tests with their literals filled).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: every raid rolls its exits, the hunt's closing, the weather and the monsters out

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The night clock: cues before each stage, the stages landing, the fog and the eyes

**Files:**
- Create: `crates/nightfall-sim/src/night/clock.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs`, `crates/nightfall-sim/src/night/tests.rs`, `crates/nightfall-sim/src/combat/mod.rs` (`Event`), `crates/nightfall-sim/src/world.rs` (`World::night`, `new`, `step`, `snapshot`, `apply_snapshot`, `Snapshot::night`), `crates/nightfall-sim/src/raid/mod.rs` (`begin_raid`), `crates/nightfall-sim/src/sight.rs` (`seen_by`, `snapshot_for`), `crates/nightfall-sim/src/senses/react.rs` (`step_enemies`, `sense`), `crates/nightfall-sim/src/messages.rs` (`PROTOCOL`), `crates/nightfall-sim/src/lib.rs`
- Modify (literals): every `Snapshot { … }` literal outside `world.rs` and `sight.rs` (`grep -rn "Snapshot {" crates/nightfall-sim crates/benilla-app/src/nightfall`) gains `night: Default::default(),`
- Modify (so the client still builds): `crates/benilla-app/src/nightfall/fight.rs` (`show_events`), only if its `match` on `Event` names every variant

**Interfaces:**
- Consumes: `World::{events, elapsed, content, map, raid, step}`, `Event`, `Snapshot`, `ServerMsg`, `PROTOCOL = 6` (earlier plans); `senses::step_enemies(…, engage)`, `react::sense`, `notice_range` (senses Tasks 2, 6; combat Task 6); `NightRules`, `StageDef`, `WeatherDef` (Task 1); `RaidRoll::{weather, closes}` (Task 3).
- Produces:
  - `nightfall_sim::{NightView, Fog, fog_of, FLOCK_GONE, FLOCK_SETTLED, FLOCK_FLOWN}`; `NightView { stage: u8, weather: u8, flocks: Vec<u8> }`; `Fog { start: f32, end: f32, color: (f32, f32, f32) }`; `fog_of(&Content, stage: usize, weather: usize) -> Fog`.
  - `Event::{NightCue { stage: u8 }, NightFell { stage: u8 }, ExitClosed { exit: u8 }, Alarm { enemy: u32, player: PlayerId }, RavensFlew { flock: u8 }}`.
  - `Snapshot::night: NightView`; `World::{night_view(&self) -> NightView, fog(&self) -> Fog, notice_scale(&self) -> f32}`; crate-only `World::{night: Night, step_night, step_clock}`, `Night { mirror, stage, cued, closed }`, `night::clock::stage_at(&[StageDef], f32) -> usize`.
  - `senses::step_enemies` gains `fog: f32, notice: f32` after `engage`; `PROTOCOL = 7`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
/// Three stages, a second apart, each cued half a second before.
const QUICK_NIGHT: &str = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
    (key: "one", name: "One", at: 2.0, warn: 0.5),
    (key: "two", name: "Two", at: 4.0, warn: 0.5, fog: 0.75, notice: 1.2)]"#;

/// The night's events since the last drain, with the step each came on.
fn night_events(w: &mut World, log: &mut Vec<(usize, Event)>, step: usize) {
    for e in w.drain_events() {
        if matches!(
            e,
            Event::NightCue { .. } | Event::NightFell { .. } | Event::ExitClosed { .. }
        ) {
            log.push((step, e));
        }
    }
}

#[test]
fn each_stage_is_cued_before_it_lands_and_only_once() {
    let mut w = world(QUICK_NIGHT, "");
    let mut log = Vec::new();
    for step in 0..secs(6.0) {
        w.step();
        night_events(&mut w, &mut log, step);
    }
    let kinds: Vec<&Event> = log.iter().map(|(_, e)| e).collect();
    assert_eq!(
        kinds,
        vec![
            &Event::NightCue { stage: 1 },
            &Event::NightFell { stage: 1 },
            &Event::NightCue { stage: 2 },
            &Event::NightFell { stage: 2 },
        ]
    );
    // Cued half a second (15 steps, give or take one for the clock's rounding) before landing.
    assert!((14..=16).contains(&(log[1].0 - log[0].0)), "{log:?}");
    assert!((14..=16).contains(&(log[3].0 - log[2].0)), "{log:?}");
    assert_eq!(w.night_view().stage, 2);
}

#[test]
fn a_clock_started_late_gives_every_passed_stage_once_in_order() {
    let mut w = world(QUICK_NIGHT, "");
    w.elapsed = 5.0;
    let mut log = Vec::new();
    for step in 0..secs(2.0) {
        w.step();
        night_events(&mut w, &mut log, step);
    }
    let kinds: Vec<&Event> = log.iter().map(|(_, e)| e).collect();
    assert_eq!(
        kinds,
        vec![
            &Event::NightCue { stage: 1 },
            &Event::NightCue { stage: 2 },
            &Event::NightFell { stage: 1 },
            &Event::NightFell { stage: 2 },
        ]
    );
    assert!(log.iter().all(|(s, _)| *s == 0), "all in the first step");
}

#[test]
fn moonrise_and_the_weather_close_the_fog_and_sharpen_the_eyes() {
    let night = format!(
        r#"{QUICK_NIGHT}, weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
            (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0, fog_color: Some((0.2, 0.2, 0.2)))]"#
    );
    // A walking hero (visibility 55) 22 yards in front of a sentry: past its 19.25-yard notice
    // range under a clear sky, inside its 23.1 yards once the eyes sharpen.
    let mut w = world_on(open(), &night, &sentry("brute", 30.0, 30.0, 0.0), "", standard);
    assert_eq!(w.fog().end, 40.0);
    join(&mut w, 0, 52.0, 30.0);
    for _ in 0..secs(3.5) {
        put(&mut w, 0, 52.0, 30.0, V2::new(-1.0, 0.0), Gait::Walk);
        w.step();
    }
    assert!(w.marks_for(0).is_empty(), "not noticed before the second stage");
    for _ in 0..secs(6.0) {
        put(&mut w, 0, 52.0, 30.0, V2::new(-1.0, 0.0), Gait::Walk);
        w.step();
    }
    assert_eq!(w.fog().end, 30.0);
    assert!((w.notice_scale() - 1.2).abs() < 1e-6);
    assert!(!w.marks_for(0).is_empty(), "noticed once the eyes sharpen");
    let mist = world_on(open(), &night, "", "", |c| RaidRoll {
        weather: 1,
        ..standard(c)
    });
    assert_eq!(
        mist.fog(),
        crate::Fog {
            start: 4.0,
            end: 28.0,
            color: (0.2, 0.2, 0.2)
        }
    );
}

#[test]
fn a_hero_sees_no_further_than_the_fog_now() {
    let night = r#"weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
        (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0)]"#;
    let post = sentry("brute", 65.0, 30.0, 0.0);
    let mut clear = world_on(open(), night, &post, "", standard);
    join(&mut clear, 0, 30.0, 30.0);
    assert_eq!(clear.seen_by(0), vec![1]);
    let mut mist = world_on(open(), night, &post, "", |c| RaidRoll {
        weather: 1,
        ..standard(c)
    });
    join(&mut mist, 0, 30.0, 30.0);
    assert!(mist.seen_by(0).is_empty());
}

#[test]
fn the_mirror_draws_the_hosts_fog() {
    let night = format!(
        r#"{QUICK_NIGHT}, weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
            (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0)]"#
    );
    let mut host = world_on(open(), &night, "", "", |c| RaidRoll {
        weather: 1,
        ..standard(c)
    });
    steps(&mut host, secs(4.5));
    let mut mirror = World::new(host.content().clone());
    mirror.apply_snapshot(host.snapshot());
    assert_eq!(mirror.night_view(), host.night_view());
    assert_eq!(mirror.fog(), host.fog());
    assert_eq!(mirror.fog().end, 21.0);
    // A mirror's clock never runs: its night is what the host said.
    steps(&mut mirror, secs(1.0));
    assert!(mirror.drain_events().is_empty());
}

#[test]
fn the_hunt_closes_its_exit_and_says_so_once() {
    let mut w = world_on(open(), QUICK_NIGHT, "", "", |c| RaidRoll {
        closes: Some((1, 2.0)),
        ..standard(c)
    });
    let mut log = Vec::new();
    for step in 0..secs(4.0) {
        w.step();
        night_events(&mut w, &mut log, step);
    }
    let closed: Vec<&Event> = log
        .iter()
        .map(|(_, e)| e)
        .filter(|e| matches!(e, Event::ExitClosed { .. }))
        .collect();
    assert_eq!(closed, vec![&Event::ExitClosed { exit: 1 }]);
}
```

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL to compile (`no variant named NightCue`, `no method named fog`).

- [ ] **Step 2: The night's events**

In `crates/nightfall-sim/src/combat/mod.rs`, `Event` gains, after `EnemyDied`:

```rust
    /// A stage of the night is coming (spec §7.2): its cue sounds and its line shows, `warn`
    /// seconds before it lands.
    NightCue { stage: u8 },
    /// A stage of the night has landed.
    NightFell { stage: u8 },
    /// An exit has closed for the rest of the raid (the hunt's, spec §7.2).
    ExitClosed { exit: u8 },
    /// A banshee's wail landed (spec §7.3): everything near is after `player`.
    Alarm { enemy: u32, player: PlayerId },
    /// A flock of ravens took off (spec §7.1), an index into the map's flocks.
    RavensFlew { flock: u8 },
```

If the client's `fight::show_events` (`crates/benilla-app/src/nightfall/fight.rs`) matches `Event` naming every variant, the five join its arm that shows nothing (`Event::NightCue { .. } | Event::NightFell { .. } | Event::ExitClosed { .. } | Event::Alarm { .. } | Event::RavensFlew { .. } => {}`); the night's own client (Task 11) shows them.

- [ ] **Step 3: The night's state, its view and its fog**

Replace `crates/nightfall-sim/src/night/mod.rs` with:

```rust
//! The night's threats (spec §7.2, §7.3, §8.1): the clock and its stages, the raid's roll,
//! signature monsters, risk zones, scent, the hunt and the ravens. The content is in `rules.rs`
//! and `places.rs`; the host's rules in the files beside them. A client's mirror runs none of
//! them: it has the night its host sends in each snapshot ([`NightView`]), and draws its fog.

mod clock;
mod places;
mod roll;
mod rules;
#[cfg(test)]
mod tests;

pub(crate) use places::{there_and_back, MonsterFile, RoadFile, ZoneFile};
pub use places::{MonsterDef, RoadDef, ZoneDef};
pub use rules::{
    CueSound, ExitRoll, HuntRules, NightRules, RavenRules, ScentRules, StageDef, StepSound,
    WeatherDef,
};

use serde::{Deserialize, Serialize};

use crate::{Content, World};

/// A flock not out this raid.
pub const FLOCK_GONE: u8 = 0;
/// A flock on the ground.
pub const FLOCK_SETTLED: u8 = 1;
/// A flock that has taken off and not settled again.
pub const FLOCK_FLOWN: u8 = 2;

/// The night as every client sees it, in each snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NightView {
    /// The stage the night is in, an index into the night's stages.
    pub stage: u8,
    /// The raid's weather, an index into the night's weathers.
    pub weather: u8,
    /// Each of the map's flocks: [`FLOCK_GONE`], [`FLOCK_SETTLED`] or [`FLOCK_FLOWN`].
    pub flocks: Vec<u8>,
}

/// The fog as the night has it now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub start: f32,
    pub end: f32,
    /// sRGB 0..1.
    pub color: (f32, f32, f32),
}

/// The fog of `weather` (the atmosphere's own when the night has no weathers) in `stage`: its
/// distances times the stage's `fog` (spec §7.2's Moonrise, §8.1's mist and rain).
pub fn fog_of(content: &Content, stage: usize, weather: usize) -> Fog {
    let a = &content.rules.atmosphere;
    let n = &content.rules.night;
    let (start, end, color) = n.weather.get(weather).map_or(
        (a.fog_start, a.fog_end, a.fog_color),
        |w| (w.fog_start, w.fog_end, w.fog_color.unwrap_or(a.fog_color)),
    );
    let k = n.stages.get(stage).map_or(1.0, |s| s.fog);
    Fog {
        start: start * k,
        end: end * k,
        color,
    }
}

/// The night's state on the host; on a client's mirror only `mirror`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Night {
    /// A client's mirror: the night its host sent.
    pub(crate) mirror: Option<NightView>,
    /// The stage the clock has reached.
    pub(crate) stage: usize,
    /// How many stages have been cued (the first counts as cued: the raid starts in it).
    pub(crate) cued: usize,
    /// The hunt's exit has closed, and said so.
    pub(crate) closed: bool,
}

impl World {
    /// The night as every client sees it: on the host from its clock and its roll, on a client's
    /// mirror as the host sent it.
    pub fn night_view(&self) -> NightView {
        if let Some(v) = &self.night.mirror {
            return v.clone();
        }
        NightView {
            stage: self.night.stage as u8,
            weather: self.raid.roll.weather,
            flocks: Vec::new(),
        }
    }

    /// The stage and the weather now, by index.
    fn sky(&self) -> (usize, usize) {
        match &self.night.mirror {
            Some(v) => (usize::from(v.stage), usize::from(v.weather)),
            None => (self.night.stage, usize::from(self.raid.roll.weather)),
        }
    }

    /// The fog now: how far heroes and enemies see on the host, what every client draws.
    pub fn fog(&self) -> Fog {
        let (stage, weather) = self.sky();
        fog_of(self.content(), stage, weather)
    }

    /// What enemies' notice range is times now (Moonrise's +20%, spec §7.2).
    pub fn notice_scale(&self) -> f32 {
        let (stage, _) = self.sky();
        self.content()
            .rules
            .night
            .stages
            .get(stage)
            .map_or(1.0, |s| s.notice)
    }

    /// The night's part of a step, after the raid's and before the enemies'. A mirror's night
    /// never runs: its host's comes in each snapshot.
    pub(crate) fn step_night(&mut self) {
        if self.night.mirror.is_some() {
            return;
        }
        self.step_clock();
    }
}
```

`crates/nightfall-sim/src/night/clock.rs`:

```rust
//! The night clock (spec §7.2): each stage's cue `warn` seconds before it lands, so the players
//! feel the clock with no timer on screen; then the stage; and the hunt's exit closing as the
//! roll said. A clock started late (a test hook) gives every passed stage's cue and landing once,
//! cues first, in order.

use crate::{Event, StageDef, World};

/// The stage the clock is in `t` seconds into the raid: the last that has landed.
pub(crate) fn stage_at(stages: &[StageDef], t: f32) -> usize {
    stages.iter().rposition(|s| s.at <= t).unwrap_or(0)
}

impl World {
    /// The clock's step: cues due, stages landed, the hunt's exit closed.
    pub(crate) fn step_clock(&mut self) {
        let content = self.content().clone();
        let stages = &content.rules.night.stages;
        let t = self.elapsed;
        while self.night.cued < stages.len()
            && t >= stages[self.night.cued].at - stages[self.night.cued].warn
        {
            let s = self.night.cued;
            // The first stage is where the raid starts: nothing to announce.
            if s > 0 {
                self.events.push(Event::NightCue { stage: s as u8 });
            }
            self.night.cued += 1;
        }
        let now = stage_at(stages, t);
        while self.night.stage < now {
            self.night.stage += 1;
            self.events.push(Event::NightFell {
                stage: self.night.stage as u8,
            });
        }
        if let Some((exit, at)) = self.raid.roll.closes {
            if !self.night.closed && t >= at {
                self.night.closed = true;
                self.events.push(Event::ExitClosed { exit });
            }
        }
    }
}
```

In `crates/nightfall-sim/src/lib.rs`, the `pub use night::{…}` gains `fog_of, Fog, NightView, FLOCK_FLOWN, FLOCK_GONE, FLOCK_SETTLED`.

- [ ] **Step 4: The world, its snapshot and its senses take the night**

In `crates/nightfall-sim/src/world.rs`:

- Add `use crate::night::{Night, NightView};`.
- `Snapshot` gains, after its last field:

```rust
    /// The night: its stage, the weather, the ravens (spec §7.2, §8.1).
    pub night: NightView,
```

- `World` gains `pub(crate) night: Night,`; `World::new`'s literal gains `night: Night::default(),`.
- `snapshot()`'s literal gains `night: self.night_view(),`; `apply_snapshot` gains `self.night.mirror = Some(snapshot.night);` (before the line that moves another of its fields out, or with `snapshot.night.clone()`).
- In `step`, right after `self.step_raid();` add:

```rust
        // The night after the raid: the clock's cues and stages, before the enemies act.
        self.step_night();
```

  and before the `if let Some(nav) = &mut self.nav {` that steps the enemies' senses add:

```rust
        // The fog and the eyes as the night has them now (Moonrise, the weather).
        let (fog, notice) = (self.fog().end, self.notice_scale());
```

  and that block's `senses::step_enemies(…, &engage,)` call gains `fog, notice,` after `&engage`.

In `crates/nightfall-sim/src/raid/mod.rs`'s `begin_raid`, after `self.raid = Raid::new(&content, roll);` add `self.night = crate::night::Night::default();` (a new raid's clock starts over).

In `crates/nightfall-sim/src/sight.rs`: in `seen_by`, `let fog = rules.atmosphere.fog_end;` becomes `let fog = self.fog().end;` (the `rules` binding stays for `share`); `snapshot_for`'s literal gains `night: self.night_view(),`.

In `crates/nightfall-sim/src/senses/react.rs`:

- `step_enemies` gains, after its `engage: &[Engage]` parameter, `fog: f32, notice: f32` (doc: `` `fog` is how far anyone sees now and `notice` what the notice range is times, both the night's (spec §7.2) ``); its line `let fog = content.rules.atmosphere.fog_end;` goes; its call `sense(e, mind, eye, h, &noises, &map, fog, r)` becomes `sense(e, mind, eye, h, &noises, &map, fog, notice, r)`.
- `sense` gains `notice: f32` after `fog: f32`, and its `notice_range(h.visibility, facing, fog, r)` becomes `notice_range(h.visibility * notice, facing, fog, r)` (the range grows with the eyes, and never past the fog).

In `crates/nightfall-sim/src/messages.rs`, `pub const PROTOCOL: u32 = 7;` (`Snapshot` changed shape).

Every other `Snapshot { … }` literal (see Files) gains `night: Default::default(),`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the six clock tests and every earlier one: the senses' tests see the atmosphere's 40-yard fog, unchanged, since their content has no weathers and its stages keep `fog` and `notice` at 1 before 8:00).
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the night clock cues each stage before it lands; the fog and the eyes follow the night; protocol 7

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Who is out tonight: monsters by the roll, newcomers per stage, the risk zones

**Files:**
- Create: `crates/nightfall-sim/src/night/arrivals.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs` (`Night::{waiting, try_in}`, `step_night`), `crates/nightfall-sim/src/night/clock.rs` (`stage_landed` called), `crates/nightfall-sim/src/night/tests.rs`
- Modify: `crates/nightfall-sim/src/enemies.rs` (`spawn_all`, `Brain::might`, `wanted`, `from_start`, `anchor`), `crates/nightfall-sim/src/world.rs` (`attach_map`, `brains`, `warnings`), `crates/nightfall-sim/src/raid/mod.rs` (`begin_raid`), `crates/nightfall-sim/src/senses/mod.rs` (`minds`, `mind_for`, `Senses::add`), `crates/nightfall-sim/src/combat/mod.rs` (`EnemyCombat::fresh`, `Combat::{place, add}`), `crates/nightfall-sim/src/combat/falling.rs` (`wound`), `crates/nightfall-sim/src/raid/containers.rs` (`Raid::new`, `step_bodies`)

**Interfaces:**
- Consumes: `spawn_all`, `Brain`, `Enemy`, `Senses::place`, `Combat::place`, `EnemyCombat`, `Engage::free`, `World::{attach_map, begin_raid, wound, map, content}`, `Raid::new`, `roll_table`, `step_bodies`, `UnitRef` (earlier plans); `SpawnDef::{stage_at, monster}`, `MapDef::zone_at`, `ZoneDef`, `EnemyDef::kit_first`, `NightRules::arrive_unseen` (Task 1); `RaidRoll::monsters` (Task 3); `step_clock` (Task 4).
- Produces:
  - `enemies::spawn_all(content: &Content, map: &HeightMap, lines: impl Fn(usize) -> bool, first_id: u32) -> (Vec<Enemy>, Vec<Brain>, Vec<String>)`: only the lines `lines` keeps, ids from `first_id`; an enemy inside a zone has its health times the zone's `hp`.
  - Crate-only `enemies::{wanted(&Content, usize, &[u8]) -> bool, from_start(&Content, usize, &[u8]) -> bool, anchor(&SpawnDef) -> V2}`; `Brain::might(&self) -> f32`.
  - `World::brains` and `World::warnings` become `pub(crate)`; `Senses::minds` becomes `pub(crate)`; crate-only `Senses::add(&mut self, &Content, usize)`; `EnemyCombat::fresh(&Content, usize) -> EnemyCombat`, `Combat::add(&mut self, &Content, usize)`.
  - Crate-only `World::{place_enemies, stage_landed, step_arrivals, might_of(&self, Option<UnitRef>) -> f32}`; `Night::{waiting: Vec<usize>, try_in: f32}`.
  - `attach_map` and `begin_raid` both place the raid's enemies (`place_enemies`), whichever comes last.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
/// Every list that runs beside the world's enemies, as long as it.
fn lists_in_step(w: &World) {
    let n = w.enemies.len();
    assert_eq!(w.brains.len(), n);
    assert_eq!(w.senses.minds.len(), n);
    assert_eq!(w.combat.enemies.len(), n);
    let mut ids: Vec<u32> = w.enemies.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "ids are unique");
}

const GHOST: &str = r#"
    (key: "ghost", name: "Ghost", odds: 0.5, spawns: [
        (place: "Ghost", kind: "brute", behaviour: Sentry(at: (50.0, 50.0), facing: 0.0))]),
"#;

#[test]
fn only_the_monsters_out_are_placed_whenever_the_raid_begins() {
    let none = world_on(open(), "", "", GHOST, standard);
    assert!(none.enemies.is_empty());
    let out = world_on(open(), "", "", GHOST, |c| RaidRoll {
        monsters: vec![0],
        ..standard(c)
    });
    assert_eq!(out.enemies.len(), 1);
    lists_in_step(&out);
    // The map first, the raid after: the raid's own monsters, placed again.
    let c = content("", "", GHOST);
    let mut w = World::new(Arc::new(c.clone()));
    w.attach_map(Arc::new(open()));
    assert!(w.enemies.is_empty());
    w.begin_raid(RaidRoll {
        monsters: vec![0],
        ..standard(&c)
    })
    .unwrap();
    assert_eq!(w.enemies.len(), 1);
    lists_in_step(&w);
}

#[test]
fn newcomers_wait_unseen_and_keep_every_list_in_step() {
    let night = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
        (key: "late", name: "Late", at: 1.0, warn: 0.0)], arrive_unseen: 20.0"#;
    let spawns = format!(
        r#"{}(place: "Late", kind: "brute", stage: Some("late"), behaviour: Sentry(at: (60.0, 30.0), facing: 0.0)),"#,
        sentry("brute", 10.0, 100.0, 0.0)
    );
    let mut w = world(night, &spawns);
    assert_eq!(w.enemies.len(), 1, "the newcomer is not there at the start");
    join(&mut w, 0, 62.0, 30.0);
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 1, "it waits while a hero stands where it comes");
    put(&mut w, 0, 10.0, 10.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(1.5));
    assert_eq!(w.enemies.len(), 2);
    lists_in_step(&w);
    assert_eq!(w.enemies[1].id, 2);
    assert_eq!(w.enemies[1].pos, V2::new(60.0, 30.0));
    // It thinks like the rest: a hero walking in front of it is noticed.
    for _ in 0..secs(5.0) {
        put(&mut w, 0, 70.0, 30.0, V2::new(-1.0, 0.0), Gait::Walk);
        w.step();
    }
    assert!(w.marks_for(0).iter().any(|m| m.enemy == 2));
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 2, "it came once");
}

#[test]
fn a_risk_zone_makes_enemies_tougher_and_its_loot_richer() {
    let mut w = world("", &sentry("brute", 100.0, 100.0, 180.0));
    assert_eq!(w.enemies[0].max_hp, 1500.0);
    join(&mut w, 0, 98.5, 100.0);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, secs(3.0));
    let blows = events_of(&mut w, |e| match e {
        Event::Damage {
            to: crate::UnitRef::Hero(0),
            amount,
            ..
        } => Some(*amount),
        _ => None,
    });
    assert!(!blows.is_empty() && blows.iter().all(|&a| a == 50.0), "{blows:?}");
    let stacks = |at: V2| {
        w.raid
            .containers
            .iter()
            .find(|c| c.at == at)
            .map(|c| c.stacks.iter().flatten().count())
    };
    assert_eq!(stacks(V2::new(10.0, 10.0)), Some(1));
    assert_eq!(stacks(V2::new(100.0, 100.0)), Some(2));
}
```

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL to compile (`field brains of struct World is private`, `no field minds`).

- [ ] **Step 2: Spawn lines filtered, zones' health, the brain's might**

In `crates/nightfall-sim/src/enemies.rs`:

- `spawn_all` becomes:

```rust
/// The enemies of the content's spawn lines that `lines` keeps, on the map's floors, with a
/// warning for each that could not be placed. Ids count from `first_id` in the content's order;
/// an enemy placed inside a zone (spec §8.1) has its health times the zone's `hp`, and its blows
/// times its `damage` (kept in its brain).
pub(crate) fn spawn_all(
    content: &Content,
    map: &HeightMap,
    lines: impl Fn(usize) -> bool,
    first_id: u32,
) -> (Vec<Enemy>, Vec<Brain>, Vec<String>) {
```

  its `let mut id = 1u32;` becomes `let mut id = first_id;`, its loop's first line inside `for (si, s) in content.spawns.iter().enumerate() {` is `if !lines(si) { continue; }`, and where it builds an enemy, before the `enemies.push(Enemy { … })`:

```rust
            let (toughness, might) = content
                .map
                .zone_at(at.ground())
                .map_or((1.0, 1.0), |z| (z.hp, z.damage));
```

  with the literal's `hp: def.hp,` and `max_hp: def.hp,` becoming `hp: def.hp * toughness,` and `max_hp: def.hp * toughness,`, and the `Brain { … }` literal gaining `might,`.
- `Brain` gains:

```rust
    /// Its blows are times this: its zone's `damage` where it was placed (spec §8.1).
    might: f32,
```

  and `impl Brain` gains:

```rust
    /// What its blows are times.
    pub(crate) fn might(&self) -> f32 {
        self.might
    }
```

- Add:

```rust
/// Whether spawn line `si` belongs in a raid whose signature monsters out are `out`: every line
/// that is not a monster's, and a monster's when it is out.
pub(crate) fn wanted(content: &Content, si: usize, out: &[u8]) -> bool {
    content.spawns[si]
        .monster
        .is_none_or(|m| out.contains(&(m as u8)))
}

/// Whether spawn line `si` is placed as the raid begins: wanted, and not waiting for a stage.
pub(crate) fn from_start(content: &Content, si: usize, out: &[u8]) -> bool {
    content.spawns[si].stage_at.is_none() && wanted(content, si, out)
}

/// Where a spawn line's enemies come: a post, a route's first point, an area's centre.
pub(crate) fn anchor(s: &SpawnDef) -> V2 {
    let p = match &s.behaviour {
        Behaviour::Sentry { at, .. } | Behaviour::Sleeper { at, .. } => *at,
        Behaviour::Patrol { route, .. } => route.first().copied().unwrap_or_default(),
        Behaviour::Wander { center, .. } => *center,
        Behaviour::Road { .. } => (0.0, 0.0),
    };
    V2::new(p.0, p.1)
}
```

  with `SpawnDef` added to its `crate::` imports.

In `crates/nightfall-sim/src/world.rs`:

- `brains: Vec<Brain>,` and `warnings: Vec<String>,` become `pub(crate) brains: Vec<Brain>,` and `pub(crate) warnings: Vec<String>,`.
- `attach_map` becomes:

```rust
    /// The host's height map: from now on the world knows walls and floors, and the raid's
    /// enemies are placed on them ([`World::place_enemies`]; any placed before start over).
    pub fn attach_map(&mut self, map: Arc<HeightMap>) {
        self.nav = Some(Navigator::new(map));
        self.place_enemies();
    }
```

In `crates/nightfall-sim/src/raid/mod.rs`'s `begin_raid`, after the line Task 4 added (`self.night = …`), add `self.place_enemies();` (nothing happens without a map; with one, the raid's own monsters).

- [ ] **Step 3: One more mind, one more fight**

In `crates/nightfall-sim/src/senses/mod.rs`:

- `minds: Vec<Mind>,` becomes `pub(crate) minds: Vec<Mind>,` (the night alerts and sniffs through it).
- `place` becomes:

```rust
    /// A calm mind for each enemy placed from these spawn lines.
    pub(crate) fn place(&mut self, content: &Content, spawns: impl IntoIterator<Item = usize>) {
        self.minds = spawns.into_iter().map(|si| mind_for(content, si)).collect();
        self.ways = vec![Way::default(); self.minds.len()];
        self.shouts.clear();
    }

    /// One more enemy, of spawn line `si`, calm (a newcomer of the night, spec §7.2).
    pub(crate) fn add(&mut self, content: &Content, si: usize) {
        self.minds.push(mind_for(content, si));
        self.ways.push(Way::default());
    }
```

- Add, outside the `impl`:

```rust
/// A calm mind for an enemy of spawn line `si`: a sentry's wide front cone, a sleeper's closed
/// eyes, everyone else's own cone.
fn mind_for(content: &Content, si: usize) -> Mind {
    let r = &content.rules.senses;
    match content.spawns[si].behaviour {
        Behaviour::Sentry { .. } => Mind::new(r.sentry_cone, false),
        Behaviour::Sleeper { .. } => Mind::new(r.cone, true),
        _ => Mind::new(r.cone, false),
    }
}
```

In `crates/nightfall-sim/src/combat/mod.rs`:

- Add:

```rust
impl EnemyCombat {
    /// A fresh fight for an enemy of `kind`: each move first ready at its kind's `kit_first`
    /// (half its wait unless the content says), so none opens with its big move unless it should.
    pub(crate) fn fresh(content: &Content, kind: usize) -> Self {
        Self {
            moves_in: content.enemies[kind].kit_first.clone(),
            engage: Engage::free(content.rules.senses.chase_reach),
            ..Self::default()
        }
    }
}
```

- `Combat::place`'s body becomes `self.enemies = enemies.iter().map(|e| EnemyCombat::fresh(content, e.kind)).collect();` (its doc: `A fresh fight for each enemy placed.`), and `impl Combat` gains:

```rust
    /// A fresh fight for one more enemy, of `kind`.
    pub(crate) fn add(&mut self, content: &Content, kind: usize) {
        self.enemies.push(EnemyCombat::fresh(content, kind));
    }
```

In `crates/nightfall-sim/src/combat/falling.rs`'s `wound`, after its early return (`if !self.heroes[hi].fight.up() || !(amount > 0.0) { return; }`) add:

```rust
        // An enemy's blow carries its zone's might (spec §8.1).
        let amount = amount * self.might_of(by);
```

In `crates/nightfall-sim/src/raid/containers.rs`:

- In `Raid::new`, the line `let stacks = roll_table(&content.loot.tables[p.table], &mut dice, content);` and the `slots` line become:

```rust
            let slots = usize::from(content.loot.kinds[p.kind].slots);
            // A risk zone's containers roll their table more times (spec §8.1).
            let rolls = 1 + content.map.zone_at(p.at).map_or(0, |z| usize::from(z.loot_rolls));
            let mut stacks = Vec::new();
            for _ in 0..rolls {
                stacks.extend(roll_table(&content.loot.tables[p.table], &mut dice, content));
            }
            stacks.truncate(slots);
```

- In `step_bodies`, `let stacks = roll_table(&content.loot.tables[table], &mut dice, &content);` becomes:

```rust
            let rolls = 1 + content.map.zone_at(at).map_or(0, |z| usize::from(z.loot_rolls));
            let mut stacks = Vec::new();
            for _ in 0..rolls {
                stacks.extend(roll_table(&content.loot.tables[table], &mut dice, &content));
            }
            stacks.truncate(usize::from(content.rules.run.container_slots));
```

- [ ] **Step 4: The arrivals**

`crates/nightfall-sim/src/night/arrivals.rs`:

```rust
//! Who is out tonight (spec §7.2, §7.3, §8.1): the enemies a raid begins with (the content's
//! spawn lines from the start, a signature monster's only when the raid rolled it out), the
//! newcomers each stage of the night brings, placed once no hero is within `arrive_unseen` yards
//! of where they come, and the risk zones' might in every enemy blow.

use crate::enemies::{self, Brain};
use crate::{UnitRef, World, DT};

/// Seconds between two tries at placing the newcomers who wait.
const TRY_EVERY: f32 = 1.0;

impl World {
    /// Every enemy this raid begins with, on the attached map; any placed before start over.
    /// Nothing happens without a map.
    pub(crate) fn place_enemies(&mut self) {
        let Some(map) = self.map().cloned() else {
            return;
        };
        let content = self.content().clone();
        let out = self.raid.roll.monsters.clone();
        let (enemies, brains, warnings) =
            enemies::spawn_all(&content, &map, |si| enemies::from_start(&content, si, &out), 1);
        self.enemies = enemies;
        self.brains = brains;
        self.warnings.extend(warnings);
        self.senses
            .place(&content, self.brains.iter().map(Brain::spawn));
        self.combat.place(&content, &self.enemies);
        self.night.waiting.clear();
    }

    /// Stage `stage` has landed: its spawn lines that this raid wants wait to come.
    pub(crate) fn stage_landed(&mut self, stage: usize) {
        let content = self.content().clone();
        let out = &self.raid.roll.monsters;
        for si in 0..content.spawns.len() {
            if content.spawns[si].stage_at == Some(stage) && enemies::wanted(&content, si, out) {
                self.night.waiting.push(si);
            }
        }
        self.night.try_in = 0.0;
    }

    /// Every second, each waiting line whose place no up hero is near comes.
    pub(crate) fn step_arrivals(&mut self) {
        if self.night.waiting.is_empty() || self.map().is_none() {
            return;
        }
        self.night.try_in -= DT;
        if self.night.try_in > 0.0 {
            return;
        }
        self.night.try_in = TRY_EVERY;
        let content = self.content().clone();
        let clear = content.rules.night.arrive_unseen;
        for si in std::mem::take(&mut self.night.waiting) {
            let at = enemies::anchor(&content.spawns[si]);
            let near = self
                .heroes
                .iter()
                .any(|h| h.fight.up() && h.pos.distance(at) <= clear);
            if near {
                self.night.waiting.push(si);
            } else {
                self.add_line(si);
            }
        }
    }

    /// Spawn line `si`'s enemies join the raid, with a mind and a fight each, ids after every
    /// enemy's so far.
    fn add_line(&mut self, si: usize) {
        let Some(map) = self.map().cloned() else {
            return;
        };
        let content = self.content().clone();
        let first = self.enemies.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let (enemies, brains, warnings) = enemies::spawn_all(&content, &map, |i| i == si, first);
        self.warnings.extend(warnings);
        for (e, b) in enemies.into_iter().zip(brains) {
            self.senses.add(&content, b.spawn());
            self.combat.add(&content, e.kind);
            self.enemies.push(e);
            self.brains.push(b);
        }
    }

    /// What blows from `by` are times: an enemy's zone's `damage` (spec §8.1), else 1.
    pub(crate) fn might_of(&self, by: Option<UnitRef>) -> f32 {
        match by {
            Some(UnitRef::Enemy(id)) => self
                .enemies
                .iter()
                .position(|e| e.id == id)
                .and_then(|i| self.brains.get(i))
                .map_or(1.0, Brain::might),
            _ => 1.0,
        }
    }
}
```

In `crates/nightfall-sim/src/night/mod.rs`: add `mod arrivals;` below `mod places;`; `Night` gains:

```rust
    /// Spawn lines whose stage has landed, waiting until no hero is near where they come.
    pub(crate) waiting: Vec<usize>,
    /// Seconds to the next try at placing them.
    pub(crate) try_in: f32,
```

and `step_night`'s `self.step_clock();` is followed by `self.step_arrivals();`.

In `crates/nightfall-sim/src/night/clock.rs`, inside the loop that lands stages, after the `NightFell` push add `self.stage_landed(self.night.stage);`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the three new tests, the earlier night tests, and every earlier enemy, senses, combat and raid test: their content has no zones and no monsters, so `spawn_all` places what it did and every blow is times 1).

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: monsters out by the roll, newcomers per stage placed unseen, tougher risk zones with richer loot

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The hunt: hunters sent after the loudest recent noise

**Files:**
- Create: `crates/nightfall-sim/src/night/hunt.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs` (`Night::{loudest, hunt_in}`, `night_holds`), `crates/nightfall-sim/src/night/tests.rs`, `crates/nightfall-sim/src/enemies.rs` (`Brain::{errand, send}`, `plan`, `walk`, `arrive`), `crates/nightfall-sim/src/senses/mod.rs` (`State` exported), `crates/nightfall-sim/src/world.rs` (`step`)

**Interfaces:**
- Consumes: `Senses::noises`, `Noise { by, at, radius }`, `Mind::state`, `State::Calm` (senses Tasks 3, 5); `Engage`, `Hold`, `World::hold_enemies` (combat Task 6); `Brain`, `plan`, `walk`, `arrive`, `Navigator::find_path`, `PATROL_BUDGET`, `ARRIVE` (world knowledge Task 8, senses Task 6); `HuntRules`, `EnemyDef::hunter` (Task 1); `Night::stage` (Task 4).
- Produces:
  - Crate-only `night::hunt::Loud { at: V3, radius: f32, t: f32 }`; `Night::{loudest: Option<Loud>, hunt_in: f32}`; `World::{night_listen, step_hunt, night_holds}`.
  - Crate-only `Brain::send(&mut self, to: V3)`: a calm brain goes there at a run, rests `hunt.rest` seconds and takes up its order again; an errand no path reaches is dropped.
  - `crate::senses::State` is crate-visible.
  - `World::step` calls `night_holds` after `hold_enemies` and `night_listen` before the step's noises are cleared.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
/// The hunt from the first second, the hunters called every second.
const HUNT_NIGHT: &str = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
    (key: "hunt", name: "The hunt", at: 1.0, warn: 0.0, hunt: true)],
    hunt: (every: 1.0, memory: 30.0, reach: 200.0, rest: 4.0)"#;

/// A hunting worgen far in the north-east and a brute beside it, a hero far in the south-west
/// who makes a loud noise none of them hears.
fn hunt_world(night: &str) -> World {
    let spawns = format!(
        "{}{}",
        sentry("worgen", 100.0, 100.0, 0.0),
        sentry("brute", 100.0, 90.0, 0.0)
    );
    let mut w = world(night, &spawns);
    join(&mut w, 0, 10.0, 10.0);
    w.make_noise(0, V3::new(10.0, 10.0, 0.0), crate::Loudness::Loud);
    w
}

#[test]
fn the_hunt_sends_hunters_toward_the_loudest_noise() {
    let mut w = hunt_world(HUNT_NIGHT);
    steps(&mut w, secs(8.0));
    let to_noise = |w: &World, i: usize| w.enemies[i].pos.distance(V2::new(10.0, 10.0));
    assert!(to_noise(&w, 0) < 100.0, "the worgen ran toward it: {:?}", w.enemies[0].pos);
    assert_eq!(w.enemies[1].pos, V2::new(100.0, 90.0), "the brute is no hunter");
}

#[test]
fn no_hunt_before_its_stage_nor_after_the_noise_is_forgotten() {
    let early = HUNT_NIGHT.replace("at: 1.0, warn: 0.0, hunt", "at: 100.0, warn: 0.0, hunt");
    let mut w = hunt_world(&early);
    steps(&mut w, secs(8.0));
    assert_eq!(w.enemies[0].pos, V2::new(100.0, 100.0));
    let forgetful = HUNT_NIGHT
        .replace("at: 1.0, warn: 0.0, hunt", "at: 3.0, warn: 0.0, hunt")
        .replace("memory: 30.0", "memory: 1.0");
    let mut w = hunt_world(&forgetful);
    steps(&mut w, secs(8.0));
    assert_eq!(w.enemies[0].pos, V2::new(100.0, 100.0));
}

#[test]
fn a_hunter_rests_where_the_noise_was_then_goes_home() {
    let mut w = hunt_world(HUNT_NIGHT);
    // Far from where it goes, so nothing it sees there holds it.
    put(&mut w, 0, 110.0, 10.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(30.0));
    let at_noise = w.enemies[0].pos.distance(V2::new(10.0, 10.0));
    let home = w.enemies[0].pos.distance(V2::new(100.0, 100.0));
    assert!(at_noise < 3.0 || home < at_noise, "{:?}", w.enemies[0].pos);
    // The noise is forgotten 30 s after it was made; the worgen rests 4 s more, then walks the
    // 127 yards home at 2.5 yards a second.
    steps(&mut w, secs(65.0));
    assert!(w.enemies[0].pos.distance(V2::new(100.0, 100.0)) < 1.0, "back at its post");
}
```

While the noise is remembered the hunt calls every second, and a hunter already there only rests again; once it is forgotten, the hunter's rest runs out and its brain takes up its order: a sentry walks back to its post.

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL to compile (`no method named send`, or the worgen staying at its post).

- [ ] **Step 2: A brain's errand**

In `crates/nightfall-sim/src/enemies.rs`:

- `Brain` gains:

```rust
    /// Where it runs now on the night's errand (the hunt, spec §7.2), before its order again.
    errand: Option<V3>,
```

  (`spawn_all`'s `Brain { … }` literal gains `errand: None,`), and `impl Brain` gains:

```rust
    /// Run to `to` (the hunt's noise), rest there, then take up its order again: its path now
    /// goes, so its next plan is the errand's.
    pub(crate) fn send(&mut self, to: V3) {
        self.errand = Some(to);
        self.path = None;
        self.next = 0;
        self.wait = 0.0;
    }
```

- In `plan`, right after `let here = V3::at(e.pos, e.z);` add:

```rust
    if let Some(goal) = b.errand {
        if here.ground().distance(goal.ground()) <= ARRIVE {
            b.errand = None;
            b.wait = content.rules.night.hunt.rest;
            return;
        }
        *budget -= 1;
        match nav.find_path(here, goal, false, PATROL_BUDGET) {
            Some(path) => {
                b.path = Some(path);
                b.next = 0;
            }
            // No way there: back to its order.
            None => b.errand = None,
        }
        return;
    }
```

- In `walk`, `let speed = content.enemies[e.kind].walk_speed;` becomes:

```rust
    // An errand is run; the order walked.
    let def = &content.enemies[e.kind];
    let speed = if b.errand.is_some() {
        def.run_speed
    } else {
        def.walk_speed
    };
```

- `arrive` begins with:

```rust
    if b.errand.take().is_some() {
        b.wait = content.rules.night.hunt.rest;
        return;
    }
```

In `crates/nightfall-sim/src/senses/mod.rs`, `use awareness::{Mind, Shout, State};` becomes `use awareness::{Mind, Shout};` and add `pub(crate) use awareness::State;` beside the module's other `pub(crate) use` lines.

- [ ] **Step 3: The hunt**

`crates/nightfall-sim/src/night/hunt.rs`:

```rust
//! The hunt (spec §7.2: "worgen packs hunt toward the loudest recent noise"): while a hunting
//! stage lasts, every `hunt.every` seconds each hunter that is calm, in the fight and within
//! `hunt.reach` yards of the loudest noise of the last `hunt.memory` seconds runs there, looks
//! about for `hunt.rest` seconds and goes home. What it finds there, its senses deal with.

use crate::combat::Hold;
use crate::senses::State;
use crate::{World, DT, V3};

/// The loudest noise heard lately, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Loud {
    pub(crate) at: V3,
    pub(crate) radius: f32,
    pub(crate) t: f32,
}

impl World {
    /// The step's noises, kept when one is the loudest of late: last in a step, once every noise
    /// of it is made. One older than the memory is forgotten.
    pub(crate) fn night_listen(&mut self) {
        if self.night.mirror.is_some() {
            return;
        }
        let memory = self.content().rules.night.hunt.memory;
        let now = self.elapsed;
        if self.night.loudest.is_some_and(|l| now - l.t > memory) {
            self.night.loudest = None;
        }
        for n in &self.senses.noises {
            if self.night.loudest.is_none_or(|l| n.radius >= l.radius) {
                self.night.loudest = Some(Loud {
                    at: n.at,
                    radius: n.radius,
                    t: now,
                });
            }
        }
    }

    /// The hunt's call, while the stage hunts: calm hunters near the loudest noise go to it.
    pub(crate) fn step_hunt(&mut self) {
        let content = self.content().clone();
        let n = &content.rules.night;
        let hunting = n.stages.get(self.night.stage).is_some_and(|s| s.hunt);
        if !hunting {
            self.night.hunt_in = 0.0;
            return;
        }
        self.night.hunt_in -= DT;
        if self.night.hunt_in > 0.0 {
            return;
        }
        self.night.hunt_in = n.hunt.every;
        let Some(loud) = self.night.loudest else {
            return;
        };
        if self.elapsed - loud.t > n.hunt.memory {
            return;
        }
        for i in 0..self.enemies.len().min(self.brains.len()) {
            let e = &self.enemies[i];
            let free = self
                .combat
                .enemies
                .get(i)
                .is_none_or(|c| c.engage.hold == Hold::Free);
            let calm = self
                .senses
                .minds
                .get(i)
                .is_some_and(|m| m.state == State::Calm);
            if content.enemies[e.kind].hunter
                && e.hp > 0.0
                && free
                && calm
                && e.pos.distance(loud.at.ground()) <= n.hunt.reach
            {
                self.brains[i].send(loud.at);
            }
        }
    }
}
```

The hunt sends a hunter again at each call while the noise is remembered; `send` on a hunter already running there only searches its path again (one search, within the step's budget).

In `crates/nightfall-sim/src/night/mod.rs`: add `mod hunt;` below `mod clock;`; `Night` gains:

```rust
    /// The loudest noise heard lately, for the hunt.
    pub(crate) loudest: Option<hunt::Loud>,
    /// Seconds to the hunt's next call.
    pub(crate) hunt_in: f32,
```

and `impl World` gains:

```rust
    /// The night's hold on the enemies, after the fight's own ([`World::hold_enemies`]) and
    /// before they strike, look and move. A mirror and a world without a map have none.
    pub(crate) fn night_holds(&mut self) {
        if self.night.mirror.is_some() || self.map().is_none() {
            return;
        }
        self.step_hunt();
    }
```

In `crates/nightfall-sim/src/world.rs`'s `step`: in the block `if self.nav.is_some() { self.hold_enemies(); … self.strike_back(); }`, add `self.night_holds();` right after `self.hold_enemies();`; and right before `self.senses.noises.clear();` add:

```rust
        // The hunt remembers the loudest of them.
        self.night_listen();
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the three hunt tests and every earlier one; no earlier content has a hunter in a hunting stage).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the hunt sends the worgen after the loudest recent noise

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Scent: trails laid and broken; trackers on a hero's trail

**Files:**
- Create: `crates/nightfall-sim/src/night/scent.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs` (`Night::trails`, `step_night`, `night_holds`), `crates/nightfall-sim/src/night/tests.rs`, `crates/nightfall-sim/src/senses/awareness.rs` (`Mind::sniff`), `crates/nightfall-sim/src/world.rs` (`remove_hero`)

**Interfaces:**
- Consumes: `Mind::{state, is_alerted, entry, there}`, `State::{Calm, Wary, Checking}`, `AwarenessRules::check` (senses Task 5); `HeightMap::is_water` (Task 2); `MapDef::on_road`, `ScentRules`, `EnemyDef::tracker` (Task 1); `Hold` (combat Task 6).
- Produces:
  - Crate-only `night::scent::{Crumb { at: V3, t: f32, stretch: u32 }, Trail { player, crumbs: VecDeque<Crumb>, stretch, broken }}` with `Trail::next_from(&self, V2, f32) -> Option<V3>`; `Night::trails: Vec<Trail>`; `World::{lay_scent, follow_scent}`.
  - Crate-only `Mind::sniff(&mut self, player: PlayerId, place: V3, r: &AwarenessRules)`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
/// `player`'s hero walks from point to point at `speed` yards a second, one input a step, in
/// stealth when `sneak` (silent, and hardly seen).
fn walk_path(w: &mut World, player: PlayerId, points: &[V2], speed: f32, sneak: bool) {
    let step = speed / TICK_HZ as f32;
    for leg in points.windows(2) {
        let (a, b) = (leg[0], leg[1]);
        let d = a.distance(b);
        let n = (d / step).ceil().max(1.0) as usize;
        let facing = (b - a).normalize_or_zero();
        for k in 1..=n {
            let p = a + (b - a) * (k as f32 / n as f32);
            if let Some(h) = w.heroes.iter_mut().find(|h| h.player == player) {
                h.stealthed = sneak;
            }
            put(w, player, p.x, p.y, facing, Gait::Walk);
            w.step();
        }
    }
}

/// `player` stands where they are for `s` seconds, sneaking still when `sneak`.
fn wait(w: &mut World, player: PlayerId, s: f32, sneak: bool) {
    let h = w.hero(player).unwrap();
    let (p, f) = (h.pos, h.facing);
    for _ in 0..secs(s) {
        if let Some(h) = w.heroes.iter_mut().find(|h| h.player == player) {
            h.stealthed = sneak;
        }
        put(w, player, p.x, p.y, f, Gait::Still);
        w.step();
    }
}

fn crumbs(w: &World, player: PlayerId) -> Vec<crate::night::Crumb> {
    w.night
        .trails
        .iter()
        .find(|t| t.player == player)
        .map(|t| t.crumbs.iter().copied().collect())
        .unwrap_or_default()
}

#[test]
fn trails_are_laid_as_heroes_walk_and_fade() {
    let mut w = world("", "");
    join(&mut w, 0, 10.0, 10.0);
    walk_path(&mut w, 0, &[V2::new(10.0, 10.0), V2::new(10.0, 30.0)], 4.0, false);
    let c = crumbs(&w, 0);
    assert!((10..=12).contains(&c.len()), "a crumb every 2 yards: {}", c.len());
    assert!(c.windows(2).all(|p| p[0].t <= p[1].t));
    wait(&mut w, 0, 31.0, false);
    assert!(crumbs(&w, 0).len() <= 1, "faded after 30 seconds");
}

#[test]
fn no_scent_is_left_in_water_or_on_a_road() {
    let mut b = flat(120.0);
    pond(&mut b, V2::new(5.0, 20.0), V2::new(15.0, 26.0));
    let mut w = world_on(b.finish(), "", "", "", standard);
    join(&mut w, 0, 10.0, 10.0);
    walk_path(&mut w, 0, &[V2::new(10.0, 10.0), V2::new(10.0, 75.0)], 4.0, false);
    let c = crumbs(&w, 0);
    assert!(c.iter().all(|k| !(20.5..=25.5).contains(&k.at.y)), "none in the pond");
    assert!(c.iter().all(|k| !(57.5..=62.5).contains(&k.at.y)), "none on the road");
    let stretches: Vec<u32> = c.iter().map(|k| k.stretch).collect();
    assert_eq!(stretches.first(), Some(&0));
    assert_eq!(stretches.last(), Some(&2), "the pond and the road each broke the trail");
}

/// A tracker at (20, 20) facing +x, and a sneaking rogue who passes 2.5 yards behind it (past
/// its notice even behind) along `path`, then stands still at its end.
fn tracker_world(map: HeightMap, path: &[V2], speed: f32, night: &str) -> World {
    let mut w = world_on(map, night, &sentry("tracker", 20.0, 20.0, 0.0), "", standard);
    join(&mut w, 0, path[0].x, path[0].y);
    walk_path(&mut w, 0, path, speed, true);
    w
}

#[test]
fn a_tracker_follows_a_sneaking_rogue_by_scent() {
    let end = V2::new(50.0, 40.0);
    let path = [V2::new(17.5, 10.0), V2::new(17.5, 40.0), end];
    let mut w = tracker_world(open(), &path, 1.8, "");
    wait(&mut w, 0, 40.0, true);
    assert!(
        w.enemies[0].pos.distance(end) < 4.0,
        "the tracker found them: {:?}",
        w.enemies[0].pos
    );
    // A brute in its place would never have known.
    let mut b = world_on(open(), "", &sentry("brute", 20.0, 20.0, 0.0), "", standard);
    join(&mut b, 0, path[0].x, path[0].y);
    walk_path(&mut b, 0, &path, 1.8, true);
    wait(&mut b, 0, 40.0, true);
    assert_eq!(b.enemies[0].pos, V2::new(20.0, 20.0));
}

#[test]
fn water_breaks_the_trail() {
    let end = V2::new(50.0, 45.0);
    let mut m = flat(120.0);
    pond(&mut m, V2::new(12.0, 27.0), V2::new(23.0, 33.0));
    let path = [V2::new(17.5, 10.0), V2::new(17.5, 45.0), end];
    let mut w = tracker_world(m.finish(), &path, 1.8, "");
    wait(&mut w, 0, 60.0, true);
    let t = w.enemies[0].pos;
    assert!(t.y < 27.0, "it stopped at the pond's near bank: {t:?}");
    assert!(t.distance(end) > 10.0);
}

#[test]
fn a_road_breaks_the_trail() {
    let end = V2::new(50.0, 75.0);
    let path = [V2::new(17.5, 45.0), V2::new(17.5, 75.0), end];
    let mut w = world_on(open(), "", &sentry("tracker", 20.0, 50.0, 0.0), "", standard);
    join(&mut w, 0, path[0].x, path[0].y);
    walk_path(&mut w, 0, &path, 1.8, true);
    wait(&mut w, 0, 60.0, true);
    let t = w.enemies[0].pos;
    assert!(t.y < 57.0, "it stopped at the road's near side: {t:?}");
    assert!(t.distance(end) > 10.0);
}

#[test]
fn a_trail_too_old_is_lost() {
    // Scent lasts 8 seconds; the rogue runs a long loop at 8 yards a second, the tracker trots
    // after at 4.5, and falls further behind than the scent lasts.
    let end = V2::new(90.0, 25.0);
    let path = [
        V2::new(17.5, 10.0),
        V2::new(17.5, 40.0),
        V2::new(110.0, 40.0),
        V2::new(110.0, 10.0),
        V2::new(40.0, 10.0),
        V2::new(40.0, 25.0),
        end,
    ];
    let mut w = tracker_world(open(), &path, 8.0, "scent: (lasts: 8.0)");
    wait(&mut w, 0, 60.0, true);
    assert!(
        w.enemies[0].pos.distance(end) > 15.0,
        "it lost them on the way: {:?}",
        w.enemies[0].pos
    );
}
```

Take the `#[allow(unused_imports)]` lines Task 3 put on the kit's `use` lines off: `pond` and `V3` are used now.

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL to compile (`no field trails`, `cannot find type Crumb`).

- [ ] **Step 2: A mind that sniffs**

In `crates/nightfall-sim/src/senses/awareness.rs`, add to `impl Mind`:

```rust
    /// A scent of `player`'s hero at `place` (a tracker, spec §7.3): its suspicion of them held
    /// halfway from `check` to `alert`, so this step's drain leaves it checking, and the place to
    /// check moved to the scent, so it walks the trail. One already alerted keeps its chase.
    pub(crate) fn sniff(&mut self, player: PlayerId, place: V3, r: &AwarenessRules) {
        if self.is_alerted() {
            return;
        }
        let e = self.entry(player);
        e.level = e.level.max((r.check + r.alert) * 0.5);
        let moved = e.place.ground().distance(place.ground()) > 0.5;
        e.place = place;
        if let State::Checking { player: p, place: at } = &mut self.state {
            if *p == player {
                *at = place;
            }
        }
        if moved {
            self.there = false;
        }
    }
```

- [ ] **Step 3: Trails and trackers**

`crates/nightfall-sim/src/night/scent.rs`:

```rust
//! Scent (spec §7.1, §7.3): every hero who is up leaves a trail, a crumb every `scent.every`
//! yards walked, each fading after `scent.lasts` seconds; none in water or on a road, where the
//! trail breaks into a new stretch. A tracker that crosses a fresh crumb grows suspicious of its
//! maker; one suspicious of a hero follows their trail crumb by crumb toward newer ones. At a
//! break, or where the crumb it needs has faded, it has lost them, and its suspicion drains as
//! any enemy's does. A long detour does the same, since a tracker trots slower than a hero runs.

use std::collections::VecDeque;

use crate::combat::Hold;
use crate::senses::State;
use crate::{PlayerId, World, V2, V3};

/// One crumb of scent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crumb {
    pub(crate) at: V3,
    /// When it was left.
    pub(crate) t: f32,
    /// Which unbroken stretch of the trail it belongs to.
    pub(crate) stretch: u32,
}

/// One hero's trail, oldest crumb first.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trail {
    pub(crate) player: PlayerId,
    pub(crate) crumbs: VecDeque<Crumb>,
    stretch: u32,
    /// The hero stands where no scent is left (water, a road): the trail's end is a break.
    broken: bool,
}

impl Trail {
    /// Where a tracker at `at` goes next on this trail: the crumb after the newest one within
    /// `pick` yards of it, while both are of one stretch; that crumb itself when it is the
    /// trail's end and the trail is whole. `None` with no crumb near, or a break there.
    pub(crate) fn next_from(&self, at: V2, pick: f32) -> Option<V3> {
        let k = self
            .crumbs
            .iter()
            .rposition(|c| c.at.ground().distance(at) <= pick)?;
        match self.crumbs.get(k + 1) {
            Some(n) if n.stretch == self.crumbs[k].stretch => Some(n.at),
            Some(_) => None,
            None if !self.broken => Some(self.crumbs[k].at),
            None => None,
        }
    }
}

impl World {
    /// Every hero up leaves their scent; old crumbs fade.
    pub(crate) fn lay_scent(&mut self) {
        let content = self.content().clone();
        let s = content.rules.night.scent;
        let now = self.elapsed;
        let map = self.map().cloned();
        for h in &self.heroes {
            let i = match self.night.trails.iter().position(|t| t.player == h.player) {
                Some(i) => i,
                None => {
                    self.night.trails.push(Trail {
                        player: h.player,
                        ..Trail::default()
                    });
                    self.night.trails.len() - 1
                }
            };
            let t = &mut self.night.trails[i];
            if !h.fight.up() {
                continue;
            }
            let wet = map.as_ref().is_some_and(|m| m.is_water(h.pos));
            if wet || content.map.on_road(h.pos) {
                t.broken = true;
                continue;
            }
            let crumb = Crumb {
                at: V3::at(h.pos, h.z),
                t: now,
                stretch: t.stretch,
            };
            if t.broken {
                t.broken = false;
                t.stretch += 1;
                t.crumbs.push_back(Crumb {
                    stretch: t.stretch,
                    ..crumb
                });
            } else if t
                .crumbs
                .back()
                .is_none_or(|c| c.at.ground().distance(h.pos) >= s.every)
            {
                t.crumbs.push_back(crumb);
            }
        }
        for t in &mut self.night.trails {
            while t.crumbs.front().is_some_and(|c| now - c.t > s.lasts) {
                t.crumbs.pop_front();
            }
        }
    }

    /// Trackers in the fight pick up a fresh trail they cross, and follow the one of the hero
    /// they suspect.
    pub(crate) fn follow_scent(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.senses.awareness;
        let pick = content.rules.night.scent.pick;
        for i in 0..self.enemies.len().min(self.senses.minds.len()) {
            let (kind, hp, at) = {
                let e = &self.enemies[i];
                (e.kind, e.hp, e.pos)
            };
            if !content.enemies[kind].tracker || hp <= 0.0 {
                continue;
            }
            if self
                .combat
                .enemies
                .get(i)
                .is_some_and(|c| c.engage.hold == Hold::Out)
            {
                continue;
            }
            let mind = &mut self.senses.minds[i];
            match mind.state {
                State::Calm => {
                    // The newest crumb within reach, of any hero.
                    let found = self
                        .night
                        .trails
                        .iter()
                        .filter_map(|t| {
                            t.crumbs
                                .iter()
                                .filter(|c| c.at.ground().distance(at) <= pick)
                                .max_by(|a, b| a.t.total_cmp(&b.t))
                                .map(|c| (t.player, *c))
                        })
                        .max_by(|a, b| a.1.t.total_cmp(&b.1.t));
                    if let Some((player, c)) = found {
                        mind.sniff(player, c.at, r);
                    }
                }
                State::Wary { player, .. } | State::Checking { player, .. } => {
                    let next = self
                        .night
                        .trails
                        .iter()
                        .find(|t| t.player == player)
                        .and_then(|t| t.next_from(at, pick));
                    if let Some(next) = next {
                        mind.sniff(player, next, r);
                    }
                }
                _ => {}
            }
        }
    }
}
```

In `crates/nightfall-sim/src/night/mod.rs`: add `mod scent;` below `mod roll;`; add `#[cfg(test)] pub(crate) use scent::Crumb;`; `Night` gains:

```rust
    /// Each hero's scent trail.
    pub(crate) trails: Vec<scent::Trail>,
```

`step_night`'s `self.step_arrivals();` is followed by `self.lay_scent();`; and `night_holds`'s `self.step_hunt();` is preceded by `self.follow_scent();`.

In `crates/nightfall-sim/src/world.rs`'s `remove_hero`, add `self.night.trails.retain(|t| t.player != player);`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the six scent tests and every earlier one; no earlier content has a tracker, and a hero's trail touches nothing else). The tracker tests take a few seconds in a debug build.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: heroes leave scent that water and roads break; trackers follow it

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Mor'Ladim, who moves only unwatched; the banshee's wail

**Files:**
- Create: `crates/nightfall-sim/src/night/signature.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs` (`night_holds`), `crates/nightfall-sim/src/night/tests.rs`, `crates/nightfall-sim/src/combat/strikes.rs` (`spell_lands`)

**Interfaces:**
- Consumes: `Engage::hold`, `Hold::Out`, `within_arc`, `World::{spell_lands, kill_enemy, enemies_within, alert, hurt_hero}`, `Event::Damage`, `HeroFight::up` (combat Tasks 2–8); `Mind::{alert, is_alerted}` (senses Task 5); `HeightMap::line_of_sight`, `EYE_HEIGHT`, `CHEST_HEIGHT` (world knowledge Task 2); `EnemyDef::watched`, `NightRules::look_cone`, `Effect::Alarm` (Task 1); `World::fog`, `Event::Alarm` (Task 4).
- Produces:
  - Crate-only `World::{watch_morladim, alarm(&mut self, ei: usize, player: PlayerId, radius: f32)}`: a watched Mor'Ladim is out of the fight for the step (no move, no turn, no blow); unwatched, he is alerted to the nearest up hero within his `watched` range wherever they are.
  - The wail: when an enemy's cast with an `Alarm` effect lands on its hero (still in range and sight), every other enemy within its radius that is in the fight is alerted to that hero, without walls, without chaining; `Event::Alarm` once.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
const TOWARD: V2 = V2::new(1.0, 0.0);
const AWAY: V2 = V2::new(-1.0, 0.0);

/// `player` stands at `(x, y)` facing `facing` for `s` seconds.
fn face(w: &mut World, player: PlayerId, x: f32, y: f32, facing: V2, s: f32) {
    for _ in 0..secs(s) {
        put(w, player, x, y, facing, Gait::Still);
        w.step();
    }
}

#[test]
fn mor_ladim_moves_only_while_no_hero_looks_at_him() {
    let mut w = world("", &sentry("morladim", 60.0, 30.0, 180.0));
    join(&mut w, 0, 30.0, 30.0);
    face(&mut w, 0, 30.0, 30.0, TOWARD, 3.0);
    assert_eq!(w.enemies[0].pos, V2::new(60.0, 30.0), "watched, he stands");
    face(&mut w, 0, 30.0, 30.0, AWAY, 3.0);
    let d = w.enemies[0].pos.distance(V2::new(30.0, 30.0));
    assert!(d < 15.0, "unwatched, he came: {d}");
    let at = w.enemies[0].pos;
    face(&mut w, 0, 30.0, 30.0, TOWARD, 2.0);
    assert!(w.enemies[0].pos.distance(at) < 0.3, "looked at again, he stops");
}

#[test]
fn one_watcher_holds_him_while_a_friend_moves() {
    let mut w = world("", &sentry("morladim", 60.0, 30.0, 180.0));
    join(&mut w, 0, 30.0, 30.0);
    join(&mut w, 1, 30.0, 40.0);
    for k in 0..secs(4.0) {
        put(&mut w, 0, 30.0, 30.0, TOWARD, Gait::Still);
        let y = 40.0 + k as f32 * 0.1;
        put(&mut w, 1, 30.0, y, V2::new(0.0, 1.0), Gait::Walk);
        w.step();
    }
    assert_eq!(w.enemies[0].pos, V2::new(60.0, 30.0));
}

#[test]
fn a_wall_or_a_fallen_watcher_does_not_hold_him() {
    let mut m = flat(120.0);
    m.add_box(V2::new(44.0, 20.0), V2::new(46.0, 40.0), 0.0, 4.0);
    let mut w = world_on(m.finish(), "", &sentry("morladim", 60.0, 30.0, 180.0), "", standard);
    join(&mut w, 0, 30.0, 30.0);
    face(&mut w, 0, 30.0, 30.0, TOWARD, 4.0);
    assert_ne!(w.enemies[0].pos, V2::new(60.0, 30.0), "a wall between: not watched");
    // A downed hero facing him holds nothing; he goes after their friend, who looks away.
    let mut w = world("", &sentry("morladim", 60.0, 30.0, 180.0));
    join(&mut w, 0, 30.0, 30.0);
    join(&mut w, 1, 30.0, 80.0);
    w.hurt_hero(0, 100_000.0);
    for _ in 0..secs(3.0) {
        put(&mut w, 0, 30.0, 30.0, TOWARD, Gait::Still);
        put(&mut w, 1, 30.0, 80.0, AWAY, Gait::Still);
        w.step();
    }
    assert_ne!(w.enemies[0].pos, V2::new(60.0, 30.0), "a downed hero watches nothing");
}

#[test]
fn watched_he_never_strikes() {
    let mut w = world("", &sentry("morladim", 60.0, 30.0, 180.0));
    join(&mut w, 0, 58.5, 30.0);
    face(&mut w, 0, 58.5, 30.0, TOWARD, 6.0);
    let hits = |w: &mut World| {
        events_of(w, |e| match e {
            Event::Damage {
                to: crate::UnitRef::Hero(0),
                ..
            } => Some(()),
            _ => None,
        })
        .len()
    };
    assert_eq!(hits(&mut w), 0, "watched from a step away, he never swings");
    face(&mut w, 0, 58.5, 30.0, AWAY, 3.0);
    assert!(hits(&mut w) >= 1, "the moment nobody looks, he does");
}

/// A banshee at (30, 30), a brute 50 yards from her and another 70, all facing away from a hero
/// 10 yards in front of her; the hero is past both brutes' sight.
fn banshee_world(map: HeightMap) -> World {
    let spawns = format!(
        "{}{}{}",
        sentry("banshee", 30.0, 30.0, 180.0),
        sentry("brute", 80.0, 30.0, 0.0),
        sentry("brute", 30.0, 100.0, 90.0)
    );
    let mut w = world_on(map, "", &spawns, "", standard);
    join(&mut w, 0, 20.0, 30.0);
    w.drain_events();
    w
}

fn alarms(w: &mut World) -> Vec<(u32, PlayerId)> {
    events_of(w, |e| match e {
        Event::Alarm { enemy, player } => Some((*enemy, *player)),
        _ => None,
    })
}

#[test]
fn the_banshees_wail_reaches_sixty_yards_and_no_further() {
    let mut w = banshee_world(open());
    w.alert(w.enemies[0].id, 0);
    face(&mut w, 0, 20.0, 30.0, TOWARD, 3.0);
    assert_eq!(alarms(&mut w), vec![(1, 0)]);
    assert!(w.senses.minds[1].alerted_to(0), "the brute 50 yards off is after the hero");
    assert!(!w.senses.minds[2].is_alerted(), "the one 70 yards off heard nothing");
}

#[test]
fn breaking_her_sight_or_killing_her_stops_the_wail() {
    let mut m = flat(120.0);
    m.add_box(V2::new(24.0, 34.0), V2::new(26.0, 50.0), 0.0, 4.0);
    let mut w = banshee_world(m.finish());
    w.alert(w.enemies[0].id, 0);
    face(&mut w, 0, 20.0, 30.0, TOWARD, 0.5);
    // Behind the wall before her cast ends.
    face(&mut w, 0, 20.0, 44.0, TOWARD, 3.0);
    assert!(alarms(&mut w).is_empty());
    assert!(!w.senses.minds[1].is_alerted());
    let mut w = banshee_world(open());
    w.alert(w.enemies[0].id, 0);
    face(&mut w, 0, 20.0, 30.0, TOWARD, 0.5);
    w.kill_enemy(0, Some(0));
    face(&mut w, 0, 20.0, 30.0, TOWARD, 3.0);
    assert!(alarms(&mut w).is_empty());
    assert!(!w.senses.minds[1].is_alerted());
}
```

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL (Mor'Ladim walks while watched, or no `Alarm` event).

- [ ] **Step 2: Mor'Ladim and the alarm**

`crates/nightfall-sim/src/night/signature.rs`:

```rust
//! The signature monsters' ways (spec §7.3) the fight's rules do not already give: Mor'Ladim, who
//! stands while any hero looks at him and otherwise comes for the nearest; and the banshee's
//! wail, an enemy ability like any other whose `Alarm` effect wakes everything near. Stitches is
//! content (a road walk, telegraphed blows); the Dark Riders' eyes along the roads are the
//! senses'.

use crate::combat::{within_arc, Hold};
use crate::{Event, Hero, PlayerId, World, CHEST_HEIGHT, EYE_HEIGHT, V2, V3};

impl World {
    /// Whether `hero` looks at a unit at `at` (its floor `z`): up, within the fog, facing it
    /// within half of `cone` either side, and nothing solid between their eyes and its chest.
    fn looks_at(&self, hero: &Hero, at: V2, z: f32, fog: f32, cone: f32) -> bool {
        hero.fight.up()
            && hero.pos.distance(at) <= fog
            && within_arc(hero.facing, at - hero.pos, cone * 0.5)
            && self.map().is_none_or(|m| {
                m.line_of_sight(
                    V3::at(hero.pos, hero.z + EYE_HEIGHT),
                    V3::at(at, z + CHEST_HEIGHT),
                )
            })
    }

    /// Mor'Ladim's step: looked at by any hero, he is out of the fight (no step, no turn, no
    /// blow); else, unless something else holds him, he is after the nearest hero up within his
    /// range, seen or not.
    pub(crate) fn watch_morladim(&mut self) {
        let content = self.content().clone();
        let fog = self.fog().end;
        let cone = content.rules.night.look_cone;
        let r = &content.rules.senses.awareness;
        for i in 0..self.enemies.len().min(self.combat.enemies.len()) {
            let (kind, hp, at, z) = {
                let e = &self.enemies[i];
                (e.kind, e.hp, e.pos, e.z)
            };
            let Some(range) = content.enemies[kind].watched else {
                continue;
            };
            if hp <= 0.0 {
                continue;
            }
            if self.heroes.iter().any(|h| self.looks_at(h, at, z, fog, cone)) {
                self.combat.enemies[i].engage.hold = Hold::Out;
                self.enemies[i].speed = 0.0;
                continue;
            }
            if self.combat.enemies[i].engage.hold == Hold::Out {
                continue;
            }
            let nearest = self
                .heroes
                .iter()
                .filter(|h| h.fight.up() && h.pos.distance(at) <= range)
                .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
                .map(|h| (h.player, V3::at(h.pos, h.z)));
            if let (Some((player, spot)), Some(mind)) = (nearest, self.senses.minds.get_mut(i)) {
                mind.alert(player, spot, r);
            }
        }
    }

    /// Enemy `ei`'s wail landed on `player`'s hero (the banshee, spec §7.3): every other enemy
    /// within `radius` yards of her that is in the fight is after that hero where they stand,
    /// walls or not. None of them shouts on.
    pub(crate) fn alarm(&mut self, ei: usize, player: PlayerId, radius: f32) {
        let Some(at) = self.hero(player).map(|h| V3::at(h.pos, h.z)) else {
            return;
        };
        let content = self.content().clone();
        let r = &content.rules.senses.awareness;
        let from = self.enemies[ei].pos;
        for j in self.enemies_within(from, radius) {
            let out = self
                .combat
                .enemies
                .get(j)
                .is_some_and(|c| c.engage.hold == Hold::Out);
            if j == ei || out {
                continue;
            }
            if let Some(m) = self.senses.minds.get_mut(j) {
                if !m.is_alerted() {
                    m.alert(player, at, r);
                }
            }
        }
        self.events.push(Event::Alarm {
            enemy: self.enemies[ei].id,
            player,
        });
    }
}
```

In `crates/nightfall-sim/src/night/mod.rs`: add `mod signature;` below `mod scent;`, and `night_holds` becomes:

```rust
    pub(crate) fn night_holds(&mut self) {
        if self.night.mirror.is_some() || self.map().is_none() {
            return;
        }
        // Mor'Ladim first: a watched one is out of the fight for everything after.
        self.watch_morladim();
        self.follow_scent();
        self.step_hunt();
    }
```

In `crates/nightfall-sim/src/combat/strikes.rs`'s `spell_lands`, after the `if hit.is_some() { … }` block that wounds the hero, add:

```rust
        // A wail (spec §7.3) wakes everything near, when it landed on its hero.
        let alarm = self.content.abilities[a].effects.iter().find_map(|e| match e {
            Effect::Alarm { radius } => Some(*radius),
            _ => None,
        });
        if let (Some(_), Some(radius)) = (hit, alarm) {
            self.alarm(ei, player, radius);
        }
```

(`spell_lands` lands a cast only while its hero is up, in range and, past 8 yards, in sight: a hero who breaks the banshee's line of sight before her cast ends, or kills her (her cast goes with her), is never alarmed for.)

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the six new tests and every earlier one).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: Mor'Ladim moves only while nobody looks at him; the banshee's wail wakes everything within 60 yards

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The Dark Riders' eyes along the roads; Stitches on his road

**Files:**
- Modify: `crates/nightfall-sim/src/senses/react.rs` (`step_enemies`), `crates/nightfall-sim/src/night/tests.rs`

**Interfaces:**
- Consumes: `step_enemies(…, fog, notice)`, `sense(…, fog, notice, r)` (Task 4); `EnemyDef::road_sight`, `MapDef::on_road`, `Behaviour::Road` made a patrol (Task 1); stage newcomers (Task 5); telegraphs, `World::telegraphs` (combat Task 8).
- Produces: an enemy with `road_sight` notices a hero standing on a road as far as `road_sight` yards at visibility 100 (past the fog), its notice range times `road_sight / notice_range`; off the road, as any enemy. Stitches is content alone: a newcomer of the last stage on a road walk, fought with telegraphed blows.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
#[test]
fn a_rider_sees_far_along_its_road_and_not_off_it() {
    let marked = |kind: &str, y: f32| {
        let mut w = world("", &sentry(kind, 10.0, 60.0, 0.0));
        join(&mut w, 0, 60.0, y);
        for _ in 0..secs(3.0) {
            put(&mut w, 0, 60.0, y, AWAY, Gait::Run);
            w.step();
        }
        !w.marks_for(0).is_empty()
    };
    assert!(marked("rider", 60.0), "a runner on the road, 50 yards off, is seen");
    assert!(!marked("rider", 75.0), "the same runner off the road is past the fog");
    assert!(!marked("brute", 60.0), "a brute sees no further on a road");
}

#[test]
fn a_rider_rides_its_road_end_to_end_and_back() {
    let line = r#"(place: "Road", kind: "rider", behaviour: Road(road: "road")),"#;
    let mut w = world("", line);
    let (mut far, mut back, mut off) = (0.0f32, f32::MAX, 0.0f32);
    for k in 0..secs(30.0) {
        w.step();
        let p = w.enemies[0].pos;
        off = off.max((p.y - 60.0).abs());
        far = far.max(p.x);
        if k > secs(15.0) {
            back = back.min(p.x);
        }
    }
    assert!(far > 110.0, "it reached the far end: {far}");
    assert!(back < 20.0, "and came back: {back}");
    assert!(off <= 3.0, "it kept to the road: {off}");
}

#[test]
fn stitches_comes_with_the_last_stage_walks_his_road_and_telegraphs() {
    let night = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
        (key: "stitches", name: "Stitches", at: 2.0, warn: 1.0)], arrive_unseen: 30.0"#;
    let monsters = r#"(key: "stitches", name: "Stitches", always: true, spawns: [
        (place: "Road", kind: "stitches", stage: Some("stitches"), behaviour: Road(road: "road"))]),"#;
    let mut w = world_on(open(), night, "", monsters, standard);
    join(&mut w, 0, 60.0, 110.0);
    steps(&mut w, secs(1.5));
    assert!(w.enemies.is_empty(), "not before his stage");
    steps(&mut w, secs(2.0));
    assert_eq!(w.enemies.len(), 1);
    assert!(w.enemies[0].pos.distance(V2::new(1.0, 60.0)) < 1.0, "at his road's start");
    steps(&mut w, secs(10.0));
    assert!(w.enemies[0].pos.x > 15.0, "walking it: {:?}", w.enemies[0].pos);
    // Next to him, he fights with a blow shown on the ground first.
    let at = w.enemies[0].pos;
    put(&mut w, 0, at.x + 2.0, at.y, AWAY, Gait::Still);
    w.step();
    w.alert(w.enemies[0].id, 0);
    let mut shown = false;
    for _ in 0..secs(6.0) {
        put(&mut w, 0, at.x + 2.0, at.y, AWAY, Gait::Still);
        w.step();
        shown |= !w.telegraphs.is_empty();
    }
    assert!(shown, "his smash showed on the ground before it landed");
}
```

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL in `a_rider_sees_far_along_its_road_and_not_off_it` (the rider sees no further than anyone); the other two pass already (Stitches and the road walk are content on Tasks 1 and 5).

- [ ] **Step 2: The riders' eyes**

In `crates/nightfall-sim/src/senses/react.rs`'s `step_enemies`, the look-and-think loop's `let sensed: Vec<Sensed> = heroes.iter().map(|h| sense(e, mind, eye, h, &noises, &map, fog, notice, r)).collect();` becomes:

```rust
        // The Dark Riders see far along their roads (spec §7.3): a hero on a road, as far as
        // `road_sight` at visibility 100, past the fog.
        let far = content.enemies[e.kind].road_sight;
        let sensed: Vec<Sensed> = heroes
            .iter()
            .map(|h| {
                let (reach, scale) = if far > 0.0 && content.map.on_road(h.pos) {
                    (fog.max(far), notice * far / r.notice_range)
                } else {
                    (fog, notice)
                };
                sense(e, mind, eye, h, &noises, &map, reach, scale, r)
            })
            .collect();
```

and the module doc gains, after its first sentence: `An enemy with a road sight (the Dark Riders) sees a hero standing on a road much further, past the fog.`

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the three new tests and every earlier one).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the Dark Riders see far along their roads; Stitches walks his at the last stage

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The ravens: flocks that take off and are heard

**Files:**
- Create: `crates/nightfall-sim/src/night/ravens.rs`
- Modify: `crates/nightfall-sim/src/night/mod.rs` (`Night::{new, flocks}`, `night_view`, `step_night`), `crates/nightfall-sim/src/night/tests.rs`, `crates/nightfall-sim/src/world.rs` (`World::new`), `crates/nightfall-sim/src/raid/mod.rs` (`begin_raid`)

**Interfaces:**
- Consumes: `World::make_noise`, `Loudness::Ravens`, `Hero::stance`, `Stance::Running` (senses Tasks 2–4); `HeroFight::in_combat` (combat Task 5); `HeightMap::ground_at`; `RavenRules`, `MapDef::flocks` (Task 1); `Dice` (combat Task 2); `FLOCK_*`, `Event::RavensFlew` (Task 4).
- Produces: crate-only `night::ravens::Flock { state: u8, since: f32 }`; `Night::new(&Content, &RaidRoll) -> Night` (each flock out at `ravens.odds`, from the raid's seed); `Night::flocks`; `World::step_ravens`; `NightView::flocks` filled.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/night/tests.rs`:

```rust
const RAVENS: &str = "ravens: (odds: 1.0, back_after: 5.0, clear: 20.0)";

#[test]
fn ravens_take_off_when_a_hero_runs_near_and_are_heard() {
    // A brute 20 yards from the flock at (30, 30), facing away: it cannot see the hero.
    let mut w = world(RAVENS, &sentry("brute", 50.0, 30.0, 0.0));
    assert_eq!(w.night_view().flocks, vec![crate::FLOCK_SETTLED; 2]);
    join(&mut w, 0, 24.0, 30.0);
    for _ in 0..secs(1.0) {
        put(&mut w, 0, 24.0, 30.0, TOWARD, Gait::Walk);
        w.step();
    }
    assert_eq!(w.night_view().flocks[0], crate::FLOCK_SETTLED, "a walker passes them");
    w.drain_events();
    put(&mut w, 0, 24.0, 30.0, TOWARD, Gait::Run);
    w.step();
    assert_eq!(w.night_view().flocks[0], crate::FLOCK_FLOWN);
    let flew = events_of(&mut w, |e| match e {
        Event::RavensFlew { flock } => Some(*flock),
        _ => None,
    });
    assert_eq!(flew, vec![0]);
    assert!(w.senses.minds[0].level(0) > 0.0, "the brute heard them go");
    assert_eq!(w.night_view().flocks[1], crate::FLOCK_SETTLED, "the far flock stays");
}

#[test]
fn ravens_take_off_near_a_fight_and_come_back_later() {
    let mut w = world(RAVENS, &sentry("brute", 40.0, 40.0, 180.0));
    join(&mut w, 0, 38.0, 40.0);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 3);
    assert_eq!(w.night_view().flocks[0], crate::FLOCK_FLOWN, "a fight 14 yards off");
    put(&mut w, 0, 100.0, 100.0, TOWARD, Gait::Still);
    steps(&mut w, secs(4.0));
    assert_eq!(w.night_view().flocks[0], crate::FLOCK_FLOWN, "not back yet");
    steps(&mut w, secs(2.0));
    assert_eq!(w.night_view().flocks[0], crate::FLOCK_SETTLED, "back once nobody is near");
}

#[test]
fn each_raid_rolls_which_flocks_are_out() {
    let c = content("ravens: (odds: 0.5)", "", "");
    let flocks = |seed: u64| {
        crate::night::Night::new(&c, &RaidRoll::standard(&c, seed))
            .flocks
            .iter()
            .map(|f| f.state)
            .collect::<Vec<u8>>()
    };
    assert_eq!(flocks(5), flocks(5));
    let all: Vec<Vec<u8>> = (0..40).map(flocks).collect();
    assert!(all.iter().any(|f| f.contains(&crate::FLOCK_GONE)));
    assert!(all.iter().any(|f| f.contains(&crate::FLOCK_SETTLED)));
}
```

Run: `cargo test -p nightfall-sim night::`
Expected: FAIL to compile (`no function or associated item named new found for struct Night`).

- [ ] **Step 2: The flocks**

`crates/nightfall-sim/src/night/ravens.rs`:

```rust
//! The ravens (spec §7.1): an alarm on the ground. Each of the map's flocks is out in a raid at
//! `ravens.odds`; a settled flock takes off when a hero runs within `run_within` yards or fights
//! within `fight_within`, a 40-yard noise made by that hero, and settles again `back_after`
//! seconds later once no hero is within `clear` yards.

use crate::combat::Dice;
use crate::{Content, Event, Loudness, RaidRoll, Stance, World, FLOCK_FLOWN, FLOCK_GONE, FLOCK_SETTLED, V3};

use super::Night;

/// Mixed into the raid's seed for the flocks' dice.
const RAVEN_SEED: u64 = 0x0BAD_BEA7_C40F_FEE5;

/// One flock in this raid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flock {
    /// [`FLOCK_GONE`], [`FLOCK_SETTLED`] or [`FLOCK_FLOWN`].
    pub(crate) state: u8,
    /// When it last took off or settled.
    pub(crate) since: f32,
}

impl Night {
    /// The night a raid begins with: its flocks rolled from the raid's seed.
    pub(crate) fn new(content: &Content, roll: &RaidRoll) -> Self {
        let mut dice = Dice::new(roll.seed ^ RAVEN_SEED);
        let odds = content.rules.night.ravens.odds;
        Self {
            flocks: content
                .map
                .flocks
                .iter()
                .map(|_| Flock {
                    state: if dice.chance(odds) {
                        FLOCK_SETTLED
                    } else {
                        FLOCK_GONE
                    },
                    since: 0.0,
                })
                .collect(),
            ..Self::default()
        }
    }
}

impl World {
    /// Settled flocks take off from a running or fighting hero near them, heard; flown ones
    /// settle again in their time.
    pub(crate) fn step_ravens(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.night.ravens;
        let now = self.elapsed;
        for k in 0..self.night.flocks.len().min(content.map.flocks.len()) {
            let at = content.map.flocks[k];
            let f = self.night.flocks[k];
            if f.state == FLOCK_SETTLED {
                let by = self
                    .heroes
                    .iter()
                    .find(|h| {
                        let d = h.pos.distance(at);
                        h.fight.up()
                            && ((h.stance == Stance::Running && d <= r.run_within)
                                || (h.fight.in_combat && d <= r.fight_within))
                    })
                    .map(|h| (h.player, h.z));
                if let Some((player, z)) = by {
                    self.night.flocks[k] = Flock {
                        state: FLOCK_FLOWN,
                        since: now,
                    };
                    let ground = self.map().and_then(|m| m.ground_at(at)).unwrap_or(z);
                    self.make_noise(player, V3::at(at, ground), Loudness::Ravens);
                    self.events.push(Event::RavensFlew { flock: k as u8 });
                }
            } else if f.state == FLOCK_FLOWN
                && now - f.since >= r.back_after
                && !self.heroes.iter().any(|h| h.pos.distance(at) <= r.clear)
            {
                self.night.flocks[k] = Flock {
                    state: FLOCK_SETTLED,
                    since: now,
                };
            }
        }
    }
}
```

In `crates/nightfall-sim/src/night/mod.rs`:

- Add `mod ravens;` below `mod places;`.
- `Night` gains:

```rust
    /// Each of the map's flocks in this raid.
    pub(crate) flocks: Vec<ravens::Flock>,
```

- `night_view`'s `flocks: Vec::new(),` becomes `flocks: self.night.flocks.iter().map(|f| f.state).collect(),`.
- `step_night` gains `self.step_ravens();` after `self.lay_scent();` (before the enemies listen, so the flock's noise is heard this step).

In `crates/nightfall-sim/src/world.rs`'s `World::new`, `night: Night::default(),` becomes `night: Night::new(&content, &raid.roll),` (the `raid` binding is made before the literal, by the run loop's Task 3; `content` is `new`'s own parameter).

In `crates/nightfall-sim/src/raid/mod.rs`'s `begin_raid`, `self.night = crate::night::Night::default();` becomes `self.night = crate::night::Night::new(&content, &self.raid.roll);`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the three raven tests and every earlier one; the earlier tests' maps have no flocks, so nothing takes off in them).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: ravens settle in rolled flocks and take off, heard, from a runner or a fight

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The client: a game's own sounds, the night's fog and weather, its cues and lines, Stitches' tread, the roll planned

**Files:**
- Create (engine): `crates/benilla-app/src/sound/cue.rs`
- Modify (engine, three lines): `crates/benilla-app/src/sound/mod.rs`
- Create: `crates/benilla-app/src/nightfall/night.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (module, resource, systems), `crates/benilla-app/src/nightfall/session.rs` (`plan_raid`), `crates/benilla-app/src/nightfall/run.rs` (`Sim::hosted`, `start_run`), `crates/benilla-app/src/nightfall/enemies.rs` (`sync`'s mount)

**Interfaces:**
- Consumes: engine `sound::kit::{play_kit, play_file, KitRef, SoundCategory, SoundKits}`, `sound::{AudioListener, SoundConfig, SoundOutput}`, `benilla_assets::WorldAssets`, `benilla_world::weather::WeatherMessage`, `benilla_world::lighting::FogOverride`, `benilla_ui::script::UiScript::run`; client `run::Sim::{world, me, events}`, `run::raid_seed`, `session::plan_raid`, `enemies::{EnemyViews, sync}`, `units::{UnitSpec, wow_to_bevy}` (earlier plans); `World::{fog, night_view}`, `Event::{NightCue, ExitClosed, Alarm}`, `RaidRoll::roll`, `NightRules`, `WeatherDef`, `CueSound`, `StepSound`, `EnemyDef::{mount, step_sound}` (Tasks 1–10).
- Produces:
  - Engine: `crate::sound::{SoundCue, Cue}`: `SoundCue { sound: Cue, at: Option<Vec3> }` (a `Message`), `Cue::{Kit(u32), Name(String), File(String)}`; a kit plays from `at` (2D without), a file always 2D.
  - Client `night::{SentWeather, plan(&Content, u64, Option<&str>, Option<&str>) -> RaidRoll, plan_from_env(&Content, u64) -> RaidRoll, clock_at() -> Option<f32>, ease(f32, f32, f32) -> f32, chat_lua(&str) -> Option<String>, weather_message(Option<&WeatherDef>) -> WeatherMessage, cue_of(&CueSound) -> Cue, intel_lines(&RaidRoll, &Content) -> Vec<String>}`; systems `night::{fog, weather, clear_weather, cues, step_sounds}`.
  - Test hooks: `NIGHTFALL_WEATHER=<key>`, `NIGHTFALL_MONSTERS=<key>,…` (the planned raid's), `NIGHTFALL_NIGHT_AT=<seconds>` (a hosted raid's clock starts there).

- [ ] **Step 1: The engine's seam for a game's own sounds**

`crates/benilla-app/src/sound/cue.rs`:

```rust
//! A game's own sounds: a kit by SoundEntries id or name, or a file, from a place in the world or
//! everywhere at once. The Lua `PlaySound` seam ([`super::ui`]) is 2D only, and no server sends a
//! game that is its own server (Nightfall) the sounds of its world: a monster's tread or a
//! flock taking off must come from where it happens. A file plays 2D whatever `at` says, as the
//! client's `PlaySoundFile` does; a kit carries the distances its SoundEntries row gives.

use bevy::prelude::*;

use benilla_assets::WorldAssets;
use benilla_world::schedule::WorldStage;

use super::kit::{self, KitRef, SoundCategory, SoundKits};
use super::{AudioListener, SoundConfig, SoundOutput};

/// What a [`SoundCue`] plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Cue {
    Kit(u32),
    Name(String),
    File(String),
}

/// Play `sound`: from `at` (Bevy space) when given, else 2D. On the SFX slider.
#[derive(Message, Clone, Debug)]
pub(crate) struct SoundCue {
    pub(crate) sound: Cue,
    pub(crate) at: Option<Vec3>,
}

/// Every cue of the frame into the kit player; drained even with no catalog, so none waits.
fn play_cues(
    mut cues: MessageReader<SoundCue>,
    kits: Option<ResMut<SoundKits>>,
    assets: Option<Res<WorldAssets>>,
    mut out: NonSendMut<SoundOutput>,
    config: Res<SoundConfig>,
    listener: Res<AudioListener>,
) {
    let (Some(mut kits), Some(assets)) = (kits, assets) else {
        cues.clear();
        return;
    };
    for cue in cues.read() {
        let played = match &cue.sound {
            Cue::File(path) => kit::play_file(
                &mut kits,
                &assets,
                &mut out,
                &config,
                path,
                SoundCategory::Sfx,
            ),
            Cue::Kit(id) => kit::play_kit(
                &mut kits,
                &assets,
                &mut out,
                &config,
                listener.pos,
                KitRef::Id(*id),
                cue.at,
                SoundCategory::Sfx,
            )
            .map(|_| ()),
            Cue::Name(name) => kit::play_kit(
                &mut kits,
                &assets,
                &mut out,
                &config,
                listener.pos,
                KitRef::Name(name),
                cue.at,
                SoundCategory::Sfx,
            )
            .map(|_| ()),
        };
        if let Err(e) = played {
            debug!("sound(cue): {cue:?} — {e:#}");
        }
    }
}

/// Registration hook for [`super::SoundPlugin`].
pub(super) fn plugin(app: &mut App) {
    app.add_message::<SoundCue>()
        .add_systems(Update, play_cues.in_set(WorldStage::Present));
}
```

In `crates/benilla-app/src/sound/mod.rs`, these three lines and nothing else: `mod cue;` in the module list (after `mod creature;`), `pub(crate) use cue::{Cue, SoundCue};` beside the other `pub(crate) use` lines, and `cue::plugin(app);` in `SoundPlugin::build`'s list of plugin calls (after `creature::plugin(app);`).

Run: `cargo check -p benilla-app -j 2`
Expected: builds (a warning that `SoundCue` and `Cue` are never used goes away in Step 3).

- [ ] **Step 2: Write the client's pure parts with their tests**

`crates/benilla-app/src/nightfall/night.rs`:

```rust
//! The night on screen and in the ears (spec §7.2, §8.1): the fog the night has now, eased in as
//! it thickens; the raid's weather through the engine's own rain; each stage's cue and its line
//! in the chat, and no timer anywhere; a line when an exit closes or a banshee's wail lands;
//! Stitches' tread from where he walks; and the next raid planned from the roll, with the test
//! hooks' weather and monsters.

use std::collections::HashMap;

use benilla_ui::script::UiScript;
use benilla_world::lighting::FogOverride;
use benilla_world::weather::WeatherMessage;
use bevy::prelude::*;
use nightfall_sim::{Content, CueSound, Event, RaidRoll, WeatherDef};

use super::content;
use super::enemies::EnemyViews;
use super::run::Sim;
use crate::sound::{Cue, SoundCue};

/// Seconds the fog takes to come most of the way to the night's (a stage thickening it).
const FOG_EASE: f32 = 4.0;
/// The chat line's colour, WoW's emote orange.
const LINE_COLOR: (f32, f32, f32) = (1.0, 0.5, 0.25);

/// The next raid: [`RaidRoll::roll`] from `seed`, with the test hooks' weather (a key) and
/// monsters (keys, comma-separated, beside the ones that always come) when given. A key the
/// content lacks is left as rolled, with a warning.
pub(super) fn plan(
    content: &Content,
    seed: u64,
    weather: Option<&str>,
    monsters: Option<&str>,
) -> RaidRoll {
    let mut roll = RaidRoll::roll(content, seed);
    if let Some(key) = weather {
        match content.rules.night.weather.iter().position(|w| w.key == key) {
            Some(i) => roll.weather = i as u8,
            None => warn!("NIGHTFALL_WEATHER: no weather \"{key}\" in rules.ron"),
        }
    }
    if let Some(list) = monsters {
        let mut out: Vec<u8> = content
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| m.always)
            .map(|(i, _)| i as u8)
            .collect();
        for key in list.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            match content.monster(key) {
                Some(i) if !out.contains(&(i as u8)) => out.push(i as u8),
                Some(_) => {}
                None => warn!("NIGHTFALL_MONSTERS: no monster \"{key}\" in monsters.ron"),
            }
        }
        out.sort_unstable();
        roll.monsters = out;
    }
    roll
}

/// [`plan`] with the hooks from the environment: `NIGHTFALL_WEATHER`, `NIGHTFALL_MONSTERS`.
pub(super) fn plan_from_env(content: &Content, seed: u64) -> RaidRoll {
    plan(
        content,
        seed,
        std::env::var("NIGHTFALL_WEATHER").ok().as_deref(),
        std::env::var("NIGHTFALL_MONSTERS").ok().as_deref(),
    )
}

/// `NIGHTFALL_NIGHT_AT=<seconds>`: a hosted raid's clock starts there, to see the late stages.
pub(super) fn clock_at() -> Option<f32> {
    std::env::var("NIGHTFALL_NIGHT_AT")
        .ok()?
        .parse()
        .ok()
        .filter(|t: &f32| t.is_finite() && *t >= 0.0)
}

/// `cur` a frame of `dt` seconds nearer `target`, most of the way in [`FOG_EASE`]; on it once
/// within a hundredth.
pub(super) fn ease(cur: f32, target: f32, dt: f32) -> f32 {
    if (cur - target).abs() < 0.01 {
        return target;
    }
    cur + (target - cur) * (1.0 - (-dt / FOG_EASE).exp())
}

/// The Lua that puts `line` in the chat in emote orange: between long brackets, so a line
/// holding `]]` is refused (the content checks for it too).
pub(super) fn chat_lua(line: &str) -> Option<String> {
    let (r, g, b) = LINE_COLOR;
    (!line.contains("]]"))
        .then(|| format!("DEFAULT_CHAT_FRAME:AddMessage([[{line}]], {r}, {g}, {b})"))
}

/// The engine's weather for a raid's: its rain at its grade with its loop, or fine skies.
pub(super) fn weather_message(w: Option<&WeatherDef>) -> WeatherMessage {
    let rain = w.map_or(0.0, |w| w.rain);
    WeatherMessage {
        weather_type: if rain > 0.0 { 1 } else { 0 },
        grade: rain,
        sound_id: w.map_or(0, |w| w.sound),
        instant: false,
    }
}

/// The engine's cue for the content's.
pub(super) fn cue_of(c: &CueSound) -> Cue {
    match c {
        CueSound::Kit(id) => Cue::Kit(*id),
        CueSound::Name(n) => Cue::Name(n.clone()),
        CueSound::File(f) => Cue::File(f.clone()),
    }
}

/// What the map table says of a planned raid beside its exits (spec §8.5): the weather and the
/// signature monsters out.
pub(super) fn intel_lines(roll: &RaidRoll, content: &Content) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(w) = content.rules.night.weather.get(usize::from(roll.weather)) {
        lines.push(format!("Weather: {}", w.name));
    }
    let out: Vec<&str> = roll
        .monsters
        .iter()
        .filter_map(|&m| content.monsters.get(usize::from(m)))
        .map(|m| m.name.as_str())
        .collect();
    lines.push(if out.is_empty() {
        "Abroad: nothing of note".into()
    } else {
        format!("Abroad: {}", out.join(", "))
    });
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Content {
        Content::from_ron(nightfall_sim::ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
        })
        .unwrap()
    }

    #[test]
    fn the_hooks_set_the_weather_and_the_monsters() {
        let c = shipped();
        let rain = c.rules.night.weather.iter().position(|w| w.key == "rain").unwrap();
        let r = plan(&c, 3, Some("rain"), Some("morladim, banshee"));
        assert_eq!(usize::from(r.weather), rain);
        let mut want = vec![
            c.monster("morladim").unwrap() as u8,
            c.monster("banshee").unwrap() as u8,
            c.monster("stitches").unwrap() as u8,
        ];
        want.sort_unstable();
        assert_eq!(r.monsters, want);
        let rolled = RaidRoll::roll(&c, 3);
        let unknown = plan(&c, 3, Some("snow"), Some("dragon"));
        assert_eq!(unknown.weather, rolled.weather);
        assert_eq!(unknown.monsters, vec![c.monster("stitches").unwrap() as u8]);
        assert_eq!(plan(&c, 3, None, None), rolled);
    }

    #[test]
    fn the_fog_eases_toward_the_night_and_lands_on_it() {
        assert_eq!(ease(40.0, 30.0, 0.0), 40.0);
        let a = ease(40.0, 30.0, 1.0);
        assert!(a < 40.0 && a > 30.0);
        assert!(ease(a, 30.0, 1.0) < a);
        assert_eq!(ease(30.005, 30.0, 0.016), 30.0);
        assert!((ease(40.0, 30.0, 100.0) - 30.0).abs() < 0.01);
    }

    #[test]
    fn a_line_for_the_chat_is_lua_safe() {
        let l = chat_lua("A bell tolls in Darkshire.").unwrap();
        assert!(l.starts_with("DEFAULT_CHAT_FRAME:AddMessage([[A bell tolls in Darkshire.]]"));
        assert_eq!(chat_lua("nope]] os.exit()"), None);
    }

    #[test]
    fn rain_is_the_engines_rain_and_mist_is_not() {
        let c = shipped();
        let w = &c.rules.night.weather;
        let rain = weather_message(w.iter().find(|w| w.key == "rain"));
        assert_eq!((rain.weather_type, rain.sound_id), (1, 8534));
        assert!(rain.grade > 0.0);
        let mist = weather_message(w.iter().find(|w| w.key == "mist"));
        assert_eq!((mist.weather_type, mist.grade, mist.sound_id), (0, 0.0, 0));
        assert_eq!(weather_message(None).weather_type, 0);
    }

    #[test]
    fn the_map_table_names_the_weather_and_whats_abroad() {
        let c = shipped();
        let mist = c.rules.night.weather.iter().position(|w| w.key == "mist").unwrap() as u8;
        let roll = RaidRoll {
            weather: mist,
            monsters: vec![
                c.monster("morladim").unwrap() as u8,
                c.monster("stitches").unwrap() as u8,
            ],
            ..RaidRoll::standard(&c, 1)
        };
        assert_eq!(
            intel_lines(&roll, &c),
            vec!["Weather: Mist".to_string(), "Abroad: Mor'Ladim, Stitches".to_string()]
        );
    }
}
```

Add `mod night;` to `crates/benilla-app/src/nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::night -j 2`
Expected: PASS (5 tests).

- [ ] **Step 3: The systems**

Append to `night.rs`, above its tests:

```rust
/// The weather last given to the engine, so it is given once per change.
#[derive(Resource, Default)]
pub(super) struct SentWeather(Option<u8>);

/// The fog eases toward the night's (the weather's, thinned by Moonrise), so a stage's fog rolls
/// in rather than snapping.
pub(super) fn fog(sim: Res<Sim>, fog: Option<ResMut<FogOverride>>, real: Res<Time<Real>>) {
    let Some(mut f) = fog else {
        return;
    };
    let want = sim.world.fog();
    let dt = real.delta_secs();
    let start = ease(f.start, want.start, dt);
    let end = ease(f.end, want.end, dt);
    let color = [
        ease(f.color[0], want.color.0, dt),
        ease(f.color[1], want.color.1, dt),
        ease(f.color[2], want.color.2, dt),
    ];
    // Read first: only a change marks the override changed.
    if f.start != start || f.end != end || f.color != color {
        f.start = start;
        f.end = end;
        f.color = color;
    }
}

/// The raid's weather into the engine once, and again if a host's snapshot says otherwise.
pub(super) fn weather(
    sim: Res<Sim>,
    mut sent: ResMut<SentWeather>,
    mut out: MessageWriter<WeatherMessage>,
) {
    let w = sim.world.night_view().weather;
    if sent.0 == Some(w) {
        return;
    }
    sent.0 = Some(w);
    out.write(weather_message(
        content().rules.night.weather.get(usize::from(w)),
    ));
}

/// Out of the raid, fine skies again.
pub(super) fn clear_weather(mut sent: ResMut<SentWeather>, mut out: MessageWriter<WeatherMessage>) {
    if sent.0.take().is_some() {
        out.write(weather_message(None));
    }
}

/// The night's events of the frame, heard and read: a stage's cue and line, an exit closing, a
/// banshee's wail landing.
pub(super) fn cues(
    sim: Res<Sim>,
    script: Option<NonSend<UiScript>>,
    mut sounds: MessageWriter<SoundCue>,
) {
    let content = content();
    let say = |line: &str| {
        if let (Some(s), Some(lua)) = (script.as_deref(), chat_lua(line)) {
            let _ = s.run(&lua);
        }
    };
    for e in &sim.events {
        match e {
            Event::NightCue { stage } => {
                let Some(s) = content.rules.night.stages.get(usize::from(*stage)) else {
                    continue;
                };
                if let Some(c) = &s.cue {
                    sounds.write(SoundCue {
                        sound: cue_of(c),
                        at: None,
                    });
                }
                if let Some(l) = &s.line {
                    say(l);
                }
            }
            Event::ExitClosed { exit } => {
                if let Some(x) = content.map.exits.get(usize::from(*exit)) {
                    say(&format!("{} is shut for the rest of the night.", x.name));
                }
            }
            Event::Alarm { .. } => say("A banshee's wail carries across the hills."),
            _ => {}
        }
    }
}

/// An enemy with a tread (Stitches) is heard as it moves, within its `heard` yards of our hero:
/// from where it is drawn.
pub(super) fn step_sounds(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    bodies: Query<&Transform>,
    real: Res<Time<Real>>,
    mut next: Local<HashMap<u32, f32>>,
    mut sounds: MessageWriter<SoundCue>,
) {
    let content = content();
    let now = real.elapsed_secs();
    let me = sim.world.hero(sim.me).map(|h| h.pos);
    for e in &sim.world.enemies {
        let Some(step) = &content.enemies[e.kind].step_sound else {
            continue;
        };
        let near = me.is_some_and(|m| m.distance(e.pos) <= step.heard);
        if e.speed <= 0.0 || e.hp <= 0.0 || !near {
            continue;
        }
        let due = next.entry(e.id).or_insert(now);
        if now < *due {
            continue;
        }
        *due = now + step.every;
        let at = views
            .drawn
            .get(&e.id)
            .and_then(|&b| bodies.get(b).ok())
            .map(|t| t.translation);
        if at.is_some() {
            sounds.write(SoundCue {
                sound: cue_of(&step.sound),
                at,
            });
        }
    }
}
```

In `crates/benilla-app/src/nightfall/mod.rs`, `NightfallPlugin::build` gains `.init_resource::<night::SentWeather>()` and:

```rust
            .add_systems(
                Update,
                (night::fog, night::weather, night::cues, night::step_sounds)
                    .after(run::step_sim)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(OnExit(Phase::Playing), night::clear_weather)
```

In `crates/benilla-app/src/nightfall/session.rs`, `plan_raid`'s last line `RaidRoll::standard(&content(), seed)` becomes `super::night::plan_from_env(&content(), seed)`, and its doc `The next raid as the night rolls it from a fresh seed (NIGHTFALL_SEED for a test run; NIGHTFALL_WEATHER and NIGHTFALL_MONSTERS for its weather and monsters).`

In `crates/benilla-app/src/nightfall/run.rs`:

- `start_run`'s fallback `RaidRoll::standard(&content(), raid_seed(&time))` becomes `super::night::plan_from_env(&content(), raid_seed(&time))`.
- In `Sim::hosted`, right after the world begins its raid (the `begin_raid(roll)` call), add:

```rust
        // A test hook: the clock starts late, to see the night's last stages (the host's alone;
        // a joined client's night is what the host sends).
        if let Some(t) = super::night::clock_at() {
            world.elapsed = t;
        }
```

  naming `hosted`'s own binding for the world where it says `world`.

In `crates/benilla-app/src/nightfall/enemies.rs`, add:

```rust
/// `UNIT_FIELD_MOUNTDISPLAYID`: the mount a unit sits on, the only mounted signal.
const FIELD_MOUNT_DISPLAY: u16 = 133;

/// The extra fields of an enemy that rides `mount` (a `CreatureDisplayInfo` id; 0 for none).
pub(super) fn mount_fields(mount: u32) -> Vec<(u16, u32)> {
    if mount == 0 {
        Vec::new()
    } else {
        vec![(FIELD_MOUNT_DISPLAY, mount)]
    }
}
```

with a test beside the module's others:

```rust
    #[test]
    fn a_rider_sits_on_its_mount_and_a_walker_on_nothing() {
        assert_eq!(mount_fields(2404), vec![(133, 2404)]);
        assert!(mount_fields(0).is_empty());
    }
```

and in `sync`, before its `units::spawn_unit(&mut commands, &UnitSpec { … })` call add `let extra = mount_fields(def.mount);`, with the literal's `extra: &[],` becoming `extra: &extra,`.

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (the night module's 5 tests, the mount test, and the earlier ones).
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds with no new warnings in `nightfall/` or `sound/`.

- [ ] **Step 5: See the night in the game**

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_SEED=7 NIGHTFALL_WEATHER=mist NIGHTFALL_NIGHT_AT=815 WOW_LIVE_SHOT=target/night-moonrise.png WOW_LIVE_SHOT_AT=35 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=8 ./target/release/azeroth-together.exe > target/night-moonrise.log 2>&1 &
```

Expected, once `target/night-moonrise-003.png` exists (then `kill` the PID): the raid starts in mist (the fog close and pale from the first shot); its clock starts at 13:35, so 5 seconds into the raid the chat shows "A bell tolls in Darkshire. The moon is rising, and the mist with it." in orange, and 20 seconds after that Moonrise lands: by the last shot the fog has closed further (mist's 28 yards to 21); no timer anywhere on screen. `grep -c "sound(cue)" target/night-moonrise.log` prints `0` with the log at its default level (a missing cue sound is a debug line; run once with `RUST_LOG=benilla_app::sound=debug` and, if a line names the bell, put another path into `rules.ron` as Task 1's Step 6 found).

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_SEED=7 NIGHTFALL_WEATHER=rain WOW_LIVE_SHOT=target/night-rain.png WOW_LIVE_SHOT_AT=40 ./target/release/azeroth-together.exe > target/night-rain.log 2>&1 &
```

Expected, once `target/night-rain.png` exists (then `kill` the PID): rain streaks fall around the hero, the fog is at the rain's 32 yards, and the log has `weather sound kit 8534`.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/sound/cue.rs crates/benilla-app/src/sound/mod.rs crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the night's fog and weather on screen, its cues heard and its lines read; a game's own sounds in the engine

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: The client: ravens on the ground, and the map table's intel

**Files:**
- Create: `crates/benilla-app/src/nightfall/ravens.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs`, `crates/benilla-app/src/nightfall/board.rs` (the board's rebuild)

**Interfaces:**
- Consumes: `units::{spawn_unit, UnitSpec, UnitKind, ground_to_bevy, yaw_of, FACTION_FRIENDLY}`, `NameCache::insert_creature`, `CreatureRecord`, `crate::target::Concealed`, `benilla_world::world_point::WorldPoint::terrain_height_under`, `board`'s rebuild and `intel_marks`, `Net::planned_raid`, `Profile::inn.intel` (earlier plans); `NightView::flocks`, `FLOCK_SETTLED`, `Event::RavensFlew`, `RavenRules`, `MapDef::flocks` (Tasks 1, 4, 10); `SoundCue`, `night::{cue_of, intel_lines}` (Task 11).
- Produces: `ravens::{Flocks, bird_spots(V2, usize) -> Vec<V2>}`; systems `ravens::{sync, cries}`; the raid board's map table lists the weather and the monsters out after the exits.

- [ ] **Step 1: Write the pure part with its test**

`crates/benilla-app/src/nightfall/ravens.rs`:

```rust
//! The ravens on screen (spec §7.1): each settled flock near our hero drawn as a few birds on the
//! ground, gone with a cry from where they sat when the flock takes off. The birds are a stand-in
//! look until a crow's display is found in the data (measure: rules.ron's night.ravens.display);
//! they are `Concealed`, so neither Tab, the mouse nor a nameplate finds them.

use std::collections::HashMap;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{Event, FLOCK_SETTLED, V2};

use super::night::cue_of;
use super::run::Sim;
use super::units::{self, UnitKind, UnitSpec};
use super::content;
use crate::names::{CreatureRecord, NameCache};
use crate::sound::SoundCue;
use crate::target::Concealed;

/// The creature entry birds are filed under in the name cache: past every enemy kind's.
const RAVEN_ENTRY: u32 = 60_000;
/// Guid serials for birds: past every enemy id.
const RAVEN_SERIAL: u64 = 0x00F0_0000;
/// How far around a flock's point its birds sit, yards.
const SPREAD: f32 = 1.2;

/// The birds drawn, by flock.
#[derive(Resource, Default)]
pub(super) struct Flocks {
    drawn: HashMap<u8, Vec<Entity>>,
    named: bool,
}

/// Where `n` birds of a flock at `at` sit: on a small circle round it, the same every time.
pub(super) fn bird_spots(at: V2, n: usize) -> Vec<V2> {
    (0..n)
        .map(|i| {
            at + V2::from_angle(
                0.7 + i as f32 * std::f32::consts::TAU / n.max(1) as f32,
                SPREAD,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birds_sit_round_their_flocks_point() {
        let at = V2::new(10.0, 20.0);
        let spots = bird_spots(at, 3);
        assert_eq!(spots.len(), 3);
        assert!(spots.iter().all(|p| (p.distance(at) - SPREAD).abs() < 1e-4));
        assert_ne!(spots[0], spots[1]);
        assert_eq!(bird_spots(at, 3), spots);
        assert!(bird_spots(at, 0).is_empty());
    }
}
```

Add `mod ravens;` to `crates/benilla-app/src/nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::ravens -j 2`
Expected: PASS.

- [ ] **Step 2: The birds and their cry**

Append to `ravens.rs`, above its tests:

```rust
/// Each settled flock within the view distance of our hero has its birds; one that took off, or
/// fell out of view, has none.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut flocks: ResMut<Flocks>,
    mut names: ResMut<NameCache>,
    world_point: WorldPoint,
) {
    let content = content();
    let r = &content.rules.night.ravens;
    if !flocks.named {
        names.insert_creature(
            RAVEN_ENTRY,
            Some(CreatureRecord {
                name: "Raven".into(),
                subname: None,
                creature_type: 0,
                pet_family: 0,
                rank: 0,
                type_flags: 0,
                civilian: true,
                racial_leader: false,
                display_id: r.display,
            }),
        );
        flocks.named = true;
    }
    let view = content.rules.atmosphere.view_distance;
    let me = sim.world.hero(sim.me).map(|h| (h.pos, h.z));
    let states = sim.world.night_view().flocks;
    for (k, &at) in content.map.flocks.iter().enumerate() {
        let key = k as u8;
        let want = me.is_some_and(|(m, _)| m.distance(at) <= view)
            && states.get(k) == Some(&FLOCK_SETTLED);
        let drawn = flocks.drawn.contains_key(&key);
        if want && !drawn {
            let z = me.map_or(0.0, |(_, z)| z);
            let birds: Vec<Entity> = bird_spots(at, usize::from(r.birds))
                .into_iter()
                .enumerate()
                .map(|(b, p)| {
                    let mut pos = units::ground_to_bevy(p, z + 50.0);
                    pos.y = world_point.terrain_height_under(pos).unwrap_or(z);
                    let e = units::spawn_unit(
                        &mut commands,
                        &UnitSpec {
                            kind: UnitKind::Creature { entry: RAVEN_ENTRY },
                            serial: RAVEN_SERIAL + u64::from(key) * 16 + b as u64,
                            display: r.display,
                            scale: r.scale,
                            level: 1,
                            faction: units::FACTION_FRIENDLY,
                            bytes0: 1 << 8,
                            weapons: &[],
                            sheath: 0,
                            extra: &[],
                        },
                        pos,
                        units::yaw_of(p - at),
                    );
                    commands.entity(e).insert(Concealed);
                    e
                })
                .collect();
            flocks.drawn.insert(key, birds);
        } else if !want && drawn {
            for e in flocks.drawn.remove(&key).unwrap_or_default() {
                commands.entity(e).despawn();
            }
        }
    }
}

/// A flock taking off cries from where it sat.
pub(super) fn cries(sim: Res<Sim>, mut sounds: MessageWriter<SoundCue>) {
    let content = content();
    let Some(cry) = &content.rules.night.ravens.cry else {
        return;
    };
    let z = sim.world.hero(sim.me).map_or(0.0, |h| h.z);
    for e in &sim.events {
        if let Event::RavensFlew { flock } = e {
            if let Some(&at) = content.map.flocks.get(usize::from(*flock)) {
                sounds.write(SoundCue {
                    sound: cue_of(cry),
                    at: Some(units::ground_to_bevy(at, z)),
                });
            }
        }
    }
}
```

`units::spawn_unit` adds the run's `NightfallEntity` marker, so the birds go with the run like every other unit; `Flocks` starts over with the next run: in `run.rs`'s `back_to_menu`, beside its other resets, add `commands.insert_resource(super::ravens::Flocks::default());` (the despawn there already took the birds).

In `crates/benilla-app/src/nightfall/mod.rs`, `NightfallPlugin::build` gains `.init_resource::<ravens::Flocks>()`, and Task 11's night systems tuple becomes `(night::fog, night::weather, night::cues, night::step_sounds, ravens::sync, ravens::cries)`. If `units::FACTION_FRIENDLY` carries an `#[allow(dead_code)]` it can lose it now.

- [ ] **Step 3: The map table's intel**

In `crates/benilla-app/src/nightfall/board.rs`, in the board's rebuild, inside `if p.inn.intel { if let Some(roll) = net.planned_raid() { … } }`, after the `for l in exit_list(&intel_marks(&roll), &content).lines() { … }` loop add:

```rust
                // The night's roll (spec §8.5's map table): the weather and who is abroad.
                for l in super::night::intel_lines(&roll, &content) {
                    lines.push(format!("  {l}"));
                }
```

and the module doc's "the map table's intel when it is built (which exits open when, from the raid the host plans)" becomes "the map table's intel when it is built (which exits open when, the weather and the signature monsters out, from the raid the host plans)".

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 5: See the ravens and the intel**

The shipped map has two flocks near the first entry. A run whose autoplay runs past them:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_GAIT=run NIGHTFALL_SEED=11 WOW_LIVE_SHOT=target/night-ravens.png WOW_LIVE_SHOT_AT=32 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=3 ./target/release/azeroth-together.exe > target/night-ravens.log 2>&1 &
```

Expected, once `target/night-ravens-005.png` exists (then `kill` the PID): in an early shot small birds sit on the ground near the hero (the stand-in look); in a later one they are gone, and enemies nearby show "?" as they heard them (the 40-yard noise). Tab never selects a bird. If no flock is out at seed 11 (each is out at 70%), try `NIGHTFALL_SEED=12`.

A test stash with the map table built, to read the intel in the inn; the autoplay walks the hero through the inn's door on its way to the raid, and the board shows while it stands in the door's ring:

```bash
mkdir -p target/nf-home-intel/nightfall
cat > target/nf-home-intel/nightfall/stash.ron <<'EOF'
(version: 3, class: "rogue", gold: 0, stash: [], inn: (intel: true, stations: [("map_table", 1)]))
EOF
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-intel WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_SEED=7 NIGHTFALL_MONSTERS=morladim NIGHTFALL_WEATHER=mist WOW_LIVE_SHOT=target/night-intel.png WOW_LIVE_SHOT_AT=10 WOW_LIVE_SHOT_COUNT=10 WOW_LIVE_SHOT_EVERY=2 ./target/release/azeroth-together.exe > target/night-intel.log 2>&1 &
```

Expected, once `target/night-intel-009.png` exists (then `kill` the PID): in the shots taken while the hero stands in the door's ring, the raid board's map table lists the exits, then `Weather: Mist` and `Abroad: Mor'Ladim, Stitches`; the later shots are in the raid, in mist.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: ravens on the ground that cry as they take off; the map table names the weather and who is abroad

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Docs, the full check, the night's monsters on screen, and co-op

**Files:**
- Modify: `CLAUDE.md` (the Nightfall section), `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line), `scripts/package.sh` (only if it copies `content/` file by file)

- [ ] **Step 1: Write the docs**

In `CLAUDE.md`'s Nightfall section, add:

```markdown
- Nightfall's night (sub-project 6): `rules.ron`'s `night:` has the clock's stages (each cued
  `warn` seconds before it lands, with a chat line; Moonrise's `fog` and `notice`; the hunt), the
  weathers and their weights, the roll's chances for exits (shut, late, the hunt's closing),
  `monsters_out`, scent, the hunt and the ravens. `map.ron` has the risk `zones` (health, blows,
  extra loot rolls), the `roads` and the flocks of `ravens`; `monsters.ron` the signature
  monsters (odds, `always`, their spawn lines; `stage:` on a line makes it come when that stage
  lands, out of sight). Enemy traits in `enemies.ron`: `watched` (Mor'Ladim), `tracker`,
  `hunter`, `road_sight` (the Dark Riders), `mount`, `step_sound`; the banshee's wail is an
  ability with `Alarm(radius)`. `nightfall-sim/src/night/` is the rules (`RaidRoll::roll`, the
  clock, arrivals, the hunt, scent, Mor'Ladim, the alarm, ravens); the height map knows water from
  the ADT liquids (`BAKE_VERSION` 2). The client draws the fog and weather from each snapshot's
  `NightView` and plays cues through `sound::SoundCue` (the engine's one new file). Test hooks:
  `NIGHTFALL_WEATHER=<key>`, `NIGHTFALL_MONSTERS=<key>,…` (the planned raid's),
  `NIGHTFALL_NIGHT_AT=<seconds>` (a hosted raid's clock starts there). Positions, displays and
  sounds marked `measure:` are stand-ins until checked in the game.
```

In the spec's status line, after the sentence about sub-project 5, add: `Sub-project 6 built (plan docs/superpowers/plans/2026-10-17-nightfall-night-threats.md).`

If `scripts/package.sh` copies `content/` file by file rather than whole, add `monsters.ron` to its Nightfall list; if it copies the folder, leave it.

- [ ] **Step 2: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS (the Darkshire bake test skips without `WOW_DATA`).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds with no new warnings in `nightfall-sim`, `nightfall-bake`, `nightfall/` or `sound/`.

- [ ] **Step 3: The signature monsters near the entry**

A scratch copy of the content (never in `content/`) that brings Mor'Ladim, the banshee, a Dark Rider and Stitches next to the first entry, and lets newcomers come in sight (`arrive_unseen` 0):

```bash
rm -rf target/nf-night && cp -r content/nightfall target/nf-night
sed -i 's/arrive_unseen: 50.0/arrive_unseen: 0.0/' target/nf-night/rules.ron
cat > target/nf-night/monsters.ron <<'EOF'
// Test only: every signature monster within sight of the first entry (-10051.8, -792.5).
[
    (key: "morladim", name: "Mor'Ladim", odds: 1.0, spawns: [
        (place: "Test", kind: "morladim", behaviour: Sentry(at: (-10030.0, -792.5), facing: 180.0))]),
    (key: "banshee", name: "A banshee", odds: 1.0, spawns: [
        (place: "Test", kind: "wailing_banshee", behaviour: Sentry(at: (-10060.0, -770.0), facing: 0.0))]),
    (key: "dark_riders", name: "The Dark Riders", odds: 1.0, spawns: [
        (place: "Test", kind: "dark_rider", behaviour: Road(road: "north_road"))]),
    (key: "stitches", name: "Stitches", always: true, spawns: [
        (place: "Test", kind: "stitches", stage: Some("stitches"),
         behaviour: Sentry(at: (-10070.0, -805.0), facing: 45.0))]),
]
EOF
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home NIGHTFALL_CONTENT=target/nf-night WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_SEED=7 NIGHTFALL_MONSTERS=morladim,banshee,dark_riders NIGHTFALL_NIGHT_AT=1540 WOW_SHOT_REQUIRE="Mor'Ladim" WOW_LIVE_SHOT=target/night-monsters.png WOW_LIVE_SHOT_AT=35 WOW_LIVE_SHOT_COUNT=10 WOW_LIVE_SHOT_EVERY=3 ./target/release/azeroth-together.exe > target/night-monsters.log 2>&1 &
```

Expected, once `target/night-monsters-009.png` exists (then `kill` the PID):
- Mor'Ladim (a large skeleton, the stand-in) stands still in the shots where the hero faces him and is nearer in those where the autoplay has turned away; he never swings while faced.
- The banshee shows a cast bar on the target frame and the chat reads "A banshee's wail carries across the hills." once she has seen the hero; enemies nearby show "!".
- A Dark Rider passes along the north road at speed (and, once a horse display is measured, mounted).
- The clock starts at 25:40, past Stitches' cue (30 s before 26:00): the chat reads "Something heavy is walking the road from Raven Hill." as the raid begins, and 20 seconds in Stitches (a huge stand-in) stands at his test post; a red circle shows on the ground before his smash once he is after the hero.
- `grep -c "panicked" target/night-monsters.log` prints `0`.

Note in the rulings' display line which stand-ins look wrong enough to fix first.

- [ ] **Step 4: The night in co-op on one PC**

Start the host in the background, and a few seconds later the client:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-host WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_NAME=Host NIGHTFALL_SEED=7 NIGHTFALL_WEATHER=mist NIGHTFALL_NIGHT_AT=830 WOW_LIVE_SHOT=target/host-night.png WOW_LIVE_SHOT_AT=45 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=6 ./target/release/azeroth-together.exe > target/host-night.log 2>&1 &
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-client WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend WOW_LIVE_SHOT=target/client-night.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=5 WOW_LIVE_SHOT_EVERY=6 ./target/release/azeroth-together.exe > target/client-night.log 2>&1 &
```

Expected, once both last shots exist (then `kill` both PIDs): both clients show the host's mist and, after Moonrise lands, the same closer fog; the client's chat shows the Moonrise line too (the events reach every player); neither log has a panic, and the client's log has no protocol refusal (both are protocol 7).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md scripts/package.sh docs/superpowers/plans/2026-10-17-nightfall-night-threats-rulings.md
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: document the night's threats; sub-project 6 built

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review against the spec

| Spec | Where |
|---|---|
| §7.2 the night deepens in stages at Dusk 0:00, the howling 8:00, Moonrise 14:00, the hunt 20:00, Stitches 26:00 (`night_stages` in `rules.ron`) | Task 1 (`NightRules::stages`, the spec's times as defaults and in `rules.ron`), Task 4 (the clock) |
| §7.2 each stage announced by sound before it lands; no timer on screen | Task 4 (`NightCue` `warn` seconds before, once), Task 11 (the cue's sound and chat line; nothing on screen counts down) |
| §7.2 the howling: worgen howls; trackers join patrols, more wanderers | Task 1 (the howling's cue; trackers' and wanderers' spawn lines with `stage: Some("howling")`), Task 5 (newcomers per stage), Task 7 (trackers) |
| §7.2 Moonrise: fog end 40 → 30 yd, notice range +20% | Task 1 (`fog: 0.75`, `notice: 1.2`), Task 4 (`fog_of`, `notice_scale` in the senses and sight), Task 11 (the fog eased in on screen) |
| §7.2 the hunt: worgen packs hunt toward the loudest recent noise; one exit may close | Task 6 (`hunter`, the loudest noise of the last 30 s), Task 3 (`closes`), Task 4 (`ExitClosed`), Task 11 (its line) |
| §7.2 Stitches walks the road from Raven Hill to Darkshire: too strong to fight, loud, telegraphed swings | Task 1 (20 000 hp elite, telegraphed cleave and smash, `step_sound`, `raven_hill_road`), Task 9 (he comes and walks it), Task 11 (his tread heard) |
| §7.2 exits that open late pull the other way | Task 3 (late exits within `late_window`, the cart near `cart_opens_at`) |
| §7.3 Mor'Ladim walks only while no hero looks at him | Task 8 (`watch_morladim`: out of the fight while looked at; after the nearest hero otherwise) |
| §7.3 Nightbane trackers follow scent, broken by water, roads or a long detour | Task 2 (water baked), Task 7 (trails, breaks, fading; trackers slower than a running hero) |
| §7.3 a banshee screams when she sees a hero, alerting everything in 60 yd | Task 1 (`banshee_wail` with `Alarm(radius: 60.0)`, ready at once), Task 8 (`alarm`; broken sight or her death stops it) |
| §7.3 the Dark Riders ride the roads at speed, see far along them | Task 1 (`Road` walks, 10 yd/s, `road_sight: 70`), Task 9 (their eyes) |
| §7.3 each raid rolls which are out | Task 3 (`pick_monsters`), Task 5 (only those out are placed) |
| §8.1 risk zones (Raven Hill, the crypts, Vul'Gol) with more, tougher enemies and the best loot; outskirts gentler; death the same everywhere | Task 1 (`zones` in `map.ron`), Task 5 (health, blows and loot rolls by zone; `death_loss` untouched) |
| §8.1 every raid a little different: loot, open exits and times, weather (clear, mist, rain shortening the fog), monsters out; the map the same | Task 3 (`RaidRoll::roll`; the loot's seed unchanged), Task 4 (the weather's fog), Task 11 (the engine's rain) |
| §7.1 ravens: take off when a hero runs within 8 yd or fights within 15 yd, a 40 yd noise | Task 10, Task 12 (drawn, their cry) |
| §8.5 the map table: which signature monsters are out, which exits open | Task 12 (`intel_lines` beside 5b's exits) |
| §10 later: rain that shortens the fog further | Task 1 (the rain's 32-yard fog), Task 11 |
| §11 `night_stages` | Task 1 |
| §12 tests: sim unit tests, determinism, shipped content parses and plays | Tasks 1–10 (`night::tests`, the roll's seeds), Task 1 (`the_night_is_in_the_shipped_content`) |
| §2 sub-project 7 (rivals) out | Not here |
| §8.1 Darkshire's guards (monsters stay out; they fight Stitches) | Not here: see the rulings |
