@AGENTS.md

# Azeroth Together (this fork)

A co-op survivors game built on the benilla engine. The plan and phase checklist: `docs/PROJECT-PLAN.md`.

## Where things live

- `crates/together-sim/` — the game rules: a pure-Rust, headless simulation at a fixed 30 Hz
  (`World::step`), plus the content types. No Bevy. Every hero has a `PlayerId`; co-op rules
  (shared XP, the run waits for every living hero's card pick) are already in.
- `content/*.ron` — classes, abilities, enemies, zones. Read from disk at startup, so edits need no
  rebuild. `crates/together-sim/tests/shipped_content.rs` checks they parse and play.
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
- Engine change made for the trimmed data: `benilla-formats/src/creatures.rs` drops a
  CreatureDisplayInfoExtra bake name whose `Textures\BakedNpcTextures\` file is not in the data, so
  humanoid NPCs are composited live instead of rendering bodiless.
  Never launch a normal (fullscreen) run without asking the user first.
- `D:` is exFAT: no symlinks (`wow-data` is a checked-out symlink stub — leave it) and cargo warns
  about hard links in the incremental cache (harmless).
