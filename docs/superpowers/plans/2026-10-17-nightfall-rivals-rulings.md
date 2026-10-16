# Nightfall sub-project 7 (Rivals): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-rivals.md`. One line each: what; why; cost if wrong.

## Scope

- A rival is a hero of another side (`Hero::side = Side::Rival(n)`) in `World::heroes`, moved only by the `PlayerInput` its bot sets, not a third list of units beside heroes and enemies; why: spec §7.4 wants "the same sight, noise and awareness rules as heroes" and the brief wants shared systems, and as a hero a rival is seen, heard, chased, struck, scented (sub-project 6's trails are by player), searched for, killed into a body and extracted by code that exists; cost if wrong: every party-only rule needs its side filter (the ones found are in Task 4; a missed one is a rival counted as a teammate somewhere).
- Rival parties leave one another be (`World::hostile` is the party against any rival); why: the spec gives rivals goals against the heroes and the night, not each other, and bot-against-bot fights nobody sees would only empty the map; cost if wrong: one rule in `hostile`, and minds that track other parties.
- Rivals do not eat, drink, use potions or light lanterns, and loot only containers inside their points of interest (bodies lying there included); why: the spec names loot, fights, extracting and hunting as their goals; cost if wrong: a bot step each.
- Darkshire's Night Watch is still not built (sub-project 6's rulings); rivals walk into Darkshire like anywhere else (spec §8.1 "rivals can follow them in"); cost if wrong: none now.
- Every place in `rivals.ron` (points of interest, entries) is a `measure:` placeholder near the first entry, as every earlier plan's placements are; cost if wrong: a content pass in the game with `NIGHTFALL_TRACE_POS=1`.

## Content

