@AGENTS.md

# Azeroth Together (this fork)

A co-op survivors game built on the benilla engine. The plan and phase checklist: `docs/PROJECT-PLAN.md`.

## Where things live

- `crates/together-sim/` — the game rules: a pure-Rust, headless simulation at a fixed 30 Hz
  (`World::step`), plus the content types. No Bevy. Every hero has a `PlayerId`; co-op rules
  (shared XP, the run waits for every living hero's card pick) are already in.
  - Megabonk-style systems, each in its own file: `stats.rs` (the hero stat block, pacts, the
    party's `Difficulty`), `interact.rs` (shrines, chests, the War Banner, pots: kinds, the fill
    circle, placement), `items.rs` (passive items and their copy limits), `powerups.rs` (party
    power-ups, health shards, pot loot), `world/arena.rs` (the arena's upkeep: shrines and pots
    coming back, elite camps, pickups, power-ups running out). Cards (`cards.rs`) are spells,
    tomes (4 slots, rank 5), pacts, items or the gold/heal/reroll fallback. A spell's upgrade card
    carries 1-3 rolled `Boost`s (damage, cooldown, range, area, duration, tick speed, shield,
    projectiles, crit, crit damage, slow, freeze), only those its tags allow, more and bigger the
    rarer; they add up in `Owned::mods` (`SpellMods`), applied in `spell_spec` before augments.
    Rank only counts upgrades toward the augment milestones (5, 10, 15). Gold: every coin goes
    into every hero's own purse; each hero pays for their own chests.
- `crates/nightfall-sim/` — Nightfall's rules: a second game, a co-op stealth extraction game in
  Duskwood at night (`docs/superpowers/specs/2026-10-16-nightfall-design.md`). Headless, 30 Hz, no
  Bevy and no `together-sim`: heroes, poses kept inside the map's bounds, content
  (`content/nightfall/*.ron`), its wire messages. Its client is `crates/benilla-app/src/nightfall/`,
  which copies what it needs from `together/` instead of sharing it.
- Nightfall's world knowledge (sub-project 2): `nightfall-sim/src/heightmap/` is the layered
  height map (half-yard columns of solid spans; floors, indoors, line of sight, A* paths with a
  cache, a byte codec), tested on hand-made maps (`heightmap/testmap.rs`). `crates/nightfall-bake/`
  bakes it from the player's own WoW data (terrain, WMO walking collision, M2 collision hulls;
  impassable chunks and everything outside the bounds blocked) and caches it, deflated, in
  `<home>/nightfall/<map>-<16 hex digits>.bin` (`benilla-config/nightfall/` by default; the hash
  covers the data files, bounds and props, so a change bakes anew). The client
  (`nightfall/terrain.rs`) does the bake: the first frame Nightfall is up as a solo player or host
  it reads the cache or bakes on a thread of its own, with a progress line at the bottom of the
  screen, then hands the map to the host's world (`World::attach_map`); a joined client never
  bakes and shows no line. Unreadable files and a failed cache write are logged (a map with
  unreadable files plays but is not cached), and a failed bake means no enemies this run. Enemies
  (`nightfall-sim/src/enemies.rs`, placed by `content/nightfall/spawns.ron` from the kinds in
  `enemies.ron`) stand, patrol and wander on its paths; the world's warnings (content the map
  could not give, paths that found no way) are logged as `nightfall: …` as they arise. Each
  player's snapshot carries the enemies near their hero and which they see (`World::seen_by`),
  kept in the client's `Sim::seen`. The client (`nightfall/enemies.rs`) draws each enemy near the
  hero as a WoW creature whose guid carries its kind's template entry (`enemy_guid_serial`), on
  the floor the host says it stands on.
- The launcher's first screen (`benilla-app/src/game_pick.rs`) picks Survivors or Nightfall; every
  Survivors system is in `together::SurvivorsSet`, gated on `GameMode::Survivors`.
- `together-net` is generic over each game's messages (`Host<Up, Down>`, `Client<Up, Down>`) and
  starts every connection with a handshake naming the game; Survivors' messages live in
  `together_sim::messages`.
