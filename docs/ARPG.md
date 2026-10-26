# Azeroth ARPG

A top-down action RPG on WoW's world, in the spirit of Diablo and Path of Exile. It is its own
game, not a mode of the survivors arena: nothing auto-casts and nothing needs a target. You walk
with WASD, aim with the mouse, and press a skill: it goes where the cursor is.

- Rules: `crates/arpg-sim` (headless, 30 Hz, no Bevy). Tests: `cargo test -p arpg-sim`.
- Client: `crates/benilla-app/src/arpg/` (camera, input, drawing; decides nothing).
- Content: `content/arpg/` (`abilities.ron`, `classes.ron`, `enemies.ron`, `zones.ron`), read at
  startup, so edits need no rebuild.
- Run: the `azeroth-together` exe with `ARPG=1`. `ARPG_CLASS`, `ARPG_ZONE`, `ARPG_CONTENT`,
  `ARPG_SEED`, `ARPG_LOOK=<n>` (the mage's look), `ARPG_CAM_DIST=<yards>`, `ARPG_HOUR=<0..24>`,
  `ARPG_AUTOPLAY=1` (circles, aims at the nearest enemy, goes through the spells; for test shots
  with the usual `WOW_BG=1 WOW_LIVE_SHOT=...`).

## How it plays

- **Abilities are aimed shapes** (`Shape` in `content.rs`): `Projectile` (a real missile that flies
  and hits the first enemy on its line; pierce, fan, splash), `Ground` (an area at the cursor,
  pulled in to range; delay, lasting ticks), `Cone` (toward the cursor), `Nova` (around you),
  `Guard` (Ice Block, Mana Shield), `Blink` (toward the cursor).
- **Casting**: a cast time roots you while it lasts; channels (Blizzard, Arcane Missiles) go on
  while the key is held and you stand still; walking or letting go stops them. Held keys recast as
  soon as they can.
- **Statuses**: Chill slows movement and windups; Freeze stops an enemy, and every hit on a frozen
  enemy is a critical strike (Shatter); burns tick each second.
- **Enemies telegraph every blow**: a red wedge (melee swing), circle (slam) or line (thrown
  missile) fills up, then the blow lands on whoever is still in it. Stepping out is the defence.
- **Dodge roll** (Space), every class, as in Path of Exile 2: a quick 5.5-yard roll toward where
  you walk (or ahead, standing still), 0.35 s, a 0.3 s pause before the next. It breaks off a cast
  and takes you out of a telegraph; it does not make you untouchable. Tuned per class in
  `classes.ron` (`dodge`).
- **The world is laid out like WoW's**, not a survivors arena: you start at Northshire Abbey and
  the valley's camps stand where WoW's do. Young wolves on the slopes east of the Abbey, kobold
  vermin in the fields north, kobold workers and laborers at Echo Ridge Mine, Defias thugs and
  Garrick Padfoot in the vineyards across the river, with Defias casters among them: Rogue
  Wizards throw Frostbolts, Apprentices slower, harder Fireballs (not in WoW's Northshire; there
  to dodge missiles). Their windup shows WoW's casting glow and a red line where the bolt goes. An enemy stays home until you come within
  its aggro range (9 yards) or hit it, which brings its idle neighbours too. Pulled more than 35
  yards from home it gives up ("Evade"), runs back whole and can't be hurt on the way. The dead
  come back after a minute (laborers 90 s, Garrick 5 min). No spawner, shrines, chests or bosses.
- **Falling**: Enter wakes you at the Abbey, whole; the enemies go home and forget you.

## Controls (mage)

| key | slot | spell |
|---|---|---|
| Left mouse | 1 | Frostbolt: bolt, chills |
| Right mouse | 2 | Fireball: slower, splash, burn |
| Q | 3 | Frost Nova: freezes around you |
| E | 4 | Cone of Cold: cone toward cursor, chills |
| R | 5 | Blizzard: channelled area at cursor |
| F | 6 | Flamestrike: area at cursor after a short delay, burning ground |
| 1 | 7 | Ice Block: immune, can't act, 4 s |
| Shift | 8 | Blink: 12 yards toward cursor |
| Space | – | Dodge roll (every class) |

WASD walks (up is north), the wheel zooms, Enter wakes you at the Abbey after you fall. Also in the content
but not on the bar: Arcane Missiles (channel), Dragon's Breath (TBC; shown with Fire Blast's look,
1.12 has none), Mana Shield. Swap them into `classes.ron` to try them.

## Decisions made without you (to review)

