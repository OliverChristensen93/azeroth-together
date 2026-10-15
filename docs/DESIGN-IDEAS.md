# Azeroth Together: design ideas and decisions

A living list of what we might add, where the idea comes from, and where it stands. Nothing here is
built unless its status says **Done**. Update the table as decisions are made; the sections below it
hold the thinking behind the open topics.

**Status:** ✅ Done · 🧪 Prototyping on the Megabonk branch · 🟢 Greenlit (we want it; design may
still need detail) · 💬 Discussing · 💡 New idea (needs a yes/no) · ⛔ Not doing

**The Megabonk branch** (`megabonk-systems`, private repo `OliverChristensen93/azeroth-together-megabonk`,
built by the "Build Megabonk-style shrines and events" session) is a playable experiment. What it
builds is marked 🧪; we take from it what works when the systems are designed for the main game.
Finished and playable (2026-10-15, 3 commits): gold shared like XP (each player pays for their own
chests), a hero stat block (damage, cooldown, area, speed, health, regen, armor, pickup range, luck,
XP, gold, crit, extra targets), tome and pact cards, elites from 1:30, a boss per zone every 4
minutes (Hogger, Sergeant Brashclaw, Fenros, Gurubashi Warlord, Anubisath Guardian), 8 stacking
items (no slots), and ten interactables used by standing in their circle: Titan Shrine, Golden
Titan Shrine, Cursed Altar, Shrine of Greed, Treasure Chest, Challenge Idol, Moonwell, Goblin
Merchant, Blood Shrine (an extra boss copy for double reward) and Champion's Cache. Run it from its
worktree (`.claude/worktrees/sweet-noyce-99e61b`) with `WOW_DATA` pointing at the main `WoW/Data`;
`TOGETHER_SHOWCASE=1` rings one of each shrine around the start.

## Decision table