- `content/*.ron` — classes, abilities, enemies, zones, and `rules.ron` (tome slots and ranks, item
  copies, shrine and elite-camp timers: what a game mode may set its own way). Read from disk at
  startup, so edits need no rebuild. `crates/together-sim/tests/shipped_content.rs` checks they
  parse and play. An ability's description holds placeholders (`{damage}`, `{range}`, `{area}`,
  `{targets}`, `{duration}`, ...; `together_sim::PLACEHOLDERS`) that `describe` fills from the
  spell's cast, so cards and tooltips always give its real numbers.
- `crates/benilla-app/src/together/` — the client: steps the sim and draws it with the engine's
  unit, spell-visual and combat-text systems. It decides nothing about the game.
  - The real WoW 1.12 UI (FrameXML from the MPQ) and character controller are ON (no `CaptureMode`):
    the local hero is a player-kind unit whose guid is our `SelfGuid`, so the controller drives it
    and PlayerFrame/TargetFrame/Minimap/BuffFrame read it. `feed_pose` reports the controller's pose
    to the sim (`PlayerInput::pose`), as a WoW client reports moves; the sim only clamps it to the
    leash. Units are filed in `GuidIndex`/`NameCache`; `write_vitals` writes health/level/XP fields.
  - `dress.rs`: players wear item *entries*, so the hero's outfit and weapons are stand-in item
    templates filed in `Items`. `ui.rs`: Lua run once per UI session (chat window colour/alpha).
  - Keys 1/2/3 pick cards while cards are up (they also press the empty action buttons; harmless).
- `crates/azeroth-together/` — the launcher binary.
- `crates/benilla-app/src/survivors/` — the upstream fork's own single-player mode, kept as reference.
  Our mode does not depend on it.
- Keep engine crates untouched where possible so upstream merges stay easy.

## Build, test, run (Windows, GNU toolchain)

The Rust toolchain here is `x86_64-pc-windows-gnu`; MinGW-w64 (WinLibs, installed with winget) must
be on `PATH` for builds:

```bash
export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"
cargo test -p together-sim -p together-net     # fast: the game rules and netcode, no engine
cargo build --release -p azeroth-together       # ~6 min incremental, ~17 min cold
./target/release/azeroth-together.exe           # data from WoW/Data, content from content/
```

- Game data: `WoW/Data/` (gitignored) holds Aethus's trimmed 1.12 `patch.MPQ` + `patch-2.MPQ`.
  Terrain exists only for Elwynn Forest, Westfall, Duskwood, Gurubashi Arena and Gates of Ahn'Qiraj.
- `TOGETHER_CLASS`, `TOGETHER_ZONE`, `TOGETHER_CONTENT` pick class, zone and content folder.
- Testing without disturbing the user's screen: `WOW_BG=1 WOW_WIN=1280x720` (background window,
  never focused), `TOGETHER_AUTOPLAY=1` (walks and picks cards by itself), and
  `WOW_LIVE_SHOT=<png> WOW_LIVE_SHOT_AT=<s> WOW_LIVE_SHOT_COUNT=<n> WOW_LIVE_SHOT_EVERY=<s>` for screenshots,
  `TOGETHER_CAM_DIST=<yards>` for a close-up starting zoom, `TOGETHER_TRACE_JUMP=1` to log jump heights.
