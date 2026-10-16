# Every class: shared tags, mechanics and general augments (draft for review)

> **Built (2026-10-16):** every tag, status, mechanic, general augment and defensive button
> below, in `crates/together-sim/src/world/classes.rs` (the shared mechanics and the hooks) with each
> class's capstones in its own file beside it (`warrior.rs` ... `druid.rs`), and all eight classes
> drafted below. Each class has 12 spells, 36 capstones and its defensive button in `content/`.

The mage ([MAGE-DRAFT.md](MAGE-DRAFT.md)) proved the augment system on one class. This draft lists
what the other eight classes need on top of it, so each class draft can refer to it:
[WARRIOR](WARRIOR-DRAFT.md), [ROGUE](ROGUE-DRAFT.md), [PRIEST](PRIEST-DRAFT.md),
[WARLOCK](WARLOCK-DRAFT.md), [HUNTER](HUNTER-DRAFT.md), [SHAMAN](SHAMAN-DRAFT.md),
[PALADIN](PALADIN-DRAFT.md), [DRUID](DRUID-DRAFT.md).

**Numbers are placeholders** for the idea's size, not balance. The rules are the mage draft's and
[DESIGN-IDEAS.md](DESIGN-IDEAS.md) sections C, F and I: about 12 spells per class, 3 capstones per
spell (one of which changes how you play it), no general augment may dead-end a spell, single-target
spells hit bosses and elites first, every build has a survival answer, heal and block spells bank
charges (11b), heal and tank builds deal damage by overflow and retaliation (11c).

## New tags

| Tag | Family | What it means |
|---|---|---|
| **Bleed** | Effect | Physical damage over time (Rend, Rupture, Rake). |
| **Poison** | Effect | Nature damage over time that can **stack** (Deadly Poison, Serpent Sting). |
| **Shout** | Delivery | A roar around you that changes what enemies (or allies) do (Demoralizing Shout). |
| **Finisher** | Effect | Spends your combo points for more effect (rogue). |
| **Ward** | Delivery | Banks **charges** on its cooldown that spend themselves when needed (11b). |
| **Leech** | Effect | Heals you for part of the damage it deals (Bloodthirst, Drain Life, Death Coil). |

Existing tags take on their full meaning: **Heal** (restores health), **Fear** (enemies flee),
**Root** (enemies cannot move but still strike what reaches them), **Summon** (a creature that
fights for you: a pet, an imp, a voidwalker), **Totem** (a summon that stands where you put it),
**Chain** (jumps from enemy to enemy), **Aura** (a lasting effect on you and allies near you).

## New status effects

| Effect | What it does |
|---|---|
| **Bleeding / Poisoned** | Damage over time, like Burning. Bleeds and burns refresh; Poisons stack (up to 5 by default for a stacking spell). |
| **Fear** | Runs away from whoever feared it for 3 s. Cannot strike. Elites half as long, bosses immune. |
| **Root** | Cannot move for 3 s, still strikes a hero in reach. Elites half as long, bosses a quarter. |
| **Weakened** | Deals 25% less damage for 6 s. |
| **Marked** | Takes +X% damage from every source (the whole party) for a time: Hunter's Mark, Faerie Fire, Mortal Wounds. The strongest mark counts. |
| **Sundered** | Stacks: +4% damage taken per stack, up to 5, for 10 s. |

On heroes: **Guard** (takes less damage for a time; Frozen Core already had one), **Hidden**
(enemies lose you and wander; they come back after it ends), **Haste** (more move speed, as Blazing
Speed), **Combo points** (rogue, up to 5).

## New mechanics

- **Damage over time without a hit.** A spell tagged Damage over time but **not** Direct deals no
  hit: all its damage comes as the damage over time ("{dot} damage over {burn_secs} sec").
  Corruption, Shadow Word: Pain, Serpent Sting work this way. A Direct spell with a burn (Pyroblast,
  Immolate) hits and then burns, as before. Its length comes from the spell (12 s for Corruption).
- **Ward spells and charges (11b).** A Ward spell banks one charge each time its cooldown comes up,
  up to its cap ({charges}). Charges spend themselves:
  - **Heal charges:** when you or an ally within 30 yd drops below 50% health, a charge heals them
    for {heal}% of their health. One charge per blow, so a long fight spends several.
  - **Block charges:** the next blow that reaches you is blocked completely and the attacker takes
    {damage} (retaliation, 11c).
  - **Thorn charges:** the next enemy that strikes you takes {damage}; the blow still lands.
  - **Overflow (11c):** a Ward spell whose bank is full strikes instead: the nearest enemy (bosses
    and elites first) within its range takes {damage}. Solo, a healer at full health is a slow damage
    build and healing power is its damage stat: a heal charge heals for {heal}% scaled by every
    damage bonus the spell has.
- **Summons and totems.** A summon is a creature (or totem) that lasts {duration} sec and strikes on
  its own beat: a pet follows you and bites, a totem stands where you put it. They are Ground areas
  that strike instead of covering an area, so Lingering (longer), Mobile (a totem that follows you)
  and Targeted (put on the thickest crowd) work on them. Pets and totems cannot be hit and are not
  drawn as creatures yet (their attacks are).
- **Buffs.** A Buff spell puts an effect on you (an Aura spell also on allies within 20 yd) for a
  time, on its cooldown: more damage, faster cooldowns, guard, haste, thorns.
- **Chains.** A Chain spell hits its target, then jumps {jumps} times to the nearest enemy not yet
  hit within 8 yd, each jump at 80% of the last.
- **Combo points (rogue).** Every hit of a builder gives 1 combo point (up to 5). A Finisher spends
  them all: +25% damage per point (Rupture's bleed, Kidney Shot's stun and Slice and Dice's length
  grow instead). A finisher with no points still fires at its base.
