# The talent web

Progression between runs, as agreed in [DESIGN-IDEAS.md](DESIGN-IDEAS.md) section D and rows 9b,
9h, 9i and 2g: **one Path of Exile-style web shared by every class**. Built on the `talent-web`
branch (first version, 2026-10-16). Every number here is a first guess, meant to be tuned by
playing; everything marked **Review** is a decision made without you.

## What is built

| Part | Where |
|---|---|
| The web as data: 235 nodes, links, effects, the point rules | `content/talents.ron` (read from disk at start, so edits need no rebuild) |
| The model: reading and checking the web, allocation rules, bonuses, the saved profile | `crates/together-sim/src/talents.rs` |
| Where talents act in a run: casts, hits, kills, card offers, augment screens | `crates/together-sim/src/world/combat/talents.rs` (hooks of one line each in `world.rs` and `world/combat.rs`) |
| The Talents screen, saving, renown at the end of a run | `crates/benilla-app/src/together/talents.rs` |
| Co-op: the allocation in the lobby pick | `together_net::HeroPick::talents`, `PROTOCOL` 6 to 7 |
| The first layout | `scripts/talent_web.py` (it wrote `talents.ron`; the RON is the source of truth now, and running the script overwrites it) |
| Tests | `crates/together-sim/tests/talent_web.rs` (the shipped web), unit tests in both sim files |

## Points: renown and talent levels

- A run earns **renown** for the class played: **10 per party level past the first, 10 per minute
  survived, 1 per 25 kills** (`together_sim::renown_for_run`). A 15-minute run to level 35 is
  about 540; an 8-minute death at level 15 about 240.
- Renown makes **talent levels**: the first level costs 100 renown, each next one 30 more
  (`points: (max: 20, first: 100, step: 30)` at the top of `talents.ron`). **One point per level, up
  to 20 points.** Twenty levels take 7,700 renown: roughly 15 good full runs, or 30 shorter ones.
- The end-of-run screen shows the renown earned, the talent level and any new point. A run the
  host ends early still counts for what it reached.
- **9h, the setting** (on the Talents screen, "Per Class / Shared"): **per class** (the default)
  counts each class's own renown; **shared** counts every class's renown together, so every class
  has the points the whole account earned. Either way each class keeps its own allocation, since
  each starts in its own place on the web. Both are built; switching is free and loses nothing.
- **Review:** renown is earned the same in test runs (the Dev panel's experience multiplier makes
  levels come faster, so test runs give more). The numbers in `points` are the main knob.

## The web's shape

```
                       warrior
              paladin             rogue        (nine starts on a ring)
        priest       inner ring        hunter
              warlock             druid
                  mage      shaman
```

- **Nine class starts on a ring**, in an order where neighbours share something: warrior, rogue,
  hunter, druid, shaman, mage, warlock, priest, paladin (and back to warrior).
- **Each class's own tree** grows outward from its start: a root node, then two branches of three
  smalls and a notable each (one toward each neighbour, themed on what it shares with it), joined
  at a **deep notable** on the rim. Each branch's middle small is a utility node (pickup range,
  experience, luck, gold, regeneration or health), so the way to a notable costs something.
