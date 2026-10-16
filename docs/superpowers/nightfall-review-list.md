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
