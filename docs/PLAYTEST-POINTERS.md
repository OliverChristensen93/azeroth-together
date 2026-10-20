# Playtest pointers, 2026-10-18: patch notes and open decisions

Branch `playtest-pointers` (worktree `D:\Games\azeroth-together-pointers`), cut from
`phase-1-sim-core`. It doesn't touch the Nightfall work (`stealth-extraction`). Part 1 is what
changed, Part 2 is what I decided on my own and you should check, and Part 3 is what isn't done.

All numbers below that live in `content/*.ron` can be changed without a rebuild.

## Part 1: Patch notes

### Enemies and the horde
- **Enemies are faster.** Every enemy's speed is ×1.35, capped at 6.65 yd/s (95% of a hero's
  7). A Defias goes from 4.2 to 5.7 and a boar from 4.6 to 6.2. Wolves go from 5.4 to the 6.65 cap.
  Bosses get the same: Hogger goes from 4.4 to 5.9. Set in `rules.ron` (`enemy_speed`,
  `enemy_speed_cap`). Before this change, enemies ran at 50-80% of your speed.
- **The horde comes from behind you.** Spawns now follow the way you have been running. About
  55% come from behind, 35% from the flanks, and the rest from ahead and to the side. Nothing
  spawns within 35° of straight ahead, so running forward, or diagonally forward, stays open.
  Standing still, they come from behind where you last ran.
- **Surrounds.** From 2:30, every 2:30, a 12 s "Surrounded! They come from every side" event
  sends the horde from all directions, 30% thicker. Set in `rules.ron` (`surround_from`,
  `surround_every`, `surround_secs`).
- **Enemies no longer walk through trees, rocks, fences or buildings.** The host's client scans
  the arena once, a yard at a time, against the game's collision: 300 cells a frame, about 5 s
  for an Elwynn-sized arena. It rescans while new colliders stream in. An enemy walks straight
  at you when the line is clear. Otherwise it follows a path around the obstacle (a flow field
  refreshed 2-3 times a second) and slides along walls. Spawns avoid blocked spots.
  Bosses' charges and leaps still go straight.

### Bosses, champions, elites
- **Hogger no longer respawns straight away.** The next boss used to come every 4:00 on a fixed
  clock, so a long fight brought a fresh 100% Hogger the moment you killed the old one. Now there
  is one boss at a time, and the next comes 4:00 after the last one falls.
- **Boss entrance.** When a boss arrives, the run pauses behind a panel with its name, level and
  every mechanic. Continue (or Enter or Space) resumes. In co-op, the run waits until everyone
  has pressed Continue.
- **A smaller boss bar.** It shows the name and health only. Hover over it to see the mechanics
  list. Click it to make the boss your target.
- **Champions (mini-bosses).** A champion comes at 2:00 and then every 4:00 (6:00, 10:00,
  14:00), halfway between bosses. It's an elite of a kind three tiers above the horde, with 3×
  an elite's health (more with more heroes) and 1.3× its size. It uses its kind's mechanics,
  has a silver-dragon frame titled "Champion", and leaves a cache when it dies. A warning
  announces it. Set in `rules.ron` (`champion_from`, `champion_every`).
- **Elite camps are worth visiting.** Camp elites used to be the horde's strongest current
  kind, which at the start is the weakest enemy (a Young Wolf elite). That's probably why it
  felt like there were no elites. Camp elites are now two tiers above the horde. Camps were
  never marked anywhere; now every elite shows on the minimap.

### The minimap and the ground
- **Minimap marks:**
  - the boss: a skull, kept on the minimap's rim when it's out of range
  - champions: a large orange dot, also kept on the rim
  - elites: red dots
  - the War Banner's destination: a red flag, kept on the rim
  - a War Banner waiting to be picked up: a green flag
  - caches from bosses, challenges and banners: gold dots, kept on the rim
- **Arena boundary.** The edge of the arena is a red-orange ring on the minimap and a glowing
  fence on the ground that follows the terrain and fades upward.
- **Shrine circles.** Every shrine, altar, chest, idol and banner shows its activation range as a
  coloured, pulsing circle on the ground. A disc inside the circle fills as you stand in it.
  The banner's destination gets the same circle. Colours: blue titan shrine, gold golden
  shrine/chest/cache, purple altar, red idol, green banner. Power Word: Barrier is a Cataclysm
  spell and isn't in the 1.12 data, so this is a mesh, like the boss telegraphs.

