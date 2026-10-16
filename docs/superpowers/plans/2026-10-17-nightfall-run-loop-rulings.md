# Nightfall sub-project 5a (The run loop): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-run-loop.md`. One line each: what; why; cost if wrong.

## Scope

- Sub-project 5 is split: 5a (this plan: loot, bags, weight, exits, death, end screen, stash and free kit, the menu kept) and 5b (`2026-10-17-nightfall-inn.md`: the inn hub, contracts, stations, the companion, the spike on WoW's windows); why: together they are some 27 tasks, past the 16 a plan should hold, and 5a alone already plays as "a whole extraction run"; cost if wrong: none, 5b builds on 5a's names.
- The menu of sub-project 1 stays in 5a, with a Stash button and panel; the inn replaces it in 5b, and the stash panel becomes the banker's; cost if wrong: one panel moved.
- `RaidRoll` is the seam sub-project 6 fills (exits open and when, entry, weather, monsters); 5a's `RaidRoll::standard` opens every exit (the late one at `cart_opens_at`) at the first entry. 5a does roll each container's table from the raid's seed, since loot must exist to play; sub-project 6 may choose the seed differently; cost if wrong: none.
- Risk zones are not tuned here: a container's table says what it holds (chests and graves richer than crates, elites' bodies richest), placed where spec §8.1's risk zones are, all marked `measure:`; sub-project 6 tunes; cost if wrong: content.
- Darkshire's guards and lamps are not part of 5a: the cart exit is placed there; the lit areas are sub-project 3's `lit` list, the guards sub-project 6's; cost if wrong: Darkshire plays as any other place until then.

## Items

- Items are named by key in content and in the stash file, by index on the wire (the handshake's content hash guarantees both ends agree); why: a content edit never shuffles a stash, and the wire stays small; cost if wrong: none found.
- Worn looks come two ways: `Look::Item` (an ItemDisplayInfo id with the stand-in template fields; only the classes' three weapon displays are known, so found weapons reuse them under other names and icons) and `Look::Outfit(class_id)`, the piece a new character of that class starts in for the slot, which the data is sure to have; why: no Blizzard data may be read into content, and an unknown display id shows a body without gear; cost if wrong: few distinct looks until displays are measured (`measure:` in items.ron's comments).
- Icons are named in content (`icon: "INV_Sword_04"`), not looked up from the display; why: every item gets the icon its kind suggests, also items with no worn look; cost if wrong: a name the data lacks shows a grey square, a content fix.
- Gear adds health and a share of damage only (`Bonus`); why: the fight has no armour, misses or resistances (combat rulings), so those are the two numbers that change a fight; cost if wrong: more fields later.
- An empty weapon hand shows nothing (the class's look no longer stands in), and abilities never need a weapon; why: what you carry is what you see, and a hero who lost everything still fights (spec §8.2: "never stops a hero fighting"), the free kit giving the weapons back; cost if wrong: one fallback in `gear::pieces_worn`.
- Nine equipment slots (head, shoulders, chest, legs, feet, hands, back, main and off hand): no rings, trinkets or ranged; why: the ones that show on the hero; cost if wrong: an enum variant and a WoW slot each.
- Four bag slots; the backpack's slots are one row (16 plus the bags'); taking a bag off packs the backpack's things to the front and is refused when they would not fit; why: simpler than WoW's per-bag rows, and nothing is ever dropped by it; cost if wrong: a panel layout change.
- Safe slots are slots of their own (2), take any item, and count for weight; why: the spec's "always kept" needs a place the player chooses; cost if wrong: one rule.
- Anything taken from a container is "found", even what the hero dropped there; why: one rule, no bookkeeping of who brought what into a pile; cost if wrong: under `found_only` a hero who drops and picks up their own kit risks it, a rare case.
- No durability and no repair; why: nothing wears out in the spec's rules; 5b's workbench crafts instead; cost if wrong: a field per item and a station level.
- A lantern is an item lit from the backpack (Use), lighting further (its `light` range and intensity) and making the hero easier to see by the senses' `lantern` ×1.6; it goes out when it leaves the hero; why: the senses plan left the toggle to the items; 5b's "quieter lanterns" are its own question; cost if wrong: one more field on `LightDef`.
- Potions act at once, share one 30 s wait (`potion_cooldown`), and work in combat; why: WoW's own potions, minus the 2-minute cooldown, which a 25-minute raid would make rare; cost if wrong: a setting.
- Eat and Drink eats the backpack's first food (its `restore` share of health over the meal) and, for a mana user, its first drink; with neither it is refused (`NoFood`, WoW's "Missing reagent"); the safe slots are not eaten from; `free_meal` (default false) gives health without food, for playtesting food as a resource or not; the fight's tests run with `free_meal: true`; why: combat's rulings left the counter to become items; cost if wrong: a setting.
- Conjure Water makes `conjured_water` items (found, at most `max_drinks` carried) in place of the drinks counter; `HeroFight::drinks` now counts the drinks carried, for the client; cost if wrong: none found.

