# Extraction Mode, Part 2 (The Living Map): Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Elwynn filled with its real creatures, idle until they notice you, with roads and towns clear; flights that leave on a timetable, some of them ambushed; and hot zones where heat climbs faster, so an extraction run is about the route you take.

**Architecture:** `together-sim` gains a wildlife layer next to the heat-driven horde. A map's creatures come from a baked `content/spawns.ron` (from the CMaNGOS Classic DB). Each one sleeps until a hero comes within waking range, stands or patrols until it notices a hero, then chases like the horde, and gives up when left alone. Wildlife state is host-only; the creatures themselves travel in snapshots as ordinary enemies. Exits switch from called flights to a timetable. Heroes' spells only seek **engaged** enemies. The arena never has wildlife, and all its enemies are always engaged, so it plays as before.

**Tech Stack:** Rust (GNU toolchain on Windows), `serde` + `ron`, `postcard`, Bevy 0.18 client, Python 3 for the bake.

**Spec:** `docs/superpowers/specs/2026-10-15-extraction-living-map-design.md`. This plan covers the spec's build steps 1–5. The Stalker (step 6) and the tuning pass with new dev-panel rows (step 7) get their own plan after playtesting.

## Global Constraints

- Work only in the worktree `D:\Games\azeroth-together-extraction` on branch `extraction-mode`. Never switch branches in `D:\Games\azeroth-together` (another session works there).
- Shell setup for every command: `source "C:/Users/Olivs/AppData/Local/Temp/claude/D--Games-azeroth-together/ced5222e-ac81-40a6-a2c3-afa972f4cede/scratchpad/env.sh"`. It puts MinGW on `PATH`, makes git trust the exFAT worktree, sets `$SK` and `$PLAN`, and moves into the worktree. If that file is gone, recreate it: export the MinGW `PATH` from `CLAUDE.md`, export `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0=D:/Games/azeroth-together-extraction`, and `cd /d/Games/azeroth-together-extraction`.
- Fast tests: `cargo test -p together-sim -p together-net`. Client: `cargo build --release -p azeroth-together` (6–10 minutes; run it in the background).
- Runs from the worktree set `WOW_DATA=D:/Games/azeroth-together/WoW/Data` and `BENILLA_HOME` to a fresh folder under `shots/`, so `TOGETHER_ZONE` is not overridden by a saved menu pick. Test runs always set `WOW_BG=1 WOW_WIN=1280x720`; never launch a normal foreground run.
- **The arena must not change.** Every existing arena test passes untouched. Every arena enemy is engaged from birth, so the targeting rule filters nothing there.
- Every tuning number lives in content (`extraction.ron`, `enemies.ron`, `spawns.ron`). Rust consts only for drawing and geometry tolerances.
- `spawns.ron` is derived from the CMaNGOS Classic DB (GPL-3.0). It carries that note in its header, and `tools/spawns/README.md` says where it came from. Never ship Blizzard data.
- Touch no engine crate outside `crates/benilla-app/src/together/`.
- Match the surrounding style: plain-sentence doc comments, small functions.
- End every commit message with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Co-op guests:** wildlife state is host-only and `Enemy::engaged` is not in snapshots, so a guest sees creatures as ordinary enemies and is told of engagement by events. Expected: a guest's world draws the same creatures, the "!" shows on both screens, and nothing crashes when a creature sleeps (it vanishes the way a recycled enemy does). Test in Task 8: `a_mirror_draws_the_awake_forest`.
2. **A sleeping creature that is engaged never sleeps; one that gives up and walks home can.** Expected: a chase is never cut short by distance, and a creature that went home reappears at its spawn, healthy. Tests in Task 6: `an_engaged_creature_never_sleeps` and `a_creature_left_behind_gives_up_walks_home_and_heals`.
3. **Crowding:** a hot zone at 5 skulls with four heroes (horde plus wildlife). Expected: awake wildlife keeps to `max_awake` per hero, and the snapshot stays about 15–20 bytes per enemy. Test in Task 8: `snapshot_size.rs` extraction case extended.
4. **The arena:** expected unchanged. The existing arena and shipped tests pass, and Task 5 adds `an_arena_enemy_is_always_engaged`.
5. **The bake's mine filter:** expected that no creature stands on the hill above Fargodeep or Jasperlode. Checked in Task 4: the bake prints the dropped count, plus a screenshot in Task 8.

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/together-sim/src/extraction.rs` | modify | Timetable `ExitDef`/`ExitState`, `AmbushDef`, `WildlifeDef`, `HotZoneDef`, `SpawnDef`; outline doc. |
| `crates/together-sim/src/content.rs` | modify | `EnemyDef::{neutral, calls_help}`, `Content::with_spawns`, validation, `Tuning` for timetables. |
| `crates/together-sim/src/world.rs` | modify | `Enemy::{engaged, wild}`, `World::wild`, events, the targeting rule, reap and move hooks. |
| `crates/together-sim/src/world/extraction.rs` | modify | Timetabled exits, ambushes, hot-zone heat, horde share. |
| `crates/together-sim/src/world/wildlife.rs` | create | Keeping, waking and sleeping, noticing, packs, giving up, idle walking. With its tests. |
| `crates/together-sim/src/world/arena.rs` | modify | Detonation seeks engaged enemies. |
| `content/extraction.ron`, `content/enemies.ron` | modify | Timetables, ambush, wildlife, hot zones, the new outline; flags and nine new kinds. |
| `content/spawns.ron` | create (baked) | Elwynn's creatures. |
| `tools/spawns/bake_spawns.py`, `tools/spawns/check_displays.py`, `tools/spawns/README.md` | create | The bake, the display check, and where the data came from. |
| `crates/benilla-app/src/together/{mod,view,hud,dev}.rs` | modify | Loading and hashing spawns; timetable HUD and drawing; event text; autoplay; showcase. |
| `crates/together-net/src/lib.rs`, `crates/together-net/tests/snapshot_size.rs` | modify | `PROTOCOL` 5; the crowded case. |

---

### Task 1: Timetabled flights

Exits stop being called. Each has a timetable: a flight lands `window` seconds before each departure, waits, and departs with every living hero in its ring. `solo_extract` goes.

**Files:**
- Modify: `crates/together-sim/src/extraction.rs` (`ExitDef`, `ExitState`, `ExtractionSettings`, `Extraction::new`, test fixture)
- Modify: `crates/together-sim/src/content.rs` (`with_extractions` validation, `Tuning` docs, `Content::tuned`)
- Modify: `crates/together-sim/src/world.rs` (`Event`)
- Modify: `crates/together-sim/src/world/extraction.rs` (`tend_exits`, tests)
- Modify: `content/extraction.ron`
- Modify: `crates/benilla-app/src/together/{view,hud,mod,dev}.rs`

**Interfaces:**
- Produces:
  - `ExitDef { name, pos, radius, first: f32, every: f32, window: f32 }`.
  - `ExitState { departs: f32, boarding: bool, rolled: bool, ambush: bool, warned: bool, came: bool }` with `ExitState::lands_in(&self, &ExitDef, now: f32) -> f32`.
  - `Event::FlightDeparted { exit: u8 }`. `Event::ExitOpened` and `Event::FlightCalled` are removed. `Event::FlightLanded`, `Event::HeroExtracted` stay.
  - `Tuning.exits` scales `first` and `every`; `Tuning.flight` scales `window`.

- [ ] **Step 1: Rewrite the exit tests (they fail)**

In `crates/together-sim/src/world/extraction.rs` tests:

1. In `an_extraction_zone_plays_extraction_and_starts_with_its_exits_shut`, replace `assert!(x.exits.iter().all(|e| !e.open));` with `assert!(x.exits.iter().all(|e| !e.boarding));`.
2. Delete these tests: `exits_open_on_their_own_time`, `a_shut_exit_calls_nothing`, `standing_in_an_open_exit_calls_a_flight_that_takes_whoever_is_in_it`, `without_solo_extract_the_flight_waits_for_everyone`, `a_waiting_flight_leaves_when_the_rest_are_aboard_or_down`, `the_last_one_out_ends_the_run_and_leaves_the_fallen`, `an_exit_opens_at_the_time_its_state_carries`.
3. Add:

```rust
    #[test]
    fn a_flight_lands_on_its_timetable_and_departs_with_whoever_is_in_its_ring() {
        let mut w = run("", 2);
        hush(&mut w);
        at(&mut w, 0, 200.0, 0.0);
        // East Gate: first departure at 60, on the ground for 30 before it.
        w.elapsed = 29.0;
        secs(&mut w, 0.5);
        assert!(!state(&w).exits[0].boarding);
        secs(&mut w, 1.0);
        assert!(state(&w).exits[0].boarding);
        assert!(w.drain_events().contains(&Event::FlightLanded { exit: 0 }));
        secs(&mut w, 29.0);
        assert!(w.hero(0).unwrap().alive, "not yet");
        secs(&mut w, 1.0);
        let (out, home) = (w.hero(0).unwrap(), w.hero(1).unwrap());
        assert!(out.extracted && !out.alive);
        assert!(home.alive && !home.extracted);
        assert!(!w.game_over());
        let x = state(&w);
        assert!(!x.exits[0].boarding);
        assert!((x.exits[0].departs - 120.0).abs() < 1e-3, "the next flight");
        let events = w.drain_events();
        assert!(events.contains(&Event::FlightDeparted { exit: 0 }));
        assert!(events.contains(&Event::HeroExtracted { player: 0, exit: 0 }));
    }

    #[test]
    fn only_heroes_in_the_ring_at_departure_are_taken() {
        let mut w = run("", 2);
        hush(&mut w);
        w.elapsed = 45.0;
        at(&mut w, 0, 200.0, 0.0);
        at(&mut w, 1, 100.0, 0.0);
        secs(&mut w, 10.0);
        // The first walks off; the second arrives just in time.
        at(&mut w, 0, 100.0, 0.0);
        at(&mut w, 1, 201.0, 2.0);
        secs(&mut w, 6.0);
        assert!(!w.hero(0).unwrap().extracted);
        assert!(w.hero(1).unwrap().extracted);
    }

    #[test]
    fn a_boarding_flight_heats_the_party() {
        let mut w = run("", 1);
        hush(&mut w);
        at(&mut w, 0, 100.0, 0.0);
        w.elapsed = 30.0;
        secs(&mut w, 10.0);
        // 1 a second for being outside and 4 for the flight on the ground.
        assert!((w.heat_of(0) - 50.0).abs() < 0.5, "{}", w.heat_of(0));
    }

    #[test]
    fn the_last_one_out_ends_the_run_and_leaves_the_fallen() {
        let mut w = run("", 2);
        hush(&mut w);
        w.elapsed = 55.0;
        at(&mut w, 0, 200.0, 0.0);
        let fallen = w.heroes.iter_mut().find(|h| h.player == 1).unwrap();
        fallen.alive = false;
        fallen.hp = 0.0;
        secs(&mut w, 6.0);
        assert!(w.hero(0).unwrap().extracted);
        assert!(!w.hero(1).unwrap().extracted, "the fallen are left behind");
        assert!(w.game_over());
    }

    #[test]
    fn an_exit_departs_at_the_time_its_state_carries() {
        let mut w = run("", 1);
        hush(&mut w);
        assert_eq!(state(&w).exits[0].departs, 60.0);
        at(&mut w, 0, 200.0, 0.0);
        state_mut(&mut w).exits[0].departs = 31.0;
        secs(&mut w, 32.0);
        assert!(w.hero(0).unwrap().extracted);
    }
```

In `crates/together-sim/src/extraction.rs` tests:
- In `TEST_MAPS`, replace the two exits with:

```ron
            (name: "East Gate", pos: (200.0, 0.0), radius: 15.0, first: 60.0, every: 60.0,
             window: 30.0),
            (name: "South Camp", pos: (0.0, -250.0), radius: 15.0, first: 120.0, every: 120.0,
             window: 30.0),
```

- In `a_map_parses_and_is_tied_to_its_zone`, change `assert!(map.settings.heat_shared && map.settings.lose_them && map.settings.solo_extract);` to `assert!(map.settings.heat_shared && map.settings.lose_them);`.
- In `tuning_scales_exits_flights_heat_horde_and_regen`, replace the two exit assertions with:

```rust
        assert_eq!(b.exits[0].first, a.exits[0].first * 0.5);
        assert_eq!(b.exits[0].every, a.exits[0].every * 0.5);
        assert_eq!(b.exits[1].window, a.exits[1].window * 2.0);
