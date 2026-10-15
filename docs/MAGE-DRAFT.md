# The mage: spells and augments (draft for review)

A first pass at the augment system on one class, to review before any code. See
[DESIGN-IDEAS.md](DESIGN-IDEAS.md) section C for the rules. **Numbers are placeholders** for the
idea's size, not balance.

## How augments work (draft 2)

- Spells rank up to **15**.
- At **rank 5** and **rank 15** an Augment screen offers 3 **general** augments from the shared library
  (end of this doc) that fit the spell's tags. Pick, reroll or **hold** (the spell stays at its rank
  until augmented).
- At **rank 10** the spell's **capstone** screen always offers the same **3 unique augments**; you
  take 1. Knowing them in advance lets you plan toward one.
- **At most one conversion per spell.**
- A spell cast by a **trigger** is the real spell, with your rank, augments and modifiers, and
  ignores its cooldown; if you do not own it, it casts at rank 1.
- **Triggers and resets have an internal cooldown** (say 8 s), so two of them can never loop.

Rules for writing augments:

- **A unique augment must do something no shared augment can.** If a shared augment already does it
  (more projectiles, more area, following you), it is not a capstone.
- **Every spell's three capstones include at least one that makes you think differently about the
  spell** and may change how you play around it.
- **No general augment may dead-end a spell:** if one removes a property (Brute Force removes
  Freeze), at least one of that spell's capstones must still work without it.

## Status effects the mage uses

| Effect | What it does |
|---|---|
| **Chill** | Slowed 40% for 3 s. The Slow tag. |
| **Freeze** | Rooted for 2 s; counts as Frozen (Shatter effects crit Frozen enemies). Elites are frozen for half as long, bosses for a quarter as long. |
| **Burning** | Fire damage over time (Ignite and friends). Burns of the same source refresh, they do not stack. |
| **Stun** | Cannot move or attack. Bosses resist it. |
| **Knockback** | Pushed away from the caster. Bosses are not moved. |

## The 12 spells

**Targeting gives each spell its job.** "Elites first" means the nearest elite or boss in range,
else the nearest enemy.

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Frostbolt** (start) | Frost · Projectile · Direct · Slow | Elites first | A bolt that Chills. | Frost single target |
| 2 | **Fireball** | Fire · Projectile · Direct · Area | Elites first | A bolt that bursts in 3.5 yd, hitting the guards around its target. | The boss and its adds |
| 3 | **Arcane Missiles** | Arcane · Projectile · Direct | Elites first | 3 missiles at one target. | Arcane single target |
| 4 | **Frost Nova** | Frost · Area · Direct · Freeze | Around you | Damages and Freezes everything within 9 yd. 7 s cooldown. | Control, panic button |
| 5 | **Arcane Explosion** | Arcane · Area · Direct | Around you | Hits everything within 7 yd. Short cooldown. | Close-range clear |
| 6 | **Blizzard** | Frost · Ground · Area · Damage over time · Slow | Densest group within 20 yd | Ice falls for 6 s, Chilling. | Zone control |
| 7 | **Flamestrike** | Fire · Ground · Area · Damage over time | Around you | A pillar of fire; leaves burning ground for 4 s. | Burst clear |
| 8 | **Fire Blast** | Fire · Direct | **The enemy closest to you** | Instant, 12 yd, short cooldown. | The "get off me" spell |
| 9 | **Cone of Cold** | Frost · Area · Direct · Slow | Ahead of you | A wide cone; Chills. | Close control |
| 10 | **Blast Wave** | Fire · Area · Direct · Slow · Knockback | Around you | A ring of fire; knocks back and Chills. | Breathing room |
| 11 | **Pyroblast** | Fire · Projectile · Direct · Damage over time | Elites first | A huge, slow bolt that leaves Burning. Long cooldown. | Boss killer |
| 12 | **Ice Barrier** (new) | Frost · Shield | You | Every 20 s, a shield worth 25% of your max health. | Defense, trigger hub |

The three fire spells now do different jobs: Pyroblast kills the boss, Fireball clears the boss's
guards, Fire Blast saves you when something reaches you. (The alternative, if four elites-first
spells is too many: Fireball aims at the densest group instead.)

## Capstones (rank 10: 1 of 3)

★ marks the capstone that most changes how you play the spell.

