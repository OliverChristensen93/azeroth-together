# Nightfall sub-project 4 (Combat): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-combat.md`. One line each: what; why; cost if wrong.

## Scope

- Loot, bags, weight and the inn stay in sub-project 5; so food and water are not items yet: Eat and Drink is free and restores health, and mana only with a conjured drink (Conjure Water fills `HeroFight::drinks`, 2 a cast, at most `max_drinks`); why: the spec lists Conjure Water as "the mage's drink" in this sub-project, and without items a drink has nowhere else to live; cost if wrong: sub-project 5 swaps the counter for water items.
- The run's end (every hero dead or out) and its screen stay sub-project 5's; a dead player uses Leave Run; why: §8.1's end screen is the run loop's; cost if wrong: a few lines in the host's step.
- Enemies' abilities deal damage only (their `Damage` and `Weapon` effects); holds, slows and Distract on heroes wait until something needs them; why: no enemy in the spec's sub-project 4 controls heroes, and controls on a client-owned position need their own design; cost if wrong: an effects arm per kind for heroes.
- Stitches, ravens and the night clock stay in sub-project 6; rivals in 7, whose kits will name these abilities by key and pick them by `tags`; cost if wrong: none now.
- No misses, dodges, parries, blocks, armour or resistances; crits only (5%, ×1.5, never on bleeds and burns); why: the spec's misses are the telegraphs' alone, and every other roll hides why a fight went as it did; cost if wrong: one roll in `World::strike`.

## Abilities

- Each ability's `spell` is vanilla's rank at level 30, read from the trimmed data's Spell.dbc (Frostbolt 8406, Fireball 8401, Fire Blast 8412, Frost Nova 865, Arcane Missiles 5145 with its missile 7270, Polymorph 12824, Blink 1953, Conjure Water 6127, Stealth 1785, Sap 2070, Cheap Shot 1833, Garrote 8632, Ambush 8724, Sinister Strike 1760, Eviscerate 6762, Gouge 1777, Sprint 2983, Distract 1725, Vanish 1856), with its costs, ranges, cooldowns and damage; why: WoW's own tooltip, icon, usability greying and range colouring then fit the numbers the host uses; cost if wrong: content edits.
- Where the spec gives a number, it beats Spell.dbc's: Frostbolt 2.5 s (dbc 3.0), Fireball 3 s (3.5), Polymorph 20 s (30), Sap 30 s (35), Sprint 8 s (15); WoW's tooltip still says the rank's own; cost if wrong: a tooltip that disagrees by a talent's worth.
- Fireball is rooted (the "biggest nuke"), Frostbolt and Polymorph slowed, Arcane Missiles and Conjure Water rooted, every rogue ability free; why: §6.1's rule applied to §6.2's table; cost if wrong: one field each.
- Blink costs 160 mana (Spell.dbc's 35% of a level-30 mage's base mana) and goes 20 yards (its radius); the spec's 15 s cooldown matches the data; cost if wrong: two numbers.
- Openers need stealth as well as an unaware target, as WoW's do (Spell.dbc's stance requirement); why: §5.4 names only the unaware rule, but an opener out of stealth is not an opener; cost if wrong: one `needs` flag per opener.
- Sap and Distract do not alert what they land on (`alerts: false`); Polymorph does (the sheep wakes alerted); Sap, Distract, Sprint, Vanish and Throw a Stone keep stealth; cost if wrong: content flags.
- Garrote is `Silent`: no ability noise and no 15-yard hit noise from its bleed; every other hit makes §5.2's hit noise; cost if wrong: the `silent` rule in `strike`.
- Effects land at once when a cast completes; a missile's flight is only the client's look, so a number can rise before the missile arrives; why: a travel-time queue on the host adds state for a visual nicety; cost if wrong: a pending-hit list in `combat`.
- Arcane Missiles' cost and cooldown are paid as the channel starts, its five ticks each land; cast-time spells pay when they go off; cost if wrong: one call moved.
- Range is centre to centre plus `reach` 1.5 yards (the target's size); in front is WoW's 180° arc; behind is the attacker in the target's rear half and facing it; abilities reaching past 8 yards need a clear line, checked as they begin and again as a cast ends (range only as it begins, as WoW does); cost if wrong: settings and one constant.
- Abilities carry `tags` (Opener, Builder, Finisher, Nuke, Control, Escape, Heal, Utility) unused by this plan; why: §7.4's rivals pick abilities by tag; cost if wrong: none.

## Presses

- The bar's presses are WoW's own `UseAction` calls, taken out of the script's calls (`UiScript::take_action_uses`) after the UI's handlers and before the engine applies them, so keys 1 to =, clicks and macros all work and the engine's cast path (which would refuse on its own data) never sees them; why: no engine change; cost if wrong: an ordering constraint in one registration.
- A press is the latest `CastPress` repeated in every input until the next, told apart by a wrapping `seq`; the first input a host sees is a baseline; why: inputs replace one another between steps, as Stealth's presses taught; cost if wrong: a press lost or replayed (pinned by tests).
- Stealth's button counts the senses' Stealth presses instead of a cast press, and stepping into stealth also needs `may_stealth` (up, out of combat, Stealth's 10 s cooldown done; it starts when stealth ends, as WoW's does); leaving is always allowed; why: one way Stealth toggles, which the senses' tests already pin; cost if wrong: a cast-press arm.
- The target is WoW's own selection (Tab, clicks) mapped back from the unit's guid; a ground ability goes where the cursor's ray meets the client's terrain, or four fifths of its range ahead when the cursor finds none; why: aiming by cursor at the press is WoW's "@cursor" with no extra click; cost if wrong: a reticle step like WoW's ground-target cursor.
- The rogue's openers live on the stealth bar (actions 73 to 84), which WoW's own feed shows when the hero wears the Stealth shapeshift form (form 30, written with the creep bit); why: WoW's rogue bar does this, and 11 abilities plus 2 shared moves do not fit 12 buttons; cost if wrong: a content change (openers on the main bar) if the form does not turn the bar; WoW's stance bar may show a Stealth button, which presses into the engine's dead cast path.
- The shared moves sit on `-` and `=` (Throw a Stone 11, Eat and Drink 12) for both classes; cost if wrong: content.

## Casting and moving

- The host enforces movement while casting three ways, since clients own their position: it lowers a slowed caster's speed to `cast_move_speed` (the controller takes it as it takes stealth's), roots a rooted caster with WoW's root (a `MoveModeMessage` from `HeroFight::rooted`), and breaks a cast whose hero went further from where it began than `allowance`: full speed for `speed_grace` 0.3 s (the slow reaching a joined client), then the slowed pace, plus `cast_slack` 0.5 yards; why: the controller does the work and the host checks it; cost if wrong: settings.
- A rooted ability (and Eat and Drink) is refused on the move ("Can't do that while moving"), as WoW refuses cast-time spells; a slowed one may begin on the move; cost if wrong: one check.
- Root, not speed 0, holds a rooted caster: at speed 0 the controller would still report moving keys as a run to the senses; cost if wrong: none found.
- Any press during a cast is refused ("Another action is in progress"); a dodge cancels the cast first; cost if wrong: WoW's spell queue is not modelled.

