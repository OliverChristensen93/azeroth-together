# Extraction mode, part 2: the living map — design

Date: 2026-10-15. Status: draft for review. Builds on
[the extraction mode design](2026-10-15-extraction-mode-design.md) (part 1, built on the
`extraction-mode` branch).

The first playtest showed the core loop works, but a run can feel like "run in a circle until it
gets too hard". Part 2 makes the map itself the challenge. Elwynn fills with its real creatures,
standing where they stand in WoW, idle until they notice you. Roads and towns are clear; the
forest between them is not. Flights leave on a timetable, so the question becomes "can I make the
Eastvale flight, and do I cut through the forest or take the road?"

## 1. Decisions

| Topic | Decision |
|---|---|
| Hero health | Done after the playtest: 1 regen a second, 10% of maximum healed per level-up, +3% of base health per level (`rules.ron`, every mode). |
| Testing | Done: a Dev panel on the menu with multipliers for exit times, flight time, heat gain, horde size and regen. |
| Extraction | Flights leave on a timetable. Nothing to call: be in the ring when it departs. |
| Exit ambushes | Some departures draw an ambush, announced before it comes. |
| The forest | Elwynn's real spawns (CMaNGOS Classic DB), idle until they notice a hero. |
| Targeting | Spells fire only at enemies that are engaged with the party. |
| Roads, towns | Clear, because the real spawn data has nothing there. No hand-drawn roads needed. |
| Hot zones | Mines, lakes, farms and Forest's Edge: dense, and heat climbs while you are in one. |
| Goldshire | Safe ring only; never an exit. |
| Pursuer | A Stalker, later in this part (section 6). |
| Rival player bots | Not now; their own spec after playtesting. |

## 2. Timetabled flights (replaces calling)

- Each exit has a timetable: the first departure and the time between departures (for example,
  Stormwind Gate first at 6:00 then every 6 minutes; Eastvale first at 9:00 then every 7).
- A flight lands 30 seconds before its departure and waits on the ground (`window`). At
  departure it leaves with every living hero standing in its ring. Nobody needs to press
  anything.
- The HUD lists the next departure of each exit, with distance and direction, and a countdown
  while a flight waits.
- With timetables there is no "flight waits for everyone" case; the `solo_extract` setting goes.
- A third exit at Westbrook Garrison (the west road toward Westfall), so there is usually a
  choice of flights.
- **Ambushes:** each departure has a chance (`ambush_chance`, start 0.5) of drawing one. 20
  seconds before it lands, the HUD warns "Defias are converging on Stormwind Gate!". A pack of
  wildlife from the nearest hot zone then arrives from one direction, sized by the party's heat.
  A party can fight it or decide to catch a later flight.
- Heat while a flight waits on the ground: `heat.flight` a second, as now.

## 3. The living map

### 3.1 Where the creatures come from

- Source: the CMaNGOS Classic DB full dump (`ClassicDB_1_12_1_z2815`, GPL-3.0; personal use, never
  distributed). Elwynn has 1,614 permanent spawns: 880 hostile, 289 neutral (boars and so on),
  245 critters and 200 friendly NPCs.