### 1. Frostbolt
- **Glacial Spike:** every 4th Frostbolt is a Glacial Spike: +200% damage and Freezes its target.
- **Shatterbolt:** always crits Frozen enemies, and those crits splash 50% in 5 yd.
- ★ **Winter's Chill:** each hit makes the target take +2% crit chance from *every* source, stacking
  to 10%. In co-op it is the whole party's setup spell.

### 2. Fireball
- ★ **Hot Streak:** two Fire crits in a row cast Pyroblast (yours, with its augments, if you own it;
  else rank 1).
- **Critical Mass:** when the hit crits, the burst crits too; Fire crits deal 2.5× instead of 2×.
- **Living Bomb:** the target becomes a bomb that explodes after 3 s for 150% in 8 yd; enemies the
  explosion hits become smaller bombs (one generation).

### 3. Arcane Missiles
- **Arcane Charge:** each cast gives a charge (max 4, +10% Arcane damage each); at 4, your next Arcane
  Explosion spends them for +100% damage and area.
- **Missile Storm:** every 10 s, one missile at every enemy in range.
- ★ **Channelled Focus:** while you stand still, missiles fire continuously and gain +15% damage per
  second (up to +60%). Moving resets it.

### 4. Frost Nova
- **Frostbite:** a Frozen enemy that dies shatters for 50% of its max health as Frost damage in 4 yd:
  chain reactions.
- **Cold Snap:** hitting 10 or more enemies resets the cooldowns of your other Frost spells (at most
  once every 8 s). Works with or without Freeze.
- ★ **Frozen Core:** when you drop below 30% health, Frost Nova casts itself and you take 50% less
  damage for 3 s (once every 15 s). Lets you play closer to the edge.

### 5. Arcane Explosion
- **Arcane Potency:** hitting 15 or more enemies makes your next Arcane spell deal triple damage.
- **Blink Detonation:** using your movement button casts Arcane Explosion where you leave and where
  you land.
- ★ **Implosion:** first pulls every enemy within 12 yd toward you, then explodes. A gather tool for
  Frost Nova, Flamestrike and the party's area spells.

### 6. Blizzard
- **Final Gale:** lasts 50% longer; when it ends, it casts Frost Nova at its centre.
- **Whiteout:** enemies inside take +20% damage from every source (the whole party).
- ★ **Frozen Orb:** instead of falling in place, Blizzard is an orb rolling slowly away from you in
  the direction you face, Chilling and damaging all it passes. You aim it with your facing.

### 7. Flamestrike
- **Firestorm:** 2 more Flamestrikes on other groups, −40% damage each.
- **Meteor:** +200% damage, +100% cooldown, stuns for 1.5 s.
- ★ **Phoenix Ground:** standing in your own burning ground heals you 2% of max health per second and
  gives +20% Fire damage. You fight from inside the fire.

### 8. Fire Blast
- **Impact:** stuns its target for 1 s and spreads the target's Burning to 3 nearby enemies.
- ★ **Inferno:** a Fire Blast that kills casts Fire Blast again at the next closest enemy: one press
  can clear everything that reached you.
- **Ignition:** always crits Burning enemies.

### 9. Cone of Cold
- **Frozen Cone:** the first 5 enemies hit are Frozen instead of Chilled.
- **Shard Burst:** +100% damage against Frozen enemies.
- ★ **Wake of Winter:** using your movement button casts Cone of Cold behind you, so running away
  becomes an attack.

### 10. Blast Wave
- ★ **Blazing Speed:** each enemy hit gives you +3% move speed for 4 s (up to 30%): dive in, then
  outrun everything.
- **Living Wave:** leaves enemies Burning, and a Burning enemy that dies sends out a small Blast Wave.
- **Dragon's Breath:** a cone ahead of you instead of a ring; +60% damage; disorients for 2 s.

### 11. Pyroblast
- **Presence of Mind:** every 3rd Pyroblast fires a second Pyroblast at another target.
- **Kindling:** every Fire crit takes 0.5 s off Pyroblast's cooldown.
- ★ **Combustion:** Pyroblast consumes every Burning effect on its target and deals their remaining
  damage at once, doubled. Damage-over-time builds now set up Pyroblast.

### 12. Ice Barrier
- ★ **Shattering Barrier:** when it breaks or expires, it deals 3× the damage it absorbed in 15 yd and
  casts Frost Nova. You walk into the horde to break it.
- **Ice Floes:** while the shield holds, your Frost spells' cooldowns are 25% shorter.
- **Frost Armor:** enemies that strike the shield are Chilled and take 20% of the hit as Frost damage.

## Shared augments (the library for ranks 5 and 15, first 32)