- Each kind becomes a class appended after the heroes' (`ClassDef::rival`, key `rival:<kind>`), cloned from a hero class (`base`: power, speed, swing, stealth) with its own name, look (`class_id` for the starting outfit, race, sex), health, bar and gear; the menus list `Content::playable()`; why: a hero's fight, pack and free kit read a class; cost if wrong: a menu loop that still walks `classes` shows rivals (Task 1 changes the menu's and points at the inn's).
- A simplified kit (`kit: Abilities([...])`) goes on the bar from action 1, the big move after it; `kit: Class("rogue")` takes the hero class's whole bar; the shipped `cutthroats` party has full rogue kits at odds 0, so only `NIGHTFALL_RIVALS` brings it; why: spec §13 "playtest both" through content alone; cost if wrong: a content change.
- The big move must have a telegraph (content refuses one without); why: spec §7.4 "one telegraphed big move" and §6.1's telegraphs for rivals' big moves; cost if wrong: none.
- `Effect::Heal { min, max }` is new, on the caster (each tick of a channel); a rival's Heal tag needs an effect, and Eat and Drink sits still out of combat with food; heroes may be given one too; cost if wrong: one effect arm.
- The rivals' abilities are their own entries in `abilities.ron` with vanilla spell ids for look and tooltip only (`measure:` against the trimmed data); numbers sit near the heroes' (a 25 to 60 hit, a 4 s control, a 140 to 190 big move every 18 to 22 s); cost if wrong: content.
- Gear and carried finds reuse `items.ron` and `loot.ron` (grave robbers carry a grave's roll, Defias a crate's, deserters a chest's); a kind's bag goes in its first bag slot; why: killed, they drop a bag worth searching; cost if wrong: content.
- Points of interest and entries live in `rivals.ron`, not `map.ron`: they are the rivals' knowledge of the map and change with them; cost if wrong: a move into `MapDef`.
- An empty `rivals.ron` text is no rivals, so every earlier content fixture loads unchanged with `rivals: ""`; cost if wrong: none.

## Senses

- A rival's `Mind` tracks only the party's heroes, with the enemies' `think`, a 120° cone (`cone`) and no sleeper; its shouts reach its own party within the senses' `shout` yards and do not chain; why: "they notice heroes the same way enemies do"; cost if wrong: settings.
- A hero sees a rival within the fog and in line of sight, a stealthed one only within `notice_range(visibility, 1.0, fog)` (2.8 yards still, 7 moving); a hero has no facing cone, as the screen shows all round; cost if wrong: one function (`sees_rival`).
- An unseen rival is `Concealed` and untrackable but still drawn (faded in stealth), as an unseen enemy is; cost if wrong: a hide of the body.
- A blow on a rival is an ambush (×`ambush`) while its mind is not alerted to the attacker; a rival's blow on a hero is one while the hero is not in combat (heroes have no mind); cost if wrong: one function (`foe_unaware`).
- A hero's blow, or a hostile ability that `alerts`, alerts the rival to the attacker where they stand, tells its party within shout range, and marks the party as struck (it then fights whatever its aggression); Sap and Distract stay quiet as on enemies; cost if wrong: one rule.
- Vanish loses rivals' minds as it loses enemies'; cost if wrong: none found.
- Rivals hold Mor'Ladim by looking at him and are his prey, trackers follow their scent, ravens rise for them, hunters go to their noise: sub-project 6's code is unchanged, since rivals are heroes to it; why: the same rules; cost if wrong: a side filter in `watch_morladim` if a rival freezing him looks like a bug in play.
- The downed and dead are left be by rivals as by enemies (the senses' `sense` returns nothing for them); cost if wrong: one filter.

## The fight

- Holds on a hero root them (`HeroFight::rooted`, which the client already turns into WoW's root) and, unless a root, refuse their presses (`Refusal::Held`, WoW's `SPELL_FAILED_STUNNED`); a hero is never made a sheep (Polymorph holds them like an incapacitate) and is freed by damage as an enemy is; why: controls on a client-owned body (combat's rulings) by the simplest means that works; cost if wrong: a sheep look and wandering for heroes.
- Slows and bleeds on heroes live in `HeroCombat` (`slow`, `burns`), as enemies' in `EnemyCombat`; combo points on a hostile hero in `HeroFight::combo_hero`; auto-attack at one on its own timer (`foe_swing_in`); cost if wrong: none found.
- Area abilities (a radius, `Aim::Me` or `Ground`) reach the hostile heroes in them; a friend is never struck; cost if wrong: none found.
- A hero's telegraphed ability (a rival's big move) casts for its windup and lands as its telegraph does, on every hostile hero inside (a dodge's moment makes it miss, `Evaded`) and every enemy inside; cut short (moved, held, killed), its telegraph goes; cost if wrong: one rule in `foe_telegraph_lands`.
- Rivals die at 0 health (`rivals.downed: false`); with `downed: true` they go down as heroes do and their own party revives them; why: "killed, they drop" their bags, and a 45-second crawl only delays it; a setting to playtest both; cost if wrong: one setting.
- A side revives only its own; cost if wrong: none found.
- A rival's body holds everything it had (`DeathLoss::Everything`), its slots grown to fit; its killer is the last party hero to strike it (`Event::RivalDied { by }`), counted in that player's kills; contracts' kill counts (by enemy kind) do not count rivals; cost if wrong: a contract objective kind for rivals.
- A rival out by an exit leaves with what it carried and settles no outcome (`Event::RivalOut`); the raid ends with the party alone, whatever rivals do; cost if wrong: none found.

## The bots

- A bot sets its rival's input at the end of a step (after the step's noises are heard), the host takes it at the next step's start: one step's lag, as a client's; cost if wrong: none found.
- Ability order by tag: heal under `heal_below` (0.5), escape under `flee_health` (0.3), opener from stealth on the unaware, control on a free target, the big move when it reaches, a finisher at `finisher_at` (3) combo points, a nuke, a builder; a bot presses at most every 0.25 s and only what is ready (cooldown, global cooldown, power), standing for a cast that must be stood for; cost if wrong: one function (`choose`) and two constants.
- Party decisions (`decide`): a hero it is alerted to is fought at aggression ≥ `engage_at` (0.5), when the party was struck, or when the hero is within `avoid_within` (10) yards; else the party flees `flee_for` (8) seconds; a member under `flee_health` makes it flee; monsters after it or guarding its point are fought; a wary member stops it, a checking or searching one leads it to look; then loot until full, `leave_by` (the hunt stage) or out of points; then at aggression ≥ `hunt_at` (0.7) and health ≥ `rest_below` (0.7) hunt once for `hunt_for` (180) seconds, else extract by the nearest open exit; cost if wrong: settings.
- A hunt goes toward the strongest suspicion any member holds, else the loudest noise a party hero made within the night's hunt memory (`Rivals::rumour`), heard or not; why: the night's hunters already know the loudest noise anywhere (sub-project 6), and a hunt with no lead never starts; cost if wrong: a hearing-only lead (hunts would be rare).
- Points of interest by worth over distance plus 50 yards, the second best a quarter of the time (the party's dice); a party visits `pois` (2 to 4) of them; cost if wrong: two constants.
- Followers walk `gap` (2.5) yards behind their leader, left and right, running when more than 8 yards behind; calm parties walk (4-yard steps, WoW's walk), fights, hunts and flights run; sneaking kinds step into stealth whenever they are not fighting or fleeing; cost if wrong: constants.
- Paths: straight when `walk_line` allows, else a search within `paths_per_step` (2) for all rivals a step, at the patrols' budget, uncached, retried after 2 s for the same goal when it found none; a goal the leader gets no nearer to for `give_up` (15) seconds is given up (a point counts as visited, an exit as shut to them, a hunt ends, a check is dropped); why: never a party stuck for the night; cost if wrong: settings.
- Looting: each member takes its own container in the point, walks within `loot_reach` (2) yards, sends Open, waits out the search, sends Take All once, and Close; acts repeat until `Carry::acted` says the host took them, as a client's do; cost if wrong: none found.
- Dodging: a rival standing in a telegraph that lands within `dodge_at` (0.6) seconds rolls once at `dodge` (0.5) to dodge its dodge's distance straight out, counted as a player's dodge press; cost if wrong: settings.

## Arrivals

- Between `parties_out` (1 to 2) parties a raid, each at its own odds (grave robbers 0.6, Defias 0.5, deserters 0.5), the likeliest kept when trimming; each comes within `arrive` (0 to 300 s) by one of the rivals' entries, at an aggression rolled within its kind's, with dice of its own; the rivals' dice are the raid's seed mixed with a constant, so the loot, the exits, the weather and the monsters roll as before; `RaidRoll::standard` brings none; cost if wrong: settings.
- A party comes once no party hero up is within the night's `arrive_unseen` (50) yards of its entry, tried every second; members stand 1.5 yards around the entry; rival numbers are `RIVAL_BASE` (64) and up, so no player's number is ever a rival's (a host gives 0 to 3); at most 192 rivals a raid; cost if wrong: none found.
- The map table's intel lists the rival parties out (when the intel station is open, as 5b's lines are); cost if wrong: one line.

## The client and the wire

- Rivals are drawn by the hero code (`view::sync`) as WoW players in their gear, with the monster faction template (14), which WoW's own reaction rules make hostile: red plates and rings, a red target frame, Tab and the attack cursor take them; no engine change; cost if wrong: a faction template measured if 14 shows them oddly (an orange "unfriendly" would be another template).
- `Snapshot::rivals` (`RivalView`: who this player's hero sees, the rivals' marks for them) is each player's own, from `snapshot_for`, as 5a's pack is; protocol 8, once for the snapshot and `RaidRoll::rivals` together; cost if wrong: none.
- Another hero (or a rival) moving under 4 yards a second is drawn with WoW's walk; friends walking now walk too; cost if wrong: one constant.
- Heals show as green numbers (`COLOR_HEAL`); a rival's Blink moves its body on the host, and the client glides it there; cost if wrong: looks.
- Test hooks: `NIGHTFALL_RIVALS=<party>,…` makes the planned raid's rivals those parties (at `NIGHTFALL_RIVALS_AT`, 5 s by default, the middle of their aggression, by the entry nearest the first map entry out of `arrive_unseen`), `NIGHTFALL_TRACE_RIVALS=1` logs each party's goal; cost if wrong: test hooks only.
- Task 12's shots are where the looks are judged (outfits from `class_id`, the walk, the stealth fade on a player unit); what they show is noted here after the run.
