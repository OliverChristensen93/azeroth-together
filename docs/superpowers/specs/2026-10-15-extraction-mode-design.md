# Extraction mode: design

Date: 2026-10-15. Status: draft for review.

A second way to play Azeroth Together, beside today's arena. The whole of Elwynn Forest is open;
Goldshire is a safe ring where no enemies spawn. Out in the forest a **heat** level climbs with
time, kills and risk, making the horde stronger and the rewards bigger. Elites in red circles, hold
circles and roaming rares sit at points of interest far from Goldshire. A run ends when every hero
has **extracted** by gryphon or is down. The aim is tension and close calls: "one more elite, or
run for the gryphon now?"

This spec covers **project 1, the in-run loop**. Meta progression (project 2) is out of scope;
see [Later](#later).

## 1. Decisions

| Topic | Decision |
|---|---|
| Rewards inside a run | XP (levels, cards) and gold, both multiplied by heat. |
| Rewards between runs | Meta-XP and random-stat meta gear: **project 2, not here.** |
| Leaving | Extract any time an extraction point is open; points open on a timer, at different times. |
| Pressure | Heat rises with time outside, kills, time in red circles and objectives; a nightfall floor rises with time. No hard end. |
| Safe ring | No new spawns inside it; heat cools there. Chasers can follow you in. No guards. |
| Extraction in co-op | One at a time: the gryphon takes whoever is in its ring when it lands. |
| Run power | The `megabonk-systems` branch's systems, unchanged except the fixes in §2. No separate jewelry system. |
| Undecided rules | Built as settings in content, so they can be playtested both ways. |

## 2. Base and dependencies

- Branch `extraction-mode`, started from the `megabonk-systems` branch
  (`megabonk` remote, `OliverChristensen93/azeroth-together-megabonk`).
- The Megabonk session has been asked for these fixes, which this mode relies on:
  - tome slots (start 4) and a tome max rank (start 5); maxed tomes are not offered;
  - a max number of copies per item (start 5); maxed items are not offered;
  - a fallback card (gold or a heal) when nothing is left to offer;
  - those numbers as content settings, not Rust consts.
- `megabonk-systems` branches from `00037fc` and lacks `4ca19bc` (join by invite code). Merge
  `phase-1-sim-core` into `extraction-mode` before building, so co-op over the internet keeps working.
- Reused from `megabonk-systems` as they are: gold and purses, the hero stat block, tomes, pacts,
  items, chests (price rising per opener), caches, charge circles (fill inside, drain outside) and
  their rune ring, shrines appearing near heroes, the War Banner, pots, power-ups, random elites with
  WoW's elite rank, the boss enemy `hogger`, and autoplay walking to interactables.

## 3. The run

### 3.1 Map

- Zone: Elwynn Forest (`map: 0`). Its terrain is in the trimmed data, with the edges of Westfall,
  Duskwood and Stormwind (tiles `[29..36] × [46..51]`, checked with `mpqx`).
- The world's centre is Goldshire. Snapshots pack enemy positions as `i16` sixteenths of a yard from
  the centre, ±2048 yd, which covers Elwynn.
- A boundary polygon (WoW x, y points) around the forest replaces the arena's leash. Heroes are
  clamped to it; nothing spawns outside it.

### 3.2 Safe ring (Goldshire)

- A green ring, radius 40 yd, around Goldshire's crossroads.
- No enemy spawns within `safe_radius + spawn_margin` (40 + 50 yd) of its centre.
- Enemies already chasing can enter. Heroes fight them off; nothing new comes.
- Heat cools at `cool_rate` while the rule in §3.3 allows.
- The **merchant** stands here (re-added for this mode only; `megabonk-systems` removed it from
  the arena): a hand of 3 items for gold, at the old merchant prices, restocked every 4 minutes.
  Going home to shop costs heat, which is the point.

### 3.3 Heat

Heat is a number of points; thresholds turn it into **skulls**, 0 to 5. The HUD shows skulls,
progress to the next, and the **reward multiplier**.

Gains (points):

| Source | Gain |
|---|---|
| Each second outside the safe ring | +1 |
| Each kill | +0.5 |
| Each second in a red circle (elite or hold), on top | +3 |
| Elite killed | +80 |
| Rare killed | +120 |
| Hold circle completed | +150 |
| Each second a gryphon is on its way | +4 |

Cooling: −15 points a second, never below the floor.
- `heat_shared: true` (default): one party heat. Gains from any hero add to it; it cools only while
  **every living hero** is inside the safe ring.
- `heat_shared: false`: each hero has their own heat. A hero's gains and cooling are their own;
  spawns around a hero use that hero's skulls. A kill heats its killer; time outside, in a circle
  or waiting for a gryphon heats the hero doing it; an elite, rare or hold completion heats every
  hero within 60 yd of it.

**Nightfall floor:** from 3:00 the floor rises 40 points a minute (5 skulls of floor at about 28
minutes). Cooling never goes below it.

Skulls:

| Skulls | Points | Spawn rate | Enemy tier | Enemy health and damage | Hunters | Rewards |
|---|---|---|---|---|---|---|
| 0 | 0 | ×1.0 | +0 | ×1.0 | — | ×1.0 |
| 1 | 100 | ×1.3 | +1 | ×1.15 | — | ×1.5 |
| 2 | 250 | ×1.6 | +1 | ×1.3 | — | ×2.0 |
| 3 | 450 | ×2.0 | +2 | ×1.5 | every 20 s | ×2.75 |
| 4 | 700 | ×2.5 | +3 | ×1.75 | every 15 s | ×3.5 |
| 5 | 1000 | ×3.0 | +4 | ×2.0 | every 10 s | ×5.0 |

"Rewards" multiplies XP and gold from kills, and raises item and tome rarity like luck does.

**Hunters:** a pack of 6 to 10 of the strongest current enemy type, 20% faster, spawned 50 to 70 yd
from a hero who is outside the safe ring, within 30° of the line from that hero to the safe ring:
they cut you off from home.

**`lose_them`** (default `false`): when no enemy is within 40 yd of any living hero for 10
seconds, heat drops one skull's worth, never below the floor.

### 3.4 Elite circles

- A fixed elite stands at a point of interest inside a red circle (radius 25 yd), shown on the
  minimap and world map.
- Candidates: Hogger (Forest's Edge), Princess (Brackwell Pumpkin Patch), Goldtooth (Fargodeep Mine).
- The elite fights only heroes inside its circle. If none is inside for 3 seconds it **evades**:
  shows "Evade", walks home ignoring everyone, and heals to full on arrival. Then it waits again.
- Killed: gold, a cache (pick 1 of 3 items), heat. It stays dead for the run.

### 3.5 Hold circles

- At Stonecairn Lake (murlocs) and Jasperlode Mine (kobolds); a red circle of radius 20 yd.
- Progress fills while any hero is inside and **drains while none is** (both 1 point a second, out of
  90). Leaving too long loses it all.
- While it fills, waves of the location's enemies come every 15 seconds; at 100% a final wave comes.
  Killing the final wave completes it: a cache, gold, heat.
- Built on `megabonk-systems`' charge circle, with waves added.

### 3.6 Roaming rares

- Mother Fang, Gruff Swiftbite, Fedfennel, Thuros Lightfingers and Narg the Taskmaster.
- The first appears between 4 and 8 minutes, then one every 6 to 10 minutes, at one of its spawn
  spots, and walks between them.
- It shows on the minimap only once a hero is within 60 yd ("Mother Fang spotted!").
- Killed: more gold than an elite, a cache with better rarity, heat.

### 3.7 Chests, shrines and the rest

- Chests stand at fixed spots across the map (points of interest and the roads between them).
- Shrines, pots, power-ups and the War Banner work as on `megabonk-systems` (appearing near
  heroes), except never inside the safe ring's spawn margin.
- Random elites keep spawning as on `megabonk-systems`.
- The timed zone boss is off in this mode (`timed_boss: false`): the elites and rares replace it.

### 3.8 Extraction

- Points: Stormwind's gate (opens at 8:00) and the Eastvale Logging Camp (opens at 14:00). A gold
  ring, dim until open; "Stormwind Gate is open."
- Any living hero in an open point's ring can call the gryphon. It lands **45 seconds** later; heat
  gains +4 a second meanwhile.
- When it lands it takes every living hero inside its ring (`solo_extract: true`, default). With
  `solo_extract: false` it waits until every living hero is inside.
- The point can be called again 30 seconds after a gryphon leaves.
- An extracted hero leaves the world; enemies forget them; their camera follows a living teammate.
- Fallen heroes left behind when the last living hero extracts are lost.

### 3.9 End of the run

The run ends when every hero is extracted or down. The end screen shows each hero's outcome
(extracted or fell), time, peak heat, kills, gold, objectives completed and the damage meter.

The card screen still pauses the run for everyone (an existing rule); timers pause with it.

## 4. Settings and tuning

All numbers in §3 live in `content/extraction.ron`, which the game reads at startup, so they change
without a rebuild. The settings:

| Setting | Default | What it does |
|---|---|---|
| `heat_shared` | `true` | One party heat, or one per hero |
| `lose_them` | `false` | Escaping all enemies drops a skull |
| `solo_extract` | `true` | The gryphon takes whoever is in its ring, or waits for everyone |
| `timed_boss` | `false` | The zone boss on its timer, as in the arena |
| `reaper_at` | none | Seconds into the run when an unbeatable pursuer comes, ending runs like Vampire Survivors' Death |

## 5. The sim (`together-sim`)

### 5.1 The `Mode` seam

`World` gets `mode: Mode`, an enum: `Arena(Arena)` and `Extraction(Extraction)`. `World::step` asks
the mode at four points:

1. **Bound:** clamp a hero's position. Arena: the leash circle. Extraction: the boundary polygon.
2. **Spawn:** what appears, where and when. Arena: today's timer, tiers and bosses. Extraction:
   heat-driven spawning outside the safe margin, hunters, hold-circle waves, the fixed elites and
   the rares.
3. **Update:** after combat each step. Extraction: heat and floor, circles and their drain, evades,
   rare spotting, extraction timers and gryphons, merchant restock.
4. **Outcome:** whether the run is over and how. Arena: everyone down. Extraction: every hero
   extracted or down.

The arena's code moves into `mode/arena.rs` unchanged; its existing tests prove nothing changed.
Extraction lives in `mode/extraction/` (heat, circles, rares, extraction points), each part in its
own file with its own tests.

### 5.2 Shared additions to `World`

- **Leashed enemies:** an enemy may have a home circle. It targets only heroes inside it; with none
  inside for 3 seconds it evades (walks home, cannot be hurt or hurt anyone, heals to full on
  arrival) and sends `Event::Evade`.
- **Recycling:** an ordinary enemy more than 120 yd from every living hero is removed. Elites,
  rares and leashed enemies never are.
- **Extracted heroes:** `Hero::extracted`; extracted heroes do not move, cast, get targeted or count
  as living for the run's rules.
- **Reward multiplier:** the mode supplies a multiplier for XP, gold and rarity; the arena's is 1.

### 5.3 Content

`content/extraction.ron` holds a list of maps (one for now, keyed `elwynn`):

- `zone` (key into `zones.ron`), `center`, `boundary` (points);
- `safe` (centre, radius, spawn margin, cool rate) and the merchant's spot and restock time;
- `heat`: the gain table, skull thresholds and per-skull effects, floor start and rate, hunter pack;
- `elites`: enemy key, position, radius;
- `holds`: position, radius, duration, wave enemies and interval, rewards;
- `rares`: enemy key, spawn spots, first and repeat windows, spot range;
- `chests`: spots;
- `extractions`: name, position, radius, opens at, gryphon delay, recall delay;
- `settings`: the §4 table.

`tests/shipped_content.rs` loads it and plays a short headless run in it.

### 5.4 Network

- The snapshot gets a small `mode` part: heat (shared, or per hero), floor, each circle's progress
  or state, each extraction point's state and gryphon timer, spotted rares, merchant stock.
- Heroes gain `extracted`; gold and items already travel on `megabonk-systems`.
- New events: `HeatSkull { up/down }`, `Evade`, `CircleStarted`, `CircleDrained`,
  `CircleCompleted`, `RareSpotted`, `ExtractionOpened`, `GryphonCalled`, `GryphonLanded`,
  `HeroExtracted`.
- No change to `together-net` beyond carrying the new types.

## 6. The client (`benilla-app/src/together/`)

- **Menu:** a mode choice next to the zone (Arena / Extraction), offered only for zones that
  `extraction.ron` describes. `TOGETHER_MODE=extraction` picks it for test runs.
- **Rings:** the `megabonk-systems` rune ring, coloured: green (safe), red (elite and hold; a hold
  ring fills and empties with its progress), gold (extraction; dim until open, pulsing when a
  gryphon is coming).
- **Minimap and world map:** circles and extraction points as landmarks, with the minimap's edge
  arrows for those out of view; rares once spotted. If the engine's landmark layer cannot take them
  without changing engine files, draw our own marker layer over the minimap.
- **HUD:** skulls and progress, the reward multiplier, the run clock with the floor marked, gold,
  extraction status ("Stormwind Gate opens in 2:10", "Gryphon lands in 0:32") and a hold bar while
  inside one.
- **WoW's own UI:** elites and rares use the target frame's elite dragon; "Evade" uses combat text;
  announcements ("Hunters are tracking you!", "Mother Fang spotted!", "Stormwind Gate is open.") use
  the centre-screen warning text.
- **Merchant:** a goblin model in Goldshire with the item card screen.
- **Gryphon:** a gryphon model comes down to the point; heroes taken vanish with a spell visual
  and their camera follows a teammate. Flying off on it is later polish.
- **End screen:** §3.9.
- **Autoplay:** `TOGETHER_AUTOPLAY` walks between points of interest, holds circles, goes home when
  heat is high and extracts when a point is open, for background screenshot runs.

## 7. Testing

- **Headless sim tests**, per part:
  - arena unchanged after the seam (existing tests);
  - heat gains and skull thresholds; cooling only while every living hero is inside (shared) or per
    hero (not shared); never below the floor;
  - no spawns inside the safe margin; hunters spawn between a hero and the safe ring;
  - an elite ignores heroes outside its circle, evades after 3 s, heals at home;
  - a hold circle fills, drains, sends waves and completes after the final wave;
  - a rare is spotted within range only;
  - extraction points open on time; a gryphon takes only heroes inside its ring; the run ends when
    all are extracted or down; `solo_extract: false` waits for everyone;
  - recycling never removes elites or rares;
  - a snapshot round trip keeps the mode state.
- **Content test:** `extraction.ron` parses and a short headless run plays.
- **Background runs:** `WOW_BG=1` with autoplay and `WOW_LIVE_SHOT` screenshots.
- **Co-op:** a host and a joining instance on one PC with autoplay, checking one-at-a-time
  extraction and spectating.

## 8. Build order

Each step ends playable.

1. **Foundations:** merge `phase-1-sim-core` in; check the data has models for Hogger, Princess,
   Goldtooth, the five rares, a goblin merchant and a gryphon (drop or swap any that are missing);
   measure the coordinates of the boundary, safe ring, points of interest and extraction points in
   game; add the `Mode` seam with the arena unchanged.
2. **Core loop:** open Elwynn, the safe ring, heat (skulls, floor, hunters), the heat HUD and rings.
3. **Extraction:** points on timers, the gryphon, one-at-a-time extraction, spectating, end screen.
4. **Elite circles and rares**, with the minimap markers.
5. **Hold circles** on the charge circle, with waves.
6. **Goldshire merchant** and a tuning pass.

## 9. Risks

| Risk | What we do |
|---|---|
| A creature's model is not in the trimmed data | Checked in step 1; swap for one that is |
| Enemies walk straight through houses and lakes (no pathfinding) | Accept for now; place points of interest in open ground; revisit if it hurts |
| A split party doubles enemies and bandwidth | Recycling; cap enemies per hero; measure in the co-op test |
| The minimap landmark layer is engine code | Our own marker layer as the fallback |
| The pause for card picks stalls the chase in co-op | Kept for now; make it a setting if playtests say so |

## Later

- **Project 2, persistence:** meta-XP (kept always, bonus for extracting, more for objectives);
  random-stat meta gear in the visible slots (Diablo-style rolls, higher item level means bigger
  numbers) dropped by elites, rares and challenges and kept only by extracting; gear brought in
  dormant and activated or upgraded with gold; the Goldshire mailbox sending one item home; the
  stash, compared after the run; hardcore mode losing the gear you brought.
- Mounts: a 3-second cast broken by damage; fast, but no attacking.
- Bounty carrier: an elite's trophy one player carries home, slowed and marked.
- Corpse run: a fallen hero's ghost runs from the Goldshire graveyard back to their body.
- Riding the gryphon out; more maps (Westfall, Duskwood).
- Tome art: rename tomes to jewelry with jewelry icons, or find book icons in the game data.
