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
28. **Item names are our own, but plain.** Seven that were exact WoW item names were renamed (e.g.
    "Stale Black Bread", "Small Healing Draught"); plain descriptive ones stay ("Silver Locket", "Worgen
    Pelt", "Bone Fragments", "Notched Shortsword"…), and a few may coincide with WoW's own. Keys don't
    change, so renaming any is a one-line edit in `content/nightfall/items.ron`.
29. **Abandoning a raid (or dropping out) counts as "missing": you lose what a death would, with no
    body to recover.** It's the plan's rule so quitting can't save your loot, and abandoning is no better
    than a disconnect. A dead hero whose player leaves before the raid ends also loses the brought items
    they'd otherwise keep. If you'd rather be kinder to someone whose connection drops, this is the knob.
30. **The free starter kit can be farmed a little.** Raid with an empty loadout, extract, sell the kit
    (~8 gold), repeat. The plan accepted it (kit items are worth 1 gold each); set their `value` to 0 in
    `content/nightfall/items.ron` if you want it gone.
31. **In a friend's lobby your stash is locked.** Once you've joined and sent your gear to the host,
    the stash panel won't move or sell anything until you leave the lobby — even between raids. It stops
    a start racing an edit (which could copy or lose an item). A finer lock is possible later.
32. **Selling has no "are you sure?".** Shift-click sells a whole stack at once, rare or not. A confirm for
    rare-and-better items is easy to add if you'd like one.
33. **A "Ready" button would be the better lobby lock** (item 31): edit freely until you press Ready, the
    host can only start when everyone is Ready, and only a Ready player is locked. A natural fit for the inn
    (sub-project 5b).
34. **One game per stash folder.** A second Nightfall started on the same save folder opens the stash
    read-only (no raids), so two windows can't overwrite each other. If the folder can't hold the lock file,
    the stash runs unlocked (rare). For co-op tests on one PC each window needs its own `BENILLA_HOME`.
35. **The companion is a wolf** (display 903, a timber wolf). The plan had an ogre's display by mistake.
    Any creature display works; it's one number in `content/nightfall/hub.ron`.
36. **A "Ready" button in the lobby** (item 33) is being built with the inn: friends can change their
    loadout until they press Ready, and the host starts only when everyone is Ready.
37. **Names: WoW's Duskwood characters where the spec names them, our own elsewhere.** The contract givers
    are the spec's Commander Althea Ebonlock, Madame Eva and Abercrombie (and Mor'Ladim's grave, Stitches);
    the elites Fenros and Naraxis are Duskwood's own too. The innkeeper and trader, which the spec doesn't
    name, got our own names, and items got plain names of our own (item 28). Say if you'd rather use WoW's
    names everywhere (e.g. Innkeeper Farley) or nowhere.
38. **Selling moved to the trader** (updates items 31 and 32). The stash's Shift-click no longer sells;
    the inn's trader buys and sells. Buying and selling touch only your stash and purse, so they still work
    after you press Ready; turning in a contract that takes from your loadout, or a companion upgrade, waits
    until you un-Ready. One click on a trader row still sells the whole stack with no "are you sure?".
39. **The inn's places were measured from the building's data, not by walking it.** The bounds, the door
    and the 11 NPC spots sit inside Goldshire inn's hall (worked out from the inn model's placement and room
    boxes); the hero starts on the dining-room floor. NPCs may stand in furniture: one walk-through would
    tell. All in `content/nightfall/hub.ron`, marked `measure:`.
40. **Panels: 360 px wide, below the inn bar at the top centre.** The banker's strip clears the stash panel
    only at about 1280 px wide or more; a tall station keeper's panel scrolls.
41. **WoW's own gossip and merchant windows can be fed without a server** (the spike, Task 12, notes in
    `docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md`): both drew in shots. The bank and bags
    would need us to answer item moves like a server (days of work), so the inn keeps its own panels. Switching
    the trader and givers to WoW's windows is a later choice.
42. **Ready is pressed at the raid board by the door**, which shows the map, the contracts taken, the party and
    who is ready. If the stash can't be saved when a raid starts (another window holds it), Ready is taken
    back instead of retrying.
43. **The Westfall road is never shut or late** (the inn's road home; Mor'Ladim's tome contract needs it).
    The other gates roll open, late or shut, and the hunt may close one. A `pinned` flag on the exit in
    `content/nightfall/map.ron`; take it off to let it roll too.
44. **The night's sounds are stand-ins from the trimmed data.** No bell exists, so Moonrise is silent (its chat
    line still shows); the worgen cue is a worgen attack, the abomination's a flesh golem's, Stitches' tread a
    giant's footstep, the ravens a bat's cry. Any sound you add to the data can replace them (`monsters.ron`).
45. **Looks from the data:** the banshee is WoW's banshee (display 915), Stitches a bone golem (12073), the
    Dark Riders ride the undead warhorse (10718); the trackers are ordinary-looking worgen.
46. **The roads were re-walked on the real ground** so Stitches and the riders can follow them: they are
    walkable routes, not exactly the drawn roads. Worth a look.
47. **A hunter skips a noise it can't find a way to within its search budget** (about 1–2% of long winding
    routes): it stays at its post rather than stall the game. Measured worst step: ~15 ms of a 33 ms tick.
48. **A downed hero's crawl keeps laying scent**, so a tracker can follow you across the crawl; only the dead
    lay none.
49. **Mor'Ladim gets a telegraphed cleave** (every elite telegraphs); being watched stops it mid-swing.
50. **Rivals travel by measured routes**: from their entry to a point of interest (now only Vul'Gol and
    Brightwood, where real chests stand) and on to an exit. The other places the plan named were too far or
    unreachable on the trimmed map. A party that can't get anywhere for a minute slips away while none of you
    sees it, taking its packs. Routes and places are in `content/nightfall/rivals.ron` (`measure:`).
51. **Every rival looks like a human in starting clothes** with the rogue's daggers; per-kind looks (a
    shortsword, a gravedigger's boots) are a later choice. The deserter's shot uses WoW's "Shoot (TEST)" spell.
52. **You only get the rivals near your hero over the network** (as with monsters); they're hidden until you
    see them, including their damage numbers and marks.
53. **Rivals count as bodies in the night:** their gaze holds Mor'Ladim, they scare ravens, the hunt hears
    their fights, and monsters arrive unseen by them too.
54. **A rival that hit you is in your fight:** no regeneration and no stealth while one is after you. Kill
    credit goes to whoever struck last.
55. **A looted rival's gear counts as found**, so it's lost on a death that loses finds.
56. **Sub-project 8, the playtest kit, was added by me** after the spec's seven were done (you asked me to keep
    going): no new design, only what makes playtesting easier. Plan: `docs/superpowers/plans/2026-10-20-nightfall-playtest-kit.md`.
57. **The Playtest panel (inn bar) sets the spec's open questions per raid**: the death rule, rival kits (simple or
    full), rivals on/off/always one, which signature monsters, the night clock ×1/×2/×4, the weather. Host-side;
    a joiner's panel says the host's apply. The ×2/×4 clock speeds every timer tied to the night (cooldowns, the
    hunt's memory), not movement. Saved in `benilla-config/nightfall-playtest.txt`.
58. **Selling a rare-or-better stack asks "Sure?"**; common stacks still sell in one click.
59. **Autoplay now sneaks, loots and leaves**: it heads for an exit once the area is looted out (about 30 s in)
    or its bags are half full, at full speed, turning aside when stuck. A test tool, not a bot to play with.