| # | Feature | Inspiration | Status | Notes |
|---|---|---|---|---|
| 0a | Co-op for up to 4, LAN and internet invite codes | — | ✅ Done | Host-authoritative; invite codes via iroh. |
| 0b | Revive a fallen friend by standing beside them | — | ✅ Done | 3 s, half health, 2 s grace. |
| 0c | Rarity-rolled spell cards, reroll, skip | Azeroth Survivors | ✅ Done | |
| 0d | Damage meter, WoW party frames | Details!/Recount | ✅ Done | |
| 1a | **Tomes**: stat upgrades that take no slot | Megabonk | 🟢 Greenlit · 🧪 | Damage, cooldown, area, speed, health, regen, armor, pickup range, luck, XP and gold gain, as cards beside spell cards. Built on the Megabonk branch. |
| 1b | **In-run items** in 5 slots: 2 rings, 2 trinkets, 1 necklace | Azeroth Survivors, Megabonk | 💬 Parked · 🧪 | Drop from chests, equipped in the run. The necklace is the strongest slot and can carry Unique effects that augment or link spells (2e). The Megabonk branch has 8 items that stack as copies, with no slots; ours differ. See [A](#a-items-and-tomes-in-a-run). |
| 2a | Evolutions: max-rank spell + a specific item becomes a new spell | Vampire Survivors | ⛔ Skipped for now | Fixed recipes turn a run into a hunt for the partner piece. We test augments (2c) first and revisit. |
| 2b | Fusions: two spells merge into one | — | ⛔ Not doing | Fixed recipes force a hunt; any-with-any has no good meaning; and fusing *removes* half of each spell (a trap's summon, a damage spell's damage), stranding the modifiers built for it. |
| 2c | **Augments**: general augments at ranks 5 and 10, a capstone (1 of the spell's 3) at rank 15 (max) | Hades, Azeroth Survivors items | 🟢 Greenlit | The core way spells grow. Mage drafted: [MAGE-DRAFT.md](MAGE-DRAFT.md). Bonus Augment screen with Pick / Reroll / Hold; pauses co-op like a level-up. See [C](#c-augments). |
| 2d | Grimoire: a codex of augments and Uniques you have found, the rest as hints | VS collection | 💡 New idea | Knowledge as progression between runs. |
| 2e | **Unique items that augment or link spells** (e.g. "When Ice Barrier breaks, cast Frost Nova") | Azeroth Survivors | 🟢 Greenlit | Mostly necklaces. Triggered casts use your rank and augments and ignore cooldown. |
| 2f | **Spell design rules**: about 12 spells per class, each reworked for the genre; single-target spells prioritize bosses and elites | Azeroth Survivors | 🟢 Greenlit | Kidney Shot just damages and stuns. Single-target and area spells both earn a place. We have 68 spells today (~7.5 per class). |
| 2g | Talents that **add augments to the pool** instead of raw power | — | 💡 New idea | The cleanest "widen, don't narrow" progression. See [C](#c-augments). |
| 3 | Gold, chests (price rises per open), a goblin merchant | Megabonk, Azeroth Survivors | 🟢 Greenlit · 🧪 | Gold, Treasure Chests and the Goblin Merchant built on the Megabonk branch. Gold also switches on dormant gear (9f). |
| 4 | Elites (drop chests) and a stage boss on a timer | Azeroth Survivors, VS, Megabonk | 🟢 Greenlit · 🧪 | Elites and a boss per zone (every 4 minutes) built on the Megabonk branch. Bosses are also where meta equipment drops (9a). |
| 5 | Risk for reward: greed shrine, cursed altar pacts (boon + bane), cursed tome, boss curse | Megabonk | 🟢 Greenlit · 🧪 | Shrine of Greed, Cursed Altar pacts and the Blood Shrine (boss curse) built on the Megabonk branch; the cursed tome is not. |
| 6 | Ground pickups: food (heal), magnet (all XP), bomb (clear screen) | Vampire Survivors | 🟢 Greenlit · 🧪 | The magnet exists on the Megabonk branch as a Moonwell shrine; food and bomb do not. WoW flavour: Conjured bread, a Seaforium charge. |
| 7a | **Freeze** the card screen | Your idea | 🟢 Greenlit | Freeze as often as you like. The card you pick from a frozen hand is not replaced, so the next level-up offers one card fewer; unfreezing deals a fresh hand. See [G](#g-the-card-screen-freeze-and-banish). |
| 7b | **Banish** a spell from the pool, once per run | Vampire Survivors | 🟢 Greenlit | |
| 8 | Map events: challenge shrines, swarm waves, statues with choice events | Megabonk, VS | 🟢 Greenlit · 🧪 | Titan Shrines (tome choice), Golden Titan Shrine and Challenge Idol (elite pack, then a cache) built on the Megabonk branch; swarm waves are not. |
| 9a | **Meta equipment**: armor and weapons, boss drops, rarities, item level by zone, Unique effects | Azeroth Survivors, Battleborn, Diablo | 🟢 Greenlit | Head, shoulders, cape, chest, bracers, hands, legs, waist, boots, weapons. Brought into the run dormant (9f). See [D](#d-progression-between-runs). |
| 9b | **Talent web** between runs (one shared Path of Exile-style web) | Path of Exile | 🟢 Start here | Try the web first, revisit spec trees if it does not work. It must support the frostfire benchmark build ([D](#d-progression-between-runs)). |
| 9c | **Classless mode** with synergy-weighted card offers | Your idea | 🟢 Greenlit | Its own mode. Needs spell tags (9d) first. See [E](#e-classless-mode). |
| 9d | **Spell tags** (school, delivery, effect) that talents, items, tomes and augments modify | Path of Exile | 💬 Discussing | The foundation for augments, the web and classless. See [B](#b-spell-tags). |
| 9e | Unlocks from challenges (new spells, items, heroes, zones) | VS, Megabonk, Brotato | 💡 New idea | "Kill the boss in under 5 minutes unlocks …". |
| 9f | **Dormant gear switched on with gold** during the run | Battleborn | 🟢 Greenlit | Gold choice: a chest (a chance at a ring) or a gear piece you know. Your hero could visibly gear up as pieces switch on. |
| 9g | Difficulty ladder (Torment levels) that raises item level and drop rarity | Diablo, Hades Heat, Brotato Danger | 💡 New idea | Makes low zones farmable and high zones worth the risk. |
| 9h | A setting: talent points and equipment per class, or shared by all classes | — | 🟢 Greenlit | Test both and keep the one that plays best. |
| 9i | Power budget for progression between runs | — | 🟢 Agreed | 0% at the start, up to about 40% late (talents and gear together), when meta synergies meet in-run picks that fit them. |
| 10a | **Defensive button** (movement, defensive or control): one per class, swappable from unlocked options | Your idea | 🟢 Greenlit | On Q, E or F (rebindable); cooldowns 30 s to 2 min; can be augmented. No cast times for now. See [F](#f-the-defensive-button-and-moving-well). |
| 10b | Movement feel | Megabonk | 🟢 Agreed | **No universal dash.** Movement comes from the defensive button (for classes that pick a movement option), from items and tomes (move speed, Goblin Rocket Boots double jump, glide, momentum), and possibly goblin launch pads on the map. See [F](#f-the-defensive-button-and-moving-well). |
| 10c | An **ultimate** beside the defensive button, possibly with a stand-still cast time | Megabonk campfire | 💬 Later | Parked until the defensive button exists. |
| 11a | **Every build needs a survival answer**, not a tank or healer: distance, control, mitigation, sustain, escape, kill speed | Your idea | 🟢 Greenlit | Each archetype is softly countered by some elite or boss, so none is always right. See [I](#i-surviving-solo-without-a-tank-or-healer). |
| 11b | **Charges**: heals and blocks cast on cooldown bank charges that spend themselves when needed | Your idea; WoW's Prayer of Mending, Earth Shield, Holy Shield | 🟢 Greenlit | Removes the "want to get hit" incentive; stacks show on the WoW buff frame. Open: every heal and block spell, or only some. |
| 11c | **Baseline damage for heal and tank builds**: overflow and retaliation | — | 🟢 Agreed | A full heal bank turns further heals into Holy damage; blocked, dodged or absorbed hits deal damage back. No item needed. A per-enemy attack limit was considered and dropped. |
| 11d | **Knockback and death grip**: player spells push enemies; some elites and bosses push or pull you, always telegraphed | Megabonk, WoW bosses | 🟢 Greenlit | Never from ordinary enemies. |
| 11e | **Elite and boss mechanics**: charges, casters, slams, leaps, ground zones, marked areas, summoners and healers, things to run to | WoW raids, Mythic+ | 🟢 Greenlit | **Every mechanic can be dodged by moving** (agreed). Crowd control on elites has diminishing returns. See [I](#i-surviving-solo-without-a-tank-or-healer). |
| 11f | **Boss bar** at the top of the screen: name, health, and one sentence explaining the mechanic | WoW boss mods, Megabonk | 🟢 Agreed | Elites get a sentence on their target frame too. |
| 11g | **Affixes** from Mythic+ and Project Ascension's Manastorm: vortex, slowing field, sanguine, bolstering, volcanic, storming, spiteful, explosive | Mythic+, Ascension | 🟢 Greenlit | Open: chosen before the run, owned by certain bosses, or added in a run's later stages. See [I](#i-surviving-solo-without-a-tank-or-healer). |
| 12 | Co-op synergies across players (one player's root enables another's Shatter) | — | 💡 New idea | Falls out of tags for free. See [H](#h-co-op-only-ideas). |
| — | Talent trees that lock a run into one spec before it starts | Azeroth Survivors | ⛔ Not doing | Takes away in-run choice: frost talents make every non-frost spell a trap pick. |
| — | A permanent stat shop as the main progression (+5% damage forever …) | VS | 💬 Discussing | Fine as a small extra; as the core it is a grind with no decisions. |

## A. Items and tomes in a run

> **Parked (2026-10-15): which of items and tomes carries the special effects.** On the Megabonk
> branch (and in Megabonk itself) items are unlimited and carry the unique effects, while tomes are
> general stat upgrades. The 5-slot jewelry idea below puts the effects in limited slots instead. A
> third option, closer to Megabonk: **items take limited slots and carry the unique effects; tomes
> are unlimited and general.** Either way, both items and tomes can be upgraded a set number of
> times. To decide after playing the Megabonk branch more.
>
> **Agreed: upgrades scale with rarity, as in Megabonk.** A duplicate or upgrade raises the existing
> tome or item, by how much depending on the card's rarity: "+5% damage" upgraded by a Common card
> goes to 10%, by a Legendary card to 21%.

**Tomes** are numbers and stack freely; **items** are effects and compete for slots.

- **Tomes** (no slot): every card adds a little to one stat, at the card's rarity. They make a build
  stronger without changing what it is.
- **Items** (5 slots: 2 rings, 2 trinkets, 1 necklace), from chests, equipped in the run: an item
  does something new, like a proc, a conversion or a rule change. Five slots means saying no to most
  items.
- **The necklace** is the strongest slot and the home of Unique effects that augment or link spells,
  like Azeroth Survivors' Ice Barrier item: "When Ice Barrier breaks or expires, deal 3× the damage
  it absorbed in 15 yards and cast Frost Nova. Ice Barrier shields 30% more, cooldown −30%."
- Ring and trinket examples:
  - **Talisman of Ephemeral Power**: every 20 s, +40% spell damage for 5 s.
  - **Band of Accuria**: crits have a chance to cast the spell again.
  - **Hand of Justice**: 2% chance on hit to hit again.
  - **Ring of the Frozen Wastes**: your Fire spells also Chill.

## B. Spell tags

Every spell gets tags from three families; talents, items, tomes and augments refer to tags, not to a
single spell's name. Tags appear as chips on the card, like Path of Exile's gem tags.

| Family | Tags |
|---|---|
| School | Physical, Fire, Frost, Arcane, Nature, Shadow, Holy (and pairs from conversion: Frostfire, Shadowflame…) |
| Delivery | Projectile, Melee, Chain, Area (nova), Ground (lasting zone), Beam, Summon, Aura, Totem, Shield |
| Effect | Direct, Damage over time, Slow, Root, Freeze, Stun, Fear, Knockback, Heal |

Examples:
- **Frostbolt**: Frost · Projectile · Direct · Slow
- **Blast Wave**: Fire · Area · Direct · Slow · Knockback
- **Chain Lightning**: Nature · Chain · Direct
- **Immolate**: Fire · Direct · Damage over time
- **Ice Barrier**: Frost · Shield
- **Frost Nova**: Frost · Area · Direct · Freeze

A conversion augment *changes tags*: Fireball augmented to Frostfire is Fire and Frost, so both
schools' bonuses apply. That is how an off-school spell joins a build without any talent deciding it
beforehand.

## C. Augments

Spells grow by **augments**. Agreed structure (draft 2, 2026-10-15), with the full mage example in
[MAGE-DRAFT.md](MAGE-DRAFT.md):

| Rank | What you get |
|---|---|
| 5 | 1 **general** augment from 3 offered, drawn from the shared library by the spell's tags |
| 10 | 1 more **general** augment, the same way |
| 15 (max) | 1 **capstone** from the spell's own **3 unique augments**, always the same 3 |

**Why this shape.** Two general picks let a spell take a conversion *and* a shape (Frostfire +
Multishot), which cross-school builds need. Fixed capstones are plannable: you know what Fireball can
become and can steer toward it, which is theorycrafting. And there is less to write: 3 capstones per
spell (36 per class) plus one shared library.

**Pacing.** A 15-minute run gives maybe 50–60 picks shared between 6 spells, tomes and items. Maxing
6 spells would take 84 rank-ups, so most spells get their two general augments and the two or three
you commit to reach their capstone. Tune when playable.

**The Augment screen (agreed).** It opens when a spell reaches rank 5 or 10, as a bonus screen that
does not use a level-up. Pick, **reroll** (with the run's rerolls) or **hold**: holding keeps the
spell at its rank until you augment it, and the screen comes back with 3 fresh options after your
next level-up. The capstone screen at 15 just shows the spell's 3 capstones. In co-op, augment
screens pause the run like level-ups (agreed for now; picking with 1/2/3 while moving is the
fallback).

- **Synergy weighting:** general offers lean toward your build's tags (Frost spells owned →
  Frostfire more likely than Spellfire), never guaranteed; talents can lean them further (2g).
- **One conversion per spell.**

**Rules for writing augments (agreed):**
- A unique augment must do something **no shared augment can** (no "+2 projectiles" capstone when
  Multishot exists).
- Every spell's three capstones include **at least one that changes how you play** the spell.
- **No general augment may dead-end a spell:** if one removes a property (Brute Force removes
  control effects), at least one of that spell's capstones must still work without it.

**Triggered casts:** a spell cast by a trigger is the real spell, with your rank, augments and
modifiers, and it ignores its own cooldown; if you do not own it, it casts at rank 1. Triggers and
cooldown resets have an **internal cooldown**, so no two of them can loop forever (Cold Snap plus
Shattering Barrier would otherwise cast Frost Nova endlessly in a dense horde).

**Targeting gives spells their jobs.** Single-target spells hit elites and bosses first; others aim
at the densest group, around you, ahead of you, or at the enemy closest to you (Fire Blast, the "get
off me" spell). Two spells of one school should differ in job, not only in numbers.

**Talents can add augments to the pool** (2g): "Frostfire conversions can appear on any Fire or
Frost spell". That widens options instead of adding raw power.

**Roll-out:** the mage first (12 spells, 36 capstones, the first ~32 shared augments) to prove the
system before writing the rest.

## D. Progression between runs

**The rule: progression between runs widens your options; the run decides the build.** If pre-run
talents make one school strictly best, every in-run choice is already made.

**The talent web (start here):**

- **One shared web.** Each class starts in its own region, and regions overlap, so a mage can walk
  toward Shadow or Nature.
- **Small nodes:** +1 Fire damage, +1 attack power, +1% move speed, +2% area.
- **Notables**, for example:
  - "Fire crits Ignite for 40% of the hit"
  - "Two Holy crits in a row instantly cast your lowest-cooldown Holy spell"
  - "Single-target damage heals the lowest-health ally within 30 yards for 10% of it"
  - "Slows have a 15% chance to Freeze"
- **Keystones** change a rule and cost something:
  - **Elemental Equilibrium:** a Fire hit makes the target take +25% Frost damage and −25% Fire
    damage for 4 s, which rewards alternating schools.
  - **Glass Cannon:** +40% damage, half max health.
- **Hybrid notables on purpose** make mixing *better* than going pure.
- **Some talents steer offers or add augments** instead of adding damage ("Fire spells appear 25%
  more often"; "Frostfire conversions can appear on any Fire or Frost spell").
- **Free respec and saved loadouts.**

**The benchmark build** the web must support (if it cannot, the web is wrong):

1. Frost side of the web: Shatter (+crit against frozen), "slows can Freeze". Fire side: Ignite.
2. In the run you find **Blast Wave**. It slows, so it can now freeze.
3. At rank 5 you augment it to Frostfire.
4. **Pyroblast** gets the same conversion: it crits frozen targets and Ignites.
5. **Fireball** becomes Frostfire Bolt: it slows (more freezes), and Hot Streak can trigger the
   augmented Pyroblast.

Every piece came from what the run offered; the web only made the pieces fit.

**Equipment (agreed):**

- **In the run:** rings, trinkets and the necklace drop from chests and are equipped there (A).
- **Brought in:** head, shoulders, cape, chest, bracers, hands, legs, waist, boots and weapons. They
  start **dormant** and you switch them on with gold during the run, Battleborn-style (9f).
- **Rarity:** Common to Legendary. **Item level** comes from the zone (and the Torment level, 9g).
- **Drops:** from bosses (a chance) and elites (rarely).
- **Uniques** change one spell or class:
  - **Thunderfury:** Chain Lightning jumps twice more and slows.
  - **Atiesh:** Arcane Explosion pulls enemies in.
- **It's WoW:** you could start in your starting clothes and visibly gear up as pieces switch on;
  the engine already dresses heroes from item displays.
- **Budget:** talents and gear together stay within the ~40% late-game power budget (9i).

## E. Classless mode

Agreed as **its own mode**; spell tags (B) make it work.

- **Pool:** every class's spells, each with tags.
- **Synergy rolls:** a card's chance to be offered grows with the tags it shares with what you already
  have. One card in each hand ignores synergy, so a pivot is always possible.
- **Your fire example plays out naturally:**
  - The run offers Fireball (mage), Flame Shock (shaman), Immolate (warlock) and Blast Wave.
  - Talents add Ignite and crit.
  - Frost Nova roots enemies, and Shatter turns those roots into crits.
  - That is a fire / damage-over-time / crit build from three classes.
- **Costs:** more spells with tags (about 12 per class), harder balance across classes, and
  healing and support spells must matter (they do in co-op).

## F. The defensive button and moving well

**One extra key per class** (Q, E or F, rebindable): a cooldown ability of 30 seconds to 2 minutes,
chosen from options you have unlocked, which augments can change ("Blink: 2 charges", "Charge
stuns on impact", "Ice Block heals you as it ends"). Instant for now.

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

**Later, perhaps an ultimate** on a second key, with a cast time you stand still for:
- Megabonk's campfire, which heals while you stand still;
- a self-cast Regrowth;
- a channelled Blizzard or Tranquility.

Standing still in a survivors game is a real price, so a cast time is a fair cost for a strong
effect.

**Movement feel**, not tied too tightly to WoW:

- **Dash charges** with a short invulnerability, the best-feeling single addition in the genre.
- **Double jump** as an item: Goblin Rocket Boots.
- **Slow fall / glide** after a jump (Slow Fall, Levitate).
- **Momentum:** speed builds while you keep moving and drops when you stop or get hit.
- **Jump pads:** goblin launchers as map gameobjects.
- **Speed on kill** for a moment.
- **A mount pickup:** ten seconds mounted that tramples small enemies.

WoW maps have little verticality, so jumps should be about crossing the horde, not climbing.

## G. The card screen: freeze and banish

- **Freeze** (agreed): on the level-up screen, freeze the hand, then pick a card as usual. The next
  level-up shows the frozen hand again instead of dealing a new one.
  - The card you picked is **not** replaced, so the next hand has one card fewer. That is the
    price of freezing.
  - Unfreezing deals a fresh, full hand at the next level-up.
  - No limit on freezes.
- **Banish**, once per run: remove one spell from the pool for the rest of the run.

## H. Co-op-only ideas

- **Cross-player combos come free with tags.** Your Frost Nova freezes; your friend's Shatter crits
  them.
- **Auras** (paladin, shaman totems) that buff nearby allies make standing together a choice with a
  payoff.
- **Healing and shield tags** become real build paths in co-op.
- **Shared chests:** whoever opens one, everyone gets a pick (or the party votes).

## I. Surviving solo without a tank or healer

**The rule (agreed):** every run must answer "what happens when the swarm reaches me?", but the
answer does not have to be a tank or a healer. The defensive button (10a) already gives each hero
one answer; spells, augments, tomes and items deepen it.

| Archetype | How it survives | Examples | Softly countered by |
|---|---|---|---|
| **Distance** | Never lets them arrive | Sprint, Blink, move-speed tomes, Blazing Speed | Chargers, leapers, casters |
| **Control** | Stops them arriving | Frost Nova, Blizzard, Cone of Cold, Psychic Scream, Blast Wave knockback | Bosses; diminishing returns on elites |
| **Mitigation** | Arrives, but hits soft | Armor, dodge (Evasion), Shield Block charges, Ice Barrier | Spells and ground effects (no dodge or block) |
| **Sustain** | Heals through it | Lifesteal, regen, heal charges | Burst: a slam, a pack hitting at once |
| **Escape** | Leaves when it goes wrong | Vanish, Ice Block, Feign Death | Being surrounded while it is on cooldown |
| **Kill speed** | Clears before it arrives | A pure damage build | Elites and bosses with lots of health |

Counters are *soft*: a build without the right answer has a harder time, never a wall. The threats in
the right-hand column come from elites and bosses, so ordinary enemies stay fair to every build.

### Charges (11b)

A heal or block spell casts on its cooldown whether or not anyone is hurt, and each cast banks a
**charge** (up to a cap, say 5). Charges spend themselves:

- **Heal charges** (Prayer of Mending, Renew, Earth Shield): when you or a nearby ally drops below a
  threshold, a charge heals them; several in a row if they keep dropping. A heal build never wants to
  be hit: a full bank is insurance, and diving a swarm with 5 charges is a real tactic.
- **Block charges** (Shield Block, Holy Shield): each charge fully blocks one melee hit.
- **Thorn charges** (Lightning Shield): each charge zaps the next enemy that hits you.
- Charges show as a stack count on the buff (the WoW buff frame already draws stacks).
- Augments work on the bank: "+2 cap"; "a heal on a full-health ally banks a charge on them";
  "spending your last charge casts Holy Nova".

### Damage for heal and tank builds (11c, agreed)

Two rules that need no item:

1. **Overflow:** while your heal bank is full, each further heal strikes the nearest enemy as Holy
   (or Nature) damage. Solo, a healer at full health is a slow damage build, and healing power is its
   damage stat. In co-op the bank is spent on friends and overflows less: the healer's trade. Holy
   Nova and Holy Shock already do both in WoW.
2. **Retaliation:** a hit you block, dodge or absorb deals damage back to the attacker, scaled from
   armor or block (Holy Shield, Thorns). Only defensive builds get it, and only for hits that did not
   hurt, so standing in the swarm with charges up is how a tank kills.

*Dropped:* every enemy having a fixed number of attacks. It helped every build equally (a glass
cannon could dive too), capped how dangerous a dense swarm can be, and read oddly.

### Knockback and death grip (11d)

- **Player spells:** Blast Wave knocks back slightly; its Concussive Blast augment pushes 4 yd.
  Knockback is a Control effect (a new **Knockback** tag): full on small enemies, half on elites,
  none on bosses.
- **On you:** only from elites and bosses, always with a tell (a cast bar, a circle on the ground),
  and a second of immunity after a push. Being knocked into the swarm is the danger.
- **Death grip** is the reverse: an area is marked, and after a few seconds every hero still inside
  is pulled to the caster.
- **In co-op** the client owns its hero's position, so a push or pull is an event the client applies
  to its own controller, as WoW's server sends a knock-back move to the client.

### Elite and boss mechanics (11e)

**The rule (agreed): every mechanic can be avoided by moving.** No mechanic forces damage on
someone, because a build of speed or control cannot heal or soak it and would die to it slowly.
Taking a hit is only ever a choice you make for a reward.

**Elite behaviours** break straight-line kiting, each a different way. Behaviour comes from the
creature, so the zone tells you what to expect:

| Behaviour | What it does | What it asks of you | Example |
|---|---|---|---|
| **Charge** | Winds up, dashes in a line | Step sideways, not away | Gnoll brute, Hogger |
| **Caster** | Stays at range and casts | Go to it | Defias wizard |
| **Slam** | Ground circle, then a knockback or stun | Leave the circle | Ogre, Thunder Clap |
| **Leap** | Jumps to you from range | Keep a control spell ready | Duskwood worgen |
| **Frenzy** | Faster below half health | Burst it, or control it at the end | Wolves |
| **Healer** | Heals enemies in a visible ring around itself | Pull the pack out of the ring, or bunch it on the healer for your area spells | Murloc oracle |
| **Summoner** | Adds appear around it | Fight close so area spells hit it and its fresh adds together | Defias conjurer |

Targeting is automatic, so "kill it first" is not a choice you make. Healers and summoners are
**single-target damage checks**: without enough single-target damage, healed and summoned enemies
outlast your area spells. Where you stand is your answer.

Crowd control on an elite has **diminishing returns**, as in WoW PvP: each stun, fear or freeze
within a short window lasts half as long, then it is immune for a while.

**Ground zones and marked areas** make elites and bosses feel like WoW fights:

- **Hellfire patches:** five small circles near you glow for about a second, then burn. Move through
  the gaps. Some zones could burn enemies too, so kiting trash through a boss's fire is a tactic.
- **By zone:** Gurubashi trolls drop poison clouds, Westfall Defias throw fire, Anubisath leave sand
  vortexes.
- **Death knight and ghoul** (a boss pair, perhaps Duskwood; check the model is in our data): the
  death knight marks an area and grips everyone inside to him; the ghoul's cleave punishes the
  heroes who land in one spot.

**Things to run to**, taken from WoW raids and Mythic+:

| Mechanic | From | How it works here |
|---|---|---|
| **Spores** | Loatheb (Naxxramas) | A spore spawns; walking into its cloud gives a big crit buff. A choice, not a must. |
| **Poisoned blood** | Hakkar (Zul'Gurub; fits Gurubashi) | Standing in the puddle poisons you lightly, but makes his Blood Siphon heal him less or hurt him. |
| **The dance** | Heigan (Naxxramas) | The arena splits into strips that erupt in turn; the safe strip keeps moving. |
| **Afflicted souls** | Mythic+ | Ghosts spawn and must be touched within a few seconds, or they hit the whole group. In co-op, players split up to reach them. |
| **Explosive orbs** | Mythic+ | Orbs appear and blow up unless destroyed. Your spells hit what is nearest, so you destroy them by walking up to them. |
| **Living Bomb, reworked** | Baron Geddon | An optional bomb on the ground. Walk over it to pick it up; a few seconds later it explodes around you, hitting enemies and the boss hard. You take a small share of your health, never a killing blow. Carry it into the pack or to the boss. |
| **Hold the circle** | Soak circles, reworked | A circle that damages the boss (or weakens it) while a hero stands in it. Standing in it costs nothing but your position. |

*Not doing:* Living Bomb as a mark you cannot refuse, and soak circles that hurt whoever stands in
them. Both force damage on someone, against the rule above.

**The boss bar (11f, agreed):** a bar across the top of the screen with the boss's name and health,
and one sentence under it explaining its mechanic, such as "Marks the ground; anyone inside is pulled
to him." A centre-screen warning (WoW's raid warning text, if the 1.12 interface has it) could come
just before a mechanic fires. Elites get a sentence on their target frame.

### Affixes (11g)

From Mythic+ and Project Ascension's Manastorm:

| Affix | What it does |
|---|---|
| **Vortex** | Pulls you and nearby enemies in when you get close. Drag a pack into it to bunch it up. |
| **Slowing field** | Slows everything inside, you included. |
| **Sanguine** | Dead enemies leave a pool that heals other enemies; pull the pack off it. |
| **Bolstering** | Each death buffs nearby enemies; don't leave a pack half-killed. |
| **Volcanic** | Fire erupts under you after a short warning. |
| **Storming** | Tornadoes drift across the area. |
| **Spiteful** | A ghost rises from a dead enemy and chases you for a few seconds. |
| **Explosive** | Orbs to walk up to and destroy (above). |

Where affixes come from is open:

- **Chosen before the run** for more reward, like a Mythic+ keystone or Hades' Heat.
- **Owned by certain bosses or elites**, as part of who they are.
- **Added in a run's later stages**, so the last minutes get harder in new ways, not just in numbers.

A likely mix: bosses keep their own fixed mechanics (learnable, explained on the boss bar); affixes
join in a run's later stages, and the difficulty ladder (9g) adds more of them, or lets you choose.

## Open questions for us

1. The talent web: its size, how points are earned, its first notables and keystones.
2. Dormant gear: what a piece costs to switch on, and whether order or rarity changes the price.
3. Items versus tomes: which carries the unique effects, and slots or not ([A](#a-items-and-tomes-in-a-run)).
4. Review the mage draft ([MAGE-DRAFT.md](MAGE-DRAFT.md)).
5. Charges: does every heal and block spell bank charges, or only some ([I](#i-surviving-solo-without-a-tank-or-healer))?
6. Affixes: chosen before the run, owned by bosses, added in later stages, or a mix?

Answered (2026-10-15):
- **Talent points and gear:** a setting, per class or shared, to test both.
- **Freeze:** the picked card is not replaced, unlimited freezes, unfreezing deals afresh.
- **Classless:** its own mode.
- **In-run slots:** 2 rings, 2 trinkets, 1 necklace.
- **Power budget:** 0% early, up to ~40% late.
- **Defensive button:** Q/E/F, 30 s–2 min, augmentable, no cast times for now.
- **Fusions:** not doing.
- **Augments:** the growth system: general augments at ranks 5 and 10, a capstone (1 of 3 fixed) at
  rank 15, max rank 15.
- **Evolutions:** skipped for now; test augments first.
- **Augment flow:** a bonus Augment screen with Pick / Reroll / Hold; holding
  keeps the spell at its rank until augmented; it pauses co-op like a level-up.
- **Talent web:** start with it.
- **Gear:** jewelry in the run; armor and weapons brought in, dormant, switched on with gold.
- **Movement:** no universal dash; the defensive button, items, tomes and maybe launch pads.
- **Upgrades:** duplicates and upgrade cards raise a tome or item by an amount scaled by rarity.
- **The Megabonk branch:** liked; merges into the main game once its remaining work is done.
- **Parked for later:** the talent web and dormant gear (all ideas kept here).
- **Survival:** every build needs an answer to the swarm, not a tank or healer; heal and block
  spells bank charges; heal and tank builds deal damage through overflow and retaliation.
- **Boss and elite mechanics:** every mechanic can be avoided by moving; Living Bomb becomes an
  optional pickup; soak circles become circles you hold to hurt the boss; a boss bar with a
  one-sentence explanation.
