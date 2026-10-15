# The mage: spells and augments (draft for review)

A first pass at the augment system on one class, to review before any code. See
[DESIGN-IDEAS.md](DESIGN-IDEAS.md) section C for the rules. **Numbers are placeholders** for the
idea's size, not balance.

Rules in short:

- Spells rank up to **14**; at **rank 7** and **rank 14** an Augment screen offers 3 augments, drawn
  from the spell's own (below) and the shared library (end of this doc) that fit its tags.
- Pick, reroll or **hold** (the spell stays at its rank until augmented).
- **At most one conversion per spell.**
- A spell cast by a **trigger** is the real spell, with your rank, augments and modifiers, and
  ignores its cooldown; if you do not own it, it casts at rank 1.

## Status effects the mage uses

| Effect | What it does |
|---|---|
| **Chill** | Slowed 40% for 3 s. The Slow tag. |
| **Freeze** | Rooted for 2 s; counts as Frozen (Shatter effects crit Frozen enemies). Elites are frozen for half as long; bosses are Chilled instead. |
| **Burning** | Fire damage over time (Ignite and friends). Burns of the same source refresh, they do not stack. |
| **Stun** | Cannot move or attack. Bosses resist it. |

## The 12 spells

Single-target spells **prioritize elites and bosses**: they hit the nearest elite or boss in range
before anything else. That is their place beside the area spells.

| # | Spell | Tags | Base behaviour | Role |
|---|---|---|---|---|
| 1 | **Frostbolt** (start) | Frost · Projectile · Direct · Slow | A bolt at the nearest enemy (elites and bosses first); Chills. | Single target, control |
| 2 | **Fireball** | Fire · Projectile · Direct · Area | A bolt that bursts in 3.5 yd. | Single target with splash |
| 3 | **Arcane Missiles** | Arcane · Projectile · Direct | 3 missiles at the 3 nearest enemies. | Spread damage |
| 4 | **Frost Nova** | Frost · Area · Freeze | Damages and Freezes everything within 9 yd. 7 s cooldown. | Control, panic button |
| 5 | **Arcane Explosion** | Arcane · Area · Direct | Hits everything within 7 yd. Short cooldown. | Close-range clear |
| 6 | **Blizzard** | Frost · Ground · Area · Slow | Ice falls on the densest group within 20 yd for 6 s, Chilling. | Zone control |
| 7 | **Flamestrike** | Fire · Ground · Area · Damage over time | A pillar of fire around you; leaves burning ground for 4 s. | Burst clear |
| 8 | **Fire Blast** | Fire · Direct | Instant, short range (18 yd), elites and bosses first. | Single target, burst |
| 9 | **Cone of Cold** | Frost · Area · Slow | A wide cone ahead of you; Chills. | Close control |
| 10 | **Blast Wave** | Fire · Area · Direct · Slow | A ring of fire around you; knocks back slightly and Chills. | Breathing room |
| 11 | **Pyroblast** | Fire · Projectile · Direct · Damage over time | A huge, slow bolt (elites and bosses first) that leaves Burning. Long cooldown. | Boss killer |
| 12 | **Ice Barrier** (new) | Frost · Shield | Every 20 s, a shield worth 25% of your max health. | Defense, trigger hub |

Ice Barrier is new to the game: the defensive spell that augments and items can hang effects on (your
"when it breaks, cast Frost Nova" idea).

## Spell augments

Two per milestone per spell. Rank 7 shapes the spell; rank 14 is the capstone.

### 1. Frostbolt
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Glacial Spike** | Every 4th Frostbolt Freezes its target. |
| 7 | **Splinter** | Pierces 2 more enemies, −30% damage after the first. |
| 14 | **Shatterbolt** | Always crits Frozen enemies, and those crits splash 50% in 5 yd. |
| 14 | **Winter's Chill** | Each hit makes the target take +2% crit chance from *every* source, stacking to 10% (party-wide in co-op). |

### 2. Fireball
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Lingering Flame** | Leaves the target Burning for 40% of the hit over 4 s. |
| 7 | **Volley of Flame** | 2 more Fireballs at other enemies, −35% damage each. |
| 14 | **Hot Streak** | Two Fire crits in a row cast Pyroblast (yours, augmented, if you own it; else rank 1). |
| 14 | **Critical Mass** | When the hit crits, the splash crits too; Fire crits deal 2.5× instead of 2×. |

### 3. Arcane Missiles
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Barrage** | +2 missiles. |
| 7 | **Seeking Missiles** | A missile whose target dies flies on to the next nearest enemy. |
| 14 | **Arcane Charge** | Each cast gives a charge (max 4, +10% Arcane damage each); at 4, your next Arcane Explosion spends them for +100% damage and area. |
| 14 | **Missile Storm** | Every 10 s, one missile at every enemy in range. |

### 4. Frost Nova
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Permafrost** | Freeze lasts 1 s longer, and enemies stay Chilled after it thaws. |
| 7 | **Shattering Nova** | +150% damage, no longer Freezes. |
| 14 | **Frostbite** | A Frozen enemy that dies shatters for 50% of its max health as Frost damage in 4 yd: chain reactions. |
| 14 | **Cold Snap** | Freezing 10 or more enemies at once resets the cooldowns of all your Frost spells. |

