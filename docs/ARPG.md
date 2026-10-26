# Azeroth ARPG

A third-person action RPG on WoW's world, in the spirit of TERA, Black Desert and Guild Wars 2's
action camera. It is its own game, not a mode of the survivors arena: nothing auto-casts and
nothing needs a target. The camera rides over your shoulder and the mouse always steers it; skills
go where the crosshair points. (It began as a top-down Diablo-style game; that camera is in the
branch history, before the commit that brought the action camera.)

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
- **Global cooldown**: a spell starts a 0.5 s global cooldown (its cast time counts toward it),
  so holding a key casts about twice a second; the slots darken while it runs. Blink and Ice Block
  are off it (`gcd: false` in `abilities.ron`), so an escape always answers.
- **Faster pacing**: Frostbolt is a 0.3 s cast you can walk during; Fireball 0.6 s and
  Flamestrike 0.4 s still root you. Mana still matters: Frostbolt 5, Fireball 15, regen 8/s, so
  steady casting slowly drains you.
- **Dodge roll** (Space), every class, as in Path of Exile 2: a quick 5.5-yard roll toward where
  you walk (or ahead, standing still), 0.35 s. It breaks off a cast and takes you out of a
  telegraph; it does not make you untouchable. It costs 40 **stamina** (the yellow bar, 100):
  two in a row, then a breather; it comes back 25/s once 0.8 s have passed since the last roll.
  All in `classes.ron` (`gcd`, `dodge`, `stamina*`).
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

The mouse turns the view (the cursor is locked and hidden while playing; a crosshair marks the
middle), WASD walks the way the camera looks, the wheel brings the camera closer or farther, Enter
wakes you at the Abbey after you fall. The crosshair aims at the first enemy along the view, else
the ground (a ring shows where ground spells land), else 45 yards ahead.
Escape opens WoW's own game menu (the world waits while it is up): Options, Sound, Interface,
Key Bindings and Exit Game work as in WoW; Logout is "Return to Abbey". Trees and other doodads
between the camera and the hero fade to a quarter, easing in and out as you walk under them. Also in the content
but not on the bar: Arcane Missiles (channel), Dragon's Breath (TBC; shown with Fire Blast's look,
1.12 has none), Mana Shield. Swap them into `classes.ron` to try them.

## Decisions made without you (to review)

1. **Its own crate, not a toggle**: `arpg-sim` shares no code with `together-sim` (V2/Rng copied).
2. **Key layout** above. W is walking, so skills are LMB/RMB/Q/E/R/F/1; Space is the roll (as in
   PoE2), so Blink moved to Shift.
3. **Camera**: over the right shoulder, 6 yards back (wheel 2.5–14), always steered by the mouse
   (no right-drag); the cursor comes back with the game menu or when the window loses focus.
   Space stays the roll (the sim has no jump), Blink stays on Shift.
4. **The sim owns the hero's movement** (WASD in the sim, flat plane), not WoW's character
   controller; the client puts bodies on the terrain. No collision with trees, houses, the river
   or the valley walls yet: you can walk up cliffs and through the Abbey.
5. **Missiles use WoW's own visuals**: the engine's missile flies to a marker at the end of the
   sim's flight at the spell's Spell.dbc speed, so the sim's speeds in `abilities.ron` must match
   Spell.dbc (Frostbolt 28, Fireball 24). If they look out of step, that is why.
6. **Numbers**: mage 300 hp, 200 mana (8/s), 0.5 s global cooldown, 100 stamina (a roll 40,
   back 25/s). Frostbolt 30 dmg 0.3 s cast on the move, 5 mana; Fireball 55 + 16 burn, 0.6 s
   cast, 15 mana; Frost Nova 12 + 4 s freeze, 12 s cd; Blizzard 20/s for 6 s; Flamestrike 60 + 10/s.
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
12. **WoW's interface is on, but hidden** apart from the game menu: action bars, unit frames,
    minimap and chat are hidden, and the panels WoW's key bindings would open (chat on Enter, the
    character sheet on C...) are turned off in Lua, without touching the saved key bindings.
13. **The see-through fade** (engine: `benilla_world::model_fade::SeeThrough`, a carry in
    `model_render/visibility.rs`) works on doodads drawn as entities, so the game turns the
    retained static pass and merged cells off (`WOW_STATIC_GX=0 WOW_STATIC_MERGE=0`); it may cost
    some frame rate. Buildings (WMOs) don't fade. The fade: 25% alpha, by how near the camera-hero
    line passes to a doodad's centre.
14. **Co-op left for later**; inputs and state are serde types so `together-net`'s pattern can carry them.

## Test checklist (by hand)

Build and run:

```powershell
$env:PATH = "C:\Users\Olivs\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:PATH"
cd D:\Games\azeroth-together-arpg
cargo build --release -p azeroth-together
$env:ARPG = "1"; .\target\release\azeroth-together.exe; Remove-Item Env:ARPG
```

1. The world loads at Northshire Abbey with the camera over the mage's right shoulder; the
   cursor is gone and a crosshair sits in the middle.
2. The mouse turns the view smoothly, up and down too; WASD walks the way you look; the mage
   faces the crosshair and backpedals/strafes. The wheel zooms. Esc gives the cursor back.
3. A blue ring sits where the crosshair meets the ground, or under the enemy it is on.
4. LMB: Frostbolt flies toward the cursor, hits the first enemy on the line, misses ones off it;
   the mage stops while casting. Holding LMB keeps casting.
5. RMB Fireball: slower, splashes, burn numbers tick after.
6. Q Frost Nova freezes nearby enemies (ice on them); Frostbolts on them crit.
7. E Cone of Cold hits only what is in front, toward the cursor.
8. R Blizzard: ice falls where the cursor was, ticks while held; walking stops it.
9. F Flamestrike: pillar at the cursor after a beat, ground burns; far clicks land at max range.
10. 1 Ice Block: blows show "Absorb", nothing gets through for 4 s.
11. Shift Blink: jumps toward the cursor.
12. Enemy windups show red wedges/circles/lines; stepping out in time takes no damage.
    Space rolls the way you walk, out of a windup, and breaks off a cast; two rolls empty most
    of the yellow stamina bar and a third waits for it. Holding LMB casts about twice a second
    while you keep walking, and the slots darken with the global cooldown; Blink answers at once.
    In the vineyards the Defias casters glow while they wind up and a red line shows the bolt's
    path; walking or rolling off the line makes it miss.
    Wolves east of the Abbey stand still until you come close; hitting one brings its pack; pulled
    far away they say "Evade" and run home; the dead come back after a minute. The mine and the
    vineyards have their camps.
13. Cooldowns darken the slots and count down; the cast bar fills; health/mana bars move.
14. Falling shows the message; Enter wakes you at the Abbey.
15. Escape opens WoW's game menu and the world stops; clicking its buttons casts nothing; Return
    to Abbey takes you home; Exit Game closes the game. Nothing else of WoW's UI shows, and Enter,
    C, R and the like open no WoW panels.
16. Walk under the trees east of the Abbey: the ones between the camera and the mage fade and come
    back as you leave; trees behind the mage stay solid.
17. Things to watch: missiles out of step with hits, camps standing somewhere odd, animations
    (casting, running, death, the roll), frame rate.