## Weight

- The spec gives `weight_free` and the floor but no weight at which the floor is reached: `weight_full` 60, the penalty growing in a straight line between (20 free, 60 full); why: three slots of heavy valuables should be felt, a whole pack of them should slow to the floor; cost if wrong: two numbers.
- One speed factor for walking, running and sneaking alike; why: WoW's controller derives its walk speed from the run speed it is given; cost if wrong: a per-gait factor if playtests want running untouched.
- Weight makes footsteps louder by the same straight line, and sneaking stays silent (0 times anything); why: the spec's "footsteps are louder" and stealth's promise; cost if wrong: a floor for sneaking noise.

## Containers

- Search times: crate 1.5 s, chest 2.5, grave 6, a body 1.5 (`search_body`); heard as it begins and every `noise_every` 2 s; the hero must stay within 1 yard of where they began, and within `reach` 3 yards of the container; why: §5.2 gives the noise, not the time; a grave dug is long and loud, a crate quick; cost if wrong: content and settings.
- A container's height counts only when content gives it (`z`); otherwise reach is on the ground alone; why: placements are not measured yet, and a wrong height would make a chest unopenable; cost if wrong: a crypt chest opened from above until measured.
- Containers are WoW GameObjects of type chest, so WoW shows its loot cursor and a right-click reaches us (the engine's own use goes to no server); the search is a right-click, not a key; why: WoW's gesture, no new binding; cost if wrong: an interact key binding like the dodge's.
- Once searched, a container is open to every hero (no owner); placed containers do not refill in a raid; cost if wrong: an owner field.
- An enemy's body holds a roll of its kind's table (`loot.ron`'s `bodies`, so `enemies.ron` is untouched) and is made only when the roll holds something; a hero's body holds what `death_loss` takes; cost if wrong: empty bodies to search for nothing.
- Dropping makes a pile on the ground (or joins one within 1.5 yards), which anyone may take from: co-op's way to spread a load; cost if wrong: none found.
- The chest's display (259) is the survivors' own; the crate's, grave's, pile's and remains' displays are stand-ins marked `measure:`; the placed containers' spots are marked `measure:` except two crates on the autoplay's circle by the first entry, which stay so a test run loots; cost if wrong: content.

## Acts

- Every act (wear, move, use, drop, open, take, put, close) is one `ActPress` repeated in every input until the hero's `Carry::acted` says the host took it; the client queues the rest (at most 32), numbered 1 to 255 and round; why: inputs replace one another between host steps (Stealth's and the bar's presses taught the same); cost if wrong: one round trip per act (Take All moves a whole container in one).
- A hero who is not up has their acts answered and ignored; cost if wrong: none found.

## Exits and extracting

- An exit's ring is flat (no height check) with a radius of 4 yards unless content says; why: no exit stands over another floor in Duskwood, and unmeasured heights would block a ring; cost if wrong: one check.
- A blow is a drop in health since the last step; healing does not reset the count; leaving the ring or falling does; cost if wrong: none found.
- Extracting is heard as the hero steps into the ring and every whole second after (§5.2's "per second in the ring"); cost if wrong: one rule.
- An extracted hero leaves the world; their outcome is final at once and sent to them; their screen says they wait for the others, and the controller lets go of the body; why: nothing is left for them to do in the raid, and a ghost hero would need senses and fights to ignore it; cost if wrong: a spectator view.
- The late exit is content's `late: true` (Darkshire's cart), open from `cart_opens_at`; cost if wrong: none.

## Death and outcomes

- `found_only` loses what was found, and also a brought thing that no longer has a slot once a found bag went with the body; why: nothing can be left nowhere; cost if wrong: a rare loss the player can see coming.
- A dead hero's outcome is settled when the raid ends, so friends can loot their body first; a recovery holds what is left on the body then; cost if wrong: none found.
- Reaching (opening) one's remains gives everything back at once to the stash, not to the backpack; only the owner sees and opens them; WoW's own point-of-interest flag marks them on the minimap and world map, set again every minute; the remains last one raid on their map, reached or not, and a new death replaces them; why: the spec's "reaching it gives everything back" and "one second chance, not a refund"; cost if wrong: a carry-out rule if playtests want more pressure.
- A raid that ends without a player's outcome (they left, the host went, the game crashed) counts as missing: the death rules with no body and no recovery; the loadout leaves the stash as the raid begins (`in_raid`, saved at once), so nothing is ever kept twice; why: an extraction game's honesty, and no duplication by quitting; cost if wrong: a gentler rule for disconnects.
- A player who leaves mid-raid drops, on the host, what a death would take, as a pile for their friends; their own client counts them missing; cost if wrong: none found.
- The host's Abandon Raid (the run bar's button, once Leave Run) ends the raid for everyone: those still in are missing, the dead are dead; the end screen shows, and Continue goes back; why: a host leaving must not hang the party; cost if wrong: none found.
- Kills are each player's `EnemyDied` events; cost if wrong: none.

## The stash

- The file is `benilla-config/nightfall/stash.ron`, computed in our own module from `local_state::home()` as the bake's cache is, not by a new function in the upstream `local_state.rs`; cost if wrong: one path function moved.
- Version 1; a file without a version reads as version 0 with the same shape; a newer one is refused, left untouched, and the session does not save (said on the menu); every save keeps the last good file as `stash.ron.bak` (a damaged file never overwrites a good backup); a damaged file is kept as `stash.ron.broken` (then `-2`, `-3`…) and the backup tried; a missing file with a backup comes back from it; why: the brief's "never lost silently"; cost if wrong: none found.
- The free kit is used when the loadout is empty as a raid begins; there is no button; its things are ordinary items afterwards, worth 0 or 1 gold; why: "always available" with no screen to learn; cost if wrong: kit items pile up in a stash (worthless, and the stash has room limits).
- The stash holds 60 stacks (`stash_slots`; 5b's stash station adds to it); what a raid brings back is kept past the limit (settling never refuses), and the panel then refuses moves in; why: never lose a raid's haul to a full stash; cost if wrong: none found.
- A key the content no longer knows stays in the stash as it was written; cost if wrong: none found.
- Anything worth gold can be sold from the stash panel (Shift-click) in 5a; 5b's trader takes over; cost if wrong: none.
- The profile remembers the class picked last; `NIGHTFALL_CLASS` beats it; cost if wrong: none.

## The interface

- Our own panels (backpack, loot, stash, end screen) with item icons and a one-line description under them, not WoW's GameTooltip; why: spec §8.4's "our own glue panels first", and 5b's spike says whether WoW's windows can be fed; cost if wrong: a tooltip pass.
- B opens our backpack; while Nightfall plays, WoW's own `OpenAllBags`, `ToggleBackpack`, `ToggleBag` and the like are quieted (WoW's bags would show empty); in a session that then picks Survivors they stay quiet, which Survivors does not use; cost if wrong: a Lua line to put them back.
- A click does the likely thing (wear, take off, light, drink, put into the open container); Shift moves to or from a safe slot; Ctrl drops; cost if wrong: panel code only.

## Co-op

- A player's `HeroPick` carries their loadout; the host makes it right for the content (`Inventory::sanitize`) and leaves out what has no room, with a warning, never refusing the player; `HeroPick` and `LobbyPlayer` lose `Eq` (a loadout holds a position); protocol 5; cost if wrong: none found.
- Each client's stash stays on its machine; the host never sees it, only the loadout brought; cost if wrong: none.

## Test hooks

- `NIGHTFALL_GIVE=<item>:<n>,…` (into the stash at start), `NIGHTFALL_STASH=1`, `NIGHTFALL_BACKPACK=1`, `NIGHTFALL_SEED=<n>`, `NIGHTFALL_AUTOPLAY_EXTRACT=1`; the autoplay loots what it walks past (standing still while it searches); test runs use `BENILLA_HOME=target/nf-home`, never the player's stash; cost if wrong: test hooks only.
