# WotLK port

`together-port` brings art from a WotLK 3.3.5 client into Azeroth Together. It rewrites each file
in 1.12's formats and packs them into one archive, `WoW\Data\patch-W.MPQ`, which sits above the
1.12 archives in the patch chain. The engine never reads a WotLK format.

## Using WotLK art from any session or branch

Art is machine-local: Blizzard data never goes into git. So every session, worktree and branch on
this machine shares **one wish list**, `WoW\wotlk-port.txt` in the main checkout, and one
archive built from it. To get something, add it to the list. That rebuilds the archive with
everything on the list, so nothing another session added is lost:

```powershell
cargo build --release -p together-port
.\target\release\together-port.exe add fill Azeroth
.\target\release\together-port.exe add wmo "World\wmo\Azeroth\Buildings\Stormwind\Stormwind.wmo"
.\target\release\together-port.exe add model "Creature\Arthas\Arthas"
.\target\release\together-port.exe add creature 24949
.\target\release\together-port.exe add file "Sound\Music\ZoneMusic\Nexus\NX_BG_Night01.mp3"
.\target\release\together-port.exe add zone Azeroth Dun Morogh
.\target\release\together-port.exe add tiles Azeroth 31-35 38-42
.\target\release\together-port.exe list
.\target\release\together-port.exe build
```

