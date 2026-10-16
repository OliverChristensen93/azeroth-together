# The paladin: spells and augments (draft for review)

> **Built (2026-10-16)** as a playable prototype: every spell, all 36 capstones and Divine Shield on F. Differences: Sanctity is +10% to all damage, not only Holy. Mechanics in `world/paladin.rs`. Not yet: balance, charges shown on the buff frame, augments for the defensive button.

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the paladin is:** plate and 160 health, a holy warrior who stands in the horde and lets the
Light do the rest. It is the clearest **tank and healer** in the game, so it lives by 11b and 11c:
block charges with retaliation, heal charges with overflow. Survival answers: **mitigation** (Holy
Shield, Divine Shield on F, auras), **sustain** (Holy Light, Lay on Hands, Holy Shock) and
**control** (Hammer of Justice).

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Hammer of Wrath** (start) | Holy · Projectile · Direct | Elites first, 24 yd | A thrown hammer of light. | Single target |
| 2 | **Judgement** | Holy · Direct | Elites first (2), 14 yd | Instant holy judgement. | Single target |
| 3 | **Exorcism** | Holy · Direct | Elites first, 22 yd | A heavy holy blow, 5 s cooldown. | Boss damage |
| 4 | **Holy Shock** | Holy · Direct · Heal | The closest, 20 yd | Damages, and heals you for 5% of your health. | Get off me, sustain |
| 5 | **Consecration** | Holy · Ground · Area · Damage over time | At your feet | Holy ground for 8 s. | Zone damage |
| 6 | **Holy Wrath** | Holy · Area · Direct | Around you, 11 yd | Wrath on everything near. | Close clear |
| 7 | **Hammer of Justice** | Holy · Direct · Stun | Elites first (2), 12 yd | Stuns for 3 s. | Elite control |
| 8 | **Holy Shield** (new) | Holy · Ward | You | Every 6 s banks a block charge (up to 4): a blocked blow strikes back with holy light. Full: strikes the closest enemy. | Mitigation, retaliation |
| 9 | **Holy Light** (new) | Holy · Heal · Ward | You and allies | Every 10 s banks a heal charge (up to 3): 35% when below half. Full: smites the closest enemy. | Sustain, overflow |
| 10 | **Retribution Aura** (new) | Holy · Aura | You and allies within 20 yd | Always on: an enemy that strikes you takes holy damage back. | Retaliation |
| 11 | **Seal of Command** (new) | Physical · Holy · Melee · Direct | The 2 closest, 6 yd | Quick holy melee strikes. | Melee |
| 12 | **Lay on Hands** (new) | Holy · Heal · Ward | You | Once a minute, below 20% health, heals you fully. Full: a great smite at the closest enemy. | Emergency |

**Defensive button: Divine Shield** (F, 90 s): you take no damage for 6 s.

## Capstones (rank 10: 1 of 3)

### 1. Hammer of Wrath
- **Hammer of the Righteous:** its hit bursts within 3 yd for 50%.
- ★ **Wrath of the Executioner:** it seeks the most wounded enemy instead, +200% against enemies
  below 20% health, and a kill makes it ready. From boss-seeker to finisher.
- **Blessed Hammer:** its crits heal you 3%.

### 2. Judgement
- **Judgement of Light:** a judged enemy that strikes you heals you 2% of your health.
- ★ **Divine Storm:** judges every enemy within 8 yd of you instead, at 60%.
- **Judgement of the Crusader:** judged enemies are Marked +10%.

### 3. Exorcism
- **Exorcist:** +100% damage to bosses and elites.
- ★ **Art of War:** your Holy crits have a 20% chance to make Exorcism ready. Crit builds feed it.
- **Banish Evil:** its target is Stunned for 2 s.

### 4. Holy Shock
- **Infusion of Light:** its crits bank a Holy Light charge (even without it).
- ★ **Glimmer of Light:** every hit leaves a Glimmer on its target; each heal you get strikes every
  Glimmered enemy for 50% of the heal. Your heals become damage.
- **Daybreak:** below 50% health it heals 15% and comes back twice as fast.

### 5. Consecration
- **Sacred Ground:** heroes standing in it heal 1% of their health a second.
- ★ **Hallowed Ground:** while you stand in it, you take 15% less damage and enemies in it are
  Weakened. Fight from your own ground.
- **Consecrated Chill:** enemies in it are Chilled.

### 6. Holy Wrath
- **Wrathful:** +100% damage to bosses and elites.
- ★ **Divine Purpose:** each enemy it hits takes 0.2 s off your defensive button. Dive to shield
  more often.
- **Stunning Wrath:** Stuns for 1 s.

### 7. Hammer of Justice
- **Fist of Justice:** your Holy crits take 1 s off its cooldown.
- ★ **Shockwave Hammer:** also stuns every enemy within 5 yd of its targets.
- **Lasting Justice:** its Stun lasts 1.5 s longer.

### 8. Holy Shield
- ★ **Avenger's Shield:** its overflow becomes a shield that strikes 3 enemies and Stuns them for
  1 s.
- **Holy Retribution:** its retaliation strikes every enemy within 5 yd.
- **Reckoning:** each block has a 20% chance to cast Judgement (yours, or rank 1).

### 9. Holy Light
- ★ **Holy Power:** each heal it gives makes your next Holy damage spell deal +50%. Heals fuel your
  damage.
- **Divine Favor:** 30% of its heals are doubled.
- **Light of Dawn:** its overflow strikes every enemy in a 10-yd cone ahead of you.

### 10. Retribution Aura
- ★ **Aura Mastery:** each blow it answers also heals the hero struck for 20% of the retaliation. Get
  hit to heal.
- **Devotion:** you and allies in reach also take 10% less damage.
- **Sanctity:** you and allies in reach deal +10% Holy damage.

### 11. Seal of Command
- **Seal of Light:** each hit heals you 1% of your health.
- ★ **Seal of Righteousness:** each hit also strikes every enemy within 4 yd of its target for 40%.
- **Seal of Wisdom:** each hit takes 0.2 s off Holy Light's cooldown.

### 12. Lay on Hands
- **Improved Lay on Hands:** for 10 s after, you take 30% less damage.
- ★ **Martyr's Hands:** it fires below 40% instead of 20%, and every enemy within 10 yd takes holy
  damage equal to the heal.
- **Hand of Protection:** after it fires, you take no damage for 2 s.

## Shared augments the paladin brings

Crusader (Physical or Holy), Sacred Flame (Holy or Fire), Twilight (Holy or Shadow), Deep Reserves
and Overflowing (Ward), Mending Link and Renewing (Heal), Sweeping (Melee), Crushing (Stun).

**Dead-end checks:** Brute Force on Hammer of Justice removes its Stun: Fist of Justice (faster
hammers on crits) still works; Shockwave Hammer and Lasting Justice lose their point. Brute Force on
Holy Wrath with Stunning Wrath removes the stun but keeps the wrath.

## Checks

- **The tank (11c):** Holy Shield with Deep Reserves and **Avenger's Shield**, Retribution Aura with
  **Aura Mastery**, Consecration with **Hallowed Ground**: stand in the horde, every blow answered,
  every answer a heal.
- **The healer:** Holy Light with **Holy Power**, Holy Shock with **Glimmer of Light**, Lay on Hands
  with **Martyr's Hands**: heals that hit.
