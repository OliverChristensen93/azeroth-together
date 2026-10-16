# Nightfall: design

Date: 2026-10-16. Status: approved; revised the same day after research into single-player and co-op
extraction games (§1.1). Sub-project 1 built (plan docs/superpowers/plans/2026-10-16-nightfall-foundation.md). Sub-project 2 built (plan docs/superpowers/plans/2026-10-17-nightfall-world-knowledge.md). Branch: `stealth-extraction`.

*Nightfall* (working title) is a second game inside Azeroth Together: a co-op stealth extraction game
in Duskwood at night. It shares the engine, the netcode and the launcher with the survivors games and
nothing else: no auto-cast, no level-up cards, no tomes, no hordes. A party sneaks into a dark, foggy
Duskwood, loots what it can, and gets out before the night gets them. The aim is the extraction
genre's tension: choosing fights, hiding, listening, and "is what I carry worth dying for?"

The survivors games keep going beside it. The launcher's first screen picks **Survivors** (today's
menu: the arena and the survivor extraction mode on `extraction-mode`) or **Nightfall** (its own
menu). Both extraction takes are developed and playtested side by side, to see which works and what
to carry across.

## 1. Decisions

| Topic | Decision |
|---|---|
| Relation to the survivors games | A separate game: own sim crate, client module, content and menu. Shares the engine, `together-net` and the launcher. |
| Launch | One exe. A first screen picks Survivors or Nightfall; each has its own main menu. |
| First map | Duskwood (map 0), always night. |
| First classes | Rogue and Mage. |
| Combat | WoW tab-target with the real UI, made mobile: per-ability movement while casting, a dodge for everyone, telegraphs on dangerous enemies. |
| Enemies | Few and placed with intent (about 60 to 120 on the map), not hordes. |
| Stealth | Every hero has a visibility and makes noise; every enemy has an awareness of each hero. |
| What the host knows about the world | A layered height map baked from the player's own WoW data; line of sight and pathfinding run on it. |
| Death loss | A setting (`death_loss`): `everything`, `everything_one_recovery` or `found_only` (§8.2), the same everywhere on the map. |
| Hub | Goldshire's Lion's Pride Inn: walked between raids, its NPCs give contracts, trade and keep the stash; its door leads to the raid (§8.4). |
| Purpose between raids | Contracts from Duskwood's own characters and stations to upgrade in the inn (§8.4, §8.5). |
| Pressure | A night clock with stages the players hear coming (§7.2), and signature monsters with behaviour to learn (§7.3). |
| Risk zones | Some places are deadlier and hold better loot; death works the same everywhere (§8.1). |
| Variety | Every raid a little different: loot, open exits, weather, which named monsters are out (§8.1). |
| Weight | A heavier bag slows walking and sneaking and makes footsteps louder; it never stops a hero fighting (§8.2). |
| Rival kits | Come from content: simplified kits first, the full class kits can be assigned for a playtest. |
| Undecided rules | Settings in content, so they can be playtested both ways. |

### 1.1 Built for no human opponents

Nightfall has no PvP, so it does not try to copy the PvP extraction games (Tarkov, Arc Raiders, Dark
and Darker), whose tension comes from other people. The successful single-player and co-op ones
(Escape from Duckov, Zero Sievert, Single Player Tarkov, Road to Vostok, Lethal Company, R.E.P.O.,
Deep Rock Galactic) keep the loot-and-get-out stakes and get the rest elsewhere:

- **A reason for every raid:** a hub to build up and quests to follow (Duckov's hideout is what its
  critics praise most). Here: the inn, its stations and the contracts.
