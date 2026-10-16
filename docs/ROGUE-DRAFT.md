# The rogue: spells and augments (draft for review)

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the rogue is:** leather and 135 health, very fast attacks at arm's length. **Builders** give
combo points (up to 5); **Finishers** spend them all for +25% each. Poisons stack, bleeds tick, and
the rogue decides fights with stuns. Survival answers: **escape** (Vanish on F), **mitigation**
(Riposte's dodge charges), **control** (Gouge, Kidney Shot, Blind) and **kill speed**.

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Sinister Strike** (start) | Physical · Melee · Direct | The closest within 5 yd | A very quick stab. Builder. | Filler |
| 2 | **Backstab** (new) | Physical · Melee · Direct | Elites first, 5 yd | +100% damage to Stunned or Feared enemies. Builder. | Single target |
| 3 | **Eviscerate** | Physical · Melee · Direct · Finisher | Elites first, 5 yd | A big finishing cut. | Burst |
| 4 | **Rupture** | Physical · Melee · Bleed · Damage over time · Finisher | Elites first (2), 6 yd | No hit: a Bleed for 12 s; points make it longer and harder. | Boss bleed |
| 5 | **Blade Flurry** | Physical · Melee · Area · Direct | Around you, 6 yd | A flurry of blades. | Close clear |
| 6 | **Fan of Knives** (new) | Physical · Area · Direct | Around you, 10 yd | Knives at everything near. Builder. | Wider clear |
| 7 | **Deadly Poison** (new) | Nature · Poison · Damage over time | The 4 closest within 10 yd | No hit: a Poison for 12 s that stacks up to 5 times. | Stacking damage |
| 8 | **Gouge** | Physical · Melee · Direct · Stun | The 2 closest within 5 yd | Stuns for 1.5 s. Builder. | Get off me |
| 9 | **Kidney Shot** | Physical · Melee · Direct · Stun · Finisher | Elites first, 5 yd | Stuns; points make it longer. | Elite control |
| 10 | **Slice and Dice** (new) | Physical · Finisher | You | Your spells' cooldowns run 30% faster for 10 s; points make it longer. | Tempo |
| 11 | **Riposte** (new) | Physical · Melee · Ward | You | Every 6 s banks a dodge charge (up to 3): a dodged blow is riposted. Full: stabs the nearest enemy. | Mitigation, retaliation |
| 12 | **Blind** (new) | Physical · Fear | The 3 closest within 10 yd | They wander off in Fear for 4 s. | Breathing room |

**Defensive button: Vanish** (F, 45 s): the horde loses you for 3 s (it stops and mills about);
your hits in the next 4 s deal +50%.

## Capstones (rank 10: 1 of 3)

### 1. Sinister Strike
- **Combat Potency:** each hit has a 25% chance to give an extra combo point.
- ★ **Blade Dance:** bounces from its target to the next closest enemy within 6 yd, up to 4 times;
  every bounce gives a combo point. You stand in the pack to fill your points.
- **Relentless Strikes:** each hit takes 0.3 s off your Finishers' cooldowns.

### 2. Backstab
- ★ **Ambush:** for 4 s after your defensive button, it deals triple damage and always crits. You
  time Vanish for the boss.
- **Opportunity:** its bonus against Stunned or Feared enemies is +200%.
- **Seal Fate:** its crits give an extra combo point.

### 3. Eviscerate
- **Restless Blades:** each combo point it spends takes 2 s off your defensive button's cooldown.
- ★ **Cold Blood:** at 5 combo points it also strikes every enemy within 6 yd of its target. You
  save points for the big pack.
- **Ruthlessness:** each Eviscerate gives back 1 combo point.

### 4. Rupture
- **Venomous Wounds:** its bleed deals +50% to Poisoned enemies.
- ★ **Crimson Tempest:** its bleed also spreads to every enemy within 8 yd of its target. Rupture
  becomes area damage.
- **Bloodletter:** when a Ruptured enemy dies, you gain 2 combo points.

### 5. Blade Flurry
- **Adrenaline Rush:** hitting 8 or more enemies makes your cooldowns run 50% faster for 4 s (at most
  every 12 s).
- ★ **Flowing Steel:** while you keep moving it comes back twice as fast; standing still, half as
  fast. Keep running through the pack.
- **Serrated Blades:** everything it hits Bleeds for 50% of the hit over 4 s.

### 6. Fan of Knives
- **Poisoned Knives:** everything it hits gets your Deadly Poison (yours, or rank 1).
- ★ **Lethal Fan:** reaches 20 yd but only hits enemies that are Bleeding or Poisoned, for +100%.
  Set them up, then fan.
- **Fan the Pack:** gives a combo point for every 3 enemies hit (up to 5).

### 7. Deadly Poison
- **Envenom:** your Finishers consume their target's poison stacks, dealing what they had left at
  once, doubled.
- ★ **Toxic Cloud:** a Poisoned enemy that dies leaves a poison cloud (5 yd, 4 s) that poisons what
  walks in. Kite them through it.
- **Mind-numbing Poison:** enemies with 3 or more stacks are Weakened.

### 8. Gouge
- **Improved Gouge:** its Stun lasts 1 s longer.
- ★ **Dirty Tricks:** it seeks bosses and elites first, and its Stun holds elites as long as normal
  enemies. Gouge becomes your elite lock.
- **Gouged Eyes:** Gouged enemies take +30% from your Finishers.

### 9. Kidney Shot
- ★ **Marked for Death:** its target is Marked (+25% damage from everyone) for its stun plus 4 s;
  bosses resist the stun but not the mark. The party's boss opener.
- **Kidney Burst:** at 5 combo points it also stuns every enemy within 5 yd of its target.
- **Quick Recovery:** when you cast it, heal 2% of your health for every point spent.

### 10. Slice and Dice
- **Adrenaline:** also +20% move speed while it lasts.
- ★ **Restless Tempo:** each kill while it lasts makes it last 1 s longer, with no limit. Keep the
  killing going.
- **Improved Slice:** your cooldowns run 45% faster instead of 30%.

### 11. Riposte
- ★ **Counterattack:** every dodge spent casts Blade Flurry (yours, or rank 1). Dive with a full bank.
- **Nimble:** every dodge gives +10% move speed for 3 s, up to 30%.
- **Cheat Death:** once every 60 s, a killing blow leaves you at 10% health and refills your charges.

### 12. Blind
- **Blinding Powder:** enemies it Fears are Chilled for 4 s after.
- ★ **Smoke Bomb:** instead, a cloud of smoke at your feet for 5 s: enemies inside lose you (they
  wander) and take +30% from you. Fight from inside it.
- **Sap:** a Blinded enemy takes +100% from your next hit on it.

## Shared augments the rogue brings

Finisher and Poison are new tags; Crippling and Virulent (Poison), Lacerate and Hemorrhage (Bleed),
Deep Reserves and Overflowing (Ward), Dread and Panic (Fear), Crushing (Stun), Sweeping (Melee),
Shadowblade (Physical or Shadow) all reach rogue spells.

**Dead-end checks:** Brute Force on Gouge or Kidney Shot removes the stun: Gouged Eyes, Marked for
Death (the mark stays) and Quick Recovery still work. Brute Force on Blind removes the Fear: Smoke
Bomb (its cloud is its own effect) and Sap still work.

## Checks

- **The poison build:** Deadly Poison with Virulent and **Toxic Cloud**, Fan of Knives with
  **Poisoned Knives**, Eviscerate with **Envenom**: stacks everywhere, cashed in by finishers.
- **The Vanish build:** Backstab with **Ambush**, Eviscerate with **Restless Blades**, Smoke Bomb:
  every finisher brings Vanish back sooner, every Vanish is a burst.
- **The duelist:** Riposte with Deep Reserves and **Counterattack**, Blade Flurry with **Flowing
  Steel**: run through the pack with five dodges up.
