# The shaman: spells and augments (draft for review)

> **Built (2026-10-16)** as a playable prototype: every spell, all 36 capstones and Ghost Wolf on F. Differences: Magma Totem pulses every second for less (not every 2 s); Lightning Overload strikes the same target again; Storm Nexus loses 5% a jump. Totems are not drawn (their hits are). Mechanics in `world/shaman.rs`. Not yet: balance, charges shown on the buff frame, augments for the defensive button.

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders.**

**Who the shaman is:** mail and 135 health, lightning that jumps, shocks for every need, and totems
planted where the fight will be. Survival answers: **control** (Earthbind Totem, Frost Shock),
**sustain** (Healing Wave's charges, Healing Stream Totem), **retaliation** (Lightning Shield's
thorn charges) and **distance** (Ghost Wolf on F).

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Lightning Bolt** (start) | Nature · Projectile · Direct | Elites first, 24 yd | A bolt of lightning. | Single target |
| 2 | **Chain Lightning** | Nature · Chain · Direct | The closest, 20 yd | Jumps 3 times to the next enemy within 8 yd, 80% each jump. | Clear |
| 3 | **Earth Shock** | Nature · Direct | The closest, 15 yd | Instant, short cooldown. | Get off me |
| 4 | **Flame Shock** | Fire · Direct · Damage over time | Elites first (2), 20 yd | A hit, then Burning for 12 s. | Damage over time |
| 5 | **Frost Shock** | Frost · Direct · Slow | The 2 closest, 20 yd | Chills 60%. | Kiting |
| 6 | **Searing Totem** | Fire · Totem · Direct | At your feet | For 20 s, bolts the closest enemy within 20 yd every second. | Steady damage |
| 7 | **Magma Totem** | Fire · Totem · Area | At your feet | For 20 s, pulses fire within 8 yd every 2 s. | Zone damage |
| 8 | **Earthbind Totem** (new) | Nature · Totem · Area · Slow | At your feet | For 15 s, Chills everything within 10 yd every second. | Control |
| 9 | **Healing Stream Totem** (new) | Nature · Totem · Heal | At your feet | For 20 s, heals heroes within 15 yd 2% every 2 s; with nobody hurt, it strikes the closest enemy instead (overflow). | Sustain |
| 10 | **Lightning Shield** | Nature · Ward | You | Every 4 s banks an orb (up to 3): the next enemy that strikes you is zapped. Full: zaps the closest enemy. | Retaliation |
| 11 | **Healing Wave** (new) | Nature · Heal · Ward | You and allies | Every 10 s banks a heal charge (up to 3): 30% when below half. Full: lightning at the closest enemy. | Sustain |
| 12 | **Stormstrike** (new) | Physical · Nature · Melee · Direct | The closest, 5 yd | Marks its target: +20% damage from every source for 6 s. | Melee, setup |

**Defensive button: Ghost Wolf** (F, 30 s): +60% move speed for 6 s.

## Capstones (rank 10: 1 of 3)

### 1. Lightning Bolt
- **Lightning Overload:** 20% chance to cast a second bolt for 50%.
- ★ **Stormkeeper:** every 5th bolt chains to 4 more enemies. Count your bolts toward packs.
- **Rolling Thunder:** its crits bank a Lightning Shield orb (up to its cap, even without it).

### 2. Chain Lightning
- **Chain Reaction:** a kill makes it ready (at most every 4 s).
- ★ **Storm Nexus:** it jumps without limit between enemies within 3 yd of each other. Gather the
  pack and it never stops.
- **Electrified:** everything it hits is Chilled for 1 s.

### 3. Earth Shock
- ★ **Fulmination:** spends your Lightning Shield orbs for +60% damage each. You bank orbs to cash
  them in.
- **Earthen Grip:** also Roots every enemy within 6 yd of its target for 2 s.
- **Shock Mastery:** each Earth Shock makes Flame Shock and Frost Shock ready.

### 4. Flame Shock
- **Lava Burst:** your Lightning Bolts always crit enemies with your Flame Shock.
- ★ **Volcanic Fury:** an enemy that dies with it erupts: burning ground (5 yd, 4 s) where it fell.
  Burn the pack's front line to set the floor on fire.
- **Searing Flames:** Searing Totem's bolts renew it on their target.

### 5. Frost Shock
- ★ **Frozen Power:** Roots for 2 s instead of Chilling; rooted targets take +30% from your Nature
  spells.
- **Frost Nova Shock:** also Chills everything within 5 yd of its target.
- **Icefury:** its crits Freeze for 2 s.

### 6. Searing Totem
- **Searing Burn:** its bolts leave Burning (40% over 4 s).
- ★ **Searing Pact:** while you stand within 8 yd of it, your Fire spells deal +20%. You fight
  beside your totem.
- **Twin Fires:** its bolts hit 2 enemies.

### 7. Magma Totem
- **Fire Nova:** when it ends, it explodes for 300% within 8 yd.
- ★ **Liquid Magma:** each pulse also spits a glob at 3 random enemies within 20 yd. A totem that
  reaches.
- **Molten Earth:** its pulses leave Burning (30% over 4 s).

### 8. Earthbind Totem
- **Earthgrab:** an enemy entering its reach is Rooted for 2 s (once each).
- ★ **Earthquake Totem:** its pulses deal +300% and Stun for 0.5 s instead of Chilling. Control
  becomes damage.
- **Stoneclaw:** enemies in its reach are Weakened.

### 9. Healing Stream Totem
- **Healing Tide:** heals twice as much on heroes below 30% health.
- ★ **Cloudburst:** it stores everything it heals and releases it as Nature damage within 10 yd
  when it ends.
- **Stoneskin:** heroes in its reach take 15% less damage.

### 10. Lightning Shield
- ★ **Thunderstorm:** spending your last orb blasts everything within 8 yd for 200% and knocks it
  back. Let them break it.
- **Static Shield:** its zaps also Chill.
- **Static Charge:** each orb spent gives +5% Nature damage for 6 s, up to 15%.

### 11. Healing Wave
- **Ancestral Fortitude:** after one of its heals, you take 20% less damage for 6 s.
- ★ **Nature's Swiftness:** below 25% health, every charge spends at once and your cooldowns run
  twice as fast for 4 s. A comeback button.
- **Tidal Focus:** charges bank 30% faster.

### 12. Stormstrike
- ★ **Windfury:** 20% chance per hit to strike twice more at once; each Windfury hit banks a
  Lightning Shield orb.
- **Stormlash:** its mark is +30%.
- **Maelstrom Weapon:** every 5th Stormstrike makes your next Lightning Bolt or Chain Lightning deal
  double.

## Shared augments the shaman brings

Forked and Conductive (Chain), Pack, Feral Bond and Frenzy (Totem: Pack plants two at once),
Frostbrand (Frost or Nature), Thunderstrike (Physical or Nature), Deep Reserves and Overflowing
(Ward), Mending Link and Renewing (Heal), Bramble and Grasping (Root). Totems are Ground areas, so
Lingering, Mobile (a totem that walks with you) and Targeted (planted on the thickest crowd) fit
them.

**Dead-end checks:** Brute Force on Frost Shock removes its Chill: Icefury and Frost Nova Shock lose
their point, Frozen Power still roots (its own effect). Brute Force on Earthbind Totem removes the
Chill: Earthgrab and Stoneclaw still work, Earthquake Totem becomes pure damage.
