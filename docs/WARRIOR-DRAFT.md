# The warrior: spells and augments (draft for review)

Built on the mage's system ([MAGE-DRAFT.md](MAGE-DRAFT.md)) and the shared additions
([CLASSES-SHARED-DRAFT.md](CLASSES-SHARED-DRAFT.md)). **Numbers are placeholders** for the idea's
size, not balance.

**Who the warrior is:** plate and 160 health, fighting at arm's length. It wades into the horde,
cleaves what reaches it and puts the boss down with heavy single blows. Its survival answers:
**mitigation** (Shield Block's block charges, Challenging Shout), **sustain** (Bloodthirst, Victory
Rush, Rallying Cry), **control** (Thunder Clap, the shouts) and **escape** (Charge on F).

## The 12 spells

| # | Spell | Tags | Targets | Base behaviour | Role |
|---|---|---|---|---|---|
| 1 | **Cleave** (start) | Physical · Melee · Direct | The 3 closest within 5 yd | A quick sweep. | Melee clear |
| 2 | **Heroic Strike** | Physical · Melee · Direct | Elites first, 6 yd | A heavy blow, short cooldown. | Single target filler |
| 3 | **Mortal Strike** | Physical · Melee · Direct | Elites first, 6 yd | A huge blow, 6 s cooldown. | Boss killer |
| 4 | **Whirlwind** | Physical · Melee · Area · Direct | Around you, 7 yd | A spin that hits everything near. | Close-range clear |
| 5 | **Thunder Clap** | Physical · Area · Direct · Slow | Around you, 8 yd | Damages and Chills. | Control |
| 6 | **Rend** (new) | Physical · Melee · Bleed · Damage over time | The 3 closest within 6 yd | No hit: a Bleed for 9 s. | Damage over time |
| 7 | **Execute** | Physical · Melee · Direct | **The 2 most wounded** within 8 yd | +100% damage to enemies below 30% health. | Finisher of the weak |
| 8 | **Bloodthirst** (new) | Physical · Melee · Direct · Leech | Elites first, 6 yd | Heals you for 30% of its damage. | Sustain |
| 9 | **Shield Block** (new) | Physical · Ward | You | Every 6 s banks a block charge (up to 3): a blocked blow strikes its attacker back. Full: shield-slams the nearest enemy. | Mitigation, retaliation |
| 10 | **Demoralizing Shout** | Physical · Area · Shout | Around you, 11 yd | Weakens everything it reaches (25% less damage, 6 s). | Mitigation |
| 11 | **Intimidating Shout** (new) | Physical · Area · Shout · Fear | Around you, 8 yd | Everything near flees in Fear for 3 s. 14 s cooldown. | Breathing room |
| 12 | **Sunder Armor** (new) | Physical · Melee · Direct | Elites first, 6 yd | Each hit Sunders: +4% damage taken per stack, up to 5. | Boss setup (whole party) |

Sweeping Strikes (a fast pulse around you) left the pool: Whirlwind and Cleave already do its job.

**Defensive button: Charge** (F, 30 s): 12 yd the way you face; every enemy within 5 yd of where you
land is Stunned for 1.5 s. Out through the horde, or into it.

## Capstones (rank 10: 1 of 3)

★ marks the capstone that most changes how you play the spell.

### 1. Cleave
- **Blood and Thunder:** enemies it hits that are not Bleeding get your Rend (yours, with its
  augments, if you know it; else rank 1).
- ★ **Rampage:** each enemy it hits gives you +1% damage for 8 s, up to 30%. Staying in the crowd
  keeps it up.
- **Cleaving Swings:** every 3rd Cleave hits every enemy within its reach, not only the 3 closest.

### 2. Heroic Strike
- **Overpower:** when it crits, it is ready again at once and its next hit deals +50%.
- ★ **Heroic Throw:** reaches 20 yd: you hurl your weapon at the elite as a Projectile (projectile
  augments fit it at rank 15). The warrior gets a ranged elite-seeker.
- **Champion's Guard:** each hit on a boss or elite banks a block charge (up to 3, as Shield Block's,
  even without it).

### 3. Mortal Strike
- **Mortal Wounds:** its target is Marked: +20% damage from every source for 6 s (the whole party).
- **Sudden Death:** each kill by your Melee spells has a 10% chance to make Mortal Strike ready.
- ★ **Colossus Smash:** also strikes every enemy within 6 yd of its target for 50% and Stuns them
  for 1 s; +50% cooldown. You fight the boss inside its guard.