```

  and `assert_eq!(same.extractions[0].exits[0].opens_at, a.exits[0].opens_at);` with `assert_eq!(same.extractions[0].exits[0].first, a.exits[0].first);`.
- Add:

```rust
    #[test]
    fn a_timetable_that_cannot_run_is_refused() {
        let err = maps_with("every: 60.0,", "every: 20.0,").unwrap_err();
        assert!(
            matches!(err, ContentError::Invalid { why: "has an exit whose timetable cannot run", .. }),
            "{err}"
        );
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p together-sim 2>&1 | grep -E "^error" | sort | uniq -c`
Expected: compile errors on `first`, `every`, `window`, `boarding`, `departs`, `FlightDeparted`.

- [ ] **Step 3: The types**

In `crates/together-sim/src/extraction.rs`, replace `ExitDef` with:

```rust
/// A place to leave from, and its timetable.
#[derive(Clone, Debug, Deserialize)]
pub struct ExitDef {
    pub name: String,
    pub pos: [f32; 2],
    pub radius: f32,
    /// Seconds into the run its first flight departs...
    pub first: f32,
    /// ...and between departures after that.
    pub every: f32,
    /// Seconds a flight waits on the ground before it departs.
    pub window: f32,
}
```

In `ExtractionSettings`, delete the `solo_extract` field (with its doc and `#[serde(default = "yes")]`) and its line in `Default`. Replace `ExitState` and `Extraction::new`'s `exits:` with:

```rust
/// How one exit stands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExitState {
    /// Seconds into the run its next flight departs: the map's timetable, as the host tuned it.
    pub departs: f32,
    /// The next flight is on the ground, boarding.
    pub boarding: bool,
    /// Whether the next departure draws an ambush has been rolled; it does; the party has been
    /// warned; the pack has come.
    pub rolled: bool,
    pub ambush: bool,
    pub warned: bool,
    pub came: bool,
}

impl ExitState {
    /// Seconds until the next flight lands at `def`'s exit; zero or less once it is down.
    pub fn lands_in(&self, def: &ExitDef, now: f32) -> f32 {
        self.departs - def.window - now
    }
}
```

```rust
            exits: def
                .exits
                .iter()
                .map(|e| ExitState {
                    departs: e.first,
                    ..ExitState::default()
                })
                .collect(),
```

In `crates/together-sim/src/content.rs`, in `with_extractions`, after the "has an exit outside its boundary" check, add:

```rust
            if map
                .exits
                .iter()
                .any(|e| e.every <= 0.0 || e.window <= 0.0 || e.window >= e.every || e.window > e.first)
            {
                return Err(invalid(&map.key, "has an exit whose timetable cannot run"));
            }
```

In `Tuning`, change the docs of `exits` to `/// Seconds until each exit's first flight, and between its flights, times.` and of `flight` to `/// Seconds a flight waits on the ground, times.`. In `Content::tuned`, replace the two exit lines with:

```rust
                exit.first *= t.exits;
                exit.every *= t.exits;
                exit.window *= t.flight;
```

In `crates/together-sim/src/world.rs`'s `Event`, delete `ExitOpened` and `FlightCalled` (with their docs). Change `FlightLanded`'s doc to `/// Extraction: the flight to exit \`exit\` landed and is boarding.`. After `FlightLanded`, add:

```rust
    /// Extraction: the flight from exit `exit` departed, with whoever was aboard.
    FlightDeparted {
        exit: u8,
    },
```

- [ ] **Step 4: The timetable**

In `crates/together-sim/src/world/extraction.rs`, replace the whole `tend_exits` function with:

```rust
    /// Flights keep their exits' timetables: each lands `window` seconds before it departs and
    /// waits, heating the party (shared) or whoever waits in its ring; at departure it takes
    /// every living hero in its ring, and the next flight is due `every` seconds later.
    fn tend_exits(&mut self, def: &ExtractionDef) {
        let living: Vec<(PlayerId, V2)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, h.pos))
            .collect();
        let now = self.elapsed;
        let mut events = Vec::new();
        let mut heat: Vec<(Option<PlayerId>, f32)> = Vec::new();
        let mut taken: Vec<(PlayerId, u8)> = Vec::new();
        let Some(x) = self.run_mut() else {
            return;
        };
        for (i, (exit, state)) in def.exits.iter().zip(x.exits.iter_mut()).enumerate() {
            let tag = i as u8;
            if state.lands_in(exit, now) > 0.0 {
                continue;
            }
            let at = V2::new(exit.pos[0], exit.pos[1]);
            let inside: Vec<PlayerId> = living
                .iter()
                .filter(|(_, p)| p.distance(at) <= exit.radius)
                .map(|&(p, _)| p)
                .collect();
            if !state.boarding {
                state.boarding = true;
                events.push(Event::FlightLanded { exit: tag });
            }
            let gain = def.heat.flight * DT;
            if def.settings.heat_shared {
                heat.push((None, gain));
            } else {
                heat.extend(inside.iter().map(|&p| (Some(p), gain)));
            }
            if now < state.departs {
                continue;
            }
            taken.extend(inside.iter().map(|&p| (p, tag)));
            events.push(Event::FlightDeparted { exit: tag });
            *state = ExitState {
                departs: state.departs + exit.every,
                ..ExitState::default()
            };
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

Add `ExitState` to that file's import: `use crate::extraction::{ExitState, ExtractionDef, ExtractionSettings, SkullDef};`.

- [ ] **Step 5: The shipped timetable**

In `content/extraction.ron`, replace the `exits: [ … ],` list with:

```ron
        exits: [
            // By Stormwind's gate (AreaPOI "Stormwind").
            (name: "Stormwind Gate", pos: (-9150.0, 360.0), radius: 15.0,
             first: 360.0, every: 360.0, window: 30.0),
            // Eastvale Logging Camp, checked in game.
            (name: "Eastvale Logging Camp", pos: (-9467.0, -1276.0), radius: 15.0,
             first: 540.0, every: 420.0, window: 30.0),
            // Westbrook Garrison, on the west road toward Westfall (AreaPOI).
            (name: "Westbrook Garrison", pos: (-9615.0, 667.0), radius: 15.0,
             first: 450.0, every: 480.0, window: 30.0),
        ],
```

and in its `settings:` line, delete `solo_extract: true, `.

- [ ] **Step 6: Sim tests pass**

Run: `cargo test -p together-sim -p together-net 2>&1 | grep -E "^error|FAILED|test result"`
Expected: all pass.

- [ ] **Step 7: The client**

`crates/benilla-app/src/together/view.rs`:
1. In `apply_events`, delete the `Event::ExitOpened { exit } => { … }` and `Event::FlightCalled { exit, .. } => { … }` arms, and replace the `Event::FlightLanded { .. } => {}` arm with:

```rust
            Event::FlightLanded { exit } => {
                if let Some(&e) = views.heroes.get(&sim.me) {
                    let text = format!("A flight is boarding at {}", exit_name(&sim.world, exit));
                    fx.text(e, text, COLOR_EXIT);
                }
            }
            // The flight is drawn from the world's state.
            Event::FlightDeparted { .. } => {}
```

2. After `const FLIGHT_DESCENT: f32 = 8.0;`, add:

```rust
/// A beam marks an exit from this many seconds before its flight lands until it departs.
const BEAM_LEAD: f32 = 60.0;
```

3. In `sync_extraction`, replace `match (state.open, view.beams[i]) {` with:

```rust
        let lands = state.lands_in(&def.exits[i], world.elapsed);
        match (lands <= BEAM_LEAD, view.beams[i]) {
```

   and replace the `let height = match (state.flight, state.landed) { … };` statement with:

```rust
        let height = if lands > FLIGHT_DESCENT {
            None
        } else if lands > 0.0 {
            Some(lands / FLIGHT_DESCENT * FLIGHT_HEIGHT)
        } else {
            Some(0.0)
        };
```

`crates/benilla-app/src/together/hud.rs`: in `extraction_status`, replace the body of the `for (i, (exit, state)) in …` loop with:

```rust
        let to = def.exit_pos(i) - hero.pos;
        let way = format!("{:.0} yd {}", to.length(), compass(to));
        let lands = state.lands_in(exit, world.elapsed);
        let line = if lands <= 0.0 {
            let left = (state.departs - world.elapsed).max(0.0);
            format!("{}: flight boarding, leaves in {}   {way}", exit.name, clock(left))
        } else {
            format!(
                "{}: flight lands in {}, leaves at {}   {way}",
                exit.name,
                clock(lands),
                clock(state.departs)
            )
        };
        s.push('\n');
        s.push_str(&line);
```

`crates/benilla-app/src/together/mod.rs`:
1. Replace `extraction_goal`'s body after `let def = …;` with:

```rust
    let speed = world.hero(me).map_or(7.0, |h| h.speed).max(1.0);
    // The soonest flight this hero can still reach before it leaves.
    let exit = (0..def.exits.len())
        .filter(|&i| {
            from.distance(def.exit_pos(i)) / speed + 5.0 < x.exits[i].departs - world.elapsed
        })
        .min_by(|&a, &b| x.exits[a].departs.total_cmp(&x.exits[b].departs))
        .map(|i| def.exit_pos(i));
    exit.or_else(|| (world.skulls_of(me) >= 3).then(|| def.safe_center()))
```

   and its doc to `/// \`TOGETHER_AUTOPLAY\` in an extraction run: to the exit of the soonest flight it can still reach; home to the safe ring at three skulls; else \`None\`, to roam to shrines as in the arena.`
2. In `showcase_for_tests`, replace the extraction block and its comment with:

```rust
    // An extraction run's flights leave within the first minutes, so a short test run reaches one.
    if let together_sim::Mode::Extraction(x) = &mut world.mode {
        for (i, exit) in x.exits.iter_mut().enumerate() {
            exit.departs = 90.0 + 30.0 * i as f32;
        }
    }
```

   and change `an extraction run's exits are open` in its doc to `an extraction run's flights come early`.

`crates/benilla-app/src/together/dev.rs`: in `ROWS`, rename `"Exits open (time)"` to `"Flight timetable"` and `"Flight time"` to `"Boarding time"`; change the hint text to `"Timetable and boarding: below 1 is sooner and shorter."`.

- [ ] **Step 8: Build**

Run: `cargo build --release -p azeroth-together 2>&1 | grep -E "^(error|warning: unused)|Finished"`
Expected: `Finished`.

- [ ] **Step 9: Commit**

```bash
git add crates content/extraction.ron
git commit -m "Extraction: flights on a timetable, boarding then departing with whoever is aboard

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Ambushes

Some departures draw an ambush: a warning `warn` seconds before the flight lands, then a pack from one side when it lands, sized by the party's heat.

**Files:**
- Modify: `crates/together-sim/src/extraction.rs` (`AmbushDef`, `ExtractionDef::ambush`)
- Modify: `crates/together-sim/src/content.rs` (pack validation)
- Modify: `crates/together-sim/src/world.rs` (`Event::AmbushWarned`)
- Modify: `crates/together-sim/src/world/extraction.rs` (`tend_exits`, `send_ambush`, tests)
- Modify: `content/extraction.ron`, `crates/benilla-app/src/together/{view,hud}.rs`

**Interfaces:**
- Consumes (Task 1): `ExitState::{rolled, ambush, warned, came, lands_in}`.
- Produces: `AmbushDef { chance, warn, pack: (u32, u32), per_skull, ring: (f32, f32) }`; `Event::AmbushWarned { exit: u8 }`.

- [ ] **Step 1: Failing tests**

In `crates/together-sim/src/world/extraction.rs` tests:

```rust
    #[test]
    fn an_ambush_is_warned_of_then_comes_from_one_side_when_the_flight_lands() {
        let mut content = test_content("");
        content.extractions[0].ambush.chance = 1.0;
        let mut w = World::new(Arc::new(content), "goldshire", 5).unwrap();
        w.add_hero(0, "mage").unwrap();
        hush(&mut w);
        // East Gate lands at 30; the warning comes 20 before.
        w.elapsed = 9.0;
        secs(&mut w, 2.0);
        assert!(w.drain_events().contains(&Event::AmbushWarned { exit: 0 }));
        assert!(w.enemies.is_empty(), "warned, not come");
        secs(&mut w, 19.5);
        let gate = V2::new(200.0, 0.0);
        assert!(w.enemies.len() >= 6, "{}", w.enemies.len());
        for e in &w.enemies {
            let d = e.pos.distance(gate);
            assert!((35.0..=61.0).contains(&d), "{d}");
        }
    }

    #[test]
    fn without_a_chance_no_ambush_comes() {
        let mut w = run("", 1);
        hush(&mut w);
        w.elapsed = 9.0;
        secs(&mut w, 22.0);
        assert!(w.enemies.is_empty());
        assert!(!w
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::AmbushWarned { .. })));
    }
```

In `crates/together-sim/src/extraction.rs` tests:

```rust
    #[test]
    fn a_pack_whose_fewest_is_more_than_its_most_is_refused() {
        let err = maps_with("pack: (6, 10)", "pack: (10, 6)").unwrap_err();
        assert!(matches!(err, ContentError::Invalid { .. }), "{err}");
    }
```

Run: `cargo test -p together-sim 2>&1 | grep -E "^error" | sort | uniq -c`
Expected: compile errors on `ambush` and `AmbushWarned`.

- [ ] **Step 2: Types and validation**

In `crates/together-sim/src/extraction.rs`, after `HunterDef`:

```rust
/// A pack sent to a waiting flight.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct AmbushDef {
    /// The chance a departure draws one.
    pub chance: f32,
    /// Seconds before its flight lands that the party is warned.
    pub warn: f32,
    /// Enemies in the pack at no heat, fewest and most...
    pub pack: (u32, u32),
    /// ...and this share more for each skull of the party's heat.
    pub per_skull: f32,
    /// How far from the exit they come, nearest and furthest.
    pub ring: (f32, f32),
}