- **A spoke** of three smalls runs inward from each start to the **inner ring** (18 generic nodes
  and three notables: Quick Study, Fortune's Favour, Fleet of Foot), which joins every class.
- **Between each pair of neighbours lies a shared cluster** (9 of them): two entries (one from
  each class's side node), an inner node with its own way to the inner ring, a **core notable**, an
  outer small, a **Lore** node that steers card offers, and on the rim a **keystone** (7) or a big
  notable (2). The neighbours' branches also link into the cluster's outer node, so regions
  overlap and every class can walk into the others' trees: a mage reaches Shadow through the
  warlock side, Nature through the shaman side.
- 235 nodes: 9 starts, 177 smalls, 42 notables, 7 keystones.

Graph rules (all tested):
- A node can be taken when it touches the class's start or a taken node. **Clicking any node in
  reach takes the shortest way to it**, if the points cover it (as in Path of Exile).
- **Other classes' starts are walls**: no path runs through them.
- **One keystone per loadout**, and keystones are dead ends (no path runs through one). **Review:**
  Path of Exile allows several; one keeps each keystone a build's defining choice and keeps the
  power budget (below) honest.
- A taken node can be given back when everything else still connects to the start without it.
  **Respec** clears the loadout. Both are free.
- **Three loadouts per class**, switched on the screen; each remembers its nodes.

## What talents do

| Kind | Examples | How it reaches the sim |
|---|---|---|
| Stat smalls | +1% damage, +1% cooldown speed, +2% area, +1% move speed, +3% max health, +0.2 regeneration, +1.5% armor, +8% pickup, +4% luck, +3% experience, +5% gold, +0.5% crit, +5% crit damage | Added to the hero's `Stats` when the run is built, exactly like a tome |
| Tag smalls | +1% Fire damage, +0.5% Melee crit, +5% duration of damage over time, +2% area with Area spells | `Tagged(tag, bonus, amount)`: applied in `World::spell_spec` **after augments**, so a Frostfire conversion collects both Fire and Frost bonuses |
| Hybrid notables | Frostfire Mastery: +6% damage with spells that are both Fire and Frost; Eclipse, Storm Earth and Fire, Shadowflame Pact, Twilight, Vengeance: +2% damage per spell of the other school you know (up to 3), both ways | `Hybrid` and `Pair`: mixing is better than going pure, on purpose |
| Mechanic notables | Fanned Flames: Fire crits deal 40% of the hit again over 4 s (Ignite). Deep Winter: Chills have a 15% chance to Freeze. Shattering Cold: +25% crit chance against Frozen enemies. Thermal Shock: Fire hits on Chilled and Frost hits on Burning enemies +20%. Deep Wounds, Serpent's Bite, Shadow Weaving: the same burn for Physical, Nature, Shadow crits. Divine Fury: two Holy crits in a row cast your shortest-cooldown other Holy spell (4 s internal cooldown). Spirit Link: single-target hits heal the most wounded ally within 30 yards for 10%. Murder, Mortal Wounds: execute damage. Battle Trance, Rapid Fire, Crusader: move speed on kill | Hooks in `strike` (hit scale before the crit, burns and heals after) and in `spell_spec` (freeze chance, crit against the Frozen, execute) |
| Lore nodes (2g: steer, don't add power) | "Fire spells are offered 30% more often"; "Ignite can be offered on any Fire spell"; "Shatter can be offered on any Slow spell"; "Echo can be offered on any Projectile spell"; "Concussive can be offered on any Melee / Area spell"; "Executioner on Damage over time spells"; "Frostfire is offered twice as often" (Frostfire Mastery); "Contagion twice as often"; +1 reroll | `OfferWeight` leans the level-up hand's spell cards; `AugmentWeight` and `Unlock` lean and widen the general augment screen (other augment rules still hold: one conversion, one cone shape, nothing twice) |
| Keystones | Elemental Equilibrium, Glass Cannon, Steady Hand, Point Blank, Juggernaut, Slow Burn, Seal of Light | See below |

**The keystones:**
- **Elemental Equilibrium** (shaman and mage): your hits mark the target with their schools for 4
  s; your next hit on it deals +25% if it brings a school the mark lacks, -25% if not. Works for
  every school, Physical included, so it rewards alternating spells.
- **Glass Cannon** (mage and warlock): +40% damage, -50% max health (the design's own numbers).
- **Steady Hand** (rogue and hunter): your spells never roll crits (augments that always crit,
  like Shatterbolt and Ignition, still do), +20% damage. It cancels every crit node: the cost.
- **Point Blank** (warrior and rogue): +30% damage within 10 yards, -20% beyond 20.
- **Juggernaut** (paladin and warrior): +25% armor, +30% max health, -15% move speed.
- **Slow Burn** (warlock and priest): burns deal double, hits -25%.
- **Seal of Light** (priest and paladin): your hits heal you for 1% of their damage, -30% max
  health.

**The Frostfire benchmark** (section D) is supported and tested twice (in the sim with a
hand-made web, and on the shipped web): a mage takes Deep Winter (Frost branch), Shattering Cold
(the shaman and mage cluster), Fanned Flames (Fire branch) and Frostfire Mastery (between them) for
**9 of 20 points**. Then Blast Wave's slow can Freeze; Frostfire Pyroblast crits the Frozen and
Ignites; Frostfire Bolt slows, so it freezes too; Hot Streak (an augment the run gives) casts the
augmented Pyroblast. Glacial Lore ("Shatter can be offered on any Slow spell") and Frostfire
Mastery's "Frostfire twice as often" steer the run toward it without deciding it.

Classes whose spells are mostly one school (warrior, rogue) find most of their tag nodes in their
own tree and the clusters beside them; a class's start does not lock it in: all 235 nodes are
reachable from every start.

## The power budget (9i)

Talents start at 0% (no points) and grow to a full web of 20 points. Agreed budget: talents and
gear together about 40% late. The test `a_full_web_stays_within_the_power_budget` estimates power
with a rough rule (`TalentEffect::power`): damage, crit and cooldown count fully, area half,
survival a quarter to a half, burns and procs by an expected crit rate, steering nothing. It then
chases a full web greedily (the most power per point, again and again) for a class's whole spell
pool:

- whole spell pool, per class: about 0.31 to 0.47, mean 0.40 (asserted at most 0.50);
- one spell alone, every condition counted as met: up to 0.62 (asserted at most 0.70).

**Review: this is above the talents' share.** The estimate is generous (it counts survival as
power and every conditional, like Thermal Shock or Point Blank, as always on), so the real gain of
a full web should be lower, perhaps 25-30%. But gear is not built yet; when it is, either lower
`points.max`, or scale the node numbers in `talents.ron`, and keep the test's limits in step. I
went from 40 to 30 to 24 to 20 points while tuning; 20 keeps notables feeling like notables.

## Co-op

- Each player's allocation for the class they picked goes in the lobby pick
  (`HeroPick::talents`, node indices), so changing talents in the lobby updates the host at once.
- The content hash now includes `talents.ron`, so host and guests always read the same web; a
  guest with a different web is refused as "different game data".
- The host gives every hero its talents in `Sim::hosted`, keeping only nodes that connect to that
  class's start and at most the web's 20 points (`World::apply_talents`). It cannot check a
  guest's renown: a modified client could claim a full web. Fine among friends. **Review.**
- A hero's gathered bonuses travel in the snapshot (`Hero::talents`), the state their mechanics
  keep (Equilibrium marks, crit streaks) stays on the host.
- Renown is awarded by each player's own game for their own hero, from the mirrored run.

## The Talents screen

From the main menu: **Talents** (bottom left, under the class tower). It shows the class picked in
the menu. Drag to move the web, the wheel to zoom about the cursor, Escape or Close to leave.
Hovering a node shows its name, kind, effects and what a click does; the path a click would take is
drawn green. Gold rings and links are taken; white rings can be taken next. Class starts wear WoW's
class colours and names, other nodes the colour of their school.

Buttons: Loadout 1-3, Respec, Per Class / Shared, Center, **Test: +5 Points** (adds 5 saved test
points each click, back to 0 after the top; they are listed on the points line), Close.

Saved in `benilla-config/together-talents.txt` (renown per class, the setting, test points, every
class's three loadouts by node id, so reordering `talents.ron` never breaks a save; an unknown id
is dropped).

Test switches: `TOGETHER_TALENTS=1` opens the screen at start; `TOGETHER_TALENT_POINTS=<n>` adds n
points for this session only.

## Decisions to review

1. **Points:** renown from level, minutes and kills; 20 points at most; first level 100 renown,
   +30 each. Per class by default, shared as the setting.
2. **One keystone at a time**, keystones as dead ends.
3. **Budget:** by the generous estimate a full web is about 40% for a focused class; gear has no
   room yet. See above.
4. **Other classes' starts are walls**; all of the web is reachable from every start.
5. **Divine Fury** casts the shortest-cooldown Holy spell other than the one that crit ("lowest
   cooldown" read as the shortest cooldown).
6. **Elemental Equilibrium** works for every school, Physical included, and a two-school spell
   (Frostfire) marks with both.
7. **Slow Burn** doubles the burns hits leave (the hit's -25% is made up for, so burns are a
   true double); ground areas' own ticks are hits and lose 25%.
8. The **Lore** nodes are smalls with two steering effects each, not notables.
9. Renown counts in test runs too.
10. Guests' allocations are trusted (connectivity and the 20-point top are enforced, renown is not).

## Not built, or uncertain

- Not tested in a running game beyond what the screenshot check of this branch shows (see the
  report): the screen's drag, wheel and hover need a real mouse; co-op with talents was not run.
- No talents in the in-run stat sheet yet (the stats they add do show, as part of each stat).
- No icons on nodes; colour by school only.
- No unlock gates (keystones from challenges, 9e) and no "banish" or extra augment option nodes.
- Classless mode (9c) would need a start for it; the web is ready for one more start node.
