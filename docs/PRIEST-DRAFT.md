# The priest: spells and augments (draft for review)

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the priest is:** cloth and 120 health, Light and Shadow both. The Light smites and heals; the
Shadow wears whole crowds down and drinks their life. Survival answers: **sustain** (Renew's heal
charges, Lightwell, Holy Nova, Devouring Plague), **mitigation** (Power Word: Shield) and **control**
(Psychic Scream on F, Hex of Weakness). In co-op the priest's heal charges spend themselves on
friends: the healer's trade (11c).

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Smite** (start) | Holy · Direct | Elites first, 20 yd | Instant holy damage. | Single target |
| 2 | **Holy Fire** | Holy · Direct · Damage over time | Elites first, 20 yd | A hit, then Burning for 7 s. | Boss damage |
| 3 | **Mind Blast** | Shadow · Direct | Elites first, 22 yd | A heavy blast, 3.5 s cooldown. | Burst |
| 4 | **Shadow Word: Pain** | Shadow · Damage over time | The 3 closest within 18 yd | No hit: shadow pain for 12 s. | Crowd damage over time |
| 5 | **Mind Flay** (new) | Shadow · Direct · Slow | The closest, 18 yd | 3 quick hits that Chill. | Get off me |
| 6 | **Devouring Plague** (new) | Shadow · Damage over time · Leech | The 2 closest within 20 yd | No hit: a plague for 8 s that heals you for half its damage. | Sustain |
| 7 | **Holy Nova** | Holy · Area · Direct · Heal | Around you, 9 yd | Damages enemies; heals you and allies within reach for 3%. | Close clear, small heal |
| 8 | **Starshards** (new) | Arcane · Ground · Area | Thickest crowd within 20 yd | Stars fall for 4 s. | Zone damage |
| 9 | **Renew** (new) | Holy · Heal · Ward | You and allies | Every 8 s banks a heal charge (up to 5): 20% of health when below half. Full: smites the nearest enemy. | Sustain, overflow damage |
| 10 | **Power Word: Shield** (new) | Holy · Shield | You | Every 15 s, a shield worth 30% of your health. | Mitigation |
| 11 | **Lightwell** (new) | Holy · Ground · Heal | At your feet | For 15 s, heals heroes inside 3% a second and smites enemies inside. | Sustain zone |
| 12 | **Hex of Weakness** (new) | Shadow · Direct | Elites first (2), 20 yd | Weakens: they deal 25% less for 8 s. | Elite mitigation |

Psychic Scream left the pool to become the defensive button.

**Defensive button: Psychic Scream** (F, 30 s): every enemy within 10 yd flees in Fear for 4 s.

## Capstones (rank 10: 1 of 3)

### 1. Smite
- ★ **Atonement:** each Smite heals you, or the most wounded ally within 30 yd, for 50% of its
  damage. Your damage spell is your heal.
- **Divine Fury:** every 4th Smite strikes 3 targets (bosses and elites first).
- **Searing Light:** each Smite on the same target deals +10% more than the last, up to +50%;
  changing target starts again.

### 2. Holy Fire
- ★ **Martyrdom:** costs 3% of your health each cast but deals +80%. Pair it with heals.
- **Purge the Wicked:** when its target dies, its burn jumps to the 2 closest enemies.
- **Holy Fervor:** each burn tick has a 5% chance to make Holy Fire ready.

### 3. Mind Blast
- ★ **Psychic Horror:** its target and every enemy within 5 yd of it flee in Fear for 2 s. Mind
  Blast becomes control around the elite.
- **Shadowy Apparitions:** its crits send a shadow at the closest other enemy for 50%.
- **Shadowy Insight:** each Shadow Word: Pain tick has a 10% chance to make Mind Blast ready.

### 4. Shadow Word: Pain
- ★ **Spreading Pain:** it goes on enemies that do not have it yet, anywhere within 30 yd. With its
  12-s length, the whole screen is in pain.
- **Misery:** enemies in pain are Marked: +10% damage from every source.
- **Death's Echo:** when an enemy in pain dies, a shadow flies to the closest enemy for 50% of the
  pain's total.

### 5. Mind Flay
- **Mind Sear:** each hit also strikes every enemy within 4 yd of the target for 50%.
- ★ **Psychic Link:** each hit also strikes every enemy with your Shadow Word: Pain for 30%. Pain
  everything, then flay one.
- **Mind Trauma:** Flayed enemies are Weakened.

### 6. Devouring Plague
- **Plaguebearer:** when a plagued enemy dies, the plague jumps to the 2 closest enemies.
- ★ **Vampiric Embrace:** while any enemy carries your plague, all your Shadow damage heals you and
  every ally within 20 yd for 15% of it. Shadow becomes the party's healer.
- **Insatiable:** heals twice as much while you are below 50% health.

### 7. Holy Nova
- **Searing Nova:** everything it hits Burns for 40% of the hit over 4 s.
- ★ **Divine Aegis:** its heal on a hero at full health becomes a shield instead, up to 20% of their
  health. Spam it to stay shielded.
- **Inspiration:** each enemy it hits makes you take 1% less damage for 4 s, up to 20%.

### 8. Starshards
- **Shooting Stars:** its crits drop a star on another crowd for 100%.
- ★ **Starfall:** instead of one spot, stars fall on 3 random enemies within 20 yd of you every
  second for 6 s. It goes where you go.
- **Lunar Guidance:** enemies it hits are Marked +10% for 4 s.

### 9. Renew
- ★ **Holy Word: Sanctify:** spending your last charge casts Holy Nova (yours, or rank 1) and heals
  you 25%. You aim to burn the bank, not to keep it.
- **Blessed Recovery:** a charge also fires when a single blow takes 15% or more of your health,
  whatever your health.
- **Inner Focus:** while your bank is full, your other Holy spells deal +15%.

### 10. Power Word: Shield
- **Body and Soul:** while the shield holds, +25% move speed.
- ★ **Rapture:** when the shield breaks, every enemy within 8 yd flees in Fear for 3 s. Let them
  break it.
- **Borrowed Time:** after it breaks, your cooldowns run 50% faster for 4 s.

### 11. Lightwell
- **Sanctuary:** heroes inside take 20% less damage.
- ★ **Light's Wrath:** when the well ends, it bursts for Holy damage equal to all it healed, within
  10 yd. Get hurt inside it, then let it blow.
- **Spring of Light:** enemies inside are Chilled.

### 12. Hex of Weakness
- **Touch of Weakness:** a hexed enemy that strikes you takes Hex's damage back, tripled.
- ★ **Hex of Domination:** its targets also take +30% from everyone and deal 50% less instead of
  25% (the whole party). The boss's curse.
- **Spreading Hex:** when a hexed enemy dies, the hex jumps to the closest enemy.

## Shared augments the priest brings

Twilight (Holy or Shadow), Sacred Flame (Holy or Fire), Mending Link and Renewing (Heal), Deep
Reserves and Overflowing (Ward), Sanguine and Bloodthirsty (Leech), Dread and Panic (through Psychic
Horror and Rapture's Fear).

**Dead-end checks:** Brute Force on Mind Flay removes its Chill: Mind Sear and Psychic Link still
work.

## Checks

- **The solo healer:** Renew with Deep Reserves and Overflowing, Smite with **Atonement**, Holy Nova
  with **Divine Aegis**: a full bank turns every Renew into damage, and every Smite heals.
- **The shadow priest:** Shadow Word: Pain with **Spreading Pain**, Mind Flay with **Psychic Link**,
  Devouring Plague with **Vampiric Embrace**: the whole screen in pain, one flay hurting all of it,
  the party healed by it.