### The War Banner
- The destination is on the minimap (above) and has its own ground circle.
- **Camera fix:** carrying the banner no longer zooms the camera to first person. The banner
  model sits at your feet while carried, and its collision hull was around your head. The
  camera's collision sweep hit it every frame and snapped in. The carried banner now has no
  collision.

### HUD
- **Movement ability button.** Your F ability (Blink, Charge, Vanish...) has its own icon above
  the left end of the action bar, with the key "F" in the corner and a cooldown shade plus
  seconds while it recharges.
- **Cooldown manager.** Under your hero there is a row of small icons for spells that depend
  on where you stand: cones, and novas or auras around you. Only spells with a base cooldown
  over 4 s are shown, with a shade and seconds left. Change the threshold with `TOGETHER_CD_MIN=<s>`.
- **Stat sheet while choosing.** The stat sheet opens by itself while a level-up, shrine or chest
  hand is up, and closes after. If you opened it with Tab, it stays open.
- **Talents panel.** A "Talents" button sits next to "Stats" (key N). It lists every bonus your
  talents give, one line per kind of bonus with all nodes of that kind added up: three +1% damage
  nodes read "+3% damage". Keystones come last with their own lines.
- **Damage meter.** Click the title to cycle Damage Done → Boss Damage → Healing & Absorbed.
  "Reset" counts from now. Boss damage, healing and absorbs are new numbers the sim now keeps
  for each hero:
  - Healing is read from health gained, so every heal counts: regen, leech, spells, shards.
  - A heal your spell lands on a friend counts as yours.
  - Absorbed is what shields and wards soaked up.
- **Run panel warnings** for surrounds and champions.

### Level-up and treasure screens
- A slowly turning burst of light rays and a breathing glow behind the cards. They're gold for a
  level-up or treasure, blue for a titan shrine, purple for an altar or augment, and red for an
  idol.
- Cards pop in one after another, with a little overshoot.
- A chest or cache hand comes with a shower of falling, spinning gold coins.

### Escape and leaving a run
- **Escape pauses the run** while WoW's game menu, or a panel opened from it, is up. In co-op it
  pauses for everyone, like the card hands do.
- **"Logout" is now "Main Menu".** It asks first: "Exiting to the main menu will end the run, but
  you keep the rewards for your progress so far." Then it ends the run and goes back to the menu.
  Renown and talent points are awarded on leaving, and gear found was already saved as it
  dropped. A host takes the party back to the lobby. A joined player leaves only themselves.

### Heroes
- **Base health regen per class**, before any tome: mage, priest and warlock 0.5/s; hunter
  0.7/s; shaman and druid 0.8/s; paladin and rogue 1.0/s; warrior 1.2/s. It's in
  `classes.ron` (`base_regen`), and the stat sheet and menu show it.
- **Racial passives** (`content/races.ron`), two each, based on the Classic racials:
  - Human: Perception (+3% crit), The Human Spirit (+0.4 regen)
  - Orc: Blood Fury (+6% damage), Hardiness (+4% armor)
  - Dwarf: Treasure Finding (chests, shrines and pots on your minimap, +15% gold), Stoneform (+5% armor)
  - Night Elf: Quickness (+6% move speed), Shadowmeld (+3% armor)
  - Undead: Cannibalize (+0.6 regen), Will of the Forsaken (+10% crit damage)
  - Tauren: Endurance (+5% max health), War Stomp (+8% area)
  - Gnome: Expansive Mind (+8% XP), Engineering Specialization (+4% cooldown speed)
  - Troll: Regeneration (+0.5 regen), Berserking (+4% damage)

  The menu lists your race's racials under the class text.
- **Tome text:** "Raises your armor for the run." is now "Raises your armor."

## Round 5

- **Healing by spell.** The meter's Healing & Absorbed view now has a row per spell, plus
  "Regeneration & other" (regen, shards, level-up heals) and "Absorbed".
  - Each spell cast, triggered cast and landing missile counts the health it gave any hero as
    that caster's spell's healing, a heal on a friend included. It does this by reading health
    before and after, so every heal path counts without touching each one.
  - A heal over time remembers whose spell laid it.
- **Co-op credit for late missiles.** A heal from a missile that lands later now counts for its
  caster, not the hero who received it.
