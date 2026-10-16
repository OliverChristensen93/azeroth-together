# Nightfall Sub-project 3 (Senses) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sneaking past and being spotted: every hero has a visibility (from how they move, stealth and light) and makes noise; every enemy keeps a suspicion of each hero, turns to look, goes to check, chases when alerted, shouts to its allies and searches when it loses its hero; the client shows the eye, the noise rings, "?" and "!" over enemies, a vignette that tightens as the hero hides and a heartbeat when an alerted enemy is near, and keeps every enemy the hero does not see out of Tab, the mouseover and the plates.

**Architecture:** `nightfall-sim` gains a `senses` module: pure functions for visibility, facing and notice range (`visibility.rs`), noise and hearing (`noise.rs`), an enemy's state of mind (`awareness.rs`), and the step that lets enemies see, hear, think and act on the height map (`react.rs`); `World` gains each hero's stance, visibility and noise, a minimal Rogue Stealth toggle, per-player marks and the seams combat will use (ambush, openers, alerts, noises, flashes, breaking stealth). The client reports its hero's gait (WoW's walk toggle) and Stealth presses in `PlayerInput`, marks unseen enemies `Concealed`, gives stealthed heroes WoW's own stealth look, and draws the marks, the eye and rings, the heartbeat and the vignette. The engine gets one new marker (`target::Concealed`) that Tab, the mouseover and both plate kinds respect, and the plates' missing stealth bit.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content), postcard (wire), WGSL (the night grade).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md`: §5 in full; §3.4's client parts (`Concealed`, plates); §4.3's client half; §7.1's sleepers; §10's vignette and heartbeat; §11's `notice_range` and `stealth_speed`; §12's tests. Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-senses-rulings.md`. This plan starts after every task of `docs/superpowers/plans/2026-10-16-nightfall-foundation.md` and `docs/superpowers/plans/2026-10-17-nightfall-world-knowledge.md` is done, and edits the files those plans wrote; every name it consumes from them is the one those plans give.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git config --global --add safe.directory D:/Games/azeroth-together-stealth` once, or `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`.
- The worktree has no `WoW/Data`: every game launch sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data` and `BENILLA_HOME=target/nf-home` (where sub-project 2's height-map cache lives). The exe does not exit after its shots: start it in the background, wait for the last shot file, then `kill` its PID.
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats` (spec §3.1). The senses are pure sim logic, unit-tested on hand-made maps (`heightmap::testmap`).
- Every number of the senses is a setting in `content/nightfall/rules.ron` under `senses:` (spec §5's tables, §11's `notice_range` and `stealth_speed`), with the spec's value as its default when left out.
- The client decides nothing about the game (spec §3.1): it reports gait and Stealth presses; the host decides stance, visibility, speed, noise, awareness and marks.
- Engine changes are only these, `#[cfg]`-free and small: a new file `crates/benilla-app/src/target/concealed.rs`, two lines in `target/mod.rs`, the unit query's filter in `target/scan.rs` (plus a test), one field and one check in `target/hover.rs`, and the plate gate in `nameplates.rs` and `vplates.rs`.
- Combat (spec §6) is sub-project 4: enemies chase and face their hero but never hit; abilities, damage and the action bar wait. The seams it will call are here (`World::{alert, ambush, opener_allowed, make_noise, flash, break_stealth}`).
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A Stealth press lost or doubled** (two inputs reaching the host between steps, the counter wrapping past 255, a new run starting with the last run's count). Expect exactly one toggle per press, none at a run's start. Pinned in Task 4 (`two_presses_between_steps_cancel_and_one_is_never_lost`, `the_first_count_is_taken_as_it_stands`).
2. **An alerted enemy whose hero can never be reached or seen again** (behind a wall no path rounds, inside a sealed house). Expect it to stand, search where it stands, and go home calm; never stuck alerted, never walking through the wall, never searching a path every step. Pinned in Task 6 (`a_place_no_path_reaches_is_searched_from_where_it_stands`, `a_lost_hero_is_searched_for_then_the_sentry_goes_home`).
3. **A shout that chains across the map.** Expect only allies within `shout` yards of the shouter, never the allies of those allies. Pinned in Task 6 (`a_shout_brings_allies_within_reach_and_not_beyond`).
4. **A hidden enemy leaking into the interface** (Tab, the plates, a stealthed unit's plate). Expect a `Concealed` unit never a Tab candidate and no plate for a concealed, stealthed or ghost unit. Pinned in Task 8 (`a_concealed_body_is_never_a_candidate`, `concealed_stealthed_and_ghostly_units_have_no_plate`).
5. **A hero who leaves mid-chase** (a client disconnects while enemies are alerted to it). Expect every enemy calm and back at its post, no panic, no mark left for anyone. Pinned in Task 6 (`a_hero_who_leaves_mid_chase_is_forgotten`).

**Not in this plan** (see the rulings file): the ravens' alarm and the trackers' trail (§7.1, sub-project 6, with the night clock); the lantern toggle (sub-project 5's items; its multiplier is here and tested); Darkshire's lit areas (the `lit` list is empty until measured); damage, abilities and the action bar (sub-project 4); §3.4's client line-of-sight query in `benilla-world` (no client check needs it yet).

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/content.rs` | `SenseRules` and its tables, `Behaviour::Sleeper`, `ClassDef::stealth`, `MapDef::lit` |
| `crates/nightfall-sim/src/senses/mod.rs` | `Senses` (host-only state), hero stance from input, footsteps, the `World` seams and marks |
| `crates/nightfall-sim/src/senses/visibility.rs` | `Gait`, `Stance`, visibility, the facing cone, notice range, fill rate |
| `crates/nightfall-sim/src/senses/noise.rs` | `Noise`, `Loudness`, footstep radius, hearing through walls |
| `crates/nightfall-sim/src/senses/awareness.rs` | `Mind`: suspicion per hero, states, shouts, `Alert`, `Mark` |
| `crates/nightfall-sim/src/senses/react.rs` | Enemies seeing, hearing, thinking, shouting, turning, checking, chasing, searching |
| `crates/nightfall-sim/src/senses/tests.rs` | The senses in whole worlds on hand-made maps |
| `crates/nightfall-sim/src/{world,enemies,lib,messages}.rs` | Hero stance fields, input gait and presses, held brains, `asleep`, protocol 3 |
| `content/nightfall/{rules,classes,map,spawns}.ron` | The senses' numbers, the Rogue's stealth, lit areas, a sleeper |
| `crates/benilla-app/src/target/concealed.rs` (new, engine) | `Concealed`, `plate_hidden` |
| `crates/benilla-app/src/target/{mod,scan,hover}.rs`, `nameplates.rs`, `vplates.rs` (engine) | Skip concealed units; no plate for creeping units |
| `crates/benilla-app/src/nightfall/stance.rs` | Gait from the controller, the Stealth key, autoplay's gait, `NIGHTFALL_TRACE_SENSES` |
| `crates/benilla-app/src/nightfall/conceal.rs` | `Concealed` and the untrackable bit on unseen enemies; sleepers lying down; stealth's look |
| `crates/benilla-app/src/nightfall/marks.rs` | "?" and "!" over enemies |
| `crates/benilla-app/src/nightfall/senses_hud.rs` | The eye, the noise rings, the heartbeat |
| `crates/benilla-app/src/nightfall/grade.rs`, `shaders/nightfall_grade.wgsl` | The vignette following visibility |
| `crates/benilla-app/src/nightfall/{mod,run,session}.rs` | Registration, the input's gait and presses, marks on the wire |

---

### Task 1: Content: the senses' numbers, sleepers, the Rogue's stealth, lit areas

**Files:**
- Modify: `crates/nightfall-sim/src/content.rs`, `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/src/enemies.rs` (the sleeper's placement), `crates/nightfall-sim/tests/shipped_content.rs`
- Modify: `content/nightfall/rules.ron`, `content/nightfall/classes.ron`, `content/nightfall/map.ron`, `content/nightfall/spawns.ron`

**Interfaces:**
- Consumes: `Rules`, `WorldRules`, `Behaviour`, `SpawnDef`, `ClassDef`, `MapDef`, `MapFile`, `Content::from_ron`, the test fixtures `CLASSES`, `MAP`, `RULES`, `ENEMIES`, `SPAWNS`, `load`, `load_all` (foundation Task 3, world-knowledge Task 5); `enemies::spawn_all` (world-knowledge Task 8).
- Produces:
  - `SenseRules { visibility: StanceTable, lantern: f32, lit: f32, flash: f32, flash_for: f32, notice_range: f32, cone: f32, sentry_cone: f32, behind_cone: f32, side: f32, behind: f32, steps: StepNoise, loudness: LoudnessTable, walls: f32, noise_memory: f32, awareness: AwarenessRules, chase_reach: f32, ambush: f32, stealth_speed: f32, heartbeat_within: f32, vignette_seen: (f32, f32), vignette_hidden: (f32, f32) }`, every struct `#[serde(default)]` with `Default` giving the spec's values.
  - `StanceTable { run, walk, still, sneak, sneak_still }`, `StepNoise { run, walk, sneak, every }`, `LoudnessTable { quiet, normal, loud, hit, open, dig, ravens, extract }`, `AwarenessRules { fill, fill_min, noise, drain, suspicious, check, alert, shout, lose_after, search_for }` (all `f32`).
  - `Rules::senses: SenseRules`; `ClassDef::stealth: bool`; `LitArea { at: V2, radius: f32 }` and `MapDef::lit: Vec<LitArea>`; `Behaviour::Sleeper { at: (f32, f32), facing: f32 }`.

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/content.rs`'s `tests` module, add:

```rust
    #[test]
    fn the_senses_default_to_the_spec_when_left_out() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        let s = &c.rules.senses;
        assert_eq!(s.notice_range, 35.0);
        assert_eq!(s.visibility.walk, 55.0);
        assert_eq!(s.steps.run, 12.0);
        assert_eq!(s.loudness.dig, 30.0);
        assert_eq!(s.awareness.alert, 100.0);
        assert_eq!(s.stealth_speed, 0.7);
        assert!(c.map.lit.is_empty());
        assert!(!c.classes[0].stealth);
    }

    #[test]
    fn senses_given_in_part_keep_the_rest() {
        let rules = RULES.replacen(
            "(hero_level: 30,",
            "(hero_level: 30, senses: (notice_range: 30.0, awareness: (drain: 5.0)),",
            1,
        );
        let c = load(CLASSES, MAP, &rules).unwrap();
        assert_eq!(c.rules.senses.notice_range, 30.0);
        assert_eq!(c.rules.senses.awareness.drain, 5.0);
        assert_eq!(c.rules.senses.awareness.noise, 35.0);
        assert_eq!(c.rules.senses.visibility.run, 100.0);
    }

    #[test]
    fn senses_that_cannot_work_are_refused() {
        for bad in [
            "awareness: (suspicious: 70.0)",
            "stealth_speed: 0.0",
            "walls: 1.5",
            "visibility: (run: 120.0)",
            "cone: 300.0",
            "steps: (every: 0.0)",
        ] {
            let rules = RULES.replacen(
                "(hero_level: 30,",
                &format!("(hero_level: 30, senses: ({bad}),"),
                1,
            );
            assert!(
                matches!(load(CLASSES, MAP, &rules), Err(ContentError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_class_may_stealth() {
        let classes = CLASSES.replace(
            "base_hp: 100.0, speed: 7.0),",
            "base_hp: 100.0, speed: 7.0, stealth: true),",
        );
        let c = load(&classes, MAP, RULES).unwrap();
        assert!(c.classes[0].stealth);
        assert!(!c.classes[1].stealth);
    }

    #[test]
    fn lit_areas_load_and_need_a_radius() {
        let map = MAP.replace("entries:", "lit: [(20.0, 30.0, 6.0)], entries:");
        let c = load(CLASSES, &map, RULES).unwrap();
        assert_eq!(
            c.map.lit,
            vec![LitArea {
                at: V2::new(20.0, 30.0),
                radius: 6.0
            }]
        );
        let none = MAP.replace("entries:", "lit: [(20.0, 30.0, 0.0)], entries:");
        assert!(matches!(
            load(CLASSES, &none, RULES),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn a_sleeper_lies_inside_the_bounds() {
        let sleeper = SPAWNS.replace(
            "Sentry(at: (60.0, 50.0), facing: 90.0)",
            "Sleeper(at: (60.0, 50.0), facing: 90.0)",
        );
        let c = load_all(CLASSES, MAP, RULES, ENEMIES, &sleeper).unwrap();
        assert!(matches!(c.spawns[0].behaviour, Behaviour::Sleeper { .. }));
        let out = sleeper.replace("at: (60.0, 50.0)", "at: (160.0, 50.0)");
        assert!(matches!(
            load_all(CLASSES, MAP, RULES, ENEMIES, &out),
            Err(ContentError::Invalid(_))
        ));
    }
```

In `crates/nightfall-sim/tests/shipped_content.rs`, add:

```rust
#[test]
fn the_shipped_senses_are_the_specs_and_the_rogue_stealths() {
    let c = shipped();
    let s = &c.rules.senses;
    assert_eq!(s.notice_range, 35.0);
    assert_eq!(s.stealth_speed, 0.7);
    assert_eq!(s.visibility.sneak_still, 8.0);
    assert_eq!(s.awareness.lose_after, 5.0);
    assert!(c.classes[c.class("rogue").unwrap()].stealth);
    assert!(!c.classes[c.class("mage").unwrap()].stealth);
    assert!(c
        .spawns
        .iter()
        .any(|sp| matches!(sp.behaviour, nightfall_sim::Behaviour::Sleeper { .. })));
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim content`
Expected: FAIL to compile (`no field senses on type Rules`, `cannot find struct LitArea`, `no variant Sleeper`).

- [ ] **Step 3: Implement the content**

In `crates/nightfall-sim/src/content.rs`:

`ClassDef` gains, after `speed`:

```rust
    /// The class can stealth (spec §6.3's Rogue toggle, the stealthed rows of §5.1).
    #[serde(default)]
    pub stealth: bool,
```

Add, after `MapDef`:

```rust
/// A lit place: a hero standing in it is easier to see (spec §5.1: campfires, Darkshire's lamps).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LitArea {
    pub at: V2,
    /// Yards.
    pub radius: f32,
}
```

`MapDef` gains, after `entries`:

```rust
    /// Its lit places (spec §5.1).
    pub lit: Vec<LitArea>,
```

`MapFile` gains, after `entries`:

```rust
    #[serde(default)]
    lit: Vec<(f32, f32, f32)>,
```

`Rules` gains, after `world`:

```rust
    /// The senses (spec §5); the spec's values when `rules.ron` leaves any out.
    #[serde(default)]
    pub senses: SenseRules,
```

Add, after `impl Default for WorldRules { … }`:

```rust
/// The senses (spec §5): how visible and how loud heroes are and how enemies grow aware of them.
/// Every number is a starting value to playtest; each left out takes the spec's.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SenseRules {
    /// Visibility 0..100 by stance (spec §5.1).
    pub visibility: StanceTable,
    /// Visibility multipliers: a lit lantern, standing in a lit area, a fire spell's flash.
    pub lantern: f32,
    pub lit: f32,
    pub flash: f32,
    /// How long a flash lasts, seconds.
    pub flash_for: f32,
    /// How far an enemy notices a hero of visibility 100 straight ahead, yards.
    pub notice_range: f32,
    /// The front cone's width, degrees: every enemy's, and a sentry's wider one (spec §7.1).
    pub cone: f32,
    pub sentry_cone: f32,
    /// The rear cone's width, degrees; between the two lie the sides.
    pub behind_cone: f32,
    /// The notice range's share at the sides and behind (the front is 1).
    pub side: f32,
    pub behind: f32,
    /// Footsteps (spec §5.2).
    pub steps: StepNoise,
    /// Every other noise (spec §5.2).
    pub loudness: LoudnessTable,
    /// A listener without line of sight to a noise hears it within this share of its radius.
    pub walls: f32,
    /// The rings by the player frame show the loudest noise of the last this-many seconds.
    pub noise_memory: f32,
    pub awareness: AwarenessRules,
    /// An alerted enemy closes to this many yards of its hero.
    pub chase_reach: f32,
    /// The first hit on an enemy not alerted to its attacker is multiplied by this (spec §5.4).
    pub ambush: f32,
    /// A stealthed hero's speed, as a share of their own (spec §6.3, §11).
    pub stealth_speed: f32,
    /// The heartbeat plays while an enemy alerted to you is this near, yards (spec §10).
    pub heartbeat_within: f32,
    /// The night grade's vignette (strength, inner edge from the centre, both 0..1) at visibility
    /// 100 and at 0: it tightens as the hero hides (spec §10).
    pub vignette_seen: (f32, f32),
    pub vignette_hidden: (f32, f32),
}

impl Default for SenseRules {
    fn default() -> Self {
        Self {
            visibility: StanceTable::default(),
            lantern: 1.6,
            lit: 1.5,
            flash: 2.0,
            flash_for: 1.0,
            notice_range: 35.0,
            cone: 120.0,
            sentry_cone: 180.0,
            behind_cone: 120.0,
            side: 0.6,
            behind: 0.3,
            steps: StepNoise::default(),
            loudness: LoudnessTable::default(),
            walls: 0.5,
            noise_memory: 1.0,
            awareness: AwarenessRules::default(),
            chase_reach: 2.0,
            ambush: 1.5,
            stealth_speed: 0.7,
            heartbeat_within: 20.0,
            vignette_seen: (0.45, 0.35),
            vignette_hidden: (0.85, 0.1),
        }
    }
}

impl SenseRules {
    /// What is wrong with these numbers, if anything: each check is one the senses rely on.
    fn problem(&self) -> Option<&'static str> {
        let v = &self.visibility;
        let a = &self.awareness;
        let l = &self.loudness;
        let in01 = |x: f32| (0.0..=1.0).contains(&x);
        let checks: [(bool, &'static str); 12] = [
            (
                [v.run, v.walk, v.still, v.sneak, v.sneak_still]
                    .into_iter()
                    .all(|x| (0.0..=100.0).contains(&x)),
                "visibility values must be 0 to 100",
            ),
            (
                [self.lantern, self.lit, self.flash].into_iter().all(|x| x >= 1.0)
                    && self.flash_for >= 0.0,
                "lantern, lit and flash must be 1 or more, flash_for 0 or more",
            ),
            (self.notice_range > 0.0, "notice_range must be above 0"),
            (
                self.cone > 0.0
                    && self.sentry_cone > 0.0
                    && self.behind_cone >= 0.0
                    && self.cone.max(self.sentry_cone) + self.behind_cone <= 360.0,
                "cone, sentry_cone and behind_cone must leave room for each other in 360 degrees",
            ),
            (in01(self.side) && in01(self.behind), "side and behind must be 0 to 1"),
            (
                [self.steps.run, self.steps.walk, self.steps.sneak]
                    .into_iter()
                    .all(|x| x >= 0.0)
                    && self.steps.every > 0.0,
                "steps need radii of 0 or more and `every` above 0",
            ),
            (
                [l.quiet, l.normal, l.loud, l.hit, l.open, l.dig, l.ravens, l.extract]
                    .into_iter()
                    .all(|x| x >= 0.0),
                "loudness radii must be 0 or more",
            ),
            (
                in01(self.walls) && self.noise_memory > 0.0,
                "walls must be 0 to 1 and noise_memory above 0",
            ),
            (
                0.0 < a.suspicious && a.suspicious < a.check && a.check < a.alert,
                "awareness needs 0 < suspicious < check < alert",
            ),
            (
                a.fill >= a.fill_min
                    && a.fill_min >= 0.0
                    && a.noise >= 0.0
                    && a.drain >= 0.0
                    && a.shout >= 0.0
                    && a.lose_after >= 0.0
                    && a.search_for >= 0.0,
                "awareness rates, reaches and times must be 0 or more, fill at least fill_min",
            ),
            (
                self.chase_reach > 0.0
                    && self.ambush >= 1.0
                    && self.stealth_speed > 0.0
                    && self.stealth_speed <= 1.0
                    && self.heartbeat_within >= 0.0,
                "chase_reach above 0, ambush 1 or more, stealth_speed above 0 up to 1, heartbeat_within 0 or more",
            ),
            (
                [
                    self.vignette_seen.0,
                    self.vignette_seen.1,
                    self.vignette_hidden.0,
                    self.vignette_hidden.1,
                ]
                .into_iter()
                .all(in01),
                "vignette values must be 0 to 1",
            ),
        ];
        checks.into_iter().find(|(ok, _)| !ok).map(|(_, why)| why)
    }
}

/// Visibility 0..100 for each stance (spec §5.1's table).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct StanceTable {
    pub run: f32,
    pub walk: f32,
    pub still: f32,
    pub sneak: f32,
    pub sneak_still: f32,
}

impl Default for StanceTable {
    fn default() -> Self {
        Self {
            run: 100.0,
            walk: 55.0,
            still: 35.0,
            sneak: 20.0,
            sneak_still: 8.0,
        }
    }
}

/// How far a footstep carries by stance, yards, and how often a moving hero steps, seconds
/// (spec §5.2).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct StepNoise {
    pub run: f32,
    pub walk: f32,
    pub sneak: f32,
    pub every: f32,
}

impl Default for StepNoise {
    fn default() -> Self {
        Self {
            run: 12.0,
            walk: 4.0,
            sneak: 0.0,
            every: 0.5,
        }
    }
}

/// How far other noises carry, yards (spec §5.2): abilities by loudness, a hit landing or a unit
/// dying, opening a chest or searching a body, digging a grave, ravens taking off, a second of
/// extracting.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LoudnessTable {
    pub quiet: f32,
    pub normal: f32,
    pub loud: f32,
    pub hit: f32,
    pub open: f32,
    pub dig: f32,
    pub ravens: f32,
    pub extract: f32,
}

impl Default for LoudnessTable {
    fn default() -> Self {
        Self {
            quiet: 8.0,
            normal: 20.0,
            loud: 40.0,
            hit: 15.0,
            open: 15.0,
            dig: 30.0,
            ravens: 40.0,
            extract: 25.0,
        }
    }
}

/// How an enemy's suspicion of a hero rises and falls, and what it does at each level (spec §5.3).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AwarenessRules {
    /// Seeing a hero fills suspicion by `fill × (1 − distance / notice range)` a second, at least
    /// `fill_min`.
    pub fill: f32,
    pub fill_min: f32,
    /// A noise heard adds this much.
    pub noise: f32,
    /// Neither seen nor heard, it drains this much a second.
    pub drain: f32,
    /// From `suspicious` an enemy turns to look ("?"), from `check` it goes to look, at `alert` it
    /// is alerted ("!").
    pub suspicious: f32,
    pub check: f32,
    pub alert: f32,
    /// An alerted enemy shouts to every ally this near, yards.
    pub shout: f32,
    /// Seconds out of sight before an alerted enemy searches, and how long it looks around there.
    pub lose_after: f32,
    pub search_for: f32,
}

impl Default for AwarenessRules {
    fn default() -> Self {
        Self {
            fill: 60.0,
            fill_min: 10.0,
            noise: 35.0,
            drain: 10.0,
            suspicious: 40.0,
            check: 60.0,
            alert: 100.0,
            shout: 15.0,
            lose_after: 5.0,
            search_for: 15.0,
        }
    }
}
```

`Behaviour` gains, after `Sentry`:

```rust
    /// Lies at a post facing `facing`: hears but does not see until alerted (spec §7.1).
    Sleeper { at: (f32, f32), facing: f32 },
```

In `Content::from_ron`:
- In the spawns' `match &s.behaviour`, change the first arm's pattern to `Behaviour::Sentry { at, .. } | Behaviour::Sleeper { at, .. } =>`.
- After the map's entries check (`for &(x, y, _) in &map.entries { … }`), add:

```rust
        if map.lit.iter().any(|&(_, _, r)| !(r > 0.0)) {
            return invalid("map.ron: a lit area needs a radius above 0".into());
        }
```

- After the `rules.world` check, add:

```rust
        if let Some(why) = rules.senses.problem() {
            return invalid(format!("rules.ron: senses: {why}"));
        }
```

- In the `MapDef { … }` literal, after `entries: …,` add:

```rust
                lit: map
                    .lit
                    .iter()
                    .map(|&(x, y, radius)| LitArea {
                        at: V2::new(x, y),
                        radius,
                    })
                    .collect(),
```

In `crates/nightfall-sim/src/lib.rs`, change the content `pub use` to:

```rust
pub use content::{
    Atmosphere, AwarenessRules, Behaviour, ClassDef, Content, ContentError, ContentSources,
    EnemyDef, HeroLight, LitArea, LoudnessTable, MapDef, Rules, SenseRules, SpawnDef,
    StanceTable, StepNoise, WeaponDef, WorldRules,
};
```

In `crates/nightfall-sim/src/enemies.rs`'s `spawn_all`, change `Behaviour::Sentry { at, facing } => {` to `Behaviour::Sentry { at, facing } | Behaviour::Sleeper { at, facing } => {` (a sleeper is placed and keeps its post as a sentry does; its blindness comes in Task 6).

- [ ] **Step 4: Write the shipped content**

In `content/nightfall/rules.ron`, after the `world: ( … ),` block and before the closing `)`:

```ron
    // The senses (spec §5): how visible and how loud heroes are, and how enemies grow aware of
    // them. Every number is a starting value to playtest; one left out takes the value shown here.
    senses: (
        // Visibility 0..100 by stance, and what light multiplies it by (a lantern, standing in a
        // lit area, a fire spell's flash for `flash_for` seconds); never past 100.
        visibility: (run: 100.0, walk: 55.0, still: 35.0, sneak: 20.0, sneak_still: 8.0),
        lantern: 1.6,
        lit: 1.5,
        flash: 2.0,
        flash_for: 1.0,
        // An enemy notices a hero within notice_range × visibility / 100 × facing yards, never
        // past the fog: facing is 1 in its front cone (degrees; a sentry's is wider), `behind` in
        // its rear cone, `side` between.
        notice_range: 35.0,
        cone: 120.0,
        sentry_cone: 180.0,
        behind_cone: 120.0,
        side: 0.6,
        behind: 0.3,
        // Footsteps: how far each carries by stance (yards), one every `every` seconds of moving.
        steps: (run: 12.0, walk: 4.0, sneak: 0.0, every: 0.5),
        // How far other noises carry, yards: abilities by loudness, a hit or a death, opening a
        // chest or searching a body, digging a grave, ravens taking off, a second of extracting.
        loudness: (quiet: 8.0, normal: 20.0, loud: 40.0, hit: 15.0, open: 15.0, dig: 30.0,
                   ravens: 40.0, extract: 25.0),
        // Through a wall (no line of sight) a noise carries this share of its radius.
        walls: 0.5,
        // The rings by the player frame show the loudest noise of the last this-many seconds.
        noise_memory: 1.0,
        // Suspicion 0..100 of each hero: sight fills it by fill × (1 − distance / notice range) a
        // second (at least fill_min), a noise adds `noise`, neither drains `drain` a second. From
        // `suspicious` an enemy turns to look ("?"), from `check` it goes to look, at `alert` it
        // is alerted ("!") and shouts to allies within `shout` yards. Out of sight `lose_after`
        // seconds, it searches where it last saw its hero for `search_for` seconds.
        awareness: (fill: 60.0, fill_min: 10.0, noise: 35.0, drain: 10.0, suspicious: 40.0,
                    check: 60.0, alert: 100.0, shout: 15.0, lose_after: 5.0, search_for: 15.0),
        // An alerted enemy closes to this many yards of its hero.
        chase_reach: 2.0,
        // The first hit on an enemy not alerted to its attacker is multiplied by this.
        ambush: 1.5,
        // A stealthed hero moves at this share of their speed.
        stealth_speed: 0.7,
        // The heartbeat plays while an enemy alerted to you is this near, yards.
        heartbeat_within: 20.0,
        // The night grade's vignette (strength, inner edge from the centre; 0..1) at visibility
        // 100 and at 0: it tightens as the hero hides.
        vignette_seen: (0.45, 0.35),
        vignette_hidden: (0.85, 0.1),
    ),
```

In `content/nightfall/classes.ron`, the Rogue gains after `speed: 7.0,`:

```ron
        // Stealth on its key (spec §6.3): sneaking at stealth_speed, nearly unseen.
        stealth: true,
```

and the file's first comment's last sentence becomes `Abilities come in sub-project 4; the Rogue's Stealth toggle is here already (sub-project 3).`

In `content/nightfall/map.ron`, after the `entries: [ … ],` block:

```ron
    // Lit places, (x, y, radius) yards: a hero standing in one is easier to see. measure:
    // Darkshire's lamps and the campfires, once walked in the game (NIGHTFALL_TRACE_POS=1).
    lit: [],
```

In `content/nightfall/spawns.ron`, change the header's first sentence to `// Where Nightfall's enemies stand and walk (spec §7.1): a sentry holds a post, a sleeper lies at one (it hears but does not see until alerted), a patrol walks its route round and round, wanderers roam an area.` and add as the last entry of the list:

```ron
    // measure: Raven Hill's crypt, where the dead lie until woken.
    (place: "Near the entry: a sleeper", kind: "fetid_corpse",
     behaviour: Sleeper(at: (-10062.0, -805.0), facing: 45.0)),
```

- [ ] **Step 5: Run the tests and check the client builds**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test, the 6 new content tests and `the_shipped_senses_are_the_specs_and_the_rogue_stealths`).
Run: `cargo check -p benilla-app`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git add content/nightfall crates/nightfall-sim
git commit -m "Nightfall content: the senses' numbers, sleepers, the Rogue's stealth, lit areas

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Visibility, the facing cone and the notice range

**Files:**
- Create: `crates/nightfall-sim/src/senses/mod.rs`, `crates/nightfall-sim/src/senses/visibility.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `SenseRules`, `AwarenessRules` (Task 1); `V2`.
- Produces:
  - `nightfall_sim::Gait::{Still, Walk, Run}` and `nightfall_sim::Stance::{Still, Walking, Running, Sneaking, SneakingStill}` (both `Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize`), `Stance::of(Gait, bool) -> Stance`, `Stance::moving(self) -> bool`.
  - Crate-only: `Light { lantern: bool, lit_area: bool, flash: bool }`; `visibility(Stance, Light, &SenseRules) -> f32`; `facing_factor(facing: V2, to: V2, cone: f32, &SenseRules) -> f32`; `notice_range(visibility: f32, facing: f32, fog: f32, &SenseRules) -> f32`; `fill_rate(distance: f32, range: f32, &AwarenessRules) -> f32`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/senses/visibility.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stance_has_its_visibility() {
        let r = SenseRules::default();
        let none = Light::default();
        assert_eq!(visibility(Stance::Running, none, &r), 100.0);
        assert_eq!(visibility(Stance::Walking, none, &r), 55.0);
        assert_eq!(visibility(Stance::Still, none, &r), 35.0);
        assert_eq!(visibility(Stance::Sneaking, none, &r), 20.0);
        assert_eq!(visibility(Stance::SneakingStill, none, &r), 8.0);
    }

    #[test]
    fn the_stance_comes_from_the_gait_and_stealth() {
        assert_eq!(Stance::of(Gait::Run, false), Stance::Running);
        assert_eq!(Stance::of(Gait::Walk, false), Stance::Walking);
        assert_eq!(Stance::of(Gait::Still, false), Stance::Still);
        assert_eq!(Stance::of(Gait::Run, true), Stance::Sneaking);
        assert_eq!(Stance::of(Gait::Walk, true), Stance::Sneaking);
        assert_eq!(Stance::of(Gait::Still, true), Stance::SneakingStill);
        assert!(Stance::Sneaking.moving());
        assert!(!Stance::SneakingStill.moving());
        assert!(!Stance::Still.moving());
    }

    #[test]
    fn light_multiplies_and_never_passes_100() {
        let r = SenseRules::default();
        let lantern = Light {
            lantern: true,
            ..Light::default()
        };
        assert!((visibility(Stance::Walking, lantern, &r) - 88.0).abs() < 1e-4);
        let all = Light {
            lantern: true,
            lit_area: true,
            flash: true,
        };
        assert_eq!(visibility(Stance::Walking, all, &r), 100.0);
        let flash = Light {
            flash: true,
            ..Light::default()
        };
        assert_eq!(visibility(Stance::SneakingStill, flash, &r), 16.0);
    }

    #[test]
    fn in_front_at_the_sides_and_behind() {
        let r = SenseRules::default();
        let north = V2::new(1.0, 0.0);
        assert_eq!(facing_factor(north, V2::new(1.0, 0.0), 120.0, &r), 1.0);
        assert_eq!(facing_factor(north, V2::new(1.0, 1.0), 120.0, &r), 1.0);
        assert_eq!(facing_factor(north, V2::new(0.0, 1.0), 120.0, &r), 0.6);
        assert_eq!(facing_factor(north, V2::new(-1.0, 0.3), 120.0, &r), 0.3);
        // A sentry's wide cone takes in what stands nearly at its side.
        assert_eq!(facing_factor(north, V2::new(0.2, 1.0), 180.0, &r), 1.0);
        // On top of it: in front.
        assert_eq!(facing_factor(north, V2::ZERO, 120.0, &r), 1.0);
    }

    #[test]
    fn the_notice_range_follows_visibility_and_facing_and_stops_at_the_fog() {
        let r = SenseRules::default();
        assert_eq!(notice_range(100.0, 1.0, 40.0, &r), 35.0);
        assert!((notice_range(55.0, 1.0, 40.0, &r) - 19.25).abs() < 1e-4);
        assert!((notice_range(20.0, 0.3, 40.0, &r) - 2.1).abs() < 1e-4);
        assert_eq!(notice_range(100.0, 1.0, 30.0, &r), 30.0);
    }

    #[test]
    fn suspicion_fills_faster_near_and_never_slower_than_the_least() {
        let a = AwarenessRules::default();
        assert_eq!(fill_rate(0.0, 20.0, &a), 60.0);
        assert_eq!(fill_rate(10.0, 20.0, &a), 30.0);
        assert_eq!(fill_rate(19.5, 20.0, &a), 10.0);
        assert_eq!(fill_rate(5.0, 0.0, &a), 0.0);
    }
}
```

`crates/nightfall-sim/src/senses/mod.rs`:

```rust
//! The senses (spec §5): how visible and how loud each hero is, what each enemy makes of them,
//! and what it does about it. The host's alone: a client's mirror gets each hero's visibility and
//! noise in the snapshot, and its own player's marks beside it.

mod visibility;

pub use visibility::{Gait, Stance};
```

In `lib.rs`, add `mod senses;` beside `mod sight;` and `pub use senses::{Gait, Stance};`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim senses::visibility`
Expected: FAIL to compile (`cannot find function visibility`, `cannot find type Stance`).

- [ ] **Step 3: Implement visibility**

At the top of `crates/nightfall-sim/src/senses/visibility.rs`:

```rust
//! How visible a hero is (spec §5.1): a number from 0 to 100 set by how they move, raised by
//! light, and how far that lets an enemy notice them, by where they stand in its view.

use serde::{Deserialize, Serialize};

use crate::{AwarenessRules, SenseRules, V2};

/// How a hero moves, as their client reports it: WoW's walk toggle, and whether they move at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gait {
    #[default]
    Still,
    Walk,
    Run,
}

/// A hero's stance: spec §5.1's rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    #[default]
    Still,
    Walking,
    Running,
    /// In stealth and moving, at whatever pace.
    Sneaking,
    SneakingStill,
}

impl Stance {
    /// The stance of a hero moving at `gait`, in stealth or not.
    pub fn of(gait: Gait, stealthed: bool) -> Stance {
        match (stealthed, gait) {
            (true, Gait::Still) => Stance::SneakingStill,
            (true, _) => Stance::Sneaking,
            (false, Gait::Still) => Stance::Still,
            (false, Gait::Walk) => Stance::Walking,
            (false, Gait::Run) => Stance::Running,
        }
    }

    /// Whether the hero's feet are moving (and so stepping).
    pub fn moving(self) -> bool {
        matches!(self, Stance::Walking | Stance::Running | Stance::Sneaking)
    }
}

/// What lights a hero up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Light {
    pub lantern: bool,
    pub lit_area: bool,
    pub flash: bool,
}

/// A hero's visibility, 0..100: the stance's row times every light on them.
pub(crate) fn visibility(stance: Stance, light: Light, r: &SenseRules) -> f32 {
    let t = &r.visibility;
    let mut v = match stance {
        Stance::Running => t.run,
        Stance::Walking => t.walk,
        Stance::Still => t.still,
        Stance::Sneaking => t.sneak,
        Stance::SneakingStill => t.sneak_still,
    };
    if light.lantern {
        v *= r.lantern;
    }
    if light.lit_area {
        v *= r.lit;
    }
    if light.flash {
        v *= r.flash;
    }
    v.clamp(0.0, 100.0)
}

/// The share of its notice range an enemy facing `facing` has for something in direction `to`
/// from it: 1 in its front cone of `cone` degrees, `behind` in its rear cone, `side` between.
/// Something on top of it is in front.
pub(crate) fn facing_factor(facing: V2, to: V2, cone: f32, r: &SenseRules) -> f32 {
    let (f, d) = (facing.normalize_or_zero(), to.normalize_or_zero());
    if f == V2::ZERO || d == V2::ZERO {
        return 1.0;
    }
    let off = f.dot(d).clamp(-1.0, 1.0).acos().to_degrees();
    if off <= cone / 2.0 {
        1.0
    } else if off >= 180.0 - r.behind_cone / 2.0 {
        r.behind
    } else {
        r.side
    }
}

/// How far an enemy notices a hero of `visibility` standing at `facing` of its view, never past
/// the `fog` (spec §5.1).
pub(crate) fn notice_range(visibility: f32, facing: f32, fog: f32, r: &SenseRules) -> f32 {
    (r.notice_range * visibility / 100.0 * facing).min(fog)
}

/// How fast seeing a hero `distance` yards off, inside a notice range of `range`, fills suspicion,
/// a second (spec §5.3).
pub(crate) fn fill_rate(distance: f32, range: f32, a: &AwarenessRules) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    (a.fill * (1.0 - distance / range)).max(a.fill_min)
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim senses::visibility`
Expected: PASS (6 tests). Warnings that `Light`, `visibility`, `facing_factor`, `notice_range` and `fill_rate` are unused go away in Tasks 4 and 6.

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: visibility by stance and light, the facing cone, the notice range

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Noise and hearing through walls

**Files:**
- Create: `crates/nightfall-sim/src/senses/noise.rs`
- Modify: `crates/nightfall-sim/src/senses/mod.rs`, `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `Stance` (Task 2); `StepNoise`, `LoudnessTable` (Task 1); `HeightMap::line_of_sight`, `CHEST_HEIGHT`, `testmap::flat` (world knowledge Tasks 1, 2, 9).
- Produces:
  - `nightfall_sim::Loudness::{Quiet, Normal, Loud, Hit, Open, Dig, Ravens, Extract}` (`Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize`) with `radius(self, &LoudnessTable) -> f32`.
  - Crate-only `Noise { by: PlayerId, at: V3, radius: f32 }` (`at` is the source's floor point); `step_radius(Stance, &StepNoise) -> f32`; `hears(ear: V3, noise: &Noise, map: Option<&HeightMap>, walls: f32) -> bool`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/senses/noise.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightmap::testmap::flat;
    use crate::V2;

    fn ear(x: f32, y: f32) -> V3 {
        V3::new(x, y, 1.7)
    }

    fn noise(x: f32, y: f32, radius: f32) -> Noise {
        Noise {
            by: 0,
            at: V3::new(x, y, 0.0),
            radius,
        }
    }

    #[test]
    fn footsteps_are_as_loud_as_the_stance() {
        let s = StepNoise::default();
        assert_eq!(step_radius(Stance::Running, &s), 12.0);
        assert_eq!(step_radius(Stance::Walking, &s), 4.0);
        assert_eq!(step_radius(Stance::Sneaking, &s), 0.0);
        assert_eq!(step_radius(Stance::Still, &s), 0.0);
        assert_eq!(step_radius(Stance::SneakingStill, &s), 0.0);
    }

    #[test]
    fn each_loudness_carries_as_far_as_its_row() {
        let t = LoudnessTable::default();
        let rows = [
            (Loudness::Quiet, 8.0),
            (Loudness::Normal, 20.0),
            (Loudness::Loud, 40.0),
            (Loudness::Hit, 15.0),
            (Loudness::Open, 15.0),
            (Loudness::Dig, 30.0),
            (Loudness::Ravens, 40.0),
            (Loudness::Extract, 25.0),
        ];
        for (l, r) in rows {
            assert_eq!(l.radius(&t), r, "{l:?}");
        }
    }

    #[test]
    fn in_the_open_a_noise_carries_its_radius() {
        let m = flat(50.0).finish();
        let n = noise(21.0, 10.0, 12.0);
        assert!(hears(ear(10.0, 10.0), &n, Some(&m), 0.5));
        assert!(!hears(ear(8.0, 10.0), &n, Some(&m), 0.5));
    }

    #[test]
    fn a_wall_halves_how_far_a_noise_carries() {
        let mut b = flat(50.0);
        b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
        let m = b.finish();
        let n = noise(25.5, 10.0, 12.0);
        assert!(hears(ear(20.0, 10.0), &n, Some(&m), 0.5));
        assert!(!hears(ear(18.0, 10.0), &n, Some(&m), 0.5));
        // Without the map nothing is known to stand between.
        assert!(hears(ear(18.0, 10.0), &n, None, 0.5));
    }

    #[test]
    fn a_noise_or_an_ear_that_is_not_a_number_is_not_heard() {
        assert!(!hears(ear(10.0, 10.0), &noise(f32::NAN, 10.0, 12.0), None, 0.5));
        assert!(!hears(
            V3::new(10.0, f32::INFINITY, 1.7),
            &noise(12.0, 10.0, 12.0),
            None,
            0.5
        ));
    }
}
```

In `senses/mod.rs`, add `mod noise;` above `mod visibility;` and, beside the other `pub use`:

```rust
pub use noise::Loudness;
pub(crate) use noise::Noise;
```

In `lib.rs`, change `pub use senses::{Gait, Stance};` to `pub use senses::{Gait, Loudness, Stance};`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim senses::noise`
Expected: FAIL to compile (`cannot find function hears`, `cannot find type Loudness`).

- [ ] **Step 3: Implement noise**

At the top of `crates/nightfall-sim/src/senses/noise.rs`:

```rust
//! Noise (spec §5.2): an event with a place and a radius, made by a hero. A listener with a clear
//! line to it hears it within its radius, and through something solid within `walls` of it. It
//! points the listener at the place, never at the hero. Footsteps are noises too, as loud as the
//! stance.

use serde::{Deserialize, Serialize};

use super::Stance;
use crate::heightmap::HeightMap;
use crate::{LoudnessTable, PlayerId, StepNoise, CHEST_HEIGHT, V3};

/// A noise a hero made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Noise {
    pub by: PlayerId,
    /// The floor point it came from; it is heard at chest height over it.
    pub at: V3,
    /// Yards.
    pub radius: f32,
}

/// How loud a thing is (spec §5.2's table), for the noises later sub-projects make through
/// [`crate::World::make_noise`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loudness {
    /// Abilities, by their loudness.
    Quiet,
    Normal,
    Loud,
    /// A hit landing, a unit dying.
    Hit,
    /// Opening a chest, searching a body.
    Open,
    /// Digging a grave.
    Dig,
    /// Ravens taking off.
    Ravens,
    /// A second in an exit's ring.
    Extract,
}

impl Loudness {
    /// How far it carries, yards.
    pub fn radius(self, t: &LoudnessTable) -> f32 {
        match self {
            Loudness::Quiet => t.quiet,
            Loudness::Normal => t.normal,
            Loudness::Loud => t.loud,
            Loudness::Hit => t.hit,
            Loudness::Open => t.open,
            Loudness::Dig => t.dig,
            Loudness::Ravens => t.ravens,
            Loudness::Extract => t.extract,
        }
    }
}

/// How far one footstep of a hero in `stance` carries, yards: nothing standing still.
pub(crate) fn step_radius(stance: Stance, s: &StepNoise) -> f32 {
    match stance {
        Stance::Running => s.run,
        Stance::Walking => s.walk,
        Stance::Sneaking => s.sneak,
        Stance::Still | Stance::SneakingStill => 0.0,
    }
}

/// Whether ears at `ear` hear `noise`: within its radius with a clear line to it, or within
/// `walls` of its radius through something solid. Without a map nothing is known to stand between.
/// A place or an ear that is not a number hears nothing.
pub(crate) fn hears(ear: V3, noise: &Noise, map: Option<&HeightMap>, walls: f32) -> bool {
    if !ear.is_finite() || !noise.at.is_finite() {
        return false;
    }
    let d = ear.ground().distance(noise.at.ground());
    if d > noise.radius {
        return false;
    }
    let source = V3::at(noise.at.ground(), noise.at.z + CHEST_HEIGHT);
    let clear = map.is_none_or(|m| m.line_of_sight(ear, source));
    clear || d <= noise.radius * walls
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim senses::noise`
Expected: PASS (5 tests). Warnings that `Noise`, `step_radius` and `hears` are unused go away in Tasks 4 and 6.

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: noise, its loudness table, and hearing through walls at half

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Heroes' senses in the world: gait, Stealth, visibility, speed, footsteps

**Files:**
- Create: `crates/nightfall-sim/src/senses/tests.rs`
- Modify: `crates/nightfall-sim/src/senses/mod.rs` (replaced whole), `crates/nightfall-sim/src/world.rs`, `crates/nightfall-sim/src/sight.rs` (test fixture), `crates/nightfall-sim/tests/shipped_content.rs` (test fixture)
- Modify (so the client still builds): `crates/benilla-app/src/nightfall/run.rs` (`step_sim`'s two `PlayerInput` literals)

**Interfaces:**
- Consumes: `Gait`, `Stance`, `Light`, `visibility` (Task 2); `Noise`, `Loudness`, `step_radius` (Task 3); `SenseRules`, `ClassDef::stealth`, `MapDef::lit` (Task 1); `World`, `Hero`, `HeroPose { pos, facing, z }`, `PlayerInput`, `HERO_FLOOR_REACH`, `testmap` (world knowledge Tasks 8, 9).
- Produces:
  - `PlayerInput` gains `gait: Gait` and `stealth_presses: u8` (a wrapping count of Stealth presses).
  - `Hero` gains `stance: Stance`, `stealthed: bool`, `visibility: f32`, `noise: f32` (the loudest noise they made in the last `noise_memory` seconds, yards).
  - `World` gains `pub(crate) senses: Senses`; `World::make_noise(&mut self, by: PlayerId, at: V3, loudness: Loudness)`, `World::flash(&mut self, player: PlayerId)`, `World::break_stealth(&mut self, player: PlayerId)`.
  - Crate-only: `senses::Senses { heroes, noises }` (`Clone, Debug, Default`) with `forget(&mut self, PlayerId)`; `senses::take_input(&mut Senses, &mut Hero, &PlayerInput, &Content)`; `senses::step_heroes(&mut Senses, &mut [Hero], &Content)`.
  - Test-only `senses::tests::{world(&str) -> World, sentry(f32, f32, f32) -> String, open() -> Arc<HeightMap>, put(&mut World, PlayerId, f32, f32, Gait, u8), steps(&mut World, usize)}`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/senses/tests.rs`:

```rust
//! The senses in whole worlds on small hand-made maps: heroes walking, running and sneaking;
//! enemies seeing, hearing, checking, chasing, shouting and searching.

use std::sync::Arc;

use crate::heightmap::testmap::flat;
use crate::heightmap::HeightMap;
use crate::{
    Content, ContentSources, Gait, HeroPose, Loudness, PlayerId, PlayerInput, Stance, World, V2,
    V3,
};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0, stealth: true),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 120.0, speed: 7.0),
]"#;
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
    entries: [(5.0, 5.0, 0.0)],
    lit: [(40.0, 40.0, 5.0)])"#;
const ENEMIES: &str = r#"[
    (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
]"#;
const RULES: &str = r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
    fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
    hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#;

/// A world on [`MAP`] (fog at 40 yd, a lit circle at (40, 40)) with `spawns`, each a RON list
/// item ending in a comma; the senses take the spec's numbers.
pub(super) fn world(spawns: &str) -> World {
    let spawns = format!("[{spawns}]");
    let content = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: RULES,
        enemies: ENEMIES,
        spawns: &spawns,
    })
    .unwrap();
    World::new(Arc::new(content))
}

/// A skeleton sentry at `(x, y)` facing `facing` degrees (0 north, 90 west), as a spawn line.
pub(super) fn sentry(x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "skeleton", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// Flat ground 50 yards across.
pub(super) fn open() -> Arc<HeightMap> {
    Arc::new(flat(50.0).finish())
}

/// `player`'s hero (a rogue, made if need be) at `(x, y)` on the ground, moving at `gait`, its
/// Stealth pressed `presses` times so far: the input for the next step.
pub(super) fn put(w: &mut World, player: PlayerId, x: f32, y: f32, gait: Gait, presses: u8) {
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
            gait,
            stealth_presses: presses,
        },
    );
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

#[test]
fn each_gait_has_its_stance_and_visibility() {
    let mut w = world("");
    for (gait, stance, vis) in [
        (Gait::Still, Stance::Still, 35.0),
        (Gait::Walk, Stance::Walking, 55.0),
        (Gait::Run, Stance::Running, 100.0),
    ] {
        put(&mut w, 0, 10.0, 10.0, gait, 0);
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!((h.stance, h.visibility), (stance, vis));
    }
}

#[test]
fn a_rogue_stealths_on_a_press_and_a_mage_cannot() {
    let mut w = world("");
    w.add_hero(1, "mage", "Jaina", 1, 1).unwrap();
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 0);
    put(&mut w, 1, 20.0, 10.0, Gait::Walk, 0);
    w.step();
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 1);
    put(&mut w, 1, 20.0, 10.0, Gait::Walk, 1);
    w.step();
    let rogue = w.hero(0).unwrap();
    assert!(rogue.stealthed);
    assert_eq!(rogue.stance, Stance::Sneaking);
    assert_eq!(rogue.visibility, 20.0);
    assert!((rogue.speed - 4.9).abs() < 1e-4, "{}", rogue.speed);
    let mage = w.hero(1).unwrap();
    assert!(!mage.stealthed);
    assert_eq!(mage.speed, 7.0);
    // Still in stealth: the least visible of all.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 1);
    w.step();
    assert_eq!(w.hero(0).unwrap().visibility, 8.0);
}

#[test]
fn the_first_count_is_taken_as_it_stands() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 5);
    w.step();
    assert!(!w.hero(0).unwrap().stealthed);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 6);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
}

#[test]
fn two_presses_between_steps_cancel_and_one_is_never_lost() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    // Two inputs before a step: the second replaces the first, and its count holds both presses.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 1);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 2);
    w.step();
    assert!(!w.hero(0).unwrap().stealthed);
    // A press whose input a later one replaced is still in the later one's count.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 3);
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 3);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
    // The count goes round past 255: 3 to 255 is an even number of presses, 255 to 0 is one.
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 255);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    assert!(!w.hero(0).unwrap().stealthed);
}

#[test]
fn breaking_stealth_gives_the_speed_back() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 1);
    w.step();
    assert!(w.hero(0).unwrap().stealthed);
    w.break_stealth(0);
    w.step();
    let h = w.hero(0).unwrap();
    assert!(!h.stealthed);
    assert_eq!(h.speed, 7.0);
    assert_eq!(h.stance, Stance::Still);
}

#[test]
fn a_lit_area_and_a_flash_raise_visibility() {
    let mut w = world("");
    put(&mut w, 0, 40.0, 41.0, Gait::Walk, 0);
    w.step();
    assert_eq!(w.hero(0).unwrap().visibility, 82.5);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    w.flash(0);
    w.step();
    assert_eq!(w.hero(0).unwrap().visibility, 70.0);
    steps(&mut w, 30);
    assert_eq!(w.hero(0).unwrap().visibility, 35.0);
}

#[test]
fn footsteps_are_as_loud_as_the_stance_and_the_rings_remember_a_second() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Run, 0);
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 12.0);
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    steps(&mut w, 29);
    assert_eq!(w.hero(0).unwrap().noise, 12.0);
    steps(&mut w, 2);
    assert_eq!(w.hero(0).unwrap().noise, 0.0);
    put(&mut w, 0, 10.0, 10.0, Gait::Walk, 0);
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 4.0);
}

#[test]
fn a_sneaking_rogue_makes_no_footstep_noise() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    put(&mut w, 0, 10.0, 10.0, Gait::Run, 1);
    steps(&mut w, 60);
    let h = w.hero(0).unwrap();
    assert_eq!(h.stance, Stance::Sneaking);
    assert_eq!(h.noise, 0.0);
}

#[test]
fn a_made_noise_counts_for_its_maker() {
    let mut w = world("");
    put(&mut w, 0, 10.0, 10.0, Gait::Still, 0);
    w.step();
    w.make_noise(0, V3::new(12.0, 10.0, 0.0), Loudness::Loud);
    // A player without a hero makes no noise.
    w.make_noise(7, V3::new(12.0, 10.0, 0.0), Loudness::Loud);
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 40.0);
}
```

In `crates/nightfall-sim/src/world.rs`'s tests (`pose`, `bad_facing`, `a_hero_stands_on_the_floor_near_the_height_its_client_reports`'s `at`, `without_a_map_a_hero_takes_the_reported_height`), in `crates/nightfall-sim/src/sight.rs`'s test `put`, and in `crates/nightfall-sim/tests/shipped_content.rs`'s `both_classes_play_in_duskwood`, every `PlayerInput { pose: … }` literal gains `..Default::default()` after its `pose` field. Find them all with `grep -rn "PlayerInput {" crates/nightfall-sim`; the only literal that names `gait` is the new `senses/tests.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`struct PlayerInput has no field named gait`, `no method named make_noise`).

- [ ] **Step 3: Implement the heroes' senses**

Replace `crates/nightfall-sim/src/senses/mod.rs` with:

```rust
//! The senses (spec §5): how visible and how loud each hero is, what each enemy makes of them,
//! and what it does about it. The host's alone: a client's mirror gets each hero's visibility and
//! noise in the snapshot, and its own player's marks beside it.
//!
//! Each step, once the heroes have their inputs, [`step_heroes`] works out every hero's stance,
//! visibility, speed and footsteps.

mod noise;
#[cfg(test)]
mod tests;
mod visibility;

pub use noise::Loudness;
pub(crate) use noise::Noise;
pub use visibility::{Gait, Stance};

use noise::step_radius;
use visibility::{visibility, Light};

use crate::{Content, Hero, PlayerId, PlayerInput, World, DT, V3};

/// What the host keeps of one hero's senses: never sent.
#[derive(Clone, Debug, Default)]
struct HeroSense {
    player: PlayerId,
    /// The Stealth count of the last input; `None` until the first, which is taken as it stands
    /// (a new run must not toggle Stealth with the last run's presses).
    presses: Option<u8>,
    /// How the hero moves, as the last input said.
    gait: Gait,
    /// Seconds until the next footstep, while moving.
    step_in: f32,
    /// Seconds of a fire spell's flash left.
    flash: f32,
    /// The noises the hero made lately: (seconds ago, radius).
    recent: Vec<(f32, f32)>,
}

/// The senses' state on the host.
#[derive(Clone, Debug, Default)]
pub(crate) struct Senses {
    heroes: Vec<HeroSense>,
    /// The noises made since the enemies last listened.
    pub(crate) noises: Vec<Noise>,
}

impl Senses {
    /// `player`'s sense state, made if need be.
    fn hero_index(&mut self, player: PlayerId) -> usize {
        match self.heroes.iter().position(|h| h.player == player) {
            Some(i) => i,
            None => {
                self.heroes.push(HeroSense {
                    player,
                    ..HeroSense::default()
                });
                self.heroes.len() - 1
            }
        }
    }

    /// A player left: nothing of their hero stays.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.noises.retain(|n| n.by != player);
    }
}

/// A hero takes its player's input: the gait, and the Stealth presses since the last input,
/// counted round so that a press is never lost when one input replaces another between steps.
/// Only a class that can stealth does.
pub(crate) fn take_input(s: &mut Senses, hero: &mut Hero, input: &PlayerInput, content: &Content) {
    let i = s.hero_index(hero.player);
    let hs = &mut s.heroes[i];
    hs.gait = input.gait;
    let pressed = hs
        .presses
        .map_or(0, |last| input.stealth_presses.wrapping_sub(last));
    hs.presses = Some(input.stealth_presses);
    if pressed % 2 == 1 && content.classes[hero.class].stealth {
        hero.stealthed = !hero.stealthed;
    }
}

/// Every hero's stance, visibility and speed for this step, and their footsteps: one every
/// `steps.every` seconds of moving, the first at once, as loud as the stance. The rings remember
/// the loudest noise of the last `noise_memory` seconds.
pub(crate) fn step_heroes(s: &mut Senses, heroes: &mut [Hero], content: &Content) {
    let r = &content.rules.senses;
    for hero in heroes.iter_mut() {
        let i = s.hero_index(hero.player);
        let hs = &mut s.heroes[i];
        hero.stance = Stance::of(hs.gait, hero.stealthed);
        hs.flash = (hs.flash - DT).max(0.0);
        let lit_area = content
            .map
            .lit
            .iter()
            .any(|l| l.at.distance(hero.pos) <= l.radius);
        let light = Light {
            lantern: false,
            lit_area,
            flash: hs.flash > 0.0,
        };
        hero.visibility = visibility(hero.stance, light, r);
        let class = &content.classes[hero.class];
        hero.speed = class.speed * if hero.stealthed { r.stealth_speed } else { 1.0 };
        for n in &mut hs.recent {
            n.0 += DT;
        }
        hs.recent.retain(|n| n.0 <= r.noise_memory);
        let mut step = None;
        if hero.stance.moving() {
            hs.step_in -= DT;
            if hs.step_in <= 0.0 {
                hs.step_in += r.steps.every;
                let radius = step_radius(hero.stance, &r.steps);
                if radius > 0.0 {
                    hs.recent.push((0.0, radius));
                    step = Some(Noise {
                        by: hero.player,
                        at: V3::at(hero.pos, hero.z),
                        radius,
                    });
                }
            }
        } else {
            hs.step_in = 0.0;
        }
        hero.noise = hs.recent.iter().map(|n| n.1).fold(0.0, f32::max);
        if let Some(n) = step {
            s.noises.push(n);
        }
    }
}

impl World {
    /// A noise `by`'s hero made at `at` (the floor it came from), as loud as `loudness` (spec
    /// §5.2): heard by the enemies at the next step and shown in the maker's rings. The seam the
    /// abilities, chests, graves and exits of later sub-projects make their noise through.
    pub fn make_noise(&mut self, by: PlayerId, at: V3, loudness: Loudness) {
        if self.hero(by).is_none() || !at.is_finite() {
            return;
        }
        let radius = loudness.radius(&self.content().rules.senses.loudness);
        if !(radius > 0.0) {
            return;
        }
        self.senses.noises.push(Noise { by, at, radius });
        let i = self.senses.hero_index(by);
        self.senses.heroes[i].recent.push((0.0, radius));
    }

    /// A fire spell's flash on `player`'s hero (spec §5.1): visibility ×`flash` for `flash_for`
    /// seconds. Sub-project 4's fire spells call it.
    pub fn flash(&mut self, player: PlayerId) {
        if self.hero(player).is_none() {
            return;
        }
        let secs = self.content().rules.senses.flash_for;
        let i = self.senses.hero_index(player);
        self.senses.heroes[i].flash = secs;
    }

    /// `player`'s hero drops out of stealth (spec §6.3: attacking or taking damage breaks it);
    /// their speed comes back at the next step. Sub-project 4's combat calls it.
    pub fn break_stealth(&mut self, player: PlayerId) {
        if let Some(h) = self.heroes.iter_mut().find(|h| h.player == player) {
            h.stealthed = false;
        }
    }
}
```

In `crates/nightfall-sim/src/world.rs`:

- Add `use crate::senses::{self, Gait, Senses, Stance};` to the imports.
- `PlayerInput` becomes:

```rust
/// What a player sends a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub pose: Option<HeroPose>,
    /// How the hero moves: WoW's walk toggle, and whether they move at all (spec §5.1).
    pub gait: Gait,
    /// How many times the player has pressed Stealth, counting round past 255: a count rather
    /// than a flag, so a press survives an input replaced before the host steps.
    pub stealth_presses: u8,
}
```

- `Hero` gains, after `speed`:

```rust
    /// How they stand and move (spec §5.1).
    pub stance: Stance,
    /// In stealth (spec §6.3).
    pub stealthed: bool,
    /// How visible they are, 0..100 (spec §5.1).
    pub visibility: f32,
    /// The loudest noise they made in the last `noise_memory` seconds, yards: the rings by the
    /// player frame (spec §5.5).
    pub noise: f32,
```

- `World` gains, after `plan_cursor`:

```rust
    /// Each hero's senses, and later each enemy's mind: the host's alone, never sent.
    pub(crate) senses: Senses,
```

and `World::new` adds `senses: Senses::default(),`.
- In `add_hero`'s `Hero { … }` literal, after `speed: def.speed,` add:

```rust
            stance: Stance::Still,
            stealthed: false,
            visibility: self.content.rules.senses.visibility.still,
            noise: 0.0,
```

- `remove_hero` gains `self.senses.forget(player);` after its two `retain`s.
- Replace `step` with:

```rust
    /// One step: each hero takes its player's input (the gait and Stealth presses first, then the
    /// pose, kept inside the map; a pose that is not a number is dropped, so a broken client
    /// cannot poison the world), then every hero's senses are worked out; once a map is attached,
    /// every enemy goes about its business.
    pub fn step(&mut self) {
        let bounds = &self.content.map.bounds;
        for (player, input) in self.inputs.drain(..) {
            let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) else {
                continue;
            };
            senses::take_input(&mut self.senses, hero, &input, &self.content);
            let Some(pose) = input.pose else { continue };
            if !pose.pos.is_finite() || !pose.facing.is_finite() {
                continue;
            }
            hero.pos = bounds.clamp(pose.pos);
            if pose.z.is_finite() {
                // On the host, the floor nearest the reported height, so a hero on a stair sees
                // from where they stand; a height with no floor near it (a jump) is kept as it is.
                let pos = hero.pos;
                hero.z = self
                    .nav
                    .as_ref()
                    .and_then(|nav| nav.map().floor_within(pos, pose.z, HERO_FLOOR_REACH))
                    .unwrap_or(pose.z);
            }
            let facing = pose.facing.normalize_or_zero();
            if facing != V2::ZERO {
                hero.facing = facing;
            }
        }
        senses::step_heroes(&mut self.senses, &mut self.heroes, &self.content);
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
        // Noises are heard the step they are made, or the next; none waits longer.
        self.senses.noises.clear();
        self.elapsed += DT;
    }
```

In `crates/benilla-app/src/nightfall/run.rs`'s `step_sim`, `PlayerInput { pose: input.pose }` becomes `PlayerInput { pose: input.pose, ..Default::default() }` and `PlayerInput { pose: Some(pose) }` becomes `PlayerInput { pose: Some(pose), ..Default::default() }` (Task 9 gives them the gait and presses).

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (everything before, and the 9 tests in `senses::tests`).
Run: `cargo check -p benilla-app`
Expected: builds.

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim crates/benilla-app/src/nightfall/run.rs
git commit -m "nightfall-sim: each hero's stance, Stealth toggle, visibility, speed and footsteps

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 5: Awareness: an enemy's state of mind

**Files:**
- Create: `crates/nightfall-sim/src/senses/awareness.rs`
- Modify: `crates/nightfall-sim/src/senses/mod.rs`, `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `AwarenessRules` (Task 1); `PlayerId`, `V3`, `DT`.
- Produces:
  - `nightfall_sim::Alert::{Suspicious, Alerted}` and `nightfall_sim::Mark { enemy: u32, alert: Alert }` (`Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize`).
  - Crate-only: `Sensed { player, noticed: Option<(V3, f32)>, in_sight: Option<V3>, heard: Option<V3> }` (`Default`); `Suspicion { player, level, place }`; `State::{Calm, Wary { player, place }, Checking { player, place }, Alerted { player, last_seen, unseen }, Searching { player, place, left }}`; `Shout { player, at }`; `Mind { cone: f32, sleeper: bool, of: Vec<Suspicion>, state: State, there: bool }` with `new(f32, bool)`, `blind()`, `level(PlayerId) -> f32`, `is_alerted()`, `alerted_to(PlayerId)`, `mark(PlayerId, &AwarenessRules) -> Option<Alert>`, `think(&mut self, &[Sensed], dt: f32, &AwarenessRules) -> Option<Shout>`, `alert(&mut self, PlayerId, V3, &AwarenessRules)`, `forget(&mut self, PlayerId)`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/senses/awareness.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::DT;

    const AT: V3 = V3::new(10.0, 10.0, 0.0);
    const ELSEWHERE: V3 = V3::new(20.0, 5.0, 0.0);

    fn sees(player: PlayerId, at: V3, fill: f32) -> Sensed {
        Sensed {
            player,
            noticed: Some((at, fill)),
            in_sight: Some(at),
            heard: None,
        }
    }

    fn hears(player: PlayerId, at: V3) -> Sensed {
        Sensed {
            player,
            heard: Some(at),
            ..Sensed::default()
        }
    }

    fn nothing(player: PlayerId) -> Sensed {
        Sensed {
            player,
            ..Sensed::default()
        }
    }

    /// `secs` of thinking with the same senses each step; the shouts it gave.
    fn think_for(m: &mut Mind, sensed: &[Sensed], secs: f32, r: &AwarenessRules) -> Vec<Shout> {
        (0..(secs / DT).round() as usize)
            .filter_map(|_| m.think(sensed, DT, r))
            .collect()
    }

    #[test]
    fn seeing_fills_at_its_rate_and_a_noise_adds_its_share() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(0, AT, 30.0)], 1.0, &r);
        assert!((m.level(0) - 30.0).abs() < 0.01, "{}", m.level(0));
        assert_eq!(m.state, State::Calm);
        m.think(&[hears(0, ELSEWHERE)], DT, &r);
        assert!((m.level(0) - 65.0).abs() < 0.01, "{}", m.level(0));
        assert_eq!(
            m.state,
            State::Checking {
                player: 0,
                place: ELSEWHERE
            }
        );
    }

    #[test]
    fn without_sight_or_noise_suspicion_drains() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(0, AT, 50.0)], 1.0, &r);
        think_for(&mut m, &[nothing(0)], 0.5, &r);
        assert!((m.level(0) - 45.0).abs() < 0.01);
        assert!(matches!(m.state, State::Wary { player: 0, .. }));
        think_for(&mut m, &[nothing(0)], 1.0, &r);
        assert!((m.level(0) - 35.0).abs() < 0.01);
        assert_eq!(m.state, State::Calm);
    }

    #[test]
    fn the_states_follow_the_thresholds_and_it_shouts_once() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(0, AT, 60.0)], 0.7, &r);
        assert_eq!(m.state, State::Wary { player: 0, place: AT });
        think_for(&mut m, &[sees(0, AT, 60.0)], 0.4, &r);
        assert_eq!(m.state, State::Checking { player: 0, place: AT });
        let shouts = think_for(&mut m, &[sees(0, AT, 60.0)], 1.0, &r);
        assert_eq!(shouts, vec![Shout { player: 0, at: AT }]);
        assert!(m.alerted_to(0));
    }

    #[test]
    fn a_check_keeps_going_until_suspicion_falls_below_suspicious() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.think(&[hears(0, AT)], DT, &r);
        m.think(&[hears(0, AT)], DT, &r);
        assert!(matches!(m.state, State::Checking { .. }));
        // 70 drains below `check` within a second, but the check goes on down to `suspicious`.
        think_for(&mut m, &[nothing(0)], 2.5, &r);
        assert!(matches!(m.state, State::Checking { .. }));
        think_for(&mut m, &[nothing(0)], 1.0, &r);
        assert_eq!(m.state, State::Calm);
    }

    #[test]
    fn an_alerted_enemy_holds_while_it_sees_and_searches_after_losing_sight() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        think_for(&mut m, &[sees(0, ELSEWHERE, 10.0)], 1.0, &r);
        think_for(&mut m, &[nothing(0)], 4.9, &r);
        assert!(m.alerted_to(0));
        assert_eq!(m.level(0), 100.0);
        think_for(&mut m, &[nothing(0)], 0.2, &r);
        assert_eq!(
            m.state,
            State::Searching {
                player: 0,
                place: ELSEWHERE,
                left: 15.0
            }
        );
        assert_eq!(m.mark(0, &r), Some(Alert::Suspicious));
    }

    #[test]
    fn a_search_counts_only_once_there_and_ends_unaware() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        think_for(&mut m, &[nothing(0)], 5.1, &r);
        think_for(&mut m, &[nothing(0)], 20.0, &r);
        assert!(matches!(m.state, State::Searching { .. }));
        m.there = true;
        think_for(&mut m, &[nothing(0)], 14.9, &r);
        assert!(matches!(m.state, State::Searching { .. }));
        think_for(&mut m, &[nothing(0)], 0.2, &r);
        assert_eq!(m.state, State::Calm);
        assert_eq!(m.level(0), 0.0);
        assert_eq!(m.mark(0, &r), None);
    }

    #[test]
    fn seeing_its_hero_during_a_search_alerts_it_again() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        think_for(&mut m, &[nothing(0)], 5.1, &r);
        let shouts = think_for(&mut m, &[sees(0, ELSEWHERE, 60.0)], 1.0, &r);
        assert_eq!(shouts.len(), 1);
        assert!(m.alerted_to(0));
    }

    #[test]
    fn a_sleeper_hears_but_does_not_see_until_alerted() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, true);
        think_for(&mut m, &[sees(0, AT, 60.0)], 3.0, &r);
        assert_eq!(m.level(0), 0.0);
        assert!(m.blind());
        assert_eq!(m.think(&[hears(0, AT)], DT, &r), None);
        assert_eq!(m.think(&[hears(0, AT)], DT, &r), None);
        assert_eq!(
            m.think(&[hears(0, AT)], DT, &r),
            Some(Shout { player: 0, at: AT })
        );
        assert!(!m.blind());
        m.think(&[sees(0, ELSEWHERE, 60.0)], DT, &r);
        assert!(matches!(
            m.state,
            State::Alerted { last_seen, .. } if last_seen == ELSEWHERE
        ));
    }

    #[test]
    fn marks_say_what_it_thinks_of_each_hero() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        assert_eq!(m.mark(0, &r), Some(Alert::Alerted));
        assert_eq!(m.mark(1, &r), None);
        think_for(&mut m, &[sees(0, AT, 10.0), sees(1, ELSEWHERE, 45.0)], 1.0, &r);
        assert_eq!(m.mark(1, &r), Some(Alert::Suspicious));
        assert!(m.alerted_to(0));
    }

    #[test]
    fn ties_go_to_the_lower_player() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(1, ELSEWHERE, 45.0), sees(0, AT, 45.0)], 1.0, &r);
        assert_eq!(m.state, State::Wary { player: 0, place: AT });
    }

    #[test]
    fn a_forgotten_hero_leaves_no_trace() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        m.forget(0);
        assert_eq!(m.state, State::Calm);
        assert_eq!(m.level(0), 0.0);
        assert_eq!(m.mark(0, &r), None);
    }
}
```

In `senses/mod.rs`, add `mod awareness;` above `mod noise;` and `pub use awareness::{Alert, Mark};` beside the other `pub use`. In `lib.rs`, change the senses `pub use` to `pub use senses::{Alert, Gait, Loudness, Mark, Stance};`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim senses::awareness`
Expected: FAIL to compile (`cannot find type Mind`).

