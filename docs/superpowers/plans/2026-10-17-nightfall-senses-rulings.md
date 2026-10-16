# Nightfall sub-project 3 (Senses): rulings

Decisions the spec (`docs/superpowers/specs/2026-10-16-nightfall-design.md`) does not settle, made
for `2026-10-17-nightfall-senses.md`. One line each: what; why; cost if wrong.

## Scope

- A minimal Rogue Stealth toggle belongs here (toggle, 70% speed, broken through `World::break_stealth`), with no cooldown, energy or other rogue ability; why: §5.1's stealthed rows and the playable result ("sneaking past") need it, and the rest of §6.3 is combat; cost if wrong: sub-project 4 moves the toggle onto the action bar and adds its cooldown.
- Which classes can stealth is content (`ClassDef::stealth`, the Rogue's `true`), not a class key in code; why: rival kits and later classes come from content (§7.4); cost if wrong: none.
- Alerted enemies chase and face their hero and never hit: sub-project 2 has no swing, so the brief's "stop at alerted, chases" applies; cost if wrong: none (the chase is where sub-project 4's swing plugs in).
- Combat's seams are here, unused by this plan's game: `World::{alert, ambush, opener_allowed, make_noise, flash, break_stealth}`; why: the brief asks to leave seams, and each is pinned by a test now; cost if wrong: a signature change in sub-project 4.
- Sleepers come in here (a `Behaviour::Sleeper` placed like a sentry, blind until alerted, drawn lying down, back to sleep after its search); why: sub-project 2's rulings defer them to the senses, and they are a senses trait; cost if wrong: one behaviour and one spawn line to remove.
- Ravens (§7.1) are not built: no raven creature model or take-off sound was found in the trimmed data (`list_chain` finds only crow spell textures and `crow_baked.m2`), and an alarm nobody can see or hear teaches nothing; `World::make_noise` with `Loudness::Ravens` is their seam; they go with sub-project 6; cost if wrong: a task there.
- Trackers' trail (§7.1, §7.3) is not built: it needs water and roads to break trails and the night clock's howling stage, both sub-project 6; cost if wrong: none now.
- The lantern's ×1.6 is in the rules and tested but no lantern toggle exists (it is an item, sub-project 5); `lit` areas come from `map.ron` and ship empty until Darkshire's lamps are measured (`measure:`); the fire flash is the `World::flash` seam for sub-project 4; cost if wrong: an untested multiplier in play until then.
- §3.4's public line-of-sight query in `benilla-world` is not built: no client check needs it yet (the host decides who sees whom); cost if wrong: a small engine task when the camera or effects want it.

## Stance and input

