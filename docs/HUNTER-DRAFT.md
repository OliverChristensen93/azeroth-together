# The hunter: spells and augments (draft for review)

> **Built (2026-10-16)** as a playable prototype: every spell, all 36 capstones and Disengage on F. Differences: Rapid Fire speeds up all your spells, not only Projectiles; the wolf bites every second; Trick Shot ricochets within 8 yd. The pet and traps are not drawn as creatures or objects (their hits are). Mechanics in `world/hunter.rs`. Not yet: balance, charges shown on the buff frame, augments for the defensive button.

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the hunter is:** mail and 135 health, the longest reach in the game, a pet at its heel and
traps behind it. Survival answers: **distance** (range, Disengage on F, Concussive Shot), **control**
(traps, Scatter Shot) and a **pet** that keeps the boss busy.

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Arcane Shot** (start) | Arcane · Projectile · Direct | Elites first, 28 yd | A fast arrow, short cooldown. | Single target |
| 2 | **Multi-Shot** | Physical · Projectile · Direct | The 3 closest within 26 yd | Three arrows. | Spread damage |
| 3 | **Aimed Shot** | Physical · Projectile · Direct | Elites first, 32 yd | A heavy shot, 4 s cooldown. | Boss killer |
| 4 | **Serpent Sting** | Nature · Projectile · Poison · Damage over time | The 2 closest within 26 yd | No hit: a poison for 15 s. | Damage over time |
| 5 | **Concussive Shot** | Physical · Projectile · Direct · Slow | The closest, 28 yd | Chills 50%. | Kiting |
| 6 | **Volley** | Physical · Ground · Area | Thickest crowd within 26 yd | Arrows rain for 6 s. | Zone damage |
| 7 | **Explosive Trap** | Fire · Ground · Area · Direct | A trap at your feet | Goes off when an enemy steps in; leaves burning ground. | Burst behind you |
| 8 | **Frost Trap** (new) | Frost · Ground · Area · Slow | A trap at your feet | When sprung, ice Chills everything within 6 yd for 8 s. | Control behind you |
| 9 | **Call Pet** (new) | Physical · Summon · Melee | Follows you | A wolf for 60 s that bites the closest enemy (elites first) within 25 yd every 1.2 s. | Steady damage |
| 10 | **Scatter Shot** (new) | Physical · Projectile · Direct · Stun · Knockback | The closest, 20 yd | Knocks back and Stuns for 2 s. | Get off me |
| 11 | **Hunter's Mark** (new) | Arcane · Direct | Elites first, 35 yd | Marks: +15% damage from every source for 20 s. | Boss setup (party) |
| 12 | **Rapid Fire** (new) | Physical | You | Every 20 s, your Projectile spells come back 40% faster for 8 s. | Tempo |

**Defensive button: Disengage** (F, 30 s): 12 yd the way you face; where you left, a Frost Trap
springs at once (Chills everything within 6 yd for 6 s).

## Capstones (rank 10: 1 of 3)

### 1. Arcane Shot
- **Arcane Barrage:** every 4th shot is three arrows (bosses and elites first).
- ★ **Trick Shot:** it ricochets to a second enemy within 10 yd, then a third. Your single-target
  arrow clears.
- **Lock and Load:** its crits make Aimed Shot ready.

### 2. Multi-Shot
- **Splinter Arrows:** each arrow bursts within 3 yd for 40%.
- ★ **Spread Shot:** a fan of arrows in a 20-yd cone ahead of you instead, every enemy in it for
  70%. You aim it by facing.
- **Serpent Spread:** applies your Serpent Sting (yours, or rank 1) to what it hits.

### 3. Aimed Shot
- **Careful Aim:** +60% crit chance against enemies above 80% health.
- ★ **Sniper Training:** after 2 s standing still, +50% range and +100% damage. Plant your feet for
  the boss.
- **Piercing Shots:** its target Bleeds for 50% of the hit over 6 s.

### 4. Serpent Sting
- ★ **Spreading Venom:** it goes on enemies without it first, anywhere within 30 yd.
- **Noxious Stings:** stung enemies take +10% from your Physical spells.
- **Scorpid Sting:** stung enemies are Weakened.

### 5. Concussive Shot
- **Improved Concussion:** 20% chance to Stun for 2 s.
- ★ **Daze Wave:** its Chill spreads to every enemy within 6 yd of its target. One shot slows a pack.
- **Frost Shot:** its Chill lasts twice as long.

### 6. Volley
- ★ **Arrowstorm:** while you stand still, it keeps falling on the thickest crowd with no end.
  Moving ends it. A turret stance.
- **Explosive Volley:** what it hits Burns for 30% over 4 s.
- **Hail of Arrows:** it Chills.

### 7. Explosive Trap
- ★ **Minefield:** lays three traps around you instead of one, at -40% each. You kite through a
  field.
- **Scorched Earth:** its burning ground lasts twice as long and Chills.
- **Big Game Trap:** +100% damage to bosses and elites.

### 8. Frost Trap
- **Freezing Trap:** the first enemy to spring it is Frozen for 4 s.
- ★ **Frozen Wake:** while you move, a small Frost Trap drops behind you every 3 s. Run circles.
- **Deep Freeze:** its Chill slows 70%.

### 9. Call Pet
- **Bestial Wrath:** every 20 s, the pet bites for triple damage for 6 s.
- ★ **Kill Command:** when your Arcane Shot or Aimed Shot crits, the pet bites its target at once
  for +100%. Crit builds feed the pet.
- **Intimidation:** every 5th bite Stuns its target for 2 s.

### 10. Scatter Shot
- **Improved Scatter:** also Chills for 4 s after the stun.
- ★ **Wyvern Sting:** its target sleeps for 6 s (a long Stun), and wakes up Poisoned (100% over 6 s).
- **Scatter Burst:** knocks back every enemy within 4 yd of its target too.

### 11. Hunter's Mark
- **Improved Mark:** +25% instead of +15%.
- ★ **Marked for the Kill:** when a Marked enemy dies, the mark jumps to the next boss or elite in
  reach, or the closest enemy. It never runs out.
- **Hunter's Prey:** your Projectile spells deal +20% more to Marked enemies.

### 12. Rapid Fire
- **Rapid Killing:** each kill while it lasts makes it last 0.5 s longer.
- ★ **Trueshot Aura:** no cooldown, always on: you and every ally within 20 yd deal +10% damage. A
  party aura instead of a burst.
- **Quick Shots:** also +20% move speed while it lasts.

## Shared augments the hunter brings

Crippling and Virulent (Poison), Pack, Feral Bond and Frenzy (Summon), Thunderstrike (Physical or
Nature), Balance (Arcane or Nature), Crushing (Stun).

**Dead-end checks:** Brute Force on Concussive Shot removes its Chill: Improved Concussion (a stun of
its own) still works. On Scatter Shot it removes the Stun: Scatter Burst still knocks back.

## Checks

- **The trapper:** Explosive Trap with **Minefield**, Frost Trap with **Frozen Wake**, Disengage:
  every escape leaves a field behind.
- **The beast master:** Call Pet with Pack and **Kill Command**, Arcane Shot with Precise: two pets,
  each crit a double bite.