1. **Its own crate, not a toggle**: `arpg-sim` shares no code with `together-sim` (V2/Rng copied).
2. **Key layout** above. W is walking, so skills are LMB/RMB/Q/E/R/F/1; Space is the roll (as in
   PoE2), so Blink moved to Shift.
3. **Camera**: fixed 60° tilt, north up, 28 yards out, wheel 12–55. No rotating.
4. **The sim owns the hero's movement** (WASD in the sim, flat plane), not WoW's character
   controller; the client puts bodies on the terrain. No collision with trees, houses, the river
   or the valley walls yet: you can walk up cliffs and through the Abbey.
5. **Missiles use WoW's own visuals**: the engine's missile flies to a marker at the end of the
   sim's flight at the spell's Spell.dbc speed, so the sim's speeds in `abilities.ron` must match
   Spell.dbc (Frostbolt 28, Fireball 24). If they look out of step, that is why.
6. **Numbers**: mage 300 hp, 200 mana (9/s). Frostbolt 30 dmg 0.35 s cast; Fireball 55 + 16 burn,
   0.8 s cast; Frost Nova 12 + 4 s freeze, 12 s cd; Blizzard 20/s for 6 s; Flamestrike 60 + 10/s.
   Enemies: wolves 40 hp, kobolds 50–70, laborers 140 (slam), thugs 90, Garrick 450 (slam). All
   guesses; the mage is far stronger than a level-1 WoW mage.
7. **Camps placed by hand.** The game data has no spawn tables, so I probed the terrain over the
   valley (`together-port spot`: subzone, height, water) and placed 14 camps (about 35 enemies)
   from memory of Northshire, each checked to stand on dry ground in the right subzone. Where
   WoW's Kobold Vermin and Workers have no model of their own in the data, they borrow other
   kobold models (displays 139 and 373); Garrick wears a Defias Cutpurse's.
8. **The roll has no invulnerability** (PoE2's way): it only moves you. There is no WoW roll
   animation, so the mage runs at the roll's speed for 0.35 s.
9. **Ranged enemies are Defias casters**, added to the vineyards for testing (WoW's Northshire has
   none). Knife throwers or archers were left out: WoW draws a thrown knife or arrow from the
   thrower's own ranged weapon, which these units have none of yet. Their bolts don't chill you.
10. **Look**: a human mage trainer's model in robes with a staff (`ARPG_LOOK` picks another).
11. **No progression yet**: no levels, loot, quests or talents.
12. **Co-op left for later**; inputs and state are serde types so `together-net`'s pattern can carry them.

## Test checklist (by hand)

Build and run:

```powershell
$env:PATH = "C:\Users\Olivs\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:PATH"
cd D:\Games\azeroth-together-arpg
cargo build --release -p azeroth-together
$env:ARPG = "1"; .\target\release\azeroth-together.exe; Remove-Item Env:ARPG
```

1. The world loads at Northshire Abbey, the camera looks down from the south, a mage stands there.
2. WASD walks the right way on screen; the mage faces the cursor and backpedals/strafes.
3. A blue ring follows the cursor on the ground, on hills too.
4. LMB: Frostbolt flies toward the cursor, hits the first enemy on the line, misses ones off it;
   the mage stops while casting. Holding LMB keeps casting.
5. RMB Fireball: slower, splashes, burn numbers tick after.
6. Q Frost Nova freezes nearby enemies (ice on them); Frostbolts on them crit.
7. E Cone of Cold hits only what is in front, toward the cursor.
8. R Blizzard: ice falls where the cursor was, ticks while held; walking stops it.
9. F Flamestrike: pillar at the cursor after a beat, ground burns; far clicks land at max range.
10. 1 Ice Block: blows show "Absorb", nothing gets through for 4 s.
11. Space Blink: jumps toward the cursor.
12. Enemy windups show red wedges/circles/lines; stepping out in time takes no damage.
    Space rolls the way you walk, out of a windup, and breaks off a cast.
    In the vineyards the Defias casters glow while they wind up and a red line shows the bolt's
    path; walking or rolling off the line makes it miss.
    Wolves east of the Abbey stand still until you come close; hitting one brings its pack; pulled
    far away they say "Evade" and run home; the dead come back after a minute. The mine and the
    vineyards have their camps.
13. Cooldowns darken the slots and count down; the cast bar fills; health/mana bars move.
14. Falling shows the message; Enter wakes you at the Abbey.
15. Things to watch: missiles out of step with hits, camps standing somewhere odd, animations
    (casting, running, death, the roll), frame rate.