- [ ] **Step 3: Implement the mind**

At the top of `crates/nightfall-sim/src/senses/awareness.rs`:

```rust
//! What an enemy makes of the heroes (spec §5.3): a suspicion of each, 0 to 100, raised by seeing
//! and hearing them and drained by time, and the state that puts the enemy in. Unaware below
//! `suspicious`; wary ("?": it stops and looks) from there; checking (it goes to look) from
//! `check`, and still on its way while suspicion stays at `suspicious` or more; alerted ("!":
//! after its hero, shouting to its allies) at `alert`. An alerted enemy out of sight of its hero
//! `lose_after` seconds searches where it last saw them for `search_for` seconds once there, then
//! goes back unaware. A sleeper (spec §7.1) hears but does not see until alerted.
//!
//! Pure state: [`super::react`] tells it what the enemy saw and heard each step, and moves the
//! enemy as it says.

use serde::{Deserialize, Serialize};

use crate::{AwarenessRules, PlayerId, V3};

/// What an enemy shows a player over its head (spec §5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alert {
    /// "?": suspicious of you, or searching for you.
    Suspicious,
    /// "!": alerted to you.
    Alerted,
}

/// One enemy's mark for one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub enemy: u32,
    pub alert: Alert,
}

/// What an enemy made of one hero this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sensed {
    pub player: PlayerId,
    /// Noticed within its notice range: where the hero stands, and how fast suspicion fills a
    /// second.
    pub noticed: Option<(V3, f32)>,
    /// In plain sight (within the fog, in line of sight), noticed or not: how an alerted enemy
    /// keeps track of its hero.
    pub in_sight: Option<V3>,
    /// The floor a noise of theirs came from, the loudest heard this step.
    pub heard: Option<V3>,
}

/// Its suspicion of one hero, and the place to check for them: where they were last seen or heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Suspicion {
    pub player: PlayerId,
    pub level: f32,
    pub place: V3,
}

/// An enemy's state of mind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum State {
    /// About its business.
    Calm,
    /// It stops and faces `place`.
    Wary { player: PlayerId, place: V3 },
    /// It goes to look at `place`.
    Checking { player: PlayerId, place: V3 },
    /// After `player`, last seen at `last_seen`, out of sight these `unseen` seconds.
    Alerted {
        player: PlayerId,
        last_seen: V3,
        unseen: f32,
    },
    /// It lost `player`: it goes to `place` and looks around, `left` seconds once there.
    Searching {
        player: PlayerId,
        place: V3,
        left: f32,
    },
}

/// A shout: allies in reach go after `player`, last seen at `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shout {
    pub player: PlayerId,
    pub at: V3,
}

