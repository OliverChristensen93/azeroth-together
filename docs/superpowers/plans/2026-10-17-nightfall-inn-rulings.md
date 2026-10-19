# Nightfall sub-project 5b (The Inn): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-inn.md`. One line each: what; why; cost if wrong.

## Scope

- 5b builds on 5a (`2026-10-17-nightfall-run-loop.md`) and replaces its menu with the inn; see 5a's rulings for the split; cost if wrong: none.
- The Stitches contract ships commented in `contracts.ron` until sub-project 6 adds Stitches to `enemies.ron` (content checks every enemy a contract names); cost if wrong: one uncommented block.
- The map table's intel shows what the `RaidRoll` holds now (which exits open when); signature monsters and weather join it when sub-project 6 rolls them; cost if wrong: none.
- Stormwind's Trade District stays out (spec §8.4's "later").

## The hub's place

- The user called the hub "Darkshire's inn" before bed; the spec says Goldshire's Lion's Pride Inn. The plan follows the spec, and puts every number of the place (map, spawn, facing, bounds, door, light, view distance, each NPC's spot) in `hub.ron` and nowhere else, with a commented Scarlet Raven Tavern block; moving the hub to Darkshire is a content edit, pinned by `the_hub_is_wherever_its_content_puts_it`; why: the spec is binding, the user's words are kept within reach; cost if wrong: editing `hub.ron`.
- The positions are marked `measure:` (the Lion's Pride Inn's ground floor as far as known); `NIGHTFALL_TRACE_POS=1` logs where the hero stands in the inn too; cost if wrong: a content pass after a walk round the inn.
- The inn is the hub's own scene on its own map (map 0 for Goldshire, the same as Duskwood): no loading between them, the terrain streams as the hero is moved; why: Goldshire and Duskwood share the Eastern Kingdoms; cost if wrong: a loading screen for a hub on another map.
- The evening is minute 1170 (19:30) with the engine's own fog and view distance (the night's given back); cost if wrong: one number.
- The inn has no enemies and no rules worth a host's authority: each client keeps its own hero inside the bounds, and the host clamps the poses friends report; why: nothing in the inn can be won or lost by moving; cost if wrong: a walk-through-walls friend, harmless.

## The lobby

- The inn is the lobby: friends who join appear and walk, their poses carried by the lobby's messages (at most ten a second, only when they moved), not by a world the host steps; why: simpler than an inn `World`, and nothing in the inn needs the sim; cost if wrong: a sim world later if the inn grows rules.
- Friends in the inn wear their class's starting outfit and weapons, not their loadout's gear; why: the gear look is drawn from the raid's `Carry::worn`, which the inn does not have; cost if wrong: the loadout's worn list in `LobbyPlayer` (it is in `HeroPick::loadout` already).
- Class is picked on the inn bar (the menu's class buttons, kept); the loadout at the banker; why: spec §8.4 "class and loadout are picked there"; cost if wrong: buttons moved.
- Ready is at the raid board in the door's ring; stepping out un-readies; alone, Ready starts; hosting, the raid starts once everyone present is ready (a friend who leaves no longer counts); joined, the host starts it; why: spec §8.4 "when everyone is ready"; cost if wrong: none found.
- The host plans the next raid (`RaidRoll`) in the lobby and sends it with every lobby message; alone, it is planned as the inn is entered; it is planned again after each raid; why: the map table's intel needs the raid before it begins, and sub-project 6 will roll it there; cost if wrong: none.
- Protocol 6.

## NPCs and panels

- NPCs are characters of a race and sex in a class's starting outfit (`body`), drawn as WoW player units (friendly, named), not creatures; why: a starting outfit is sure to exist in the data, while NPC creature display ids would have to be measured; cost if wrong: a `display` field per NPC once measured; target frames show them as players.
- A right-click within 6 yards opens an NPC's panel; walking more than 6 yards away closes it; why: WoW's gesture and WoW's reach; cost if wrong: a number.
- The banker opens the stash panel (5a's) and keeps the stash station (open from the start); the trader buys at `sells_for` × value (1.0) and sells a fixed stock; selling moved from the stash panel to the trader; cost if wrong: content and one click.
- Our own panels, as the spec says first; the spike (Task 12) is timeboxed to 3 hours, its code thrown away, its findings in `docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md`; why: spec §8.4 and §13; cost if wrong: a later plan to switch.

## Contracts

- At most `contracts_at_once` (2) taken; a contract's `needs` must be finished first; non-repeatable ones once; why: spec §8.5 "one or two at a time"; cost if wrong: content.
- Objectives are kills (of a kind, optionally within a circle, counted over any number of raids, whether the hero got out or died), things to bring (counted live from the stash and the loadout, taken at the turn-in from the stash first) and an extract by a named exit (or any) while taken; why: each spec contract is one of those (Ebonlock's worgen, Abercrombie's remains, Madame Eva's tome and Westfall road); cost if wrong: an objective kind.
- Kills count from each outcome's `slain` (kind and place), once per raid (`InnProgress::applied` against `Profile::raids`); a missing raid has no outcome and moves nothing; why: an outcome applied twice must not count twice; cost if wrong: none found.
- Madame Eva's tome is an item in a grave of its own (`ladims_grave`, measure:) at Raven Hill; cost if wrong: content.
- Rewards: gold and stations unlocked (spec §8.5 "unlocks stations and their upgrades"); upgrades themselves cost gold and materials; cost if wrong: content.

## Stations

- Each station is locked until a contract unlocks it, except the stash (open from the start); levels are paid with gold and materials from the stash only (not the loadout); a purchase pays and gives, or does neither; cost if wrong: one rule.
- Gifts: stash slots, recipes (made at the station that teaches them), the map table's intel, the kennel's companion; why: spec §8.5's list; the workbench crafts (there is no durability to repair, see 5a's rulings); cost if wrong: a gift kind.
- The lantern-maker's "brighter, quieter lanterns": a bright lantern (more range and light) and a shuttered one (less range, seen ×1.2 instead of the rules' ×1.6, `LightDef::seen`); why: "quieter" means less noticed, the only cost a lantern has; cost if wrong: content.
- Crafted and bought things go into the stash; cost if wrong: none.

## The companion

- One companion kind (a watch hound, display measure:), from the kennel's first level; it carries one stack (any count, any item) in a slot of its own (`Place::Pet`): never lost (no death mode, no missing raid takes it), not weighed (the companion carries it); why: spec §8.2 "carries one item that is never lost", as Duckov's dog; cost if wrong: a count limit.
- The companion is a look in the raid: every client draws it a step behind and beside its hero from `Carry::pet`; enemies ignore it and it does not fight; why: the spec gives it no other part; cost if wrong: a unit of its own in the sim later.
- The stack goes into the companion's slot by Shift-click (after the safe slots fill), in the raid's backpack and the inn's stash panel; cost if wrong: panel code.

## The stash file

- Version 3 adds `inn:` (contracts, stations, recipes, stash bonus, intel, companion, the raid last applied) and the loadout's `pet`; a version 1 or 2 file reads with an empty inn; a 5a build refuses a version 3 file (5a's newer-file guard), never overwriting it; cost if wrong: none found.
