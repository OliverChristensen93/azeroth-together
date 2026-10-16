# The druid: spells and augments (draft for review)

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the druid is:** leather and 135 health, a little of everything: moon and sun from range, roots
and storms for control, claws when something gets close, and the best heals over time. Survival
answers: **control** (Entangling Roots, Hurricane), **sustain** (Rejuvenation's charges,
Tranquility), **retaliation** (Thorns) and **mitigation** (Barkskin on F).

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Wrath** (start) | Nature · Projectile · Direct | Elites first, 24 yd | A bolt of nature. | Single target |
| 2 | **Moonfire** | Arcane · Direct · Damage over time | The 2 closest, 22 yd | A hit, then Burning moonlight for 9 s. | Damage over time |
| 3 | **Starfire** | Arcane · Direct | Elites first, 24 yd | A heavy starlight blow, 4 s cooldown. | Boss damage |
| 4 | **Insect Swarm** | Nature · Damage over time | The 3 closest within 20 yd | No hit: a swarm for 12 s; swarmed enemies are Weakened. | Crowd damage over time |
| 5 | **Hurricane** | Nature · Ground · Area · Damage over time · Slow | Thickest crowd within 20 yd | A storm for 6 s that Chills. | Zone control |
| 6 | **Entangling Roots** | Nature · Direct · Root | The 4 closest within 16 yd | Roots for 3 s. | Control |
| 7 | **Rejuvenation** (new) | Nature · Heal · Ward | You and allies | Every 5 s banks a heal charge (up to 5): 12% when below half. Full: a thorn at the closest enemy. | Sustain |
| 8 | **Tranquility** (new) | Nature · Ground · Heal | At your feet | For 6 s, heals heroes within 10 yd 4% a second and stings enemies there. | Big heal |
| 9 | **Thorns** (new) | Nature · Aura | You and allies within 20 yd | Always on: an enemy that strikes you takes nature damage back. | Retaliation |
| 10 | **Swipe** (new) | Physical · Melee · Area · Cone · Direct | A cone ahead of you, 7 yd | A bear's swipe. | Close clear |
| 11 | **Rake** (new) | Physical · Melee · Bleed · Damage over time | The 3 closest within 5 yd | No hit: a Bleed for 9 s. | Melee damage over time |
| 12 | **Faerie Fire** (new) | Arcane · Direct | Elites first (3), 30 yd | Marks: +10% damage from every source for 15 s. | Boss setup (party) |

Insect Swarm, Moonfire and Starfire keep their keys; Wrath is still the start.

**Defensive button: Barkskin** (F, 45 s): you take 40% less damage for 10 s.

## Capstones (rank 10: 1 of 3)

### 1. Wrath
- **Nature's Grace:** its crits make your cooldowns run 30% faster for 3 s.
- ★ **Eclipse:** every 6th Wrath begins a Solar Eclipse: your Arcane spells deal +40% for 8 s. Mix
  your schools on purpose.
- **Wrath of Cenarius:** +50% damage to Rooted enemies.

### 2. Moonfire
- **Shooting Stars:** each of its ticks has a 10% chance to make Starfire ready.
- ★ **Moonkin Aura:** each hit gives you and allies within 20 yd +2% crit chance for 8 s, up to 10%.
- **Sunfire:** also hits every enemy within 5 yd of its targets.

### 3. Starfire
- **Moonstrike:** always crits enemies with your Moonfire.
- ★ **Starsurge:** +20% damage for each enemy with your Moonfire within its range, up to +100%.
  Spread moonfire, then strike.
- **Improved Starfire:** Stuns its target for 1 s.

### 4. Insect Swarm
- **Swarming:** when a swarmed enemy dies, the swarm jumps to the 2 closest enemies.
- ★ **Plague of Locusts:** becomes a cloud that follows you (6 yd) and swarms everything inside it.
- **Nature's Torment:** swarmed enemies take +15% from your Nature spells.

### 5. Hurricane
- **Cyclone:** the 3 enemies closest to its centre are lifted (Stunned) while it lasts.
- ★ **Eye of the Storm:** it centres on you and goes where you go; you take 20% less damage inside
  it.
- **Tornado:** each second it pulls enemies within 10 yd one yard toward its centre.

### 6. Entangling Roots
- **Nature's Grasp:** an enemy that strikes you is Rooted (each at most every 10 s).
- ★ **Mass Entanglement:** roots every enemy within 8 yd of you instead.
- **Thorned Vines:** rooted enemies take damage over time (100% of the hit over 3 s).

### 7. Rejuvenation
- ★ **Efflorescence:** each charge spent leaves flowers at your feet (5 yd, 4 s) that heal heroes 2%
  a second and Root enemies that enter. You fight on your flowers.
- **Gift of the Wild:** its overflow also heals every ally within 20 yd 5%.
- **Rampant Growth:** charges bank twice as fast while you are below 50% health.

### 8. Tranquility
- ★ **Tranquil Mind:** enemies inside flee in Fear. A sanctuary while it lasts.
- **Restoration:** heals 50% more on heroes below 50% health.
- **Hymn of Life:** heroes inside take 20% less damage.

### 9. Thorns
- **Brambles:** attackers are also Rooted for 1 s.
- ★ **Thorn Volley:** every 5 blows you take, thorns burst at every enemy within 8 yd for 200%.
- **Nature's Ward:** allies within 20 yd get your Thorns too.

### 10. Swipe
- **Maul:** every 3rd Swipe Stuns what it hits for 1 s.
- ★ **Bear Form:** while you have Swipe, you take 15% less damage and Swipe deals +30%, but your
  Arcane and Nature spells deal 20% less. A form, not a spell.
- **Thrash:** everything it hits Bleeds for 60% over 6 s.

### 11. Rake
- **Rip:** its bleed deals +50% to bosses and elites.
- ★ **Ferocious Bite:** a Raked enemy below 25% health dies to your next Rake. Bleed them low,
  then finish them.
- **Blood in the Water:** a kill on a bleeding enemy renews Rake on every enemy within 5 yd.

### 12. Faerie Fire
- **Improved Faerie Fire:** your hits on marked enemies have +10% crit chance.
- ★ **Fae Transfusion:** your hits on marked enemies heal you 0.5% of your health (at most 5% a
  second). The mark is your sustain.
- **Faerie Spread:** when a marked enemy dies, the mark jumps to the closest enemy.

## Shared augments the druid brings

Balance (Arcane or Nature), Frostbrand (Frost or Nature), Thunderstrike (Physical or Nature), Bramble
and Grasping (Root), Lacerate and Hemorrhage (Bleed), Deep Reserves and Overflowing (Ward), Mending
Link and Renewing (Heal), Sweeping (Melee).

**Dead-end checks:** Brute Force on Entangling Roots removes the Root: Mass Entanglement and Thorned
Vines still deal their damage; Nature's Grasp still roots (its own effect). Brute Force on Hurricane
removes its Chill: Cyclone, Eye of the Storm and Tornado still work.