/// One enemy's mind.
#[derive(Clone, Debug)]
pub(crate) struct Mind {
    /// Its front cone, degrees.
    pub cone: f32,
    /// Hears but does not see until alerted.
    pub sleeper: bool,
    /// Its suspicion of each hero met.
    pub of: Vec<Suspicion>,
    pub state: State,
    /// It has reached the place it is checking or searching (or found no way there).
    pub there: bool,
}

impl Mind {
    pub(crate) fn new(cone: f32, sleeper: bool) -> Self {
        Self {
            cone,
            sleeper,
            of: Vec::new(),
            state: State::Calm,
            there: false,
        }
    }

    /// Blind now: a sleeper that has not been alerted since it last lay down.
    pub(crate) fn blind(&self) -> bool {
        self.sleeper && !matches!(self.state, State::Alerted { .. } | State::Searching { .. })
    }

    /// Its suspicion of `player`'s hero.
    pub(crate) fn level(&self, player: PlayerId) -> f32 {
        self.of
            .iter()
            .find(|s| s.player == player)
            .map_or(0.0, |s| s.level)
    }

    pub(crate) fn is_alerted(&self) -> bool {
        matches!(self.state, State::Alerted { .. })
    }

    pub(crate) fn alerted_to(&self, player: PlayerId) -> bool {
        matches!(self.state, State::Alerted { player: p, .. } if p == player)
    }