- **Lifesteal.** A Leech spell heals you for a share of the damage it deals (its damage over time
  too).
- **Traps.** A trap is a ground area put at your feet that waits, armed, until an enemy steps into
  it, then goes off (Explosive Trap, Frost Trap). Unsprung traps fade after 30 s.
- **"The most wounded" targeting.** Execute picks the enemies in reach with the least health left,
  as a share of their maximum, bosses and elites first.

## The defensive button (10a)

Every class has one defensive spell on **F**, defined in `content/abilities.ron` and named by the
class in `content/classes.ron` (`defensive: "blink"`), with its own effect in the simulation. The
simulation owns its cooldown (the client only asks); a movement one is done by the client's
controller, which owns the hero's position, and reported with the input, as Blink is.

| Class | Button | Cooldown | Kind | Effect |
|---|---|---|---|---|
| Mage | **Blink** | 30 s | Movement | 10 yd the way you face. |
| Warrior | **Charge** | 30 s | Movement, control | 12 yd the way you face; enemies within 5 yd of where you land are Stunned for 1.5 s. |
| Rogue | **Vanish** | 45 s | Escape | Enemies lose you for 3 s; your next hits within 4 s deal +50%. |
| Priest | **Psychic Scream** | 30 s | Control | Every enemy within 10 yd flees in Fear for 4 s. |
| Warlock | **Soulstone** | 120 s | Escape | For 60 s, a killing blow leaves you at 40% health instead and every enemy within 10 yd flees in Fear. |
| Hunter | **Disengage** | 30 s | Movement, control | 12 yd the way you face; where you left, a Frost Trap that Chills everything in 6 yd for 6 s. |
| Shaman | **Ghost Wolf** | 30 s | Movement | +60% move speed for 6 s. |
| Paladin | **Divine Shield** | 90 s | Defensive | You take no damage for 6 s. |
| Druid | **Barkskin** | 45 s | Defensive | You take 40% less damage for 10 s. |

Augments for defensive buttons are for later (DESIGN-IDEAS 10a): "Charge: 2 charges", "Vanish heals
20%"; the button is a real ability, so the same augment system can reach it.

## New general augments (the shared library grows from 35)

Open to any spell with the tag, any class. The mage's 35 stay as they were.

| Tag | Augment | Effect |
|---|---|---|
| Fire or Shadow | **Shadowflame** (conversion) | Counts as Fire and Shadow. A Fire spell heals you for 10% of its damage; a Shadow spell also leaves Burning. |
| Holy or Shadow | **Twilight** (conversion) | Counts as Holy and Shadow. +20% damage to bosses and elites. |
| Holy or Fire | **Sacred Flame** (conversion) | Counts as Holy and Fire. A Holy spell also leaves Burning; a Fire spell heals you for 10% of its damage. |
| Physical or Nature | **Thunderstrike** (conversion) | Counts as Physical and Nature. Each hit arcs to one more enemy within 8 yd for 50%. |
| Arcane or Nature | **Balance** (conversion) | Counts as Arcane and Nature. +15% critical strike chance. |
| Frost or Nature | **Frostbrand** (conversion) | Counts as Frost and Nature. A Nature spell also Chills; a Frost spell also Poisons (30% over 4 s). |
| Physical or Holy | **Crusader** (conversion) | Counts as Physical and Holy. Each hit heals you for 2% of your health (at most once a second). |
| Physical or Shadow | **Shadowblade** (conversion) | Counts as Physical and Shadow. +25% damage to enemies that are Feared, Stunned or Rooted. |
| Bleed | **Lacerate** | Its Bleed stacks up to 3 times. |
| Bleed | **Hemorrhage** | Enemies it makes Bleed take +20% from your Physical hits. |
| Poison | **Crippling** | Poisoned enemies are Chilled. |
| Poison | **Virulent** | Its Poison stacks 3 more times. |
| Fear | **Dread** | Feared enemies take +25% damage from you. |
| Fear | **Panic** | Its Fear lasts 1.5 s longer. |
| Root | **Bramble** | Rooted enemies take +25% damage from you. |
| Root | **Grasping** | Its Root lasts 1.5 s longer. |
| Stun | **Crushing** | +40% damage to Stunned enemies. |
| Chain | **Forked** | Jumps 2 more times. |
| Chain | **Conductive** | Its jumps lose no damage. |
| Summon or Totem | **Pack** | Two at once, -35% damage each. |
| Summon or Totem | **Feral Bond** | Its strikes heal you for 5% of their damage. |
| Summon or Totem | **Frenzy** | Strikes 40% faster. |
| Shout | **Booming Voice** | +50% area. |
| Shout | **Commanding** | Its effects last 50% longer. |
| Ward | **Deep Reserves** | Holds 2 more charges. |
| Ward | **Overflowing** | Its overflow strikes 2 enemies for +50%. |
| Heal | **Mending Link** | A heal it gives also heals every other hero within 10 yd for half as much. |
| Heal | **Renewing** | A heal it gives also heals 30% more over 6 s. |
| Leech | **Sanguine** | Lifesteal past full health becomes a shield, up to 20% of your health. |
| Leech | **Bloodthirsty** | Heals you for 10% more of its damage. |
| Melee | **Sweeping** | Its single-target hits also strike one enemy beside the target for 50%. |

That is 31 new general augments: 66 in the library. Brute Force (removes control) now also removes
Fear and Root.

**Dead-end checks** live in each class's draft, as the mage's did for Brute Force.

## What is not here yet

- Pets and totems are not drawn as creatures (their attacks are), and cannot be attacked.
- Charges are not shown on the buff frame yet.
- Defensive buttons have no augments yet.
- Elite and boss mechanics (11e) do not exist yet, so "softly countered by" is untested.
