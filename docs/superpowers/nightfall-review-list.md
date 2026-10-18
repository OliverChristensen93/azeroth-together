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
11. **The browser (wasm) build was already broken before Nightfall**: `together-net`'s tokio
    `rt-multi-thread` feature in `benilla-app` (since commit 6a3218d on the Survivors line) does not
    build for wasm32. Nightfall adds nothing new that breaks it. If you want the browser build back,
    the game crates and modules need `cfg(not(target_arch = "wasm32"))` gates; otherwise drop the
    wasm job from this fork's CI. Your call.
12. **Duskwood's walls bake in about 3–4 s** on the host's first launch (a progress line shows at
    the menu) and load from a 54 MB cache in `benilla-config/nightfall/` after that. A joined friend
    never bakes.
10. **The WoW interface is hidden at Nightfall's menu** (it is a full-screen panel with no hero
    yet) and shown in a run, as Survivors does.
13. **A "?" can hang in the fog with no enemy visible under it.** Your hero "sees" an enemy out to
    the fog's end (40 yd), but in the last ~10 yd the fog and the night swallow the model, so the
    mark floats over what looks like empty ground. I kept it: a "?" in the murk reads as "something
    out there noticed you", which suits the mood. If you'd rather marks only show for enemies you can
    make out, shorten the hero's sight to the fog's start or a bit past it (a content number).
14. **Sneaking felt very safe in the test runs**: walking past the start drew three alerted enemies
    in ~20 s; sneaking drew one "?" and one "!" in the same time, and none in a 48 s run. Tune
    `visibility`/`notice_range` in `content/nightfall/rules.ron` after you've played it.
15. **On foot you can't really shake a chase yet.** Enemies run 7–8 yd/s against your 7 (4.9 in
    stealth), and an alerted one tracks you at any facing out to 40 yd, so a chaser right behind you
    sees you again at every corner. Only the Fetid Corpse can be outrun. The escapes are sub-project 4's
    Sprint, Blink and Vanish (with a test that breaking sight with Sprint sends the chaser searching).
    If that still feels unfair, slow the enemies' run speed in `content/nightfall/enemies.ron`.
16. **A sleeper woken by a noise stays "blind" until it is alerted.** It gets up and walks to the noise,
    but can't see you standing right there until something alerts it. It follows the "sleepers are blind
    until alerted" rule; it may look odd in play. Easy to change (let a woken sleeper see once it stands).
17. **A target that slips out of sight stays selected for 3 s**, then drops. Without the grace, a target
    stepping behind a tree would drop at once in a fight; without the drop, you could follow a hidden
    enemy with the target frame. The 3 s is a constant in `nightfall/conceal.rs`.
18. **Reviving works through a thin wall.** A friend within 3 yd of a downed hero revives them even
    with a wall between (the reach is flat distance). The spec just says "next to"; if it feels wrong
    in play, require line of sight or the same floor.
19. **A friend can fight while reviving.** The spec only asks the reviver to stand next to the downed
    hero for 5 s, so pressing abilities doesn't interrupt it; stepping out of reach or being downed does.
20. **A downed hero who crawls looks like they're walking.** The downed hero kneels when still, but
    WoW has no crawl animation and movement beats the kneel, so crawling (at the slow crawl speed)
    plays the normal walk. Options: root a downed hero (no crawl), slow the walk animation down, or
    accept it. The spec says a downed hero "can crawl".
21. **The death screen has no "Release Spirit".** WoW's death popup is kept hidden (Nightfall has no
    spirit to release); a dead hero sees "Your body lies where you fell" instead. Sub-project 5 decides
    what happens next (spectating the others, the end screen).
22. **A friend who joins sees enemy telegraphs a little late.** Their screen runs ~0.13 s behind the
    host plus the network delay, and the dodge's 0.3 s window starts when the press reaches the host, so
    a joined player must dodge ~0.15–0.25 s earlier by their own screen. Fixable later by drawing
    telegraphs ahead by the playback lag. Worth feeling out in a real co-op game.
23. **Casting while walking may break over the internet.** A slowed cast (Frostbolt, Polymorph) allows
    ~0.44 s for the slow to reach a joined player; above ~100 ms one-way (invite relays) a walking cast
    can break with "moved". Tune `speed_grace`/`cast_slack` in `content/nightfall/rules.ron` after a real
    online test.
24. **An elite worgen downed a lone level-30 mage in about 8 s** in the co-op test, before the friend could
    revive the rogue. Elites may be too strong for one hero; they're content numbers in
    `content/nightfall/enemies.ron`.
25. **Searching a container works through a thin wall** (like reviving): within 3 yd counts, wall or not.
26. **A dead hero who quits before the raid ends is counted "missing"** on their own client, not dead.
    Rare; worth a look if it happens.
27. **Teammates see each other's loadout in the lobby**, including where a dead hero's remains lie.
    Everyone in a lobby is on your team, so I kept it.