    /// What it shows `player` (spec §5.5): "!" alerted to them, "?" searching for them or at least
    /// `suspicious` of them.
    pub(crate) fn mark(&self, player: PlayerId, r: &AwarenessRules) -> Option<Alert> {
        match self.state {
            State::Alerted { player: p, .. } if p == player => Some(Alert::Alerted),
            State::Searching { player: p, .. } if p == player => Some(Alert::Suspicious),
            _ if self.level(player) >= r.suspicious => Some(Alert::Suspicious),
            _ => None,
        }
    }

    fn entry(&mut self, player: PlayerId) -> &mut Suspicion {
        let i = match self.of.iter().position(|s| s.player == player) {
            Some(i) => i,
            None => {
                self.of.push(Suspicion {
                    player,
                    level: 0.0,
                    place: V3::default(),
                });
                self.of.len() - 1
            }
        };
        &mut self.of[i]
    }

    /// Alerted to `player`, last seen at `at`: by an ally's shout, or a hit (sub-project 4).
    pub(crate) fn alert(&mut self, player: PlayerId, at: V3, r: &AwarenessRules) {
        let e = self.entry(player);
        e.level = r.alert;
        e.place = at;
        self.state = State::Alerted {
            player,
            last_seen: at,
            unseen: 0.0,
        };
        self.there = false;
    }