impl Default for AmbushDef {
    fn default() -> Self {
        Self {
            chance: 0.0,
            warn: 20.0,
            pack: (6, 10),
            per_skull: 0.25,
            ring: (40.0, 60.0),
        }
    }
}
```

In `ExtractionDef`, after `exits`:

```rust
    #[serde(default)]
    pub ambush: AmbushDef,
```

Re-export `AmbushDef` from `lib.rs` with the other extraction types. In `with_extractions`, after the timetable check:

```rust
            let packs = [map.ambush.pack, map.heat.hunters.pack];
            if packs.iter().any(|&(lo, hi)| lo > hi) {
                return Err(invalid(&map.key, "has a pack whose fewest is more than its most"));
            }
```

In `world.rs`'s `Event`, after `FlightDeparted`:

```rust
    /// Extraction: a pack is converging on the flight due at exit `exit`.
    AmbushWarned {
        exit: u8,
    },
```

- [ ] **Step 3: Rolling, warning, sending**

In `tend_exits`, before `let Some(x) = self.run_mut() else {`, add:

```rust
        // Each departure's ambush is rolled once.
        let unrolled: Vec<usize> = match &self.mode {
            Mode::Extraction(x) => (0..x.exits.len()).filter(|&i| !x.exits[i].rolled).collect(),
            Mode::Arena => return,
        };
        for i in unrolled {
            let ambush = self.rng.f32() < def.ambush.chance;
            if let Some(x) = self.run_mut() {
                x.exits[i].rolled = true;
                x.exits[i].ambush = ambush;
            }
        }
        let mut packs = Vec::new();
```

Inside the loop, replace `if state.lands_in(exit, now) > 0.0 {` with:

```rust
            let lands = state.lands_in(exit, now);
            if state.ambush && !state.warned && lands <= def.ambush.warn {
                state.warned = true;
                events.push(Event::AmbushWarned { exit: tag });
            }
            if state.ambush && !state.came && lands <= 0.0 {
                state.came = true;
                packs.push(i);
            }
            if lands > 0.0 {
```

After the loop, before `for (who, amount) in heat {`, add:

```rust
        for exit in packs {
            self.send_ambush(def, exit);
        }
```

Add to `impl World`:

```rust
    /// A pack converging on exit `exit`'s flight from one side: the strongest enemies in play,
    /// more of them the hotter the party.
    fn send_ambush(&mut self, def: &ExtractionDef, exit: usize) {
        let at = def.exit_pos(exit);
        let skulls = def.skull_index(self.party_heat());
        let skull = def.heat.skulls[skulls];
        let (lo, hi) = def.ambush.pack;
        let base = lo + self.rng.index((hi - lo + 1) as usize) as u32;
        let count = (base as f32 * (1.0 + def.ambush.per_skull * skulls as f32)).round() as u32;
        let roster = self.zone().enemy_ix.clone();
        let (_, top) = self.tiers();
        let kind = roster[(top + skull.tier).min(roster.len() - 1)];
        let side = self.rng.range(0.0, std::f32::consts::TAU);
        let bounds = def.bounds();
        for _ in 0..count {
            let angle = side + self.rng.range(-0.5, 0.5);
            let r = self.rng.range(def.ambush.ring.0, def.ambush.ring.1);
            let pos = at + V2::from_angle(angle, r);
            if !bounds.inside(pos, 0.0) || def.in_margin(pos) {
                continue;
            }
            let id = self.spawn_enemy(kind, pos, at, false);
            self.toughen(id, skull.toughness, 1.0);
        }
    }
```

- [ ] **Step 4: Shipped values and the client**

In `content/extraction.ron`, after `exits: [ … ],`:

```ron
        // Half the departures draw a pack, announced 20 seconds before the flight lands.
        ambush: (chance: 0.5, warn: 20.0, pack: (6, 10), per_skull: 0.25, ring: (40.0, 60.0)),
```

`view.rs` `apply_events`, after the `FlightDeparted` arm:

```rust
            Event::AmbushWarned { exit } => {
                if let Some(&e) = views.heroes.get(&sim.me) {
                    let text = format!("Ambush converging on {}!", exit_name(&sim.world, exit));
                    fx.text(e, text, COLOR_HEAT);
                }
            }
```

`hud.rs` `extraction_status`: change `let line = if lands <= 0.0 {` to `let mut line = if lands <= 0.0 {`, and after that `let … ;` statement add:

```rust
        if state.warned && !state.came {
            line.push_str("   AMBUSH COMING");
        }
```

- [ ] **Step 5: Tests and build**

Run: `cargo test -p together-sim -p together-net 2>&1 | grep -E "^error|FAILED|test result"` → all pass.
Run: `cargo build --release -p azeroth-together 2>&1 | grep -E "^(error|warning: unused)|Finished"` → `Finished`.

- [ ] **Step 6: Commit**

```bash
git add crates content/extraction.ron
git commit -m "Extraction: ambushes on some departures, warned of before the flight lands

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Wildlife content: enemy kinds, spawns, hot zones, the outline

The data part of the living map: enemy flags and new kinds, `spawns.ron`'s format and loading, hot zones, the wildlife settings and the new outline. No behaviour yet.

**Files:**
- Modify: `crates/together-sim/src/content.rs` (`EnemyDef`, `with_spawns`, test `ENEMIES`)
- Modify: `crates/together-sim/src/extraction.rs` (`WildlifeDef`, `HotZoneDef`, `SpawnDef`, `ExtractionDef`, `HeatDef`)
- Modify: `crates/together-sim/src/lib.rs`
- Modify: `content/enemies.ron`, `content/extraction.ron`
- Create: `tools/spawns/check_displays.py`

**Interfaces:**
- Produces:
  - `EnemyDef::{neutral: bool, calls_help: bool}`.
  - `WildlifeDef { share, wake, notice, notice_per_level, notice_min, notice_per_skull, social, help, toughness, reward, give_up, leash, respawn, max_awake: u32, walk }` (`#[serde(default)]`, defaults as in the spec).
  - `HotZoneDef { name, center: [f32; 2], radius }` with `contains(V2) -> bool`; `ExtractionDef::hot_zone_at(V2) -> Option<&HotZoneDef>`.
  - `SpawnDef { enemy: String, pos: [f32; 2], wander: f32, respawn: f32, path: Vec<[f32; 2]>, enemy_ix: usize }` with `at() -> V2`.
  - `ExtractionDef::{wildlife, hot_zones, horde_share (default 1.0), spawns}`; `HeatDef::{notice, hot_zone}` (default 0).
  - `Content::with_spawns(self, &str) -> Result<Content, ContentError>`.

- [ ] **Step 1: Failing tests**

In `crates/together-sim/src/content.rs`'s test `ENEMIES`, add after Hogger:

```ron
        (key: "boar", name: "Stonetusk Boar", display: 503, level: 1, hp: 10.0, speed: 4.6,
         damage: 2.0, xp: 1, neutral: true),
        (key: "kobold", name: "Kobold Miner", display: 373, level: 1, hp: 10.0, speed: 4.2,
         damage: 2.0, xp: 1, calls_help: true),
```

In `crates/together-sim/src/extraction.rs` tests, add:

```rust
    const ONE_SPAWN: &str = r#"[
        (map: "goldshire", spawns: [
            (enemy: "wolf", pos: (150.0, 0.0), respawn: 30.0, path: [(150.0, 0.0), (150.0, 40.0)]),
        ]),
    ]"#;

    #[test]
    fn spawns_attach_to_their_map_and_name_their_enemies() {
        let c = test_content("").with_spawns(ONE_SPAWN).unwrap();
        let s = &c.extractions[0].spawns;
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].enemy_ix, 0);
        assert_eq!(s[0].at(), V2::new(150.0, 0.0));
        assert_eq!(s[0].path.len(), 2);
        assert_eq!(s[0].wander, 0.0);
        assert!(c.enemies[2].neutral && !c.enemies[2].calls_help);
        assert!(c.enemies[3].calls_help);
    }

    #[test]
    fn spawns_for_a_missing_map_or_creature_are_refused() {
        let base = || test_content("");
        let err = base()
            .with_spawns(&ONE_SPAWN.replace(r#"map: "goldshire""#, r#"map: "nowhere""#))
            .unwrap_err();
        assert!(matches!(err, ContentError::UnknownKey { .. }), "{err}");
        let err = base()
            .with_spawns(&ONE_SPAWN.replace(r#"enemy: "wolf""#, r#"enemy: "dragon""#))
            .unwrap_err();
        assert!(matches!(err, ContentError::UnknownKey { .. }), "{err}");
    }

    #[test]
    fn a_hot_zone_holds_what_is_within_its_radius() {
        let zone = HotZoneDef {
            name: "Den".into(),
            center: [10.0, 0.0],
            radius: 5.0,
        };
        assert!(zone.contains(V2::new(13.0, 0.0)));
        assert!(!zone.contains(V2::new(16.0, 0.0)));
        let c = test_content("");
        let map = &c.extractions[0];
        assert!(map.hot_zones.is_empty() && map.hot_zone_at(V2::ZERO).is_none());
        assert_eq!(map.horde_share, 1.0);
        assert_eq!(map.wildlife.wake, 140.0);
        assert_eq!(map.heat.notice, 0.0);
    }
```

Run: `cargo test -p together-sim 2>&1 | grep -E "^error" | sort | uniq -c`
Expected: compile errors (`neutral`, `with_spawns`, `HotZoneDef`, …).

- [ ] **Step 2: Enemy flags**

In `EnemyDef` (`content.rs`), after `radius`:

```rust
    /// Never notices a hero: fights back only when hurt (boars, Northshire's wolves). Only an
    /// extraction map's wildlife minds it.
    #[serde(default)]
    pub neutral: bool,
    /// Shouts when it engages, bringing its kind from further off (Defias, kobolds, gnolls).
    #[serde(default)]
    pub calls_help: bool,
```

- [ ] **Step 3: Wildlife, hot zones, spawns**

In `extraction.rs`, after `AmbushDef`:

```rust
/// How a map's creatures behave.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct WildlifeDef {
    /// The share of the map's spawns a run keeps (every one in a hot zone is kept).
    pub share: f32,
    /// A creature wakes when a living hero comes this near its spawn, and sleeps again (unless
    /// engaged) when every living hero is 20 yd further off than that.
    pub wake: f32,
    /// A creature notices a hero this near...
    pub notice: f32,
    /// ...plus this much for each level it has over the party's (less for each under)...
    pub notice_per_level: f32,
    /// ...never under this...
    pub notice_min: f32,
    /// ...times one plus this for each skull of the party's heat.
    pub notice_per_skull: f32,
    /// Its kind this near it engages with it...
    pub social: f32,
    /// ...or this near, when its kind calls for help.
    pub help: f32,
    /// Its health and damage, times the horde's of its kind.
    pub toughness: f32,
    /// XP and gold from it, times.
    pub reward: f32,
    /// Seconds an engaged creature nobody has hurt, and that has hurt nobody, keeps on before
    /// it gives up...
    pub give_up: f32,
    /// ...once every living hero is further than this.
    pub leash: f32,
    /// Its spawn's respawn time, times.
    pub respawn: f32,
    /// Creatures awake at once, at most, per living hero.
    pub max_awake: u32,
    /// The share of its speed a creature walks at while idle or on patrol.
    pub walk: f32,
}

impl Default for WildlifeDef {
    fn default() -> Self {
        Self {
            share: 0.5,
            wake: 140.0,
            notice: 10.0,
            notice_per_level: 1.0,
            notice_min: 5.0,
            notice_per_skull: 0.15,
            social: 12.0,
            help: 30.0,
            toughness: 1.5,
            reward: 2.0,
            give_up: 10.0,
            leash: 40.0,
            respawn: 1.0,
            max_awake: 120,
            walk: 0.4,
        }
    }
}

/// A place dense with creatures, where heat climbs faster.
#[derive(Clone, Debug, Deserialize)]
pub struct HotZoneDef {
    pub name: String,
    pub center: [f32; 2],
    pub radius: f32,
}

impl HotZoneDef {
    pub fn contains(&self, p: V2) -> bool {
        p.distance(V2::new(self.center[0], self.center[1])) <= self.radius
    }
}

/// One of a map's creatures, from `spawns.ron`.
#[derive(Clone, Debug, Deserialize)]
pub struct SpawnDef {
    /// A key into `enemies.ron`.
    pub enemy: String,
    pub pos: [f32; 2],
    /// How far from its spawn it wanders while idle; 0 stands.
    #[serde(default)]
    pub wander: f32,
    /// Seconds it stays dead.
    pub respawn: f32,
    /// A patrol, walked in a loop; empty for none.
    #[serde(default)]
    pub path: Vec<[f32; 2]>,
    /// [`SpawnDef::enemy`] resolved to an index into [`crate::Content::enemies`].
    #[serde(skip)]
    pub enemy_ix: usize,
}

impl SpawnDef {
    pub fn at(&self) -> V2 {
        V2::new(self.pos[0], self.pos[1])
    }
}
```

In `ExtractionDef`, after `ambush`:

```rust
    #[serde(default)]
    pub wildlife: WildlifeDef,
    #[serde(default)]
    pub hot_zones: Vec<HotZoneDef>,
    /// The heat-driven horde's size, times: smaller once the forest is dangerous too.
    #[serde(default = "one")]
    pub horde_share: f32,
    /// The map's creatures, from `spawns.ron` ([`crate::Content::with_spawns`]).
    #[serde(skip)]
    pub spawns: Vec<SpawnDef>,
```

with, next to `fn yes()`:

```rust
fn one() -> f32 {
    1.0
}
```

In `HeatDef`, after `flight`:

```rust
    /// Gained for each creature that notices a hero.
    #[serde(default)]
    pub notice: f32,
    /// Gained each second in a hot zone, on top of being outside.
    #[serde(default)]
    pub hot_zone: f32,
```

In `impl ExtractionDef`:

```rust
    /// The hot zone `p` is in, if any.
    pub fn hot_zone_at(&self, p: V2) -> Option<&HotZoneDef> {
        self.hot_zones.iter().find(|z| z.contains(p))
    }
```

Change `ExtractionDef::boundary`'s doc to `/// The playable area's outline in WoW \`(x, y)\`: a polygon, corners in order.` and `Bounds::Polygon`'s doc to `/// A polygon, corners in order.` (the clamp projects onto the nearest edge either way).

In `lib.rs`'s extraction re-export, add `HotZoneDef, SpawnDef, WildlifeDef`.

In `content.rs`, after `with_extractions`:

```rust
    /// These contents with the creatures in `text` (`spawns.ron`), each set tied to its
    /// extraction map and each creature to its enemy.
    pub fn with_spawns(mut self, text: &str) -> Result<Self, ContentError> {
        const FILE: &str = "spawns.ron";
        #[derive(Deserialize)]
        struct SpawnSet {
            map: String,
            spawns: Vec<ExtractionSpawn>,
        }
        type ExtractionSpawn = crate::extraction::SpawnDef;
        let sets: Vec<SpawnSet> = parse(FILE, text)?;
        let maps = index("extraction.ron", self.extractions.iter().map(|m| m.key.as_str()))?;
        let enemies = index("enemies.ron", self.enemies.iter().map(|e| e.key.as_str()))?;
        for set in sets {
            let m = resolve(&maps, FILE, &set.map, &set.map)?;
            let mut spawns = set.spawns;
            for s in &mut spawns {
                s.enemy_ix = resolve(&enemies, FILE, &set.map, &s.enemy)?;
            }
            self.extractions[m].spawns = spawns;
        }
        Ok(self)
    }
```

- [ ] **Step 4: Sim tests pass**

Run: `cargo test -p together-sim 2>&1 | grep -E "^error|FAILED|test result"`
Expected: all pass.

- [ ] **Step 5: The new kinds' models are in the data**

Build the extraction tool: `cargo build --release -p benilla-formats --bin mpqx 2>&1 | tail -1`.

Create `tools/spawns/check_displays.py`:

```python
"""Check that creature displays have their model in the game data.

    python -I tools/spawns/check_displays.py <mpqx.exe> <WoW/Data> <display id>...

Exits 1 if any display's model is missing (the client would draw nothing for it).
"""
import os
import struct
import subprocess
import sys
import tempfile


def rows(path):
    data = open(path, 'rb').read()
    _, count, _, size, _ = struct.unpack('<4s4I', data[:20])
    records = data[20:20 + count * size]
    strings = data[20 + count * size:]
    for r in range(count):
        yield records[r * size:(r + 1) * size], strings


def u32(row, field):
    return struct.unpack_from('<I', row, field * 4)[0]


def text(strings, offset):
    return strings[offset:strings.index(b'\0', offset)].decode('latin-1')


def main():
    mpqx, data, ids = sys.argv[1], sys.argv[2], [int(a) for a in sys.argv[3:]]
    tmp = tempfile.mkdtemp()

    def extract(vpath, out):
        return subprocess.run([mpqx, data, vpath, out], capture_output=True).returncode == 0

    cdi, cmd = os.path.join(tmp, 'cdi.dbc'), os.path.join(tmp, 'cmd.dbc')
    assert extract('DBFilesClient\\CreatureDisplayInfo.dbc', cdi)
    assert extract('DBFilesClient\\CreatureModelData.dbc', cmd)
    model_of = {u32(r, 0): u32(r, 1) for r, _ in rows(cdi)}
    path_of = {u32(r, 0): text(s, u32(r, 2)) for r, s in rows(cmd)}
    missing = 0
    for d in ids:
        model = model_of.get(d)
        path = path_of.get(model, '')
        m2 = path[:-4] + '.m2' if path.lower().endswith('.mdx') else path
        ok = bool(m2) and extract(m2, os.path.join(tmp, 'model.m2'))
        print(f"{d}: model {model} {m2 or '?'} {'ok' if ok else 'MISSING'}")
        missing += not ok
    sys.exit(1 if missing else 0)


main()
```

Run:

```bash
python -I tools/spawns/check_displays.py target/release/mpqx.exe D:/Games/azeroth-together/WoW/Data 441 10791 11415 10912 10913 365 381 163 368
```

Expected: nine `ok` lines. For any `MISSING`, use the display of the kind's stat template from Step 6 below (the creature then wears a stand-in model), and ledger a `Ruling:` naming it.

- [ ] **Step 6: Enemy kinds**

In `content/enemies.ron`:
1. Add `neutral: true,` to `young_wolf`, `timber_wolf`, `stonetusk_boar` and `rockhide_boar` (after their `level:`).
2. Add `calls_help: true,` to `defias_thug`, `defias_cutpurse`, `defias_bandit`, `defias_rogue_wizard`, `kobold_tunneler` and `kobold_miner`.
3. After the Elwynn block (before `// --- Westfall`), add:

```ron
    // --- Elwynn's wildlife only (spawns.ron), each with the numbers of a like creature above ---
    // Like defias_bandit.
    (key: "murloc_forager", name: "Murloc Forager", display: 441, level: 9,
     hp: 283.0, speed: 4.2, damage: 34.0, xp: 14),
    // Like gray_forest_wolf; gnolls call for help.
    (key: "riverpaw_runt", name: "Riverpaw Runt", display: 10791, level: 8, calls_help: true,
     hp: 240.0, speed: 4.2, damage: 30.0, xp: 12),
    // Like defias_bandit, at a wolf's pace.
    (key: "prowler", name: "Prowler", display: 11415, level: 9,
     hp: 283.0, speed: 5.4, damage: 34.0, xp: 14),
    // Like defias_thug.
    (key: "kobold_worker", name: "Kobold Worker", display: 10912, level: 3, neutral: true,
     calls_help: true, hp: 21.0, speed: 4.2, damage: 7.0, xp: 2),
    // Like timber_wolf.
    (key: "kobold_vermin", name: "Kobold Vermin", display: 10913, level: 2, neutral: true,
     hp: 16.0, speed: 4.2, damage: 3.0, xp: 2),
    // Like defias_thug.
    (key: "kobold_laborer", name: "Kobold Laborer", display: 365, level: 4, calls_help: true,
     hp: 21.0, speed: 4.2, damage: 7.0, xp: 2),
    // Like young_forest_bear.
    (key: "longsnout", name: "Longsnout", display: 381, level: 10, neutral: true, radius: 1.3,
     hp: 420.0, speed: 4.6, damage: 42.0, xp: 24),
    // Like kobold_miner.
    (key: "kobold_geomancer", name: "Kobold Geomancer", display: 163, level: 8, calls_help: true,
     hp: 132.0, speed: 4.2, damage: 21.0, xp: 7),
    // Like gray_forest_wolf.
    (key: "mine_spider", name: "Mine Spider", display: 368, level: 9,
     hp: 240.0, speed: 4.2, damage: 30.0, xp: 12),
```

- [ ] **Step 7: The map's wildlife, hot zones and outline**

In `content/extraction.ron`:
1. Replace `boundary: […],` and its comment with:

```ron
        // Elwynn from the spawn data's density map: south of Stormwind's walls (the city is not
        // in the trimmed data), Northshire and the north-east ridge, Redridge's mountains on the
        // east, Duskwood's river on the south (further south in the west, for Forest's Edge),
        // Westfall on the west. Corners in order.
        boundary: [
            (-9135.0, 820.0), (-9135.0, 300.0), (-8850.0, 300.0), (-8850.0, -1300.0),
            (-9300.0, -1480.0), (-9950.0, -1480.0), (-9950.0, -400.0), (-10150.0, -100.0),
            (-10150.0, 700.0), (-9700.0, 820.0),
        ],
```

2. In `heat: ( … )`, after `flight: 4.0,`, add `notice: 2.0,` and `hot_zone: 3.0,`.
3. After the `ambush:` line, add:

```ron
        // Elwynn's creatures (spawns.ron): half of them kept, tougher than the horde, waking
        // out of sight and noticing heroes who come close.
        wildlife: (share: 0.5, wake: 140.0, notice: 10.0, notice_per_level: 1.0,
                   notice_min: 5.0, notice_per_skull: 0.15, social: 12.0, help: 30.0,
                   toughness: 1.5, reward: 2.0, give_up: 10.0, leash: 40.0, respawn: 1.0,
                   max_awake: 120, walk: 0.4),
        // Dense places where heat climbs faster: every creature in them is kept.
        hot_zones: [
            (name: "Jasperlode Mine", center: (-9180.0, -630.0), radius: 100.0),
            (name: "Fargodeep Mine", center: (-9790.0, 140.0), radius: 100.0),
            (name: "Crystal Lake", center: (-9480.0, -340.0), radius: 110.0),
            (name: "Stone Cairn Lake", center: (-9380.0, -1080.0), radius: 150.0),
            (name: "Brackwell Pumpkin Patch", center: (-9800.0, -890.0), radius: 90.0),
            (name: "Forest's Edge", center: (-10010.0, 610.0), radius: 120.0),
        ],
        // The heat-driven horde at half size: the forest is dangerous too.
        horde_share: 0.5,
```

Run: `cargo test -p together-sim 2>&1 | grep -E "^error|FAILED|test result"` → all pass (the shipped map still parses and validates with the new outline and exits).

- [ ] **Step 8: Commit**

```bash
git add crates/together-sim content tools/spawns/check_displays.py
git commit -m "Content: wildlife, hot zones and spawns for extraction maps; Elwynn's new kinds and outline

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The bake

`tools/spawns/bake_spawns.py` turns the extracted spawn list into `content/spawns.ron`.

**Files:**
- Create: `tools/spawns/bake_spawns.py`, `tools/spawns/README.md`, `content/spawns.ron` (generated)
- Modify: `crates/together-sim/tests/shipped_content.rs`

**Interfaces:**
- Consumes: the extracted spawns JSON at `C:\Users\Olivs\AppData\Local\Temp\claude\D--Games-azeroth-together\ced5222e-ac81-40a6-a2c3-afa972f4cede\scratchpad\spawndb\elwynn_spawns.json` (fields: `name, zone, subzone, permanent, reaction_to_alliance, rank, x, y, z, wander_distance, spawn_time_s, movement, waypoints`); `with_spawns` (Task 3).
- Produces: `content/spawns.ron` with one set, `map: "elwynn"`.

- [ ] **Step 1: The failing shipped test**

In `crates/together-sim/tests/shipped_content.rs`, after the `.and_then(|c| c.with_extractions(…))` line in `shipped()`, add:

```rust
    .and_then(|c| c.with_spawns(include_str!("../../../content/spawns.ron")))
```

and append:

```rust
#[test]
fn elwynns_forest_is_full_but_goldshire_is_clear() {
    let c = shipped();
    let map = &c.extractions[0];
    assert!(map.spawns.len() > 600, "{}", map.spawns.len());
    let bounds = map.bounds();
    for s in &map.spawns {
        assert!(bounds.inside(s.at(), 0.0), "outside the outline: {:?}", s.pos);
        assert!(!map.in_margin(s.at()), "in Goldshire's margin: {:?}", s.pos);
    }
}
```

Run: `cargo test -p together-sim --test shipped_content 2>&1 | grep -E "^error" | head -2`
Expected: `couldn't read … content/spawns.ron`.

- [ ] **Step 2: The bake**

Create `tools/spawns/bake_spawns.py`:

```python
"""Bake Elwynn's creatures for extraction mode.

    python -I tools/spawns/bake_spawns.py <elwynn_spawns.json> content/spawns.ron

Keeps every permanent hostile or neutral creature of a kind in KINDS that stands inside the map's
outline, out of Goldshire's margin and not deep inside a mine (it would stand on the hill above),
and writes them as content/spawns.ron. See README.md for where the JSON comes from.
"""
import json
import math
import sys

# Creature name in the database -> key in content/enemies.ron.
KINDS = {
    'Murloc Forager': 'murloc_forager', 'Rockhide Boar': 'rockhide_boar',
    'Riverpaw Runt': 'riverpaw_runt', 'Mangy Wolf': 'mangy_wolf', 'Prowler': 'prowler',
    'Defias Thug': 'defias_thug', 'Defias Bandit': 'defias_bandit',
    'Gray Forest Wolf': 'gray_forest_wolf', 'Stonetusk Boar': 'stonetusk_boar',
    'Kobold Miner': 'kobold_miner', 'Young Forest Bear': 'young_forest_bear',
    'Kobold Worker': 'kobold_worker', 'Kobold Tunneler': 'kobold_tunneler', 'Murloc': 'murloc',
    'Kobold Vermin': 'kobold_vermin', 'Defias Cutpurse': 'defias_cutpurse',
    'Young Wolf': 'young_wolf', 'Defias Rogue Wizard': 'defias_rogue_wizard',
    'Timber Wolf': 'timber_wolf', 'Kobold Laborer': 'kobold_laborer',
    'Murloc Streamrunner': 'murloc_streamrunner', 'Forest Spider': 'forest_spider',
    'Longsnout': 'longsnout', 'Kobold Geomancer': 'kobold_geomancer',
    'Mine Spider': 'mine_spider',
}

# Keep in step with content/extraction.ron's `boundary` and `safe`.
OUTLINE = [
    (-9135.0, 820.0), (-9135.0, 300.0), (-8850.0, 300.0), (-8850.0, -1300.0),
    (-9300.0, -1480.0), (-9950.0, -1480.0), (-9950.0, -400.0), (-10150.0, -100.0),
    (-10150.0, 700.0), (-9700.0, 820.0),
]
SAFE = (-9480.1, 63.5)
SAFE_MARGIN = 90.0
# A mine creature this far below the open-air creatures around it is underground.
UNDERGROUND = 8.0


def inside(x, y):
    hit = False
    j = len(OUTLINE) - 1
    for i in range(len(OUTLINE)):
        (ax, ay), (bx, by) = OUTLINE[i], OUTLINE[j]
        if (ay > y) != (by > y) and x < (bx - ax) * (y - ay) / (by - ay) + ax:
            hit = not hit
        j = i
    return hit


def underground(s, surface):
    if 'Mine' not in s.get('subzone', ''):
        return False
    near = sorted(t['z'] for t in surface if math.hypot(t['x'] - s['x'], t['y'] - s['y']) < 80)
    return not near or s['z'] < near[len(near) // 2] - UNDERGROUND


def path_of(s):
    if s.get('movement') not in ('path', 'linpath'):
        return []
    points = [(p['x'], p['y']) for p in sorted(s.get('waypoints') or [], key=lambda p: p['point'])]
    length = sum(math.dist(a, b) for a, b in zip(points, points[1:]))
    if len(points) < 2 or length < 5:
        return []
    if s['movement'] == 'linpath':
        points += points[-2:0:-1]
    return points


def main():
    spawns = json.load(open(sys.argv[1], encoding='utf-8'))
    pool = [
        s for s in spawns
        if s.get('zone') == 'Elwynn Forest' and s.get('permanent')
        and s['reaction_to_alliance'] in ('hostile', 'neutral') and s['rank'] == 0
        and s['name'] in KINDS
    ]
    surface = [s for s in pool if 'Mine' not in s.get('subzone', '')]
    kept, dropped = [], {'outline': 0, 'goldshire': 0, 'underground': 0}
    for s in pool:
        if not inside(s['x'], s['y']):
            dropped['outline'] += 1
        elif math.dist((s['x'], s['y']), SAFE) < SAFE_MARGIN:
            dropped['goldshire'] += 1
        elif underground(s, surface):
            dropped['underground'] += 1
        else:
            kept.append(s)
    kept.sort(key=lambda s: (KINDS[s['name']], s['x'], s['y']))
    out = [
        '// Elwynn Forest\'s creatures for extraction mode, baked by tools/spawns/bake_spawns.py from',
        '// the CMaNGOS Classic DB (ClassicDB_1_12_1_z2815, GPL-3.0; see tools/spawns/README.md).',
        '// Do not edit by hand: change the bake and run it again.',
        '[',
        '    (map: "elwynn", spawns: [',
    ]
    for s in kept:
        line = (f'        (enemy: "{KINDS[s["name"]]}", pos: ({s["x"]:.1}, {s["y"]:.1}), '
                f'wander: {float(s.get("wander_distance") or 0):.1}, '
                f'respawn: {float(s["spawn_time_s"][0]):.0}')
        path = path_of(s)
        if path:
            line += ', path: [' + ', '.join(f'({x:.1}, {y:.1})' for x, y in path) + ']'
        out.append(line + '),')
    out += ['    ]),', ']', '']
    open(sys.argv[2], 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
    counts = {}
    for s in kept:
        counts[KINDS[s['name']]] = counts.get(KINDS[s['name']], 0) + 1
    print(f'kept {len(kept)} of {len(pool)}; left out {dropped}')
    for k, n in sorted(counts.items(), key=lambda kv: -kv[1]):
        print(f'  {n:4} {k}')
    print(f'  patrols: {sum(1 for s in kept if path_of(s))}')


main()
```

Run:

```bash
python -I tools/spawns/bake_spawns.py "C:/Users/Olivs/AppData/Local/Temp/claude/D--Games-azeroth-together/ced5222e-ac81-40a6-a2c3-afa972f4cede/scratchpad/spawndb/elwynn_spawns.json" content/spawns.ron
```

Expected: `kept` roughly 850–1,050 of about 1,130, some left out for each reason (`underground` mostly Fargodeep), and around 40 patrols. If `kept` is under 600, stop and investigate.

- [ ] **Step 3: Where the data came from**

Create `tools/spawns/README.md`:

```markdown
# Elwynn's creatures

`content/spawns.ron` is baked from the CMaNGOS Classic DB full dump
(`Full_DB/ClassicDB_1_12_1_z2815.sql.gz` in github.com/cmangos/classic-db, last changed in commit
`28ef6259`, 2024-01-17), which is GPL-3.0. That makes `spawns.ron` GPL-3.0 data: fine for this
personal project, and one more reason the game is never published.

1. The dump's `creature`, `creature_template`, movement and spawn-group tables were parsed into
   `elwynn_spawns.json`: every spawn in Elwynn's box with its creature, level, rank, faction
   reaction (from `FactionTemplate.dbc`), terrain zone and subzone (from the ADT area ids),
   respawn time, wander distance, movement and waypoints.
2. `bake_spawns.py` keeps the creatures extraction mode uses and writes `content/spawns.ron`.
3. `check_displays.py` checks a creature display's model is in the game data.

To bake again: `python -I tools/spawns/bake_spawns.py <elwynn_spawns.json> content/spawns.ron`.
Keep its `OUTLINE` and `SAFE` in step with `content/extraction.ron`.
```

- [ ] **Step 4: Tests pass**

Run: `cargo test -p together-sim 2>&1 | grep -E "^error|FAILED|test result"`
Expected: all pass, including `elwynns_forest_is_full_but_goldshire_is_clear`.

- [ ] **Step 5: Commit**

```bash
git add tools/spawns content/spawns.ron crates/together-sim/tests/shipped_content.rs
git commit -m "Content: Elwynn's creatures baked from the CMaNGOS Classic DB

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Wildlife wakes and sleeps; spells seek engaged enemies

Creatures exist only near living heroes: they wake at their spawn and sleep again when every hero is far. A killed one stays dead for its respawn time, and is worth more than the horde. Heroes' spells and items only seek engaged enemies. Unengaged creatures stand still in this task (Task 6 makes them notice and walk).

**Files:**
- Create: `crates/together-sim/src/world/wildlife.rs`
- Modify: `crates/together-sim/src/world.rs`
- Modify: `crates/together-sim/src/world/arena.rs` (Detonation)
- Modify: `crates/together-sim/src/world/extraction.rs` (horde count, recycle, `pub(super) mod tests`)
- Modify: `crates/benilla-app/src/together/view.rs` (a no-op arm)

**Interfaces:**
- Consumes (Tasks 3–4): `ExtractionDef::{spawns, wildlife, hot_zone_at, in_margin}`, `SpawnDef::at`.
- Produces:
  - `Enemy::engaged: bool` (pub) and `Enemy::is_wild() -> bool`.
  - `pub(crate) struct WildLink { slot, path_at, goal: Option<V2>, quiet: f32, returning: bool }` in `world::wildlife`; `Enemy::wild: Option<WildLink>` (`pub(crate)`, `#[serde(skip)]`).
  - `World::wild: Vec<Wild>` (private; `Wild { spawn: usize, enemy: Option<u32>, dead_until: f32 }`).
  - `World::keep_wildlife`, `World::tend_wildlife`, `World::wild_died(Option<usize>) -> f32`.
  - `Event::Engaged { id: u32 }`.

- [ ] **Step 1: Failing tests**

In `crates/together-sim/src/world/extraction.rs`, make the test module `pub(super) mod tests` (it is `mod tests` now), and its helpers `run`, `hush`, `at`, `secs`, `state` and `state_mut` `pub(crate) fn` instead of `pub(super) fn`, so the tests of `world::wildlife` (a sibling module) can use them.

Create `crates/together-sim/src/world/wildlife.rs` with only tests for now:

```rust
//! Extraction mode's wildlife ([`crate::extraction::WildlifeDef`]): a map's creatures, kept for a
//! run by its seed, waking near living heroes and sleeping when they are gone; noticing heroes
//! who come close, with their kind; giving up and walking home when left alone; and walking
//! their patrols meanwhile. A killed one stays dead for its respawn time. The state is the
//! host's alone: the creatures travel to co-op clients as ordinary enemies.

use super::*;
use crate::extraction::ExtractionDef;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::tests::test_content;
    use crate::extraction::HotZoneDef;
    use crate::world::extraction::tests::{at, secs};

    /// Wolves at (150, 0), (155, 5) and (180, 0); a neutral boar at (0, 200); kobolds that
    /// call for help at (0, -150) and (0, -175); a wolf patrolling from (-200, 0) to (-200, 50).
    const SPAWNS: &str = r#"[
        (map: "goldshire", spawns: [
            (enemy: "wolf", pos: (150.0, 0.0), respawn: 30.0),
            (enemy: "wolf", pos: (155.0, 5.0), respawn: 30.0),
            (enemy: "wolf", pos: (180.0, 0.0), respawn: 30.0),
            (enemy: "boar", pos: (0.0, 200.0), respawn: 30.0),
            (enemy: "kobold", pos: (0.0, -150.0), respawn: 30.0),
            (enemy: "kobold", pos: (0.0, -175.0), respawn: 30.0),
            (enemy: "wolf", pos: (-200.0, 0.0), respawn: 30.0,
             path: [(-200.0, 0.0), (-200.0, 50.0)]),
        ]),
    ]"#;

    /// A mage in the test map with every creature kept, the horde hushed, and `edit` made to the
    /// map.
    fn forest(edit: impl FnOnce(&mut ExtractionDef)) -> World {
        let mut c = test_content("").with_spawns(SPAWNS).unwrap();
        c.extractions[0].wildlife.share = 1.0;
        edit(&mut c.extractions[0]);
        let mut w = World::new(Arc::new(c), "goldshire", 3).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.spawn_timer = f32::MAX;
        w
    }

    /// The awake creature nearest `(x, y)`, within 2 yd.
    fn creature(w: &World, x: f32, y: f32) -> Option<&Enemy> {
        let at = V2::new(x, y);
        w.enemies
            .iter()
            .filter(|e| e.is_wild() && e.pos.distance(at) < 2.0)
            .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
    }

    #[test]
    fn a_creature_wakes_near_a_hero_and_sleeps_once_they_have_gone() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        assert!(creature(&w, 150.0, 0.0).is_some());
        assert!(creature(&w, -200.0, 0.0).is_none(), "too far to wake");
        at(&mut w, 0, -100.0, 0.0);
        secs(&mut w, 0.5);
        assert!(creature(&w, 150.0, 0.0).is_none(), "asleep again");
        assert!(creature(&w, -200.0, 0.0).is_some());
    }

    #[test]
    fn heroes_spells_never_seek_an_idle_creature() {
        let mut w = forest(|_| {});
        // 22 yd from the wolf at (150, 0): in Frostbolt's reach, out of its notice.
        at(&mut w, 0, 128.0, 0.0);
        secs(&mut w, 2.0);
        let wolf = creature(&w, 150.0, 0.0).unwrap();
        assert!(!wolf.engaged);
        assert_eq!(wolf.hp, wolf.max_hp);
    }

    #[test]
    fn a_hurt_creature_engages() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.damage_enemy(0, 0, id, 1.0, false);
        assert!(w.enemy(id).unwrap().engaged);
        assert!(w.drain_events().contains(&Event::Engaged { id }));
    }

    #[test]
    fn a_killed_creature_stays_dead_for_its_respawn_time() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.step();
        secs(&mut w, 25.0);
        assert!(creature(&w, 150.0, 0.0).is_none(), "still dead");
        secs(&mut w, 6.0);
        assert!(creature(&w, 150.0, 0.0).is_some(), "back after 30 s");
    }

    #[test]
    fn a_creature_of_the_forest_is_worth_more_than_the_horde() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 0.0;
        w.drain_events();
        w.step();
        let xp = w.drain_events().into_iter().find_map(|e| match e {
            Event::GemDropped { xp, .. } => Some(xp),
            _ => None,
        });
        assert_eq!(xp, Some(2), "a wolf's 1, times the wildlife's reward of 2");
    }

    #[test]
    fn no_more_than_the_cap_are_awake() {
        let mut w = forest(|d| d.wildlife.max_awake = 1);
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        assert_eq!(w.enemies.iter().filter(|e| e.is_wild()).count(), 1);
    }

    #[test]
    fn a_share_of_the_forest_is_kept_but_all_of_a_hot_zone() {
        let w = forest(|d| {
            d.wildlife.share = 0.0;
            d.hot_zones.push(HotZoneDef {
                name: "Den".into(),
                center: [160.0, 0.0],
                radius: 30.0,
            });
        });
        let kept: Vec<usize> = w.wild.iter().map(|x| x.spawn).collect();
        assert_eq!(kept, vec![0, 1, 2]);
    }

    #[test]
    fn the_horde_recycle_leaves_the_forest_to_sleep() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        // 120 to 150 yd off: past the horde's recycle reach, inside waking reach.
        at(&mut w, 0, 30.0, 0.0);
        w.step();
        assert!(creature(&w, 155.0, 5.0).is_some());
        assert!(creature(&w, 180.0, 0.0).is_some());
    }

    #[test]
    fn an_arena_enemy_is_always_engaged() {
        let mut w = World::new(Arc::new(crate::content::tests::sample()), "elwynn", 1).unwrap();
        w.add_hero(0, "mage").unwrap();
        secs(&mut w, 2.0);
        assert!(!w.enemies.is_empty());
        assert!(w.enemies.iter().all(|e| e.engaged && !e.is_wild()));
    }
}
```

In `world.rs`, add `mod wildlife;` after `mod extraction;`.

Run: `cargo test -p together-sim 2>&1 | grep -E "^error" | sort | uniq -c`
Expected: compile errors (`is_wild`, `engaged`, `wild`, `Engaged`, `damage_enemy` is private to `world`, which `world::wildlife::tests` can reach).

- [ ] **Step 2: Enemy and world fields, events**

In `world.rs`:
1. `use crate::extraction::{Bounds, Extraction, Mode};` stays. Add `use wildlife::{Wild, WildLink};` after the `mod wildlife;` line.
2. In `struct Enemy`, after `pub boss: bool,`:

```rust
    /// Chasing the heroes: the horde always is; a creature of the map's wildlife once it notices
    /// a hero or is hurt. Heroes' spells only seek engaged enemies.
    pub engaged: bool,
    /// The map's creature this is, while it is one (extraction): the host's alone.
    #[serde(skip)]
    pub(crate) wild: Option<WildLink>,
```

3. Add:

```rust
impl Enemy {
    /// One of the map's creatures, not the horde.
    pub fn is_wild(&self) -> bool {
        self.wild.is_some()
    }
}
```

4. Every `Enemy { … }` literal (in `spawn_enemy`, `apply_snapshot` and the tests' `dummy`) gets `engaged: true,` and `wild: None,`.
5. In `struct World`, after `mode`:

```rust
    /// The map's creatures kept for this run (extraction), asleep or awake: the host's alone.
    wild: Vec<Wild>,
```

   and in `World::new`'s `Self { … }`, `wild: Vec::new(),`. After the scatter loop in `World::new`, add `world.keep_wildlife();`.
6. In `Event`, after `AmbushWarned`:

```rust
    /// Extraction: creature `id` of the map's wildlife noticed a hero (or was hurt) and gives
    /// chase.
    Engaged {
        id: u32,
    },
```

- [ ] **Step 3: The targeting rule**

In `world.rs`:
- `nearest`: add `.filter(|e| e.engaged)` before `.map(|e| (e.id, from.distance(e.pos)))`.
- `cast`, `AbilityKind::Nova`: change `.filter(|e| from.distance(e.pos) <= stats.radius)` to `.filter(|e| e.engaged && from.distance(e.pos) <= stats.radius)`.
- `on_kill` (Ember Core): change `.filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= EMBER_RADIUS)` to `.filter(|e| e.engaged && e.hp > 0.0 && e.pos.distance(pos) <= EMBER_RADIUS)`.

In `world/arena.rs` (Detonation): change `.filter(|e| e.hp > 0.0 && e.pos.distance(at) <= DETONATION_RADIUS)` to `.filter(|e| e.engaged && e.hp > 0.0 && e.pos.distance(at) <= DETONATION_RADIUS)`.

In `damage_enemy`, after `e.last_hit = Some(by);`:

```rust
        // A creature of the forest that is hurt gives chase.
        let woke = !e.engaged;
        e.engaged = true;
        if let Some(link) = &mut e.wild {
            link.quiet = 0.0;
            link.returning = false;
        }
```

and at the end of `damage_enemy`, after the `Damage` event push:

```rust
        if woke {
            self.events.push(Event::Engaged { id });
        }
```

In `move_enemies`, at the top of the `for (i, e) in self.enemies.iter_mut().enumerate() {` loop body:

```rust
            // A creature that has not noticed anyone stands (Task 6 walks it).
            if !e.engaged {
                e.moving = false;
                e.pos += shove[i] * (DT * 8.0).min(0.5);
                continue;
            }
```

- [ ] **Step 4: Keeping, waking, sleeping, dying**

Above `#[cfg(test)]` in `world/wildlife.rs`:

```rust
/// Steps between wake-and-sleep passes.
const WAKE_EVERY: u64 = 15;
/// Yards past `wake` every living hero must be before an idle creature sleeps again.
const SLEEP_MARGIN: f32 = 20.0;

/// One of the map's creatures kept for a run.
#[derive(Clone, Debug)]
pub(super) struct Wild {
    /// Index into the map's [`ExtractionDef::spawns`].
    pub spawn: usize,
    /// The enemy it is while awake.
    pub enemy: Option<u32>,
    /// Seconds into the run it may wake again after dying.
    pub dead_until: f32,
}

/// What an awake creature knows of itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WildLink {
    /// Index into [`World::wild`].
    pub slot: usize,
    /// The patrol point it walks toward.
    pub path_at: usize,
    /// Where a wanderer is walking.
    pub goal: Option<V2>,
    /// Seconds engaged without hurting or being hurt.
    pub quiet: f32,
    /// Gave up, walking home.
    pub returning: bool,
}

impl World {
    /// The run's share of the map's creatures, chosen by the run's seed: every one in a hot
    /// zone, none in the safe ring's margin.
    pub(super) fn keep_wildlife(&mut self) {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        for (i, s) in def.spawns.iter().enumerate() {
            let at = s.at();
            if def.in_margin(at) {
                continue;
            }
            let hot = def.hot_zone_at(at).is_some();
            if hot || self.rng.f32() < def.wildlife.share {
                self.wild.push(Wild {
                    spawn: i,
                    enemy: None,
                    dead_until: 0.0,
                });
            }
        }
    }

    /// Each step's wildlife upkeep. Nothing in the arena.
    pub(super) fn tend_wildlife(&mut self) {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return;
        };
        if self.tick % WAKE_EVERY == 0 {
            self.wake_and_sleep(def);
        }
    }

    /// Idle creatures with every living hero far off sleep; sleeping ones near a living hero,
    /// and not dead, wake at their spawn, up to the map's cap.
    fn wake_and_sleep(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).collect();
        if living.is_empty() {
            return;
        }
        let wake = def.wildlife.wake;
        let mut sleepers = Vec::new();
        for (slot, w) in self.wild.iter().enumerate() {
            let Some(id) = w.enemy else {
                continue;
            };
            match self.enemy(id) {
                Some(e) if e.engaged => {}
                Some(e) if living.iter().any(|p| p.distance(e.pos) <= wake + SLEEP_MARGIN) => {}
                _ => sleepers.push((slot, id)),
            }
        }
        for (slot, id) in sleepers {
            self.enemies.retain(|e| e.id != id);
            self.wild[slot].enemy = None;
        }
        let cap = def.wildlife.max_awake as usize * living.len();
        let mut awake = self.wild.iter().filter(|w| w.enemy.is_some()).count();
        for slot in 0..self.wild.len() {
            if awake >= cap {
                break;
            }
            let w = &self.wild[slot];
            if w.enemy.is_some() || self.elapsed < w.dead_until {
                continue;
            }
            let at = def.spawns[w.spawn].at();
            if living.iter().any(|p| p.distance(at) <= wake) {
                self.spawn_wild(def, slot);
                awake += 1;
            }
        }
    }

    /// Creature `slot` awake at its spawn, tougher than the horde, not yet engaged.
    fn spawn_wild(&mut self, def: &ExtractionDef, slot: usize) {
        let spawn = &def.spawns[self.wild[slot].spawn];
        let at = spawn.at();
        let id = self.spawn_enemy(spawn.enemy_ix, at, at + V2::new(1.0, 0.0), false);
        self.toughen(id, def.wildlife.toughness, 1.0);
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
            e.engaged = false;
            e.wild = Some(WildLink {
                slot,
                ..WildLink::default()
            });
        }
        self.wild[slot].enemy = Some(id);
    }

    /// The creature in `slot` died (`None`: not a creature of the forest): it stays dead for its
    /// spawn's respawn time. What its kill pays, times.
    pub(super) fn wild_died(&mut self, slot: Option<usize>) -> f32 {
        let Some(slot) = slot else {
            return 1.0;
        };
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return 1.0;
        };
        let respawn = def.spawns[self.wild[slot].spawn].respawn * def.wildlife.respawn;
        let now = self.elapsed;
        let w = &mut self.wild[slot];
        w.enemy = None;
        w.dead_until = now + respawn;
        def.wildlife.reward
    }
}
```

In `world.rs`:
- In `step()`, after `self.tend_extraction();` add `self.tend_wildlife();`.
- In `reap_enemies`, add `e.wild.as_ref().map(|l| l.slot)` as the last element of the tuple pushed to `dead` (and `slot` as the last name in the `for (id, pos, def, elite, boss, max_hp, killer) in dead` pattern). Then change `let paid = self.reward_of(killer);` to:

```rust
            let paid = self.reward_of(killer) * self.wild_died(slot);
```

In `world/extraction.rs`:
- In `spawn_in_extraction`, change `let alive = self.enemies.iter().filter(|e| !e.elite && !e.boss).count();` to `let alive = self.enemies.iter().filter(|e| !e.elite && !e.boss && !e.is_wild()).count();`.
- In `recycle`, change the retain to `.retain(|e| e.elite || e.boss || e.is_wild() || living.iter().any(|p| p.distance(e.pos) <= reach));`.

In `crates/benilla-app/src/together/view.rs`'s `apply_events`, add an arm (Task 8 fills it in):

```rust
            Event::Engaged { .. } => {}
```

- [ ] **Step 5: Tests pass**

Run: `cargo test -p together-sim -p together-net 2>&1 | grep -E "^error|FAILED|test result"`
Expected: all pass, including the nine new wildlife tests and every arena test.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "Sim: the forest's creatures wake near heroes and sleep without them; spells seek engaged enemies

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Noticing, packs, giving up, walking

Idle creatures walk their patrols or wander, notice heroes who come within their range (by level and heat), bring their kind with them, and give up and walk home when left alone.

**Files:**
- Modify: `crates/together-sim/src/world/wildlife.rs`
- Modify: `crates/together-sim/src/world.rs` (`move_enemies`, `Event::CalledHelp`)
- Modify: `crates/benilla-app/src/together/view.rs` (a no-op arm)

**Interfaces:**
- Consumes (Task 5): `Wild`, `WildLink`, `World::{wild, tend_wildlife}`, `Enemy::{engaged, wild}`; `World::{party_heat, add_heat}` (private to `world`).
- Produces: `World::{notice, engage, give_up, idle_goals}`; `Event::CalledHelp { id: u32 }`.

- [ ] **Step 1: Failing tests**

Change the tests' helper import to `use crate::world::extraction::tests::{at, secs, state_mut};`, then append inside `mod tests` in `world/wildlife.rs` (`PlayerInput` comes in through `use super::*`):

```rust
    #[test]
    fn a_creature_notices_a_hero_who_comes_close_and_its_pack_comes_too() {
        let mut w = forest(|d| d.heat.notice = 2.0);
        at(&mut w, 0, 138.0, 0.0);
        secs(&mut w, 0.5);
        assert!(w.enemies.iter().all(|e| !e.engaged), "12 yd: out of notice");
        let heat = w.heat_of(0);
        at(&mut w, 0, 142.0, 0.0);
        secs(&mut w, 0.2);
        let engaged: Vec<V2> = w.enemies.iter().filter(|e| e.engaged).map(|e| e.pos).collect();
        assert_eq!(engaged.len(), 2, "the wolf and the one beside it: {engaged:?}");
        assert!(w.heat_of(0) >= heat + 4.0 - 0.01, "two noticed: {}", w.heat_of(0));
    }

    #[test]
    fn a_kobold_calls_its_kin_from_further_off() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 0.0, -142.0);
        secs(&mut w, 0.5);
        assert_eq!(w.enemies.iter().filter(|e| e.engaged).count(), 2);
        assert!(w
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::CalledHelp { .. })));
    }

    #[test]
    fn a_neutral_creature_never_notices_but_fights_back() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 0.0, 195.0);
        secs(&mut w, 2.0);
        let boar = creature(&w, 0.0, 200.0).unwrap();
        assert!(!boar.engaged);
        let id = boar.id;
        w.damage_enemy(0, 0, id, 1.0, false);
        assert!(w.enemy(id).unwrap().engaged);
    }

    #[test]
    fn heat_and_level_change_how_far_a_creature_notices() {
        // At 5 skulls a level-1 wolf notices from 10 × 1.75 = 17.5 yd.
        let mut w = forest(|_| {});
        at(&mut w, 0, 135.0, 0.0);
        state_mut(&mut w).heat = 1000.0;
        secs(&mut w, 0.5);
        assert!(w.enemies.iter().any(|e| e.engaged && e.is_wild()));
        // A party nine levels above it: 10 − 9 = 1, held at the minimum of 5 yd.
        let mut w = forest(|_| {});
        w.level = 10;
        at(&mut w, 0, 143.0, 0.0);
        secs(&mut w, 0.5);
        assert!(!w.enemies.iter().any(|e| e.engaged && e.is_wild()));
    }

    #[test]
    fn an_engaged_creature_never_sleeps() {
        let mut w = forest(|_| {});
        at(&mut w, 0, 100.0, 0.0);
        secs(&mut w, 0.5);
        let id = creature(&w, 150.0, 0.0).unwrap().id;
        w.damage_enemy(0, 0, id, 1.0, false);
        w.heroes[0].abilities.clear();
        at(&mut w, 0, -250.0, 0.0);
        secs(&mut w, 1.0);
        assert!(w.enemy(id).is_some_and(|e| e.engaged));
    }

    #[test]
    fn a_creature_left_behind_gives_up_walks_home_and_heals() {
        let mut w = forest(|_| {});
        w.heroes[0].abilities.clear();
        at(&mut w, 0, 142.0, 0.0);
        secs(&mut w, 0.2);
        let id = creature(&w, 150.0, 0.0).filter(|e| e.engaged).expect("noticed").id;
        w.enemies.iter_mut().find(|e| e.id == id).unwrap().hp = 1.0;
        // The hero runs off faster than a wolf can follow.
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                ..Default::default()
            },
        );
        secs(&mut w, 40.0);
        let wolf = w.enemy(id).expect("awake still");
        assert!(!wolf.engaged);
        assert!(wolf.pos.distance(V2::new(150.0, 0.0)) < 2.0, "{:?}", wolf.pos);
        assert_eq!(wolf.hp, wolf.max_hp);
    }

    #[test]
    fn a_patrolling_creature_walks_its_path() {
        let mut w = forest(|_| {});
        at(&mut w, 0, -150.0, 60.0);
        secs(&mut w, 0.5);
        let start = creature(&w, -200.0, 0.0).unwrap().pos;
        secs(&mut w, 8.0);
        let now = w.enemies.iter().find(|e| e.is_wild()).unwrap().pos;
        assert!(now.y > start.y + 5.0, "{start:?} to {now:?}");
    }
```

Run: `cargo test -p together-sim world::wildlife 2>&1 | grep -E "^error|FAILED$|test result" | head`
Expected: a compile error for `CalledHelp`. With that error alone, also expect the noticing, kobold, heat-and-level, give-up and patrol tests to fail once it compiles.

- [ ] **Step 2: The event**

In `world.rs`'s `Event`, after `Engaged`:

```rust
    /// Extraction: creature `id` shouted for help, bringing its kind from further off.
    CalledHelp {
        id: u32,
    },
```

In `view.rs`'s `apply_events`, add `Event::CalledHelp { .. } => {}` next to the `Engaged` arm.

- [ ] **Step 3: Noticing, packs, giving up**

In `world/wildlife.rs`, after `const SLEEP_MARGIN`:

```rust
/// Steps between notice checks.
const NOTICE_EVERY: u64 = 3;
/// How near a walking creature must come to a point to have reached it.
const ARRIVE: f32 = 1.5;
```

In `tend_wildlife`, after the `wake_and_sleep` call:

```rust
        if self.tick % NOTICE_EVERY == 0 {
            self.notice(def);
        }
        self.give_up(def);
```

Add to `impl World`:

```rust
    /// Awake creatures that are not chasing notice a living hero within their range: the map's
    /// `notice`, more for creatures above the party's level and less below (never under
    /// `notice_min`), and more for each skull of the party's heat. Neutral kinds never notice.
    fn notice(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).collect();
        if living.is_empty() {
            return;
        }
        let wl = &def.wildlife;
        let skulls = def.skull_index(self.party_heat()) as f32;
        let sharp = 1.0 + wl.notice_per_skull * skulls;
        let party = self.level as f32;
        let content = self.content.clone();
        let seen: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| {
                let Some(link) = &e.wild else {
                    return false;
                };
                let kind = &content.enemies[e.def];
                if e.engaged || link.returning || kind.neutral {
                    return false;
                }
                let range = (wl.notice + (kind.level as f32 - party) * wl.notice_per_level)
                    .max(wl.notice_min)
                    * sharp;
                living.iter().any(|p| p.distance(e.pos) <= range)
            })
            .map(|e| e.id)
            .collect();
        for id in seen {
            self.engage(def, id);
        }
    }

    /// Creature `id` gives chase, and with it every idle creature of its kind within `social`,
    /// or within `help` when its kind calls for help (it shouts). Each adds the map's notice heat.
    fn engage(&mut self, def: &ExtractionDef, id: u32) {
        let Some(e) = self.enemy(id).filter(|e| !e.engaged) else {
            return;
        };
        let (kind, at) = (e.def, e.pos);
        let calls = self.content.enemies[kind].calls_help;
        let reach = if calls {
            def.wildlife.help.max(def.wildlife.social)
        } else {
            def.wildlife.social
        };
        let pack: Vec<u32> = self
            .enemies
            .iter()
            .filter(|o| {
                o.id == id
                    || (o.def == kind
                        && !o.engaged
                        && o.wild.as_ref().is_some_and(|l| !l.returning)
                        && o.pos.distance(at) <= reach)
            })
            .map(|o| o.id)
            .collect();
        if calls {
            self.events.push(Event::CalledHelp { id });
        }
        for m in pack {
            if let Some(o) = self.enemies.iter_mut().find(|o| o.id == m) {
                o.engaged = true;
                if let Some(l) = &mut o.wild {
                    l.quiet = 0.0;
                }
            }
            self.events.push(Event::Engaged { id: m });
            self.add_heat(None, def.heat.notice);
        }
    }

    /// An engaged creature that nobody has hurt, and that has hurt nobody, for `give_up`
    /// seconds, with every living hero further than `leash`, gives up and walks home.
    fn give_up(&mut self, def: &ExtractionDef) {
        let living: Vec<V2> = self.heroes.iter().filter(|h| h.alive).map(|h| h.pos).collect();
        let wl = def.wildlife;
        for e in &mut self.enemies {
            if !e.engaged {
                continue;
            }
            let Some(l) = &mut e.wild else {
                continue;
            };
            l.quiet += DT;
            if l.quiet >= wl.give_up && living.iter().all(|p| p.distance(e.pos) > wl.leash) {
                l.returning = true;
                l.quiet = 0.0;
                l.goal = None;
                e.engaged = false;
            }
        }
    }

    /// Where each enemy that is not chasing walks this step, in [`World::enemies`]' order:
    /// home for one that gave up (healed on arrival), a patrol's next point, or a wanderer's
    /// spot; `None` stands. Empty in the arena.
    pub(super) fn idle_goals(&mut self) -> Vec<Option<V2>> {
        let content = self.content.clone();
        let Some(def) = self.map(&content) else {
            return Vec::new();
        };
        let mut goals = Vec::with_capacity(self.enemies.len());
        for i in 0..self.enemies.len() {
            let e = &self.enemies[i];
            let Some(link) = e.wild.clone().filter(|_| !e.engaged) else {
                goals.push(None);
                continue;
            };
            let spawn = &def.spawns[self.wild[link.slot].spawn];
            let (home, pos) = (spawn.at(), e.pos);
            let goal = if link.returning {
                if pos.distance(home) <= ARRIVE {
                    let e = &mut self.enemies[i];
                    e.hp = e.max_hp;
                    if let Some(l) = &mut e.wild {
                        l.returning = false;
                    }
                    None
                } else {
                    Some(home)
                }
            } else if !spawn.path.is_empty() {
                let point = |k: usize| V2::new(spawn.path[k][0], spawn.path[k][1]);
                let mut next = link.path_at % spawn.path.len();
                if pos.distance(point(next)) <= ARRIVE {
                    next = (next + 1) % spawn.path.len();
                }
                if let Some(l) = &mut self.enemies[i].wild {
                    l.path_at = next;
                }
                Some(point(next))
            } else if spawn.wander > 0.0 {
                match link.goal {
                    Some(g) if pos.distance(g) > ARRIVE => Some(g),
                    _ => {
                        let angle = self.rng.range(0.0, std::f32::consts::TAU);
                        let g = home + V2::from_angle(angle, self.rng.range(0.0, spawn.wander));
                        if let Some(l) = &mut self.enemies[i].wild {
                            l.goal = Some(g);
                        }
                        Some(g)
                    }
                }
            } else {
                None
            };
            goals.push(goal);
        }
        goals
    }
```

- [ ] **Step 4: Walking**

In `world.rs`'s `move_enemies`:
1. Just before `let mut swings = Vec::new();`, add:

```rust
        let idle = self.idle_goals();
        let walk = self
            .map(&self.content)
            .map_or(1.0, |d| d.wildlife.walk);
```

2. Replace the Task 5 block at the top of the loop body (`// A creature that has not noticed anyone stands …`) with:

```rust
            // A creature that is not chasing walks its patrol, wanders, or walks home.
            if !e.engaged {
                let returning = e.wild.as_ref().is_some_and(|l| l.returning);
                let pace = e.speed * if returning { 1.0 } else { walk } * DT;
                let step = idle.get(i).copied().flatten().map_or(V2::ZERO, |goal| {
                    let to = goal - e.pos;
                    let len = to.length();
                    if len <= 1e-3 {
                        V2::ZERO
                    } else {
                        to * (pace.min(len) / len)
                    }
                });
                e.moving = step != V2::ZERO;
                if e.moving {
                    e.facing = step.normalize_or_zero();
                }
                e.pos += step + shove[i] * (DT * 8.0).min(0.5);
                continue;
            }
```

3. Where a swing is pushed (`swings.push((e.id, player, e.damage));`), add before it:

```rust
                if let Some(l) = &mut e.wild {
                    l.quiet = 0.0;
                }
```

`self.map(&self.content)` borrows `self` immutably only for that statement; `idle_goals` runs before the loop's mutable borrow.

- [ ] **Step 5: Tests pass**

Run: `cargo test -p together-sim -p together-net 2>&1 | grep -E "^error|FAILED|test result"`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "Sim: creatures notice heroes by level and heat, bring their kind, give up and walk home, patrol

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Hot zones heat up; the horde at its share

**Files:**
- Modify: `crates/together-sim/src/world/extraction.rs` (`tend_heat`, `spawn_in_extraction`, tests)

**Interfaces:**
- Consumes (Task 3): `HeatDef::hot_zone`, `ExtractionDef::{hot_zone_at, horde_share}`.

- [ ] **Step 1: Failing tests**

In `world/extraction.rs` tests (add `use crate::extraction::HotZoneDef;` inside the module):

```rust
    #[test]
    fn heat_climbs_faster_in_a_hot_zone() {
        let mut c = test_content("");
        c.extractions[0].heat.hot_zone = 3.0;
        c.extractions[0].hot_zones.push(HotZoneDef {
            name: "Den".into(),
            center: [150.0, 0.0],
            radius: 30.0,
        });
        let mut w = World::new(Arc::new(c), "goldshire", 1).unwrap();
        w.add_hero(0, "mage").unwrap();
        hush(&mut w);
        at(&mut w, 0, 150.0, 0.0);
        secs(&mut w, 2.0);
        // 1 a second outside and 3 in the hot zone.
        assert!((w.heat_of(0) - 8.0).abs() < 0.1, "{}", w.heat_of(0));
    }

    #[test]
    fn the_horde_share_shrinks_the_horde() {
        let horde = |share: f32| {
            let mut c = test_content("");
            c.extractions[0].horde_share = share;
            let mut w = World::new(Arc::new(c), "goldshire", 4).unwrap();
            w.add_hero(0, "mage").unwrap();
            at(&mut w, 0, 200.0, 0.0);
            w.elapsed = 300.0;
            secs(&mut w, 3.0);
            w.enemies.len()
        };
        let (full, half) = (horde(1.0), horde(0.5));
        assert!(half * 10 < full * 7, "{half} of {full}");
    }
```

Run: `cargo test -p together-sim 2>&1 | grep -E "FAILED$" | head`
Expected: both new tests fail.

- [ ] **Step 2: Implement**

In `tend_heat`, replace the `let living: Vec<(PlayerId, bool)> = …;` statement and the shared/per-hero branches with:

```rust
        let living: Vec<(PlayerId, bool, bool)> = self
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| (h.player, def.in_safe(h.pos), def.hot_zone_at(h.pos).is_some()))
            .collect();
        let Some(x) = self.run_mut() else {
            return;
        };
        x.floor = floor;
        let (outside, cool, hot) = (
            def.heat.outside * DT,
            def.safe.cool * DT,
            def.heat.hot_zone * DT,
        );
        if def.settings.heat_shared {
            if living.iter().any(|&(_, home, _)| !home) {
                x.heat += outside;
                if living.iter().any(|&(_, _, in_hot)| in_hot) {
                    x.heat += hot;
                }
            } else if !living.is_empty() {
                x.heat -= cool;
            }
            x.heat = x.heat.max(floor);
        } else {
            for &(p, home, in_hot) in &living {
                let h = &mut hero_heat(x, p).1;
                let next = if home {
                    *h - cool
                } else {
                    *h + outside + if in_hot { hot } else { 0.0 }
                };
                *h = next.max(floor);
            }
            for (_, h) in &mut x.heat_by {
                *h = h.max(floor);
            }
        }
```

(the `x.peak = …` line after it stays).

In `spawn_in_extraction`, change the `target` line to:

```rust
        let target = (target_alive(self.elapsed, outside.len()) as f32
            * busiest
            * horde
            * def.horde_share) as usize;
```

- [ ] **Step 3: Tests pass and commit**

Run: `cargo test -p together-sim -p together-net 2>&1 | grep -E "^error|FAILED|test result"` → all pass.

```bash
git add crates
git commit -m "Sim: heat climbs faster in hot zones; the horde at the map's share

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Client, protocol, and seeing it run

**Files:**
- Modify: `crates/benilla-app/src/together/{mod,view,hud}.rs`
- Modify: `crates/together-net/src/lib.rs`, `crates/together-net/tests/snapshot_size.rs`
- Modify: `crates/together-sim/src/world/wildlife.rs` (the mirror test)
- Modify: `CLAUDE.md`

**Interfaces:**
- Consumes: everything above.
- Produces: `PROTOCOL` 5; the content hash covers `spawns.ron`.

- [ ] **Step 1: A mirror draws the awake forest (test first)**

Append in `world/wildlife.rs` tests:

```rust
    #[test]
    fn a_mirror_draws_the_awake_forest() {
        let mut host = forest(|_| {});
        at(&mut host, 0, 100.0, 0.0);
        secs(&mut host, 0.5);
        let mut mirror = forest(|_| {});
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        let awake = host.enemies.iter().filter(|e| e.is_wild()).count();
        assert!(awake > 0);
        assert_eq!(mirror.enemies.len(), host.enemies.len());
        // What sleeps on the host goes from the mirror too.
        at(&mut host, 0, -100.0, 0.0);
        secs(&mut host, 0.5);
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        assert_eq!(mirror.enemies.len(), host.enemies.len());
    }
```

Run: `cargo test -p together-sim a_mirror_draws 2>&1 | grep "test result"`
Expected: pass. This guards co-op; it passes because the creatures travel as ordinary enemies.

- [ ] **Step 2: Protocol, loading, hashing**

- `crates/together-net/src/lib.rs`: `pub const PROTOCOL: u32 = 5;`.
- `crates/benilla-app/src/together/mod.rs` `prepare()`: after the `extraction` read, add:

```rust
    let spawns = read("spawns.ron", include_str!("../../../../content/spawns.ron"))?;
```

  After `.and_then(|c| c.with_extractions(&extraction))`, add `.and_then(|c| c.with_spawns(&spawns))`. Change the hash text to `format!("{zones}\n{rules}\n{extraction}\n{spawns}")`. Change the `CONTENT_TEXTS` doc's tail to `the zones with the rules, extraction maps and spawns after them.`

- `crates/together-net/tests/snapshot_size.rs`:
  - In `shipped()`, add `.and_then(|c| c.with_spawns(include_str!("../../../content/spawns.ron")))` after its `with_extractions` line.
  - In the extraction test, change the horde count to `let horde = w.enemies.iter().filter(|e| !e.elite && !e.boss && !e.is_wild()).count();`.
  - Change its last `eprintln!` to also print the creatures: `eprintln!("extraction at 5 skulls: {} enemies ({} creatures): {bytes} bytes", w.enemies.len(), w.enemies.iter().filter(|e| e.is_wild()).count());`.

- [ ] **Step 3: Event text and HUD**

`view.rs` `apply_events`, replace the `Event::Engaged { .. } => {}` and `Event::CalledHelp { .. } => {}` arms with:

```rust
            Event::Engaged { id } => {
                if let Some(&e) = views.enemies.get(&id) {
                    fx.text(e, "!".into(), COLOR_HEAT);
                }
            }
            Event::CalledHelp { id } => {
                if let Some(&e) = views.enemies.get(&id) {
                    fx.text(e, "calls for help!".into(), COLOR_HEAT);
                }
            }
```

`hud.rs` `extraction_status`, after the `if def.in_safe(hero.pos) { … }` block:

```rust
    if let Some(zone) = def.hot_zone_at(hero.pos) {
        s.push_str(&format!("\n{}: dense, and the heat climbs faster here.", zone.name));
    }
```

- [ ] **Step 4: Tests and build**

Run: `cargo test -p together-sim -p together-net -- --nocapture 2>&1 | grep -E "^error|FAILED|test result|extraction at 5"`
Expected: all pass. The snapshot line shows the enemy count and bytes, about 15–20 bytes per enemy.

Run: `cargo build --release -p azeroth-together 2>&1 | grep -E "^(error|warning: unused)|Finished"` (background) → `Finished`.

- [ ] **Step 5: See it run**

```bash
mkdir -p shots/home-lm
bash C:/Users/Olivs/AppData/Local/Temp/claude/D--Games-azeroth-together/ced5222e-ac81-40a6-a2c3-afa972f4cede/scratchpad/shot-run.sh lm shots/lm-005.png 400 BENILLA_HOME=D:/Games/azeroth-together-extraction/shots/home-lm TOGETHER_ZONE=elwynn_extraction TOGETHER_AUTOPLAY=1 TOGETHER_SHOWCASE=1 TOGETHER_TRACE_POS=1 WOW_LIVE_SHOT=shots/lm.png WOW_LIVE_SHOT_AT=25 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=20
```

(`shot-run.sh` runs the client in the background window and stops that one process after its last shot. If it is gone, run the client with those variables and stop it by its process id once `shots/lm-005.png` exists.)

Check in the shots and the log:
- creatures standing in the forest around the hero, not chasing until close;
- "!" over creatures that engage;
- the HUD's timetable lines ("flight lands in…, leaves at…", then "boarding");
- a hot zone line if the walk passes one;
- no creature standing on a hilltop above a mine;
- the snapshot of a mirror is covered by Step 1. For a co-op check, run the host and guest pair from the part 1 plan's Task 11, Step 6.

Record what the shots show, and anything wrong, in the ledger.

- [ ] **Step 6: Document and commit**

In `CLAUDE.md`, extend the extraction bullet with: ` Elwynn's creatures are \`content/spawns.ron\`, baked by \`tools/spawns/bake_spawns.py\` from the CMaNGOS Classic DB (GPL-3.0; see \`tools/spawns/README.md\`).`

```bash
git add crates CLAUDE.md
git commit -m "Client: the living map's creatures, '!' and calls for help, timetable and hot zone HUD; protocol 5

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Not in this plan

- The Stalker (spec section 6), and the tuning pass with new dev-panel rows for wildlife share and notice range (spec build step 7): the next plan, after playtesting.
- Grey minimap dots for idle creatures (spec section 5). The engine's minimap colours dots by tracking, not by aggression, so this needs engine work; the "!" text marks engagement instead.
- Elites in red circles, hold circles, rares and the merchant from the part 1 spec: still to come.
- Ambush packs drawn from the nearest hot zone's creatures (spec section 2): this plan sends the strongest horde kind, like hunters; the creature-specific version comes with the tuning pass.
