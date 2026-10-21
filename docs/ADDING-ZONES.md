# Adding a zone

A zone is one entry in `content/zones.ron`: a map, a centre and a leash (how far heroes may go
from it, 150 yd in the shipped zones), its enemies, boss and interactables. The arena is the
circle of that radius around the centre. Content is read at startup, so a new zone needs no
rebuild of the game.

What a zone needs from the game data is the terrain, buildings, doodads, water and sounds inside
that circle. The trimmed 1.12 data (`WoW\Data`) has only Elwynn Forest, part of Stormwind,
Westfall, Duskwood, Gurubashi Arena and the Gates of Ahn'Qiraj. Anything else comes from a WotLK
3.3.5 client through `together-port` ([WOTLK-PORT.md](WOTLK-PORT.md)).

## 1. Is the area in the data?

`together-port spot` answers for one point, reading the 1.12 data and the installed port:

```powershell
cargo build --release -p together-port
.\target\release\together-port.exe spot Azeroth -8800 500
```

```text
tile Azeroth_31_48
  area 1519 Stormwind City
  ground z 65.3: a zone centred here wants z about 67
  219 doodads and 5 buildings on the tile
```

Ask it for the centre and for points a leash away in each direction (`x ± 150`, `y ± 150`). A
tile 1.12 lacks says `not in the game data`. Stormwind is an example: its eastern tiles
(31_47, 31_48) are in the trimmed data, but the western half of the city (30_47, 30_48) is not.

Coordinates are WoW's world `(x, y, z)`, the ones the server and `.gps` use: x grows north, y
grows west. Good sources are the zones already in `zones.ron`, a private server's teleport list
(TrinityCore's `game_tele` table lists towns and landmarks), or a zone's map bounds:
`together-port dbc <wotlk Data> <vanilla Data> wotlk WorldMapArea 2 <AreaTable id>` gives its
left/right (world y) and top/bottom (world x) edges.

## 2. Bring in what is missing

Everything goes through the machine's shared wish list (`WoW\wotlk-port.txt`), which every session
and branch builds the same archive from. Run these from anywhere in the checkout:

| missing | wish-list line |
|---|---|
| Terrain tiles | `together-port add zone Azeroth Stormwind City` (a WotLK AreaTable zone name: the zone and its subzones), or `tiles Azeroth 30-30 47-48` by tile number |
| Buildings and doodads on tiles 1.12 has | `fill Azeroth` (already on the list: it gave Stormwind its city) |
| An enemy whose creature model 1.12 lacks | `creature <WotLK display id>`, then use the id written to `WoW\wotlk-port-ids.txt` in `content/enemies.ron` |
| A sound | `file <path>`: run the zone once and look in the log for `reading Sound\...` or `file not in patch chain` warnings |

`zone` and `tiles` port only tiles 1.12 lacks, mark them in the map's WDT, and bring every model,
building and ground texture they name. A `together-port` built before those two kinds rejects the
whole wish list, so add them to the shared list only once the branch that has them is merged
wherever sessions build the port.

Sounds the trimmed data lacks show up zone by zone. The water loops are on the list already
(`file Sound\Ambience\Water\...`, for every lake and river). Snow footsteps
(`Sound\Character\Footsteps\mFoot*Snow*.wav`) and building ambience such as
`Sound\Ambience\WMOAmbience\Ironforge.wav` are known gaps for Dun Morogh and Ironforge.

New continents (Outland, Northrend) are not playable yet: their tiles convert, but their
`Map.dbc`, `AreaTable` and `Light` rows are not ported.

**Ported data is machine-local.** `scripts/package.sh` never ships Blizzard data, and a friend's
AzerothSurvivors `Data` folder has no `patch-W.MPQ`. A zone that needs the port works only on a
machine that has built it, so for co-op both players need it (the WotLK client and
`together-port build`).

## 3. Pick the centre

- Put it on open, dry ground. `spot` gives the ground height; use about 2 yd above it as the
  centre's `z`. The game drops units onto the floor under them, but `z` is where the camera and
  the hero start before the terrain has loaded, and a `z` under the ground lets the hero fall
  through it.
- Water and buildings inside the arena are fine. The host moves a shrine or elite camp that lands
  in water or a building (`view::settle_shrines`). Enemies, pickups and spell marks stand on the
  floor the hero fights on, building floors included (`together/floor.rs`).
- The leash is a wall. Keep the circle inside the tiles you have, or the hero runs to the edge of
  the loaded world.

## 4. Write the entry

Copy a zone in `content/zones.ron` and change it:

```ron
(
    key: "stormwind",
    name: "Stormwind City",
    continent: "Eastern Kingdoms",
    description: "One line or two for the menu.",
    map: 0,                          // Map.dbc id: 0 Eastern Kingdoms, 1 Kalimdor
    center: (-8800.0, 500.0, 67.0),  // WoW x, y, z
    leash: 150.0,
    enemies: ["young_wolf", "defias_thug"],  // keys from content/enemies.ron, weakest first
    boss: "hogger",
    item_level: 10,
    shrines_at_once: 2,
    pots: 10,
    elite_camps: 3,
    interactables: [
        (kind: TitanShrine, count: 2, weight: 10),
        (kind: Chest, count: 2, weight: 10),
    ],
),
```

`crates/together-sim/tests/shipped_content.rs` plays every zone with every class, so
`cargo test -p together-sim` checks the new entry parses and plays.

## 5. Try it

Set `TOGETHER_ZONE=<key>` and a scratch `BENILLA_HOME`, so a test run does not change your saves:

```powershell
Remove-Item Env:TOGETHER_ZONE -ErrorAction SilentlyContinue; $env:BENILLA_HOME="$env:TEMP\together-test"; $env:TOGETHER_ZONE='stormwind'; .\target\release\azeroth-together.exe
```

Look for:

- ground with no holes or spikes, textures that blend, no gaps or steps at tile edges;
- water at the right height, with a depth tint at the shore and its sound;
- no swimming in the air inside buildings (a ported building's liquid field is rewritten for
  1.12, see WOTLK-PORT.md);
- the hero, enemies, pots and shrines on the ground or on the building floor you are on, and the
  leash keeping you inside a building rather than lifting you onto its roof;
- missing-file warnings in the log, for the wish list.