    /// A hero left the run: nothing of them stays, and a state about them ends.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.of.retain(|s| s.player != player);
        let about = match self.state {
            State::Calm => None,
            State::Wary { player: p, .. }
            | State::Checking { player: p, .. }
            | State::Alerted { player: p, .. }
            | State::Searching { player: p, .. } => Some(p),
        };
        if about == Some(player) {
            self.state = State::Calm;
            self.there = false;
        }
    }

    /// One step of thinking (spec §5.3): what it saw and heard of each hero raises its suspicion
    /// of them, neither drains it, and the state follows. The hero it is alerted to is held at
    /// `alert` while it is after them. Returns a shout the step it becomes alerted.
    pub(crate) fn think(&mut self, sensed: &[Sensed], dt: f32, r: &AwarenessRules) -> Option<Shout> {
        let blind = self.blind();
        for s in sensed {
            let chasing = self.alerted_to(s.player);
            if let State::Alerted {
                player,
                last_seen,
                unseen,
            } = &mut self.state
            {
                if *player == s.player {
                    *unseen += dt;
                    if let Some(at) = s.in_sight.or(s.noticed.map(|n| n.0)) {
                        *last_seen = at;
                        *unseen = 0.0;
                    }
                }
            }
            let e = self.entry(s.player);
            let mut fed = false;
            if let (false, Some((at, fill))) = (blind, s.noticed) {
                e.level += fill * dt;
                e.place = at;
                fed = true;
            }
            if let Some(at) = s.heard {
                e.level += r.noise;
                e.place = at;
                fed = true;
            }
            if chasing {
                e.level = r.alert;
                fed = true;
            }
            if !fed {
                e.level -= r.drain * dt;
            }
            e.level = e.level.clamp(0.0, r.alert);
        }
        if let State::Alerted {
            player,
            last_seen,
            unseen,
        } = self.state
        {
            if unseen >= r.lose_after {
                self.state = State::Searching {
                    player,
                    place: last_seen,
                    left: r.search_for,
                };
                self.there = false;
                self.entry(player).level = r.check;
            }
            return None;
        }
        // The hero it is most suspicious of; on a tie the lower player, so a run plays the same.
        let top = self
            .of
            .iter()
            .copied()
            .max_by(|a, b| a.level.total_cmp(&b.level).then(b.player.cmp(&a.player)));
        if let Some(t) = top.filter(|t| t.level >= r.alert) {
            self.state = State::Alerted {
                player: t.player,
                last_seen: t.place,
                unseen: 0.0,
            };
            self.there = false;
            return Some(Shout {
                player: t.player,
                at: t.place,
            });
        }
        if let State::Searching { left, .. } = &mut self.state {
            if self.there {
                *left -= dt;
            }
            if *left <= 0.0 {
                self.state = State::Calm;
                self.there = false;
                for s in &mut self.of {
                    s.level = 0.0;
                }
            }
            return None;
        }
        let checking = matches!(self.state, State::Checking { .. });
        let next = match top {
            Some(t) if t.level >= r.check || (checking && t.level >= r.suspicious) => {
                State::Checking {
                    player: t.player,
                    place: t.place,
                }
            }
            Some(t) if t.level >= r.suspicious => State::Wary {
                player: t.player,
                place: t.place,
            },
            _ => State::Calm,
        };
        // A check going on to the same place keeps its arrival; anything else starts afresh.
        let same = matches!(
            (self.state, next),
            (State::Checking { place: a, .. }, State::Checking { place: b, .. })
                if a.ground().distance(b.ground()) <= 1.0
        );
        if !same {
            self.there = false;
        }
        self.state = next;
        None
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim senses::awareness`
Expected: PASS (11 tests). Warnings that `Mind` and its kin are unused go away in Task 6.

- [ ] **Step 5: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: an enemy's suspicion of each hero, its states, shouts and searches

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Enemies react: see, hear, turn, check, chase, shout, search, go home

**Files:**
- Create: `crates/nightfall-sim/src/senses/react.rs`
- Modify: `crates/nightfall-sim/src/senses/mod.rs`, `crates/nightfall-sim/src/senses/tests.rs`, `crates/nightfall-sim/src/enemies.rs`, `crates/nightfall-sim/src/world.rs`, `crates/nightfall-sim/src/sight.rs` (test fixture), `crates/nightfall-sim/src/messages.rs` (test fixture)

**Interfaces:**
- Consumes: `Mind`, `Sensed`, `State`, `Shout`, `Mark`, `Alert` (Task 5); `facing_factor`, `notice_range`, `fill_rate` (Task 2); `Noise`, `hears` (Task 3); `Senses` (Task 4); `Enemy`, `Brain`, `Order`, `spawn_all`, `enemies::step`, `ARRIVE`, `MAX_STEP`, `WANDER_BUDGET`, `Navigator::{map, find_path}`, `HeightMap::{line_of_sight, walk_line, floor_within}`, `EYE_HEIGHT`, `CHEST_HEIGHT` (world knowledge Tasks 1–3, 8, 9).
- Produces:
  - `Enemy` gains `asleep: bool` (a sleeper lying at its post, for the client).
  - `World::marks_for(&self, player: PlayerId) -> Vec<Mark>`; `World::alert(&mut self, enemy: u32, by: PlayerId)`.
  - Crate-only in `enemies.rs`: `pub(crate) const ARRIVE`, `pub(crate) fn heading`, `pub(crate) fn follow(e: &mut Enemy, path: &[V3], next: &mut usize, speed: f32, map: &HeightMap) -> bool`, `Brain::{spawn(&self) -> usize, hold(&mut self, bool), at_post(&self, V2) -> bool}`.
  - Crate-only in `senses`: `Senses::place(&mut self, &Content, impl IntoIterator<Item = usize>)`; `step_enemies(&mut Senses, &mut [Enemy], &mut [Brain], &[Hero], &mut Navigator, &Content)`; `react::Way`.

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/senses/tests.rs`, change the imports to:

```rust
use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Alert, Content, ContentSources, Gait, HeroPose, Loudness, Mark, PlayerId, PlayerInput, Stance,
    World, V2, V3,
};
```

and append:

```rust
#[test]
fn a_walking_hero_in_front_of_a_sentry_is_spotted_then_chased() {
    let mut w = world(&sentry(25.0, 25.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 37.0, 25.0, Gait::Walk, 0);
    let mut was_suspicious = false;
    for _ in 0..300 {
        w.step();
        was_suspicious |= w.marks_for(0)
            == [Mark {
                enemy: 1,
                alert: Alert::Suspicious,
            }];
    }
    assert!(was_suspicious);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Alerted
        }]
    );
    let e = &w.enemies[0];
    let to_hero = V2::new(37.0, 25.0) - e.pos;
    assert!(to_hero.length() <= 2.3, "{:?}", e.pos);
    assert!(e.facing.dot(to_hero.normalize_or_zero()) > 0.9, "{:?}", e.facing);
}

#[test]
fn a_sneaking_rogue_behind_a_sentry_goes_unseen_and_a_walker_does_not() {
    let marks = |presses: u8| {
        let mut w = world(&sentry(25.0, 25.0, 0.0));
        w.attach_map(open());
        put(&mut w, 0, 20.5, 25.0, Gait::Walk, 0);
        w.step();
        put(&mut w, 0, 20.5, 25.0, Gait::Walk, presses);
        steps(&mut w, 300);
        (w.marks_for(0), w.enemies[0].pos)
    };
    let (sneaking, at) = marks(1);
    assert!(sneaking.is_empty(), "{sneaking:?}");
    assert_eq!(at, V2::new(25.0, 25.0));
    let (walking, _) = marks(0);
    assert!(!walking.is_empty());
}

#[test]
fn through_a_wall_running_is_heard_at_half_the_distance() {
    let run_behind_wall = |x: f32| {
        let mut w = world(&sentry(20.0, 25.0, 180.0));
        let mut b = flat(50.0);
        b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
        w.attach_map(Arc::new(b.finish()));
        put(&mut w, 0, x, 25.0, Gait::Run, 0);
        steps(&mut w, 45);
        (w.marks_for(0), w.enemies[0].pos)
    };
    let (near, at) = run_behind_wall(25.5);
    assert!(!near.is_empty());
    assert!(at.x < 24.0, "it never walks through the wall: {at:?}");
    let (far, _) = run_behind_wall(29.0);
    assert!(far.is_empty(), "{far:?}");
}

#[test]
fn an_enemy_goes_to_check_the_noise_not_the_hero() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 45.0, 45.0, Gait::Still, 0);
    w.step();
    let at = V3::new(14.0, 14.0, 0.0);
    w.make_noise(0, at, Loudness::Normal);
    w.step();
    w.make_noise(0, at, Loudness::Normal);
    steps(&mut w, 75);
    let e = &w.enemies[0];
    assert!(e.pos.distance(V2::new(14.0, 14.0)) <= 1.0, "{:?}", e.pos);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Suspicious
        }]
    );
}

#[test]
fn a_shout_brings_allies_within_reach_and_not_beyond() {
    // A sees the hero; B is 12 yards from A, C 22 from A but only 10 from B.
    let spawns = format!(
        "{}{}{}",
        sentry(10.0, 25.0, 0.0),
        sentry(10.0, 37.0, 180.0),
        sentry(10.0, 47.0, 180.0)
    );
    let mut w = world(&spawns);
    w.attach_map(open());
    put(&mut w, 0, 16.0, 25.0, Gait::Walk, 0);
    let alerted = |w: &World, enemy: u32| {
        w.marks_for(0).contains(&Mark {
            enemy,
            alert: Alert::Alerted,
        })
    };
    let mut a = false;
    for _ in 0..300 {
        w.step();
        if alerted(&w, 1) {
            a = true;
            break;
        }
    }
    assert!(a);
    assert!(alerted(&w, 2), "{:?}", w.marks_for(0));
    assert!(
        !w.marks_for(0).iter().any(|m| m.enemy == 3),
        "a shout does not chain: {:?}",
        w.marks_for(0)
    );
}

#[test]
fn a_lost_hero_is_searched_for_then_the_sentry_goes_home() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    let mut b = flat(50.0);
    house(&mut b, V2::new(30.0, 30.0), V2::new(40.0, 40.0), None, true);
    w.attach_map(Arc::new(b.finish()));
    put(&mut w, 0, 20.0, 10.0, Gait::Run, 0);
    let alerted = [Mark {
        enemy: 1,
        alert: Alert::Alerted,
    }];
    let mut was_alerted = false;
    for _ in 0..150 {
        w.step();
        if w.marks_for(0) == alerted {
            was_alerted = true;
            break;
        }
    }
    assert!(was_alerted);
    // The hero slips into a closed, roofed house: out of every sight and every path.
    put(&mut w, 0, 35.0, 35.0, Gait::Still, 0);
    let searching = [Mark {
        enemy: 1,
        alert: Alert::Suspicious,
    }];
    let mut searched_there = false;
    for _ in 0..900 {
        w.step();
        searched_there |= w.marks_for(0) == searching
            && w.enemies[0].pos.distance(V2::new(20.0, 10.0)) <= 1.0;
    }
    assert!(searched_there);
    assert!(w.marks_for(0).is_empty());
    let e = &w.enemies[0];
    assert!(e.pos.distance(V2::new(10.0, 10.0)) <= 0.6, "{:?}", e.pos);
    assert!(e.facing.x > 0.99, "{:?}", e.facing);
}

#[test]
fn a_place_no_path_reaches_is_searched_from_where_it_stands() {
    let mut w = world(&sentry(20.0, 25.0, 0.0));
    let mut b = flat(50.0);
    b.add_box(V2::new(24.0, 0.0), V2::new(24.5, 50.0), 0.0, 3.0);
    w.attach_map(Arc::new(b.finish()));
    put(&mut w, 0, 30.0, 25.0, Gait::Still, 0);
    w.step();
    w.alert(1, 0);
    w.step();
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Alerted
        }]
    );
    // Five seconds unseen, fifteen looking around where it stands, then home.
    steps(&mut w, 900);
    assert!(w.marks_for(0).is_empty());
    let e = &w.enemies[0];
    assert!(e.pos.x < 24.0, "{:?}", e.pos);
    assert!(e.pos.distance(V2::new(20.0, 25.0)) <= 0.6, "{:?}", e.pos);
}

#[test]
fn a_sleeper_sleeps_through_a_hero_in_front_and_wakes_to_noise() {
    let mut w = world(
        r#"(place: "Crypt", kind: "skeleton", behaviour: Sleeper(at: (10.0, 10.0), facing: 0.0)),"#,
    );
    w.attach_map(open());
    put(&mut w, 0, 13.0, 10.0, Gait::Still, 0);
    steps(&mut w, 90);
    assert!(w.marks_for(0).is_empty());
    assert!(w.enemies[0].asleep);
    for _ in 0..3 {
        w.make_noise(0, V3::new(13.0, 10.0, 0.0), Loudness::Normal);
        w.step();
    }
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: 1,
            alert: Alert::Alerted
        }]
    );
    assert!(!w.enemies[0].asleep);
}

#[test]
fn a_hero_who_leaves_mid_chase_is_forgotten() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 20.0, 10.0, Gait::Still, 0);
    put(&mut w, 1, 5.0, 40.0, Gait::Still, 0);
    w.step();
    w.alert(1, 0);
    steps(&mut w, 10);
    w.remove_hero(0);
    steps(&mut w, 300);
    assert!(w.marks_for(0).is_empty());
    assert!(w.marks_for(1).is_empty());
    let e = &w.enemies[0];
    assert!(e.pos.distance(V2::new(10.0, 10.0)) <= 0.6, "{:?}", e.pos);
}

#[test]
fn the_same_run_twice_is_the_same_with_the_senses() {
    let run = || {
        let spawns = format!(
            "{}{}",
            sentry(25.0, 25.0, 0.0),
            r#"(place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (15.0, 35.0), radius: 6.0)),"#
        );
        let mut w = world(&spawns);
        w.attach_map(open());
        for k in 0..300 {
            put(&mut w, 0, 30.0 + (k % 60) as f32 * 0.1, 25.0, Gait::Run, 0);
            w.step();
        }
        (w.snapshot(), w.marks_for(0))
    };
    assert_eq!(run(), run());
}
```

In `crates/nightfall-sim/src/sight.rs`'s test `without_a_map_only_distance_counts` and `crates/nightfall-sim/src/messages.rs`'s test `a_full_partys_state_is_small`, each `Enemy { … }` literal gains `asleep: false,` after `speed`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim senses`
Expected: FAIL to compile (`no method named marks_for`, `no field asleep on type Enemy`).

- [ ] **Step 3: Let the brains be held**

In `crates/nightfall-sim/src/enemies.rs`:

- The module doc's last sentence becomes `What they notice is the senses' ([`crate::senses`]): while an enemy is not calm its [`Brain`] is held, and on release it finds its way back to its order. How they fight comes with sub-project 4.`
- `const ARRIVE` becomes `pub(crate) const ARRIVE`, and `fn heading` becomes `pub(crate) fn heading`.
- `Enemy` gains, after `speed`:

```rust
    /// A sleeper lying at its post (spec §7.1), which the client draws lying down.
    pub asleep: bool,
```

- `Brain` gains, after `warned`:

```rust
    /// Held by its senses: its order waits while it turns, checks, chases or searches.
    held: bool,
```

- In `spawn_all`, the `Enemy { … }` literal gains `asleep: matches!(s.behaviour, Behaviour::Sleeper { .. }),` after `speed: 0.0,`, and the `Brain { … }` literal gains `held: false,` after `warned: false,`.
- Add after the `Brain` struct:

```rust
impl Brain {
    /// Its spawn line in the content.
    pub(crate) fn spawn(&self) -> usize {
        self.spawn
    }

    /// Held by its senses, or released. Taking hold drops the path it was on, so on release it
    /// searches its way back: to its post, or to the patrol point it was going to.
    pub(crate) fn hold(&mut self, on: bool) {
        if on && !self.held {
            self.path = None;
            self.next = 0;
            self.wait = 0.0;
        }
        self.held = on;
    }

    /// Whether `pos` is at its post (a sentry's or a sleeper's).
    pub(crate) fn at_post(&self, pos: V2) -> bool {
        matches!(&self.order, Order::Sentry { post, .. } if post.ground().distance(pos) <= ARRIVE)
    }
}
```

- In `plan`, the first line becomes `if b.held || b.wait > 0.0 || b.path.is_some() || *budget == 0 {`.
- Replace `walk` with:

```rust
/// An enemy walks its path for a step at its walking speed, on the floor under it; one its senses
/// hold is moved by them instead.
fn walk(e: &mut Enemy, b: &mut Brain, map: &HeightMap, content: &Content) {
    if b.held {
        return;
    }
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
    if follow(e, &path, &mut b.next, speed, map) {
        b.path = None;
        arrive(e, b, content);
    }
}

/// `e` goes a step along `path` from point `next` at `speed`, on the floor under it, turned the
/// way it moves; whether it has reached the end.
pub(crate) fn follow(
    e: &mut Enemy,
    path: &[V3],
    next: &mut usize,
    speed: f32,
    map: &HeightMap,
) -> bool {
    let from = e.pos;
    let mut left = speed * DT;
    while left > 1e-6 && *next < path.len() {
        let to = path[*next];
        let d = e.pos.distance(to.ground());
        if d <= left {
            e.pos = to.ground();
            e.z = to.z;
            left -= d;
            *next += 1;
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
    *next >= path.len()
}
```

- [ ] **Step 4: Write the reactions**

`crates/nightfall-sim/src/senses/react.rs`:

```rust
//! Enemies acting on what they make of the heroes (spec §5.3). Each step every enemy looks (its
//! notice range, by its facing and the hero's visibility, in line of sight and never past the
//! fog) and listens (the noises made since the last step), thinks ([`Mind::think`]), and shouts
//! the step it becomes alerted: every ally within `shout` yards is alerted to the same hero, and
//! those allies do not shout on. An enemy that is not calm holds its [`Brain`]: wary, it stops and
//! faces the place; checking, it walks there and stands; alerted, it runs at its hero to
//! `chase_reach` and faces them (the swing is sub-project 4's); searching, it walks to where its
//! hero was last seen and looks around. A place no path reaches is looked at from where it
//! stands. Calm again, its brain finds its way back to its order.

use std::sync::Arc;

use super::awareness::{Mind, Sensed, State};
use super::noise::{hears, Noise};
use super::visibility::{facing_factor, fill_rate, notice_range};
use super::Senses;
use crate::enemies::{self, heading, Brain, Enemy, ARRIVE, WANDER_BUDGET};
use crate::heightmap::{HeightMap, Navigator};
use crate::{Content, Hero, SenseRules, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

/// Seconds between turns while an enemy looks around.
const LOOK_EVERY: f32 = 2.5;
/// A chase searches its path again once its hero is this far from the path's end, yards.
const REPATH: f32 = 2.0;
/// Seconds before searching again for a goal no path reached.
const RETRY: f32 = 1.0;

/// The way an enemy goes while its senses hold it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Way {
    route: Option<Route>,
    /// Seconds before searching again for a goal no path reached.
    retry: f32,
}

#[derive(Clone, Debug)]
struct Route {
    goal: V3,
    path: Arc<[V3]>,
    next: usize,
}

/// How a step toward a place went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Progress {
    There,
    Going,
    /// No way there: it stands.
    Stuck,
}

/// One step of every enemy's senses: look, listen, think, shout, act.
pub(crate) fn step_enemies(
    s: &mut Senses,
    enemies: &mut [Enemy],
    brains: &mut [Brain],
    heroes: &[Hero],
    nav: &mut Navigator,
    content: &Content,
) {
    if s.minds.len() != enemies.len() {
        return;
    }
    let r = &content.rules.senses;
    let fog = content.rules.atmosphere.fog_end;
    let map = nav.map().clone();
    let noises = std::mem::take(&mut s.noises);
    let mut shouts = std::mem::take(&mut s.shouts);
    for (i, e) in enemies.iter().enumerate() {
        let mind = &mut s.minds[i];
        let eye = V3::at(e.pos, e.z + EYE_HEIGHT);
        let sensed: Vec<Sensed> = heroes
            .iter()
            .map(|h| sense(e, mind, eye, h, &noises, &map, fog, r))
            .collect();
        if let Some(shout) = mind.think(&sensed, DT, &r.awareness) {
            shouts.push((i, shout));
        }
    }
    for (i, shout) in shouts {
        let from = enemies[i].pos;
        for (j, ally) in enemies.iter().enumerate() {
            if j != i
                && ally.pos.distance(from) <= r.awareness.shout
                && !s.minds[j].is_alerted()
            {
                s.minds[j].alert(shout.player, shout.at, &r.awareness);
            }
        }
    }
    let mut searches = content.rules.world.paths_per_step;
    for (i, e) in enemies.iter_mut().enumerate() {
        let mind = &mut s.minds[i];
        let held = mind.state != State::Calm;
        brains[i].hold(held);
        if !held {
            s.ways[i] = Way::default();
            e.asleep = mind.sleeper && brains[i].at_post(e.pos);
            continue;
        }
        e.asleep = false;
        react(e, mind, &mut s.ways[i], &map, nav, content, &mut searches);
    }
}

/// What enemy `e` sees and hears of hero `h` this step.
#[allow(clippy::too_many_arguments)]
fn sense(
    e: &Enemy,
    mind: &Mind,
    eye: V3,
    h: &Hero,
    noises: &[Noise],
    map: &HeightMap,
    fog: f32,
    r: &SenseRules,
) -> Sensed {
    let feet = V3::at(h.pos, h.z);
    let d = e.pos.distance(h.pos);
    let in_sight = (d <= fog && map.line_of_sight(eye, V3::at(h.pos, h.z + CHEST_HEIGHT)))
        .then_some(feet);
    let noticed = in_sight.filter(|_| !mind.blind()).and_then(|at| {
        let facing = facing_factor(e.facing, h.pos - e.pos, mind.cone, r);
        let range = notice_range(h.visibility, facing, fog, r);
        (d <= range).then(|| (at, fill_rate(d, range, &r.awareness)))
    });
    let heard = noises
        .iter()
        .filter(|n| n.by == h.player && hears(eye, n, Some(map), r.walls))
        .max_by(|a, b| a.radius.total_cmp(&b.radius))
        .map(|n| n.at);
    Sensed {
        player: h.player,
        noticed,
        in_sight,
        heard,
    }
}

/// A held enemy acts on its state for a step.
fn react(
    e: &mut Enemy,
    mind: &mut Mind,
    way: &mut Way,
    map: &Arc<HeightMap>,
    nav: &mut Navigator,
    content: &Content,
    searches: &mut u32,
) {
    let def = &content.enemies[e.kind];
    let r = &content.rules.senses;
    match mind.state {
        State::Calm => {}
        State::Wary { place, .. } => stand(e, Some(place.ground())),
        State::Checking { place, .. } | State::Searching { place, .. } if !mind.there => {
            if go(e, way, place, def.walk_speed, ARRIVE, map, nav, searches) != Progress::Going {
                mind.there = true;
            }
        }
        State::Checking { .. } => stand(e, None),
        State::Searching { left, .. } => look_around(e, left, r.awareness.search_for),
        State::Alerted { last_seen, .. } => {
            if go(e, way, last_seen, def.run_speed, r.chase_reach, map, nav, searches)
                != Progress::Going
            {
                stand(e, Some(last_seen.ground()));
            }
        }
    }
}

/// It stands, turned toward `face` when given.
fn stand(e: &mut Enemy, face: Option<V2>) {
    e.speed = 0.0;
    if let Some(p) = face {
        if p.distance(e.pos) > 1e-3 {
            e.facing = heading(p - e.pos);
        }
    }
}

/// It stands and looks around: a new way every [`LOOK_EVERY`] seconds of its search, the same
/// ways every run.
fn look_around(e: &mut Enemy, left: f32, total: f32) {
    e.speed = 0.0;
    let turn = ((total - left) / LOOK_EVERY).floor();
    e.facing = V2::from_angle(e.id as f32 * 1.3 + turn * 2.2, 1.0);
}

/// A step toward `goal` at `speed`, stopping `reach` short of it: straight when nothing stands in
/// the way, else along a path, searched again when the goal moves (at most `searches` this step).
/// A goal no path reaches is not searched for again for [`RETRY`] seconds.
#[allow(clippy::too_many_arguments)]
fn go(
    e: &mut Enemy,
    way: &mut Way,
    goal: V3,
    speed: f32,
    reach: f32,
    map: &Arc<HeightMap>,
    nav: &mut Navigator,
    searches: &mut u32,
) -> Progress {
    way.retry = (way.retry - DT).max(0.0);
    let to = goal.ground();
    let d = e.pos.distance(to);
    if d <= reach {
        e.speed = 0.0;
        way.route = None;
        return Progress::There;
    }
    let here = V3::at(e.pos, e.z);
    if map.walk_line(here, goal) {
        way.route = None;
        // Aim a little inside the reach, so it ends within it rather than on the goal.
        let aim = to - (to - e.pos) * (reach * 0.9 / d);
        let mut next = 0;
        enemies::follow(e, &[V3::at(aim, goal.z)], &mut next, speed, map);
        return Progress::Going;
    }
    let stale = way.route.as_ref().is_none_or(|rt| {
        rt.next >= rt.path.len() || rt.goal.ground().distance(to) > REPATH
    });
    if stale {
        if way.retry > 0.0 {
            e.speed = 0.0;
            return Progress::Stuck;
        }
        if *searches == 0 {
            e.speed = 0.0;
            return Progress::Going;
        }
        *searches -= 1;
        match nav.find_path(here, goal, false, WANDER_BUDGET) {
            Some(path) => {
                way.route = Some(Route {
                    goal,
                    path,
                    next: 0,
                })
            }
            None => {
                way.route = None;
                way.retry = RETRY;
                e.speed = 0.0;
                return Progress::Stuck;
            }
        }
    }
    let Some(rt) = way.route.as_mut() else {
        return Progress::Stuck;
    };
    let path = rt.path.clone();
    enemies::follow(e, &path, &mut rt.next, speed, map);
    Progress::Going
}
```

- [ ] **Step 5: Give the senses their minds**

In `crates/nightfall-sim/src/senses/mod.rs`:

- Add `mod react;` below `mod noise;` and `pub(crate) use react::step_enemies;` below the `pub use` lines; change the private imports to:

```rust
use awareness::{Mind, Shout};
use noise::step_radius;
use react::Way;
use visibility::{visibility, Light};

use crate::{Behaviour, Content, Hero, PlayerId, PlayerInput, World, DT, V3};
```

- The module doc's second paragraph becomes: `Each step, once the heroes have their inputs, [`step_heroes`] works out every hero's stance, visibility, speed and footsteps; then, once the map is attached, [`step_enemies`] lets every enemy look, listen, think and act.`
- Replace `pub(crate) struct Senses { … }` with:

```rust
/// The senses' state on the host.
#[derive(Clone, Debug, Default)]
pub(crate) struct Senses {
    heroes: Vec<HeroSense>,
    /// The noises made since the enemies last listened.
    pub(crate) noises: Vec<Noise>,
    /// Each enemy's mind, beside it in the world's enemies.
    minds: Vec<Mind>,
    /// Each enemy's way while its senses hold it.
    ways: Vec<Way>,
    /// Shouts raised between steps (an alert from a hit), given out at the next.
    shouts: Vec<(usize, Shout)>,
}
```

- In `impl Senses`, replace `forget` with:

```rust
    /// A player left: nothing of their hero stays, and no enemy keeps a thought of them.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.noises.retain(|n| n.by != player);
        self.shouts.retain(|(_, s)| s.player != player);
        for m in &mut self.minds {
            m.forget(player);
        }
    }

    /// A calm mind for each enemy placed from these spawn lines: a sentry's wide front cone, a
    /// sleeper's closed eyes, everyone else's own cone.
    pub(crate) fn place(&mut self, content: &Content, spawns: impl IntoIterator<Item = usize>) {
        let r = &content.rules.senses;
        self.minds = spawns
            .into_iter()
            .map(|si| match content.spawns[si].behaviour {
                Behaviour::Sentry { .. } => Mind::new(r.sentry_cone, false),
                Behaviour::Sleeper { .. } => Mind::new(r.cone, true),
                _ => Mind::new(r.cone, false),
            })
            .collect();
        self.ways = vec![Way::default(); self.minds.len()];
        self.shouts.clear();
    }
```

- Add to `impl World`:

```rust
    /// What each enemy thinks of `player`'s hero, for the marks over their heads (spec §5.5). The
    /// host's: a client's mirror has no minds and gets its own player's marks from its host.
    pub fn marks_for(&self, player: PlayerId) -> Vec<Mark> {
        let r = &self.content().rules.senses.awareness;
        self.enemies
            .iter()
            .zip(&self.senses.minds)
            .filter_map(|(e, m)| m.mark(player, r).map(|alert| Mark { enemy: e.id, alert }))
            .collect()
    }

    /// Enemy `enemy` is alerted to `by`'s hero where they stand, and shouts at the next step: the
    /// seam sub-project 4's hits use. Nothing happens for an enemy or a hero that is not there.
    pub fn alert(&mut self, enemy: u32, by: PlayerId) {
        let Some(at) = self.hero(by).map(|h| V3::at(h.pos, h.z)) else {
            return;
        };
        let Some(i) = self.enemies.iter().position(|e| e.id == enemy) else {
            return;
        };
        let content = self.content().clone();
        let Some(mind) = self.senses.minds.get_mut(i) else {
            return;
        };
        mind.alert(by, at, &content.rules.senses.awareness);
        self.senses.shouts.push((i, Shout { player: by, at }));
    }