Run it from anywhere inside the checkout, including a worktree under `.claude\worktrees\`. It
always uses the main checkout's `WoW` folder; `TOGETHER_PORT_ROOT` names another.

| kind | what it ports |
|---|---|
| `fill <Map>` | Every model, WMO and ground texture the map's 1.12 tiles name but the trimmed 1.12 data lacks. The tiles are there, their buildings and doodads were cut. `fill Azeroth` gives Stormwind its city, Goldshire its smithy, and about 180 more. |
| `wmo <path.wmo>` | One WMO: root and group files (WMO v17 in both versions, copied as is), the textures 1.12 lacks, and its doodads converted. |
| `model <path>` | One M2 (no extension), converted, with its hard-coded textures and event sounds. It replaces 1.12's own model if 1.12 has one. |
| `creature <id>` | A WotLK `CreatureDisplayInfo` row (16 to 12 columns), its `CreatureModelData`, model, skins and `CreatureSoundData` and sounds. A display id 1.12 already uses is 1.12's own creature and is left alone. The ids each entry got are written to `WoW\wotlk-port-ids.txt`. |
| `file <path>` | Any file, copied as is: BLP textures, WAV and MP3 sounds, anything whose format did not change. |
| `zone <Map> <name>` | WotLK's terrain tiles of a zone that the 1.12 data lacks, converted (below), and everything they name, as `fill` does. A tile is the zone's if one of its chunks is the WotLK `AreaTable` zone of that name on that map, or one of its subzones; only tiles inside the zone's `WorldMapArea` bounds are looked in. `zone Azeroth Dun Morogh` brings Dun Morogh and Ironforge, `zone Azeroth Redridge Mountains` Redridge. |
| `tiles <Map> <x0-x1> <y0-y1>` | The same for a block of tiles by number (`Azeroth_33_41.adt` is x 33, y 41). A tile 1.12 has is always 1.12's own. |
| `race 10` / `race 11` | The Blood Elf and Draenei heroes (below). |
| `wotlk <Data folder>` | Which WotLK client to read; once per machine (or set `WOTLK_DATA`). |

Find a path with `together-port vlist WoW\Data <text>` (what 1.12 has). Pull one WotLK file out to
look at with `together-port extract <wotlk Data> <path> <out>`. Look at WotLK or 1.12 DBC rows with
`together-port dbc`. Paths are the client's own, backslashes, any case.

`zone` and `tiles` are newer than the other kinds, and a `together-port` built before them rejects
the whole wish list over one line it does not know. So they become usable on the shared list once
the branch that adds them is merged and every session builds from it; until then, test them
against a scratch copy (`TOGETHER_PORT_ROOT` naming a folder with its own `WoW\wotlk-port.txt`
and a copy of `WoW\Data`), never on the shared list.

A build takes a lock (`WoW\wotlk-port.lock`), so two sessions never write the archive at once.
If a build was killed, delete the lock by hand. The archive in place is moved aside while a
build reads the 1.12 data, so a build always starts from vanilla, and is put back if the build
fails. Before it is written, every file is read back with `benilla-mpq`, every model with
`benilla-m2`, every WMO with `benilla-wmo` and every terrain tile with `benilla-adt` and the
client's terrain mesher.

After a build, run the game as it is; data needs no rebuild of the game.

## What can be ported, and what not yet

- **Areas whose 1.12 tiles exist.** Any map in the trimmed data (Elwynn, Stormwind, Westfall,
  Duskwood, Gurubashi Arena, Gates of Ahn'Qiraj) can be completed with `fill <Map>`.
- **Areas of a 1.12 map whose tiles were cut** (Dun Morogh and Ironforge, Redridge, the rest of
  the Eastern Kingdoms and Kalimdor): `zone <Map> <name>` or `tiles`. The map's 1.12 WDT is
  marked where it does not mark a ported tile. Zone names, music and lighting come from 1.12's own
  `AreaTable` and `Light` rows, which still have every old-world zone.
- **New maps** (Outland, Northrend). The tiles convert, and a map 1.12 has no WDT for gets
  WotLK's, but the client cannot enter the map without its `Map.dbc`, `AreaTable` and `Light`
  rows, which are not ported yet. Northrend's alpha maps are compressed and 8-bit, which the
  converter handles; it has not been looked at in the game.
- **What converted terrain loses:** WotLK's vertex shading (`MCCV`, the tinted ground of newer
  zones), its ambient sound emitters (`MCSE`), the flight-ceiling bounds (`MFBO`), and alpha maps
  past 4 bits (8-bit maps are rounded to 4). Slime has no ADT drawing in 1.12 and is not drawn.
- **Particle emitters, ribbons and lights** on converted models are dropped: ported torches and
  braziers have no flames yet. `build` prints how many.
- **NPC humanoids** (a `CreatureDisplayInfoExtra` row: dressed NPC bodies) are not ported;
  creatures with their own model are.

## How the formats are bridged

- **M2** v264 to v256 (`m2.rs`):
  - skin profiles are embedded and `.anim` files folded in;
  - each sequence gets a window on one shared timeline, with `ranges` rebuilt;
  - compressed quaternions are expanded;
  - 1.12's PlayableAnimationLookup is rebuilt from HumanMale's;
  - the Death Knight eye glow (1703) and belt geosets (18xx) are hidden;
  - lights, ribbons and particles are dropped.
- **DBC** rows are remapped column by column (`port.rs`). A WotLK id 1.12 already uses for
  something else gets a fresh id.
- **Textures** are copied as is (both are BLP2), except character skin layers, which are halved
  (`blp.rs`). WotLK's skins are 512×512, while 1.12's compositor paints at fixed pixels for a
  256×256 body.
- **WMOs** are v17 in both and are copied as is.
- **ADT** terrain tiles are v18 in both (`adt.rs`); what differs:
  - WotLK's water is one `MH2O` chunk of liquid instances per map chunk. Each instance becomes a
    cell rect in 1.12's per-chunk `MCLQ`: one 0x324-byte block per liquid kind (header bits
    `0x04` river, `0x08` ocean, `0x10` magma, `0x20` slime, in that order), each cell's nibble
    the kind as 1.12's own tiles have it (4 water, 1 ocean, 6 magma, 3 slime; `0xf` dry). The
    kind is the WotLK `LiquidType` row's `Type`. Heights and depths are the instance's own; a
    depth it lacks is taken from the terrain below at 1.12's authored rate, and magma gets
    world-continuous texture coordinates;
  - alpha maps: an RLE-compressed layer (`MCLY` `0x200`) is unpacked, and an 8-bit one (the
    WDT's `MPHD` `0x4`) rounded to 4 bits, so every layer is 1.12's 2048-byte map;
  - `MCCV`, `MCSE`, `MFBO`, `MTXF` and `MH2O` are dropped, and every `MHDR`, `MCIN` and `MCNK`
    offset recomputed. `MCNR` keeps its 13 bytes of padding past its declared size.
- The output is 1.12's MPQ envelope (`mpq_out.rs`): v1, 4 KiB sectors, zlib, never
  single-unit or encrypted.

The tool mounts only the archives a stock 3.3.5a install has (`source.rs`), so any 3.3.5 client
gives the same result. A private server's own `patch-?.MPQ` files are left out.

## The Blood Elf and Draenei heroes

`race 10` and `race 11` bring:

- **Bodies:** the four body models.
- **Rows:**
  - their `CreatureDisplayInfo`, `CreatureModelData` and `CreatureSoundData` rows;
  - the `SoundEntries` rows those and the models' sound events name, with the sound files.
- **Customization:**
  - `CharSections` (reordered; Death Knight rows left out) with their textures, halved;
  - `CharHairGeosets`, `CharacterFacialHairStyles`, and `CharBaseInfo` with no Death Knight.
- **Outfits:** `CharStartOutfit`. Each class and sex wears the 1.12 outfit another race of that class starts in, since WotLK's are Burning Crusade items.
- **Voices:** `VocalUISounds` lines 1.12 speaks, and `EmotesTextSound`.
- **`ChrRaces` rows:** Blood Elves play under the Orc faction template and Draenei under the Human one.
- **Helms:** every 1.12 helm in the `_BeM/_BeF/_DrM/_DrF` shapes.
- **Race icons:** WotLK's race icon sheet, as `UI-CharacterCreate-Races-Wrath.blp`.

These are the only engine changes:

- `PLAYABLE_RACES` adds 10 and 11.
- The race name tables, faction side and PvP team.
- `creature_type` treats 10 and 11 as Humanoid.
- The helm prefixes `Be`/`Dr`.

The menu (`together/menu.rs`) adds a fifth row to each faction column. A machine without the
archive draws a Blood Elf hero as a Human and a Draenei as a Night Elf
(`together/dress.rs` `drawn_race`), so a friend who has only 1.12 data still sees the hero.

Gaps:

- WotLK has no `DeathSkeletons` models for the two races, so a fallen hero leaves no bone pile.
- 58 textures WotLK's own tables name do not exist in WotLK either: scalp and facial-hair
  overlays, and one Draenei torso skin. WotLK skips them too.

## Tests

- `cargo test -p together-port` covers the converters. Where the archive is installed, it also
  reads it through the client's own loaders, every ported terrain tile included (on its map, and
  meshed). `TOGETHER_PORT_ROOT` points the tests at a scratch copy too.
- With the archive installed, eight `benilla-formats` data tests that count the shipped 1.12 rows
  fail by design, because they count the ported rows too. They are the CreatureSoundData,
  CreatureDisplayInfo (two), ChrRaces language, SoundEntries (two) and VocalUISounds (two) counts.
  Move `patch-W.MPQ` aside to run them against 1.12 alone.
- The archive is Blizzard data: never ship it (`scripts/package.sh` does not).