# Nightfall: things to go through together

Decisions made while building overnight (2026-10-16 → 17) without you, and places where the result
may not be what you pictured. Each line says what was decided, why, and how to change it.

## Open questions

1. **The hub's inn: Goldshire or Darkshire?** The spec (commit cfa9d12) says Goldshire's Lion's Pride
   Inn in Elwynn; your message before bed said Darkshire's inn. Built as the spec says, with the inn's
   place in content (`content/nightfall/`), so moving it to Darkshire's Scarlet Raven Tavern is a
   content edit, not code.

## Decisions to check

(Added as the work goes; newest last.)

1. **The pick screen** is a 92% black veil over the live world, so WoW's own frames (player frame,
   minimap, action bars) show faintly under it. A fully opaque backdrop or hiding the WoW UI on that
   screen would be cleaner. (`crates/benilla-app/src/game_pick.rs`, `spawn_root`.)
2. **The hero's light** was barely visible at the planned strength; raised in
   `content/nightfall/rules.ron` (`hero_light`). Tune to taste: it is a content number.
3. **Nightfall's chat window** rendered as a white box (Survivors fixes this with a bit of Lua in
   `together/ui.rs`); the same fix is copied into Nightfall.
4. **The Nightfall menu is a plain panel** at the bottom of the screen over the dark woods, with the
   class buttons, Play Solo, Host, Join (paste an invite code) and Back. The inn (sub-project 5b)
   replaces it. The picked class has no highlight yet, only its line of text.
5. **Duskwood switches in at Nightfall's menu**, not at run start behind a loading screen as the
   spec says: both are map 0, so nothing loads either way.
6. **Positions to measure in the game:** entry points, exits, enemy camps, containers, the inn's
   rooms and NPC spots are placed near the known start and marked `measure:` in content. Walk with
   `NIGHTFALL_TRACE_POS=1` and paste the logged positions into the RON files.
7. **"Indoors" under trees:** the baked map counts ground under big tree and rock models as indoors
   (about 8% of Duskwood), which will muffle noise there until sub-project 3 refines it.
8. **Merged `phase-1-sim-core` into this branch** (commit 7f7c97a), so the game-pick gating covers
   Survivors' newest systems and its protocol bump (8, now in `together_sim::messages`). A schedule
   test now fails if any game system runs outside its game's set.
9. **Two Survivors-side things for you, on `phase-1-sim-core`** (left alone because you have
   uncommitted edits there): `together/tooltip.rs` keeps `shown` in a `Local`, which a VM-memo guard
   test flags; and Survivors' UI tuning replaces `ActionButton_SetTooltip` for the whole UI session,
   so after visiting Survivors, Nightfall's action-button tooltips stay silent until a `/reload`.
10. **The WoW interface is hidden at Nightfall's menu** (it is a full-screen panel with no hero
    yet) and shown in a run, as Survivors does.