### 5. Arcane Explosion
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Arcane Wave** | Hits a ring from 5 to 12 yd instead of a circle; +50% damage. |
| 7 | **Arcane Concentration** | Each enemy hit takes 0.1 s off the cooldown (up to 1 s). |
| 14 | **Arcane Potency** | Hitting 15 or more enemies makes your next Arcane spell deal triple damage. |
| 14 | **Blink Detonation** | Using your movement button casts Arcane Explosion where you leave and where you land. |

### 6. Blizzard
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Ice Storm** | Follows you instead of staying put. |
| 7 | **Improved Blizzard** | Each tick has a 15% chance to Freeze an enemy for 1 s. |
| 14 | **Final Gale** | Lasts 50% longer; when it ends, it casts Frost Nova at its centre. |
| 14 | **Whiteout** | Enemies inside take +20% damage from every source (the whole party). |

### 7. Flamestrike
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Pillar of Flame** | Lands on the densest group within 25 yd instead of around you. |
| 7 | **Scorched Earth** | The burning ground lasts twice as long and is 30% wider. |
| 14 | **Firestorm** | 2 more Flamestrikes on other groups, −40% damage each. |
| 14 | **Meteor** | +200% damage, +100% cooldown, stuns for 1.5 s. |

### 8. Fire Blast
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Impact** | Stuns its target for 1 s and spreads the target's Burning to 3 nearby enemies. |
| 7 | **Twin Blast** | 2 charges. |
| 14 | **Ignition** | Always crits Burning enemies. |
| 14 | **Inferno** | A Fire Blast that kills casts Fire Blast again at the next target. |

### 9. Cone of Cold
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Frozen Cone** | The first 5 enemies hit are Frozen instead of Chilled. |
| 7 | **Ring of Cold** | A full ring around you instead of a cone, −25% damage. |
| 14 | **Shard Burst** | +100% damage against Frozen enemies. |
| 14 | **Wake of Winter** | Using your movement button casts Cone of Cold behind you. |

### 10. Blast Wave
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Concussive Blast** | Knocks enemies back 4 yd. |
| 7 | **Dragon's Breath** | A cone ahead of you instead of a ring; +60% damage; disorients for 2 s. |
| 14 | **Blazing Speed** | Each enemy hit gives you +3% move speed for 4 s, up to 30%. |
| 14 | **Living Wave** | Leaves enemies Burning, and a Burning enemy that dies sends out a small Blast Wave. |

### 11. Pyroblast
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Molten Core** | The Burning part deals twice as much and lasts 8 s. |
| 7 | **Pyroclasm** | Stuns its target for 2 s and splashes 40% in 5 yd. |
| 14 | **Presence of Mind** | Every 3rd Pyroblast fires a second Pyroblast at another target. |
| 14 | **Kindling** | Every Fire crit takes 0.5 s off Pyroblast's cooldown. |

### 12. Ice Barrier
| Rank | Augment | Effect |
|---|---|---|
| 7 | **Thick Ice** | Absorbs 50% more. |
| 7 | **Frost Armor** | Enemies that strike the shield are Chilled. |
| 14 | **Shattering Barrier** | When it breaks or expires: 3× the damage it absorbed in 15 yd, and it casts Frost Nova. |
| 14 | **Ice Floes** | While the shield holds, your Frost spells' cooldowns are 25% shorter. |

## Shared augments (the tag library, first 22)

Open to any spell with the tag, any class. The mage would see these mixed into its offers.

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
| Ground | **Lingering** | Lasts 50% longer. |
| Ground | **Mobile** | Follows you. |
| Damage over time | **Contagion** | When the target dies, the effect jumps to a nearby enemy. |
| Damage over time | **Quickened** | Ticks 30% faster. |
| Direct | **Precise** | +20% crit chance. |
| Direct | **Executioner** | +100% damage to enemies below 30% health. |
| Slow | **Deep Chill** | Its Chill has a 15% chance to Freeze. |
| Slow | **Cold Bite** | +25% damage to Chilled enemies. |
| Freeze | **Lasting Frost** | Freeze lasts 1 s longer. |
| Freeze | **Shatter** | +50% crit chance against Frozen enemies. |
| Shield | **Fortified** | Absorbs 30% more; while it holds, +15% damage. |
| Any | **Heavy** | +60% damage, +40% cooldown. |
| Any | **Twin** | Casts twice, −40% damage each. |

## Checks

**The frostfire benchmark** (DESIGN-IDEAS section D) works with this draft, without any talent web:
1. Blast Wave Chills. At rank 7 it rolls **Frostfire** (shared), so it counts as Frost too; **Deep
   Chill** on a slowing spell, or later a talent, lets Chills Freeze.
2. Pyroblast at 7: **Frostfire**. It crits Frozen enemies through **Shatter** (shared, on a Frost Nova
   or Cone of Cold build).
3. Fireball at 7: **Frostfire**, so it Chills; at 14: **Hot Streak** casts the augmented Pyroblast.

**Your Ice Barrier example** works as a build: Ice Barrier with **Thick Ice** and **Shattering
Barrier**, plus Frost Nova with **Shattering Nova** (+150% damage, no Freeze). The barrier breaks
often in melee and fires a huge Frost Nova each time, ignoring its cooldown.

**What to look at in review:**
- Is any augment a "must take" that makes the others pointless?
- Does every spell have one rank-7 option that keeps it simple and one that changes how it plays?
- Are there two capstones that make each other absurd (Cold Snap with Frostbite, say)?
