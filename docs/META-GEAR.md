# Meta equipment: gear brought into a run

What is built for decision rows 9a, 9f, 9g (item level only), 9h and 9i of
[DESIGN-IDEAS.md](DESIGN-IDEAS.md) (section D, "Equipment (agreed)"), and every decision taken
on the way, for review. Decisions are marked **Decision**; the numbers are first guesses to tune
by playing.

## In one paragraph

Bosses (half the time) and elites (rarely) drop a piece of armour or a weapon for **every**
hero, each rolling their own. Pieces go straight into a stash kept on disk. At the menu's
**Equipment** screen you wear up to eleven pieces (head, shoulders, back, chest, wrists, hands,
waist, legs, feet, main hand, off hand; a two-hander fills both hands). Every run starts with
them **dormant**: you see them in a gear bar under your tomes and items, dimmed, each with its
gold price. Spend gold on one (click it, or press **G** for the best one you can pay for) and it
switches on: its stats join yours, a unique starts changing its spell, and your hero visibly
puts it on. Gold also buys chests, so every coin is "a chest, or a piece I know".

## Where it lives

| What | File |
|---|---|
| A piece: slots, rarity, item level, stats, names, prices, uniques, drop rolls | `crates/together-sim/src/gear.rs` |
| The stash: lockers per class or shared, equip and take off, save as RON | `crates/together-sim/src/armory.rs` |
| Gear in a run: bring, switch on, drops, Atiesh's pull | `crates/together-sim/src/world/gear.rs` |
| Hooks in the world (one line each) | `world.rs` (`Hero::gear`, `PlayerInput::gear_on`, `Event::GearFound`/`GearOn`, the drop in `reap_enemies`), `world/combat.rs` (uniques in `spell_spec`, the pull in `nova`) |
| Zone item levels | `content/zones.ron` (`item_level`), `ZoneDef::item_level` |
| The stash file and test hooks | `crates/benilla-app/src/together/armory.rs` |
| Looks (display ids) and dressing the hero | `crates/benilla-app/src/together/gear_look.rs` |
| The menu's Equipment screen | `crates/benilla-app/src/together/gear_menu.rs` |
| The run's gear bar and the G key | `crates/benilla-app/src/together/gear_run.rs` |
| WoW-style item tooltips | `crates/benilla-app/src/together/gear_tip.rs` |
| Co-op: gear in the pick | `crates/together-net/src/lib.rs` (`HeroPick::gear`, `PROTOCOL` 7) |

## Pieces

A piece stores only: slot, rarity, item level, armour type (cloth, leather, mail, plate), a
seed, and for a legendary its unique. Its stat lines, name and look all follow from those.

**Decision: stats are derived from the seed, not stored.** A piece is small on the wire and in
the save file, and a rebalance reaches pieces already found. The price: changing the roll code
re-rolls every non-unique piece's lines (their slot, rarity and level stay).

**Stats** use the existing `Stat` block, in tome units so the game's own balance (a tome of
each stat is worth the same) carries over:

- A piece's budget = its slot's share × its rarity's share × its item level's share.
  - Slot shares: main hand 2, chest and legs 1.2, head, shoulders and off hand 1, hands, waist
    and feet 0.8, back and wrists 0.6 (a two-hander counts as both hands, 3). Eleven in all.
  - Rarity shares: Common 0.45, Uncommon 0.6, Rare 0.75, Epic 0.9, Legendary 1.0.
  - Item level share: 0.25 + 0.75 × level / 65 (Elwynn's 10 is 0.37, Gates of Ahn'Qiraj's 65 is
    1.0).
- Lines: Common 1, Uncommon and Rare 2, Epic and Legendary 3. The first line takes the biggest
  share. Each slot rolls from its own pool (weapons lean to damage, crit and cooldown; chest and
  legs to health and armour; feet to move speed; waist to pickup, gold and luck, and so on).
- Shown with tenths: "+2.4% damage", "+0.15 health a second".

**Power budget (9i).** A full set of eleven legendary-strength pieces at item level 65 adds 2.75
common tomes' worth, about +22% damage were it all damage. A usual set of rares and epics from
the middle zones is about half that. Talents get the other half of the agreed 40%. Uniques add
their spell's change on top, which pays off only when the run offers that spell. A test
(`a_full_best_set_stays_well_inside_the_power_budget`) holds the ceiling.

**Names** read like WoW items: material by item level (Linen, Woolen, Silk, Mageweave,
Runecloth for cloth, and so on), the slot's noun for its armour ("Hood", "Spaulders",
"Sabatons"), a prefix for rare ("Runed", "Ornate") and epic ("Ancient", "Dread"), and from
uncommon up a suffix for the main stat: "of the Tiger" (damage), "of the Owl" (cooldown), "of the
Bear" (health), "of the Cheetah" (speed), "of the Eagle" (crit) and so on.

