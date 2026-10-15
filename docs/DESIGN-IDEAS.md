# Azeroth Together: design ideas and decisions

A living list of what we might add, where the idea comes from, and where it stands. Nothing here is
built unless its status says **Done**. Update the table as decisions are made; the sections below it
hold the thinking behind the open topics.

**Status:** ✅ Done · 🟢 Greenlit (we want it; design may still need detail) · 💬 Discussing ·
💡 New idea (needs a yes/no) · ⛔ Not doing

## Decision table

| # | Feature | Inspiration | Status | Notes |
|---|---|---|---|---|
| 0a | Co-op for up to 4, LAN and internet invite codes | — | ✅ Done | Host-authoritative; invite codes via iroh. |
| 0b | Revive a fallen friend by standing beside them | — | ✅ Done | 3 s, half health, 2 s grace. |
| 0c | Rarity-rolled spell cards, reroll, skip | Azeroth Survivors | ✅ Done | |
| 0d | Damage meter, WoW party frames | Details!/Recount | ✅ Done | |
| 1a | **Tomes**: stat upgrades that take no slot | Megabonk | 🟢 Greenlit | Damage, cooldown, area, speed, health, regen, armor, pickup range, luck, XP and gold gain. Come as cards beside spell cards. |
| 1b | **In-run items** that take a slot (trinket, neck, rings) | Azeroth Survivors, Megabonk | 🟢 Greenlit | Passive and proc effects; few slots, so choosing matters. See [A](#a-items-and-tomes-in-a-run). |
| 2a | **Evolutions**: max-rank spell + a specific item or tome becomes a stronger spell | Vampire Survivors | 🟢 Greenlit | See [C](#c-evolutions-fusions-and-augments). |
| 2b | **Fusions**: two spells merge into one (Frostbolt + Pyroblast → a frostfire spell) | Your idea | 💬 Discussing | Frees a slot; carries both spells' tags; anything that casts either casts the fusion. |
| 2c | **Augments**: at ranks 4 and 8 pick a modifier for that spell (e.g. "Pyroblast deals Frost damage") | Hades boons | 💡 New idea | Lets off-school spells join a build mid-run. |
| 2d | **Grimoire**: a codex in the menu of discovered evolutions and fusions, undiscovered ones as hints | VS collection, PoE wiki culture | 💡 New idea | Knowledge as meta-progression; feeds theorycrafting. |
| 3 | Gold, chests (price rises per open), a goblin merchant | Megabonk, Azeroth Survivors | 🟢 Greenlit | Being prototyped on the Megabonk branch. |
| 4 | Elites (drop chests) and a stage boss on a timer | Azeroth Survivors, VS, Megabonk | 🟢 Greenlit | Bosses are also where meta equipment drops (9a). |
| 5 | Risk for reward: greed shrine, cursed tome, boss curse | Megabonk | 🟢 Greenlit | Being prototyped on the Megabonk branch. |
| 6 | Ground pickups: food (heal), magnet (all XP), bomb (clear screen) | Vampire Survivors | 🟢 Greenlit | WoW flavour: Conjured bread, a goblin magnet, a Seaforium charge. |
| 7a | **Freeze** the card screen | Your idea | 🟢 Greenlit | Freeze, pick a card now; the next level-up shows the frozen hand instead of a new one. Details in [G](#g-the-card-screen-freeze-and-banish). |
| 7b | **Banish** a spell from the pool, once per run | Vampire Survivors | 🟢 Greenlit | |
| 8 | Map events: challenge shrines, swarm waves, statues with choice events | Megabonk, VS | 🟢 Greenlit | Being prototyped on the Megabonk branch. |
| 9a | **Meta equipment**: boss drops, rarities, item level by zone, Unique effects that change a spell or class | Azeroth Survivors, Battleborn, Diablo | 🟢 Greenlit | See [D](#d-progression-between-runs). Open: class-bound or shared. |
| 9b | **Talent web** between runs (one shared Path of Exile-style web, not one tree per spec) | Path of Exile | 💬 Discussing | Must widen options, never pre-decide the build. See [D](#d-progression-between-runs). |
| 9c | **Classless mode** with synergy-weighted card offers | Your idea | 💬 Discussing | Viable on top of spell tags (9d). See [E](#e-classless-mode). |
| 9d | **Spell tags** (school, delivery, effect) that talents, items and tomes modify | Path of Exile | 💬 Discussing | The foundation for 2, 9b and 9c. See [B](#b-spell-tags). |
| 9e | Unlocks from challenges (new spells, items, heroes, zones) | VS, Megabonk, Brotato | 💡 New idea | "Kill the boss in under 5 minutes unlocks …". |
| 9f | **Battleborn-style gear activation**: the gear you bring is dormant and you switch it on in the run with gold | Battleborn | 💡 New idea | Turns meta gear into an in-run decision. |
| 9g | Difficulty ladder (Torment levels) that raises item level and drop rarity | Diablo, Hades Heat, Brotato Danger | 💡 New idea | Makes low zones farmable and high zones worth the risk. |
| 10a | **Defensive / movement button**: one per class, swappable from unlocked options | Your idea | 🟢 Greenlit | See [F](#f-the-defensive-button-and-moving-well). |
| 10b | Movement feel: dash charges, double jump, slow fall, momentum, jump pads | Megabonk | 💬 Discussing | See [F](#f-the-defensive-button-and-moving-well). |
| 11 | Equipment both in a run (1b) and between runs (9a) | — | 🟢 Greenlit | Covered by 1b and 9a. |
| 12 | Co-op synergies across players (one player's root enables another's Shatter) | — | 💡 New idea | Falls out of tags for free. See [H](#h-co-op-only-ideas). |
| — | Talent trees that lock a run into one spec before it starts | Azeroth Survivors | ⛔ Not doing | Takes away in-run choice: frost talents make every non-frost spell a trap pick. |
| — | A permanent stat shop as the main progression (+5% damage forever …) | VS | 💬 Discussing | Fine as a small extra; as the core it is a grind with no decisions. |

## A. Items and tomes in a run

The split Megabonk uses, and the one we take: **tomes** are numbers and stack freely; **items** are
effects and compete for a few slots.

- **Tomes** (no slot): every card adds a little to one stat, at the card's rarity. They make a build
  stronger without changing what it is.
- **Items** (slots: say 2 trinkets, 1 neck, 2 rings): an item does something new, like a proc, a
  conversion or a rule change. Five slots means you say no to most items, which is the decision
  we want.
- Item examples in WoW flavour:
  - **Talisman of Ephemeral Power**: every 20 s, +40% spell damage for 5 s.
  - **Band of Accuria**: crits have a chance to cast your lowest-rank spell again.
  - **Hand of Justice**: 2% chance on hit to hit again.
  - **Darkmoon Card: Blue Dragon**: casts sometimes cost no cooldown.
  - **Ring of the Frozen Wastes**: your Fire spells also Chill.
- Items carry **tags** too (B), so "+1 Chain jump" is a ring that only a Chain build wants.

## B. Spell tags

Every spell gets tags from three families; talents, items, tomes and augments refer to tags, never
to a single spell's name. Tags appear as small chips on the card, like Path of Exile's gem tags.

| Family | Tags |
|---|---|
| School | Physical, Fire, Frost, Arcane, Nature, Shadow, Holy |
| Delivery | Projectile, Melee, Chain, Area (nova), Ground (lasting zone), Beam, Summon, Aura, Totem |
| Effect | Direct, Damage over time, Control (Root, Slow, Stun, Fear), Heal, Shield |

Examples:
- **Frostbolt**: Frost · Projectile · Direct · Slow
- **Blizzard**: Frost · Ground · Area
- **Chain Lightning**: Nature · Chain · Direct
- **Immolate**: Fire · Direct · Damage over time
- **Consecration**: Holy · Ground · Area
- **Frost Nova**: Frost · Area · Root

Modifiers then read naturally:
- "+15% Fire damage"
- "Chain spells jump once more"
- "Projectiles: +1"
- "Damage over time ticks 10% faster"
- "Ground effects last 25% longer"
- "+30% crit chance against Rooted enemies" (Shatter)

The point: a spell is worth taking when its *tags* fit your build, not only when its school matches
your talents. Pyroblast is a Projectile and a Direct hit, so a projectile build wants it even when
it is mostly frost.

## C. Evolutions, fusions and augments

Three ways a spell grows past rank 8, each solving a different problem:

1. **Evolution** (Vampire Survivors): spell at max rank + a specific item or tome in your build →
   an evolved spell. Example: Arcane Missiles + Tome of Intellect → **Arcane Barrage**, which fires
   at every enemy in range. Recipes are fixed, so they reward knowledge: plan around one, adapt when
   the item does not show.
2. **Fusion** (your idea): two spells, both max rank, sharing at least one tag (or on a recipe list),
   fuse into one spell in one slot.
   - The fused spell carries **both** sets of tags, so every Fire *and* every Frost modifier applies.
     That is the reason to take Pyroblast in a frost build.
   - Anything that casts or triggers either parent casts the fusion instead. A "cast Frostbolt on
     crit" item casts the frostfire spell.
   - Freeing a slot is itself the reward: fusing lets a build hold seven spells' worth of power in
     six slots.
   - Examples:
     - Frostbolt + Fireball → **Frostfire Bolt**
     - Chain Lightning + Holy Shock → **Chain Heal** (heals allies on its jumps)
     - Shadow Bolt + Immolate → **Shadowburn**
     - Multi-Shot + Explosive Trap → **Volley**
3. **Augment** (Hades-like): at ranks 4 and 8, the upgrade card offers a choice of modifiers for
   that spell.
   - Examples for Pyroblast: "deals Frost damage instead", "+1 projectile, −20% damage each",
     "leaves burning ground".
   - Conversion augments are how an off-school spell joins a build mid-run, without any talent
     deciding it beforehand.

The **Grimoire** (2d) records every evolution and fusion you have found and shows the rest as hints
("A frost spell and a fire spell, both mastered…"). Discovering recipes becomes progression too.

## D. Progression between runs

**The rule we hold everything to: progression between runs widens your options; the run decides
the build.** Your criticism of Azeroth Survivors' trees is the thing to avoid: if pre-run talents
make one school strictly best, every in-run choice is already made.

How the talent web follows that rule:

- **One shared web, Path of Exile style.** Each class starts in its own region (the mage near Fire,
  Frost and Arcane; the paladin near Holy and Physical), and regions overlap, so a mage can walk
  toward Shadow or Nature.
- **Three node sizes:**
  - **Small:** +1 Fire damage, +1 attack power, +1% move speed, +2% area.
  - **Notable:** your examples:
    - "Fire crits Ignite for 40% of the hit"
    - "Two Holy crits in a row instantly cast your lowest-cooldown Holy spell"
    - "Single-target damage heals the lowest-health ally within 30 yards for 10% of it"
  - **Keystone:** a rule change with a cost, for example:
    - **Elemental Equilibrium:** a Fire hit makes the target take +25% Frost damage and −25% Fire
      damage for 4 s, which rewards alternating schools.
    - **Glass Cannon:** +40% damage, but max health is halved.
- **Hybrid notables on purpose.** "Frostfire: +25% Fire damage against Chilled enemies, +25% Frost
  damage against Burning ones" makes mixing *better* than going pure, which is the opposite of what
  spec trees do.
- **Talents can steer offers instead of adding damage.** "Fire spells appear 25% more often" guides
  a run without forcing it; you can still take the Holy card that showed up.
- **Free respec and saved loadouts**, so trying a new idea costs nothing.
- **A power budget.** Everything from between runs together should be roughly a quarter to a third of
  a finished run's power. Most power must still come from the run, or the choices in it stop
  mattering.

**Equipment (9a), from your Battleborn and Azeroth Survivors notes:**

- **Slots:** weapon, head, chest, trinket, two rings. Six slots, saved as loadouts per hero.
- **Rarity:** Common to Legendary. **Item level** comes from the zone's level (and the Torment
  level, 9g), so low zones give low gear and high zones are worth the risk.
- **Drops:** from bosses (a chance) and elites (rarely).
- **Uniques** change one spell or class, and are the theorycrafting hooks:
  - **Thunderfury:** Chain Lightning jumps two more times and slows.
  - **Atiesh:** Arcane Explosion pulls enemies in.
  - **Ashkandi:** your Charge leaves a shockwave.
- **It's WoW:** the gear can show on your hero's model. The engine already dresses heroes from item
  displays.
- **Battleborn's twist (9f):** gear you bring starts dormant and you activate pieces during the run
  with gold. Your loadout becomes a plan you carry out under pressure, not a flat bonus.

## E. Classless mode

Viable, and tags (B) are what make it work.

- **Pool:** every class's spells, each with tags.
- **Synergy rolls:** a card's chance to be offered grows with the tags it shares with what you
  already have, for example +50% per shared tag. One card in each hand ignores synergy, so a pivot
  is always possible.
- **Your fire example plays out naturally:**
  - The run offers Fireball (mage), Flame Shock (shaman), Immolate (warlock) and Blast Wave.
  - Talents add Ignite and crit.
  - Frost Nova roots enemies, and the Shatter talent turns those roots into crits.
  - That is a fire / damage-over-time / crit build assembled from three classes.
- **Costs to weigh:**
  - It needs many more spells with tags: we have 68 today.
  - Balance across classes gets harder.
  - Healing and support spells must matter, which they do in co-op.
- **Suggestion:** make it its own hero (an "Adventurer" or "Wanderer") or a mode unlocked later,
  so the classes keep their identity. Synergy rolls could also apply to normal classes, more
  gently.

## F. The defensive button and moving well

**One extra key per class** (Shift, or a bar slot): a cooldown ability you choose from options you
have unlocked, and that in-run cards can upgrade ("Blink: 2 charges", "Charge stuns on impact").

| Class | Movement | Defensive | Control |
|---|---|---|---|
| Mage | Blink | Ice Block | Frost Nova |
| Warrior | Charge (toward the cursor) | Shield Wall | Intimidating Shout |
| Rogue | Sprint | Evasion | Vanish (drop threat) |
| Priest | Levitate (glide) | Power Word: Shield | Psychic Scream |
| Paladin | Blessing of Freedom | Divine Shield | Hammer of Justice |
| Hunter | Disengage-style leap | Feign Death | Frost Trap |
| Druid | Dash / Travel Form | Barkskin | Entangling Roots |
| Shaman | Ghost Wolf | Grounding | Earthbind Totem |
| Warlock | Shadow step (a demonic leap) | Soulstone (self-revive) | Death Coil |

**Movement feel**, not tied too tightly to WoW:

- **Dash charges** with a short invulnerability, the best-feeling single addition in the genre.
- **Double jump** as an item: Goblin Rocket Boots, a joke that plays well.
- **Slow fall / glide** after a jump (Slow Fall, Levitate): long air time over the horde.
- **Momentum:** speed builds while you keep moving and drops when you stop or get hit; a tome
  stat.
- **Jump pads:** goblin launchers as map gameobjects that fling you across the arena.
- **Speed on kill** for a moment (Blood Fury style), so aggressive play is also mobile play.
- **A mount pickup:** ten seconds mounted that tramples small enemies.

WoW maps have little verticality, so jumps should be about crossing the horde, not climbing.

## G. The card screen: freeze and banish

- **Freeze** (your design): on the level-up screen, freeze the hand, then pick a card as usual. The
  next level-up shows the frozen hand again instead of dealing a new one.
  - Open: does the card you picked get replaced, or does the next hand have one card fewer?
  - Open: how many freezes per run?
- **Banish**, once per run: remove one spell from the pool for the rest of the run, so it never
  shows again.

## H. Co-op-only ideas

- **Cross-player combos come free with tags.** Your Frost Nova roots; your friend's Shatter crits
  them. Your Hunter's Mark makes a target take more from everyone.
- **Auras** (paladin, shaman totems) that buff nearby allies make standing together a choice with a
  payoff, against the horde's pressure to spread out.
- **Healing and shield tags** become real build paths in co-op.
- **Shared chests:** whoever opens one, everyone gets a pick (or the party votes).

## Open questions for us

1. Talent points and gear: per class (as in Azeroth Survivors) or shared across the account? A
   middle way: class-specific Uniques and weapons, a shared stash for the rest.
2. Freeze: replace the picked card or not, and how many freezes per run?
3. Classless: its own hero/mode, or a toggle on every class?
4. Evolutions *and* fusions *and* augments, or a subset? All three are compatible, but three systems
   is a lot to learn.
5. In-run item slots: how many, and which?
6. The power budget for progression between runs: is roughly 25–35% right?
7. The defensive button: which key, and roughly what cooldowns (10–30 s)?