```

In `crates/nightfall-sim/src/world.rs`:

- In `attach_map`, after `self.nav = Some(Navigator::new(map));` add:

```rust
        // Every enemy starts calm, with the eyes its behaviour gives it.
        self.senses
            .place(&self.content, self.brains.iter().map(Brain::spawn));
```

- In `step`, the `if let Some(nav) = &mut self.nav { … }` block becomes:

```rust
        if let Some(nav) = &mut self.nav {
            senses::step_enemies(
                &mut self.senses,
                &mut self.enemies,
                &mut self.brains,
                &self.heroes,
                nav,
                &self.content,
            );
            enemies::step(
                &mut self.enemies,
                &mut self.brains,
                nav,
                &self.content,
                &mut self.warnings,
                &mut self.plan_cursor,
            );
        }
```

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (everything before, the world-knowledge enemy and sight tests unchanged, and the 10 new tests in `senses::tests`). The longest, `a_lost_hero_is_searched_for_then_the_sentry_goes_home`, takes a second or two in a debug build.
Run: `cargo check -p benilla-app`
Expected: builds.

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: enemies see and hear heroes, turn, check, chase, shout and search

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Each player's marks on the wire, and the ambush and opener seams

**Files:**
- Modify: `crates/nightfall-sim/src/senses/mod.rs`, `crates/nightfall-sim/src/senses/tests.rs`, `crates/nightfall-sim/src/messages.rs`
- Modify (the wire changed): `crates/benilla-app/src/nightfall/session.rs` (`host_send`, `Heard`, `client_exchange`), `crates/benilla-app/src/nightfall/run.rs` (`Sim::marks`, the inbox, `step_sim`)

**Interfaces:**
- Consumes: `World::marks_for`, `Mind::alerted_to` (Tasks 5–6); `ServerMsg::State { snapshot, seen }`, `Heard::State`, `Sim { inbox, seen }` (world knowledge Task 9).
- Produces:
  - `World::ambush(&self, enemy: u32, by: PlayerId) -> f32` (the `ambush` multiplier when the enemy is not alerted to that hero, else 1; 1 for an enemy that is not there); `World::opener_allowed(&self, enemy: u32, by: PlayerId) -> bool`.
  - `ServerMsg::State { snapshot: Snapshot, seen: Vec<u32>, marks: Vec<Mark> }`; `PROTOCOL = 3`.
  - Client: `run::Sim` gains `pub marks: Vec<Mark>`; `session::Heard::State(Snapshot, Vec<u32>, Vec<Mark>)`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/senses/tests.rs`:

```rust
#[test]
fn an_ambush_and_an_opener_need_an_enemy_not_alerted_to_the_attacker() {
    let mut w = world(&sentry(10.0, 10.0, 0.0));
    w.attach_map(open());
    put(&mut w, 0, 10.0, 45.0, Gait::Still, 0);
    put(&mut w, 1, 45.0, 10.0, Gait::Still, 0);
    w.step();
    assert_eq!(w.ambush(1, 0), 1.5);
    assert!(w.opener_allowed(1, 0));
    w.alert(1, 0);
    assert_eq!(w.ambush(1, 0), 1.0);
    assert!(!w.opener_allowed(1, 0));
    // Alerted to one hero, it can still be ambushed by another.
    assert_eq!(w.ambush(1, 1), 1.5);
    assert!(w.opener_allowed(1, 1));
    // An enemy that is not there takes no ambush and no opener.
    assert_eq!(w.ambush(99, 0), 1.0);
    assert!(!w.opener_allowed(99, 0));
}
```

In `crates/nightfall-sim/src/messages.rs`'s tests, change `use crate::{Content, ContentSources, Enemy, World, V2};` to `use crate::{Alert, Content, ContentSources, Enemy, Mark, World, V2};`, add:

```rust
    #[test]
    fn a_state_with_marks_survives_the_wire() {
        let msg = ServerMsg::State {
            snapshot: crate::Snapshot {
                elapsed: 1.5,
                heroes: Vec::new(),
                enemies: Vec::new(),
            },
            seen: vec![3, 4],
            marks: vec![Mark {
                enemy: 3,
                alert: Alert::Alerted,
            }],
        };
        let bytes = postcard::to_stdvec(&msg).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), msg);
    }
```

and in `a_full_partys_state_is_small` replace

```rust
        let msg = ServerMsg::State {
            snapshot,
            seen: (1..=20).collect(),
        };
        let bytes = postcard::to_stdvec(&msg).unwrap().len();
        // Four heroes and the twenty enemies near one of them, 15 times a second, stay near
        // 20 KB a second.
        assert!(bytes < 1400, "{bytes} bytes");
```

with

```rust
        let msg = ServerMsg::State {
            snapshot,
            seen: (1..=20).collect(),
            marks: (1..=20)
                .map(|enemy| Mark {
                    enemy,
                    alert: Alert::Suspicious,
                })
                .collect(),
        };
        let bytes = postcard::to_stdvec(&msg).unwrap().len();
        // Four heroes with their senses and the twenty enemies near one of them, all marked, 15
        // times a second, stay near 22 KB a second.
        assert!(bytes < 1500, "{bytes} bytes");
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`no method named ambush`, `variant ServerMsg::State has no field named marks`).

- [ ] **Step 3: Implement the seams and the message**

Add to `impl World` in `crates/nightfall-sim/src/senses/mod.rs`:

```rust
    /// What the first hit by `by`'s hero on enemy `enemy` is multiplied by (spec §5.4): `ambush`
    /// while the enemy is not alerted to that hero (alerted to another, it is still caught
    /// unawares by this one), else 1. Sub-project 4's damage calls it.
    pub fn ambush(&self, enemy: u32, by: PlayerId) -> f32 {
        match self.mind_of(enemy) {
            Some(m) if !m.alerted_to(by) => self.content().rules.senses.ambush,
            _ => 1.0,
        }
    }

    /// Whether `by`'s hero may open on enemy `enemy` (spec §5.4: Sap, Cheap Shot, Garrote and
    /// Ambush need it not alerted to them). An enemy that is not there takes no opener.
    pub fn opener_allowed(&self, enemy: u32, by: PlayerId) -> bool {
        self.mind_of(enemy).is_some_and(|m| !m.alerted_to(by))
    }

    /// The mind of enemy `enemy`, on the host.
    fn mind_of(&self, enemy: u32) -> Option<&Mind> {
        let i = self.enemies.iter().position(|e| e.id == enemy)?;
        self.senses.minds.get(i)
    }
```

In `crates/nightfall-sim/src/messages.rs`: change `use crate::{PlayerId, PlayerInput, Snapshot};` to `use crate::{Mark, PlayerId, PlayerInput, Snapshot};`, set `pub const PROTOCOL: u32 = 3;`, and replace the `State` variant with:

```rust
    /// The world after a step, as this player gets it: the enemies near their hero, which of
    /// them their hero sees (spec §4.3), and what each thinks of their hero (spec §5.5).
    State {
        snapshot: Snapshot,
        seen: Vec<u32>,
        marks: Vec<Mark>,
    },
```

- [ ] **Step 4: Run the sim's tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (everything before, `an_ambush_and_an_opener_need_an_enemy_not_alerted_to_the_attacker`, `a_state_with_marks_survives_the_wire`).

- [ ] **Step 5: Carry the marks through the client's session and run**

In `crates/benilla-app/src/nightfall/session.rs`:
- Add `Mark` to its `nightfall_sim` import.
- In `host_send`, the `ServerMsg::State { … }` literal becomes:

```rust
            &ServerMsg::State {
                snapshot: world.snapshot_for(p.player),
                seen: world.seen_by(p.player),
                marks: world.marks_for(p.player),
            },
```

- `Heard`'s `State(Snapshot, Vec<u32>),` becomes:

```rust
    /// A state, which enemies in it our hero sees, and what they think of our hero.
    State(Snapshot, Vec<u32>, Vec<Mark>),
```

- In `client_exchange`, the `State` arm becomes `ClientEvent::Message(ServerMsg::State { snapshot, seen, marks }) => heard.push(Heard::State(snapshot, seen, marks)),`.

In `crates/benilla-app/src/nightfall/run.rs`:
- Add `Mark` to its `nightfall_sim` import.
- The `inbox` field becomes:

```rust
    /// Joined: the host's snapshots not yet played, oldest first, each with what our hero sees
    /// in it and what its enemies think of our hero.
    inbox: VecDeque<(Snapshot, Vec<u32>, Vec<Mark>)>,
```

- `Sim` gains, after `seen`:

```rust
    /// What the enemies think of our hero (spec §5.5): worked out here alone or hosting, the
    /// host's word when joined.
    pub marks: Vec<Mark>,
```

and `Sim::hosted` and `Sim::mirror` add `marks: Vec::new(),`.
- `apply_next` becomes:

```rust
    fn apply_next(&mut self) {
        if let Some((snapshot, seen, marks)) = self.inbox.pop_front() {
            self.world.apply_snapshot(snapshot);
            self.seen = seen;
            self.marks = marks;
            self.stepped = true;
        }
    }
```

- In `step_sim`, the client loop's `if let Heard::State(snapshot, seen) = h { sim.inbox.push_back((snapshot, seen)); }` becomes `if let Heard::State(snapshot, seen, marks) = h { sim.inbox.push_back((snapshot, seen, marks)); }`, and in the host loop, after `sim.seen = sim.world.seen_by(sim.me);`, add `sim.marks = sim.world.marks_for(sim.me);`.

- [ ] **Step 6: Run the client's tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo check -p benilla-app`
Expected: builds.

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-sim crates/benilla-app/src/nightfall/session.rs crates/benilla-app/src/nightfall/run.rs
git commit -m "Nightfall: each player's marks on the wire; the ambush and opener seams

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The engine: `Concealed`, and no plate for creeping units

**Files:**
- Create: `crates/benilla-app/src/target/concealed.rs`
- Modify: `crates/benilla-app/src/target/mod.rs` (two lines), `crates/benilla-app/src/target/scan.rs` (the unit query's filter, one test), `crates/benilla-app/src/target/hover.rs` (`TargetHoverState`: one field, one check), `crates/benilla-app/src/nameplates.rs` (`drive_nameplates`: one query term, the gate), `crates/benilla-app/src/vplates.rs` (`PlateWorld::units`: one query term, the gate)

**Interfaces:**
- Consumes: `ObjectStore`, `ObjectFields::{unit_is_ghost_visual, unit_is_stealthed}`; `TargetScan`, `TargetHoverState::is_candidate`; the scan tests' `store`, `F_*` fixtures.
- Produces: `crate::target::Concealed` (a `Component`); `crate::target::plate_hidden(Option<&ObjectStore>, bool) -> bool`.

The reference hides plates and names on `UNIT_FIELD_BYTES_1` byte 3 `& 3` (`0x607101`, `0x60f62e`), ghost or creep; the plates today test only the ghost bit. The fix moves them to the reference.

- [ ] **Step 1: Write the failing tests**

`crates/benilla-app/src/target/concealed.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use benilla_protocol::field::FIELD_UNIT_BYTES_1;
    use benilla_protocol::messages::ObjectFields;

    use super::*;

    fn store(bytes_1: u32) -> ObjectStore {
        ObjectStore(ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, bytes_1)]))
    }

    #[test]
    fn concealed_stealthed_and_ghostly_units_have_no_plate() {
        assert!(!plate_hidden(Some(&store(0)), false));
        assert!(plate_hidden(Some(&store(0)), true));
        assert!(plate_hidden(Some(&store(0x0100_0000)), false));
        assert!(plate_hidden(Some(&store(0x0200_0000)), false));
        // Untrackable hides the minimap dot, not the plate.
        assert!(!plate_hidden(Some(&store(0x0400_0000)), false));
        assert!(plate_hidden(None, true));
        assert!(!plate_hidden(None, false));
    }
}
```

In `crates/benilla-app/src/target/scan.rs`'s tests, after `a_body_the_draw_election_culled_is_still_a_candidate`, add:

```rust
    #[test]
    fn a_concealed_body_is_never_a_candidate() {
        use bevy::ecs::system::RunSystemOnce;

        const SEEN: u64 = 0xBEEF;
        const HIDDEN: u64 = 0xF00D;
        let mut world = World::new();
        world.init_resource::<Reputations>();
        world.init_resource::<NameCache>();
        world.spawn((
            SelfPlayer,
            Transform::default(),
            Guid(1),
            store(&[
                (F_TYPE, TYPE_PLAYER),
                (F_FLAGS, CONTROLLED),
                (F_HEALTH, 100),
                (F_MAXHEALTH, 100),
            ]),
        ));
        for guid in [SEEN, HIDDEN] {
            let e = world
                .spawn((
                    NetEntity {
                        kind: EntityKind::Unit,
                        display_id: None,
                        scale: 1.0,
                    },
                    Guid(guid),
                    Transform::from_xyz(0.0, 0.0, 5.0),
                    store(&[(F_HEALTH, 100), (F_MAXHEALTH, 100)]),
                ))
                .id();
            if guid == HIDDEN {
                world.entity_mut(e).insert(crate::target::Concealed);
            }
        }
        let pool = world
            .run_system_once(|scan: TargetScan| {
                scan.build(ScanSide::Enemy)
                    .iter()
                    .map(|c| c.guid)
                    .collect::<Vec<_>>()
            })
            .expect("the scan runs as a one-shot system");
        assert_eq!(pool, [SEEN]);
    }
```

In `crates/benilla-app/src/target/mod.rs`, add `mod concealed;` beside `mod by_name;` and, beside the other `pub(crate) use` lines:

```rust
// A game mode's hidden units (Nightfall's unseen enemies): no TAB, mouseover or plate.
pub(crate) use concealed::{plate_hidden, Concealed};
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p benilla-app --lib target::`
Expected: FAIL to compile (`cannot find function plate_hidden`, `cannot find struct Concealed`).

- [ ] **Step 3: Implement the marker and the gates**

At the top of `crates/benilla-app/src/target/concealed.rs`:

```rust
//! A unit the player is not to know of though it stands in view, as a game mode's own rules decide
//! (Nightfall hides the enemies its hero does not see). TAB ([`super::scan`]) and the mouseover
//! pick ([`super::hover`]) skip it, and no plate or name floats over it; its body is still drawn.

use bevy::prelude::*;

use crate::net::ObjectStore;

/// Marks a unit kept out of targeting and plates.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct Concealed;

/// Whether a unit has no plate and no name: concealed, or `UNIT_FIELD_BYTES_1` byte 3 `& 3`
/// (ghost or creep), as `0x607101` and `0x60f62e` test it.
pub(crate) fn plate_hidden(store: Option<&ObjectStore>, concealed: bool) -> bool {
    concealed || store.is_some_and(|s| s.0.unit_is_ghost_visual() || s.0.unit_is_stealthed())
}
```

In `crates/benilla-app/src/target/scan.rs`, in `TargetScan`'s `units` query, the filter `Without<SelfPlayer>,` becomes `(Without<SelfPlayer>, Without<super::Concealed>),`, and the struct's doc gains the sentence `A [`super::Concealed`] unit is never a candidate.`

In `crates/benilla-app/src/target/hover.rs`:
- `TargetHoverState` gains, after `units`:

```rust
    /// Units a game mode hides from the player ([`super::Concealed`]): never picked.
    concealed: Query<'w, 's, (), With<super::Concealed>>,
```

- In `is_candidate`, the `EntityKind::Unit | EntityKind::Player` arm becomes:

```rust
            EntityKind::Unit | EntityKind::Player => {
                !self.concealed.contains(entity)
                    && !self.excludes(is_self, store)
                    && self.units.admits_unit(entity, store)
            }
```

and its doc's first sentence gains `, never a concealed one` after `every unit and player outside targeting but the two [`Self::excludes`] drops`.

In `crates/benilla-app/src/nameplates.rs`'s `drive_nameplates`:
- The `units` query's tuple gains, after `Option<&InheritedVisibility>,`:

```rust
        // A game mode's hidden unit has no name ([`crate::target::Concealed`]).
        Has<crate::target::Concealed>,
```

- The loop header becomes `for (entity, net, guid, tf, store, is_self, drawn, concealed) in &units {`.
- The gate

```rust
        } else if store.is_some_and(|s| s.0.unit_is_ghost_visual()) {
            // The ghost vis-flag suppresses the name, target rescue included: `0x607101` and
            // `0x60f62e` both test `bytes_1` byte 3 `& 3`.
            false
```

becomes

```rust
        } else if crate::target::plate_hidden(store, concealed) {
            // The ghost and creep vis-flags suppress the name, target rescue included:
            // `0x607101` and `0x60f62e` both test `bytes_1` byte 3 `& 3`. So does a game mode's
            // concealment.
            false
```

In `crates/benilla-app/src/vplates.rs`:
- `PlateWorld`'s `units` tuple gains, after `Option<&'static ObjectStore>,`: `Has<crate::target::Concealed>,`.
- In `drive_vplates`, the loop header becomes `for (entity, net, guid, tf, store, concealed) in &world.units {`, and

```rust
        // `0x60f62e`: `bytes_1` byte 3 `& 3` (ghost or creep) has no plate.
        if store.is_some_and(|s| s.0.unit_is_ghost_visual()) {
            continue;
        }
```

becomes

```rust
        // `0x60f62e`: `bytes_1` byte 3 `& 3` (ghost or creep) has no plate, nor has a unit a
        // game mode conceals.
        if crate::target::plate_hidden(store, concealed) {
            continue;
        }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p benilla-app --lib target::`
Run: `cargo test -p benilla-app --lib plates`
Expected: PASS (the new `concealed_stealthed_and_ghostly_units_have_no_plate` and `a_concealed_body_is_never_a_candidate`, and every existing target and plate test).
Run: `cargo check -p benilla-app`
Expected: builds. The hover's skip shares `is_candidate`, which both its posed pass and its box fallback call.

- [ ] **Step 5: Check the wasm build** (only where `rustup target list --installed` lists `wasm32-unknown-unknown`)

Run: `cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git add crates/benilla-app/src/target crates/benilla-app/src/nameplates.rs crates/benilla-app/src/vplates.rs
git commit -m "Targeting: a Concealed marker Tab, mouseover and plates skip; creeping units get no plate

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The client: the stance reaches the host, Stealth's look, hidden enemies concealed

**Files:**
- Create: `crates/benilla-app/src/nightfall/stance.rs`, `crates/benilla-app/src/nightfall/conceal.rs`
- Modify: `crates/benilla-app/src/nightfall/run.rs` (`autoplay`, `LocalInput`, `feed_pose`, `step_sim`), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Gait`, `PlayerInput { pose, gait, stealth_presses }`, `Hero::{stealthed, stance, visibility, noise}`, `Enemy::asleep`, `Sim::{seen, marks}` (Tasks 4–7); `crate::target::Concealed` (Task 8); `run::{Sim, LocalInput, feed_pose, step_sim}`, `view::Views`, `enemies::{EnemyViews, place}` (foundation Task 9, world knowledge Task 11); `crate::player::Player::move_flags()`, `crate::creature_anim::move_flags`, `UiScript::has_keyboard_focus`, `crate::net::{ObjectStore, Guid, FieldChanged}`, `benilla_protocol::field::{FIELD_UNIT_AURA, FIELD_UNIT_AURAFLAGS, FIELD_UNIT_BYTES_1}`.
- Produces:
  - `stance::{gait_of(u32) -> Gait, AutoGait, auto_gait() -> AutoGait}`; systems `stance::{stealth_key, trace}`.
  - `conceal::{bytes_1(&ObjectFields, Option<u8>, bool, bool) -> u32, stealth_aura(bool) -> [(u16, u32); 2]}`; systems `conceal::{enemies, heroes}`.
  - `run::LocalInput` gains `pub gait: Gait` and `pub stealth_presses: u8`; `run::autoplay()` becomes `pub(super)`.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/stance.rs`:

```rust
//! The local hero's stance as the host needs it (spec §5.1): how they move, read off WoW's own
//! controller (its walk toggle, `/` on the number pad, and the movement keys held), and the
//! Rogue's Stealth key, counted into [`LocalInput`]. Stealth sits on `1` until sub-project 4 puts
//! it on the action bar. `NIGHTFALL_AUTOPLAY_GAIT` (`walk`, `run` or `sneak`) says how the
//! autoplay walk moves, `sneak` stealthing too; `NIGHTFALL_TRACE_SENSES=1` logs the hero's senses
//! every 2 seconds.

use std::sync::OnceLock;

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use nightfall_sim::{Alert, Gait};

use super::enemies::EnemyViews;
use super::run::{autoplay, LocalInput, Sim};
use crate::creature_anim::move_flags;
use crate::target::Concealed;

/// The key that toggles Stealth until abilities arrive (sub-project 4).
const STEALTH_KEY: KeyCode = KeyCode::Digit1;
/// The movement bits that carry the body across the ground.
const TRANSLATING: u32 = move_flags::FORWARD
    | move_flags::BACKWARD
    | move_flags::STRAFE_LEFT
    | move_flags::STRAFE_RIGHT;
/// Seconds between autoplay's Stealth presses while its rogue is not (yet) in stealth.
const AUTO_PRESS_EVERY: f32 = 2.0;

/// The gait the controller's movement flags say: still with no movement key held (a jump in
/// place included), walking with the walk toggle on, else running.
pub(super) fn gait_of(flags: u32) -> Gait {
    if flags & TRANSLATING == 0 {
        Gait::Still
    } else if flags & move_flags::WALK_MODE != 0 {
        Gait::Walk
    } else {
        Gait::Run
    }
}

/// How the autoplay walk moves, from `NIGHTFALL_AUTOPLAY_GAIT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AutoGait {
    Walk,
    Run,
    /// Walking in stealth: autoplay presses Stealth until the hero is in it.
    Sneak,
}

impl AutoGait {
    /// The setting's value: `walk` (and anything unknown), `run` or `sneak`, any case.
    pub(super) fn parse(value: Option<&str>) -> AutoGait {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("run") => AutoGait::Run,
            Some("sneak") => AutoGait::Sneak,
            _ => AutoGait::Walk,
        }
    }

    /// The gait it reports: the autoplay walk moves the body itself, so no key is held.
    pub(super) fn gait(self) -> Gait {
        match self {
            AutoGait::Run => Gait::Run,
            AutoGait::Walk | AutoGait::Sneak => Gait::Walk,
        }
    }
}

pub(super) fn auto_gait() -> AutoGait {
    static GAIT: OnceLock<AutoGait> = OnceLock::new();
    *GAIT.get_or_init(|| {
        AutoGait::parse(std::env::var("NIGHTFALL_AUTOPLAY_GAIT").ok().as_deref())
    })
}

fn trace_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_TRACE_SENSES").is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controllers_flags_give_the_gait() {
        assert_eq!(gait_of(0), Gait::Still);
        assert_eq!(gait_of(move_flags::WALK_MODE), Gait::Still);
        assert_eq!(gait_of(move_flags::FORWARD), Gait::Run);
        assert_eq!(gait_of(move_flags::BACKWARD), Gait::Run);
        assert_eq!(
            gait_of(move_flags::STRAFE_LEFT | move_flags::WALK_MODE),
            Gait::Walk
        );
        // A jump in place carries nobody across the ground.
        assert_eq!(gait_of(move_flags::FALLING), Gait::Still);
        assert_eq!(gait_of(move_flags::FORWARD | move_flags::FALLING), Gait::Run);
    }

    #[test]
    fn the_autoplay_gait_is_read_from_its_setting() {
        assert_eq!(AutoGait::parse(None), AutoGait::Walk);
        assert_eq!(AutoGait::parse(Some("RUN")), AutoGait::Run);
        assert_eq!(AutoGait::parse(Some("sneak")), AutoGait::Sneak);
        assert_eq!(AutoGait::parse(Some("crawl")), AutoGait::Walk);
        assert_eq!(AutoGait::Sneak.gait(), Gait::Walk);
        assert_eq!(AutoGait::Run.gait(), Gait::Run);
    }
}
```

`crates/benilla-app/src/nightfall/conceal.rs`:

```rust
//! What the hero may know, kept to the interface (spec §4.3, §3.4): an enemy our hero does not see
//! is [`Concealed`] (no Tab, no mouseover, no plate) and untrackable (no minimap dot), though it
//! is still drawn where the fog and real walls allow. A sleeper lies down (stand state 3).
//! Stealthed heroes wear WoW's own stealth: `UNIT_FIELD_BYTES_1`'s creep bit (the stealth walk,
//! no plate, no footstep sounds) and the Stealth aura in their first slot (its translucency, and
//! its icon on the buff frame).

