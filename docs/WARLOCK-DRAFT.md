# The warlock: spells and augments (draft for review)

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the warlock is:** cloth and 120 health, curses on everything and demons at its side. Its
damage is mostly over time, so it must survive while the damage lands. Survival answers:
**sustain** (Drain Life, Death Coil, Siphon Life), **control** (Howl of Terror, Death Coil's Fear),
a **pet** that soaks (the Voidwalker) and **escape** (Soulstone on F).

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Shadow Bolt** (start) | Shadow · Projectile · Direct | Elites first, 24 yd | A bolt of shadow. | Single target |
| 2 | **Corruption** | Shadow · Damage over time | The 3 closest within 20 yd | No hit: corruption for 12 s. | Crowd damage over time |
| 3 | **Curse of Agony** | Shadow · Damage over time | Elites first (2), 25 yd | No hit: a long curse, 16 s. | Boss damage over time |
| 4 | **Immolate** | Fire · Direct · Damage over time | Elites first (2), 20 yd | A hit, then Burning for 9 s. | Fire damage |
| 5 | **Rain of Fire** | Fire · Ground · Area · Damage over time | Thickest crowd within 20 yd | Fire falls for 6 s. | Zone damage |
| 6 | **Hellfire** | Fire · Ground · Area · Damage over time | Follows you | For 5 s, burns everything within 7 yd of you. | Close clear |
| 7 | **Death Coil** | Shadow · Projectile · Direct · Fear · Leech | The closest, 22 yd | Fears its target for 3 s and heals you for all its damage. | Get off me, sustain |
| 8 | **Drain Life** (new) | Shadow · Damage over time · Leech | Elites first, 20 yd | No hit: drains for 5 s, healing you for 60% of it. | Sustain |
| 9 | **Howl of Terror** (new) | Shadow · Area · Fear | Around you, 10 yd | Everything near flees for 4 s. 15 s cooldown. | Breathing room |
| 10 | **Summon Imp** (new) | Fire · Summon · Direct | Follows you | An imp for 30 s that firebolts the closest enemy (elites first) every 1.5 s. | Steady damage |
| 11 | **Summon Voidwalker** (new) | Shadow · Summon · Area | Follows you | A voidwalker for 30 s that pulses shadow around itself; while it is out you take 20% less damage. | Mitigation |
| 12 | **Searing Pain** | Fire · Direct | The closest, 15 yd | A quick searing burst. | Get off me |

Shadowburn left the pool (Death Coil and Searing Pain cover close threats; Shadow Bolt the boss).

**Defensive button: Soulstone** (F, 120 s): for 60 s, a killing blow leaves you at 40% health
instead, and every enemy within 10 yd flees in Fear for 3 s.

## Capstones (rank 10: 1 of 3)

### 1. Shadow Bolt
- **Nightfall:** your damage over time ticks have a 5% chance to make your next Shadow Bolt deal
  triple.
- ★ **Shadow Bolt Volley:** it hits the 5 closest enemies (not elites first) for 60%. The boss spell
  becomes a clearing spell.
- **Improved Shadow Bolt:** each hit makes its target take +4% Shadow damage, up to 20% (whole party).

### 2. Corruption
- **Siphon Life:** its ticks heal you for 20% of their damage.
- ★ **Seed of Corruption:** a Corrupted enemy that dies bursts for the corruption's full damage
  within 6 yd and corrupts what it hits. Corrupt one pack, watch it chain.
- **Everlasting Affliction:** your Shadow Bolts renew Corruption on their target.

### 3. Curse of Agony
- ★ **Curse of Doom:** when the curse ends on a target still alive, it deals twice its total at
  once. A timed bomb on the boss.
- **Amplify Curse:** +50% damage, and cursed enemies are Weakened.
- **Curse of Exhaustion:** cursed enemies are Chilled.

### 4. Immolate
- ★ **Fire and Brimstone:** also hits every enemy within 5 yd of its target, all of them Burning.
- **Backdraft:** its crits make your Fire spells come back 30% faster for 4 s.
- **Emberstorm:** its burn deals +50% to bosses and elites.

### 5. Rain of Fire
- ★ **Firestorm Rain:** it follows the thickest crowd, moving to it every second.
- **Aftermath:** its ticks Chill.
- **Pyroclasm:** each tick has a 15% chance to Stun what it hits for 1 s.

### 6. Hellfire
- ★ **Infernal Pact:** it burns you for 1% of your health a tick, but deals +100%, and every enemy
  that dies inside it heals you 3%. A trade you play around.
- **Ritual of Fire:** everything it touches Burns for 40% of the hit over 4 s.
- **Hellfire Ward:** while it burns, you take 20% less damage.

### 7. Death Coil
- **Horrify:** its Fear spreads to every enemy within 5 yd of its target.
- ★ **Death's Embrace:** below 35% health, it comes back three times as fast. Your panic button.
- **Dark Bargain:** its heal also heals allies within 10 yd.

### 8. Drain Life
- **Soul Harvest:** when a drained enemy dies, +5% damage for 20 s (up to 25%).
- ★ **Drain Aura:** drains every enemy within 6 yd of you at once. Stand in the pack and drink.
- **Improved Drain:** heals twice as much from bosses and elites.

### 9. Howl of Terror
- **Nightmare:** feared enemies take shadow damage over time (60% over 4 s).
- ★ **Shadowfury:** instead of Fear, it Stuns everything within 10 yd for 2 s and deals +200%.
- **Demonic Presence:** feared enemies are Marked +15%.

### 10. Summon Imp
- **Burning Imp:** its firebolts leave Burning (50% over 4 s).
- ★ **Demonic Sacrifice:** the imp never comes: while you have no imp, your Fire spells deal +25%.
  Trade the pet for power.
- **Improved Imp:** its firebolts hit 2 enemies.

### 11. Summon Voidwalker
- ★ **Sacrifice:** when you drop below 30% health, the voidwalker gives itself up: a shield worth
  40% of your health (it comes back on its cooldown). Your emergency shield.
- **Torment:** enemies it hits are Weakened.
- **Consume Shadows:** while it is out, you heal 1% of your health a second.

### 12. Searing Pain
- **Improved Searing Pain:** +50% crit chance against enemies within 5 yd.
- ★ **Soul Fire:** every 5th Searing Pain is a Soul Fire: +300% damage, and it Stuns for 1 s.
- **Fel Flame:** each hit makes your burns and corruption on its target last 2 s longer.

## Shared augments the warlock brings

Shadowflame (Fire or Shadow), Twilight (Holy or Shadow), Pack, Feral Bond and Frenzy (Summon),
Sanguine and Bloodthirsty (Leech), Dread and Panic (Fear).

**Dead-end checks:** Brute Force on Death Coil or Howl of Terror removes the Fear: Death's Embrace,
Dark Bargain, Shadowfury and Nightmare still work.

## Checks

- **The affliction build:** Corruption with Quickened and **Seed of Corruption**, Curse of Agony with
  **Curse of Doom**, Shadow Bolt with **Nightfall**: every pack bursts in a chain.
- **The drain tank:** Drain Life with **Drain Aura**, Voidwalker with **Sacrifice**, Hellfire with
  **Infernal Pact**: stand in the pack, burning and drinking.
