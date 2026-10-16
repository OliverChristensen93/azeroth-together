# Nightfall sub-project 6 (The night's threats): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-night-threats.md`. One line each: what; why; cost if wrong.

## Scope

- Darkshire's Night Watch (monsters stay out, the guards fight Stitches, §8.1) is not built: no enemy fights another enemy in the sim, and 5a's rulings left the guards here only by name; Stitches walks the Raven Hill road there and back, turning at Darkshire's end; cost if wrong: a task with guards as a side of their own (allies of heroes, foes of monsters) and a haven zone enemies' paths avoid.
- The entry is not rolled: the roll's list in §8.1 (loot, exits, weather, monsters) does not name it, and the autoplay's loot crates sit by the first entry; cost if wrong: one line in `RaidRoll::roll` and a test hook for the entry.
- The weather does not change during a raid (§14: "weather beyond the per-raid roll" is out); cost if wrong: a stage field that sets a weather.
- Rivals stay out (sub-project 7); the night's rules are written so a rival can later be a hero-like maker of noise and scent (trails are by `PlayerId` now); cost if wrong: trails keyed by a unit, not a player.
- Every new place (zones, roads, flocks, the signature monsters' posts and routes, the howling's wanderers) is a `measure:` placeholder, as every earlier plan's placements are; the plan's tests use hand-made maps and its screenshots a scratch copy of the content near the first entry; cost if wrong: a content pass in the game with `NIGHTFALL_TRACE_POS=1`.

## The clock

- Stage times, `warn`, `fog` and `notice` are per stage and absolute, not cumulative: Moonrise's 0.75 and 1.2 are written again on the hunt and Stitches; why: a stage reads as what the night is while it lasts; cost if wrong: one fold in `fog_of` and `notice_scale`.
- Moonrise's "fog end 40 → 30" is a factor (0.75) on the weather's fog, both start and end, so mist under Moonrise is 28 × 0.75 = 21 yards; why: the spec's numbers under a clear sky, and the weather still matters later in the night; cost if wrong: a stage field for an absolute fog.
- The first stage has no cue (the raid begins in it); each later stage's cue is an event `warn` seconds before it lands (20 s, Stitches 30 s), and its landing another; the content checks a cue never comes before the previous stage lands; cost if wrong: none found.
- "Announced by sound" is the cue's sound (a SoundEntries kit or a file, from content) plus a chat line in WoW's emote orange; the line is no timer and can be left out in content; why: a deaf player and a test shot see it too; cost if wrong: drop the lines from `rules.ron`.
- The howling's howls are its cue alone, not a loop of howls through the stage; cost if wrong: an ambience entry per stage.
- A clock started late (`NIGHTFALL_NIGHT_AT`) gives every passed stage's cue and landing in its first step, cues first, each once; why: a test of the late stages must see the same newcomers and closings a full night would; cost if wrong: none found.
- On the client the fog eases toward the night's over about 4 s (`FOG_EASE`); the host's sight changes at once; why: Moonrise "thickens", it does not snap; cost if wrong: one constant.

## The roll