use benilla_protocol::field::{FIELD_UNIT_AURA, FIELD_UNIT_AURAFLAGS, FIELD_UNIT_BYTES_1};
use benilla_protocol::messages::{ObjectFields, ObjectType};
use bevy::prelude::*;

use super::enemies::EnemyViews;
use super::run::Sim;
use super::view::Views;
use crate::net::{FieldChanged, Guid, ObjectStore};
use crate::target::Concealed;

/// Stealth (Rank 3), a level-30 rogue's: its aura's state kit fades the body.
const STEALTH_AURA: u32 = 1786;
/// `UNIT_FIELD_AURAFLAGS`' nibble for a live, cancelable aura, as the survivors' auras write it.
const AURA_LIVE: u32 = 0x3;
/// `UNIT_FIELD_BYTES_1` byte 3's bits: ghost, creep (stealth), untrackable.
const VIS_GHOST: u8 = 0x1;
const VIS_CREEP: u8 = 0x2;
const VIS_UNTRACKABLE: u8 = 0x4;
/// `UNIT_FIELD_BYTES_1` byte 0's stand states: standing, sleeping.
const STAND: u8 = 0;
const STAND_SLEEP: u8 = 3;

/// `UNIT_FIELD_BYTES_1` as `store` holds it, with the creep and untrackable bits as asked and the
/// stand state replaced when given; the ghost bit, the loyalty and the form stay.
pub(super) fn bytes_1(store: &ObjectFields, stand: Option<u8>, creep: bool, untrackable: bool) -> u32 {
    let mut vis = 0;
    if store.unit_is_ghost_visual() {
        vis |= VIS_GHOST;
    }
    if creep {
        vis |= VIS_CREEP;
    }
    if untrackable {
        vis |= VIS_UNTRACKABLE;
    }
    u32::from(stand.unwrap_or(store.unit_stand_state()))
        | u32::from(store.unit_loyalty_level()) << 8
        | u32::from(store.unit_shapeshift_form()) << 16
        | u32::from(vis) << 24
}

/// The aura fields that put Stealth in the first slot, or take it out. A hero carries no other
/// aura yet, so the slot's flag word is ours whole.
pub(super) fn stealth_aura(on: bool) -> [(u16, u32); 2] {
    if on {
        [(FIELD_UNIT_AURA, STEALTH_AURA), (FIELD_UNIT_AURAFLAGS, AURA_LIVE)]
    } else {
        [(FIELD_UNIT_AURA, 0), (FIELD_UNIT_AURAFLAGS, 0)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(bytes_1: u32) -> ObjectFields {
        ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, bytes_1)])
    }

    #[test]
    fn the_creep_and_untrackable_bits_come_and_go_and_the_rest_stays() {
        // Sitting (stand state 1) in shapeshift form 5.
        let f = fields(0x0005_0001);
        assert_eq!(bytes_1(&f, None, true, false), 0x0205_0001);
        assert_eq!(bytes_1(&f, None, false, true), 0x0405_0001);
        assert_eq!(bytes_1(&f, Some(STAND_SLEEP), false, false), 0x0005_0003);
        assert_eq!(bytes_1(&fields(0x0200_0000), None, false, false), 0);
    }

    #[test]
    fn a_ghost_stays_a_ghost() {
        assert_eq!(bytes_1(&fields(0x0100_0000), None, true, true), 0x0700_0000);
    }

    #[test]
    fn stealth_puts_its_aura_in_the_first_slot_and_takes_it_out() {
        assert_eq!(
            stealth_aura(true),
            [(FIELD_UNIT_AURA, 1786), (FIELD_UNIT_AURAFLAGS, 0x3)]
        );
        assert_eq!(
            stealth_aura(false),
            [(FIELD_UNIT_AURA, 0), (FIELD_UNIT_AURAFLAGS, 0)]
        );
    }
}
```

In `nightfall/mod.rs`, add `mod conceal;` and `mod stance;` beside the other modules.

Run: `cargo test -p benilla-app --lib nightfall::stance`
Run: `cargo test -p benilla-app --lib nightfall::conceal`
Expected: PASS (2 and 3 tests). Warnings that `auto_gait`, `trace_on` and `STAND` are unused go away in Step 3.

- [ ] **Step 2: Report the gait and the presses**

In `crates/benilla-app/src/nightfall/run.rs`:
- Add `Gait` to its `nightfall_sim` import.
- `fn autoplay() -> bool {` becomes `pub(super) fn autoplay() -> bool {`.
- Replace `LocalInput` with:

```rust
/// The local hero's input for the next step.
#[derive(Resource, Default)]
pub(super) struct LocalInput {
    pose: Option<HeroPose>,
    /// How the hero moves, read off the controller ([`super::stance::gait_of`]).
    pub gait: Gait,
    /// Stealth presses this session, counted round past 255 ([`PlayerInput::stealth_presses`]).
    pub stealth_presses: u8,
}

impl LocalInput {
    /// The input for the next step, with `pose`.
    fn with_pose(&self, pose: Option<HeroPose>) -> PlayerInput {
        PlayerInput {
            pose,
            gait: self.gait,
            stealth_presses: self.stealth_presses,
        }
    }
}
```

- In `feed_pose`, after the `input.pose = Some(HeroPose { … });` statement, add:

```rust
    // The autoplay walk moves the body itself, holding no key: it says its own gait.
    input.gait = if autoplay() {
        super::stance::auto_gait().gait()
    } else {
        super::stance::gait_of(player.move_flags())
    };
```

- In `step_sim`, `PlayerInput { pose: input.pose, ..Default::default() }` becomes `input.with_pose(input.pose)`, and `PlayerInput { pose: Some(pose), ..Default::default() }` becomes `input.with_pose(Some(pose))`.

- [ ] **Step 3: Write the systems**

Append to `stance.rs`, above its tests:

```rust
/// The Stealth key, counted into the next input (not while an edit box has the keyboard); the
/// autoplay `sneak` presses it every [`AUTO_PRESS_EVERY`] seconds until the hero is in stealth.
pub(super) fn stealth_key(
    keys: Res<ButtonInput<KeyCode>>,
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    real: Res<Time<Real>>,
    mut input: ResMut<LocalInput>,
    mut last_auto: Local<Option<f32>>,
) {
    let typing = script.as_ref().is_some_and(|s| s.has_keyboard_focus());
    if keys.just_pressed(STEALTH_KEY) && !typing {
        input.stealth_presses = input.stealth_presses.wrapping_add(1);
    }
    if autoplay() && auto_gait() == AutoGait::Sneak {
        let now = real.elapsed_secs();
        let out_of_stealth = sim.world.hero(sim.me).is_some_and(|h| !h.stealthed);
        if out_of_stealth && last_auto.is_none_or(|t| now - t >= AUTO_PRESS_EVERY) {
            input.stealth_presses = input.stealth_presses.wrapping_add(1);
            *last_auto = Some(now);
        }
    }
}

/// `NIGHTFALL_TRACE_SENSES=1`: every 2 seconds, our hero's stance, visibility and noise, how many
/// of the drawn enemies are concealed, what the enemies think of us, and how near the nearest
/// enemy alerted to us is.
pub(super) fn trace(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    concealed: Query<(), With<Concealed>>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
) {
    if !trace_on() || real.elapsed_secs() - *last < 2.0 {
        return;
    }
    *last = real.elapsed_secs();
    let Some(h) = sim.world.hero(sim.me) else {
        return;
    };
    let hidden = views
        .drawn
        .values()
        .filter(|&&e| concealed.contains(e))
        .count();
    let marks: Vec<String> = sim
        .marks
        .iter()
        .map(|m| format!("{} {:?}", m.enemy, m.alert))
        .collect();
    let nearest = sim
        .marks
        .iter()
        .filter(|m| m.alert == Alert::Alerted)
        .filter_map(|m| sim.world.enemy(m.enemy))
        .map(|e| e.pos.distance(h.pos))
        .fold(f32::INFINITY, f32::min);
    info!(
        "nightfall: senses: {:?}{}, visibility {:.0}, noise {:.0} yd, {hidden} of {} drawn enemies concealed, marks [{}], nearest alerted {}",
        h.stance,
        if h.stealthed { " (stealth)" } else { "" },
        h.visibility,
        h.noise,
        views.drawn.len(),
        marks.join(", "),
        if nearest.is_finite() { format!("{nearest:.0} yd") } else { "none".into() },
    );
}
```

Append to `conceal.rs`, above its tests:

```rust
/// Every drawn enemy our hero does not see is concealed and untrackable; one they see is neither.
/// A sleeper at its post lies down.
pub(super) fn enemies(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    mut bodies: Query<(&mut ObjectStore, Has<Concealed>)>,
) {
    for enemy in &sim.world.enemies {
        let Some(&e) = views.drawn.get(&enemy.id) else {
            continue;
        };
        let Ok((mut store, concealed)) = bodies.get_mut(e) else {
            continue;
        };
        let hide = !sim.seen.contains(&enemy.id);
        if hide != concealed {
            if hide {
                commands.entity(e).insert(Concealed);
            } else {
                commands.entity(e).remove::<Concealed>();
            }
        }
        let stand = if enemy.asleep { STAND_SLEEP } else { STAND };
        let now = bytes_1(
            &store.0,
            None,
            store.0.unit_is_stealthed(),
            store.0.unit_is_untrackable(),
        );
        let want = bytes_1(&store.0, Some(stand), false, hide);
        if now != want {
            store
                .0
                .merge(ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, want)]));
        }
    }
}