- The client reports a gait (still, walk, run) read off WoW's controller: still with no movement key bit set (a jump in place included), walk when the walk toggle (`MOVEFLAG_WALK_MODE`) is on, else run, backpedalling included; why: the brief says the walk toggle decides, and `Player::move_flags()` already carries both; cost if wrong: one pure function (`stance::gait_of`).
- The host trusts the reported gait as it trusts the pose (friends' co-op, §8.3), and takes no speed check; cost if wrong: a cheater sneaks at a run.
- Stealth travels as a wrapping count of presses (`PlayerInput::stealth_presses`), not a flag or an edge: an input replaced before the host steps keeps its presses, and the host toggles once per odd difference; the first count a hero's host sees is a baseline, so a new run never toggles on the last run's presses; cost if wrong: a press lost or doubled (pinned by tests).
- Stealth sits on `1`, read as a raw key and skipped while an edit box has the keyboard; pressing `1` also presses the empty action button 1, which does nothing; why: sub-project 4 will put Stealth on the action bar, likely there; cost if wrong: one constant.
- The host owns the hero's speed (×`stealth_speed` in stealth) and the client's controller takes it through the existing `UnitSpeeds` path in `view::write_vitals`; why: one authority, no new client code; cost if wrong: a round trip's lag on a joined client's speed change.
- Autoplay reports its own gait from `NIGHTFALL_AUTOPLAY_GAIT` (walk by default) because the autoplay walk moves the body with no key held; `sneak` presses Stealth every 2 s until the hero is in it; cost if wrong: test hooks only.

## Visibility

- The rear cone is its own width (`behind_cone`, 120°) and the sides are what lies between it and the front cone; a sentry's "wide front cone" is `sentry_cone` 180°; why: the spec gives "front 120°" and "sides" and "behind" without widths; cost if wrong: settings.
- Facing compares ground directions only (heights ignored); why: Duskwood's slopes are gentle against a 35-yard range; cost if wrong: a hero directly below a balcony counts as in front.
- Each hero's stance, stealth, visibility and noise ride in the snapshot to every player; why: the client needs its own for the eye, rings and vignette, and four heroes' worth is ten bytes each; cost if wrong: none.

## Noise

- A moving hero steps every `steps.every` (0.5) seconds, the first step at once; why: the spec's "every step" needs a cadence; cost if wrong: a setting.
- A noise sits at its source's floor and is heard at chest height over it, by ears at eye height, over ground distance; why: a ray to the floor point grazes the ground; cost if wrong: none found.
- At most one noise per hero counts per enemy per step (the loudest); why: a footstep and an ability in the same step should not add 70; cost if wrong: slower alerts from stacked noises.
- Every noise has a maker (`by`); why: suspicion is per hero (§5.3), and every noise in the spec's table is made by a hero (ravens set off by one); cost if wrong: a noise with no maker needs a "nobody" suspicion in sub-project 6.
- The rings show the loudest noise of the last `noise_memory` (1) second: one ring under 10 yards, two under 20, three from 20; cost if wrong: a pure function.

## Awareness

- A check keeps going until suspicion falls below `suspicious`, not `check` (hysteresis); at the place it stands until calm; why: draining 10 a second from a single noise's 70 would stop it after a second, so no check would reach its place; cost if wrong: one condition (a setting would let both be playtested, per the user's habit of toggling undecided designs).
- The hero an enemy is most suspicious of drives its state; ties go to the lower player number, for determinism; cost if wrong: none.
- An alerted enemy holds its suspicion at `alert` and keeps track of its hero by plain sight (in the fog, in line of sight, any facing), not by the notice range; so stealthing in its view does not shake it, breaking line of sight does; why: §5.3 says it "loses sight"; cost if wrong: chases last longer than players expect (Vanish, sub-project 4, will need to clear an alert).
- On losing its hero an enemy searches with its suspicion of them set to `check`, its 15 seconds counting only once it is there; a place no path reaches is searched from where it stands; the search ends with every suspicion at 0; cost if wrong: settings and one rule.
- Shouts reach allies within `shout` yards of the shouter regardless of walls, and do not chain; an ally already alerted keeps its own hero; a hit's alert (`World::alert`) shouts too, at the next step; why: no chain keeps one mistake from waking the map; cost if wrong: a chaining rule (a natural setting to playtest).
- Marks: "!" when alerted to you, "?" when searching for you or at least `suspicious` of you; derived per player on the host and sent in `State`; cost if wrong: none.
- Enemies' reactions search their paths from their own allowance (`paths_per_step` again, beside the patrols'), with the wanderers' small budget, and a goal no path reaches is retried after 1 second; a chase goes straight when `walk_line` allows; why: bounded work every step; cost if wrong: chases that give up on long detours (they then search where they stand).
- An enemy held by its senses drops its path; calm again, its brain searches its way back to its post or to the patrol point it was walking to; cost if wrong: none.
- The ambush and the openers are per attacker: an enemy alerted to your friend can still be ambushed by you (§5.4 says "alerted to its attacker"); seams answer on the host only (a mirror has no minds); cost if wrong: none.

## The client

- An enemy not in our seen set is `Concealed` and untrackable (`UNIT_FIELD_BYTES_1` byte 3 bit `0x4`, which `unit_dot_eligible` reads); its body is still drawn; a current target that becomes concealed is kept, as WoW keeps a target out of sight; why: clearing would drop targets behind every pillar in sub-project 4's fights; cost if wrong: one system writing `DeselectGuid`.
- The plate gate now hides on `byte 3 & 3` (ghost or creep), which is what the reference tests (`0x607101`, `0x60f62e`); the engine read only the ghost bit; the fix is engine-wide but nothing in Survivors stealths; cost if wrong: none found.
- Stealth's look is WoW's own: the creep bit (stealth walk, no plate, footstep sounds already muted for it) and Stealth Rank 3 (1786) in the first aura slot, ringing the aura watch as Survivors' auras do, for the translucency and the buff icon; the hero's first aura flag word is written whole (heroes carry no other aura yet); cost if wrong: if the aura's kit does not arm on the trimmed data the hero is not faded (the task's manual check says to note it).
- "?" and "!" are Bevy UI text over the head (2.7 yards × the model's scale), hidden while concealed; not combat text, which rises, fades and stays where it was spawned; cost if wrong: a styling change.
- The eye sits under the player frame's portrait, placed in WoW's 768-high interface units, and shows its number for tuning; its rings and colours are our own UI shapes (no textures needed); cost if wrong: a styling change.
- The heartbeat is a stand-in thump (`Sound\Effects\DeathImpacts\mDeathImpactSmallDirtA.wav`, played twice 0.16 s apart, 0.45 to 0.9 s between beats by distance) through Lua's `PlaySoundFile`, because the trimmed data holds no heartbeat sound; cost if wrong: one constant.
- The vignette tightens by strength and by its inner edge: the night grade's spare uniform (`unused`) becomes `vignette_from`, eased at 3 a second toward values lerped from `vignette_seen` to `vignette_hidden` in `rules.ron`; why: §10 says "tightens", which a strength alone does not; cost if wrong: two settings.
- The heartbeat distance and vignette numbers live in `rules.ron` under `senses` though the host ignores them, beside the spec's other numbers; cost if wrong: none.
- §11's `notice_range` and `stealth_speed` keep their names, nested under `senses:` as sub-project 2 nested its own under `world:`; cost if wrong: none.

## The wire

- `State` gains `marks` beside `seen`; the protocol goes to 3; `Hero` gains `stance`, `stealthed`, `visibility`, `noise`; `Enemy` gains `asleep`; cost if wrong: none.