- **Danger with a learnable shape:** enemies as deadly as players would be (Zero Sievert), monsters
  far stronger than the player that are avoided rather than fought (Lethal Company), bots with goals
  of their own (Single Player Tarkov's questing bots). Here: senses, signature monsters, rivals.
- **Clocks that force the pace,** since nobody else does (Deep Rock Galactic's swarms come faster
  the longer a mission runs). Here: the night clock.
- **Softer, chosen losses:** a protected item (Duckov's dog), a second chance at a lost body
  (Duckov's grave), death penalties set by difficulty (Zero Sievert). Here: `death_loss`, safe slots,
  the companion, the free kit.
- **Every raid a little different,** so the same map stays uncertain.

## 2. Sub-projects

The game is built in seven sub-projects, each with its own implementation plan, in this order. Each
one ends in something playable.

| # | Sub-project | Playable result |
|---|---|---|
| 1 | Foundation: pick-game screen, the new crates, generic netcode, Duskwood at night with fog | Walk through dark, foggy Duskwood with a friend |
| 2 | World knowledge: the baked height map, line of sight, pathfinding | Enemies patrol and walk around houses, not through them |
| 3 | Senses: visibility, noise, awareness, hidden enemies kept out of the UI | Sneaking past and being spotted |
| 4 | Combat: Rogue and Mage kits, mobility, dodge, telegraphs, eating and drinking | Fights worth choosing |
| 5 | Run loop and the inn: loot, bags, weight, exits, death loss, the companion, Goldshire's inn with its stash, trader, contracts and stations | A whole extraction run with a reason to go back in |
| 6 | The night's threats: the night clock, signature monsters, risk zones, every raid a little different | Raids that stay uncertain |
| 7 | Rivals: bot adventurer parties | Others out there with goals of their own |

The run loop and the inn come before rivals: a single-player extraction game rests on the reason to
go back in (§1.1); rivals add to it.

§3 is specified fully for sub-project 1; the later sections are the design each later plan starts from.

## 3. Architecture

### 3.1 Where things live

| Path | What |
|---|---|
| `crates/nightfall-sim/` | The game rules: headless, fixed 30 Hz, its own `World`, `Snapshot`, `Event`, `PlayerInput`, `PlayerId`. Depends on neither `together-sim` nor Bevy. |
| `crates/benilla-app/src/nightfall/` | The client: steps or mirrors the sim and draws it with the engine. It decides nothing about the game. |
| `content/nightfall/` | Its RON files: classes, abilities, enemies, items, rivals, the map, `rules.ron`. Read from disk at startup. |
| `crates/together-net/` | Made generic over each game's messages (§3.3). |
| `crates/azeroth-together/` | The one launcher binary, unchanged except what §3.2 needs. |

The client copies what it needs from `together/` (unit spawning, vitals, dress, the WoW UI wiring)
rather than sharing it, so the two games can change apart. Code worth sharing later moves into a
small shared module only when both games use it as it is.

### 3.2 The pick-game screen

- `run_together` (`benilla-app/src/lib.rs`) gets a top-level `GameMode` state: `Pick` (default),
  `Survivors`, `Nightfall`. `Pick` shows two choices and nothing else.
- Every system `TogetherPlugin` adds is gated on `GameMode::Survivors`, including today's ungated
  ones (`daylight`, `follow_map`, the HUD and the meter), so they never fight Nightfall's night or map.
  `menu::spawn_root` stays at startup but its root shows only in `Survivors` + `Phase::Menu`.
- `NightfallPlugin` is added beside it, gated on `GameMode::Nightfall`, with its own `Phase`
  (`Menu`, `Playing`).
- `together::prepare` still runs before the app is built (its content is needed by Survivors);
  `nightfall::prepare` runs beside it and reads `content/nightfall/`. The start map stays Elwynn;
  picking Nightfall switches `CurrentMap` to Duskwood at run start, behind the loading screen.
- `NIGHTFALL=1` skips the pick screen straight into Nightfall's menu; `TOGETHER_*` variables skip it
  into Survivors, as today.
- A back button on each game's main menu returns to `Pick`.

### 3.3 Generic netcode

- Today only `together-net/src/lib.rs` names `together-sim` types (`PlayerInput`, `Snapshot`,
  `Event`, `PlayerId`); `wire.rs` is already generic.
- `Host`, `Client`, `HostEvent`, `ClientEvent` and the connection tasks become generic over a
  client-message type and a server-message type. Each game defines its own `ClientMsg`/`ServerMsg`.
  The `together-sim` dependency moves out of `together-net` into the survivors side.
- The protocol handshake gains a game tag (`b"survivors"`, `b"nightfall"`) beside `PROTOCOL` and the
  content hash, so a host refuses a client of the other game with a clear message.
- Invite codes, the host key, the port (7777) and relays are shared unchanged.

### 3.4 Engine changes

Kept small and in the style the fork already uses (carries in upstream files touch few lines):

- `benilla-world` lighting: a public `FogOverride { start, end, color }` resource, applied where the
  resolved fog is written (`lighting/resolve.rs`), absent by default. Today's fog fields are
  crate-private.
- `benilla-app` targeting: a `Concealed` marker that `TargetScan` (`target/scan.rs`) and the hover
  pick skip, so Tab cannot find a hidden enemy.
- Nameplates: hide plates of `Concealed` units, and of stealthed units (`BYTES_1` byte 3 `0x2`;
  today plates check only the ghost bit).
- `benilla-world` collision: a public line-of-sight segment query, for the client's own checks
  (camera, effects). The host's game rules do not use it (§4).

### 3.5 Sub-project 1 in full

Done means: from the pick screen, Nightfall's menu starts a run (solo or hosted, a friend joins by
invite code); the party appears at a Duskwood entry point at midnight in 40-yard fog with the night
grade on; heroes walk and run with the real WoW controller and UI, as a Rogue or a Mage (no abilities
yet); the run ends from a menu button. Concretely:

- `nightfall-sim`: `World` with heroes, positions reported by clients (as in survivors), the map's
  bounds as a polygon, entry points from content, `Snapshot`/`Event`/`PlayerInput`, fixed 30 Hz,
  deterministic tests.
- `content/nightfall/`: `map.ron` (Duskwood: bounds, entry points), `classes.ron` (Rogue, Mage:
  model, outfit, stats), `rules.ron` (the atmosphere numbers of §11).
- Client: pick screen, Nightfall menu (class, host/join/solo, back), units in the WoW UI, night,
  fog override, view distance, the night grade pass, the hero light. The menu is a thin start: in
  sub-project 5 its choices move into the inn (§8.4).
- `together-net` generic, survivors still passing its tests and its co-op test.

## 4. World knowledge (sub-project 2)

The survivors sim is flat: enemies see and walk through buildings. Nightfall's host needs walls,
floors and cover everywhere on the map, wherever each hero is, headless and testable.

### 4.1 The baked height map

- A **layered height map** in the style of Recast's heightfield: the map is cut into columns
  (0.5 yd), each holding a list of solid spans (from, to height). Standable floors are the tops of
  spans with enough headroom above them; several floors per column give second storeys, bridges,
  crypts and caves.
- Built from the map's own collision data via `benilla-formats` (no Bevy): terrain tiles (including
  impassable chunks), WMO collision triangles and M2 collision hulls. Tree trunks block; canopies do
  not, as in the engine's own collision.
- Extra props placed from content (§9.3) are added to it.
- **Baked on the host at first launch** from the player's own WoW data, then cached in
  `benilla-config/nightfall/<map>-<hash>.bin` (the hash covers the data files and the prop list).
  Nothing Blizzard-derived ships in the zip. Only the host needs it (the host decides the game).
- Baking runs off the main thread with a progress line on the loading screen; a target of under a
  minute for Duskwood.

### 4.2 Queries

- **Line of sight:** a ray from eye height to eye height (or to chest height) through the columns'
  spans. Used for enemy sight, rival sight, and what each hero can see.
- **Floors:** "the floor under this point" and "the floor nearest this height", for placing units
  and for movement.
- **Pathfinding:** A* over standable floor cells, linking neighbours whose height step a unit can
  climb; paths are smoothed with line-of-walk checks. Long paths are cached per (start cell, goal
  cell) for patrol routes.
- **Indoors:** a column is indoors when a solid span sits above its floor; noise through walls and
  rain use it.

### 4.3 What each hero sees

- The host sends, per player, which enemies and rivals that hero can see (in the fog distance and in
  line of sight from the hero, or from a teammate within 10 yd).
- The client draws every unit inside the fog (real walls already hide them on screen), but an unseen
  unit gets the `Concealed` marker: no Tab target, no nameplate, no minimap dot.

## 5. Senses (sub-project 3)

All numbers are starting values in `content/nightfall/rules.ron`.

### 5.1 Visibility

Each hero has a visibility from 0 to 100, from stance:

| Stance | Visibility |
|---|---|
| Running | 100 |
| Walking | 55 |
| Standing still | 35 |
| Stealthed, moving | 20 |
| Stealthed, still | 8 |

Multipliers: lantern lit ×1.6, standing in a lit area (campfires, Darkshire's lamps) ×1.5, a fire
spell's flash ×2 for 1 second. The result is clamped to 100.

An enemy can notice a hero within its **notice range**: `35 yd × visibility / 100 × facing`, where
facing is 1.0 in its front 120° cone, 0.6 at the sides and 0.3 behind. It also needs line of sight.
Enemies see no further than the fog distance.

### 5.2 Noise

Noise is an event with a position and a radius:

| Source | Radius |
|---|---|
| Running steps | 12 yd (every step) |
| Walking steps | 4 yd |
| Stealthed steps | 0 |
| Abilities | Each has a loudness: quiet 8, normal 20, loud 40 yd |
| A hit landing, a unit dying | 15 yd |
| Opening a chest, searching a body | 15 yd |
| Digging a grave | 30 yd |
| Ravens taking off | 40 yd |
| Extracting (per second in the ring) | 25 yd |

A listener without line of sight to the source hears it at half the radius (walls muffle). Noise
points enemies at the **source position**, not at the hero.

### 5.3 Awareness

Each enemy keeps a suspicion of each hero, 0 to 100.

- Seeing a hero fills it per second by `60 × (1 − distance / notice range)`, at least 10.
- A noise heard adds 35 and records the source as the place to check.
- States: **unaware** below 40; **suspicious** ("?") from 40: turns to face; from 60 walks to the place
  to check; **alerted** ("!") at 100: attacks, and shouts, alerting every ally within 15 yd to that
  hero's position.
- Without sight or noise suspicion drains 10 per second. An alerted enemy that loses sight for 5 s
  **searches**: goes to the last seen position and looks around for 15 s, then returns to its post
  or route, unaware.

### 5.4 Ambush

- The first hit on an enemy that is not alerted to its attacker deals ×1.5.
- Rogue openers (Sap, Cheap Shot, Garrote, Ambush) need the target not alerted to the rogue.

### 5.5 On screen

- By the player frame: an eye for visibility, rings for the noise made in the last second.
- Over enemies: "?" when suspicious of you, "!" when alerted to you.
- The night grade's vignette tightens as the hero's visibility drops.

## 6. Combat (sub-project 4)

### 6.1 Rules

- WoW tab-target combat: the real action bars, cast bars, global cooldown, ranges and facing rules.
- Heroes are a fixed level (30, matching Duskwood's creatures); there is no levelling during a run.
  Power comes from gear (§8).
- **Movement while casting** is per ability: `free` (instants), `slowed` (cast-time spells: the hero
  moves at `cast_move_speed`, 50% to start), `rooted` (channels and the biggest nukes).
- **Shared moves** for every class:
  - **Dodge:** a 6 yd dash in the move direction, 2 charges, 8 s to recharge each; telegraphed hits
    miss for 0.3 s; a 20 yd noise.
  - **Throw a stone:** a 20 yd noise at a chosen spot within 25 yd, every 20 s.
  - **Eat and drink:** restore health and mana over 15 s while sitting still. Nothing regenerates in
    combat.
- **Telegraphs:** elites, rares, Stitches and rivals' big moves show a ground circle or cone 1 to
  1.5 s before they land. Normal enemies swing plainly.

### 6.2 Mage

| Spell | Notes |
|---|---|
| Frostbolt | 2.5 s cast, slows, normal noise |
| Fireball | 3 s cast, loud, fire flash |
| Fire Blast | Instant, loud, fire flash |
| Frost Nova | Instant, roots enemies within 10 yd, normal |
| Arcane Missiles | Channel, rooted, normal |
| Polymorph | 1.5 s cast, quiet; a sheep wanders for 20 s or until hit |
| Blink | Instant, 15 s cooldown, quiet |
| Conjure Water | Out of combat; the mage's drink |

### 6.3 Rogue

| Ability | Notes |
|---|---|
| Stealth | Toggle; moves at 70% speed; broken by attacking or taking damage |
| Sap | Opener on a humanoid: out of the fight 30 s, quiet |
| Cheap Shot | Opener: stun 4 s |
| Garrote | Opener: bleed, silent |
| Ambush | Opener: big hit from behind |
| Sinister Strike | Builds a combo point |
| Eviscerate | Finisher |
| Gouge | Incapacitate 4 s from the front |
| Sprint | Speed for 8 s |
| Distract | A 40 yd noise at a chosen spot; nearby enemies turn to it |
| Vanish | Drops combat and re-enters stealth; 5 min cooldown |

Exact numbers (damage, energy, mana, cooldowns) are set in `content/nightfall/abilities.ron` during
sub-project 4, from vanilla's values at level 30 as the start.

## 7. Enemies and rivals

### 7.1 Enemies (sub-projects 2 to 4)

About 60 to 120 on the map, placed in content by point of interest:

| Kind | Behaviour |
|---|---|
| Sentry | Stands at a post, wide front cone |
| Patrol | Walks a route of points |
| Wanderer | Beasts that roam an area |
| Sleeper | Hears but does not see until alerted |
| Tracker | Worgen: follows a hero's trail of the last 30 s once suspicious |
| Caster | Fights at range |
| Elite | Guards the best loot, uses telegraphs |

Creatures are Duskwood's own: Nightbane worgen, skeletons and ghouls at Raven Hill, spiders in
Twilight Grove, ogres at Vul'Gol, and so on.

**Ravens** are an alarm: a flock on the ground takes off when a hero runs within 8 yd or any hero
fights within 15 yd, a 40 yd noise.

### 7.2 The night clock (sub-project 6)

The night deepens in stages, each announced by sound before it lands, so players feel the clock
without a timer on screen. Starting values (`night_stages` in `rules.ron`):

| From | Stage | What changes |
|---|---|---|
| 0:00 | Dusk | The map as placed |
| 8:00 | The howling | Worgen howls; trackers (§7.1) join patrols, more wanderers |
| 14:00 | Moonrise | The fog thickens (fog end 40 → 30 yd), enemies' notice range +20% |
| 20:00 | The hunt | Worgen packs hunt toward the loudest recent noise; one exit may close |
| 26:00 | Stitches | Stitches walks the road from Raven Hill to Darkshire: too strong to fight, loud, telegraphed swings |

Exits that open late (§8.1) pull the other way: the safest way out comes when the night is worst.

### 7.3 Signature monsters (sub-project 6)

A few named monsters with one behaviour each that players learn and counter, in the manner of
Lethal Company's monsters. Each raid rolls which are out (§8.1). Starting set:

| Monster | Behaviour | Counter |
|---|---|---|
| Mor'Ladim | Walks only while no hero looks at him | Keep him in view while someone else moves |
| Nightbane trackers | Follow scent: a hero's trail of the last 30 s | Water, roads, or a long detour break the trail |
| A banshee | Screams when she sees a hero, alerting everything in 60 yd | Break line of sight fast; kill her before she screams |
| The Dark Riders | Ride the roads at speed, see far along them | Keep off the roads; cross them carefully |
| Stitches | The last stage of the clock (§7.2) | Be gone, or be far from the road |

### 7.4 Rivals (sub-project 7)

- Parties of 2 to 3 bots: grave robbers, Defias scavengers, Night Watch deserters.
- They follow **the same sight, noise and awareness rules as heroes**: they can be snuck past,
  overheard and ambushed, and they notice heroes the same way enemies do.
- Goals: visit points of interest for loot, fight monsters on the way, then extract or hunt heroes,
  by the party's `aggression` (0 to 1).
- They carry bags; killed, they drop them.
- **Kits come from content** (`content/nightfall/rivals.ron`): each rival type lists abilities from
  the same ability system as heroes, which the AI uses by tag (opener, nuke, control, escape, heal).
  The first rivals get **simplified kits** (3 or 4 abilities and one telegraphed big move); giving a
  rival a full Rogue or Mage kit is a content change, for a playtest of both.

## 8. The run and loot (sub-project 5)

### 8.1 The run

- A party of 1 to 4 heroes starts at one of several entry points on Duskwood's edge.
- Target length: 20 to 30 minutes.
- **Exits** (3 or 4): the bridge north to Elwynn, the Westfall road past Raven Hill, the Deadwind road
  east, and a Night Watch cart in Darkshire that opens late (`cart_opens_at`). A hero extracts by
  standing in an exit's ring for 10 s; taking damage restarts the count; extracting makes noise (§5.2).
- **Darkshire** is guarded by the Night Watch and lit. Monsters stay out (Stitches excepted: the
  guards fight him); heroes standing there are very visible, and rivals can follow them in. It is
  not a safe zone.
- **Risk zones:** places such as Raven Hill's cemetery, the crypts and Vul'Gol hold more, tougher
  enemies and the best loot; the outskirts are gentler. Only danger and loot differ: death works the
  same everywhere.
- **Every raid a little different** (sub-project 6): each raid rolls its loot (from each container's
  table), which exits are open and when, the weather (clear, mist, rain shortening the fog), and
  which signature monsters (§7.3) are out. The map itself stays the same, so knowledge of it pays.
- **Falling:** a hero at 0 health is downed for 45 s and can crawl. A teammate revives them by
  standing next to them for 5 s, a 20 yd noise. After 45 s they are dead: their body stays where they
  fell, holding their gear (§8.2).
- The run ends when every hero has extracted or died. The end screen shows each hero's outcome,
  time, kills and what they carried out.

### 8.2 Loot

- Items are `nightfall-sim`'s own table in `content/nightfall/items.ron`, each with a WoW item display
  so equipped gear shows on the hero (as `together/dress.rs` does). Kinds: weapons, armour, potions,
  food and water, valuables (sold only), keys, bags.
- The **backpack** has 16 slots; bags found add slots.
- **Weight:** every item weighs something. Past `weight_free` a hero walks and sneaks slower (down to
  `weight_slow_floor` of normal speed) and their footsteps are louder (noise radius × up to
  `weight_noise_max`, §5.2). Weight never stops a hero fighting, so a solo player is never stuck;
  in co-op, spreading the load between bags is the teamwork.
- **Death loss** (`death_loss`), the same in every zone:
  - `everything`: all gear and bags are lost with the body.
  - `everything_one_recovery`: all gear stays on the body where the hero fell, and the body is still
    there in that player's next raid on the same map, marked on their map. Reaching it gives
    everything back; dying again before reaching it loses the first body for good. One second chance
    under pressure, not a refund (in the manner of Escape from Duckov's grave).
  - `found_only`: only what was found this raid is lost.

  In every mode, teammates can loot a fallen hero's body during the same raid. `safe_slots` (2) are
  always kept.
- **The companion** (unlocked at the inn, §8.5): a small pet that follows the hero and carries one
  item that is never lost, the way Escape from Duckov's dog does.
- Containers: chests, crates, bodies, rivals' bags and graves (dug, loud). Rarer loot sits in the
  risk zones (§8.1).

### 8.3 Between runs

- A **stash** per player, saved locally (`benilla-config/nightfall/stash.ron`), and gold.
- A **free basic kit** for each class, always available, so nobody is stuck with nothing.
- In co-op each hero's stash lives on their own machine; the host trusts the loadout a client brings
  (friends' co-op, not competitive).

### 8.4 The inn: Goldshire's Lion's Pride Inn

The hub between raids, walked like the rest of the game. It replaces the menu screens of sub-project 1.

- **Its own scene,** separate from any raid: the inn's interior in Elwynn, at a warm evening light,
  no fog and no enemies, a contrast to the night outside. Heroes are kept inside its rooms by the same
  bounds clamp the raid uses. Its building and the ground around it are in the game data.
- **The door is the way out:** stepping into it opens the raid board: which map (Duskwood first,
  more later), the contracts taken, and Ready. When everyone is ready, the raid starts at its entry
  point.
- **The co-op lobby:** friends who join appear in the inn and walk around in it, as on Deep Rock
  Galactic's station or Helldivers 2's ship. Class and loadout are picked there.
- **NPCs at their places:** a contract giver per character (§8.5), a banker for the stash, a trader,
  and the stations' keepers. Clicking one opens its panel.
- **Panels:** our own glue panels first. A spike checks whether WoW's real windows (gossip, quest,
  merchant, bank) can be fed locally, without a server; if they can, the inn switches to them.
- Stormwind's Trade District could be a later hub, once a test shows the city stands without the
  ground tiles missing from the data.

### 8.5 Contracts and stations

- **Contracts** come from Duskwood's own characters, taken in the inn before a raid, one or two at a
  time. Each gives a route and a reason: Commander Althea Ebonlock (thin the worgen at Brightwood),
  Madame Eva (fetch a tome from Mor'Ladim's grave), Abercrombie (bring remains from Raven Hill),
  and the big one, Stitches. Completing them pays gold and unlocks stations and their upgrades.
- **Stations** in the inn, each upgraded with gold and found materials, upgrades kept forever:
  - the stash (more room);
  - a workbench (repair and craft);
  - alchemy (potions);
  - a cooking fire (food and drink);
  - a lantern-maker (brighter, quieter lanterns);
  - the map table (intel before a raid: which signature monsters are out, which exits open);
  - the kennel (the companion, §8.2).

## 9. Duskwood (sub-projects 1 and 2)

### 9.1 Bounds and entries

- Bounds: a polygon around Duskwood's zone in content.
- Entry points: on the roads at the north bridge, the west road and the east road, and one in the
  southern woods.

### 9.2 Points of interest

To check in sub-project 2 (which have real interiors, which are reachable):
Raven Hill cemetery and village, Forlorn Rowe, Manor Mistmantle, Roland's Doom (mine), Vul'Gol Ogre
Mound (cave), Tranquil Gardens Cemetery, Brightwood Grove, Twilight Grove, Yorgen Farmstead,
Addle's Stead, the Rotting Orchard, Darkshire.

### 9.3 Extra cover

- `content/nightfall/props.ron` places WoW's own models (fences, carts, crates, gravestones, ruined
  walls) where the ground is too open. The client spawns them; the bake (§4.1) includes them.

## 10. Atmosphere (sub-project 1)

- **Night:** the time of day is forced to about midnight (minute 0) for every run, through the
  engine's manual lighting minute, as survivors forces noon today.
- **Fog:** `FogOverride` with `fog_end` 40 yd and `fog_start` 10 yd, dark blue-green.
- **View distance:** written to the engine's view distance directly (lower than the setting's
  177 yd minimum); only the client's own streaming is affected, the host's rules use the bake.
- **Grade:** a night colour pass made from the survivors grade pass (`survivors/grade.rs`):
  desaturated, cool, with a vignette that tightens as visibility drops (sub-project 3).
- **Light:** a small light around each hero; the lantern (a toggle) lights much further and raises
  visibility (§5.1). Low ground mist.
- **Sound:** the engine's positional footsteps, Duskwood's own ambience, ravens, and a heartbeat when
  an enemy alerted to you is within 20 yd.
- Later: rain that shortens the fog further.

## 11. Settings

All in `content/nightfall/rules.ron`, read at startup:

| Setting | Start | What it does |
|---|---|---|
| `fog_end`, `fog_start` | 40, 10 yd | The fog |
| `night_minute` | 0 | Time of day |
| `cast_move_speed` | 0.5 | Speed while casting a `slowed` spell |
| `notice_range` | 35 yd | Notice range at visibility 100 |
| `stealth_speed` | 0.7 | Rogue speed in stealth |
| `death_loss` | `everything_one_recovery` | `everything`, `everything_one_recovery` or `found_only` |
| `safe_slots` | 2 | Slots always kept |
| `weight_free` | 20 | Weight carried with no penalty |
| `weight_slow_floor` | 0.7 | Slowest walking and sneaking speed at full weight |
| `weight_noise_max` | 1.5 | Footstep noise radius at full weight |
| `night_stages` | §7.2 | The night clock's stages and their times |
| `cart_opens_at` | 1080 s | When Darkshire's exit opens |
| `downed_for` | 45 s | Time to revive a fallen hero |

## 12. Testing

- `nightfall-sim` unit tests: visibility and notice range, noise with and without walls, awareness
  states and search, line of sight and pathfinding on small hand-made height maps, determinism.
- `crates/nightfall-sim/tests/shipped_content.rs`: `content/nightfall/` parses and plays.
- A bake test over a corner of Duskwood, skipped without WoW data.
- `together-net` and `together-sim` tests still pass after the netcode change; the survivors co-op
  test on one PC still works.
- Client hooks: `NIGHTFALL=1`, `NIGHTFALL_CLASS`, `NIGHTFALL_AUTOPLAY` (walks the map, later sneaks
  and loots), `NIGHTFALL_SHOWCASE` (later: one of each enemy kind near the start). The engine's
  `WOW_BG`, `WOW_WIN` and `WOW_LIVE_SHOT*` work as for survivors.
- Co-op on one PC: `NIGHTFALL_HOST=1` and `NIGHTFALL_JOIN=127.0.0.1` (or an invite code).

## 13. Open questions

- **Rival kits:** simplified or full class kits; both possible through content (§7.4). Decide after
  playtests.
- **Death loss:** which of the three `death_loss` modes is the default (§8.2). Decide after playtests.
- **WoW's own windows in the inn:** whether gossip, quest, merchant and bank windows can be fed
  without a server (§8.4). A spike in sub-project 5.
- **Dodge key:** which key, without clashing with WoW's default bindings. Decide in sub-project 4.
- **The name:** *Nightfall* is a working title.

## 14. Not in this spec

- Other maps and classes.
- Progression beyond the stash, gold, contracts and the inn's stations.
- Weather beyond the per-raid roll of §8.1.
- Player-versus-player.
