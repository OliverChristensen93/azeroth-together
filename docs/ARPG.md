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
- A spawner keeps enemies coming around you, more each minute (Hogger can turn up after 3 min).

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
| Space | 8 | Blink: 12 yards toward cursor |

WASD walks (up is north), the wheel zooms, Enter starts over after you fall. Also in the content
but not on the bar: Arcane Missiles (channel), Dragon's Breath (TBC; shown with Fire Blast's look,
1.12 has none), Mana Shield. Swap them into `classes.ron` to try them.

## Decisions made without you (to review)

1. **Its own crate, not a toggle**: `arpg-sim` shares no code with `together-sim` (V2/Rng copied).
2. **Key layout** above. W is walking, so skills are LMB/RMB/Q/E/R/F/1/Space.
3. **Camera**: fixed 60° tilt, north up, 28 yards out, wheel 12–55. No rotating.
4. **The sim owns the hero's movement** (WASD in the sim, flat plane), not WoW's character
   controller; the client puts bodies on the terrain. No collision with trees, houses or water
   yet, and spawns can land in a lake or a house.
5. **Missiles use WoW's own visuals**: the engine's missile flies to a marker at the end of the
   sim's flight at the spell's Spell.dbc speed, so the sim's speeds in `abilities.ron` must match
   Spell.dbc (Frostbolt 28, Fireball 24). If they look out of step, that is why.
6. **Numbers**: mage 300 hp, 200 mana (9/s). Frostbolt 30 dmg 0.35 s cast; Fireball 55 + 16 burn,
   0.8 s cast; Frost Nova 12 + 4 s freeze, 12 s cd; Blizzard 20/s for 6 s; Flamestrike 60 + 10/s.
   Enemies: wolves and kobolds 35–80 hp, wizards throw fireballs, Kobold Miner slams. All guesses.
7. **Look**: a human mage trainer's model in robes with a staff (`ARPG_LOOK` picks another).
8. **No progression yet**: no levels, loot or talents; you fight until you fall, then start over.
9. **Co-op left for later**; inputs and state are serde types so `together-net`'s pattern can carry them.

## Test checklist (by hand)

Build and run:

```powershell
$env:PATH = "C:\Users\Olivs\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:PATH"
cd D:\Games\azeroth-together-arpg
cargo build --release -p azeroth-together
$env:ARPG = "1"; .\target\release\azeroth-together.exe; Remove-Item Env:ARPG
```

1. The world loads in Elwynn near Goldshire, the camera looks down from the south, a mage stands there.
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
13. Cooldowns darken the slots and count down; the cast bar fills; health/mana bars move.
14. Falling shows the message; Enter starts over.
15. Things to watch: missiles out of step with hits, enemies spawning in water or houses,
    animations (casting, running, death), frame rate with 40 enemies.
