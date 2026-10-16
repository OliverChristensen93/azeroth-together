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
   It leaves out creatures more than 5 yd under the terrain (deep in the mines), which it reads
   with `terrain.py` from the game data's ADT tiles.
3. `check_displays.py` checks a creature display's model is in the game data.

`mpqx` is the engine's extractor: `cargo build --release -p benilla-formats --bin mpqx`. To bake
again (both tools want absolute paths on Windows):
`python -I tools/spawns/bake_spawns.py <elwynn_spawns.json> content/spawns.ron <mpqx.exe> <WoW/Data>`.
Keep its `OUTLINE` and `SAFE` in step with `content/extraction.ron`.