## Resources and regeneration

- A level-30 rogue has 1050 health, 100 energy back at 10 a second (WoW's 20 per 2 s) always, a 26-46 swing every 1.8 s; a level-30 mage 880 health and 1300 mana; why: a level-30 hero in modest gear; cost if wrong: content.
- "Nothing regenerates in combat" covers health and mana; energy always comes back (it is the rogue's rhythm, not a reserve); out of combat, `regen` gives health and mana a second, starting at 0 so eating and drinking are how a hero heals; why: the extraction game's tension, and a setting to playtest the other way; cost if wrong: one setting.
- In combat means an enemy is alerted to the hero, or a blow dealt or taken in the last `combat_linger` 5 s; cost if wrong: a setting.
- Auto-attack is the rogue's alone (a class with a `swing`), one main-hand blow; it starts when an `attack` ability lands, stops in stealth, on its target's death, and when its hero lands a hold that a blow would free (Sap, Gouge, Polymorph); right-clicking an enemy does not start it (the engine's local attack goes to no server); cost if wrong: an attack press in `PlayerInput`.
- Combo points sit on one enemy: a builder on another starts over, and they go with their enemy's death; a finisher spends them all; cost if wrong: none found.

## Holds

- A stun is not freed by damage; incapacitate, polymorph, sap and root are, by any blow including bleeds; a hold's own damage (Gouge, Frost Nova) lands before it; cost if wrong: one method.
- A held enemy out of the fight (stun, incapacitate, polymorph, sap, dead) neither looks, thinks, hears shouts nor moves: its mind freezes as it was; a rooted or casting one thinks and turns but stands; why: Sap's "out of the fight 30 s" and a stun's helplessness; cost if wrong: an awareness rule.
- A sheep wanders within 4 yards of where it was turned at 1 yard a second, and is drawn with display 856 (WoW's sheep); cost if wrong: the look (the debuff and the wandering stay).
- Distract turns the unalerted enemies within 10 yards of the spot to face it for 10 s while they are calm or wary, beside its 40-yard noise; why: §6.3's "nearby enemies turn to it"; a noise alone adds 35, under the 40 that turns an enemy; cost if wrong: a hold.
- Vanish turns every chase after the rogue into a search where it last saw them (suspicion at `check`), and puts them in stealth; why: the senses' rulings said Vanish must clear an alert, and alerted enemies track by plain sight; cost if wrong: an awareness rule.

## Enemies

- Every enemy kind has a melee `swing` (none in the senses' test content, so their chases end in no blows) struck from `reach` 2.5 yards, a little past the senses' 2-yard chase stop; cost if wrong: content.
- A caster keeps its longest bolt's range less 2 yards from its hero while it has a clear line to them, and closes in when it has none; cost if wrong: one constant.
- A move is used when ready (its `every`, half of it at the start so nothing opens with its big move) and when it reaches; else a plain swing; cost if wrong: content.
- A telegraph is placed as its windup starts: a circle around the caster (`on_self`) or under the hero, a cone from the caster toward the hero; it lands on every up hero inside it then, its caster held still meanwhile; a hold that takes the caster out of the fight cancels it; cost if wrong: one rule.
- The elites are Duskwood's own rares, Fenros (a worgen with a cone cleave) and Naraxis (a spider with a stomp), placed near the entry and marked `measure:`, as the earlier plans placed every group; WoW's elite dragon shows on their target frame; cost if wrong: content.

## The dodge

- The dodge key is a WoW key binding, `NIGHTFALL_DODGE`, registered under "Nightfall" so it shows in WoW's own Key Bindings window and is saved with the player's bindings; its defaults are Caps Lock and the mouse's back button (`BUTTON5`), both unbound in 1.12's `DefaultBindings.wtf` (read from the trimmed data: among the plain keys it leaves free are I, J, Y, Caps Lock, F6, F7 and `BUTTON5`; F, G, V, X and Space are taken); why: the spec asks for a setting that clashes with nothing; Caps Lock sits under the little finger beside WASD; cost if wrong: two default keys, or the binding's registration if WoW's window does not list a late one (then a raw key).
- The client hops at once (a knockback hop, which the controller flies with its own collision, 5 yards a second up and 6 yards along) when its mirror shows the hero up, free-footed and with a charge, and counts the press; the host spends the charge, starts the 0.3 s in which telegraphed blows miss (from the press's arrival, host time), makes the 20-yard noise and cancels a cast or a meal; why: clients own their position, and a round trip's wait would make a dodge useless; cost if wrong: a dodge the host refuses has still moved the hero (friends' co-op trust).
- Dodging does not break stealth (its noise is its cost); it goes the way the movement keys point, straight back with none; cost if wrong: one call.

## Falling

- Enemies leave the downed and the dead be: the senses neither see nor hear them, so the enemy after them loses them, searches where they fell and goes home; why: §8.1's revive needs a chance; cost if wrong: one filter (a setting would let both be playtested).
- A revive is any teammate up within 3 yards for 5 s, no key; walking away starts it over; the 20-yard noise comes as it begins; a revived hero gets up with 30% health; cost if wrong: settings.
- A downed hero crawls at 30% speed, kneels (stand state 8) and shows 1 health so WoW plays no death; a dead one shows 0 (WoW's death), is rooted, and WoW's "Release Spirit" popup is closed whenever it shows; cost if wrong: the look.
- A downed or dead hero cannot press ("You are dead"); their stealth, auto-attack and Sprint end as they fall; cost if wrong: none found.

## The client

- Every event goes to every player, a client's own events (refusals, a Blink's end) shown only to it; events wait per client while its connection is behind (capped at 4096); the protocol goes to 4; cost if wrong: a few bytes.
- The fight is shown through the engine's own seams: `CastEvent` and `SpellGoTargets` for casts and impacts, `SwingMessage` for blows, `CombatTextSpawn` for numbers (yellow ours, white enemies', red our hero's, "Evade" for a dodged telegraph), `CastBarFeed` for our cast bar, `UiErrorKeys` with WoW's own GlobalStrings for refusals, the engine's `Cooldowns` for the bar's sweeps (rewritten when the host's wait moves by more than 0.15 s), `Engaged` for the ready stance, `TeleportMessage` for a Blink; cost if wrong: per-seam fixes, no engine change.
- Hero buffs go in aura slots from 0 (Stealth, Sprint, the meal), enemy debuffs from 32 (holds, slows, bleeds), written whole when they change; `auras::heroes` replaces sub-project 3's `conceal::heroes`; cost if wrong: none found.
- Telegraphs are our own unlit red mesh draped over the client's terrain (4 rings of 32 points), brightening as the blow nears; WoW's ground decal draws circles only; cost if wrong: a look.
- `NIGHTFALL_AUTOFIGHT` targets with WoW's own `TargetNearestEnemy()` and presses through `UseAction`, so a test run exercises the real path; it does not walk to its target (the autoplay circle does the walking); cost if wrong: test hooks only.