- **Swarms halfway to each boss.** The repeating 2:30 surround timer is gone. One swarm comes
  halfway to each boss: at 2:00 before the first, and 2:00 after each boss falls. None comes
  while a boss is up.
  - The run panel shows one timer, "Swarm in 1:33", then "Hogger in 2:00".
  - The warning says "Swarm!".
  - `surround_secs` (12 s) is still in `rules.ron`; `surround_from` and `surround_every` are gone.
- **Weather and graphics:** moved to its own session and branch (`weather-graphics`).

## Round 4: four enemy ranks

Champions are gone. The enemies are now:

| Rank | What | Health | Size | Target frame |
|---|---|---|---|---|
| Normal | the horde | 1× | 1× | plain |
| Rare | about 1 in 70 horde spawns from 1:30; a Challenge Idol raises 2 (one more per extra hero) | 15× its own kind | 1.3× | silver dragon |
| Elite | the camps' guards: fixed spots, 20 yd aggro, 35 yd leash, evade home and heal; back 3 min after a kill | 30× its own kind (two steps above the horde) | 1.6× | gold dragon |
| Boss | the zone's boss, the next 4 min after the last dies | 50× the horde's strongest kind at the time; the **3rd** boss (the last) 75× | at least 2× (bigger models keep their own) | gold dragon with skull |

- All the numbers are in `rules.ron`: `rare_health`, `elite_health`, `boss_health`,
  `final_boss_health`, `final_boss`.
- Rewards: rares 5× XP and gold, elites 10×. Bosses leave a cache, as before.
- Minimap: elites are larger red dots ("(elite)" on hover), rares smaller pale dots ("(rare)").
- **Decided:** rares stay at 15× (`rare_health` in `rules.ron`).
- **Decision for you:** "the last boss" is the third boss of a run. The run's clock goes on
  after 15 min, so any boss after the third is also 75×.
- **Decision for you:** Gurubashi Arena has no elite camp now. With its 30 yd leash, a 30×
  elite would stand within reach of the start, and a paladin's elite-seeking spell spent a whole
  test minute on it without killing anything.
- **Change:** aimed spells now treat a camp elite at rest (full health, guarding) as the last
  choice, so they don't pull a camp on their own. Once it's in a fight it's a normal target.
  Area spells still hit it if you stand close.

## Round 3

- **Talent tooltips** showed raw colour codes (`|cffffc800…|r`). The hover text was written
  straight into the label instead of being turned into coloured text. It's now parsed into
  coloured spans. (A bug from the talent-web merge, not seen on screen since: needs a mouse
  over a node.)
- **Minimap hover names.** Every mark this mode puts on the minimap has a name in the
  minimap's own tooltip, like a tracked NPC: "Hogger (boss)", "Mangy Wolf (champion)", "Defias
  Thug (elite)", "War Banner: carry it here", "Cache", and for a dwarf every shrine, chest and
  pot by name. (Not seen on screen: needs the mouse over a mark.)
- **Elite target frames.** Elites, champions and bosses never showed WoW's elite dragon. Our
  creature guids lacked the creature entry in the bits the client reads the creature record
  from, so the target frame looked up "creature 0" and read rank normal. The name came out
  right only because names use another field. Guids now carry the entry: elites and bosses get
  the gold dragon, and champions the rare-elite one. Seen on screen: Hogger's target frame has
  the dragon.
- **Health.** Elites have 8× the health of an ordinary enemy of their own kind (was 4×).
  Champions have 15×, and bosses 50×, the health of the horde's strongest kind at that moment.
  So a boss keeps pace with the run however late it comes. The bosses' own `hp` in
  `enemies.ron` is no longer used. All three numbers are in `rules.ron` (`elite_health`,
  `champion_health`, `boss_health`). Decision for you: you said "elites/mini-bosses 150 for
  a 10 hp enemy". I gave camp elites 8× rather than 15×, so the camps don't become as
  tough as champions; raise `elite_health` if you want them equal.
- **Crowd control on elites and bosses.**
  - Stuns, freezes, roots and fears last a quarter as long on elites and bosses. Elites used
    to get half.
  - Bosses used to be immune to stuns and fears; now they take them at a quarter length too.
  - Slows hit elites and bosses at half strength. Their duration was already shortened.
  - Capstones that promise "full control" on elites still do.

## Round 2: your answers, done