- Each exit that is not late: shut 15%, else opening late (15:00 to 22:00) 25%, else open from the start; the late ones (the cart) open within 2 minutes of `cart_opens_at`; at least `open_least` (2) exits open in a raid, one of them from the start (the first that is not late is opened when every one fell shut or late); why: §8.1 "which exits are open and when" and §7.2's late exits pulling the other way, never a raid with no way out; cost if wrong: numbers in `rules.ron`.
- "One exit may close" in the hunt: half the raids, one exit open by then and not late closes as the hunt lands, never when it would leave no exit open at some time; the map table does not tell (an exit closing is the hunt's surprise), and the chat says when it shuts; cost if wrong: a line on the board.
- The weather by weight, clear 5, mist 3, rain 2; mist is fog only (28 yards, paler); rain is the engine's own rain at grade 0.7 with its loop kit 8534 and a 32-yard fog; why: §8.1 names the three, §10 "rain that shortens the fog"; cost if wrong: content.
- Rain changes nothing in the senses (no muffled footsteps): the spec gives it only the fog; cost if wrong: a noise factor per weather.
- Signature monsters: each at its odds (Mor'Ladim, the banshee, the riders 50%, the trackers 60%), between one and three of them, the likeliest kept when trimming; Stitches always, since he is the clock's last stage ("the last stage of the clock", §7.3); cost if wrong: `always: false` and odds on his line.
- The roll's dice are the raid's seed mixed with a constant, so the loot (5a's own use of the seed) rolls as before; flocks have dice of their own; cost if wrong: none.
- `RaidRoll::standard` (5a's tests, and a hosted raid with no planned roll) has only the `always` monsters, the first weather and nothing closing; the inn's `plan_raid` and the start's fallback now roll; cost if wrong: none.

## Arrivals and zones

- A stage's newcomers are spawn lines with `stage:`; they come once no hero who is up is within `arrive_unseen` (50) yards of where they come, tried every second; distance only, not sight; why: the fog is 40 yards at most, so 50 is out of sight; cost if wrong: a line-of-sight check per hero.
- "Trackers join patrols" is their own patrol lines coming with the howling, in raids that roll the trackers; the howling's "more wanderers" come every raid; cost if wrong: content.
- A signature monster's lines live in `monsters.ron` and are appended to the content's spawn lines, so every system that reads a line (minds, warnings, the content's checks) treats them alike; cost if wrong: none found.
- Enemies added mid-raid push onto every list beside the world's enemies (brains, minds, ways, fights) in one place (`add_line`), ids after the highest so far; the dead stay in the lists as before; cost if wrong: none found (pinned by a test).
- Risk zones are polygons in `map.ron`; the first holding a point counts; an enemy's health is times the zone's `hp` where it is placed, its blows times `damage` from the zone where it was placed (kept in its brain, so a chase out of the zone keeps its might); containers and bodies inside roll their table `loot_rolls` more times; outskirts are a zone of factors under 1 (0.85); "more enemies" is content (more lines in the zones); death is untouched; cost if wrong: zone fields.
- The shipped zones: Raven Hill (cemetery and crypt) and Vul'Gol at ×1.3 health, ×1.25 blows, one more loot roll; the northern outskirts (where the first entry is) at ×0.85; cost if wrong: content.

## The hunt

- "Worgen packs" are the enemy kinds marked `hunter` (the Nightbane worgen, Fenros, the trackers); packs are not grouped: each hunter near the noise goes on its own, which at a noise looks like a pack; cost if wrong: a pack id on spawn lines.
- The loudest recent noise is the largest radius of the last 30 s, anywhere, heard or not by anyone; every 15 s while a hunting stage lasts, every hunter that is calm and in the fight within 150 yards runs there (its run speed), rests 4 s, and goes home; a hunter that meets a hero on the way is the senses'; cost if wrong: numbers in `rules.ron`.
- The hunt is a brain's errand (calm-only, like a patrol's next point), not a suspicion: a noise's 35 points would drain before a hunter 100 yards off arrived; cost if wrong: none found.

## Scent and trackers

- A crumb every 2 yards walked by a hero who is up (crawling or dead, no scent), lasting 30 s; none in water or on a road, and the next crumb after either starts a new stretch a tracker cannot cross; cost if wrong: numbers, and a rule for the downed.
- Water is baked from the ADT's liquids (MCLQ) at least 0.3 yards deep over the terrain; pools inside buildings (WMO liquids) are not read; the height map's codec and the bake's cache go to version 2, so every player bakes once more; cost if wrong: an indoor pool a trail crosses.
- A tracker picks up a trail when it is calm and a crumb lies within 3 yards of it (it grows suspicious of the crumb's maker), or when it is already suspicious of a hero whose trail is within 3 yards; it follows from the newest crumb near it to the next of the same stretch, its suspicion held halfway from `check` to `alert` so it walks; why: §7.1 "once suspicious", and a calm tracker crossing a trail is the trackers' whole point; cost if wrong: one rule in `follow_scent`.
- "A long detour" breaks the trail because a tracker trots at 4.5 yards a second against a running hero's 7: a long enough trail outlasts the scent before the tracker reaches its end; no path-length rule; cost if wrong: a rule on how far a tracker follows.

## Signature monsters

- Mor'Ladim is "looked at" by a hero who is up, within the fog, facing him within 45° either side (`look_cone` 90°) and with a clear line from their eyes to his chest; the hero's facing, not the camera's: right-mouse steering turns the hero with the camera, and the host has no camera; cost if wrong: the camera's yaw in `PlayerInput`.
- Watched, he is out of the fight for the step (the fight's own `Hold::Out`): no step, no turn, no blow, no thought; unwatched, he is after the nearest hero up within 90 yards, seen or not, at 6.5 yards a second; why: a learnable rule with one counter ("keep him in view while someone else moves"); cost if wrong: his range and speed in `enemies.ron`.
- He is an elite of 4200 health, so he can be killed by a party that keeps him watched; cost if wrong: content.
- The banshee's scream is an enemy ability, `banshee_wail` (2 s cast, 40 yards, Psychic Scream's spell 8122 for its look and sound): it is ready the moment she is alerted (`first: 0`), lands only if she still has her hero in range and sight as it ends (breaking her line of sight or killing her stops it), and every other enemy in the fight within 60 yards of her is alerted to that hero, through walls, without shouting on; she wails again after 40 s; why: §7.3's counter ("break line of sight fast; kill her before she screams") is exactly the cast's own rules; cost if wrong: content.
- The banshee is a caster: she keeps her wail's range less 2 yards from her hero (the fight's rule for casters), so she screams from afar rather than closing in; cost if wrong: a `swing` and no range.
- The Dark Riders ride a road from end to end and back at 10 yards a second; a hero standing on a road is noticed as far as 70 yards at visibility 100 (past the fog), off the road as by anyone; their mount display is unknown in the trimmed data, so they ride on foot (`mount: 0`) until measured; cost if wrong: content.
- Stitches: 20 000 health, elite, a telegraphed cleave and smash, a tread heard (on the client) within 60 yards every 1.4 s while he moves; his loudness is for the players' ears, not a noise for the enemies; cost if wrong: a noise of his own.
- Displays are stand-ins from Duskwood's own creatures, all `measure:`: Mor'Ladim the skeletal warrior (200) grown, the trackers the Nightbane dark runner (736), the banshee the shadow weaver (203), the riders the skeletal warrior, Stitches the fetid corpse (559) grown, the ravens a small bloodseeker bat (14562); Task 1's Step 6 lists the trimmed data's banshee, crow, abomination and horse models for the real ids; cost if wrong: content edits.
- Sound paths in content (`File(…)`) are names the 1.12 client is likely to have, checked against the trimmed data in Task 1's Step 6 and kept `measure:` where nothing fits; a missing file is a debug line and silence, never an error; cost if wrong: a silent cue until fixed.

## The ravens

- Ravens are built now (the senses' rulings deferred them for want of a model): the sim's rules are the spec's (8 yards running, 15 fighting, a 40-yard noise made by the hero who set them off), "fighting" being a hero in combat (the fight's `in_combat`); a flock settles again 120 s later once no hero is within 20 yards; each flock is out in a raid at 70%; cost if wrong: numbers in `rules.ron`.
- The birds are three small stand-in creatures per settled flock within the view distance, `Concealed` (no Tab, mouse or plate), despawned with a cry from where they sat as the flock takes off; no flight animation; cost if wrong: a rise-and-fade on the client.

## The client and the engine

- The engine gains one file, `sound/cue.rs` (`SoundCue`, `Cue`), and three lines in `sound/mod.rs`: a game's own sound by kit (from a place, with its SoundEntries distances) or by file (2D, as `PlaySoundFile`); no upstream file's logic changes; cost if wrong: a positional file path in the kit player.
- Stage cues play 2D (the night is everywhere); Stitches' tread and the ravens' cry play from where they are when their content names a kit, 2D when a file, gated by distance on our side; cost if wrong: positional files (above).
- The weather goes to the engine as its own `WeatherMessage` (rain or fine, its grade and loop kit), given once per change and fine again when the raid ends; cost if wrong: none found.
- The map table's intel adds the weather and the monsters out (Stitches among them) beside 5b's exits; it does not show an exit's closing; cost if wrong: one line.
- Test hooks: `NIGHTFALL_WEATHER=<key>` and `NIGHTFALL_MONSTERS=<key>,…` change the planned raid's weather and monsters (the always ones stay), `NIGHTFALL_NIGHT_AT=<seconds>` starts a hosted raid's clock there; unknown keys warn and leave the roll as it was; cost if wrong: test hooks only.

## The wire

- `Snapshot` gains `night: NightView` (the stage, the weather, each flock's state: a few bytes), sent to every player; the night's events (cues, landings, an exit closing, an alarm, ravens) go to every player as the fight's do; protocol 7; cost if wrong: none found.