/// Every hero's body wears their stealth as the host has it: the creep bit and the Stealth aura,
/// ringing the aura watch as a server's update would, so the aura's kit (the translucency) comes
/// and goes.
pub(super) fn heroes(
    sim: Res<Sim>,
    views: Res<Views>,
    mut bodies: Query<(&mut ObjectStore, &Guid)>,
    mut fields: MessageWriter<FieldChanged>,
) {
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let Ok((mut store, guid)) = bodies.get_mut(e) else {
            continue;
        };
        if store.0.unit_is_stealthed() == hero.stealthed {
            continue;
        }
        let untrackable = store.0.unit_is_untrackable();
        let mut pairs = vec![(
            FIELD_UNIT_BYTES_1,
            bytes_1(&store.0, None, hero.stealthed, untrackable),
        )];
        pairs.extend(stealth_aura(hero.stealthed));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        fields.write(FieldChanged {
            entity: e,
            guid: guid.0,
            kind: ObjectType::Player,
            index: FIELD_UNIT_AURA,
            old: 0,
            new: 1,
        });
    }
}
```

- [ ] **Step 4: Register them**

In `NightfallPlugin::build`, add:

```rust
            .add_systems(
                Update,
                stance::stealth_key
                    .before(run::feed_pose)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (conceal::enemies, conceal::heroes, stance::trace)
                    .chain()
                    .after(enemies::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 5: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 6: See a walker spotted and a sneaker pass**

Walking (the autoplay circles 15 yards around the entry, past the sentry and the patrol placed near it):

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_GAIT=walk NIGHTFALL_TRACE_SENSES=1 WOW_LIVE_SHOT=target/senses-walk.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=6 ./target/release/azeroth-together.exe > target/senses-walk.log 2>&1 &
```

Expected, once `target/senses-walk-003.png` exists (then `kill` the PID):
- `target/senses-walk.log` has `nightfall: senses: Walking, visibility 55, noise 4 yd, …` lines; within the run's first minute some mark goes from `Suspicious` to `Alerted`, and `nearest alerted` falls to a few yards.
- The concealed count is not always 0 nor always every drawn enemy: enemies behind walls and trees, or past 40 yards, are concealed.
- The shots show an alerted enemy running at the hero and standing at it, facing it.

Sneaking, as the Rogue (the first class):

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_GAIT=sneak NIGHTFALL_TRACE_SENSES=1 WOW_LIVE_SHOT=target/senses-sneak.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=6 ./target/release/azeroth-together.exe > target/senses-sneak.log 2>&1 &
```

Expected: the log's lines read `Sneaking (stealth), visibility 20, noise 0 yd`; marks are fewer and later than walking, and an enemy behind the hero's path stays unmarked. The shots show the rogue faded (the Stealth aura's translucency), in the stealth walk, with the Stealth icon on the buff frame, and the sleeper near the entry lying down. If the rogue is not faded, the aura kit did not arm: note it in the hand-off (the creep bit's stealth walk and the missing plate still show stealth). Stop the run.

- [ ] **Step 7: Commit**

```bash
git add crates/benilla-app/src/nightfall
git commit -m "Nightfall: the hero's gait and Stealth reach the host; stealth's look; unseen enemies concealed

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: "?" and "!" over the enemies

**Files:**
- Create: `crates/benilla-app/src/nightfall/marks.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Sim::marks`, `Mark`, `Alert` (Task 7); `Concealed` (Task 8); `EnemyViews::drawn` (world knowledge Task 11); `NightfallEntity`, `content()`; `crate::char_select::wow_font`; `benilla_world::view::WorldCamera`.
- Produces: `marks::MarkViews` (a `Resource`), `marks::MarkText` (a `Component`); pure `mark_face(Alert) -> (&'static str, Color)`, `mark_height(f32) -> f32`; system `marks::draw`.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/marks.rs`:

```rust
//! "?" and "!" over the enemies suspicious of or alerted to our hero (spec §5.5): a glyph of WoW's
//! own font floating over the head, from this player's marks ([`Sim::marks`]). Hidden while the
//! enemy is [`Concealed`]: what our hero does not see, the interface does not show. A text node
//! per marked enemy rather than combat text, which rises, fades and stays where it was spawned.

use std::collections::{HashMap, HashSet};

use benilla_world::view::WorldCamera;
use bevy::prelude::*;
use nightfall_sim::Alert;

use super::enemies::EnemyViews;
use super::run::Sim;
use super::{content, NightfallEntity};
use crate::target::Concealed;

/// The glyph's size, UI pixels.
const SIZE: f32 = 30.0;
/// Half the glyph's width and its height, UI pixels, to set it centred over the head.
const HALF_W: f32 = 8.0;
const HIGH: f32 = 34.0;

/// A mark's text node.
#[derive(Component)]
pub(super) struct MarkText;

/// Which text node shows which enemy's mark.
#[derive(Resource, Default)]
pub(super) struct MarkViews {
    nodes: HashMap<u32, Entity>,
}

/// The glyph and colour of a mark: a gold "?", a red "!".
pub(super) fn mark_face(alert: Alert) -> (&'static str, Color) {
    match alert {
        Alert::Suspicious => ("?", Color::srgb(1.0, 0.82, 0.0)),
        Alert::Alerted => ("!", Color::srgb(1.0, 0.15, 0.1)),
    }
}

/// How high over an enemy's feet its mark floats, yards: over the head of a model at `scale`.
pub(super) fn mark_height(scale: f32) -> f32 {
    2.7 * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_for_suspicion_and_a_bang_for_an_alert() {
        assert_eq!(mark_face(Alert::Suspicious).0, "?");
        assert_eq!(mark_face(Alert::Alerted).0, "!");
        assert_ne!(mark_face(Alert::Suspicious).1, mark_face(Alert::Alerted).1);
    }

    #[test]
    fn the_mark_floats_over_a_bigger_model_higher() {
        assert_eq!(mark_height(1.0), 2.7);
        assert!(mark_height(1.3) > mark_height(1.0));
    }
}
```

Add `mod marks;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::marks`
Expected: PASS (2 tests).

- [ ] **Step 2: Write the system**

Append to `marks.rs`, above its tests:

```rust
/// Every mark over its enemy's head, where the camera sees it; a mark whose enemy is concealed,
/// off screen or no longer marked is hidden. Nodes are made as marks first appear and go with the
/// run's other entities.
#[allow(clippy::type_complexity)]
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    enemy_views: Res<EnemyViews>,
    assets: Res<AssetServer>,
    ui_scale: Res<UiScale>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    bodies: Query<(&Transform, Has<Concealed>)>,
    mut views: ResMut<MarkViews>,
    mut texts: Query<(&mut Node, &mut Text, &mut TextColor, &mut Visibility), With<MarkText>>,
) {
    let content = content();
    let s = ui_scale.0;
    let cam = cams.single().ok();
    // A node that went with the last run is made again when its enemy is next marked.
    views.nodes.retain(|_, e| texts.contains(*e));
    let mut shown = HashSet::new();
    for m in &sim.marks {
        let Some(enemy) = sim.world.enemy(m.enemy) else {
            continue;
        };
        let Some(&body) = enemy_views.drawn.get(&m.enemy) else {
            continue;
        };
        let Ok((tf, concealed)) = bodies.get(body) else {
            continue;
        };
        let Some((camera, cam_tf)) = cam else {
            continue;
        };
        if concealed {
            continue;
        }
        let head = tf.translation + Vec3::Y * mark_height(content.enemies[enemy.kind].scale);
        let Ok(at) = camera.world_to_viewport(&GlobalTransform::from(*cam_tf), head) else {
            continue;
        };
        let node = *views.nodes.entry(m.enemy).or_insert_with(|| {
            commands
                .spawn((
                    MarkText,
                    NightfallEntity,
                    Text::new(""),
                    TextFont {
                        font: crate::char_select::wow_font(&assets),
                        font_size: SIZE,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    GlobalZIndex(340),
                    Visibility::Hidden,
                ))
                .id()
        });
        // A node spawned this frame is placed from the next.
        let Ok((mut n, mut text, mut color, mut vis)) = texts.get_mut(node) else {
            continue;
        };
        shown.insert(m.enemy);
        let (glyph, tint) = mark_face(m.alert);
        n.left = Val::Px(at.x / s - HALF_W);
        n.top = Val::Px(at.y / s - HIGH);
        if text.0 != glyph {
            text.0 = glyph.into();
        }
        color.0 = tint;
        *vis = Visibility::Inherited;
    }
    for (id, &node) in &views.nodes {
        if !shown.contains(id) {
            if let Ok((_, _, _, mut vis)) = texts.get_mut(node) {
                *vis = Visibility::Hidden;
            }
        }
    }
}
```

- [ ] **Step 3: Register it**

In `NightfallPlugin::build`, add `.init_resource::<marks::MarkViews>()` beside the other resources, and `marks::draw` after `stance::trace` in Task 9's `(conceal::enemies, conceal::heroes, stance::trace)` chain, which becomes `(conceal::enemies, conceal::heroes, stance::trace, marks::draw)`.

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 5: See the marks**

Run Task 9 Step 6's walking command again with `WOW_LIVE_SHOT=target/marks-walk.png WOW_LIVE_SHOT_COUNT=8 WOW_LIVE_SHOT_EVERY=3` in place of its shot settings, logging to `target/marks-walk.log`.
Expected: shots taken while the log reads `Suspicious` for an enemy show a gold "?" over its head; while it reads `Alerted`, a red "!"; an enemy behind a building (concealed) shows neither though its mark is in the log. Stop the run.

- [ ] **Step 6: Commit**

```bash
git add crates/benilla-app/src/nightfall
git commit -m "Nightfall: \"?\" and \"!\" over the enemies suspicious of or alerted to you

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The eye, the noise rings, the heartbeat and the vignette

**Files:**
- Create: `crates/benilla-app/src/nightfall/senses_hud.rs`
- Modify: `crates/benilla-app/src/nightfall/grade.rs`, `crates/benilla-app/src/shaders/nightfall_grade.wgsl`, `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Hero::{visibility, noise}`, `Sim::marks`, `SenseRules::{heartbeat_within, vignette_seen, vignette_hidden}` (Tasks 1, 4, 7); `NightGrade`, `grade::plugin` (foundation Task 11); `UiScript::run`; `crate::char_select::wow_font`.
- Produces:
  - `senses_hud::{EyeRoot, EyeWhite, EyePupil, EyeRing, EyeNumber}` (components); pure `eye_center(f32) -> Vec2`, `eye_look(f32) -> (f32, [f32; 3])`, `ring_count(f32) -> usize`, `beat_interval(f32, f32) -> Option<f32>`; systems `senses_hud::{spawn_eye, drive_eye, heartbeat}`.
  - `NightGrade`'s `unused` field becomes `vignette_from` (the vignette's inner edge, default 0.35); pure `grade::{vignette_for(f32, (f32, f32), (f32, f32)) -> (f32, f32), ease(f32, f32, f32, f32) -> f32}`; system `grade::follow_visibility`.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/senses_hud.rs`:

```rust
//! The senses on screen (spec §5.5, §10): an eye under the player frame that opens and brightens
//! with the hero's visibility (its number beneath, for tuning), up to three rings around it for
//! the loudest noise they made in the last second, and a heartbeat while an enemy alerted to them
//! is near. The vignette is the night grade's ([`super::grade::follow_visibility`]).

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use nightfall_sim::Alert;

use super::run::Sim;
use super::{content, NightfallEntity};

/// The eye's box, UI pixels: the rings fit inside it.
const BOX: f32 = 84.0;
/// The eye at its widest, and its pupil, UI pixels.
const EYE_W: f32 = 34.0;
const PUPIL: f32 = 11.0;
/// The rings' diameters, UI pixels, innermost first.
const RINGS: [f32; 3] = [48.0, 64.0, 80.0];
/// Where the eye's centre sits in WoW's 768-high interface units: under the player frame's
/// portrait (the frame is anchored at the top left).
const EYE_AT: Vec2 = Vec2::new(60.0, 112.0);
/// The heartbeat's thump: the trimmed data has no heartbeat, so a small body falling on dirt
/// stands in for one.
const THUMP: &str = r"Sound\Effects\DeathImpacts\mDeathImpactSmallDirtA.wav";
/// The second beat of each heartbeat follows the first by this much, seconds.
const DUB_AFTER: f32 = 0.16;

#[derive(Component)]
pub(super) struct EyeRoot;
#[derive(Component)]
pub(super) struct EyeWhite;
#[derive(Component)]
pub(super) struct EyePupil;
/// One ring, innermost 0.
#[derive(Component)]
pub(super) struct EyeRing(usize);
#[derive(Component)]
pub(super) struct EyeNumber;

/// The eye's centre on a window `height` logical pixels high, before the UI scale.
pub(super) fn eye_center(height: f32) -> Vec2 {
    EYE_AT * (height / 768.0)
}

/// How open the eye is (a share of its width) and its colour at `visibility` 0..100: nearly shut
/// and slate-dark when hidden, wide and lamplit when seen.
pub(super) fn eye_look(visibility: f32) -> (f32, [f32; 3]) {
    let t = (visibility / 100.0).clamp(0.0, 1.0);
    let dim = [0.32, 0.38, 0.48];
    let lit = [1.0, 0.86, 0.45];
    let mix = |i: usize| dim[i] + (lit[i] - dim[i]) * t;
    (0.12 + 0.88 * t, [mix(0), mix(1), mix(2)])
}

/// How many rings show for a noise of `radius` yards: none for silence, one for a footstep's
/// whisper, two for a run, three for anything louder.
pub(super) fn ring_count(radius: f32) -> usize {
    if radius <= 0.0 {
        0
    } else if radius < 10.0 {
        1
    } else if radius < 20.0 {
        2
    } else {
        3
    }
}

/// Seconds between heartbeats with an enemy alerted to us `distance` yards off: from 0.9 at
/// `within` down to 0.45 on top of us; none past `within`.
pub(super) fn beat_interval(distance: f32, within: f32) -> Option<f32> {
    (distance <= within)
        .then(|| 0.45 + 0.45 * (distance / within.max(1e-3)).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eye_sits_under_the_player_frame_at_any_height() {
        assert_eq!(eye_center(768.0), Vec2::new(60.0, 112.0));
        assert_eq!(eye_center(720.0), Vec2::new(56.25, 105.0));
    }

    #[test]
    fn the_eye_opens_and_brightens_with_visibility() {
        let (shut, dark) = eye_look(0.0);
        let (wide, lit) = eye_look(100.0);
        assert!((shut - 0.12).abs() < 1e-6);
        assert!((wide - 1.0).abs() < 1e-6);
        assert!(lit[0] > dark[0]);
        assert_eq!(eye_look(250.0), eye_look(100.0));
    }

    #[test]
    fn louder_noise_more_rings() {
        assert_eq!(ring_count(0.0), 0);
        assert_eq!(ring_count(4.0), 1);
        assert_eq!(ring_count(12.0), 2);
        assert_eq!(ring_count(40.0), 3);
    }

    #[test]
    fn the_heart_beats_faster_the_nearer_the_hunter() {
        assert_eq!(beat_interval(25.0, 20.0), None);
        assert!((beat_interval(20.0, 20.0).unwrap() - 0.9).abs() < 1e-6);
        assert_eq!(beat_interval(0.0, 20.0), Some(0.45));
        assert!(beat_interval(5.0, 20.0) < beat_interval(15.0, 20.0));
    }
}
```

In `crates/benilla-app/src/nightfall/grade.rs`, add above `pub(super) fn plugin`:

```rust
/// How fast the vignette follows the hero's visibility: this share of the gap a second.
const VIGNETTE_EASE: f32 = 3.0;

/// The vignette's (strength, inner edge) for a hero at `visibility` 0..100: `seen` at 100,
/// `hidden` at 0, straight between (spec §10: it tightens as the hero hides).
pub(super) fn vignette_for(visibility: f32, seen: (f32, f32), hidden: (f32, f32)) -> (f32, f32) {
    let t = (visibility / 100.0).clamp(0.0, 1.0);
    (
        hidden.0 + (seen.0 - hidden.0) * t,
        hidden.1 + (seen.1 - hidden.1) * t,
    )
}

/// `from` moved toward `to` by `rate` of the gap a second over `dt` seconds, never past it.
pub(super) fn ease(from: f32, to: f32, rate: f32, dt: f32) -> f32 {
    from + (to - from) * (rate * dt).min(1.0)
}
```

and at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vignette_tightens_as_the_hero_hides() {
        let seen = (0.45, 0.35);
        let hidden = (0.85, 0.1);
        assert_eq!(vignette_for(100.0, seen, hidden), seen);
        assert_eq!(vignette_for(0.0, seen, hidden), hidden);
        let (s, from) = vignette_for(50.0, seen, hidden);
        assert!((s - 0.65).abs() < 1e-6 && (from - 0.225).abs() < 1e-6);
        assert_eq!(vignette_for(150.0, seen, hidden), seen);
    }

    #[test]
    fn easing_closes_the_gap_without_passing_it() {
        assert!((ease(0.0, 1.0, 3.0, 0.1) - 0.3).abs() < 1e-6);
        assert_eq!(ease(0.0, 1.0, 3.0, 1.0), 1.0);
        assert_eq!(ease(0.5, 0.5, 3.0, 0.1), 0.5);
    }
}
```

Add `mod senses_hud;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::senses_hud`
Run: `cargo test -p benilla-app --lib nightfall::grade`
Expected: PASS (4 and 2 tests).

- [ ] **Step 2: Let the vignette's inner edge move**

In `crates/benilla-app/src/shaders/nightfall_grade.wgsl`, the `Params` comment `// xyz tint multiplier, w unused.` becomes `// xyz tint multiplier, w the vignette's inner edge: 0 the centre, 1 the corners.`, and `c = c * (1.0 - params.a.z * smoothstep(0.35, 1.0, d));` becomes `c = c * (1.0 - params.a.z * smoothstep(params.b.w, 1.0, d));`.

In `grade.rs`:
- `NightGrade`'s `pub unused: f32,` becomes:

```rust
    /// Where the vignette starts darkening, from the centre (0) to the corners (1).
    pub vignette_from: f32,
```

and its `Default` has `vignette_from: 0.35,` in place of `unused: 0.0,`.
- Add:

```rust
/// The vignette follows the local hero's visibility, eased so a change of stance never pops; at
/// the menu it stands as for a hero in plain sight.
fn follow_visibility(
    sim: Option<Res<super::run::Sim>>,
    real: Res<Time<Real>>,
    mut grades: Query<&mut NightGrade>,
) {
    let r = &super::content().rules.senses;
    let visibility = sim
        .as_ref()
        .and_then(|s| s.world.hero(s.me))
        .map_or(100.0, |h| h.visibility);
    let (strength, from) = vignette_for(visibility, r.vignette_seen, r.vignette_hidden);
    let dt = real.delta_secs();
    for mut g in &mut grades {
        g.vignette = ease(g.vignette, strength, VIGNETTE_EASE, dt);
        // An inner edge at the corners would leave the shader's smoothstep with no width.
        g.vignette_from = ease(g.vignette_from, from, VIGNETTE_EASE, dt).min(0.95);
    }
}
```

- In `plugin`, `.add_systems(Update, attach.run_if(in_state(GameMode::Nightfall)))` becomes `.add_systems(Update, (attach, follow_visibility).run_if(in_state(GameMode::Nightfall)))`, and the module doc gains the sentence `Its vignette tightens as the local hero's visibility drops (spec §10).`

- [ ] **Step 3: Write the eye and the heartbeat**

Append to `senses_hud.rs`, above its tests:

```rust
/// The eye and its rings, once per run (they go with the run's other entities).
pub(super) fn spawn_eye(
    mut commands: Commands,
    assets: Res<AssetServer>,
    eyes: Query<(), With<EyeRoot>>,
) {
    if !eyes.is_empty() {
        return;
    }
    let font = crate::char_select::wow_font(&assets);
    commands
        .spawn((
            EyeRoot,
            NightfallEntity,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(BOX),
                height: Val::Px(BOX),
                ..default()
            },
            GlobalZIndex(350),
        ))
        .with_children(|root| {
            for (i, d) in RINGS.into_iter().enumerate() {
                root.spawn((
                    EyeRing(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px((BOX - d) / 2.0),
                        top: Val::Px((BOX - d) / 2.0),
                        width: Val::Px(d),
                        height: Val::Px(d),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.85, 0.85, 0.8, 0.45)),
                    Visibility::Hidden,
                ));
            }
            root.spawn((
                EyeWhite,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((BOX - EYE_W) / 2.0),
                    width: Val::Px(EYE_W),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.85)),
                BorderColor::all(Color::WHITE),
            ))
            .with_children(|eye| {
                eye.spawn((
                    EyePupil,
                    Node {
                        width: Val::Px(PUPIL),
                        height: Val::Px(PUPIL),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                ));
            });
            root.spawn((
                EyeNumber,
                Text::new(""),
                TextFont {
                    font,
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(BOX / 2.0 - 7.0),
                    top: Val::Px(BOX - 12.0),
                    ..default()
                },
            ));
        });
}

/// The eye follows the hero: under the player frame, open and bright by visibility, ringed by
/// noise, its number beneath.
#[allow(clippy::type_complexity)]
pub(super) fn drive_eye(
    sim: Res<Sim>,
    ui_scale: Res<UiScale>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<EyeRoot>>,
        Query<(&mut Node, &mut BorderColor), With<EyeWhite>>,
        Query<&mut BackgroundColor, With<EyePupil>>,
        Query<(&EyeRing, &mut Visibility)>,
        Query<&mut Text, With<EyeNumber>>,
    )>,
) {
    let (Some(hero), Ok(window)) = (sim.world.hero(sim.me), windows.single()) else {
        return;
    };
    let s = ui_scale.0;
    let c = eye_center(window.height());
    if let Ok(mut root) = nodes.p0().single_mut() {
        root.left = Val::Px(c.x / s - BOX / 2.0);
        root.top = Val::Px(c.y / s - BOX / 2.0);
    }
    let (open, rgb) = eye_look(hero.visibility);
    let color = Color::srgb(rgb[0], rgb[1], rgb[2]);
    if let Ok((mut white, mut border)) = nodes.p1().single_mut() {
        let h = (EYE_W * open).max(3.0);
        white.height = Val::Px(h);
        white.top = Val::Px((BOX - h) / 2.0);
        *border = BorderColor::all(color);
    }
    if let Ok(mut pupil) = nodes.p2().single_mut() {
        *pupil = BackgroundColor(color.with_alpha(0.3 + 0.7 * open));
    }
    let rings = ring_count(hero.noise);
    let mut ring_q = nodes.p3();
    for (ring, mut vis) in &mut ring_q {
        let want = if ring.0 < rings {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    if let Ok(mut text) = nodes.p4().single_mut() {
        let number = format!("{:.0}", hero.visibility);
        if text.0 != number {
            text.0 = number;
        }
    }
}

/// When the next heartbeat is due, and its second beat.
#[derive(Default)]
pub(super) struct Beat {
    next: f32,
    dub: Option<f32>,
}

/// The heartbeat (spec §10): a lub-dub through WoW's own `PlaySoundFile`, faster the nearer the
/// nearest enemy alerted to our hero, while one is within `heartbeat_within` yards.
pub(super) fn heartbeat(
    sim: Res<Sim>,
    script: Option<NonSend<UiScript>>,
    real: Res<Time<Real>>,
    mut beat: Local<Beat>,
) {
    let within = content().rules.senses.heartbeat_within;
    let now = real.elapsed_secs();
    let nearest = sim.world.hero(sim.me).and_then(|h| {
        sim.marks
            .iter()
            .filter(|m| m.alert == Alert::Alerted)
            .filter_map(|m| sim.world.enemy(m.enemy))
            .map(|e| e.pos.distance(h.pos))
            .min_by(f32::total_cmp)
    });
    let Some(interval) = nearest.and_then(|d| beat_interval(d, within)) else {
        // The first beat comes at once when a hunter closes in.
        beat.next = now;
        beat.dub = None;
        return;
    };
    let Some(script) = script else {
        return;
    };
    let thump = || {
        let _ = script.run(&format!("PlaySoundFile([[{THUMP}]])"));
    };
    if beat.dub.is_some_and(|t| now >= t) {
        beat.dub = None;
        thump();
    }
    if now >= beat.next {
        thump();
        beat.dub = Some(now + DUB_AFTER);
        beat.next = now + interval;
    }
}
```

- [ ] **Step 4: Register them**

In `NightfallPlugin::build`, the chain `(conceal::enemies, conceal::heroes, stance::trace, marks::draw)` becomes `(conceal::enemies, conceal::heroes, stance::trace, marks::draw, senses_hud::spawn_eye, senses_hud::drive_eye, senses_hud::heartbeat)`.

- [ ] **Step 5: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo test -p benilla-app --lib shaders`
Expected: PASS (the edited shader still answers at its embedded path).
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 6: See the eye, the rings and the vignette**

Run Task 9 Step 6's walking command with `NIGHTFALL_AUTOPLAY_GAIT=run WOW_LIVE_SHOT=target/eye-run.png WOW_LIVE_SHOT_COUNT=3 WOW_LIVE_SHOT_EVERY=5`, then its sneaking command with `WOW_LIVE_SHOT=target/eye-sneak.png WOW_LIVE_SHOT_COUNT=3 WOW_LIVE_SHOT_EVERY=5`, stopping each after its last shot.
Expected:
- `eye-run-*.png`: under the player frame a wide, amber eye reading `100`, with two rings; the screen's corners lightly shaded.
- `eye-sneak-*.png`: the eye nearly shut and slate-dark reading `20`, no rings; the corners clearly darker and the dark reaching further in.
- In the running log, while `nearest alerted` reads under 20 yd the heartbeat plays; it cannot be heard in a background run, so list "hear the heartbeat when an enemy chases you" in the hand-off for the user to try.

- [ ] **Step 7: Commit**

```bash
git add crates/benilla-app/src/nightfall crates/benilla-app/src/shaders/nightfall_grade.wgsl
git commit -m "Nightfall: the eye and noise rings, a heartbeat near a hunter, the vignette by visibility

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Docs, the full check and co-op

**Files:**
- Modify: `CLAUDE.md`, `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line)

- [ ] **Step 1: Document the senses in `CLAUDE.md`**

Under "Where things live", after the Nightfall world-knowledge entry sub-project 2 added, add:

```markdown
- Nightfall's senses (sub-project 3): `nightfall-sim/src/senses/` — `visibility.rs` (stance from
  the client's gait and Stealth; light; the facing cone; notice range), `noise.rs` (footsteps by
  stance, `Loudness`, hearing at half through walls), `awareness.rs` (each enemy's suspicion of
  each hero: wary "?", checking, alerted "!", searching; sleepers), `react.rs` (enemies turning,
  checking, chasing to `chase_reach`, shouting to allies within reach, searching, going home).
  Every number is in `content/nightfall/rules.ron` under `senses:`. Seams for combat:
  `World::{alert, ambush, opener_allowed, make_noise, flash, break_stealth}`. The host sends each
  player their marks (`World::marks_for`) beside their seen set. Client: `nightfall/stance.rs`
  (gait from the controller's walk toggle and keys, Stealth on `1`), `conceal.rs` (unseen enemies
  get the engine's `target::Concealed`: no Tab, mouseover or plate; stealth's creep bit and aura),
  `marks.rs` ("?"/"!"), `senses_hud.rs` (the eye, rings, heartbeat), `grade.rs` (the vignette).
```

Under "Build, test, run", add:

```markdown
- Nightfall's senses: `NIGHTFALL_AUTOPLAY_GAIT=walk|run|sneak` sets how the autoplay walk moves
  (`sneak` stealths the Rogue); `NIGHTFALL_TRACE_SENSES=1` logs the hero's stance, visibility,
  noise, concealed enemies, marks and the nearest alerted enemy every 2 s.
```

- [ ] **Step 2: Mark the spec**

In the spec's status line, after the sentence sub-project 2 added (`Sub-project 2 built …`), add: `Sub-project 3 built (plan docs/superpowers/plans/2026-10-17-nightfall-senses.md).`

- [ ] **Step 3: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS (the Darkshire bake test skips without `WOW_DATA`).
Run: `cargo test -p benilla-app --lib nightfall`
Run: `cargo test -p benilla-app --lib target`
Run: `cargo test -p benilla-app --lib plates`
Run: `cargo test -p benilla-app --lib game_pick`
Expected: PASS.
Run (only where `rustup target list --installed` lists `wasm32-unknown-unknown`): `cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu`
Expected: builds.

- [ ] **Step 4: Senses in co-op on one PC**

Start the host in the background, and a few seconds later the client:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_GAIT=sneak NIGHTFALL_NAME=Host NIGHTFALL_TRACE_SENSES=1 WOW_LIVE_SHOT=target/host-senses.png WOW_LIVE_SHOT_AT=50 ./target/release/azeroth-together.exe > target/host-senses.log 2>&1 &
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_GAIT=run NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend NIGHTFALL_TRACE_SENSES=1 WOW_LIVE_SHOT=target/client-senses.png WOW_LIVE_SHOT_AT=45 ./target/release/azeroth-together.exe > target/client-senses.log 2>&1 &
```

Expected: the host's log reads `Sneaking (stealth)` with visibility 20; the client's reads `Running` with visibility 100 and noise 12 yd, and its marks come from the host (an enemy alerted to the running mage shows `Alerted` in the client's log). The host's shot shows its own rogue faded; the client's shot shows the host's rogue faded with no plate over it (stealth hides it). Stop both.

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md
git commit -m "Docs: Nightfall's senses, their settings and test hooks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