- **Fix: the game showed only the sky** when started from
  `D:\Games\azeroth-together\target\release`. A dev build looks for the game data in the folder
  it was built from, and this worktree has none. The launcher now uses the main checkout's
  `WoW\Data` when it's started from a worktree build without `WOW_DATA` set
  (`crates/azeroth-together/src/main.rs`).
- **4. Champions and camp elites keep to their spot.** A champion now appears at a spot of its
  own, as far from the heroes as the arena allows, and stays there. It's marked on the minimap
  (an orange dot, kept on the rim), and its warning says to find it there. Camp elites and
  champions behave like WoW mobs:
  - They fight a hero who comes within 20 yd or who hits them.
  - Past 35 yd from their spot they give up, run home at 1.6× speed, take no damage on the way
    (evade), and heal to full when they arrive.
  - `CAMP_LEASH` is in `world.rs`.
- **5.** The boss entrance waits for everyone's Continue. Continue now counts as your Ready.
- **6. Pause and Ready.** Escape pauses the run for everyone. Once your menu closes, a
  "Paused" panel shows a Ready button (Enter or Space) and who the party is still waiting for.
  The run goes on when everyone is ready. Playing alone, it goes on as soon as the menu closes.
- **8. Main Menu shows the results first.** Confirming ends the run and brings up the results
  screen with the renown earned. Enter then goes to the menu.
  - A host ends the run for the whole party.
  - A joined player's hero falls for good, so it can't be raised. That player sees their own
    results and leaves; the others play on.
  - The popup reads "Exiting to the main menu will end the run, but reward you for your progress
    so far."
- **10.** Every level-up heals each living hero for 10% of its max health.
- **12. Keys.** WoW's own talent and character frames are off. The stat sheet moved from Tab to
  **C**, and the talent sheet is on **N**. The Controls help lists both, plus Esc.
- **18.** "For the run" is gone everywhere, including the item cards' "Kept for the rest of the
  run."
- **Seen working since:** the banner camera fix. A test frame shows the hero carrying the banner
  with the camera at its normal distance, and the destination flag on the minimap.
- **Not seen on screen:** the Ready panel in co-op and the Main Menu results screen. Sim tests
  cover pausing until everyone is ready and giving up for good (423 passing).

## Part 2: Decisions to review

Answered: 4, 5, 6, 7 (yes), 8, 9 (fine for now), 10, 11 (fine for now), 12 and 18 (see Round 2).
Still open: 1-3, 13-17 and 19.

1. **Enemy speed: ×1.35 with a 6.65 yd/s cap rather than full player speed.** You remembered WoW
   mobs as running at player speed, and they do. But in a survivors game, enemies at 7.0 mean a
   hero without speed tomes can never open a gap, and every kite turns into a slow bleed.
   To try the full WoW feel, set `enemy_speed: 1.6` and `enemy_speed_cap: 7.0` in `rules.ron`.
   Pacts that speed up enemies still stack on top of the cap.
2. **Spawn direction split:** 55% behind, 35% flanks, 10% ahead-and-aside, with 35° straight
   ahead kept clear. The constants are in `world/horde.rs`; I can move them to `rules.ron` if
   you want to tune them.
3. **Surround events every 2:30 for 12 s.** Is that too often, too long, or should it be tied to
   something like a shrine or an idol instead of a clock?
4. **Champions:** 2:00 and every 4:00, 3× elite health, three tiers up, and a cache drop.
   Do you want named rares from each zone instead, like Mother Fang or Gruff Swiftbite? The
   zone comments list candidates.
5. **Boss entrance pauses co-op until everyone presses Continue.** Alternative: only the host's
   Continue counts, or there's no pause in co-op.
6. **Escape pauses co-op for everyone.** Same question. A friend sitting in the Options panel
   holds the whole party.
7. **Main Menu as a joined player** leaves only you; the host and the others keep playing.
   Main Menu as host ends the run for everybody.
8. **Leaving gives renown for the run so far, with no results screen.** I could show the results
   screen first: the run would count as lost and you'd press Enter as usual.
9. **Base regen numbers**: 0.5 cloth to 1.2 warrior. You asked about 0.5 for casters; the rest
   are my guesses.
10. **Health on level-up: not added.** Heroes still get no health from levelling; max health
    comes only from Tome of Fortitude, items, pacts and talents. Should levelling give a little,
    say +2% max health a level?