**Weapons**: the main hand is a staff or greatsword (two-handed), or a sword, mace, axe or
dagger; the off hand an orb or tome, a dagger or sword, or a shield, by armour type and seed.

## Rarity and uniques

Common to Legendary in WoW's colours. **Decision: every legendary is a unique**, as WoW's orange
items are (Thunderfury, Atiesh). A dropped legendary is one of the uniques, three times likelier
one for a spell in the finder's class pool.

| Unique | Slot | Spell | Effect |
|---|---|---|---|
| Thunderfury, Blessed Blade of the Windseeker | Main hand (sword) | Chain Lightning | Jumps to 2 more enemies and Chills everything it strikes. |
| Atiesh, Greatstaff of the Guardian | Two-hand (staff) | Arcane Explosion | Pulls enemies within 16 yards 4 yards toward you before it bursts (elites half as far, bosses not), and reaches 20% further. |
| Robe of the Archmage | Chest | Arcane Missiles | 2 more missiles at each target. |
| Mantle of the Blackwing Cabal | Shoulders | Fireball | Burst 50% wider; leaves enemies Burning for 30% of the hit. |
| Glacial Cloak | Back | Frost Nova | Reaches 30% further; Freezes 1 second longer. |
| Frostfire Sandals | Feet | Frostbolt | Its Chill has a 20% chance to Freeze (fits the benchmark build's "slows can freeze"). |
| Gloves of the Hypnotic Flame | Hands | Flamestrike | Burning ground lasts 3 seconds longer; Flamestrike comes 20% sooner. |
| Ashkandi, Greatsword of the Brotherhood | Two-hand (sword) | Cleave | Strikes 2 more enemies, reaches 30% further. |
| Nemesis Skullcap | Head | Corruption | Afflicts 2 more enemies. |

Six are the mage's (the finished class), three show the idea for others. Each also carries
legendary-strength stats.

**Atiesh and Implosion.** The mage's Implosion capstone already pulls (it snaps everything within
12 yards to the caster). Atiesh's pull is gentler and comes on every cast without the capstone;
with both, Implosion's snap happens and Atiesh only adds its reach. Review whether that overlap is
fine or Atiesh should do something else.

## Item level (9g, as far as it goes)

`item_level` per zone in `content/zones.ron`: Elwynn 10, Westfall 20, Duskwood 30, Gurubashi
Arena 45, Gates of Ahn'Qiraj 62. A boss's pieces are 3 levels higher. `gear::item_level` takes a
`torment` argument (10 levels a step) for the difficulty ladder when it is built; nothing passes
one yet.

## Drops

- **Bosses: 50% per hero**, rarity in 1000: Common 250, Uncommon 350, Rare 250, Epic 120,
  Legendary 30.
- **Elites: 1.2% per hero**, rarity: Common 450, Uncommon 350, Rare 150, Epic 45, Legendary 5.
  Elites are camp guards, Challenge Idol packs and random spawns from 1:30; a 15-minute run kills
  very roughly a hundred, so about one elite drop a run.
- Luck raises the rarer tiers as it does for cards.
- **Decision: personal loot.** Every hero rolls their own on every boss and elite kill, fallen or
  not, so friends never compete for a drop.
- **Decision: a piece is armour of the finder's class** (cloth for mage, priest and warlock;
  leather for rogue and druid; mail for hunter and shaman; plate for warrior and paladin). The
  armour type changes only the look and the name; any class may wear any piece.
- Found pieces wait in the hero's `gear.found` for the rest of the run and are written to the
  stash **as they drop** (not only at the end), so a crash or quitting loses nothing. They can be
  worn from the next run on. A floating "Loot: <name>" in its quality's colour announces each.

## The stash and the menu screen

`benilla-config/together-armory.ron` (the shared settings folder, so every worktree sees one
stash). The **Equipment** button sits bottom left under the class tower, with "n new" beside it
when there are unseen finds. The screen opens over the zone list, so the hero stays in view and
wears every chosen piece at once:

- the worn pieces in two columns plus a weapon row, as on WoW's character sheet, each with its
  name in its quality's colour;
- "All switched on: ..." with what the worn set adds up to, and the uniques worn;
- the stash, 40 icons a page, by slot and then rarest first, new ones marked "New";
- every icon has a WoW item tooltip: name, item level, slot and type ("Two-Hand / Staff",
  "Head / Cloth"), stat lines, "Equip: ..." in green for a unique, its flavour text in yellow,
  and what a click does (and what it replaces).

