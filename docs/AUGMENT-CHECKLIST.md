# Augment checklist

Every mage augment now has an automated check: it casts the spell with and without the augment in
an emptied arena and asserts what the description promises. The checks are in
`crates/together-sim/src/world/combat/augment_checks.rs` (new) and `world/combat.rs`'s tests
(older). Run them with:

```powershell
cargo test -p together-sim
```

## Bugs the checks found (fixed)

| Augment | What was wrong | Now |
|---|---|---|
| Quickened | A burn's last tick dealt a full tick however little was left, so a faster burn dealt **30% more** in total instead of the same damage sooner. | Same total, ends 30% sooner (it ticks more often). |
| Molten | The same last-tick rounding. | Double damage over twice as long, exactly. |
| Firestorm | The two extra Flamestrikes picked crowds at random *with replacement*, so both could land on the same crowd. | Two different crowds, each clear of the first strike. |
| Living Wave, Contagion | An enemy killed by its burn's **last** tick had the burn removed before its death was handled, so no Blast Wave burst and no jump. | The burn is still on it when it dies. |
| Critical Mass | Set Fire crits to 250%, which would erase the new Tome of Ferocity's crit damage. | Adds +50% on top. |

## Try them in-game

Every spell's capstones can be tried with a test loadout (names as on the cards, any case). From
`D:\Games\azeroth-together` in PowerShell:

```powershell
$env:TOGETHER_CLASS="mage"; $env:TOGETHER_LOADOUT="frostbolt@10+glacial spike"; cargo run --release -p azeroth-together
```

Then clear it again with `Remove-Item Env:TOGETHER_LOADOUT`. Several spells at once:
`"frostbolt@10+glacial spike,fireball@10+hot streak,blizzard@10+frozen orb"`. A spell can carry
general augments too: `"frost_nova@15+ring+frostbite+lasting frost"`.

### Capstones (rank 10), what to look for

| Spell | Capstone | Look for |
|---|---|---|
| frostbolt | Glacial Spike | Every 4th bolt freezes its target. |
| frostbolt | Shatterbolt | Frozen enemies always take crits that splash. |
| frostbolt | Winter's Chill | Targets take more crits the more bolts land. |
| fireball | Hot Streak | Two Fire crits in a row fire a Pyroblast. |
| fireball | Critical Mass | The burst crits whenever the hit does. |
| fireball | Living Bomb | The target explodes after 3 s, chaining smaller bombs. |
| arcane_missiles | Arcane Charge | Arcane damage climbs; Arcane Explosion spends the charges. |
| arcane_missiles | Missile Storm | Every 10 s, a missile at every enemy in range. |
| arcane_missiles | Channelled Focus | Standing still, it fires twice as often. |
| frost_nova | Frostbite | Frozen enemies that die shatter on their neighbours. |
| frost_nova | Cold Snap | Hitting 10+ resets your other Frost cooldowns. |
| frost_nova | Frozen Core | Dropping below 30% health casts Frost Nova and halves damage taken. |
| arcane_explosion | Arcane Potency | Hitting 15+ makes the next Arcane spell triple. |
| arcane_explosion | Blink Detonation | Blink (F) explodes where you left and where you land. |
| arcane_explosion | Implosion | Enemies within 12 yd are pulled in first. |
| blizzard | Final Gale | Lasts longer and ends in a Frost Nova. |
| blizzard | Whiteout | Enemies inside take +20% damage. |
| blizzard | Frozen Orb | The storm rolls away the way you face. |
| flamestrike | Firestorm | Two more Flamestrikes on other crowds. |
| flamestrike | Meteor | Triple damage, slower, stuns. |
| flamestrike | Phoenix Ground | Standing in the fire heals you. |
| fire_blast | Impact | Stuns and spreads the target's burns. |
| fire_blast | Inferno | A kill casts it again at the next enemy. |
| fire_blast | Ignition | Always crits Burning enemies. |
| cone_of_cold | Frozen Cone | Freezes instead of chilling. |
| cone_of_cold | Shard Burst | Double damage on Frozen enemies. |
| cone_of_cold | Wake of Winter | Blink leaves a cone behind you. |
| blast_wave | Blazing Speed | Each enemy hit makes you faster. |
| blast_wave | Living Wave | Burning enemies that die send out small Blast Waves. |
| blast_wave | Dragon's Breath | A stunning cone ahead instead of a ring. |
| pyroblast | Presence of Mind | Every 3rd cast fires a second Pyroblast. |
| pyroblast | Kindling | Fire crits shorten Pyroblast's cooldown. |
| pyroblast | Combustion | Cashes in the target's burns at once, doubled. |
| ice_barrier | Shattering Barrier | The shield's end deals 3× what it absorbed and casts Frost Nova. |
| ice_barrier | Ice Floes | Frost cooldowns run faster while it holds. |
| ice_barrier | Frost Armor | Enemies that hit the shield are chilled and hurt. |

### General augments (ranks 5 and 15)

All 35 are checked. The number ones (Spellfire, Spellfrost, Wide, Concentrated, Heavy, Hasted,
Multishot, Lingering) are checked on the spell's numbers, which the tooltip shows too; the rest
by what happens: Frostfire (Fire chills, Frost burns), Piercing (hits the enemy behind),
Returning (hits twice), Echo, Ring (misses the middle, reaches 12 yd), the cone ones (Reversed,
Four Winds, Full Circle), Mobile (the ground follows you), Targeted (lands on a far crowd),
Contagion, Quickened, Molten, Precise, Executioner (double on enemies below 30%), Ignite, Deep
Chill (chills sometimes freeze), Cold Bite, Lasting Frost, Permafrost, Shatter, Brute Force,
Concussive, Fortified, Reflective, Twin Charges (casts twice in a row) and Twin.
