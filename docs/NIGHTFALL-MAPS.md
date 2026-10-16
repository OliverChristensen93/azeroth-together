# Nightfall: map design

Date: 2026-10-16. Status: ideas agreed in discussion, for review. Complements the spec
(`docs/superpowers/specs/2026-10-16-nightfall-design.md`) and does not change it: Duskwood stays the
first map and sub-projects 1 to 7 are built as specified. What this document adds lands as map 2
(Western Plaguelands) and a later sub-project, the map toolkit (§9).

## 1. What makes a good extraction map

Classic WoW's open zones have nothing to hide behind, and its dungeons are corridors with one way
forward. Extraction maps sit between the two. What Tarkov, Arc Raiders, Hunt: Showdown and Dark and
Darker share:

1. **Points of interest with ground between them.** A handful of landmark places hold the best loot;
   the ground between is broken and risky to cross.
2. **Every place has two or more ways in and out.** Nobody is cornered, and danger can come from
   anywhere, which is what makes hiding and listening matter.
3. **Loops, not lines.** Routes form a network with circles, so there is always a way around.
4. **Height.** Floors, cellars, roofs and towers: danger above and below, not only ahead.
5. **Cover every 10 to 20 yards in the open:** walls, fences, wagons, rocks, bushes.
6. **Underground routes** linking places out of sight: slow, dark, safe from what is outside.
7. **Landmarks to steer by:** a tower, a lit window, a bell.
8. **Mixed pace:** tight interiors, courtyards, open crossings.
9. **Exits on the edges,** each behind some danger.

Some one-door rooms are good design: put the best loot behind them, so being trapped is the risk
taken (Tarkov's dorm rooms).

## 2. Game data

We assume the full data of classic, The Burning Crusade and Wrath of the Lich King, and models from
later expansions used as props. Nothing Blizzard-made ever ships; each player brings their own data.

- **Today's trimmed 1.12 set** already holds Elwynn, Westfall, Duskwood, Gurubashi Arena, the gates of
  Ahn'Qiraj, all three battlegrounds (Alterac Valley, Arathi Basin, Warsong Gulch: maps
  `PVPZone01/03/04`), and loose buildings and dungeon pieces: Dire Maul, Blackrock, crypts, gold
  mines, a spider mine, ogre mounds, cave tunnels, caves, shipwrecks, Silithid hives, and a kit of
  houses, barns, farms, towers and wall sections.
- **The full classic data** adds Western Plaguelands and every other classic zone and dungeon, in the
  formats the engine already reads.
- **TBC, WotLK and later models** use newer formats. One offline converter turns them into what the
  engine reads, rather than teaching the engine every format; it keeps the engine close to upstream.
  WotLK-era tools come with it: Noggit (the community map editor) and a Blender add-on for WoW
  buildings (§6.6).

## 3. Map 1: Duskwood, made extraction-shaped

As the spec places it, plus what §5 to §8 add:

- **Compounds from WoW's own buildings** (`props.ron`): abandoned two-storey houses, barns, farms, the
  lumber mill, town halls, guard towers, Great Wall sections and fences turn open fields into walled
  farmsteads with several ways in.
- **An underground network** under Raven Hill, the mines and Manor Mistmantle, from the crypt, mine
  and cave pieces (§7).
- **Brush** along the forest's edges and the cemeteries (§5), and darkness levels with light pools
  (§4): Darkshire's lamps, campfires at camps, the black woods between.

## 4. Darkness and light

Light becomes a rule of the game, the same for heroes and enemies, not only a look.

### 4.1 What the engine does today

- WoW's own point lights already light the ground, buildings and models: up to 256 tracked, each
  surface taking its 3 nearest. A carried torch can light the ground around it.
- There are no shadows: light passes through walls (a torch inside a house brightens the wall
  outside).
- Pitch dark needs an ambient and sun override, beside today's fog override: a small engine change of
  the same kind.

### 4.2 Darkness levels

- **Per area, in content,** not one global blackout: some places nearly black (crypts, deep woods,
  houses without fires), roads and clearings moonlit. A map built only of black is unreadable and
  frustrating rather than frightening; games that do darkness well (Hunt, Thief) lean on pools of
  light, silhouettes and sound.
- A **light level** at any point: the area's darkness plus every light source near it (campfires,
  wall torches, lanterns, torch-bearers, spell flashes).

### 4.3 The rules

| Rule | Effect |
|---|---|
| Standing in light | A hero (or enemy) is seen from much further (raises the visibility of spec §5.1) |
| Looking into dark from light | Sight into darker places is shorter: a sentry at its campfire is nearly blind to the dark around it |
| Carrying a torch or lantern | The carrier sees further and is seen from much further |
| Torch-bearing patrols | Seen coming from far; their light reveals heroes as they pass. Hiding while a light sweeps past |
| Camps around fires | Sentries keep to their light; creeping round the edge of firelight is the way past |
| Dousing a light | A hero can put out a wall torch or a campfire to darken a route. Enemies notice a light gone out and come to check (Thief-style) |

Starting values go into `rules.ron` when this is built; the senses of the spec (§5) read the light
level where they now read "a lit area".

## 5. Brush: places to hide

- **Brush zones** in content: patches of bushes, tall grass, dead crops. A hero still or creeping in
  brush is nearly invisible; moving through it rustles (a noise, spec §5.2).