Click a stash piece to wear it (what it pushes out, including an off hand under a new two-hander
or a two-hander under a new off hand, goes back to the stash); click a worn piece to take it off.

**Decision: the stash holds 120 pieces.** Past that, the weakest non-unique pieces are dropped
to make room. There is no salvage or delete button yet.

### Per class or shared (9h)

A button on the screen switches between the two; **per class is the default**.

- Per class: each class has its own locker, worn set and finds.
- Shared: one locker for every class; finds made while shared go into it.

**Decision: the two are separate lockers, both kept.** Switching changes which one is used,
never what is in them, so nothing is lost either way (pieces found per class do not show in the
shared locker and the other way round). Merge them, or pick one, once you know which plays better.

## Switching on with gold (9f)

- **Prices** (rounded to 5): rarity base (Common 15, Uncommon 25, Rare 40, Epic 60,
  Legendary 90) × (0.75 + item level / 130), **plus 10 for every piece already on this run**. An
  epic at item level 30 costs 60 first, 70 if one piece is on already. Order never changes what a
  whole set costs.
- **Decision: gear prices do not touch chest prices** and chests do not touch gear prices; they
  only compete for the same gold.
- **The gear bar** sits under the tomes and items strip: one icon per brought piece, its border in
  its quality's colour, dimmed with its price in the corner (gold when you can pay, grey when not)
  until it is on. Beside it: "G: switch on <name> (<price> gold)" when you can pay for one, else
  what the next costs. Click an icon, or press **G** for the best piece you can pay for (highest
  rarity, then the most power). It works while a card screen is up, as card picks do.
- The hero's stats, health and speed update at once; a unique's spell changes from its next
  cast. Power Word: Fortitude's glow plays on the hero, with "<name> awakens".
- **Decision: switched-on gear stays on for the run.** There is no switching off.

## Looking geared (it's WoW)

A run's hero starts in its starting outfit and puts each piece on as it switches on; the menu's
hero wears everything it will bring. Looks come from 1.12 displays that the trimmed game data
has complete (model and every texture, checked against the MPQs):

- Common and Uncommon: plain armour of the piece's type (two looks a slot).
- Rare: the dungeon set (tier 0) of a class wearing that armour, picked by the seed: Magister's,
  Devout or Dreadmist for cloth, and so on.
- Epic: raid tier 1 or 2 (Arcanist, Netherwind, Nemesis, Bloodfang, Dragonstalker, Wrath...).
- Uniques: their own looks: Thunderfury's blue blade (its real 1.12 display), Atiesh (the mage's),
  Ashkandi, Netherwind robe for the Robe of the Archmage, Nemesis spaulders for the Mantle,
  tier 3 Frostfire for the Sandals, Arcanist gloves, the Nemesis Skullcap itself.
- Weapons: four quality steps per kind, from plain to raid weapons (Zul'Gurub, Ahn'Qiraj).

**Known gap: capes do not show.** The trimmed data has no cape textures, so a back piece changes
nothing on the model (its icon and tooltip work).

## Co-op

Each player's worn set travels in their `HeroPick` (the Hello and Pick messages, and the lobby
roster), so the host has every guest's gear when the run starts; `together_net::PROTOCOL` went
from 6 to 7. The host's world calls `World::equip` for every roster player, which keeps only
valid pieces (a unique in its own slot, sane item levels, one per slot, no off hand beside a
two-hander). A guest's G or click travels as `PlayerInput::gear_on`; the host checks the price and
the purse. Snapshots carry each hero's gear, so every client dresses every hero, and a guest's
finds reach the guest's own stash through its mirrored hero.

## Test switches

- `TOGETHER_GEAR_SEED=1`: start from a test stash (one of every unique, a piece of every rarity
  in every slot; the mage wears seven uniques and three epics) kept in
  `together-armory-test.ron`, never touching the real stash.
- `TOGETHER_GEAR_SCREEN=1`: the Equipment screen opens at the menu.
- `TOGETHER_GEAR_ON=1`: every brought piece is switched on as the run starts.
- `TOGETHER_AUTOPLAY=1` also switches pieces on as soon as it can pay.

## Open for review

1. The numbers: drop chances, prices, the 2.75-tome ceiling, stash size.
2. Atiesh against Implosion (above).
3. Per class and shared as separate lockers, or one merged stash.
4. Whether the armour type should follow the wearer (a mage always looks in cloth) instead of the
   finder.
5. A way to salvage or delete pieces, and to compare a stash piece with the worn one side by side.
6. Rings, trinkets and the necklace stay a separate, parked in-run feature (1b); none of this
   touches them.