### 4. Whirlwind
- ★ **Bladestorm:** becomes a storm of blades that follows you for 4 s, striking everything within
  reach every half second at 40%. Run through the horde.
- **Furious Spin:** each kill it makes takes 0.5 s off its cooldown.
- **Second Wind:** each enemy it hits heals you for 1% of your health, up to 10% a spin.

### 5. Thunder Clap
- ★ **Shockwave:** a 12-yard cone ahead of you instead; +40% damage; Stuns for 2 s instead of
  Chilling. You aim it with your facing.
- **Aftershock:** leaves the ground shaking for 4 s: it hits for 25% and Chills every second.
- **Thunderstruck:** enemies it hits are also Weakened (25% less damage) for 6 s.

### 6. Rend
- **Gushing Wound:** when a Bleeding enemy dies, its bleed bursts onto every enemy within 5 yd.
- ★ **Blood Frenzy:** your Bleeds heal you for 20% of the damage they deal. Spread them wide to
  stay alive.
- **Deep Wounds:** your Melee crits make their target Bleed with Rend (yours, or rank 1).

### 7. Execute
- **Executioner's Chain:** a kill casts Execute again at once at the next most wounded enemy.
- **Massacre:** its bonus counts below 50% health instead of 30%.
- ★ **Victory Rush:** each kill heals you for 5% of your health. Execute becomes your sustain:
  you want wounded enemies around you.

### 8. Bloodthirst
- ★ **Blood Craze:** below 50% health, its cooldown is halved and it heals twice as much. You can
  hold yourself at half health on purpose.
- **Unbridled Wrath:** its crits cast Whirlwind (yours, or rank 1), at most every 4 s.
- **Bloodbath:** its target also Bleeds for 100% of the hit over 6 s.

### 9. Shield Block
- ★ **Revenge:** every block spent strikes every enemy within 6 yd for 150% of its damage. Diving
  in with a full bank is your area damage.
- **Shield Bash:** a blocked attacker is Stunned for 1.5 s.
- **Last Stand:** below 30% health, every charge refills at once (at most every 20 s).

### 10. Demoralizing Shout
- ★ **Rallying Cry:** each enemy it reaches heals you for 1% of your health, up to 15%. Shout from
  the thick of it.
- **Crippling Presence:** Weakened enemies are also Chilled.
- **War Cry:** also gives you and every ally within 20 yd +15% damage for 6 s.

### 11. Intimidating Shout
- ★ **Challenging Shout:** instead of scattering them, it pulls every enemy within 15 yd to you,
  and you take 40% less damage for 4 s. Gather, then Whirlwind.
- **Shattering Roar:** everything it reaches is also Sundered twice.
- **Fearsome:** its Fear holds elites as long as normal enemies.

### 12. Sunder Armor
- **Expose Armor:** at 5 stacks the armor shatters: the stacks go and the target is Marked for +25%
  for 8 s.
- ★ **Devastate:** hits every enemy within 6 yd of you instead (at 60%), each taking a stack. The
  party's area spells follow.
- **Sundering Blows:** your Melee crits on a Sundered enemy add a stack.

## Shared augments the warrior brings

New tags Bleed, Shout, Ward and Leech (see the shared draft): Lacerate and Hemorrhage (Bleed),
Booming Voice and Commanding (Shout), Deep Reserves and Overflowing (Ward), Sanguine and
Bloodthirsty (Leech), Sweeping (Melee), Dread and Panic (Fear), Crushing (Stun, through Charge's and
Shockwave's stuns).

**Dead-end checks:** Brute Force on Thunder Clap removes its Chill: Aftershock and Thunderstruck still
work. Brute Force on Intimidating Shout removes its Fear: Challenging Shout and Shattering Roar still
work. Brute Force on Shockwave removes the Stun: it still hits a cone for +40%.

## Checks

- **The tank build:** Shield Block with Deep Reserves and **Revenge**, Demoralizing Shout with
  **Rallying Cry**, Intimidating Shout with **Challenging Shout**: gather the horde, take its blows on
  five block charges, each block a spin, each shout a heal. Retaliation (11c) is its damage.
- **The bleed build:** Rend with Lacerate and **Blood Frenzy**, Cleave with **Blood and Thunder**,
  Whirlwind with **Bladestorm**: run through the crowd spreading stacked bleeds that heal you.
- **The boss build:** Sunder Armor with **Expose Armor**, Mortal Strike with **Mortal Wounds**,
  Heroic Strike with **Heroic Throw**; the party's damage on the boss goes up by half.
