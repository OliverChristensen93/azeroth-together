# Project plan: co-op Azeroth survivors game

Working title: **Azeroth Together** (rename freely).

A survivors-style game in the WoW world, built for 2–4 player co-op from day one.
It runs on the open-source **benilla** engine (a WoW 1.12.1 client rebuilt in Rust + Bevy),
taken from a fork of [OutThisLife/azeroth-survivors-re](https://github.com/OutThisLife/azeroth-survivors-re).
WotLK 3.3.5 data (Northrend zones, newer spell effects and sounds) gets added on a separate, later track.

Timelines assume part-time work with AI doing most of the coding. They are rough.

---

## 1. What we know (from the investigation)

- **Aethus's `AzerothSurvivors.exe` is closed source.** It is version 0.77.6, and its game crate is `wenilla-rogue`.
  Mods for it are limited to addons (interface only) and probably MPQ reskins. We are **not** building on it.
- **The `azeroth-survivors-re` repo has real source code** (MIT/Apache). It is a simpler fan remake, not Aethus's code.
  - The engine (benilla) is hundreds of files: WoW format loaders, rendering, animation, spell effects, sound.
  - The game is only `crates/benilla-app/src/survivors/`, 13 files, offline, written for one local player.
  - Content we can reuse as reference: 73 ability definitions (9 classes), 16 zones, 10 passive stats (in `data.rs`).
  - It is set up to build for the browser (WebAssembly). The engine is natively a desktop program, but a native Windows build is unverified.
- **The engine reads only 1.12-era data.** Models: format versions 256–263 (Classic and TBC). Archives: the MPQ v1 header only.
  WotLK models (v264) are rejected today.
- **Engine version:** Bevy 0.18.

## 2. Decisions (recommended defaults, change any of them)

| Decision | Default | Why |
|---|---|---|
| Platform | **Native Windows first**, browser later | Easier networking, debugging and performance |
| Netcode model | **Host-authoritative**: the host runs the real game, others send inputs and receive state | No determinism requirements; simplest correct co-op |
| Players | 2–4 | Matches the genre and the Discord mod |
| Connection | **Direct IP / LAN first**, then an invite-code relay | Ship the hard part (gameplay sync) before the lobby work |
| Data version | **1.12 first**, 3.3.5 as a later track | 1.12 works today |
| Content format | Abilities, enemies and zones in **data files (RON)**, not Rust code | Add or tune content without recompiling; mod-friendly |
| Levelling | **Shared party XP**; each player picks their own cards on level-up | Keeps the party together, keeps builds personal |
| Death | **Downed state**, revived by a teammate standing nearby; run ends if everyone is down | Standard co-op survivors behaviour |
| Scaling | Enemy health and spawn rate scale with player count | Keeps difficulty fair |

## 3. Architecture

```
upstream benilla crates     untouched, so upstream updates merge cleanly
        │
 ┌──────┴────────────────────────────────────────────────────┐
 │ game-content  RON data: abilities, enemies, zones, cards   │
 │ game-sim      game rules; pure Rust, headless, fixed tick  │
 │ game-net      messages + transport (inputs up, state down) │
 │ game-client   Bevy app: draws sim state with benilla       │
 └────────────────────────────────────────────────────────────┘
```

- **game-sim** holds every rule: heroes, enemies, abilities, hits, XP, cards, loot.
  - Fixed 30 Hz tick.
  - No rendering dependencies, so it runs headless on the host and is easy to unit-test.
  - It must not know which WoW version the art comes from.
- **game-net** carries messages between host and clients.
  - Clients send inputs (move direction, aim, card picks).
  - The host sends snapshots (~20 Hz, delta-compressed, quantized positions) plus events (casts, hits, deaths, loot).
  - Projectiles and spell effects are spawned on each client from cast events, never replicated one by one.
    This keeps hundreds of enemies affordable.
  - Library: spike `bevy_replicon` (+ renet) against `lightyear` in Phase 2. Check which supports Bevy 0.18.
- **game-client** turns sim state into WoW visuals.
  - Maps each sim entity to a creature display ID, each ability to a spell-visual reference, plus sounds.
  - Interpolates remote entities; predicts only the local hero's movement.
  - Owns the HUD and party frames.
- **Visual references are abstract.** An ability says `visual: "arcane_missiles"`, and a lookup table maps that to
  a 1.12 or 3.3.5 SpellVisual ID. This keeps the WotLK track from touching gameplay code.

## 4. Phases

### Phase 0: Foundation (≈1 week)
- [ ] Choose a project location (e.g. `D:\Dev\azeroth-together`) and fork the repo on GitHub.
- [ ] Install the Rust toolchain from `rust-toolchain.toml` and fetch the 1.12.1 data (~5.5 GB).
- [ ] **Build and run the existing survivors mode natively on Windows.** This is the biggest unknown.
- [ ] Add a `CLAUDE.md` with build/test commands and conventions for AI-assisted work.

**Done when:** the existing survivors mode runs natively.
**Gate:** if a native build is not feasible, decide between fixing it and switching to the browser build.

### Phase 1: Single-player sim core (≈1–2 weeks)
- [ ] Create `game-sim`, `game-content` and `game-client` crates next to the engine.
- [ ] One hero moving in one zone. Enemies spawn and chase. 3 abilities. XP, levels and cards.
- [ ] Ability, enemy and zone data in RON, loaded at startup.
- [ ] Unit tests for the sim (damage, cooldowns, levelling), running headless.

**Done when:** a playable single-player run in our own code, drawn by benilla.

### Phase 2: Co-op prototype (≈2 weeks)
- [ ] Netcode library spike: replicon vs lightyear; pick one.
- [ ] Host / join by IP. 2 players. Inputs sent up, snapshots and events sent down.
- [ ] Interpolation for remote heroes and enemies; local movement prediction.
- [ ] A debug overlay for ping, bandwidth and entity counts.

**Done when:** two PCs play a 10-minute run with 200+ enemies, smoothly, on LAN and over the internet.

### Phase 3: Full game loop (≈3–5 weeks)
- [ ] Port all 73 abilities and 16 zones from `data.rs` into RON.
- [ ] Bosses and elites; downed and revive; scaling by player count.
- [ ] Shared XP, per-player cards; loot and gold rules.
- [ ] Party frames, damage meter, run results screen.

**Done when:** feature parity with the repo's survivors mode, in co-op.

### Phase 4: Co-op UX and polish (≈2 weeks)
- [ ] Lobby: invite code through a small relay or rendezvous server (or Steam networking later).
- [ ] Reconnect, host-leaves handling, pause rules, settings.
- [ ] Per-player progression saved locally.

**Done when:** a friend can join with an invite code, no port forwarding needed.

### Phase 5: WotLK 3.3.5 data track (≈4–8 weeks; can start in parallel after Phase 1)
Reference: wowdev.wiki (file formats and DBC layouts).
- [ ] **5a. Archives:** MPQ v2 header and the 3.3.5 archive chain (`common`, `expansion`, `lichking`, locale, patches).
- [ ] **5b. Database tables:** 3.3.5 layouts for Spell, SpellVisual, SpellVisualKit, SpellVisualEffectName,
      SoundEntries, CreatureDisplayInfo, CreatureModelData, Map, AreaTable and LiquidType.
- [ ] **5c. Models:** M2 v264 with external `.skin` files and the new animation track layout.
- [ ] **5d. Terrain:** 3.3.5 ADT/WDT, including MH2O water and vertex colours.
- [ ] **5e. First content:** Arcane Blast visuals and sounds, one Northrend creature, then one Northrend zone (e.g. Borean Tundra).
- [ ] **Early shortcut to test:** Arcane Blast is a TBC spell. TBC models (v260–263) may already load, so try its effects before the v264 work.

**Done when:** a co-op run in a Northrend zone, using WotLK spell effects.

### Phase 6: Our own features (open-ended)
Items and gear, talents, new classes (Death Knight once 3.3.5 data works), more zones, achievements.

## 5. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Native Windows build doesn't work | Blocks the default plan | Phase 0 gate; fall back to the browser build |
| Netcode bandwidth with hundreds of enemies | Laggy co-op | Event-driven effects, delta snapshots, quantization; measure in Phase 2 |
| Upstream engine changes cause merge conflicts | Maintenance cost | Keep engine crates untouched; our code lives in our own crates |
| Rust + Bevy learning curve, long compile times | Slower progress | Small steps, headless sim tests, incremental builds |
| 3.3.5 format details are fiddly | Rendering bugs | Test against known models; wowdev.wiki; do it as a separate track |
| Blizzard assets | Can't publish or sell | Personal use only; never ship game data. Each player supplies their own client data |

## 6. Legal and scope

- The engine code is MIT/Apache, so we can use and change it.
- The WoW data (1.12 and 3.3.5) belongs to Blizzard. This is a **personal project to play with friends**.
  It is not to be published or sold, and the game data is never distributed.
  Each player needs their own copy of the client data.

## 7. Open questions for you

1. Where should the project live on disk, and do you want a GitHub fork?
2. How much Rust or programming experience do you have? This sets how much I explain versus just build.
3. Steam or not? Steam networking would make invites trivial, but requires a Steam app ID.
4. Do you already have a WotLK 3.3.5a client, or is that a later concern?
