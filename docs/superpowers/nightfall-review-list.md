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
