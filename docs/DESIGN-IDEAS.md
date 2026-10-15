# Azeroth Together: design ideas and decisions

A living list of what we might add, where the idea comes from, and where it stands. Nothing here is
built unless its status says **Done**. Update the table as decisions are made; the sections below it
hold the thinking behind the open topics.

**Status:** ✅ Done · 🧪 Prototyping on the Megabonk branch · 🟢 Greenlit (we want it; design may
still need detail) · 💬 Discussing · 💡 New idea (needs a yes/no) · ⛔ Not doing

**The Megabonk branch** (`megabonk-systems`, private repo `OliverChristensen93/azeroth-together-megabonk`,
built by the "Build Megabonk-style shrines and events" session) is a playable experiment. What it
builds is marked 🧪; we take from it what works when the systems are designed for the main game.
As of 2026-10-15 it has gold, a hero stat block (damage, cooldown, area, speed, health, regen,
armor, pickup range, luck, XP, gold, crit, extra projectiles), tome and pact cards, elites, and
seven interactables (Titan Shrine, Golden Titan Shrine, Cursed Altar, Shrine of Greed, Treasure
Chest, Challenge Idol, Moonwell); items and a stage boss are in progress.

## Decision table

| # | Feature | Inspiration | Status | Notes |
|---|---|---|---|---|
| 0a | Co-op for up to 4, LAN and internet invite codes | — | ✅ Done | Host-authoritative; invite codes via iroh. |
| 0b | Revive a fallen friend by standing beside them | — | ✅ Done | 3 s, half health, 2 s grace. |
| 0c | Rarity-rolled spell cards, reroll, skip | Azeroth Survivors | ✅ Done | |
| 0d | Damage meter, WoW party frames | Details!/Recount | ✅ Done | |
| 1a | **Tomes**: stat upgrades that take no slot | Megabonk | 🟢 Greenlit · 🧪 | Damage, cooldown, area, speed, health, regen, armor, pickup range, luck, XP and gold gain, as cards beside spell cards. Built on the Megabonk branch. |
| 1b | **In-run items** in 5 slots: 2 rings, 2 trinkets, 1 necklace | Azeroth Survivors, Megabonk | 🟢 Greenlit · 🧪 | Drop from chests, equipped in the run. The necklace is the strongest slot and can carry Unique effects that augment or link spells (2e). Being built on the Megabonk branch (its slot rules may differ). See [A](#a-items-and-tomes-in-a-run). |
| 2a | Evolutions: max-rank spell + a specific item becomes a new spell | Vampire Survivors | ⛔ Skipped for now | Fixed recipes turn a run into a hunt for the partner piece. We test augments (2c) first and revisit. |
| 2b | Fusions: two spells merge into one | — | ⛔ Not doing | Fixed recipes force a hunt; any-with-any has no good meaning; and fusing *removes* half of each spell (a trap's summon, a damage spell's damage), stranding the modifiers built for it. |
| 2c | **Augments**: choose a modifier for a spell at ranks 7 and 14 (max rank 14) | Hades, Azeroth Survivors items | 🟢 Greenlit | The core way spells grow. Pools: ~4 spell-specific per spell plus a shared tag library (agreed). How the pick is offered: 💬 see [C](#c-augments). |
| 2d | Grimoire: a codex of augments and Uniques you have found, the rest as hints | VS collection | 💡 New idea | Knowledge as progression between runs. |
| 2e | **Unique items that augment or link spells** (e.g. "When Ice Barrier breaks, cast Frost Nova") | Azeroth Survivors | 🟢 Greenlit | Mostly necklaces. Triggered casts use your rank and augments and ignore cooldown. |
| 2f | **Spell design rules**: about 12 spells per class, each reworked for the genre; single-target spells prioritize bosses and elites | Azeroth Survivors | 🟢 Greenlit | Kidney Shot just damages and stuns. Single-target and area spells both earn a place. We have 68 spells today (~7.5 per class). |
| 2g | Talents that **add augments to the pool** instead of raw power | — | 💡 New idea | The cleanest "widen, don't narrow" progression. See [C](#c-augments). |
| 3 | Gold, chests (price rises per open), a goblin merchant | Megabonk, Azeroth Survivors | 🟢 Greenlit · 🧪 | Gold and Treasure Chests built on the Megabonk branch; the merchant is not. Gold also switches on dormant gear (9f). |
| 4 | Elites (drop chests) and a stage boss on a timer | Azeroth Survivors, VS, Megabonk | 🟢 Greenlit · 🧪 | Elites built on the Megabonk branch, the boss in progress there. Bosses are also where meta equipment drops (9a). |
| 5 | Risk for reward: greed shrine, cursed altar pacts (boon + bane), cursed tome, boss curse | Megabonk | 🟢 Greenlit · 🧪 | Shrine of Greed and Cursed Altar pacts built on the Megabonk branch; cursed tome and boss curse are not. |
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
| 10b | Movement feel: dash charges, double jump, slow fall, momentum, jump pads | Megabonk | 💬 Discussing | See [F](#f-the-defensive-button-and-moving-well). |
| 10c | An **ultimate** beside the defensive button, possibly with a stand-still cast time | Megabonk campfire | 💬 Later | Parked until the defensive button exists. |
| 12 | Co-op synergies across players (one player's root enables another's Shatter) | — | 💡 New idea | Falls out of tags for free. See [H](#h-co-op-only-ideas). |
| — | Talent trees that lock a run into one spec before it starts | Azeroth Survivors | ⛔ Not doing | Takes away in-run choice: frost talents make every non-frost spell a trap pick. |
| — | A permanent stat shop as the main progression (+5% damage forever …) | VS | 💬 Discussing | Fine as a small extra; as the core it is a grind with no decisions. |

## A. Items and tomes in a run

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
| Effect | Direct, Damage over time, Slow, Root, Freeze, Stun, Fear, Heal |

Examples:
- **Frostbolt**: Frost · Projectile · Direct · Slow
- **Blast Wave**: Fire · Area · Direct · Slow
- **Chain Lightning**: Nature · Chain · Direct
- **Immolate**: Fire · Direct · Damage over time
- **Ice Barrier**: Frost · Shield
- **Frost Nova**: Frost · Area · Freeze

A conversion augment *changes tags*: Fireball augmented to Frostfire is Fire and Frost, so both
schools' bonuses apply. That is how an off-school spell joins a build without any talent deciding it
beforehand.

## C. Augments

Spells grow by **augments** chosen at **rank 7** and **rank 14**; 14 is max rank.

**Pacing.** A 15-minute run gives maybe 50–60 picks shared between 6 spells, tomes and items. Maxing
all 6 spells would take 78 rank-ups, so most spells reach rank 7 (their first augment) and two or
three committed spells reach 14. Rank 14 is a commitment, not a given. Tune when playable.

**How the milestone works (proposed, 💬).** A spell that reaches rank 7 (or 14) is *ready to
augment*, and an **Augment screen** opens right away with 3 augments for it (rarity-rolled: rarer
augments are stronger versions or rarer kinds). It is a bonus screen: it does not use a level-up.
On it you can:

- **Pick** one.
- **Reroll** for 3 new options, using the run's rerolls.
- **Hold**: take nothing now; the Augment screen comes back with 3 fresh options after your next
  level-up, again as a bonus.

The price of holding: the spell **stays at rank 7** (no further rank cards for it) and has no
augment while you wait. So you can hunt for Frostfire when Spellfire shows up, keep levelling
everything else, and the cost falls only on the spell you are being picky about. A spell ends a run
with at most two augments.

- *The alternative considered:* the augment hand replaces the next level-up, and skipping costs that
  level-up. Harsher, and hunting a conversion would mean giving up spell and tome picks.
- **Synergy weighting:** the augment hand leans toward your build's tags (Frost spells owned →
  Frostfire more likely than Spellfire), never guaranteed; talents can lean it further.
- **Co-op:** level-ups pause the run until everyone has picked, and augment screens add pauses. Try
  the pause first; if it interrupts too much, let augment screens be picked with 1/2/3 while moving.

**The two tiers do different jobs:**

| Milestone | Job | Kinds |
|---|---|---|
| Rank 7 | Shape the spell to the build you are forming | Conversion, shape, trade-off |
| Rank 14 | The capstone that defines the build | Trigger, scaling, rule-breaking spell-specific effects |

Kinds, with examples:

| Kind | Examples |
|---|---|
| Conversion | Fireball deals Frostfire damage; Shadow Bolt deals Shadowflame |
| Shape | +2 projectiles; Cone of Cold becomes a ring; single target chains to 3 |
| Trade-off | +80% damage, no longer freezes; +100% damage, +50% cooldown |
| Trigger | Ice Barrier breaking casts Frost Nova; two Fire crits in a row cast Pyroblast (Hot Streak) |
| Scaling | gains crit chance from your move speed; damage grows with your max health |

**How many augments each spell needs: not six of its own.** With about 12 spells per class and 9
classes, six hand-made augments each would be ~650 augments to design and balance. Instead, each
milestone's pool is built from two sources:

1. **Spell-specific augments**: about 2 per milestone, so **4 per spell**. These are the ones that
   make a spell special: Ice Barrier's break casting Frost Nova; Chain Lightning arcing back to its
   first target; Pyroblast's Hot Streak.
2. **Shared tag augments**: a library of perhaps 30–40, each open to any spell with the right tags.
   - Every **Projectile** spell can roll "+2 projectiles, −25% damage each".
   - Every **Damage over time** spell can roll "spreads to a nearby enemy when the target dies".
   - Every elemental spell can roll the **conversions** to the other schools.
   - Every **Area** spell can roll "+40% area, −20% damage".

A rank-7 offer of 3 cards then draws from roughly 2 specific plus 6–10 shared options, and a rank-14
offer likewise, so a spell plays differently from run to run while each class needs only ~48
specific augments. Two guard rails:

- A spell takes **one conversion at most**, so it cannot hop schools twice.
- **Talents can add augments to the pool** (2g): "Frostfire conversions can appear on any Fire or
  Frost spell". That widens options rather than adding raw power, which is exactly the kind of
  progression between runs we want.

**Rule for triggered casts:** a spell cast by a trigger is the real spell, with your rank, augments
and modifiers, and it ignores its own cooldown. If you do not own it, it casts at rank 1. This makes
"+100% damage, +50% cooldown" Frost Nova behind a low-cooldown Ice Barrier a real build.

Unique items (2e) can add augments or triggers too, which is where builds get wild.

**Roll-out:** one class first (the mage: 12 spells, ~48 specific augments, the first ~20 shared ones)
to prove the system before writing the rest.

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
3. At rank 7 you augment it to Frostfire.
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

## Open questions for us

1. The augment flow: the bonus Augment screen with Pick / Reroll / Hold, as in [C](#c-augments)?
2. Co-op: should augment screens pause the run like level-ups?
3. The talent web: its size, how points are earned, its first notables and keystones.
4. Dormant gear: what a piece costs to switch on, and whether order or rarity changes the price.
5. Movement feel (10b): which of dash charges, double jump, glide, momentum and jump pads.
6. What to take from the Megabonk branch once it is playable.

Answered (2026-10-15):
- **Talent points and gear:** a setting, per class or shared, to test both.
- **Freeze:** the picked card is not replaced, unlimited freezes, unfreezing deals afresh.
- **Classless:** its own mode.
- **In-run slots:** 2 rings, 2 trinkets, 1 necklace.
- **Power budget:** 0% early, up to ~40% late.
- **Defensive button:** Q/E/F, 30 s–2 min, augmentable, no cast times for now.
- **Fusions:** not doing.
- **Augments:** the growth system, at ranks 7 and 14, max rank 14; pools of ~4 spell-specific
  augments per spell plus a shared tag library.
- **Evolutions:** skipped for now; test augments first.
- **Talent web:** start with it.
- **Gear:** jewelry in the run; armor and weapons brought in, dormant, switched on with gold.