11. **Racial numbers and choices** are mine. The Dwarf "Treasure Finding" minimap marks are
    the only non-stat racial. Others that would be fun but need new code:
    - Night Elf Shadowmeld as a real "standing still makes you harder to target"
    - Undead Cannibalize healing from corpses
    - Tauren War Stomp as an actual stun
12. **Talents key N** also opens WoW's own talent frame binding if it's there; I haven't seen
    that in a run. Tab for Stats already shares WoW's target-nearest key.
13. **Cooldown manager rules:** "positional" means a Cone tag, Around or Ahead targeting, or a
    nova with no target of its own. The 4 s threshold is checked against the base cooldown from
    `abilities.ron`, not the cooldown after tomes. It sits under the hero at 60% of the screen
    height. Is that where you want it, or should it be a settings panel instead of an env var?
14. **The F button position** (left of the action bar, 96 px up) is a guess until you see it
    on your screen.
15. **Obstacle scan:**
    - A ray at 0.9 and 1.6 yd above the ground.
    - Slopes steeper than about 40° count as walls.
    - Shrines and banner models count as obstacles while they stand.
    - Only the host scans; co-op clients mirror.
    - Big bodies (bosses) path on the same 1-yard grid, so they can brush through narrow gaps.
16. **Fence look:** red-orange, 1.6 yd tall, fading upward. You might prefer something
    subtler, like a line of rune decals, or nothing on the ground and only on the minimap.
17. **The level-up dimming** moved from the card screen to the rays layer: 0.55 black instead of
    0.62.
18. **Kept "for the rest of the run"** on item cards ("Kept for the rest of the run."). You
    mentioned tomes only.

19. **Early difficulty.** In a test run where the hero stood still (`TOGETHER_STAND=1`), the faster
    horde overwhelmed a level-3 hunter at 0:21. Wolves, the first enemies in Elwynn, hit the
    6.65 cap. If the first minute feels too punishing, lower `enemy_speed`, or lower only the
    weak wolves' speed in `enemies.ron`.

## Part 3: Not done, or not verified

Seen working in background test runs (screenshots): the boss entrance and small boss bar,
ray bursts (gold, blue, purple) with popping cards, the stat sheet opening with cards up, the
Talents button, the F button, the cooldown manager, shrine circles, the arena fence (Gurubashi)
and minimap ring, minimap marks, the champion warning, racials (orc +6% damage / 4% armor,
human crit and regen), the obstacle grid (a dump of Elwynn shows trees, the Goldshire building
and fences), and the Main Menu Lua wiring (logged at startup). Sim tests: 419 passing.

Not seen on screen:

- **The coin shower**: the autoplay hero never opened a chest in the test runs.
- **Escape pause and the Main Menu popup**: a background test window can't take key presses.
  The wiring logs that it's installed.
- **The banner camera fix**: it relies on the explanation above, and I didn't repeat the carry.
- **Meter Reset and view switching** (clicks).
- **Boss-bar click to target** (it uses WoW's `TargetByName`).
- One test run exited 15 s in with code 1 and no crash report; the same run again was fine.

Also:

- **Co-op not tested** with two instances. Things that ride the network:
  - the menu pause
  - the boss intro hold
  - champions
  - surrounds
  - the meter's new numbers in snapshots
- **Healing per spell** isn't tracked. The healing view shows Healing and Absorbed totals per
  hero, not per ability.
- **Heals from missiles that land later** (not in the cast step) count as the receiver's own
  healing, not the caster's. That only matters in co-op.
- **Obstacle quality** depends on what the collision has, and narrow gaps between trees may
  read as walls. Watch for enemies piling up behind something. To see what the scan found,
  set `TOGETHER_OBSTACLE_DUMP=grid.pgm`; it writes the grid as a picture, north up.

## Playing this branch

Run **`D:\Games\azeroth-together-pointers\target\release\azeroth-together.exe`**. This branch
eleasezeroth-together.exe`**. This branch
has its own build folder now. The main checkout's `target\` is shared with other branches (the
weather session builds there), and two checkouts in one build folder overwrite each other's
compiled code and exe.
- The exe reads this worktree's `content\` and borrows the main checkout's `WoW\Data`.
- It uses your usual saves in `D:\Gameszeroth-togetherenilla-config`. My test runs used a
  scratch folder (`BENILLA_HOME`) and left your saves alone.
- To rebuild, run `cargo build --release -p azeroth-together` from the worktree, with no
  `CARGO_TARGET_DIR` set.