Open to any spell with the tag, any class.

| Tag | Augment | Effect |
|---|---|---|
| Fire or Frost | **Frostfire** (conversion) | Counts as Fire *and* Frost. A Fire spell also Chills; a Frost spell also leaves Burning. |
| Fire or Arcane | **Spellfire** (conversion) | Counts as Fire *and* Arcane; +15% area. |
| Frost or Arcane | **Spellfrost** (conversion) | Counts as Frost *and* Arcane; −15% cooldown. |
| Projectile | **Multishot** | +2 projectiles, −25% damage each. |
| Projectile | **Piercing** | Passes through 2 enemies. |
| Projectile | **Returning** | Flies back to you, hitting again on the way. |
| Area | **Wide** | +40% area, −20% damage. |
| Area | **Concentrated** | −30% area, +40% damage. |
| Area | **Echo** | Repeats at half strength 1 s later. |
| Area | **Ring** | Hits a ring from 5 to 12 yd instead of a circle; +50% damage. |
| Ground | **Lingering** | Lasts 50% longer. |
| Ground | **Mobile** | Follows you. |
| Ground | **Targeted** | Lands on the densest group within 25 yd instead of around you. |
| Damage over time | **Contagion** | When the target dies, the effect jumps to a nearby enemy. |
| Damage over time | **Quickened** | Ticks 30% faster. |
| Damage over time | **Molten** | The damage-over-time part deals double and lasts twice as long; the direct part −30%. |
| Direct | **Precise** | +20% crit chance. |
| Direct | **Executioner** | +100% damage to enemies below 30% health. |
| Fire · Direct | **Ignite** | Leaves the target Burning for 40% of the hit over 4 s. |
| Slow | **Deep Chill** | Its Chill has a 15% chance to Freeze. |
| Slow | **Cold Bite** | +25% damage to Chilled enemies. |
| Freeze | **Lasting Frost** | Freeze lasts 1 s longer. |
| Freeze | **Permafrost** | Enemies stay Chilled after they thaw. |
| Freeze | **Shatter** | +50% crit chance against Frozen enemies. |
| Slow, Freeze or Stun | **Brute Force** | +120% damage, loses its control effect. |
| Knockback | **Concussive** | Knocks back 4 yd farther and stuns for 0.5 s. |
| Shield | **Fortified** | Absorbs 30% more; while it holds, +15% damage. |
| Shield | **Reflective** | 20% of what it absorbs is dealt back to the attacker. |
| Cooldown of 3 s or more | **Twin Charges** | 2 charges. |
| Any | **Heavy** | +60% damage, +40% cooldown. |
| Any | **Twin** | Casts twice, −40% damage each. |
| Any | **Hasted** | −25% cooldown, −15% damage. |

**Dead-end check for Brute Force** (it removes control): Frost Nova keeps Cold Snap and Frozen Core;
Cone of Cold keeps Wake of Winter; Frostbolt keeps Glacial Spike and Winter's Chill; Blizzard and
Blast Wave lose nothing their capstones need.

## Checks

**The frostfire benchmark** (DESIGN-IDEAS section D) works with this draft, without any talent web:
1. Blast Wave Chills. At rank 5 it rolls **Frostfire**, so it counts as Frost too; at 15, **Deep
   Chill**: its Chills can Freeze.
2. Pyroblast at 5: **Frostfire**; at 15: **Shatter**, so it crits Frozen enemies. Its capstone
   **Combustion** pays off every Burning effect the frostfire spells leave.
3. Fireball at 5: **Frostfire**, so it Chills; capstone **Hot Streak** casts the augmented Pyroblast.

**Your Ice Barrier example** works as a build: Ice Barrier with **Fortified** and the capstone
**Shattering Barrier**, plus Frost Nova with **Brute Force** (+120% damage, no Freeze) and **Cold
Snap**. The barrier breaks often in melee and fires a huge Frost Nova each time.

**Review notes:**
- Draft 1 (2026-10-15): balanced enough at first look; freeze durations set (elites half, bosses a
  quarter); Cold Snap made to work without Freeze.
- Draft 3 (2026-10-15): the capstone moved to rank 10 (it came too late at 15); general augments at 5
  and 15.
- Draft 2 (2026-10-15): ranks 5 and 10 general, rank 15 a fixed choice of 3 capstones; spell
  augments that duplicated shared ones cut or moved to the library; targeting per spell; Frost Nova
  Direct, Blizzard Damage over time, Blast Wave Knockback.