- Brush blocks sight but not sound, so it bakes as concealment, not as a wall.
- **Enemies sometimes search brush** when suspicious, so brush feels safe without being a guarantee.
- **Models:** classic WoW's bushes are small and sparse; denser foliage from later expansions (Grizzly
  Hills, Howling Fjord and newer) placed as props.
- **Lone houses and props with loot** scattered between landmarks, to hide around and to search.

## 6. Ways in and out of buildings

Most WoW buildings have one door and little height. In order of cost:

1. **Vault points** at windows and low walls: marked in content as pairs of spots (inside, outside).
   A key press there climbs through with an animation and a noise. No change to the building; the
   cheapest fix with the most effect.
2. **Cellar hatches:** a hatch in a house leads down into the underground network (§7), so every
   such house has a second way out, downward.
3. **Ruins first:** where danger is highest, pick broken buildings; the Plaguelands are full of them.
4. **Breakable spots:** a boarded wall or door, kicked through once per raid (loud), turns a dead end
   into a door.
5. **Roofs:** ladder vault points up to roofs; towns get rooftop routes. Use the stairs WoW's towers,
   two-storey houses, inns and keeps already have.
6. **Our own buildings (later):** with the WotLK-era Blender tools, extra doors cut into existing
   buildings, or new buildings made for the game. The strongest fix, and real art work.

## 7. Underground routes, on the same map

- Built from WoW's crypt, mine, cave and tunnel pieces, placed **under the terrain of the same map**.
- Entered by cellar hatches, crypt doors and mine mouths: a short "climbing down" fade moves the hero
  within the map. **No separate instance:** no loading screen to break the tension, enemies can
  follow, sound carries, and the co-op host keeps one world.
- Several entrances across the map share one network, so it links places out of sight: slow, dark
  and dangerous, but out of the night's open ground.
- The host's baked collision (spec §4) includes the pieces, so line of sight and pathfinding work
  underground as above.
- **Magic portals** stay rare: one-way, loud, special exits (a Scourge translocator, a mage's
  portal stone).

## 8. Getting down from high places

- **Fixed rides** from towers and high points: a bat or gryphon carries the hero along a set path to a
  set spot, like a flight path (Arc Raiders' ziplines, WoW-style). Loud and visible, no fighting
  during it. The cheapest: the survivors extraction branch already flies gryphons.
- **A glider** from high points (the Goblin Glider of WotLK engineering) or Slow Fall: free direction,
  but it needs changes to how the controller falls, so later.

## 9. Map 2: Western Plaguelands

The flagship map once the full classic data is in: night and plague, undead and the Scarlet Crusade.
The raid covers about the middle third of the zone, not all of it.

| Place | What it gives |
|---|---|
| **Ruins of Andorhal** | A ruined town: broken walls, gaps, half-standing towers. Ways in everywhere; rooftop routes; the heart of the map |
| **Caer Darrow** | An island town on Darrowmere Lake: approached from any shore, by swimming (slow, exposed) or by boat |
| **Hearthglen** | The Scarlet fortress on its hill, one road in today. Alternate ways in: a collapsed wall section, a cliff path, a sewer or crypt from below (§7) |
| **North of Andorhal (new)** | Around the plague-cauldron farms (Felstone Field, Dalson's Tears, Gahrron's Withering, the Writhing Haunt): a Cult of the Damned camp or a Scourge-held manor, with fires to sneak around (§4) |
| **Between them** | Lone houses and props with loot, brush (§5), farm ruins, Sorrow Hill's graves, and the underground network tying Andorhal, the farms and Hearthglen's undercroft together |

Exits on the edges: the roads toward Tirisfal, the Bulwark, Eastern Plaguelands and Alterac, and a
ride (§8) from a high point.

## 10. Later maps, by data

- **Already in our data:** Arathi Basin (a short map: farm, stables, blacksmith, lumber mill, gold
  mine), Alterac Valley (a big one: towers and bunkers with floors, caves, two fortified bases),
  Westfall (Moonbrook, Jangolode Mine, burnt farms).
- **Full classic data:** Stratholme (a whole ruined city), Blackrock Depths (an underground city:
  prison, arena, tavern, bridges), Dire Maul with its courtyards and wings, Tyr's Hand, Stormwind.
- **WotLK data:** Culling of Stratholme, Zul'Drak (a troll city on levels and ramps), Dalaran and its
  sewers, Wintergrasp, Naxxramas, Utgarde Keep, Grizzly Hills' forts.

## 11. The map toolkit (a later sub-project)

After the spec's sub-projects (or folded into one once its basics are needed), one sub-project builds
what every map uses:

1. **A placement tool:** a dev mode to walk the map and drop a prop, vault point, hatch, brush zone,
   light source or spawn at the cursor, saved to the map's RON files.
2. **Vault points** (§6.1) and **cellar hatches** (§6.2), in the host's rules and the client.
3. **Brush zones** (§5) in the bake and the senses.
4. **Light sources and darkness levels** (§4): the light level rule, torch-bearers, camps, dousing,
   and the engine's ambient override.
5. **Underground pieces** placed below terrain, with the climbing fade (§7).
6. **Fixed rides** (§8).
7. **The asset converter** for TBC, WotLK and later models (§2).

Map 2 then needs only data and content.

## 12. Open questions

- How dark is too dark: playtest the darkness levels in screenshots and in play before settling.
- Whether enemies use vault points and hatches, or only heroes (easier to escape if only heroes).
- Glider or fixed rides only.
- When to get the full classic data, which every co-op friend needs too.