- A bake script (`tools/bake_spawns.py`, run by hand, its output committed) turns the dump into
  `content/elwynn_spawns.ron`: every hostile and neutral spawn in Elwynn with its creature key,
  position, wander radius, respawn time and patrol path. It leaves out critters, friendly NPCs,
  event-only spawns (Scourge Invasion, Darkmoon Faire), and spawns inside the mines (below the
  mine entrances' height).
- `content/enemies.ron` gains an entry per creature kind the bake needs (Murloc Forager, Riverpaw
  Runt, Prowler, Kobold Miner and so on), with the display ids from the database.
- `wildlife_share` (start 0.5) keeps that share of the spawns, chosen once per run by the run's
  seed. Wildlife is tougher than the horde (section 3.3), so fewer creatures still make a pull
  dangerous. Hot zones keep all of theirs.

### 3.2 Only near the heroes

- A spawn exists in the world only while a living hero is within `wake_range` (start 140 yd).
  Further off it sleeps: not simulated, not in snapshots. About 40 to 80 creatures are awake
  around a hero at a time.
- `wake_range` is beyond the distance the client draws units at, so creatures are always already
  standing there when they come into sight: nothing appears from nowhere.
- Fallback, only if a playtest shows the awake count costs too much: one creature stands for its
  group, and the rest of the group comes out of its camp when it is pulled.
- A killed spawn stays dead for its respawn time (most are 180 s in the data; `respawn` scales
  it), whether or not anyone is near.
- Sleeping and waking keep each spawn's place, so a camp is where you left it.

### 3.3 Idle until noticed

- An awake creature stands or walks its patrol until a hero comes within its notice range:
  `notice` yards (start 10, plus 1 per level it has over the hero's party level, never under 5),
  times `1 + 0.15 per skull` of the party's heat. At 4 skulls little can be slipped past.
- Neutral creatures (boars, Northshire wolves) never notice; they fight back only when hit.
- Once it notices a hero it is **engaged**: it chases like the horde, and `heat.notice` (start 2)
  is added.
- **Social pull:** creatures of the same kind within `social_range` (start 12 yd) of it engage
  too.
- **Calling for help:** kinds marked `calls_help` in `enemies.ron` (Defias, kobolds, gnolls)
  shout when they engage, and every awake creature of their kind within `help_range` (start 30
  yd) engages too and comes running. It brings creatures that were already standing there, never
  new ones.
- Wildlife is tougher than the horde: health and damage times `wildlife_toughness` (start 1.5),
  and more XP and gold. Tagging a camp costs something.
- An engaged creature that has not hurt or been hurt by a hero for 10 seconds, with every hero
  more than 40 yd away, gives up, walks back to its spawn and heals (WoW's evade).

### 3.4 Targeting

- Heroes' spells and items fire only at **engaged** enemies. The heat-driven horde, hunters and
  ambushes are always engaged. Idle and neutral wildlife is never a target until it engages or a
  hero walks into it.
- This is what makes sneaking past a camp possible.

### 3.5 Hot zones

- Polygons in `extraction.ron` around Jasperlode Mine, Fargodeep Mine, Stone Cairn Lake, Crystal
  Lake, the Brackwell Pumpkin Patch and Forest's Edge.
- Inside one, heat climbs by `heat.hot_zone` a second (start 3) on top of the time outside.
- They keep all their spawns (no `wildlife_share` thinning), and the part 1 plan's elites, hold
  circles and rares live in them.

### 3.6 The horde

- The heat-driven horde stays, but it is no longer the only danger, so its size starts at half
  (`horde_share`, start 0.5). The dev panel's horde multiplier tunes both together.

### 3.7 The map's outline

- Moves south to take in Forest's Edge and the Duskwood river (Hogger's spawn points go down to x
  -10107), and follows the empty mountain cells in the north-east and east. Traced from the
  spawn density map, then checked in game with `TOGETHER_TRACE_POS`.

## 4. Sim

- `Wildlife` state in `World`: per spawn, asleep or awake (and the enemy id while awake), dead
  until, and its patrol progress. Asleep spawns cost nothing per step but a distance check, done
  every 15 steps.
- `Enemy` gains `engaged: bool`, a `home` (spawn point and wander radius) and an optional patrol.
  `move_enemies` moves unengaged enemies along their patrol or wander, and engaged ones as now.
- The notice check runs every 3 steps, only for awake unengaged enemies near a hero.
- Snapshots carry awake enemies only (as now); the 150 per hero cap still applies to the horde.
  Wildlife has its own ceiling (`max_awake`, start 120 per hero).
- `cast_abilities` and item procs pick targets among engaged enemies only.
- New events: `Engaged { id }` (an enemy noticed a hero; the client shows a "!" over it),
  `CalledHelp { id }` (the shout), `AmbushWarned { exit }`, `FlightDeparted { exit }`.

## 5. Client

- The minimap's red dots already show every enemy. Engaged ones stay red, idle ones are drawn
  grey, so a camp can be seen and skirted.
- A "!" floats over an enemy that engages, and the call-for-help shout plays as WoW's own
  combat text.
- HUD: the timetable lines (section 2) and a "Hot zone" line with the extra heat while inside
  one.

## 6. The Stalker (built last in this part)

- At 3 skulls (party heat), a single named pursuer appears out of sight: a worgen from Duskwood
  in Elwynn. The HUD warns "Something is tracking you."
- It follows the trail of the hottest hero at about the hero's speed, ignores the notice rules,
  and shows on the minimap only within 60 yd.
- It is elite and hard to kill; killing it pays a cache and a lot of heat. Going home to
  Goldshire loses it (it waits at the edge of the margin, then leaves).
- One at a time; after it is killed or lost, the next can come 4 minutes later.

## 7. Build order

1. **Timetabled flights, ambushes, the third exit:** small, and it replaces calling.
2. **The bake:** script, `elwynn_spawns.ron`, the new enemy kinds, the outline moved south.
3. **Wildlife:** sleeping and waking, idle and engaged, notice, social pull, calling for help,
   respawn, give up and go home; the targeting rule.
4. **Hot zones and the halved horde.**
5. **Client:** grey idle dots, "!", HUD lines.
6. **The Stalker.**
7. A tuning pass with the dev panel; the dev panel gains wildlife share and notice range.

## 8. Testing

- Headless sim tests per step: departures take only heroes in the ring at departure; an ambush
  is warned before it comes; a spawn wakes and sleeps with distance and keeps its death across
  sleeps; an idle enemy is never targeted; noticing respects range, level and heat; social pull
  and calling for help; a disengaged enemy walks home and heals; hot zones add heat; the Stalker
  follows and is lost at home.
- Snapshot size at a crowded moment (a hot zone at 5 skulls with four heroes).
- Background runs with screenshots, and a co-op run.

## Later

- Rival player bots (parties that may be friendly or not, third-party risk, limited flight
  seats).
- Mounts (fast, no attacks: the road-runner's tool).
- Project 2: meta progression.