- Co-op test on one PC: run one instance with `TOGETHER_HOST=1 TOGETHER_AUTOPLAY=1` and a second,
  a few seconds later, with `TOGETHER_JOIN=127.0.0.1 TOGETHER_AUTOPLAY=1` (`TOGETHER_NAME` names the hero).
  The host starts the run 20 s in once someone has joined. Co-op is host-authoritative: the host runs
  `together-sim`, `together-net` (TCP, port 7777) sends inputs up and snapshots + events down, and each
  client mirrors the world (`World::apply_snapshot`) but owns its own hero's position.
  Internet co-op is by invite code (`together-net/src/invite.rs`, iroh 1.3 over number 0's lookup and
  relays): the host logs `together: invite code <code>` and `TOGETHER_JOIN=<code>` joins with it; the
  key behind the code is `benilla-config/together-host-key.txt`. `cargo test -p together-net -- --ignored`
  runs the online invite test.
  Spells and augments (`together-sim`: `spell.rs` tags/targeting/statuses, `augments.rs` the 32
  general augments and the mage's 36 capstones, `world/combat.rs` casting and every mechanic):
  `TOGETHER_XP_RATE=<n>` multiplies experience; `TOGETHER_LOADOUT=frostbolt@4,fireball@9+frostfire`
  starts the host's hero with those spells, ranks and augments (names as on the cards, any case);
  `TOGETHER_CLASS` beats the menu's saved pick. The mage's movement ability is Blink on F (30 s).
  The menu's Dev panel (`together/dev.rs`, saved in `benilla-config/together-dev-run.txt`) sets
  experience, hero and enemy health, enemy damage, horde size and the rank new spells start at, for
  runs this player starts or hosts; `TOGETHER_DEV_PANEL=1` opens it. Spell tooltips are
  `together/tooltip.rs` (WoW's own action-button tooltip is turned off in `ui.rs`);
  `TOGETHER_TIP_SLOT=<n>` shows button n's for a test shot (`TOGETHER_TIP_SHIFT=1` as if Shift
  were held). The tomes and items carried are a strip of icons under the player frame, and the
  stat sheet opens with its Stats button or Tab (`together/inventory.rs`; `TOGETHER_STATS=1`
  opens it).
  `TOGETHER_FRAIL=<player>` drops that hero to 1 health 15 s into the run, to see a fall and a revive.
  `TOGETHER_SHOWCASE=1` puts one of each shrine, one of each power-up and an elite camp around the
  start, and brings the boss 8 s in; `TOGETHER_AUTOPLAY` also walks to shrines and pots and carries
  banners. The host moves a shrine or camp that lands in water or a building (`view::settle_shrines`).
  On Windows every test window (`WOW_LIVE_SHOT` etc.) opens behind the user's work, parked on the
  primary monitor's right edge with a 32 px sliver showing (`capture/probes/run.rs`); keep it so.
- Sharing with a friend: `scripts/package.sh` builds the `share` profile without `dev` and writes
  `dist/AzerothTogether.zip` (exe, `content/`, `PLAYING.txt`; never Blizzard data). The friend copies
  the `Data` folder from their own AzerothSurvivors install next to the exe.
- Engine change made for the trimmed data: `benilla-formats/src/creatures.rs` drops a
  CreatureDisplayInfoExtra bake name whose `Textures\BakedNpcTextures\` file is not in the data, so
  humanoid NPCs are composited live instead of rendering bodiless.
  Never launch a normal (fullscreen) run without asking the user first.
- Nightfall: `NIGHTFALL=1` skips the pick screen into Nightfall (any `TOGETHER_*` variable skips it
  into Survivors; with both kinds set, the `NIGHTFALL*` ones win and Nightfall starts).
  `NIGHTFALL_CLASS` (`rogue`, `mage`), `NIGHTFALL_NAME`, `NIGHTFALL_CONTENT` (a folder without
  `map.ron` is an error, not a fallback), `NIGHTFALL_AUTOPLAY=1` (starts a run and walks a
  circle), `NIGHTFALL_TRACE_POS=1` (logs the hero's WoW position every 2 s, for measuring entry
  points), and co-op with `NIGHTFALL_HOST=1` / `NIGHTFALL_JOIN=<code or address>`.
  `cargo test -p nightfall-sim` runs its rules.
- Nightfall's bake: `cargo test -p nightfall-bake` (its Darkshire test needs
  `WOW_DATA=D:/Games/azeroth-together/WoW/Data` in a worktree without `WoW/Data`, and skips without
  it); `cargo run --release -p nightfall-bake --example bake_duskwood` times the whole bake;
  `cargo run -p nightfall-bake --example buildings -- <x> <y> [radius]` lists the buildings around
  a point, for placing enemies. `NIGHTFALL_TRACE_SEEN=1` logs how many enemies the hero sees.
  Enemy groups in `spawns.ron` marked `measure:` still sit near the entry: move each to its place
  once measured in the game (`NIGHTFALL_TRACE_POS=1`); a group whose point no path reaches is left
  out and named in the log as `nightfall: spawns.ron: …`.
- `D:` is exFAT: no symlinks (`wow-data` is a checked-out symlink stub — leave it) and cargo warns
  about hard links in the incremental cache (harmless).
