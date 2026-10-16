# Nightfall Sub-project 5a (The Run Loop) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A whole extraction run: heroes go into Duskwood with what they chose to bring (or the free kit), search chests, crates, graves and bodies for items with WoW looks, carry them in a 16-slot backpack widened by bags and slowed and made louder by weight, get out by standing 10 seconds in an exit's ring, or die and lose what `death_loss` says (a body friends can loot, and in the default mode a second chance at it next raid); an end screen shows each hero's outcome, and a stash with gold, saved on the player's own machine, carries what came out into the next raid.

**Architecture:** `nightfall-sim` gains its items and loot as content (`items.rs`, `content/nightfall/items.ron` and `loot.ron`, exits in `map.ron`, `run:` rules), a pure `Inventory` (`inventory.rs`), a `raid` module (what each hero carries and does with it, containers and their tables, exits, deaths, outcomes, the `RaidRoll` seam sub-project 6 fills) and a pure `Profile` (`profile.rs`: the stash, gold, the loadout, the free kit, settling a raid, a raid that never ended). Weight reaches the senses' pace and footsteps, gear the fight's damage and health, and food and water become items Eat and Drink uses. On the wire a player's `HeroPick` carries their loadout (the host trusts it), each player's snapshot carries their own backpack and the loot near them, and the host tells each player their outcome. The client keeps the profile in `benilla-config/nightfall/stash.ron` (versioned, with a backup, never lost silently), shows it in the menu, and draws the raid's containers, backpack, loot, exits, extraction and end screen with our own panels.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content and the stash file), postcard (wire).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md`: §8.1 (exits, the 10 s ring, the run's end, the end screen), §8.2 (items, backpack, bags, weight, `death_loss`, bodies, `safe_slots`, containers), §8.3 (stash, gold, the free kit, co-op trust), §11 (`death_loss`, `safe_slots`, `weight_*`, `cart_opens_at`), §1.1's softer losses. **Sub-project 5 is split in two** (it does not fit 16 tasks): this plan (5a) is the run loop with the menu kept; `docs/superpowers/plans/2026-10-17-nightfall-inn.md` (5b) is the inn hub, contracts, stations, the companion and the spike on WoW's windows, built on this one. Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-run-loop-rulings.md`. This plan starts after every task of the foundation, world-knowledge, senses and combat plans is done, and edits files those plans wrote; every name it consumes from them is the one those plans give.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call. Other agents commit in this worktree: stage only the files a task names, never `git add -A`.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`. This machine runs out of memory on wide builds: build `benilla-app` and `azeroth-together` with `-j 2` (`-j 1` if a build is killed).
- The worktree has no `WoW/Data`: every game launch sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data` and `BENILLA_HOME=target/nf-home` (the test home: its `nightfall/stash.ron` is a test stash, never the player's). The exe does not exit after its shots: start it in the background, wait for the last shot file, then `kill` its PID.
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats` (spec §3.1). Inventory, weight, containers, extraction, `death_loss`, outcomes and the profile are pure sim logic with unit tests. Reading and writing the stash file is the client's.
- Every number is content: `content/nightfall/rules.ron` under `run:` (with §11's values as defaults), `items.ron`, `loot.ron`, the exits in `map.ron`. `death_loss` defaults to `everything_one_recovery` (§11).
- Co-op (spec §8.3): each client keeps its own stash; the host trusts the loadout a client brings (made safe for the content, never refused); the host tells each player their outcome; a raid a player never heard the end of counts as missing for them.
- No engine crate changes. New files are preferred; edits to files earlier plans wrote are spelled out.
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Nothing Blizzard-derived goes into `content/` or a commit; content names WoW ids and icon names only.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **An act lost, doubled or replayed.** Inputs replace one another on the host between steps; an act (take, wear, drop, use, open) rides in every input until the host says it took it. Expect each act taken exactly once, the next one only after, and a new run starting from nothing. Pinned in Task 3 (`an_act_is_taken_once_however_often_it_is_sent`) and Task 8 (`acts_go_one_at_a_time_and_only_once_taken`).
2. **Items made or lost between a body, a pack and an outcome** (a death, a friend looting the body, the run ending, the remains reclaimed). Expect every stack in exactly one place at every moment, the outcome's recovery holding what is left on the body and nothing else. Pinned in Task 6 (`nothing_is_made_or_lost_between_the_body_the_packs_and_the_outcomes`, `a_friend_loots_a_fallen_heros_body`).
3. **A stash file that is corrupt, cut short, old or from a newer build.** Expect never a silent loss: the backup is tried, a broken file is kept beside it, a newer one is left untouched and the session does not overwrite it, and every case says so on the menu. Pinned in Task 7 (`a_newer_profile_is_refused_not_misread`, `an_old_file_without_a_version_loads`) and Task 9 (`a_corrupt_file_falls_back_to_the_backup_and_is_kept`, `a_newer_file_is_never_overwritten`, `saving_keeps_the_last_good_file_as_the_backup`).
4. **A bag taken off, or a pack resized, with things in it** (wearing a smaller bag, a found bag lost at death, a loadout from an old file with more than fits). Expect the move refused, or what does not fit returned to the caller, never dropped on the floor of the code. Pinned in Task 2 (`a_bag_comes_off_only_when_its_things_fit`, `sanitize_returns_what_has_no_room`) and Task 7 (`unknown_or_overflowing_things_go_back_to_the_stash`).
5. **The extract count at its edges** (a blow at second 9, a late exit before it opens, a downed hero in the ring, the outcome sent once). Expect the count to start over on a blow, nothing before `cart_opens_at`, no count while downed, and each outcome handed out exactly once. Pinned in Task 6 (`a_blow_starts_the_count_over`, `the_cart_opens_late`, `the_downed_do_not_extract`, `each_outcome_is_taken_once`).

**Not in this plan** (see the rulings file): the inn, its NPCs, contracts, stations, the companion and the spike on WoW's own windows (sub-project 5b); rolling which exits open, the weather and the signature monsters, and risk-zone tuning (sub-project 6, which fills `RaidRoll`); rivals and their bags (sub-project 7); durability and repair; WoW's own bag, loot and character windows (5b's spike says whether they can be fed).

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/items.rs` (new) | `ItemDef` and its parts (`ItemKind`, `EquipSlot`, `Look`, `Bonus`, `ItemUse`, `LightDef`); container kinds, loot tables, placed containers, bodies; their checks |
| `crates/nightfall-sim/src/content.rs` | `RunRules`, `DeathLoss`, `ExitDef`; classes' free kits; `items.ron` and `loot.ron` loaded |
| `crates/nightfall-sim/src/inventory.rs` (new) | `Inventory`, `Stack`, `Place`, `InvError`: slots, stacks, bags, gear, weight, death, sanitizing |
| `crates/nightfall-sim/src/raid/mod.rs` (new) | `Raid` (host state), `Carry`, `Act`, `ActPress`, `Holder`, `LootView` and its marks, `Loadout`, `Recovery`, `HeroOutcome`, `Fate`, `RaidRoll`, `ExitPlan`, `burden`; the world's accessors |
| `crates/nightfall-sim/src/raid/acts.rs` (new) | Acts taken once; wearing, moving, using, dropping; the carry refreshed |
| `crates/nightfall-sim/src/raid/containers.rs` (new) | Placed containers rolled from their tables, searching, bodies, piles, taking and putting |
| `crates/nightfall-sim/src/raid/extract.rs` (new) | Exits and extracting, deaths and bodies, the raid's end, outcomes, abandoning |
| `crates/nightfall-sim/src/raid/tests.rs` (new) | The raid in whole worlds on hand-made maps |
| `crates/nightfall-sim/src/profile.rs` (new) | `Profile`: stash, gold, loadout, the free kit, beginning and settling raids, a raid that never ended; versioned |
| `crates/nightfall-sim/src/{world,lib,messages}.rs`, `senses/mod.rs`, `combat/{mod,effects,heroes,tests}.rs` | Carry and loot in the world and snapshots; weight in pace and steps; gear in damage; food and water as items; protocol 5 |
| `content/nightfall/{items,loot}.ron` (new), `{map,rules,classes}.ron` | The items, the loot, the exits, the run's rules, the free kits |
| `crates/benilla-app/src/nightfall/stash.rs` (new) | The stash file (versioned, backup, broken files kept), the `Stash` resource, settling raids |
| `crates/benilla-app/src/nightfall/widgets.rs` (new) | Item icons, slots and text buttons for the panels |
| `crates/benilla-app/src/nightfall/stash_panel.rs` (new) | The menu's stash panel: stash, loadout, gold, selling |
| `crates/benilla-app/src/nightfall/backpack.rs` (new) | The raid's backpack and loot panels; clicks into acts |
| `crates/benilla-app/src/nightfall/loot_view.rs` (new) | Containers drawn, right-click to search, autoplay's looting |
| `crates/benilla-app/src/nightfall/gear.rs` (new) | Worn looks, lantern light, the load line |
| `crates/benilla-app/src/nightfall/exits.rs` (new) | Exit rings, the extract line, the remains' marker, autoplay's way out |
| `crates/benilla-app/src/nightfall/end_screen.rs` (new) | The end screen, and waiting once out |
| `crates/benilla-app/src/nightfall/{mod,run,session,menu,units,dress,fight}.rs` | Registration, acts in the input, outcomes on the wire, the Stash button, `spawn_object`, dress helpers, the `NoFood` refusal |

---

### Task 1: Content: items, loot, exits, the run's rules, free kits

**Files:**
- Create: `crates/nightfall-sim/src/items.rs`, `content/nightfall/items.ron`, `content/nightfall/loot.ron`
- Modify: `crates/nightfall-sim/src/content.rs`, `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`
- Modify (every `ContentSources { … }` literal gains `items` and `loot`): the test fixtures in `crates/nightfall-sim/src/{content,world,enemies,sight,messages}.rs`, `senses/tests.rs`, `combat/mod.rs`, `combat/tests.rs`
- Modify: `content/nightfall/map.ron`, `content/nightfall/rules.ron`, `content/nightfall/classes.ron`
- Modify (so the client still builds): `crates/benilla-app/src/nightfall/mod.rs` (`prepare`, `content_texts`)

**Interfaces:**
- Consumes: `Content::from_ron`, `ContentSources`, `ContentError`, `ClassDef`, `EnemyDef`, `MapDef`, `MapFile`, `Rules`, `Bounds`, the test fixtures `CLASSES`, `RULES`, `ENEMIES`, `SPAWNS` (foundation Task 3, world knowledge Task 5, combat Task 1); `Loudness` (senses Task 3); `V2`.
- Produces:
  - `nightfall_sim::{ItemDef, ItemKind, EquipSlot, Look, Bonus, ItemUse, LightDef, ContainerKindDef, Table, TableEntry, PlacedContainer, LootContent}`; `EquipSlot::{COUNT, ALL, index(self) -> usize, wow_slot(self) -> u8, name(self) -> &'static str}`.
  - `nightfall_sim::{RunRules, DeathLoss, ExitDef}`; `Rules::run: RunRules`; `MapDef::exits: Vec<ExitDef>`.
  - `ClassDef::free_kit: Vec<(String, u16)>` (as written) and `ClassDef::free_items: Vec<(usize, u16)>` (resolved).
  - `ContentSources::{items, loot}: &str`; `Content::{items: Vec<ItemDef>, loot: LootContent, conjured: Option<usize>}`; `Content::item(&self, key: &str) -> Option<usize>`.
  - Client: `nightfall::content_texts() -> &'static [String; 8]`.

- [ ] **Step 1: Write the items' types and their checks, with failing tests**

`crates/nightfall-sim/src/items.rs`:

```rust
//! Nightfall's things (spec §8.2) as content: what an item is, weighs and does, how it looks worn,
//! and the loot tables, containers and bodies that hold items on the map. Read from
//! `content/nightfall/items.ron` and `loot.ron`, and checked as they load, so a mistake is a clear
//! line at startup rather than a chest that never opens.

use serde::{Deserialize, Serialize};

use crate::{Bounds, EnemyDef, Loudness, V2};

/// What an item is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon,
    Armour,
    /// Used from the backpack: health or mana at once.
    Potion,
    /// Eaten by Eat and Drink: health over the meal.
    Food,
    /// Drunk by Eat and Drink: mana over the meal.
    Drink,
    /// Worth gold, and nothing else.
    Valuable,
    /// Opens a locked container, and is kept.
    Key,
    /// Worn in a bag slot: more backpack slots.
    Bag,
    /// Lit from the backpack: light, and a hero easier to see (spec §5.1).
    Lantern,
    /// Kept for the inn's stations (sub-project 5b).
    Material,
}

/// Where a piece of gear is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Head,
    Shoulders,
    Chest,
    Legs,
    Feet,
    Hands,
    Back,
    MainHand,
    OffHand,
}

impl EquipSlot {
    pub const COUNT: usize = 9;
    pub const ALL: [EquipSlot; 9] = [
        EquipSlot::Head,
        EquipSlot::Shoulders,
        EquipSlot::Chest,
        EquipSlot::Legs,
        EquipSlot::Feet,
        EquipSlot::Hands,
        EquipSlot::Back,
        EquipSlot::MainHand,
        EquipSlot::OffHand,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// WoW's equipment slot, which `PLAYER_VISIBLE_ITEM_*` is indexed by.
    pub fn wow_slot(self) -> u8 {
        match self {
            EquipSlot::Head => 0,
            EquipSlot::Shoulders => 2,
            EquipSlot::Chest => 4,
            EquipSlot::Legs => 6,
            EquipSlot::Feet => 7,
            EquipSlot::Hands => 9,
            EquipSlot::Back => 14,
            EquipSlot::MainHand => 15,
            EquipSlot::OffHand => 16,
        }
    }

    /// The slot's name, for the panels.
    pub fn name(self) -> &'static str {
        match self {
            EquipSlot::Head => "Head",
            EquipSlot::Shoulders => "Shoulders",
            EquipSlot::Chest => "Chest",
            EquipSlot::Legs => "Legs",
            EquipSlot::Feet => "Feet",
            EquipSlot::Hands => "Hands",
            EquipSlot::Back => "Back",
            EquipSlot::MainHand => "Main hand",
            EquipSlot::OffHand => "Off hand",
        }
    }
}

fn armour_class() -> u8 {
    4
}

fn cloth() -> u8 {
    1
}

fn cloth_material() -> u8 {
    7
}

/// How a worn item looks on the hero (spec §8.2: "each with a WoW item display").
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum Look {
    /// An `ItemDisplayInfo.dbc` id with the template fields a stand-in item needs
    /// (`nightfall::dress`), as the classes' weapons give them.
    Item {
        display: u32,
        inventory_type: u8,
        #[serde(default = "armour_class")]
        class: u8,
        #[serde(default = "cloth")]
        subclass: u8,
        #[serde(default = "cloth_material")]
        material: u8,
        #[serde(default)]
        sheath: u8,
    },
    /// The piece a new character of class `class_id` (`ChrClasses.dbc`) starts in for this
    /// item's slot (`CharStartOutfit.dbc`), in the hero's own race and sex: a look the data is
    /// sure to have. A slot that outfit leaves bare shows nothing.
    Outfit { class_id: u8 },
}

/// What worn gear adds (spec §6.1: power comes from gear).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Bonus {
    /// Health, added to the class's.
    pub health: f32,
    /// Damage, a fraction added to every blow and spell (0.1 is 10% more).
    pub damage: f32,
}

/// What a potion does when used.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum ItemUse {
    Heal(f32),
    Mana(f32),
}

/// A lit lantern's light.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct LightDef {
    /// Yards.
    pub range: f32,
    pub intensity: f32,
}

fn one_u8() -> u8 {
    1
}

fn one_u16() -> u16 {
    1
}

fn one_f32() -> f32 {
    1.0
}

/// One kind of thing.
#[derive(Clone, Debug, Deserialize)]
pub struct ItemDef {
    pub key: String,
    pub name: String,
    pub kind: ItemKind,
    /// The icon, `Interface\Icons\<icon>.blp`.
    pub icon: String,
    /// WoW's quality: 0 poor, 1 common, 2 uncommon, 3 rare, 4 epic.
    #[serde(default = "one_u8")]
    pub quality: u8,
    /// Each one's weight (spec §8.2).
    pub weight: f32,
    /// How many stack in one slot.
    #[serde(default = "one_u16")]
    pub stack: u16,
    /// Gold each, when sold.
    #[serde(default)]
    pub value: u32,
    /// Where it is worn (weapons and armour).
    #[serde(default)]
    pub slot: Option<EquipSlot>,
    #[serde(default)]
    pub look: Option<Look>,
    #[serde(default)]
    pub bonus: Bonus,
    /// A bag's slots.
    #[serde(default)]
    pub bag_slots: u8,
    /// Food and drink: the share of health or mana a meal of it gives back.
    #[serde(default = "one_f32")]
    pub restore: f32,
    /// A potion's effect.
    #[serde(default)]
    pub on_use: Option<ItemUse>,
    /// A lantern's light.
    #[serde(default)]
    pub light: Option<LightDef>,
    /// A line for the panels.
    #[serde(default)]
    pub text: String,
}

/// A kind of container the content places: how it looks, how long it takes to search and how
/// loud that is.
#[derive(Clone, Debug, Deserialize)]
pub struct ContainerKindDef {
    pub key: String,
    pub name: String,
    /// `GameObjectDisplayInfo.dbc` id.
    pub display: u32,
    #[serde(default = "one_f32")]
    pub scale: f32,
    /// Seconds to search it, standing by it.
    pub search: f32,
    /// How loud searching it is (spec §5.2: a chest 15 yards, a grave dug 30).
    pub sound: Loudness,
    #[serde(default = "eight")]
    pub slots: u8,
}

fn eight() -> u8 {
    8
}

fn one_one() -> (u16, u16) {
    (1, 1)
}

#[derive(Clone, Debug, Deserialize)]
struct TableEntryFile {
    item: String,
    weight: f32,
    #[serde(default = "one_one")]
    count: (u16, u16),
}

#[derive(Clone, Debug, Deserialize)]
struct TableFile {
    key: String,
    rolls: (u8, u8),
    #[serde(default)]
    nothing: f32,
    items: Vec<TableEntryFile>,
}

#[derive(Clone, Debug, Deserialize)]
struct PlacedFile {
    kind: String,
    table: String,
    at: (f32, f32),
    #[serde(default)]
    z: Option<f32>,
    #[serde(default)]
    lock: Option<String>,
    #[serde(default)]
    place: String,
}

#[derive(Clone, Debug, Deserialize)]
struct BodyFile {
    enemy: String,
    table: String,
}

/// `loot.ron` as written.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LootFile {
    kinds: Vec<ContainerKindDef>,
    tables: Vec<TableFile>,
    containers: Vec<PlacedFile>,
    bodies: Vec<BodyFile>,
    pile_display: u32,
    remains_display: u32,
}

/// One line of a loot table: an item, how likely, how many.
#[derive(Clone, Debug, PartialEq)]
pub struct TableEntry {
    /// Index into [`crate::Content::items`].
    pub item: usize,
    pub weight: f32,
    pub count: (u16, u16),
}

/// A loot table: `rolls` picks, each an entry by weight, or nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub key: String,
    pub rolls: (u8, u8),
    pub nothing: f32,
    pub items: Vec<TableEntry>,
}

/// A container the content places on the map.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedContainer {
    /// Index into [`LootContent::kinds`].
    pub kind: usize,
    /// Index into [`LootContent::tables`].
    pub table: usize,
    pub at: V2,
    /// Its height, when measured; without it, only the ground distance counts.
    pub z: Option<f32>,
    /// The key item it needs, an index into the items.
    pub lock: Option<usize>,
    /// Where it is, for the content's readers.
    pub place: String,
}

/// The loot of a map, resolved.
#[derive(Clone, Debug, Default)]
pub struct LootContent {
    pub kinds: Vec<ContainerKindDef>,
    pub tables: Vec<Table>,
    pub containers: Vec<PlacedContainer>,
    /// (enemy kind, table): what that kind's body holds.
    pub bodies: Vec<(usize, usize)>,
    /// `GameObjectDisplayInfo.dbc` ids for things dropped on the ground and for a hero's remains.
    pub pile_display: u32,
    pub remains_display: u32,
}

/// Every item's numbers and kind-specific parts make sense.
pub(crate) fn check_items(items: &[ItemDef]) -> Result<(), String> {
    for (i, it) in items.iter().enumerate() {
        let k = &it.key;
        if items[..i].iter().any(|o| o.key == *k) {
            return Err(format!("items.ron: item \"{k}\" is listed twice"));
        }
        if !(it.weight.is_finite() && it.weight >= 0.0) || it.stack == 0 {
            return Err(format!("items.ron: \"{k}\": weight is 0 or more and stack at least 1"));
        }
        let gear = matches!(it.kind, ItemKind::Weapon | ItemKind::Armour);
        if gear != it.slot.is_some() {
            return Err(format!(
                "items.ron: \"{k}\": weapons and armour have a slot, nothing else does"
            ));
        }
        let one = gear || matches!(it.kind, ItemKind::Bag | ItemKind::Lantern);
        if one && it.stack != 1 {
            return Err(format!("items.ron: \"{k}\": gear, bags and lanterns do not stack"));
        }
        if (it.kind == ItemKind::Bag) != (it.bag_slots > 0) {
            return Err(format!("items.ron: \"{k}\": a bag has slots, nothing else does"));
        }
        if it.kind == ItemKind::Lantern && it.light.is_none() {
            return Err(format!("items.ron: \"{k}\": a lantern needs its light"));
        }
        if it.kind == ItemKind::Potion && it.on_use.is_none() {
            return Err(format!("items.ron: \"{k}\": a potion needs on_use"));
        }
        if matches!(it.kind, ItemKind::Food | ItemKind::Drink)
            && !(it.restore > 0.0 && it.restore <= 1.0)
        {
            return Err(format!("items.ron: \"{k}\": restore is above 0 and at most 1"));
        }
        if !(it.bonus.health.is_finite() && it.bonus.damage.is_finite()) {
            return Err(format!("items.ron: \"{k}\": a bonus is a number"));
        }
    }
    Ok(())
}

impl LootFile {
    /// Names into indices, every name and number checked.
    pub(crate) fn resolve(
        self,
        items: &[ItemDef],
        enemies: &[EnemyDef],
        bounds: &Bounds,
    ) -> Result<LootContent, String> {
        let item = |key: &str| {
            items
                .iter()
                .position(|i| i.key == key)
                .ok_or_else(|| format!("loot.ron: no item \"{key}\" in items.ron"))
        };
        for (i, k) in self.kinds.iter().enumerate() {
            if self.kinds[..i].iter().any(|o| o.key == k.key) {
                return Err(format!("loot.ron: container kind \"{}\" is listed twice", k.key));
            }
            if !(k.search >= 0.0 && k.scale > 0.0) || k.slots == 0 {
                return Err(format!(
                    "loot.ron: \"{}\": search is 0 or more, scale above 0, slots at least 1",
                    k.key
                ));
            }
        }
        let mut tables = Vec::new();
        for (i, t) in self.tables.iter().enumerate() {
            if self.tables[..i].iter().any(|o| o.key == t.key) {
                return Err(format!("loot.ron: table \"{}\" is listed twice", t.key));
            }
            if t.rolls.0 > t.rolls.1 || !(t.nothing >= 0.0) {
                return Err(format!(
                    "loot.ron: table \"{}\": rolls go from least to most, nothing is 0 or more",
                    t.key
                ));
            }
            let mut entries = Vec::new();
            for e in &t.items {
                if !(e.weight >= 0.0) || e.count.0 == 0 || e.count.0 > e.count.1 {
                    return Err(format!(
                        "loot.ron: table \"{}\", \"{}\": weight 0 or more, count from 1",
                        t.key, e.item
                    ));
                }
                entries.push(TableEntry {
                    item: item(&e.item)?,
                    weight: e.weight,
                    count: e.count,
                });
            }
            tables.push(Table {
                key: t.key.clone(),
                rolls: t.rolls,
                nothing: t.nothing,
                items: entries,
            });
        }
        let table = |key: &str| {
            tables
                .iter()
                .position(|t| t.key == key)
                .ok_or_else(|| format!("loot.ron: no table \"{key}\""))
        };
        let mut containers = Vec::new();
        for c in &self.containers {
            let kind = self
                .kinds
                .iter()
                .position(|k| k.key == c.kind)
                .ok_or_else(|| format!("loot.ron: no container kind \"{}\"", c.kind))?;
            let at = V2::new(c.at.0, c.at.1);
            if !bounds.contains(at) {
                return Err(format!(
                    "loot.ron: the {} at ({}, {}) is outside the map",
                    c.kind, c.at.0, c.at.1
                ));
            }
            containers.push(PlacedContainer {
                kind,
                table: table(&c.table)?,
                at,
                z: c.z,
                lock: c.lock.as_deref().map(item).transpose()?,
                place: c.place.clone(),
            });
        }
        let mut bodies = Vec::new();
        for b in &self.bodies {
            let enemy = enemies
                .iter()
                .position(|e| e.key == b.enemy)
                .ok_or_else(|| format!("loot.ron: no enemy \"{}\" in enemies.ron", b.enemy))?;
            bodies.push((enemy, table(&b.table)?));
        }
        Ok(LootContent {
            kinds: self.kinds,
            tables,
            containers,
            bodies,
            pile_display: self.pile_display,
            remains_display: self.remains_display,
        })
    }
}
```

In `lib.rs` add `mod items;` and:

```rust
pub use items::{
    Bonus, ContainerKindDef, EquipSlot, ItemDef, ItemKind, ItemUse, LightDef, Look, LootContent,
    PlacedContainer, Table, TableEntry,
};
```

- [ ] **Step 2: Write the content's tests**

In `crates/nightfall-sim/src/content.rs`'s test module, add:

```rust
    const ITEMS: &str = r#"[
        (key: "knife", name: "Knife", kind: Weapon, icon: "INV_Weapon_ShortBlade_01", weight: 1.0,
         slot: Some(MainHand),
         look: Some(Item(display: 6443, inventory_type: 13, class: 2, subclass: 15, material: 1,
                         sheath: 3))),
        (key: "bread", name: "Bread", kind: Food, icon: "INV_Misc_Food_11", weight: 0.5, stack: 10),
        (key: "water", name: "Water", kind: Drink, icon: "INV_Drink_07", weight: 0.5, stack: 10),
        (key: "pouch", name: "Pouch", kind: Bag, icon: "INV_Misc_Bag_09", weight: 0.5, bag_slots: 4),
        (key: "key", name: "Key", kind: Key, icon: "INV_Misc_Key_03", weight: 0.1),
    ]"#;
    const LOOT: &str = r#"(
        kinds: [(key: "chest", name: "Chest", display: 259, search: 2.0, sound: Open)],
        tables: [(key: "t", rolls: (1, 2), nothing: 1.0,
                  items: [(item: "bread", weight: 2.0, count: (1, 3)), (item: "knife", weight: 1.0)])],
        containers: [(kind: "chest", table: "t", at: (20.0, 20.0), lock: Some("key"))],
        bodies: [(enemy: "skeleton", table: "t")],
    )"#;
    const EXIT_MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
        entries: [(50.0, 50.0, 10.0)],
        exits: [(key: "bridge", name: "The Bridge", at: (90.0, 90.0)),
                (key: "cart", name: "The Cart", at: (10.0, 90.0), radius: 5.0, late: true)])"#;

    fn load_loot(map: &str, rules: &str, items: &str, loot: &str) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            map,
            rules,
            enemies: ENEMIES,
            spawns: SPAWNS,
            abilities: "[]",
            items,
            loot,
        })
    }

    #[test]
    fn items_loot_and_exits_load() {
        let c = load_loot(EXIT_MAP, RULES, ITEMS, LOOT).unwrap();
        assert_eq!(c.item("bread"), Some(1));
        assert_eq!(c.item("cake"), None);
        assert_eq!(c.loot.tables[0].items[0].item, 1);
        assert_eq!(c.loot.containers[0].lock, Some(4));
        assert_eq!(c.loot.containers[0].z, None);
        assert_eq!(c.loot.bodies, vec![(0, 0)]);
        assert_eq!(c.map.exits.len(), 2);
        assert_eq!(c.map.exits[0].radius, 4.0);
        assert!(c.map.exits[1].late);
        assert_eq!(c.rules.run.death_loss, DeathLoss::EverythingOneRecovery);
        assert_eq!(c.rules.run.safe_slots, 2);
        assert_eq!(c.rules.run.backpack_slots, 16);
        assert_eq!(c.conjured, None);
    }

    #[test]
    fn an_item_with_the_wrong_parts_is_refused() {
        for bad in [
            ITEMS.replace("slot: Some(MainHand),", ""),
            ITEMS.replace("bag_slots: 4", "bag_slots: 0"),
            ITEMS.replace("weight: 0.1", "weight: -1.0"),
            ITEMS.replace("\"water\", name", "\"bread\", name"),
        ] {
            assert!(
                matches!(load_loot(EXIT_MAP, RULES, &bad, LOOT), Err(ContentError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn loot_naming_what_is_not_there_is_refused() {
        for bad in [
            LOOT.replace("item: \"bread\"", "item: \"cake\""),
            LOOT.replace("table: \"t\", at", "table: \"u\", at"),
            LOOT.replace("enemy: \"skeleton\"", "enemy: \"dragon\""),
            LOOT.replace("rolls: (1, 2)", "rolls: (3, 2)"),
            LOOT.replace("at: (20.0, 20.0)", "at: (200.0, 20.0)"),
        ] {
            assert!(
                matches!(load_loot(EXIT_MAP, RULES, ITEMS, &bad), Err(ContentError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn an_exit_outside_the_map_is_refused() {
        let map = EXIT_MAP.replace("at: (90.0, 90.0)", "at: (190.0, 90.0)");
        assert!(matches!(load_loot(&map, RULES, ITEMS, LOOT), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn the_weight_rules_must_make_sense() {
        // RULES opens with `(hero_level: 30,`; the run's rules go in right after it.
        let with = |run: &str| {
            let rules =
                RULES.replacen("(hero_level: 30,", &format!("(hero_level: 30, run: ({run}),"), 1);
            assert_ne!(rules, RULES);
            load_loot(EXIT_MAP, &rules, ITEMS, LOOT)
        };
        assert!(with("weight_free: 25.0").is_ok());
        assert!(with("death_loss: found_only").is_ok());
        for bad in [
            "weight_free: 60.0, weight_full: 60.0",
            "weight_slow_floor: 0.0",
            "weight_slow_floor: 1.5",
            "weight_noise_max: 0.5",
            "extract_for: 0.0",
            "backpack_slots: 0",
        ] {
            assert!(matches!(with(bad), Err(ContentError::Invalid(_))), "{bad}");
        }
    }

    #[test]
    fn a_free_kit_names_real_items() {
        let classes = CLASSES.replacen(
            "base_hp: 100.0,",
            "base_hp: 100.0, free_kit: [(\"knife\", 1), (\"bread\", 4)],",
            1,
        );
        let c = Content::from_ron(ContentSources {
            classes: &classes,
            map: EXIT_MAP,
            rules: RULES,
            enemies: ENEMIES,
            spawns: SPAWNS,
            abilities: "[]",
            items: ITEMS,
            loot: LOOT,
        })
        .unwrap();
        assert_eq!(c.classes[0].free_items, vec![(0, 1), (1, 4)]);
        let bad = classes.replace("\"bread\", 4", "\"cake\", 4");
        let r = Content::from_ron(ContentSources {
            classes: &bad,
            map: EXIT_MAP,
            rules: RULES,
            enemies: ENEMIES,
            spawns: SPAWNS,
            abilities: "[]",
            items: ITEMS,
            loot: LOOT,
        });
        assert!(matches!(r, Err(ContentError::Invalid(_))));
    }
```

The fixtures `CLASSES` (its first class has `base_hp: 100.0,`), `RULES` (one RON struct ending in `)`), `ENEMIES` (with `"skeleton"`) and `SPAWNS` are the module's own from the earlier plans. Every other `ContentSources { … }` literal in the crate (`grep -rn "ContentSources {" crates/nightfall-sim`: the test fixtures of `content.rs`'s `load_all`, `world.rs`, `enemies.rs`, `sight.rs`, `messages.rs`, `senses/tests.rs`, `combat/mod.rs` and `combat/tests.rs`) gains, after `abilities`:

```rust
            items: "[]",
            loot: "()",
```

and the one in `tests/shipped_content.rs`'s `shipped` gains:

```rust
        items: include_str!("../../../content/nightfall/items.ron"),
        loot: include_str!("../../../content/nightfall/loot.ron"),
```

(`messages.rs`'s `a_full_partys_state_is_small` reads the shipped files with `include_str!` too: its literal gains the same two `include_str!` lines, `"../../../content/nightfall/items.ron"` and `…/loot.ron`.)

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`struct ContentSources has no field named items`, `no method named item`).

- [ ] **Step 3: Load and check them**

In `content.rs`:

- Add to the `use` lines: `use crate::items::{check_items, LootFile};` and `use crate::{ItemDef, LootContent};`.
- Add:

```rust
/// What a death takes (spec §8.2), the same everywhere on the map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathLoss {
    /// All gear and bags are lost with the body.
    Everything,
    /// All gear stays on the body, and the body is there in the player's next raid on the map.
    #[default]
    EverythingOneRecovery,
    /// Only what was found this raid is lost.
    FoundOnly,
}

/// The run's rules (spec §8, §11): every number a setting, §11's value its default.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RunRules {
    pub death_loss: DeathLoss,
    /// Slots always kept, whatever happens.
    pub safe_slots: u8,
    /// The backpack's own slots, before bags.
    pub backpack_slots: u8,
    /// Stacks the stash holds (sub-project 5b's stash station adds to it).
    pub stash_slots: u16,
    /// Weight carried with no penalty.
    pub weight_free: f32,
    /// Weight at which the penalty is whole: `weight_slow_floor` of normal speed and footsteps
    /// `weight_noise_max` times as far; heavier changes nothing more.
    pub weight_full: f32,
    pub weight_slow_floor: f32,
    pub weight_noise_max: f32,
    /// Seconds in an exit's ring to get out.
    pub extract_for: f32,
    /// When an exit marked `late` opens, seconds into the raid.
    pub cart_opens_at: f32,
    /// How near a container must be to search it or take from it, yards.
    pub reach: f32,
    /// Seconds to search a body.
    pub search_body: f32,
    /// A search is heard every so many seconds while it lasts.
    pub noise_every: f32,
    /// The slots of a body or of things dropped on the ground.
    pub container_slots: u8,
    /// Seconds between two potions.
    pub potion_cooldown: f32,
    /// Eat and Drink restores health with no food in the backpack (a drink is still needed for
    /// mana): the setting to playtest food as a resource or not.
    pub free_meal: bool,
    /// The item Conjure Water makes.
    pub conjured_drink: String,
}

impl Default for RunRules {
    fn default() -> Self {
        Self {
            death_loss: DeathLoss::EverythingOneRecovery,
            safe_slots: 2,
            backpack_slots: 16,
            stash_slots: 60,
            weight_free: 20.0,
            weight_full: 60.0,
            weight_slow_floor: 0.7,
            weight_noise_max: 1.5,
            extract_for: 10.0,
            cart_opens_at: 1080.0,
            reach: 3.0,
            search_body: 1.5,
            noise_every: 2.0,
            container_slots: 16,
            potion_cooldown: 30.0,
            free_meal: false,
            conjured_drink: "conjured_water".into(),
        }
    }
}

impl RunRules {
    fn check(&self) -> Result<(), String> {
        let finite = [
            self.weight_free,
            self.weight_full,
            self.weight_slow_floor,
            self.weight_noise_max,
            self.extract_for,
            self.cart_opens_at,
            self.reach,
            self.search_body,
            self.noise_every,
            self.potion_cooldown,
        ];
        if finite.iter().any(|n| !n.is_finite()) {
            return Err("rules.ron: run: every number is a number".into());
        }
        if self.backpack_slots == 0 || self.safe_slots > 8 || self.stash_slots == 0 {
            return Err(
                "rules.ron: run: a backpack of 1 slot or more, at most 8 safe slots, a stash".into(),
            );
        }
        if !(self.weight_free >= 0.0 && self.weight_full > self.weight_free) {
            return Err("rules.ron: run: weight_full is above weight_free".into());
        }
        if !(self.weight_slow_floor > 0.0 && self.weight_slow_floor <= 1.0) {
            // Above 0, so a hero is never stuck (spec §8.2).
            return Err("rules.ron: run: weight_slow_floor is above 0 and at most 1".into());
        }
        if self.weight_noise_max < 1.0 {
            return Err("rules.ron: run: weight_noise_max is 1 or more".into());
        }
        if !(self.extract_for > 0.0 && self.reach > 0.0 && self.noise_every > 0.0) {
            return Err("rules.ron: run: extract_for, reach and noise_every are above 0".into());
        }
        if self.cart_opens_at < 0.0 || self.search_body < 0.0 || self.potion_cooldown < 0.0 {
            return Err(
                "rules.ron: run: cart_opens_at, search_body and potion_cooldown are 0 or more"
                    .into(),
            );
        }
        if self.container_slots == 0 {
            return Err("rules.ron: run: container_slots is at least 1".into());
        }
        Ok(())
    }
}

/// A way out of the map (spec §8.1).
#[derive(Clone, Debug, PartialEq)]
pub struct ExitDef {
    pub key: String,
    pub name: String,
    pub at: V2,
    /// The ring's radius, yards.
    pub radius: f32,
    /// Opens at `cart_opens_at`, not at the start (Darkshire's cart).
    pub late: bool,
}

fn four() -> f32 {
    4.0
}

#[derive(Deserialize)]
struct ExitFile {
    key: String,
    name: String,
    at: (f32, f32),
    #[serde(default = "four")]
    radius: f32,
    #[serde(default)]
    late: bool,
}
```

- `Rules` gains:

```rust
    /// The run (spec §8, §11).
    #[serde(default)]
    pub run: RunRules,
```

- `MapFile` gains `#[serde(default)] exits: Vec<ExitFile>,` and `MapDef` gains:

```rust
    /// The ways out (spec §8.1).
    pub exits: Vec<ExitDef>,
```

- `ClassDef` gains:

```rust
    /// The free basic kit (spec §8.3): items and counts, as written.
    #[serde(default)]
    pub free_kit: Vec<(String, u16)>,
    /// The kit resolved: indices into [`Content::items`].
    #[serde(skip)]
    pub free_items: Vec<(usize, u16)>,
```

- `ContentSources` gains `pub items: &'a str,` and `pub loot: &'a str,`; `Content` gains:

```rust
    pub items: Vec<ItemDef>,
    pub loot: LootContent,
    /// The item Conjure Water makes, when the content has it.
    pub conjured: Option<usize>,
```

- In `from_ron`, after the map's entries are checked (the `bounds` local is made by then), add:

```rust
        let mut exits = Vec::new();
        for e in &map.exits {
            let at = V2::new(e.at.0, e.at.1);
            if !bounds.contains(at) || !(e.radius > 0.0) {
                return invalid(format!(
                    "map.ron: exit \"{}\" must be inside the map with a radius above 0",
                    e.key
                ));
            }
            exits.push(ExitDef {
                key: e.key.clone(),
                name: e.name.clone(),
                at,
                radius: e.radius,
                late: e.late,
            });
        }
```

and the `MapDef { … }` literal gains `exits,` (any other `MapDef { … }` literal, `grep -rn "MapDef {" crates/nightfall-sim crates/benilla-app/src/nightfall`, gains `exits: Vec::new(),`).
- Before the final `Ok(Self {`, add (the `enemies` local is the parsed `Vec<EnemyDef>`):

```rust
        let items: Vec<ItemDef> = parse("items.ron", src.items)?;
        check_items(&items).map_err(ContentError::Invalid)?;
        let loot_file: LootFile = parse("loot.ron", src.loot)?;
        let loot = loot_file
            .resolve(&items, &enemies, &bounds)
            .map_err(ContentError::Invalid)?;
        rules.run.check().map_err(ContentError::Invalid)?;
        let mut classes = classes;
        for c in &mut classes {
            let mut kit = Vec::new();
            for (key, count) in &c.free_kit {
                let Some(i) = items.iter().position(|it| it.key == *key) else {
                    return invalid(format!(
                        "classes.ron: \"{}\"'s free kit names no item \"{key}\"",
                        c.key
                    ));
                };
                if *count == 0 {
                    return invalid(format!("classes.ron: \"{}\": a kit count is 1 or more", c.key));
                }
                kit.push((i, *count));
            }
            c.free_items = kit;
        }
        let conjured = items.iter().position(|i| i.key == rules.run.conjured_drink);
```

and the final literal gains `items,`, `loot,` and `conjured,`. (If the earlier plans moved the parsed classes into an already mutable binding, the `let mut classes = classes;` line is harmless; if `enemies` is consumed into the literal before this point, move this block above that use.)
- Add to `impl Content`:

```rust
    /// The item with this key, by index.
    pub fn item(&self, key: &str) -> Option<usize> {
        self.items.iter().position(|i| i.key == key)
    }
```

In `lib.rs`'s `pub use content::{ … }` add `DeathLoss, ExitDef, RunRules`.

In `crates/benilla-app/src/nightfall/mod.rs`: `CONTENT_TEXTS` and `content_texts()` hold `[String; 8]` (the sixth and seventh entries `items` and `loot` after `abilities`); `prepare` reads two more files the way it reads the others:

```rust
    let items = read("items.ron", include_str!("../../../../content/nightfall/items.ron"))?;
    let loot = read("loot.ron", include_str!("../../../../content/nightfall/loot.ron"))?;
```

and its `ContentSources { … }` literal gains `items: &items, loot: &loot,`; the `CONTENT_TEXTS.set(…)` array gains `items, loot` at its end. The session's content hash (`our_content_hash`) hashes every entry of the array, so it covers the new files with no change.

Run: `cargo test -p nightfall-sim content`
Expected: PASS (the 6 new tests with the module's earlier ones).

- [ ] **Step 4: Write the shipped content**

`content/nightfall/items.ron`:

```ron
// Nightfall's things (spec §8.2). `icon` is the icon's name under Interface\Icons. `look` is how
// worn gear shows: `Item(display: …)` an ItemDisplayInfo id with the stand-in template's fields
// (the classes' own weapons), or `Outfit(class_id: …)` the piece a new character of that class
// starts in for the slot (always in the data; worn in the hero's own race and sex). `bonus` adds
// health and a share of damage; `restore` is the share of health (food) or mana (drink) a meal
// gives back; `value` is gold each.
[
    // The free kits' weapons: the looks the classes had before items.
    (key: "worn_shortsword", name: "Worn Shortsword", kind: Weapon, icon: "INV_Sword_04",
     weight: 2.5, value: 1, slot: Some(MainHand),
     look: Some(Item(display: 7492, inventory_type: 13, class: 2, subclass: 7, material: 1,
                     sheath: 3))),
    (key: "worn_dagger", name: "Worn Dagger", kind: Weapon, icon: "INV_Weapon_ShortBlade_01",
     weight: 1.0, value: 1, slot: Some(OffHand),
     look: Some(Item(display: 6443, inventory_type: 13, class: 2, subclass: 15, material: 1,
                     sheath: 3))),
    (key: "apprentice_staff", name: "Apprentice's Staff", kind: Weapon, icon: "INV_Staff_02",
     weight: 3.0, value: 1, slot: Some(MainHand),
     look: Some(Item(display: 1927, inventory_type: 17, class: 2, subclass: 10, material: 2,
                     sheath: 2))),
    // Found weapons.
    (key: "night_watch_blade", name: "Night Watch Blade", kind: Weapon, icon: "INV_Sword_05",
     quality: 2, weight: 3.0, value: 40, slot: Some(MainHand), bonus: (damage: 0.10),
     look: Some(Item(display: 7492, inventory_type: 13, class: 2, subclass: 7, material: 1,
                     sheath: 3)),
     text: "Darkshire's own steel, taken off a guard who did not come back."),
    (key: "raven_hill_dirk", name: "Raven Hill Dirk", kind: Weapon, icon: "INV_Weapon_ShortBlade_05",
     quality: 2, weight: 1.0, value: 30, slot: Some(OffHand), bonus: (damage: 0.06),
     look: Some(Item(display: 6443, inventory_type: 13, class: 2, subclass: 15, material: 1,
                     sheath: 3))),
    (key: "darkshire_staff", name: "Darkshire Staff", kind: Weapon, icon: "INV_Staff_08",
     quality: 2, weight: 3.5, value: 50, slot: Some(MainHand), bonus: (health: 40.0, damage: 0.12),
     look: Some(Item(display: 1927, inventory_type: 17, class: 2, subclass: 10, material: 2,
                     sheath: 2))),
    // Armour, in looks the starting outfits are sure to have.
    (key: "watchmans_tunic", name: "Watchman's Tunic", kind: Armour, icon: "INV_Chest_Chain_05",
     quality: 2, weight: 5.0, value: 35, slot: Some(Chest), bonus: (health: 90.0),
     look: Some(Outfit(class_id: 1))),
    (key: "acolytes_robe", name: "Acolyte's Robe", kind: Armour, icon: "INV_Chest_Cloth_17",
     quality: 2, weight: 2.5, value: 35, slot: Some(Chest), bonus: (health: 45.0, damage: 0.05),
     look: Some(Outfit(class_id: 5))),
    (key: "scavengers_leggings", name: "Scavenger's Leggings", kind: Armour,
     icon: "INV_Pants_Leather_09", quality: 1, weight: 3.0, value: 20, slot: Some(Legs),
     bonus: (health: 50.0), look: Some(Outfit(class_id: 4))),
    (key: "gravediggers_boots", name: "Gravedigger's Boots", kind: Armour, icon: "INV_Boots_05",
     quality: 1, weight: 2.0, value: 15, slot: Some(Feet), bonus: (health: 30.0),
     look: Some(Outfit(class_id: 2))),
    // Bags.
    (key: "linen_pouch", name: "Linen Pouch", kind: Bag, icon: "INV_Misc_Bag_09", weight: 0.5,
     value: 5, bag_slots: 4),
    (key: "duskwood_satchel", name: "Duskwood Satchel", kind: Bag, icon: "INV_Misc_Bag_10",
     quality: 2, weight: 1.0, value: 25, bag_slots: 8),
    // Light: lit from the backpack, a hero sees further and is seen sooner (spec §5.1, §10).
    (key: "hooded_lantern", name: "Hooded Lantern", kind: Lantern, icon: "INV_Misc_Lantern_01",
     weight: 1.5, value: 10, light: Some((range: 18.0, intensity: 1.4))),
    // Food and drink, for Eat and Drink.
    (key: "tough_bread", name: "Tough Hunk of Bread", kind: Food, icon: "INV_Misc_Food_11",
     weight: 0.2, stack: 20, value: 1),
    (key: "smoked_venison", name: "Smoked Venison", kind: Food, icon: "INV_Misc_Food_14",
     weight: 0.3, stack: 20, value: 2),
    (key: "spring_water", name: "Refreshing Spring Water", kind: Drink, icon: "INV_Drink_07",
     weight: 0.3, stack: 20, value: 1),
    (key: "conjured_water", name: "Conjured Water", kind: Drink, icon: "INV_Drink_18",
     weight: 0.1, stack: 20),
    // Potions.
    (key: "minor_healing_potion", name: "Minor Healing Potion", kind: Potion,
     icon: "INV_Potion_51", weight: 0.3, stack: 5, value: 8, on_use: Some(Heal(250.0))),
    (key: "minor_mana_potion", name: "Minor Mana Potion", kind: Potion, icon: "INV_Potion_76",
     weight: 0.3, stack: 5, value: 8, on_use: Some(Mana(300.0))),
    // Valuables: gold, and weight.
    (key: "silver_locket", name: "Silver Locket", kind: Valuable, icon: "INV_Jewelry_Necklace_04",
     quality: 2, weight: 0.2, value: 25),
    (key: "ruby_shard", name: "Ruby Shard", kind: Valuable, icon: "INV_Misc_Gem_Ruby_01",
     quality: 2, weight: 0.3, stack: 10, value: 40),
    (key: "gold_ring", name: "Gold Ring", kind: Valuable, icon: "INV_Jewelry_Ring_03",
     quality: 3, weight: 0.1, value: 60),
    (key: "old_coins", name: "Old Coins", kind: Valuable, icon: "INV_Misc_Coin_02",
     weight: 0.2, stack: 20, value: 6),
    (key: "engraved_skull", name: "Engraved Skull", kind: Valuable,
     icon: "INV_Misc_Bone_HumanSkull_01", quality: 2, weight: 4.0, value: 45,
     text: "Heavy, and worth it to the right buyer in Darkshire."),
    // Keys.
    (key: "crypt_key", name: "Crypt Key", kind: Key, icon: "INV_Misc_Key_03", weight: 0.1),
    // Materials, for the inn's stations.
    (key: "linen_cloth", name: "Linen Cloth", kind: Material, icon: "INV_Fabric_Linen_01",
     weight: 0.1, stack: 20, value: 1),
    (key: "worgen_pelt", name: "Worgen Pelt", kind: Material, icon: "INV_Misc_Pelt_Wolf_01",
     weight: 1.5, stack: 10, value: 4),
    (key: "spider_silk", name: "Spider Silk", kind: Material, icon: "INV_Misc_Thread_01",
     weight: 0.1, stack: 20, value: 3),
    (key: "bone_fragments", name: "Bone Fragments", kind: Material, icon: "INV_Misc_Bone_08",
     weight: 0.5, stack: 20, value: 2),
    (key: "iron_scrap", name: "Iron Scrap", kind: Material, icon: "INV_Ingot_03",
     weight: 1.0, stack: 20, value: 3),
]
```

`content/nightfall/loot.ron`:

```ron
// What holds things on the map (spec §8.2). `kinds` are the containers the content places: a
// GameObjectDisplayInfo id, how long a search takes standing by it and how loud it is (spec §5.2:
// Open 15 yards, Dig 30). A table makes `rolls` picks, each an entry by weight or `nothing`;
// `count` is how many of it. `containers` are placed at WoW (x, y); `z` (optional) when measured,
// so one in a crypt under a hill is not opened from the hill. `bodies` are what an enemy kind's
// body holds once it dies. `pile_display` is things dropped on the ground; `remains_display` a
// hero's body from their last raid.
//
// The placed containers' displays and places are marked `measure:`: check each in a shot
// (NIGHTFALL_TRACE_POS=1 logs where the hero stands) and move it where it belongs. The two crates
// on the autoplay's circle around the first entry stay, so a test run loots.
(
    kinds: [
        // `TreasureChest01.mdx`, the survivors' chest.
        (key: "chest", name: "Chest", display: 259, scale: 1.0, search: 2.5, sound: Open, slots: 8),
        // measure: a crate's display; the titan cache stands in.
        (key: "crate", name: "Crate", display: 1387, scale: 0.8, search: 1.5, sound: Open, slots: 6),
        // measure: a grave mound; the vase stands in, low and small.
        (key: "grave", name: "Grave", display: 52, scale: 0.6, search: 6.0, sound: Dig, slots: 6),
    ],
    tables: [
        (key: "crate", rolls: (1, 3), nothing: 2.0, items: [
            (item: "tough_bread", weight: 4.0, count: (1, 3)),
            (item: "spring_water", weight: 4.0, count: (1, 3)),
            (item: "linen_cloth", weight: 3.0, count: (1, 4)),
            (item: "iron_scrap", weight: 2.0, count: (1, 2)),
            (item: "minor_healing_potion", weight: 1.0),
            (item: "old_coins", weight: 2.0, count: (2, 6)),
            (item: "linen_pouch", weight: 0.5),
        ]),
        (key: "chest", rolls: (2, 4), nothing: 1.0, items: [
            (item: "minor_healing_potion", weight: 3.0, count: (1, 2)),
            (item: "minor_mana_potion", weight: 2.0, count: (1, 2)),
            (item: "silver_locket", weight: 2.0),
            (item: "ruby_shard", weight: 1.5, count: (1, 2)),
            (item: "hooded_lantern", weight: 1.0),
            (item: "duskwood_satchel", weight: 0.7),
            (item: "scavengers_leggings", weight: 1.0),
            (item: "gravediggers_boots", weight: 1.0),
            (item: "night_watch_blade", weight: 0.5),
            (item: "darkshire_staff", weight: 0.5),
        ]),
        (key: "grave", rolls: (1, 3), nothing: 1.0, items: [
            (item: "bone_fragments", weight: 4.0, count: (2, 5)),
            (item: "old_coins", weight: 3.0, count: (3, 10)),
            (item: "gold_ring", weight: 1.0),
            (item: "engraved_skull", weight: 1.0),
            (item: "raven_hill_dirk", weight: 0.6),
            (item: "acolytes_robe", weight: 0.6),
        ]),
        (key: "worgen", rolls: (0, 2), nothing: 2.0, items: [
            (item: "worgen_pelt", weight: 4.0),
            (item: "old_coins", weight: 2.0, count: (1, 4)),
            (item: "smoked_venison", weight: 1.0),
        ]),
        (key: "undead", rolls: (0, 2), nothing: 2.0, items: [
            (item: "bone_fragments", weight: 4.0, count: (1, 3)),
            (item: "old_coins", weight: 1.0, count: (1, 3)),
        ]),
        (key: "spider", rolls: (0, 2), nothing: 2.0, items: [
            (item: "spider_silk", weight: 4.0, count: (1, 3)),
        ]),
        (key: "caster", rolls: (1, 2), nothing: 1.0, items: [
            (item: "minor_mana_potion", weight: 2.0),
            (item: "linen_cloth", weight: 3.0, count: (1, 3)),
            (item: "silver_locket", weight: 1.0),
        ]),
        (key: "elite", rolls: (2, 3), items: [
            (item: "watchmans_tunic", weight: 1.0),
            (item: "night_watch_blade", weight: 1.0),
            (item: "darkshire_staff", weight: 1.0),
            (item: "gold_ring", weight: 1.5),
            (item: "duskwood_satchel", weight: 1.0),
            (item: "ruby_shard", weight: 2.0, count: (2, 4)),
        ]),
    ],
    containers: [
        // On the autoplay's 15-yard circle around the first entry, so a test run loots.
        (kind: "crate", table: "crate", at: (-10036.8, -792.5), place: "By the entry (east)"),
        (kind: "crate", table: "crate", at: (-10051.8, -777.5), place: "By the entry (west)"),
        // measure: Raven Hill cemetery's graves.
        (kind: "grave", table: "grave", at: (-10760.0, 300.0), place: "Raven Hill cemetery"),
        (kind: "grave", table: "grave", at: (-10770.0, 310.0), place: "Raven Hill cemetery"),
        (kind: "grave", table: "grave", at: (-10750.0, 320.0), place: "Raven Hill cemetery"),
        // measure: Raven Hill's crypt, locked.
        (kind: "chest", table: "chest", at: (-10745.0, 295.0), lock: Some("crypt_key"),
         place: "Raven Hill crypt"),
        // measure: Tranquil Gardens Cemetery.
        (kind: "grave", table: "grave", at: (-10560.0, -680.0), place: "Tranquil Gardens"),
        // measure: the Yorgen Farmstead's cellar.
        (kind: "chest", table: "chest", at: (-10930.0, -390.0), place: "Yorgen Farmstead"),
        // measure: Addle's Stead.
        (kind: "crate", table: "crate", at: (-10900.0, 280.0), place: "Addle's Stead"),
        // measure: Vul'Gol Ogre Mound.
        (kind: "chest", table: "chest", at: (-10450.0, -1440.0), place: "Vul'Gol"),
        // measure: Brightwood Grove.
        (kind: "crate", table: "crate", at: (-10280.0, -1250.0), place: "Brightwood Grove"),
    ],
    bodies: [
        (enemy: "nightbane_worgen", table: "worgen"),
        (enemy: "black_ravager", table: "worgen"),
        (enemy: "skeletal_warrior", table: "undead"),
        (enemy: "fetid_corpse", table: "undead"),
        (enemy: "venom_web_spider", table: "spider"),
        (enemy: "nightbane_shadow_weaver", table: "caster"),
        (enemy: "fenros", table: "elite"),
        (enemy: "naraxis", table: "elite"),
    ],
    // measure: a sack on the ground (the chest stands in) and a heap of bones (the vase).
    pile_display: 259,
    remains_display: 52,
)
```

In `content/nightfall/map.ron`, before the closing `)`:

```ron
    // The ways out (spec §8.1): stand `extract_for` seconds in a ring of `radius` yards (4 when
    // left out). `late` opens at rules.ron's `cart_opens_at`. measure: every exit, in the game.
    exits: [
        (key: "bridge", name: "The bridge to Elwynn", at: (-9930.0, -480.0)),
        (key: "westfall_road", name: "The Westfall road", at: (-10650.0, 760.0)),
        (key: "deadwind_road", name: "The Deadwind road", at: (-10900.0, -1800.0)),
        (key: "darkshire_cart", name: "The Night Watch cart", at: (-10560.0, -1150.0), late: true),
    ],
```

In `content/nightfall/rules.ron`, before the closing `)`:

```ron
    // The run (spec §8, §11). death_loss: everything, everything_one_recovery or found_only.
    // Past weight_free a hero slows and steps louder, linearly, to weight_slow_floor of normal
    // speed and weight_noise_max times the footstep radius at weight_full; never stuck.
    // free_meal: Eat and Drink heals with no food (a drink still needed for mana).
    run: (
        death_loss: everything_one_recovery,
        safe_slots: 2,
        backpack_slots: 16,
        stash_slots: 60,
        weight_free: 20.0,
        weight_full: 60.0,
        weight_slow_floor: 0.7,
        weight_noise_max: 1.5,
        extract_for: 10.0,
        cart_opens_at: 1080.0,
        reach: 3.0,
        search_body: 1.5,
        noise_every: 2.0,
        container_slots: 16,
        potion_cooldown: 30.0,
        free_meal: false,
        conjured_drink: "conjured_water",
    ),
```

In `content/nightfall/classes.ron`, the rogue gains `free_kit: [("worn_shortsword", 1), ("worn_dagger", 1), ("tough_bread", 4), ("spring_water", 2)],` and the mage `free_kit: [("apprentice_staff", 1), ("tough_bread", 4), ("spring_water", 4)],` (each after its `weapons: [ … ],`).

In `tests/shipped_content.rs` add:

```rust
#[test]
fn the_shipped_content_has_its_items_loot_exits_and_kits() {
    let c = shipped();
    assert!(c.items.len() >= 20);
    assert!(c.conjured.is_some());
    assert!(c.map.exits.len() >= 3);
    assert!(c.map.exits.iter().any(|e| e.late));
    assert!(!c.loot.containers.is_empty());
    assert!(!c.loot.bodies.is_empty());
    for class in &c.classes {
        assert!(!class.free_items.is_empty(), "{} has a free kit", class.key);
    }
}
```

- [ ] **Step 5: Run every test of the crate, and check the client still builds**

Run: `cargo test -p nightfall-sim`
Expected: PASS.
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim content/nightfall crates/benilla-app/src/nightfall/mod.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: items, loot tables, containers, exits and the run's rules as content

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The inventory: slots, stacks, bags, gear, weight, death

**Files:**
- Create: `crates/nightfall-sim/src/inventory.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `Content::{items, rules.run}`, `ItemKind`, `EquipSlot`, `Bonus`, `DeathLoss` (Task 1).
- Produces:
  - `nightfall_sim::{Inventory, Stack, Place, InvError, MAX_BAGS}`.
  - `Stack { item: u16, count: u16, found: bool }` with `Stack::new(item: usize, count: u16)` and `Stack::found(item: usize, count: u16)`.
  - `Place::{Equip(EquipSlot), Bag(u8), Pack(u16), Safe(u8)}`; `InvError::{Empty, NoSuchPlace, WrongSlot, Full, BagInUse}`.
  - `Inventory { equip: [Option<Stack>; 9], bags: [Option<Stack>; 4], pack: Vec<Option<Stack>>, safe: Vec<Option<Stack>> }` with `new(&Content)`, `pack_size(&self, &Content) -> usize`, `get(&self, Place) -> Option<&Stack>`, `stacks(&self) -> Vec<(Place, Stack)>`, `is_empty(&self) -> bool`, `weight(&self, &Content) -> f32`, `count_of(&self, item: usize) -> u32`, `bonus(&self, &Content) -> Bonus`, `first_of(&self, ItemKind, &Content) -> Option<Place>`, `first_empty(&self, in_safe: bool) -> Option<Place>`, `add(&mut self, Stack, &Content) -> Option<Stack>`, `take(&mut self, Place) -> Option<Stack>`, `take_one(&mut self, Place) -> Option<Stack>`, `move_to(&mut self, Place, Place, &Content) -> Result<(), InvError>`, `split_for_death(&mut self, DeathLoss, &Content) -> Vec<Stack>`, `clear_found(&mut self)`, `sanitize(&mut self, &Content) -> Vec<Stack>`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/inventory.rs` starts with its types (Step 3 adds the code) and ends with these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentSources;

    fn content(run: &str) -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: &format!(
                r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                run: (backpack_slots: 3, safe_slots: 1, {run}))"#
            ),
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand), bonus: (damage: 0.2)),
                (key: "helm", name: "Helm", kind: Armour, icon: "a", weight: 2.0, slot: Some(Head),
                 bonus: (health: 50.0)),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 5),
                (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 2),
                (key: "sack", name: "Sack", kind: Bag, icon: "a", weight: 0.5, bag_slots: 1),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    const KNIFE: usize = 0;
    const HELM: usize = 1;
    const BREAD: usize = 2;
    const GEM: usize = 3;
    const POUCH: usize = 4;
    const SACK: usize = 5;

    #[test]
    fn stacks_merge_then_fill_empty_slots_and_overflow_comes_back() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        assert_eq!((inv.pack.len(), inv.safe.len()), (3, 1));
        assert_eq!(inv.add(Stack::new(BREAD, 3), &c), None);
        assert_eq!(inv.add(Stack::new(BREAD, 4), &c), None);
        assert_eq!(inv.get(Place::Pack(0)).unwrap().count, 5);
        assert_eq!(inv.get(Place::Pack(1)).unwrap().count, 2);
        // A found stack does not merge with a brought one.
        assert_eq!(inv.add(Stack::found(BREAD, 1), &c), None);
        assert!(inv.get(Place::Pack(2)).unwrap().found);
        let left = inv.add(Stack::new(GEM, 4), &c);
        assert_eq!(left, Some(Stack::new(GEM, 4)));
        assert_eq!(inv.count_of(BREAD), 8);
    }

    #[test]
    fn gear_goes_only_in_its_slot_and_swaps() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(KNIFE, 1), &c);
        inv.add(Stack::new(HELM, 1), &c);
        assert_eq!(
            inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::Head), &c),
            Err(InvError::WrongSlot)
        );
        inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::MainHand), &c).unwrap();
        inv.move_to(Place::Pack(1), Place::Equip(EquipSlot::Head), &c).unwrap();
        assert_eq!(inv.bonus(&c), Bonus { health: 50.0, damage: 0.2 });
        inv.add(Stack::new(BREAD, 1), &c);
        // Bread cannot go where the helm was, so the helm cannot swap with it.
        assert_eq!(
            inv.move_to(Place::Equip(EquipSlot::Head), Place::Pack(0), &c),
            Err(InvError::WrongSlot)
        );
        inv.move_to(Place::Equip(EquipSlot::Head), Place::Pack(1), &c).unwrap();
        assert_eq!(inv.get(Place::Pack(1)).unwrap().item, HELM as u16);
        assert!(inv.get(Place::Equip(EquipSlot::Head)).is_none());
    }

    #[test]
    fn a_bag_comes_off_only_when_its_things_fit() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(POUCH, 1), &c);
        inv.move_to(Place::Pack(0), Place::Bag(0), &c).unwrap();
        assert_eq!(inv.pack_size(&c), 5);
        assert_eq!(inv.pack.len(), 5);
        for item in [BREAD, GEM, KNIFE, HELM] {
            inv.add(Stack::new(item, 1), &c);
        }
        // Four stacks and the pouch itself would need 5 of the 3 slots left.
        assert_eq!(
            inv.move_to(Place::Bag(0), Place::Pack(4), &c),
            Err(InvError::BagInUse)
        );
        assert_eq!(inv.pack.len(), 5);
        assert!(inv.get(Place::Bag(0)).is_some());
        inv.take(Place::Pack(3));
        inv.take(Place::Pack(2));
        inv.move_to(Place::Bag(0), Place::Pack(4), &c).unwrap();
        assert_eq!(inv.pack.len(), 3);
        assert_eq!(inv.count_of(POUCH), 1);
        assert_eq!(inv.count_of(BREAD) + inv.count_of(GEM), 2);
        // Wearing the smaller sack where the pouch was: 4 slots, and the 3 stacks fit.
        inv.take(Place::Pack(0));
        inv.add(Stack::new(SACK, 1), &c);
        let sack = inv.pack.iter().position(|s| s.is_some_and(|s| s.item == SACK as u16));
        inv.move_to(Place::Pack(sack.unwrap() as u16), Place::Bag(1), &c).unwrap();
        assert_eq!(inv.pack.len(), 4);
        assert_eq!(inv.take(Place::Bag(1)), None, "a bag comes off only by moving it");
    }

    #[test]
    fn weight_counts_everything_carried() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(GEM, 4), &c);
        inv.add(Stack::new(KNIFE, 1), &c);
        inv.move_to(Place::Pack(1), Place::Equip(EquipSlot::MainHand), &c).unwrap();
        inv.add(Stack::new(BREAD, 2), &c);
        inv.move_to(Place::Pack(1), Place::Safe(0), &c).unwrap();
        assert!((inv.weight(&c) - (4.0 + 1.0 + 1.0)).abs() < 1e-5);
        assert_eq!(inv.first_of(ItemKind::Valuable, &c), Some(Place::Pack(0)));
        assert_eq!(inv.first_empty(false), Some(Place::Pack(1)));
        assert_eq!(inv.first_empty(true), None);
    }

    #[test]
    fn death_takes_what_its_mode_says_and_never_the_safe_slot() {
        let c = content("");
        let filled = || {
            let mut inv = Inventory::new(&c);
            inv.add(Stack::new(KNIFE, 1), &c);
            inv.move_to(Place::Pack(0), Place::Equip(EquipSlot::MainHand), &c).unwrap();
            inv.add(Stack::found(POUCH, 1), &c);
            inv.move_to(Place::Pack(0), Place::Bag(0), &c).unwrap();
            inv.add(Stack::new(BREAD, 2), &c);
            inv.add(Stack::found(GEM, 3), &c);
            inv.add(Stack::new(HELM, 1), &c);
            inv.add(Stack::new(SACK, 1), &c);
            inv.add(Stack::found(BREAD, 1), &c);
            inv.move_to(Place::Pack(4), Place::Safe(0), &c).unwrap();
            inv.add(Stack::new(KNIFE, 1), &c);
            inv
        };
        let mut all = filled();
        let lost = all.split_for_death(DeathLoss::Everything, &c);
        assert_eq!(lost.len(), 7);
        assert_eq!(all.stacks(), vec![(Place::Safe(0), Stack::found(BREAD, 1))]);
        assert_eq!(all.pack.len(), 3);
        let mut found = filled();
        let lost = found.split_for_death(DeathLoss::FoundOnly, &c);
        // The pouch and the gem were found; with the pouch gone, the four brought stacks left in
        // the pack must fit 3 slots: the last (the second knife) goes too.
        assert!(lost.contains(&Stack::found(POUCH, 1)));
        assert!(lost.contains(&Stack::found(GEM, 3)));
        assert_eq!(lost.len(), 3);
        assert_eq!(found.count_of(KNIFE), 1);
        assert_eq!(found.get(Place::Safe(0)), Some(&Stack::found(BREAD, 1)));
        // Nothing made, nothing lost: 8 stacks before, 8 between what is kept and the body.
        assert_eq!(found.stacks().len() + lost.len(), 8);
    }

    #[test]
    fn sanitize_returns_what_has_no_room() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.equip[EquipSlot::Head.index()] = Some(Stack::new(KNIFE, 1));
        inv.bags[0] = Some(Stack::new(GEM, 1));
        inv.pack = vec![
            Some(Stack::new(BREAD, 99)),
            Some(Stack::new(42, 1)),
            Some(Stack::new(GEM, 0)),
            Some(Stack::new(HELM, 1)),
            Some(Stack::new(GEM, 2)),
        ];
        inv.safe = vec![Some(Stack::new(GEM, 1)), Some(Stack::new(BREAD, 1))];
        let left = inv.sanitize(&c);
        assert!(inv.get(Place::Equip(EquipSlot::Head)).is_none());
        assert!(inv.bags.iter().all(Option::is_none));
        assert_eq!(inv.pack.len(), 3);
        assert_eq!(inv.safe.len(), 1);
        assert_eq!(inv.get(Place::Pack(0)), Some(&Stack::new(BREAD, 5)));
        assert_eq!(inv.get(Place::Safe(0)), Some(&Stack::new(GEM, 1)));
        // The pack keeps bread, helm and gems; the second safe stack, the misplaced knife and
        // the bag-slot gem have no room left.
        assert_eq!(left.len(), 3);
        assert!(left.contains(&Stack::new(KNIFE, 1)));
    }

    #[test]
    fn a_failed_move_changes_nothing_and_take_one_takes_one() {
        let c = content("");
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(BREAD, 3), &c);
        let before = inv.clone();
        assert_eq!(inv.move_to(Place::Pack(1), Place::Pack(0), &c), Err(InvError::Empty));
        assert_eq!(inv.move_to(Place::Pack(0), Place::Pack(9), &c), Err(InvError::NoSuchPlace));
        assert_eq!(inv, before);
        assert_eq!(inv.take_one(Place::Pack(0)), Some(Stack::new(BREAD, 1)));
        assert_eq!(inv.get(Place::Pack(0)).unwrap().count, 2);
        inv.clear_found();
        assert_eq!(inv.take(Place::Pack(0)), Some(Stack::new(BREAD, 2)));
        assert!(inv.is_empty());
    }
}
```

Add `mod inventory;` and `pub use inventory::{InvError, Inventory, Place, Stack, MAX_BAGS};` to `lib.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim inventory`
Expected: FAIL to compile (`cannot find type Inventory`).

- [ ] **Step 3: Write the inventory**

Above the tests in `inventory.rs`:

```rust
//! What a hero carries (spec §8.2): what they wear, the bags that widen their backpack, the
//! backpack's slots, and the safe slots nothing takes. Pure, so the host's raid and the stash
//! between raids hold things by the same rules.

use serde::{Deserialize, Serialize};

use crate::{Bonus, Content, DeathLoss, EquipSlot, ItemKind};

/// Bags a hero can wear at once.
pub const MAX_BAGS: usize = 4;

/// Some of one item; `found` marks what was picked up this raid (spec §8.2's `found_only`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    /// Index into [`Content::items`].
    pub item: u16,
    pub count: u16,
    pub found: bool,
}

impl Stack {
    /// `count` of `item`, brought.
    pub fn new(item: usize, count: u16) -> Self {
        Self {
            item: item as u16,
            count,
            found: false,
        }
    }

    /// `count` of `item`, found this raid.
    pub fn found(item: usize, count: u16) -> Self {
        Self {
            found: true,
            ..Self::new(item, count)
        }
    }
}

/// Where in an inventory a stack sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    Equip(EquipSlot),
    Bag(u8),
    Pack(u16),
    Safe(u8),
}

/// Why a move was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvError {
    /// Nothing there.
    Empty,
    /// No such slot.
    NoSuchPlace,
    /// It does not go there (a helm in the boots' slot, a sword in a bag slot).
    WrongSlot,
    /// No room for more of it there.
    Full,
    /// Taking the bag off would leave its things nowhere.
    BagInUse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    /// By [`EquipSlot::index`].
    pub equip: [Option<Stack>; EquipSlot::COUNT],
    pub bags: [Option<Stack>; MAX_BAGS],
    /// As many slots as [`Inventory::pack_size`].
    pub pack: Vec<Option<Stack>>,
    pub safe: Vec<Option<Stack>>,
}

impl Inventory {
    /// Empty, with the rules' backpack and safe slots.
    pub fn new(content: &Content) -> Self {
        let r = &content.rules.run;
        Self {
            equip: [None; EquipSlot::COUNT],
            bags: [None; MAX_BAGS],
            pack: vec![None; usize::from(r.backpack_slots)],
            safe: vec![None; usize::from(r.safe_slots)],
        }
    }

    /// The backpack's slots with the bags worn now.
    pub fn pack_size(&self, content: &Content) -> usize {
        let bags: usize = self
            .bags
            .iter()
            .flatten()
            .map(|b| usize::from(content.items[usize::from(b.item)].bag_slots))
            .sum();
        usize::from(content.rules.run.backpack_slots) + bags
    }

    pub fn get(&self, at: Place) -> Option<&Stack> {
        match at {
            Place::Equip(s) => self.equip[s.index()].as_ref(),
            Place::Bag(i) => self.bags.get(usize::from(i))?.as_ref(),
            Place::Pack(i) => self.pack.get(usize::from(i))?.as_ref(),
            Place::Safe(i) => self.safe.get(usize::from(i))?.as_ref(),
        }
    }

    fn slot(&mut self, at: Place) -> Option<&mut Option<Stack>> {
        match at {
            Place::Equip(s) => Some(&mut self.equip[s.index()]),
            Place::Bag(i) => self.bags.get_mut(usize::from(i)),
            Place::Pack(i) => self.pack.get_mut(usize::from(i)),
            Place::Safe(i) => self.safe.get_mut(usize::from(i)),
        }
    }

    /// Every stack and where it sits: worn, bags, backpack, safe slots.
    pub fn stacks(&self) -> Vec<(Place, Stack)> {
        let mut out = Vec::new();
        for s in EquipSlot::ALL {
            if let Some(st) = self.equip[s.index()] {
                out.push((Place::Equip(s), st));
            }
        }
        for (i, b) in self.bags.iter().enumerate() {
            if let Some(st) = b {
                out.push((Place::Bag(i as u8), *st));
            }
        }
        for (i, p) in self.pack.iter().enumerate() {
            if let Some(st) = p {
                out.push((Place::Pack(i as u16), *st));
            }
        }
        for (i, p) in self.safe.iter().enumerate() {
            if let Some(st) = p {
                out.push((Place::Safe(i as u8), *st));
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.stacks().is_empty()
    }

    /// Everything carried, weighed (spec §8.2).
    pub fn weight(&self, content: &Content) -> f32 {
        self.stacks()
            .iter()
            .map(|(_, s)| content.items[usize::from(s.item)].weight * f32::from(s.count))
            .sum()
    }

    pub fn count_of(&self, item: usize) -> u32 {
        self.stacks()
            .iter()
            .filter(|(_, s)| usize::from(s.item) == item)
            .map(|(_, s)| u32::from(s.count))
            .sum()
    }

    /// What the worn gear adds up to.
    pub fn bonus(&self, content: &Content) -> Bonus {
        let mut b = Bonus::default();
        for s in self.equip.iter().flatten() {
            let add = &content.items[usize::from(s.item)].bonus;
            b.health += add.health;
            b.damage += add.damage;
        }
        b
    }

    /// The first backpack slot holding something of `kind`.
    pub fn first_of(&self, kind: ItemKind, content: &Content) -> Option<Place> {
        self.pack
            .iter()
            .position(|s| s.is_some_and(|s| content.items[usize::from(s.item)].kind == kind))
            .map(|i| Place::Pack(i as u16))
    }

    /// The first empty backpack slot, or safe slot.
    pub fn first_empty(&self, in_safe: bool) -> Option<Place> {
        if in_safe {
            self.safe
                .iter()
                .position(Option::is_none)
                .map(|i| Place::Safe(i as u8))
        } else {
            self.pack
                .iter()
                .position(Option::is_none)
                .map(|i| Place::Pack(i as u16))
        }
    }

    /// `stack` into the backpack: onto stacks of the same item and the same `found` first, then
    /// into empty slots. What does not fit comes back.
    pub fn add(&mut self, mut stack: Stack, content: &Content) -> Option<Stack> {
        let max = content.items[usize::from(stack.item)].stack.max(1);
        for s in self.pack.iter_mut().flatten() {
            if s.item == stack.item && s.found == stack.found && s.count < max {
                let n = (max - s.count).min(stack.count);
                s.count += n;
                stack.count -= n;
                if stack.count == 0 {
                    return None;
                }
            }
        }
        for s in self.pack.iter_mut() {
            if s.is_none() {
                let n = stack.count.min(max);
                *s = Some(Stack { count: n, ..stack });
                stack.count -= n;
                if stack.count == 0 {
                    return None;
                }
            }
        }
        Some(stack)
    }

    /// The whole stack at `at`. A bag comes off only by [`Inventory::move_to`], which minds its
    /// things.
    pub fn take(&mut self, at: Place) -> Option<Stack> {
        if matches!(at, Place::Bag(_)) {
            return None;
        }
        self.slot(at)?.take()
    }

    /// One of the stack at `at`.
    pub fn take_one(&mut self, at: Place) -> Option<Stack> {
        if matches!(at, Place::Bag(_)) {
            return None;
        }
        let slot = self.slot(at)?;
        let s = slot.as_mut()?;
        let one = Stack { count: 1, ..*s };
        s.count -= 1;
        if s.count == 0 {
            *slot = None;
        }
        Some(one)
    }

    /// Whether `stack` may sit at `at`.
    fn fits(at: Place, stack: &Stack, content: &Content) -> bool {
        let def = &content.items[usize::from(stack.item)];
        match at {
            Place::Equip(s) => def.slot == Some(s),
            Place::Bag(_) => def.kind == ItemKind::Bag,
            Place::Pack(_) | Place::Safe(_) => true,
        }
    }

    /// Move the stack at `from` to `to`: onto a stack of the same item, or swapping with what is
    /// there. Wearing or taking off a bag resizes the backpack. Nothing changes when the move
    /// cannot be made.
    pub fn move_to(&mut self, from: Place, to: Place, content: &Content) -> Result<(), InvError> {
        if from == to {
            return Ok(());
        }
        let before = self.clone();
        let done = self.try_move(from, to, content);
        if done.is_err() {
            *self = before;
        }
        done
    }

    fn try_move(&mut self, from: Place, to: Place, content: &Content) -> Result<(), InvError> {
        if self.slot(to).is_none() {
            return Err(InvError::NoSuchPlace);
        }
        let moving = *self.get(from).ok_or(InvError::Empty)?;
        let there = self.get(to).copied();
        let max = content.items[usize::from(moving.item)].stack.max(1);
        if let Some(t) = there {
            let pile = matches!(to, Place::Pack(_) | Place::Safe(_));
            if pile && t.item == moving.item && t.found == moving.found && max > 1 {
                let n = (max - t.count).min(moving.count);
                if n == 0 {
                    return Err(InvError::Full);
                }
                *self.slot(to).expect("checked") = Some(Stack {
                    count: t.count + n,
                    ..t
                });
                let left = moving.count - n;
                *self.slot(from).expect("held a stack") =
                    (left > 0).then_some(Stack { count: left, ..moving });
                return Ok(());
            }
        }
        if !Self::fits(to, &moving, content) {
            return Err(InvError::WrongSlot);
        }
        if let Some(t) = there {
            if !Self::fits(from, &t, content) {
                return Err(InvError::WrongSlot);
            }
        }
        *self.slot(to).expect("checked") = Some(moving);
        *self.slot(from).expect("held a stack") = there;
        if matches!(from, Place::Bag(_)) || matches!(to, Place::Bag(_)) {
            self.fit_pack(content)?;
        }
        Ok(())
    }

    /// The backpack resized to its bags: grown with empty slots, or shrunk by first dropping
    /// empty slots at its end, then packing its stacks to the front; refused when they would not
    /// fit.
    fn fit_pack(&mut self, content: &Content) -> Result<(), InvError> {
        let size = self.pack_size(content);
        while self.pack.len() > size && self.pack.last() == Some(&None) {
            self.pack.pop();
        }
        if self.pack.len() > size {
            let stacks: Vec<Stack> = self.pack.iter().flatten().copied().collect();
            if stacks.len() > size {
                return Err(InvError::BagInUse);
            }
            self.pack = stacks.into_iter().map(Some).collect();
        }
        self.pack.resize(size, None);
        Ok(())
    }

    /// Death (spec §8.2): the stacks a death takes, by `mode`, out of the inventory; the safe
    /// slots are always kept. Under `found_only` what was brought stays, but a brought thing
    /// that has no slot left once a found bag is gone goes with the body too.
    pub fn split_for_death(&mut self, mode: DeathLoss, content: &Content) -> Vec<Stack> {
        let lose = |s: &Stack| match mode {
            DeathLoss::Everything | DeathLoss::EverythingOneRecovery => true,
            DeathLoss::FoundOnly => s.found,
        };
        let mut lost = Vec::new();
        for slot in self
            .equip
            .iter_mut()
            .chain(self.bags.iter_mut())
            .chain(self.pack.iter_mut())
        {
            if slot.as_ref().is_some_and(|s| lose(s)) {
                lost.extend(slot.take());
            }
        }
        let size = self.pack_size(content);
        let kept: Vec<Stack> = self.pack.iter().flatten().copied().collect();
        self.pack = vec![None; size];
        for (i, s) in kept.into_iter().enumerate() {
            if i < size {
                self.pack[i] = Some(s);
            } else {
                lost.push(s);
            }
        }
        lost
    }

    /// Out of the raid: what was found is now simply carried.
    pub fn clear_found(&mut self) {
        for s in self
            .equip
            .iter_mut()
            .chain(self.bags.iter_mut())
            .chain(self.pack.iter_mut())
            .chain(self.safe.iter_mut())
            .flatten()
        {
            s.found = false;
        }
    }

    /// An inventory from elsewhere (a client's loadout, an old stash file) made right for this
    /// content: unknown items and empty stacks go, counts are held to the stack size, things in
    /// slots they do not fit move to the backpack, and the backpack and safe slots take the
    /// rules' sizes. What has no room is returned, so the caller decides (a host leaves it out;
    /// the stash keeps it).
    pub fn sanitize(&mut self, content: &Content) -> Vec<Stack> {
        let items = &content.items;
        let clean = |s: Stack| -> Option<Stack> {
            let def = items.get(usize::from(s.item))?;
            (s.count > 0).then(|| Stack {
                count: s.count.min(def.stack.max(1)),
                ..s
            })
        };
        let mut loose = Vec::new();
        for slot in EquipSlot::ALL {
            if let Some(st) = self.equip[slot.index()].take().and_then(clean) {
                if items[usize::from(st.item)].slot == Some(slot) {
                    self.equip[slot.index()] = Some(st);
                } else {
                    loose.push(st);
                }
            }
        }
        for b in self.bags.iter_mut() {
            if let Some(st) = b.take().and_then(clean) {
                if items[usize::from(st.item)].kind == ItemKind::Bag {
                    *b = Some(st);
                } else {
                    loose.push(st);
                }
            }
        }
        let pack: Vec<Stack> = self.pack.drain(..).flatten().filter_map(clean).collect();
        let safe: Vec<Stack> = self.safe.drain(..).flatten().filter_map(clean).collect();
        let safe_size = usize::from(content.rules.run.safe_slots);
        self.safe = vec![None; safe_size];
        let mut out = Vec::new();
        for (i, s) in safe.into_iter().enumerate() {
            if i < safe_size {
                self.safe[i] = Some(s);
            } else {
                out.push(s);
            }
        }
        let size = self.pack_size(content);
        self.pack = vec![None; size];
        for (i, s) in pack.into_iter().chain(loose).enumerate() {
            if i < size {
                self.pack[i] = Some(s);
            } else {
                out.push(s);
            }
        }
        out
    }
}
```

In `sanitize_returns_what_has_no_room` the order the leftovers arrive in is safe overflow first, then the pack's own overflow, then the loose ones; the test checks only their count and the knife, so a different order of that tail stays correct.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim inventory`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/inventory.rs crates/nightfall-sim/src/lib.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the inventory: backpack, bags, gear, safe slots, weight, death

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 3: The world carries: loadouts, acts taken once, weight, gear, lanterns, potions

**Files:**
- Create: `crates/nightfall-sim/src/raid/mod.rs`, `crates/nightfall-sim/src/raid/acts.rs`, `crates/nightfall-sim/src/raid/containers.rs`, `crates/nightfall-sim/src/raid/tests.rs`
- Modify: `crates/nightfall-sim/src/world.rs` (`Hero`, `PlayerInput`, `Snapshot`, `World`, `new`, `add_hero`, `remove_hero`, `step`, `snapshot`, `snapshot_for`, `apply_snapshot`), `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/src/senses/mod.rs` (`step_heroes`), `crates/nightfall-sim/src/combat/effects.rs` (`strike`)
- Modify (literals): every other `Hero { … }`, `Snapshot { … }` and full `PlayerInput { … }` literal in `crates/nightfall-sim` and `crates/benilla-app/src/nightfall`

**Interfaces:**
- Consumes: `Inventory`, `Stack`, `Place` (Task 2); `Content::{items, loot, rules.run, classes}`, `ItemKind`, `ItemUse`, `EquipSlot`, `RunRules`, `DeathLoss` (Task 1); `World::{hero_slot, make_noise, content, hero, heroes, elapsed}`, `Hero::{fight, z}`, `HeroFight::{up, power, max_power, drinks, life}`, `Life`, `Loudness`, `Gait`, `HeroPose`, `testmap::flat` (earlier plans).
- Produces:
  - `nightfall_sim::{Carry, Act, ActPress, Holder, ContainerMark, OpenView, ExitMark, LootView, Recovery, Loadout, Fate, HeroOutcome, ExitPlan, RaidRoll, burden}`.
  - `Carry { load: f32, lantern: Option<u16>, worn: Vec<(EquipSlot, u16)>, extracting: f32, acted: u8 }`; `Hero::carry: Carry`.
  - `Act::{Open(u32), Close, Take(u16), TakeAll, Put(Place), Move { from: Place, to: Place }, Drop(Place), Use(Place)}`; `ActPress { seq: u8, act: Act }`; `PlayerInput::act: Option<ActPress>`.
  - `Holder::{Placed(u16), Body(u32), Hero(PlayerId), Remains(PlayerId), Pile}`; `ContainerMark { id: u32, holder: Holder, at: V2, z: Option<f32>, searched: bool, empty: bool, locked: bool }`; `OpenView { id: u32, stacks: Vec<Option<Stack>>, search: Option<(f32, f32)> }`; `ExitMark { exit: u8, open: bool, opens_in: Option<f32> }`; `LootView { marks, open, exits }`.
  - `Recovery { map: String, at: V3, items: Vec<Stack> }`; `Loadout { inventory: Inventory, recovery: Option<Recovery> }`; `Fate::{Extracted { exit: String }, Died, Missing}`; `HeroOutcome { player, name, fate, time: f32, kills: u32, carried: Inventory, recovered: Vec<Stack>, recovery: Option<Recovery> }`.
  - `ExitPlan { open: bool, opens_at: f32 }`; `RaidRoll { seed: u64, entry: usize, exits: Vec<ExitPlan> }` with `RaidRoll::standard(&Content, seed: u64) -> RaidRoll` (sub-project 6 rolls its own).
  - `burden(load: f32, &RunRules) -> (f32, f32)` (pace, footstep loudness).
  - `Snapshot::{pack: Option<(PlayerId, Inventory)>, loot: LootView}`.
  - `World::{begin_raid(&mut self, RaidRoll) -> Result<(), String>, give_loadout(&mut self, PlayerId, &Loadout) -> Vec<Stack>, pack(&self, PlayerId) -> Option<&Inventory>, loot_view(&self, Option<PlayerId>) -> LootView}`; crate-only `World::{take_act, refresh_carry, gear_damage, raid_leave}`, `World::raid: Raid`.
  - Crate-only `raid::{Raid, Container, Search, Exiting, in_reach, search_of, REACH_UP}`; `Raid::{new, pack, pack_mut, take_pack, last_act, set_last_act, potion_ready, set_potion_ready, open_of, close, recover, take_recovered, add_kill, kills_of, forget, open_view, add_container, drop_on_ground}`.
  - Test-only `raid::tests::{world, world_with, BRUTE_FAR, item, join, stand, walk, act, steps, loadout}`.

- [ ] **Step 1: Write the raid's types**

`crates/nightfall-sim/src/raid/mod.rs`:

```rust
//! The raid (spec §8): what each hero carries and does with it, the containers on the map and
//! what they hold, the exits and extracting, deaths and what they cost, and each hero's outcome.
//! The host's: a client sees its own backpack, the containers near it and the one it has open
//! ([`LootView`]) in its snapshot, and is told its outcome.

mod acts;
mod containers;
#[cfg(test)]
mod tests;

pub(crate) use containers::{in_reach, search_of};

use serde::{Deserialize, Serialize};

use crate::{Content, EquipSlot, Inventory, Place, PlayerId, RunRules, Stack, World, V2, V3};

/// What a hero carries, as every client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Carry {
    /// The weight carried (spec §8.2).
    pub load: f32,
    /// The lit lantern, an index into the content's items.
    pub lantern: Option<u16>,
    /// What they wear, for every client to dress them in.
    pub worn: Vec<(EquipSlot, u16)>,
    /// Seconds in an open exit's ring (spec §8.1).
    pub extracting: f32,
    /// The last act the host took from them ([`ActPress::seq`]).
    pub acted: u8,
}

/// Something a player does with what they carry or find.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    /// Search a container within reach, or open one already searched.
    Open(u32),
    /// Stop searching, and close what is open.
    Close,
    /// The stack in this slot of the open container, into the backpack.
    Take(u16),
    /// Everything in the open container that fits.
    TakeAll,
    /// The stack at this place into the open container.
    Put(Place),
    Move { from: Place, to: Place },
    /// The stack at this place onto the ground, where friends can pick it up.
    Drop(Place),
    /// Light or put out a lantern, drink a potion.
    Use(Place),
}

/// An act, repeated in every input until the host takes it ([`Carry::acted`] says when); `seq`
/// tells it from the last one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActPress {
    pub seq: u8,
    pub act: Act,
}

/// What holds a container's things.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Holder {
    /// A chest, crate or grave the content placed: its kind, an index into the loot's kinds.
    Placed(u16),
    /// A dead enemy, by id.
    Body(u32),
    /// A fallen hero.
    Hero(PlayerId),
    /// A hero's body from their last raid (spec §8.2's one recovery), seen only by them.
    Remains(PlayerId),
    /// Things dropped on the ground.
    Pile,
}

/// A container near the player, for drawing and clicking.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerMark {
    pub id: u32,
    pub holder: Holder,
    pub at: V2,
    pub z: Option<f32>,
    pub searched: bool,
    pub empty: bool,
    pub locked: bool,
}

/// The container a player has open, or is searching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenView {
    pub id: u32,
    /// Its slots, once searched.
    pub stacks: Vec<Option<Stack>>,
    /// While searching: seconds left, and the whole search.
    pub search: Option<(f32, f32)>,
}

/// An exit as the raid has it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExitMark {
    /// Index into the map's exits.
    pub exit: u8,
    pub open: bool,
    /// Seconds until it opens; `None` when it stays shut this raid.
    pub opens_in: Option<f32>,
}

/// What a player sees of the raid's loot: the containers near them, the one they have open, the
/// exits.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LootView {
    pub marks: Vec<ContainerMark>,
    pub open: Option<OpenView>,
    pub exits: Vec<ExitMark>,
}

/// A body left for its player's next raid on the map (spec §8.2, `everything_one_recovery`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recovery {
    /// The map's key.
    pub map: String,
    pub at: V3,
    pub items: Vec<Stack>,
}

/// What a player brings into a raid (spec §8.3: the host trusts it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loadout {
    pub inventory: Inventory,
    /// Their remains from the last raid, when there are any.
    pub recovery: Option<Recovery>,
}

/// How a hero's raid ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fate {
    /// Out, by this exit.
    Extracted { exit: String },
    Died,
    /// The raid ended around them (it was abandoned): counted as a death with no body.
    Missing,
}

/// One hero's raid, as its end screen and its player's stash take it (spec §8.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeroOutcome {
    pub player: PlayerId,
    pub name: String,
    pub fate: Fate,
    /// Seconds into the raid it ended for them.
    pub time: f32,
    pub kills: u32,
    /// What they keep: everything carried out, or what a death left them.
    pub carried: Inventory,
    /// What they took back from their remains, for the stash.
    pub recovered: Vec<Stack>,
    /// The body they left for their next raid.
    pub recovery: Option<Recovery>,
}

/// One exit in one raid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExitPlan {
    pub open: bool,
    /// Seconds into the raid it opens.
    pub opens_at: f32,
}

/// What makes this raid this raid (spec §8.1): the seed its loot is rolled from, the entry, and
/// which exits open when. Sub-project 6 rolls the exits, the weather and the signature monsters
/// here; until then [`RaidRoll::standard`] is every raid.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RaidRoll {
    pub seed: u64,
    /// Index into the map's entries.
    pub entry: usize,
    /// One per map exit.
    pub exits: Vec<ExitPlan>,
}

impl RaidRoll {
    /// The first entry, every exit open, the late ones at `cart_opens_at`.
    pub fn standard(content: &Content, seed: u64) -> Self {
        let cart = content.rules.run.cart_opens_at;
        Self {
            seed,
            entry: 0,
            exits: content
                .map
                .exits
                .iter()
                .map(|e| ExitPlan {
                    open: true,
                    opens_at: if e.late { cart } else { 0.0 },
                })
                .collect(),
        }
    }
}

/// Weight's cost (spec §8.2): the share of normal speed, and how many times further footsteps
/// carry. Nothing up to `weight_free`, all of it from `weight_full`, in a straight line between;
/// the floor is above 0, so a hero is never stuck.
pub fn burden(load: f32, r: &RunRules) -> (f32, f32) {
    let over = ((load - r.weight_free) / (r.weight_full - r.weight_free)).clamp(0.0, 1.0);
    (
        1.0 - over * (1.0 - r.weight_slow_floor),
        1.0 + over * (r.weight_noise_max - 1.0),
    )
}

/// A container in the host's world.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Container {
    pub(crate) id: u32,
    pub(crate) holder: Holder,
    pub(crate) at: V2,
    pub(crate) z: Option<f32>,
    pub(crate) stacks: Vec<Option<Stack>>,
    pub(crate) searched: bool,
    /// The key item it needs.
    pub(crate) lock: Option<u16>,
}

/// A search under way.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Search {
    pub(crate) player: PlayerId,
    pub(crate) id: u32,
    pub(crate) left: f32,
    pub(crate) total: f32,
    /// Where the hero stood as it began: walking off stops it.
    pub(crate) from: V2,
    /// Seconds to the next time it is heard.
    pub(crate) noise_in: f32,
}

/// A hero's extract count.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Exiting {
    pub(crate) player: PlayerId,
    pub(crate) t: f32,
    /// Health at the last step: less now is a blow, which starts the count over.
    pub(crate) last_hp: f32,
}

/// The raid's host-only state: never sent whole.
#[derive(Clone, Debug, Default)]
pub(crate) struct Raid {
    pub(crate) roll: RaidRoll,
    pub(crate) packs: Vec<(PlayerId, Inventory)>,
    pub(crate) containers: Vec<Container>,
    pub(crate) next_id: u32,
    pub(crate) searches: Vec<Search>,
    pub(crate) opened: Vec<(PlayerId, u32)>,
    /// Each player's last act taken.
    pub(crate) acts: Vec<(PlayerId, u8)>,
    /// When each player may drink a potion again.
    pub(crate) potions: Vec<(PlayerId, f32)>,
    pub(crate) exiting: Vec<Exiting>,
    pub(crate) kills: Vec<(PlayerId, u32)>,
    /// Enemies whose body has been made.
    pub(crate) bodied: Vec<u32>,
    /// Fallen heroes and their body's container (0 for none).
    pub(crate) fallen: Vec<(PlayerId, u32)>,
    /// What each player took back from their remains.
    pub(crate) recovered: Vec<(PlayerId, Vec<Stack>)>,
    pub(crate) outcomes: Vec<HeroOutcome>,
    /// Outcomes before this index have been handed out.
    pub(crate) taken: usize,
    pub(crate) had_heroes: bool,
    pub(crate) over: bool,
    /// A client's mirror: the loot view its host sent.
    pub(crate) mirror: Option<LootView>,
}

impl Raid {
    pub(crate) fn pack(&self, player: PlayerId) -> Option<&Inventory> {
        self.packs.iter().find(|(p, _)| *p == player).map(|(_, i)| i)
    }

    /// `player`'s inventory, an empty one made if they have none yet.
    pub(crate) fn pack_mut(&mut self, player: PlayerId, content: &Content) -> &mut Inventory {
        let i = match self.packs.iter().position(|(p, _)| *p == player) {
            Some(i) => i,
            None => {
                self.packs.push((player, Inventory::new(content)));
                self.packs.len() - 1
            }
        };
        &mut self.packs[i].1
    }

    pub(crate) fn take_pack(&mut self, player: PlayerId) -> Option<Inventory> {
        let i = self.packs.iter().position(|(p, _)| *p == player)?;
        Some(self.packs.remove(i).1)
    }

    pub(crate) fn last_act(&self, player: PlayerId) -> Option<u8> {
        self.acts.iter().find(|(p, _)| *p == player).map(|(_, s)| *s)
    }

    pub(crate) fn set_last_act(&mut self, player: PlayerId, seq: u8) {
        match self.acts.iter_mut().find(|(p, _)| *p == player) {
            Some(a) => a.1 = seq,
            None => self.acts.push((player, seq)),
        }
    }

    pub(crate) fn potion_ready(&self, player: PlayerId) -> f32 {
        self.potions
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(0.0, |(_, t)| *t)
    }

    pub(crate) fn set_potion_ready(&mut self, player: PlayerId, at: f32) {
        match self.potions.iter_mut().find(|(p, _)| *p == player) {
            Some(t) => t.1 = at,
            None => self.potions.push((player, at)),
        }
    }

    pub(crate) fn open_of(&self, player: PlayerId) -> Option<u32> {
        self.opened.iter().find(|(p, _)| *p == player).map(|(_, id)| *id)
    }

    /// `player` stops searching, and closes what they have open.
    pub(crate) fn close(&mut self, player: PlayerId) {
        self.opened.retain(|(p, _)| *p != player);
        self.searches.retain(|s| s.player != player);
    }

    pub(crate) fn recover(&mut self, player: PlayerId, stacks: Vec<Stack>) {
        match self.recovered.iter_mut().find(|(p, _)| *p == player) {
            Some(r) => r.1.extend(stacks),
            None => self.recovered.push((player, stacks)),
        }
    }

    pub(crate) fn take_recovered(&mut self, player: PlayerId) -> Vec<Stack> {
        match self.recovered.iter().position(|(p, _)| *p == player) {
            Some(i) => self.recovered.remove(i).1,
            None => Vec::new(),
        }
    }

    pub(crate) fn add_kill(&mut self, player: PlayerId) {
        match self.kills.iter_mut().find(|(p, _)| *p == player) {
            Some(k) => k.1 += 1,
            None => self.kills.push((player, 1)),
        }
    }

    pub(crate) fn kills_of(&self, player: PlayerId) -> u32 {
        self.kills
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(0, |(_, k)| *k)
    }

    /// `player` is gone from the raid: nothing they were doing goes on.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.searches.retain(|s| s.player != player);
        self.opened.retain(|(p, _)| *p != player);
        self.acts.retain(|(p, _)| *p != player);
        self.potions.retain(|(p, _)| *p != player);
        self.exiting.retain(|e| e.player != player);
        self.fallen.retain(|(p, _)| *p != player);
    }

    /// What `player` has open or is searching.
    pub(crate) fn open_view(&self, player: PlayerId) -> Option<OpenView> {
        if let Some(s) = self.searches.iter().find(|s| s.player == player) {
            return Some(OpenView {
                id: s.id,
                stacks: Vec::new(),
                search: Some((s.left.max(0.0), s.total)),
            });
        }
        let id = self.open_of(player)?;
        let c = self.containers.iter().find(|c| c.id == id)?;
        Some(OpenView {
            id,
            stacks: c.stacks.clone(),
            search: None,
        })
    }
}

impl World {
    /// This raid is `roll`'s: its entry, its exits, its loot rolled from its seed. Called before
    /// the heroes are added, so they start at its entry.
    pub fn begin_raid(&mut self, roll: RaidRoll) -> Result<(), String> {
        let content = self.content().clone();
        if roll.entry >= content.map.entries.len() {
            return Err(format!("no entry {} on {}", roll.entry, content.map.key));
        }
        if roll.exits.len() != content.map.exits.len() {
            return Err(format!(
                "{} exits planned for {}'s {}",
                roll.exits.len(),
                content.map.key,
                content.map.exits.len()
            ));
        }
        self.raid = Raid::new(&content, roll);
        Ok(())
    }

    /// `player`, already added, brings `loadout` (spec §8.3: the host trusts it, once it is made
    /// right for this content): their gear, backpack and safe slots, full health with what they
    /// wear, and their remains from their last raid when it was on this map. What has no room is
    /// returned, and left out.
    pub fn give_loadout(&mut self, player: PlayerId, loadout: &Loadout) -> Vec<Stack> {
        let content = self.content().clone();
        let mut inv = loadout.inventory.clone();
        let left = inv.sanitize(&content);
        *self.raid.pack_mut(player, &content) = inv;
        let remains = loadout
            .recovery
            .as_ref()
            .filter(|r| r.map == content.map.key && r.at.is_finite());
        if let Some(r) = remains {
            let items: Vec<Stack> = r
                .items
                .iter()
                .copied()
                .filter(|s| usize::from(s.item) < content.items.len() && s.count > 0)
                .collect();
            let at = content.map.bounds.clamp(r.at.ground());
            let slots = items.len();
            self.raid
                .add_container(Holder::Remains(player), at, Some(r.at.z), items, slots, true, None);
        }
        self.refresh_carry(player);
        if let Some(hi) = self.hero_slot(player) {
            let h = &mut self.heroes[hi];
            h.hp = h.max_hp;
        }
        left
    }

    /// `player`'s inventory: the host's, or on a client's mirror its own as the host sent it.
    pub fn pack(&self, player: PlayerId) -> Option<&Inventory> {
        self.raid.pack(player)
    }

    /// The raid's loot as `player` sees it: the containers within the view distance of their hero
    /// (their own remains, nobody else's), the one they have open, and the exits; every
    /// container for `None`. A client's mirror answers with what its host sent.
    pub fn loot_view(&self, player: Option<PlayerId>) -> LootView {
        if let Some(view) = &self.raid.mirror {
            return view.clone();
        }
        let content = self.content();
        let near = player.and_then(|p| self.hero(p)).map(|h| h.pos);
        let reach = content.rules.atmosphere.view_distance;
        let marks = self
            .raid
            .containers
            .iter()
            .filter(|c| match c.holder {
                Holder::Remains(owner) => player.is_none_or(|p| p == owner),
                _ => true,
            })
            .filter(|c| near.is_none_or(|p| p.distance(c.at) <= reach))
            .map(|c| ContainerMark {
                id: c.id,
                holder: c.holder,
                at: c.at,
                z: c.z,
                searched: c.searched,
                empty: c.stacks.iter().all(Option::is_none),
                locked: c.lock.is_some(),
            })
            .collect();
        let open = player.and_then(|p| self.raid.open_view(p));
        let exits = (0..content.map.exits.len())
            .map(|i| {
                let plan = self.raid.roll.exits.get(i).copied().unwrap_or(ExitPlan {
                    open: false,
                    opens_at: 0.0,
                });
                ExitMark {
                    exit: i as u8,
                    open: plan.open && self.elapsed >= plan.opens_at,
                    opens_in: plan.open.then(|| (plan.opens_at - self.elapsed).max(0.0)),
                }
            })
            .collect();
        LootView { marks, open, exits }
    }

    /// What `player`'s worn gear multiplies their blows and spells by.
    pub(crate) fn gear_damage(&self, player: PlayerId) -> f32 {
        let content = self.content();
        1.0 + self
            .raid
            .pack(player)
            .map_or(0.0, |i| i.bonus(content).damage)
    }

    /// `player` leaves the raid without an outcome (they left the session): what a death would
    /// take is dropped where they stood, for their friends, and nothing of theirs goes on.
    pub(crate) fn raid_leave(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let at = self.hero(player).map(|h| V3::at(h.pos, h.z));
        if let (Some(at), Some(mut inv)) = (at, self.raid.take_pack(player)) {
            for s in inv.split_for_death(content.rules.run.death_loss, &content) {
                self.raid.drop_on_ground(at, s, &content);
            }
        }
        self.raid.forget(player);
    }
}
```

`crates/nightfall-sim/src/raid/containers.rs` (Task 5 adds the tables, searching and bodies):

```rust
//! Containers: the chests, crates and graves the content places, the bodies enemies and heroes
//! leave, a hero's remains from their last raid, and things dropped on the ground.

use super::{Container, Holder, Raid, RaidRoll};
use crate::{Content, Loudness, Stack, V2, V3};

/// How far up or down a container may be from a hero and still be in reach, yards: a chest in a
/// crypt is not opened from the hill above it.
pub(crate) const REACH_UP: f32 = 3.0;
/// Things dropped this near a pile join it, yards.
const PILE_JOIN: f32 = 1.5;

/// Whether a hero at `pos` (feet at `z`) reaches a container at `at` (at height `cz`, when it is
/// known).
pub(crate) fn in_reach(pos: V2, z: f32, at: V2, cz: Option<f32>, reach: f32) -> bool {
    pos.distance(at) <= reach && cz.is_none_or(|cz| (z - cz).abs() <= REACH_UP)
}

/// How long searching what `holder` holds takes, and how loud it is (spec §5.2).
pub(crate) fn search_of(holder: Holder, content: &Content) -> (f32, Option<Loudness>) {
    match holder {
        Holder::Placed(k) => {
            let k = &content.loot.kinds[usize::from(k)];
            (k.search, Some(k.sound))
        }
        Holder::Body(_) | Holder::Hero(_) => {
            (content.rules.run.search_body, Some(Loudness::Open))
        }
        Holder::Remains(_) | Holder::Pile => (0.0, None),
    }
}

impl Raid {
    /// A raid as `roll` has it.
    pub(crate) fn new(_content: &Content, roll: RaidRoll) -> Self {
        Self {
            roll,
            next_id: 1,
            ..Self::default()
        }
    }

    /// A new container holding `stacks`, with at least `slots` slots; its id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_container(
        &mut self,
        holder: Holder,
        at: V2,
        z: Option<f32>,
        stacks: Vec<Stack>,
        slots: usize,
        searched: bool,
        lock: Option<u16>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut held: Vec<Option<Stack>> = stacks.into_iter().map(Some).collect();
        if held.len() < slots {
            held.resize(slots, None);
        }
        self.containers.push(Container {
            id,
            holder,
            at,
            z,
            stacks: held,
            searched,
            lock,
        });
        id
    }

    /// `stack` on the ground at `at`: onto a pile there with room, or a new pile.
    pub(crate) fn drop_on_ground(&mut self, at: V3, stack: Stack, content: &Content) {
        let here = at.ground();
        let near = self.containers.iter().position(|c| {
            c.holder == Holder::Pile
                && c.at.distance(here) <= PILE_JOIN
                && c.stacks.iter().any(Option::is_none)
        });
        match near {
            Some(i) => {
                let c = &mut self.containers[i];
                let free = c.stacks.iter().position(Option::is_none).expect("has room");
                c.stacks[free] = Some(stack);
            }
            None => {
                let slots = usize::from(content.rules.run.container_slots);
                self.add_container(Holder::Pile, here, Some(at.z), vec![stack], slots, true, None);
            }
        }
    }
}
```

In `crates/nightfall-sim/src/lib.rs` add `mod raid;` and:

```rust
pub use raid::{
    burden, Act, ActPress, Carry, ContainerMark, ExitMark, ExitPlan, Fate, HeroOutcome, Holder,
    Loadout, LootView, OpenView, RaidRoll, Recovery,
};
```

- [ ] **Step 2: Write the failing tests**

`crates/nightfall-sim/src/raid/tests.rs`:

```rust
//! The raid in whole worlds on small hand-made maps: carrying and acts first; containers, exits,
//! deaths and outcomes after.

use std::sync::Arc;

use super::*;
use crate::heightmap::testmap::flat;
use crate::{ContentSources, Gait, HeroPose, Life, PlayerInput, DT};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0,
     free_kit: [("knife", 1), ("bread", 2)]),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 800.0,
     speed: 7.0, power_max: 1000.0, bar: [(8, "water"), (12, "eat")]),
]"#;
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)],
    entries: [(5.0, 5.0, 0.0)],
    exits: [(key: "bridge", name: "The Bridge", at: (50.0, 50.0)),
            (key: "cart", name: "The Cart", at: (50.0, 10.0), late: true)])"#;
const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 1000.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 40.0, max: 40.0, every: 2.0))),
]"#;
const ABILITIES: &str = r#"[
    (key: "water", name: "Water", spell: 6127, aim: Me, cost: 50.0, cast: 3.0, gcd: 1.5,
     moving: Rooted, needs: (out_of_combat: true), effects: [ConjureWater(count: 2)]),
    (key: "eat", name: "Eat", spell: 2639, aim: Me, sound: Quiet,
     needs: (out_of_combat: true, still: true), effects: [Eat(secs: 15.0)]),
]"#;
const ITEMS: &str = r#"[
    (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0, slot: Some(MainHand),
     bonus: (damage: 0.5)),
    (key: "helm", name: "Helm", kind: Armour, icon: "a", weight: 2.0, slot: Some(Head),
     bonus: (health: 100.0)),
    (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 10, restore: 0.6),
    (key: "water", name: "Water", kind: Drink, icon: "a", weight: 0.5, stack: 10),
    (key: "conjured", name: "Conjured Water", kind: Drink, icon: "a", weight: 0.1, stack: 20),
    (key: "potion", name: "Potion", kind: Potion, icon: "a", weight: 0.2, stack: 5,
     on_use: Some(Heal(300.0))),
    (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10, value: 40),
    (key: "rock", name: "Rock", kind: Material, icon: "a", weight: 10.0, stack: 5),
    (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 4),
    (key: "lantern", name: "Lantern", kind: Lantern, icon: "a", weight: 1.0,
     light: Some((range: 18.0, intensity: 1.4))),
    (key: "key", name: "Key", kind: Key, icon: "a", weight: 0.1),
]"#;
const LOOT: &str = r#"(
    kinds: [(key: "chest", name: "Chest", display: 259, search: 2.0, sound: Open, slots: 8),
            (key: "grave", name: "Grave", display: 52, search: 6.0, sound: Dig, slots: 8)],
    tables: [(key: "gems", rolls: (2, 2), items: [(item: "gem", weight: 1.0, count: (3, 3))]),
             (key: "mixed", rolls: (1, 4), nothing: 1.0,
              items: [(item: "gem", weight: 1.0), (item: "bread", weight: 2.0, count: (1, 3)),
                      (item: "potion", weight: 1.0)])],
    containers: [(kind: "chest", table: "gems", at: (12.0, 5.0)),
                 (kind: "grave", table: "mixed", at: (12.0, 20.0)),
                 (kind: "chest", table: "gems", at: (30.0, 30.0), lock: Some("key"))],
    bodies: [(enemy: "brute", table: "gems")],
)"#;

/// A brute standing far from every spot the tests use, past its notice range from all of them
/// and past the loudest noise they make.
pub(super) const BRUTE_FAR: &str =
    r#"(place: "Far", kind: "brute", behaviour: Sentry(at: (5.0, 55.0), facing: 0.0)),"#;

fn rules(run: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0),
        run: (backpack_slots: 4, safe_slots: 1, stash_slots: 6, cart_opens_at: 20.0,
              conjured_drink: "conjured", {run}))"#
    )
}

/// A world on flat ground 60 yards across, its map attached, with `run` rules on top of the
/// tests' own and `spawns` (RON list items, each ending in a comma).
pub(super) fn world_with(run: &str, spawns: &str) -> World {
    let content = crate::Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(run),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
    })
    .unwrap();
    let mut w = World::new(Arc::new(content));
    w.attach_map(Arc::new(flat(60.0).finish()));
    w
}

pub(super) fn world() -> World {
    world_with("", "")
}

pub(super) fn item(w: &World, key: &str) -> usize {
    w.content().item(key).unwrap()
}

fn pose(x: f32, y: f32) -> Option<HeroPose> {
    Some(HeroPose {
        pos: V2::new(x, y),
        facing: V2::new(1.0, 0.0),
        z: 0.0,
    })
}

/// `player`'s next input: their hero at `(x, y)`, still.
pub(super) fn stand(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.set_input(
        player,
        PlayerInput {
            pose: pose(x, y),
            gait: Gait::Still,
            ..Default::default()
        },
    );
}

/// `player`'s next input: their hero walking, at `(x, y)`.
pub(super) fn walk(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.set_input(
        player,
        PlayerInput {
            pose: pose(x, y),
            gait: Gait::Walk,
            ..Default::default()
        },
    );
}

/// `player`'s hero of `class` standing at `(x, y)`, its first input taken.
pub(super) fn join(w: &mut World, player: PlayerId, class: &str, x: f32, y: f32) {
    w.add_hero(player, class, "Hero", 1, 0).unwrap();
    stand(w, player, x, y);
    w.step();
}

/// `player`'s next input: act `a` (press number `seq`), standing where they are.
pub(super) fn act(w: &mut World, player: PlayerId, seq: u8, a: Act) {
    let h = w.hero(player).unwrap();
    let at = HeroPose {
        pos: h.pos,
        facing: h.facing,
        z: h.z,
    };
    w.set_input(
        player,
        PlayerInput {
            pose: Some(at),
            act: Some(ActPress { seq, act: a }),
            ..Default::default()
        },
    );
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

/// A loadout: `pack` into the backpack, `worn` worn, `safe` in the safe slots.
pub(super) fn loadout(
    w: &World,
    pack: &[(&str, u16)],
    worn: &[(&str, EquipSlot)],
    safe: &[(&str, u16)],
) -> Loadout {
    let c = w.content();
    let mut inv = Inventory::new(c);
    for &(key, slot) in worn {
        inv.equip[slot.index()] = Some(Stack::new(item(w, key), 1));
    }
    for &(key, n) in pack {
        assert!(inv.add(Stack::new(item(w, key), n), c).is_none());
    }
    for (i, &(key, n)) in safe.iter().enumerate() {
        inv.safe[i] = Some(Stack::new(item(w, key), n));
    }
    Loadout {
        inventory: inv,
        recovery: None,
    }
}

#[test]
fn a_loadout_is_worn_weighed_and_counted() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    let lo = loadout(
        &w,
        &[("rock", 5), ("bread", 2)],
        &[("knife", EquipSlot::MainHand), ("helm", EquipSlot::Head)],
        &[],
    );
    assert!(w.give_loadout(0, &lo).is_empty());
    let h = w.hero(0).unwrap();
    assert!((h.carry.load - (50.0 + 1.0 + 1.0 + 2.0)).abs() < 1e-4);
    assert_eq!(
        h.carry.worn,
        vec![
            (EquipSlot::Head, item(&w, "helm") as u16),
            (EquipSlot::MainHand, item(&w, "knife") as u16)
        ]
    );
    assert_eq!((h.max_hp, h.hp), (1100.0, 1100.0));
    assert!((w.gear_damage(0) - 1.5).abs() < 1e-6);
    assert!((w.gear_damage(7) - 1.0).abs() < 1e-6);
}

#[test]
fn weight_slows_the_pace_and_makes_steps_louder() {
    let r = RunRules::default();
    let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5;
    assert!(near(burden(0.0, &r), (1.0, 1.0)));
    assert!(near(burden(20.0, &r), (1.0, 1.0)));
    assert!(near(burden(40.0, &r), (0.85, 1.25)));
    assert!(near(burden(60.0, &r), (0.7, 1.5)));
    assert!(near(burden(500.0, &r), (0.7, 1.5)));
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("rock", 5)], &[], &[]));
    for i in 0..30 {
        walk(&mut w, 0, 5.0 + i as f32 * 0.1, 5.0);
        w.step();
    }
    let h = w.hero(0).unwrap();
    // 50 carried: three quarters of the way from weight_free to weight_full.
    assert!((h.speed - 7.0 * 0.775).abs() < 1e-3, "{}", h.speed);
    assert!((h.noise - 4.0 * 1.375).abs() < 1e-3, "{}", h.noise);
}

#[test]
fn an_act_is_taken_once_however_often_it_is_sent() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1), ("bread", 1)], &[], &[]));
    let lantern = item(&w, "lantern") as u16;
    for _ in 0..3 {
        act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
        w.step();
    }
    assert_eq!(w.hero(0).unwrap().carry.lantern, Some(lantern));
    assert_eq!(w.hero(0).unwrap().carry.acted, 1);
    act(&mut w, 0, 2, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().carry.lantern, None);
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    let c = &w.hero(0).unwrap().carry;
    assert_eq!((c.lantern, c.acted), (None, 2));
}

#[test]
fn the_downed_cannot_act_but_are_answered() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1)], &[], &[]));
    w.heroes[0].fight.life = Life::Downed {
        left: 40.0,
        revive: 0.0,
    };
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    w.step();
    let c = &w.hero(0).unwrap().carry;
    assert_eq!((c.lantern, c.acted), (None, 1));
}

#[test]
fn a_potion_heals_at_once_then_waits() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("potion", 2)], &[], &[]));
    let potion = item(&w, "potion");
    w.heroes[0].hp = 400.0;
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().hp, 700.0);
    act(&mut w, 0, 2, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().hp, 700.0);
    assert_eq!(w.pack(0).unwrap().count_of(potion), 1);
    steps(&mut w, (30.0 / DT) as usize);
    act(&mut w, 0, 3, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().hp, 1000.0);
    assert_eq!(w.pack(0).unwrap().count_of(potion), 0);
}

#[test]
fn wearing_and_dropping_by_act() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("knife", 1), ("bread", 2)], &[], &[]));
    act(
        &mut w,
        0,
        1,
        Act::Move {
            from: Place::Pack(0),
            to: Place::Equip(EquipSlot::MainHand),
        },
    );
    w.step();
    let knife = item(&w, "knife") as u16;
    assert_eq!(w.hero(0).unwrap().carry.worn, vec![(EquipSlot::MainHand, knife)]);
    act(&mut w, 0, 2, Act::Drop(Place::Pack(1)));
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "bread")), 0);
    assert!((w.hero(0).unwrap().carry.load - 1.0).abs() < 1e-5);
    let view = w.loot_view(Some(0));
    let pile = view.marks.iter().find(|m| m.holder == Holder::Pile).unwrap();
    assert!(pile.searched && !pile.empty);
    assert!(pile.at.distance(V2::new(5.0, 5.0)) < 1e-4);
}

#[test]
fn gear_health_comes_whole_only_to_the_whole() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("helm", 1)], &[], &[]));
    let wear = Act::Move {
        from: Place::Pack(0),
        to: Place::Equip(EquipSlot::Head),
    };
    let off = Act::Move {
        from: Place::Equip(EquipSlot::Head),
        to: Place::Pack(0),
    };
    act(&mut w, 0, 1, wear);
    w.step();
    assert_eq!((w.heroes[0].max_hp, w.heroes[0].hp), (1100.0, 1100.0));
    act(&mut w, 0, 2, off);
    w.step();
    assert_eq!((w.heroes[0].max_hp, w.heroes[0].hp), (1000.0, 1000.0));
    // Wearing it hurt does not heal: no health from taking a helm off and on.
    w.heroes[0].hp = 600.0;
    act(&mut w, 0, 3, wear);
    w.step();
    assert_eq!((w.heroes[0].max_hp, w.heroes[0].hp), (1100.0, 600.0));
}

#[test]
fn a_raid_must_name_an_entry_and_every_exit() {
    let mut w = world();
    let standard = RaidRoll::standard(w.content(), 9);
    assert_eq!(standard.exits[0], ExitPlan { open: true, opens_at: 0.0 });
    assert_eq!(standard.exits[1], ExitPlan { open: true, opens_at: 20.0 });
    let far = RaidRoll {
        entry: 3,
        ..standard.clone()
    };
    assert!(w.begin_raid(far).is_err());
    let shut = RaidRoll {
        exits: Vec::new(),
        ..standard.clone()
    };
    assert!(w.begin_raid(shut).is_err());
    assert!(w.begin_raid(standard).is_ok());
    assert_eq!(w.raid.roll.seed, 9);
}

#[test]
fn a_mirror_sees_its_own_pack_and_the_loot_near_it() {
    let mut host = world();
    join(&mut host, 0, "rogue", 5.0, 5.0);
    host.give_loadout(0, &loadout(&host, &[("bread", 2)], &[], &[]));
    act(&mut host, 0, 1, Act::Drop(Place::Pack(0)));
    host.step();
    let mut mirror = world();
    mirror.apply_snapshot(host.snapshot_for(0));
    assert_eq!(mirror.pack(0), host.pack(0));
    assert_eq!(mirror.loot_view(Some(0)), host.loot_view(Some(0)));
    let mut whole = world();
    whole.apply_snapshot(host.snapshot());
    assert_eq!(whole.snapshot(), host.snapshot());
}

#[test]
fn a_hero_who_leaves_drops_what_a_death_would() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 8.0, 5.0);
    stand(&mut w, 0, 5.0, 5.0);
    stand(&mut w, 1, 8.0, 5.0);
    w.step();
    w.give_loadout(1, &loadout(&w, &[("rock", 2)], &[], &[("bread", 1)]));
    w.remove_hero(1);
    let piles: Vec<&Container> = w
        .raid
        .containers
        .iter()
        .filter(|c| c.holder == Holder::Pile)
        .collect();
    assert_eq!(piles.len(), 1);
    let held: Vec<Stack> = piles[0].stacks.iter().flatten().copied().collect();
    assert_eq!(held, vec![Stack::new(item(&w, "rock"), 2)]);
    assert!(w.pack(1).is_none());
}
```

Run: `cargo test -p nightfall-sim raid`
Expected: FAIL to compile (`no field carry on type Hero`, `struct PlayerInput has no field named act`, `no method named take_act`).

- [ ] **Step 3: Write the acts**

`crates/nightfall-sim/src/raid/acts.rs`:

```rust
//! What heroes do with what they carry and find (spec §8.2): wear, move, use and drop things,
//! open and search containers, take from them and put into them. Each act comes once
//! ([`ActPress`]): it rides in every input until the host takes it, so an input the host
//! replaced before its step loses nothing, and the same act seen again does nothing.

use super::{in_reach, search_of, Act, ActPress, Holder, Search};
use crate::{Content, EquipSlot, ItemKind, ItemUse, Place, PlayerId, PlayerInput, Stack, World, V3};

impl World {
    /// `player`'s act in `input`, unless it is the one already taken. A hero who is not up is
    /// answered, so their client moves on, but does nothing.
    pub(crate) fn take_act(&mut self, player: PlayerId, input: &PlayerInput) {
        let Some(ActPress { seq, act }) = input.act else {
            return;
        };
        if self.raid.last_act(player) == Some(seq) {
            return;
        }
        self.raid.set_last_act(player, seq);
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        self.heroes[hi].carry.acted = seq;
        if !self.heroes[hi].fight.up() {
            return;
        }
        let content = self.content().clone();
        match act {
            Act::Move { from, to } => {
                let _ = self.raid.pack_mut(player, &content).move_to(from, to, &content);
            }
            Act::Use(at) => self.use_item(player, hi, at, &content),
            Act::Drop(at) => {
                let here = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
                if let Some(stack) = self.raid.pack_mut(player, &content).take(at) {
                    self.raid.drop_on_ground(here, stack, &content);
                }
            }
            Act::Open(id) => self.open(player, hi, id, &content),
            Act::Close => self.raid.close(player),
            Act::Take(slot) => self.take_from(player, Some(usize::from(slot)), &content),
            Act::TakeAll => self.take_from(player, None, &content),
            Act::Put(at) => self.put_into(player, at, &content),
        }
        self.refresh_carry(player);
    }

    /// A lantern lit or put out; a potion drunk, when the last one's wait is over.
    fn use_item(&mut self, player: PlayerId, hi: usize, at: Place, content: &Content) {
        let Some(stack) = self.raid.pack(player).and_then(|i| i.get(at)).copied() else {
            return;
        };
        let def = &content.items[usize::from(stack.item)];
        match def.kind {
            ItemKind::Lantern => {
                let c = &mut self.heroes[hi].carry;
                c.lantern = if c.lantern == Some(stack.item) {
                    None
                } else {
                    Some(stack.item)
                };
            }
            ItemKind::Potion => {
                let now = self.elapsed;
                let Some(effect) = def.on_use else {
                    return;
                };
                if self.raid.potion_ready(player) > now {
                    return;
                }
                if self.raid.pack_mut(player, content).take_one(at).is_none() {
                    return;
                }
                let h = &mut self.heroes[hi];
                match effect {
                    ItemUse::Heal(n) => h.hp = (h.hp + n).min(h.max_hp),
                    ItemUse::Mana(n) => h.fight.power = (h.fight.power + n).min(h.fight.max_power),
                }
                self.raid
                    .set_potion_ready(player, now + content.rules.run.potion_cooldown);
            }
            _ => {}
        }
    }

    /// Search container `id` within reach, or open it once searched: a locked one needs its key
    /// carried; a hero's own remains give everything back at once (spec §8.2).
    fn open(&mut self, player: PlayerId, hi: usize, id: u32, content: &Content) {
        let (pos, z) = (self.heroes[hi].pos, self.heroes[hi].z);
        let Some(ci) = self.raid.containers.iter().position(|c| c.id == id) else {
            return;
        };
        let (at, cz, holder, lock, searched) = {
            let c = &self.raid.containers[ci];
            (c.at, c.z, c.holder, c.lock, c.searched)
        };
        if !in_reach(pos, z, at, cz, content.rules.run.reach) {
            return;
        }
        if let Holder::Remains(owner) = holder {
            if owner == player {
                let c = self.raid.containers.remove(ci);
                self.raid.recover(player, c.stacks.into_iter().flatten().collect());
            }
            return;
        }
        if let Some(key) = lock {
            let has = self
                .raid
                .pack(player)
                .is_some_and(|i| i.count_of(usize::from(key)) > 0);
            if !has {
                return;
            }
        }
        if self.raid.searches.iter().any(|s| s.player == player && s.id == id) {
            return;
        }
        self.raid.close(player);
        let (total, _) = search_of(holder, content);
        if searched || total <= 0.0 {
            self.raid.containers[ci].searched = true;
            self.raid.opened.push((player, id));
        } else {
            self.raid.searches.push(Search {
                player,
                id,
                left: total,
                total,
                from: pos,
                noise_in: 0.0,
            });
        }
    }

    /// The stack in `slot` (or every stack) of the open container into the backpack, as found
    /// this raid; what does not fit stays.
    fn take_from(&mut self, player: PlayerId, slot: Option<usize>, content: &Content) {
        let Some(id) = self.raid.open_of(player) else {
            return;
        };
        let Some(ci) = self.raid.containers.iter().position(|c| c.id == id) else {
            return;
        };
        let slots: Vec<usize> = match slot {
            Some(s) => vec![s],
            None => (0..self.raid.containers[ci].stacks.len()).collect(),
        };
        for s in slots {
            let Some(stack) = self.raid.containers[ci].stacks.get(s).copied().flatten() else {
                continue;
            };
            let left = self
                .raid
                .pack_mut(player, content)
                .add(Stack { found: true, ..stack }, content);
            self.raid.containers[ci].stacks[s] = left.map(|l| Stack {
                found: stack.found,
                ..l
            });
        }
    }

    /// The stack at `at` into the open container's first empty slot.
    fn put_into(&mut self, player: PlayerId, at: Place, content: &Content) {
        let Some(id) = self.raid.open_of(player) else {
            return;
        };
        let Some(ci) = self.raid.containers.iter().position(|c| c.id == id) else {
            return;
        };
        let Some(free) = self.raid.containers[ci].stacks.iter().position(Option::is_none) else {
            return;
        };
        let Some(stack) = self.raid.pack_mut(player, content).take(at) else {
            return;
        };
        self.raid.containers[ci].stacks[free] = Some(stack);
    }

    /// `player`'s carry after a change: the weight, what they wear, their lantern (out once it
    /// left them), the drinks they carry, and their health with what they wear. More health from
    /// gear arrives whole only to a hero at full health, so taking gear off and on heals nobody.
    pub(crate) fn refresh_carry(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let Some(inv) = self.raid.pack(player) else {
            return;
        };
        let load = inv.weight(&content);
        let worn: Vec<(EquipSlot, u16)> = EquipSlot::ALL
            .iter()
            .filter_map(|&s| inv.equip[s.index()].map(|st| (s, st.item)))
            .collect();
        let drinks: u32 = inv
            .stacks()
            .iter()
            .filter(|(_, s)| content.items[usize::from(s.item)].kind == ItemKind::Drink)
            .map(|(_, s)| u32::from(s.count))
            .sum();
        let bonus = inv.bonus(&content);
        let lantern_kept = self.heroes[hi]
            .carry
            .lantern
            .is_some_and(|l| inv.count_of(usize::from(l)) > 0);
        let h = &mut self.heroes[hi];
        h.carry.load = load;
        h.carry.worn = worn;
        if !lantern_kept {
            h.carry.lantern = None;
        }
        h.fight.drinks = drinks.min(u32::from(u8::MAX)) as u8;
        let max = content.classes[h.class].base_hp + bonus.health;
        if (max - h.max_hp).abs() > f32::EPSILON {
            let whole = h.hp >= h.max_hp;
            h.max_hp = max;
            if whole && h.fight.up() {
                h.hp = max;
            }
            h.hp = h.hp.min(max);
        }
    }
}
```

- [ ] **Step 4: Put the raid in the world, weight in the senses, gear in the fight**

In `crates/nightfall-sim/src/world.rs`:

- Add to the imports: `use crate::raid::{Carry, Raid};` and `use crate::{ActPress, Inventory, LootView, RaidRoll};`.
- `Hero` gains, after its last field:

```rust
    /// What they carry (spec §8.2), as every client sees it.
    pub carry: Carry,
```

and `add_hero`'s `Hero { … }` literal gains `carry: Carry::default(),`. In `add_hero`, every `map.entries[0]` becomes `map.entries[self.raid.roll.entry]`, so heroes start at the raid's entry.
- `PlayerInput` gains:

```rust
    /// The act the host has not taken yet, repeated until it has ([`ActPress`]).
    pub act: Option<ActPress>,
```

- `Snapshot` gains:

```rust
    /// The receiving player's own inventory ([`World::snapshot_for`]).
    pub pack: Option<(PlayerId, Inventory)>,
    /// The raid's loot as the receiving player sees it.
    pub loot: LootView,
```

- `World` gains:

```rust
    /// The raid: what heroes carry, the containers, the exits and the outcomes.
    pub(crate) raid: Raid,
```

and in `World::new`, before `Self {`, add `let raid = Raid::new(&content, RaidRoll::standard(&content, 0));`, and `raid,` to the literal.
- `remove_hero` gains as its first statement `self.raid_leave(player);`.
- In `step`, after the loop `for (player, input) in &inputs { self.take_presses(*player, input); }`, add:

```rust
        for (player, input) in &inputs {
            self.take_act(*player, input);
        }
```

- `snapshot`'s `Snapshot { … }` literal gains `pack: None,` and `loot: self.loot_view(None),`; `snapshot_for`'s gains `pack: self.raid.pack(player).map(|i| (player, i.clone())),` and `loot: self.loot_view(Some(player)),`.
- At the end of `apply_snapshot` add:

```rust
        self.raid.mirror = Some(snapshot.loot);
        self.raid.packs = snapshot.pack.into_iter().collect();
```

In `crates/nightfall-sim/src/senses/mod.rs`, in `step_heroes`:

- Before `hero.speed = …`, add `let (pace, loud) = crate::burden(hero.carry.load, &content.rules.run);`.
- `hero.speed = class.speed * if hero.stealthed { r.stealth_speed } else { 1.0 };` becomes:

```rust
        // Weight slows walking and sneaking alike (spec §8.2); the fight's own pace comes after.
        hero.speed = class.speed * if hero.stealthed { r.stealth_speed } else { 1.0 } * pace;
```

- `let radius = step_radius(hero.stance, &r.steps);` becomes `let radius = step_radius(hero.stance, &r.steps) * loud;` (sneaking stays silent: 0 times anything).
- `lantern: false,` becomes `lantern: hero.carry.lantern.is_some(),`.

In `crates/nightfall-sim/src/combat/effects.rs`, `World::strike` gains as its first statement:

```rust
        // Worn gear's share of damage (spec §6.1: power comes from gear).
        let amount = amount * self.gear_damage(by);
```

Every other literal: `grep -rn "Hero {" crates/nightfall-sim/src crates/benilla-app/src/nightfall` finds the test heroes (in `combat/tests.rs` and the client's `fallen.rs` tests among them): each gains `carry: Default::default(),`. `grep -rn "Snapshot {" crates/nightfall-sim/src crates/benilla-app/src/nightfall` outside `world.rs`: each gains `pack: None,` and `loot: Default::default(),`. A `PlayerInput { … }` literal that names every field without `..Default::default()` (the client's `LocalInput::with_pose`) gains `act: None,` (Task 8 fills it).

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test and the 10 in `raid::tests`).
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: heroes carry their loadout; acts taken once; weight slows and louder steps; gear adds health and damage

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Food and drink as items; Conjure Water makes drinks

**Files:**
- Modify: `crates/nightfall-sim/src/raid/mod.rs` (`Raid::{eat, has_meal}`), `crates/nightfall-sim/src/combat/mod.rs` (`Refusal::NoFood`), `crates/nightfall-sim/src/combat/effects.rs` (the `Eat` and `ConjureWater` arms), `crates/nightfall-sim/src/combat/heroes.rs` (`check`), `crates/nightfall-sim/src/combat/tests.rs` (rules, one test removed), `crates/nightfall-sim/src/raid/tests.rs`
- Modify: `crates/benilla-app/src/nightfall/fight.rs` (`refusal_key`)

**Interfaces:**
- Consumes: `Effect::{Eat, ConjureWater}`, `Meal`, `Refusal`, `World::{land, check, hero_slot}`, `MANA`, `CombatRules::max_drinks`, `HeroFight::drinks` (combat Tasks 1, 3–5); `Inventory::{first_of, take_one, add, count_of}`, `Stack::found` (Task 2); `Content::conjured`, `RunRules::free_meal`, `ItemDef::restore` (Task 1); `World::refresh_carry`, `raid::tests::{world, world_with, join, loadout, item}` (Task 3).
- Produces:
  - `Refusal::NoFood`; `fight::refusal_key(Refusal::NoFood, _) == "SPELL_FAILED_REAGENTS"`.
  - Crate-only `Raid::eat(&mut self, PlayerId, ItemKind, &Content) -> Option<f32>` (one eaten from the backpack: its `restore`) and `Raid::has_meal(&self, PlayerId, &Content) -> bool`.
  - Eat and Drink eats the backpack's first food (its `restore` share of health over the meal, all of it with `free_meal` and no food) and, for a mana user, its first drink (its share of mana); with neither and no `free_meal` it is refused (`NoFood`). Conjure Water puts `count` of the content's `conjured` item in the backpack, at most `max_drinks` carried. `HeroFight::drinks` counts the drinks carried.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/raid/tests.rs`:

```rust
#[test]
fn eating_eats_food_and_drinks_a_drink_from_the_pack() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2), ("water", 1)], &[], &[]));
    assert_eq!(w.hero(0).unwrap().fight.drinks, 1);
    let eat = w.content().ability("eat").unwrap();
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    // Bread gives back 60% of health over the meal, water all the mana.
    assert!((meal.health - 800.0 * 0.6 / 15.0).abs() < 1e-3);
    assert!((meal.mana - 1000.0 / 15.0).abs() < 1e-3);
    let p = w.pack(0).unwrap();
    assert_eq!(p.count_of(item(&w, "bread")), 1);
    assert_eq!(p.count_of(item(&w, "water")), 0);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 0);
}

#[test]
fn a_rogue_eats_and_keeps_the_water() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 1), ("water", 1)], &[], &[]));
    w.land(0, w.content().ability("eat").unwrap(), None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert_eq!(meal.mana, 0.0);
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "water")), 1);
}

#[test]
fn water_conjures_drinks_into_the_pack_up_to_the_cap() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[], &[], &[]));
    let water = w.content().ability("water").unwrap();
    let conjured = item(&w, "conjured");
    w.land(0, water, None, None);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), 2);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 2);
    assert!(w.pack(0).unwrap().stacks().iter().all(|(_, s)| s.found));
    for _ in 0..12 {
        w.land(0, water, None, None);
    }
    let cap = u32::from(w.content().rules.combat.max_drinks);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), cap);
}

#[test]
fn with_nothing_to_eat_eating_is_refused() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("knife", 1)], &[], &[]));
    let eat = w.content().ability("eat").unwrap();
    let hi = w.hero_slot(0).unwrap();
    assert_eq!(w.check(hi, eat, None, None).err(), Some(crate::Refusal::NoFood));
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    assert!(w.check(hi, eat, None, None).is_ok());
    let mut free = world_with("free_meal: true", "");
    join(&mut free, 0, "rogue", 5.0, 5.0);
    free.give_loadout(0, &loadout(&free, &[], &[], &[]));
    let hi = free.hero_slot(0).unwrap();
    assert!(free.check(hi, eat, None, None).is_ok());
}
```

Run: `cargo test -p nightfall-sim raid`
Expected: FAIL to compile (`no variant named NoFood`).

- [ ] **Step 2: Eat from the backpack**

Add to `impl Raid` in `crates/nightfall-sim/src/raid/mod.rs` (and `ItemKind` to its `use crate::{…}`):

```rust
    /// One of the first `kind` (food or drink) in `player`'s backpack eaten: the share of health
    /// or mana it gives back. The safe slots are not eaten from.
    pub(crate) fn eat(&mut self, player: PlayerId, kind: ItemKind, content: &Content) -> Option<f32> {
        let inv = self
            .packs
            .iter_mut()
            .find(|(p, _)| *p == player)
            .map(|(_, i)| i)?;
        let at = inv.first_of(kind, content)?;
        let s = inv.take_one(at)?;
        Some(content.items[usize::from(s.item)].restore)
    }

    /// Whether `player`'s backpack holds anything to eat or drink.
    pub(crate) fn has_meal(&self, player: PlayerId, content: &Content) -> bool {
        self.pack(player).is_some_and(|i| {
            i.first_of(ItemKind::Food, content).is_some()
                || i.first_of(ItemKind::Drink, content).is_some()
        })
    }
```

In `crates/nightfall-sim/src/combat/mod.rs`, `Refusal` gains after `NotInSight`:

```rust
    /// Eat and Drink with nothing to eat or drink in the backpack.
    NoFood,
```

In `crates/nightfall-sim/src/combat/heroes.rs`, in `check`, right after the `Busy` refusal (`if f.casting.is_some() { return Err(Refusal::Busy); }`), add (and `Effect` to the file's imports if it is not there):

```rust
        if a.has(|e| matches!(e, Effect::Eat { .. }))
            && !content.rules.run.free_meal
            && !self.raid.has_meal(hero.player, content)
        {
            return Err(Refusal::NoFood);
        }
```

In `crates/nightfall-sim/src/combat/effects.rs` (add `ItemKind` and `Stack` to its `use crate::{…}`), the `Effect::Eat { secs }` arm becomes:

```rust
            Effect::Eat { secs } => {
                // Food and drink are items (spec §8.2): one of each from the backpack, the drink
                // only for a mana user; with `free_meal`, health comes back without food.
                let player = self.heroes[hi].player;
                let mana_user = content.classes[self.heroes[hi].class].power == MANA;
                let food = self.raid.eat(player, ItemKind::Food, &content);
                let drink = if mana_user {
                    self.raid.eat(player, ItemKind::Drink, &content)
                } else {
                    None
                };
                let health = food
                    .or(content.rules.run.free_meal.then_some(1.0))
                    .unwrap_or(0.0);
                let hero = &mut self.heroes[hi];
                hero.fight.eating = secs;
                self.combat.heroes[ci].meal = Some(Meal {
                    health: hero.max_hp * health / secs,
                    mana: hero.fight.max_power * drink.unwrap_or(0.0) / secs,
                });
                self.refresh_carry(player);
            }
```

and the `Effect::ConjureWater { count }` arm becomes:

```rust
            Effect::ConjureWater { count } => {
                // Conjured drinks are items in the backpack (found: they were made here), at
                // most `max_drinks` of them carried.
                let player = self.heroes[hi].player;
                if let Some(drink) = content.conjured {
                    let have = self.raid.pack(player).map_or(0, |i| i.count_of(drink));
                    let room = u32::from(content.rules.combat.max_drinks).saturating_sub(have);
                    let n = u32::from(count).min(room) as u16;
                    if n > 0 {
                        let _ = self
                            .raid
                            .pack_mut(player, &content)
                            .add(Stack::found(drink, n), &content);
                    }
                    self.refresh_carry(player);
                }
            }
```

In `crates/nightfall-sim/src/combat/tests.rs`:
- `rules`'s format string ends `combat: (crit_chance: 0.0, {combat}), run: (free_meal: true))"#` instead of `combat: (crit_chance: 0.0, {combat}))"#`: the fight's tests eat without carrying food, as they did before food was an item.
- Delete the test `eating_needs_a_drink_for_mana_and_water_gives_two` (drinks are items now; `raid::tests` covers them).

In `crates/benilla-app/src/nightfall/fight.rs`, `refusal_key`'s match gains `Refusal::NoFood => "SPELL_FAILED_REAGENTS",` (WoW's "Missing reagent").

- [ ] **Step 3: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 4 new tests, and every combat test with the one removed).
Run: `cargo test -p benilla-app --lib nightfall::fight -j 2`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall/fight.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: Eat and Drink eats food and drink from the backpack; Conjure Water makes drink items

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Containers: rolled from their tables, searched aloud, bodies, piles

**Files:**
- Modify: `crates/nightfall-sim/src/raid/containers.rs` (`Raid::new` places the content's containers; tables; `step_searches`, `step_bodies`, `step_raid`), `crates/nightfall-sim/src/world.rs` (`step`), `crates/nightfall-sim/src/raid/tests.rs`

**Interfaces:**
- Consumes: `Raid`, `Container`, `Search`, `Holder`, `in_reach`, `search_of`, `add_container`, the acts `Open`, `Take`, `TakeAll`, `Put`, `Close` (Task 3); `LootContent`, `Table`, `PlacedContainer` (Task 1); `Dice::{new, unit}` (combat Task 2); `World::{make_noise, enemies, kill_enemy}`, `Enemy::{id, kind, pos, z, hp}`, `Loudness` (earlier plans).
- Produces:
  - `Raid::new(&Content, RaidRoll)` places every container of the content's loot, each filled from its table by the raid's seed.
  - Crate-only `roll_table(&Table, &mut Dice, &Content) -> Vec<Stack>`, `seed_for(seed: u64, i: u64) -> u64`; `World::{step_searches, step_bodies, step_raid}`.
  - `World::step` calls `step_raid` right after `step_fighters`: searches move on (heard every `noise_every`), what is open closes when its hero walks off, enemies dead since the last step leave bodies holding their kind's table.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/raid/tests.rs`:

```rust
fn chest(w: &World) -> u32 {
    w.raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Placed(0) && c.lock.is_none())
        .unwrap()
        .id
}

#[test]
fn the_content_places_its_containers_filled_from_their_tables() {
    let w = world();
    let placed: Vec<&Container> = w
        .raid
        .containers
        .iter()
        .filter(|c| matches!(c.holder, Holder::Placed(_)))
        .collect();
    assert_eq!(placed.len(), 3);
    let gems: Vec<Stack> = placed[0].stacks.iter().flatten().copied().collect();
    // Two rolls of three gems each, one stack.
    assert_eq!(gems, vec![Stack::new(item(&w, "gem"), 6)]);
    assert_eq!(placed[0].stacks.len(), 8);
    assert!(!placed[0].searched);
    assert_eq!(placed[2].lock, Some(item(&w, "key") as u16));
}

#[test]
fn the_same_seed_rolls_the_same_loot_and_seeds_differ() {
    let grave = |seed: u64| {
        let mut w = world();
        let roll = RaidRoll::standard(w.content(), seed);
        w.begin_raid(roll).unwrap();
        w.raid.containers[1].stacks.clone()
    };
    assert_eq!(grave(7), grave(7));
    let rolls: Vec<_> = (0..20).map(grave).collect();
    assert!(rolls.iter().any(|r| *r != rolls[0]), "twenty seeds all rolled the same");
}

#[test]
fn searching_a_chest_takes_its_time_and_is_heard() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[], &[], &[]));
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    let view = w.loot_view(Some(0)).open.unwrap();
    assert_eq!(view.id, id);
    assert!(view.stacks.is_empty());
    let (left, total) = view.search.unwrap();
    assert_eq!(total, 2.0);
    assert!(left < 2.0);
    steps(&mut w, 3);
    assert!(w.hero(0).unwrap().noise >= 15.0, "a chest is heard 15 yards off");
    steps(&mut w, (2.0 / DT) as usize);
    let open = w.loot_view(Some(0)).open.unwrap();
    assert!(open.search.is_none());
    assert_eq!(open.stacks[0], Some(Stack::new(item(&w, "gem"), 6)));
    act(&mut w, 0, 2, Act::Take(0));
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 6);
    assert!(w.pack(0).unwrap().stacks()[0].1.found);
    let mark = w.loot_view(Some(0)).marks.into_iter().find(|m| m.id == id).unwrap();
    assert!(mark.searched && mark.empty);
}

#[test]
fn walking_off_stops_a_search_and_closes_what_is_open() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    stand(&mut w, 0, 11.0, 7.0);
    w.step();
    assert!(w.loot_view(Some(0)).open.is_none());
    assert!(!w.raid.containers.iter().find(|c| c.id == id).unwrap().searched);
    // Searched, then walked away from: it closes.
    stand(&mut w, 0, 11.0, 5.0);
    w.step();
    act(&mut w, 0, 2, Act::Open(id));
    steps(&mut w, (2.2 / DT) as usize);
    assert!(w.loot_view(Some(0)).open.is_some());
    stand(&mut w, 0, 20.0, 5.0);
    w.step();
    assert!(w.loot_view(Some(0)).open.is_none());
}

#[test]
fn a_locked_chest_needs_its_key_and_a_far_one_cannot_be_opened() {
    let mut w = world();
    join(&mut w, 0, "rogue", 30.0, 28.0);
    let locked = w.raid.containers[2].id;
    act(&mut w, 0, 1, Act::Open(locked));
    w.step();
    assert!(w.loot_view(Some(0)).open.is_none());
    w.give_loadout(0, &loadout(&w, &[("key", 1)], &[], &[]));
    act(&mut w, 0, 2, Act::Open(locked));
    w.step();
    assert!(w.loot_view(Some(0)).open.is_some());
    act(&mut w, 0, 3, Act::Open(chest(&w)));
    w.step();
    let open = w.loot_view(Some(0)).open.unwrap();
    assert_eq!(open.id, locked, "the far chest did not open, and the search went on");
}

#[test]
fn what_does_not_fit_stays_in_the_container() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(
        0,
        &loadout(&w, &[("rock", 5), ("bread", 1), ("water", 1), ("potion", 1)], &[], &[]),
    );
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    steps(&mut w, (2.2 / DT) as usize);
    act(&mut w, 0, 2, Act::TakeAll);
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 0);
    let c = w.raid.containers.iter().find(|c| c.id == id).unwrap();
    assert_eq!(c.stacks[0], Some(Stack::new(item(&w, "gem"), 6)));
    // Put the rocks in; then the gems fit.
    act(&mut w, 0, 3, Act::Put(Place::Pack(0)));
    w.step();
    act(&mut w, 0, 4, Act::Take(0));
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 6);
    let c = w.raid.containers.iter().find(|c| c.id == id).unwrap();
    assert!(c.stacks.contains(&Some(Stack::new(item(&w, "rock"), 5))));
}

#[test]
fn a_dead_enemy_leaves_a_body_holding_its_table() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    let id = w.enemies[0].id;
    w.kill_enemy(0, Some(0));
    w.step();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Body(id))
        .unwrap();
    assert_eq!(body.stacks[0], Some(Stack::new(item(&w, "gem"), 6)));
    let body = body.id;
    act(&mut w, 0, 1, Act::Open(body));
    w.step();
    let (_, total) = w.loot_view(Some(0)).open.unwrap().search.unwrap();
    assert_eq!(total, 1.5);
    steps(&mut w, 60);
    assert_eq!(w.raid.containers.iter().filter(|c| c.holder == Holder::Body(id)).count(), 1);
}

#[test]
fn a_friend_picks_up_what_was_dropped() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 6.0, 5.0);
    stand(&mut w, 0, 5.0, 5.0);
    stand(&mut w, 1, 6.0, 5.0);
    w.step();
    w.give_loadout(0, &loadout(&w, &[("bread", 3)], &[], &[]));
    w.give_loadout(1, &loadout(&w, &[], &[], &[]));
    act(&mut w, 0, 1, Act::Drop(Place::Pack(0)));
    stand(&mut w, 1, 6.0, 5.0);
    w.step();
    let pile = w.loot_view(Some(1)).marks.iter().find(|m| m.holder == Holder::Pile).unwrap().id;
    act(&mut w, 1, 1, Act::Open(pile));
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    act(&mut w, 1, 2, Act::TakeAll);
    w.step();
    assert_eq!(w.pack(1).unwrap().count_of(item(&w, "bread")), 3);
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "bread")), 0);
}
```

Run: `cargo test -p nightfall-sim raid`
Expected: FAIL (`the_content_places_its_containers_filled_from_their_tables` finds no container; the search tests never finish).

- [ ] **Step 2: Place, roll and search**

In `crates/nightfall-sim/src/raid/containers.rs`:

- The `use` lines become:

```rust
use super::{Container, Holder, Raid, RaidRoll};
use crate::combat::Dice;
use crate::{Content, Loudness, PlayerId, Stack, Table, World, DT, V2, V3};
```

- Add:

```rust
/// How far a hero may stray from where a search began, yards.
const SEARCH_STRAY: f32 = 1.0;
/// Mixed into the seed for enemies' bodies, so a body never rolls what a chest did.
const BODY_SEED: u64 = 0xB0D7_5EED_0000_0001;

/// The `i`th roll from a raid's `seed`: every container its own dice, the same every time.
pub(crate) fn seed_for(seed: u64, i: u64) -> u64 {
    seed ^ (i + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// A whole number from `lo` to `hi`.
fn pick(dice: &mut Dice, lo: u16, hi: u16) -> u16 {
    if hi <= lo {
        return lo;
    }
    let span = hi - lo + 1;
    lo + ((dice.unit() * f32::from(span)) as u16).min(span - 1)
}

/// A table's roll (spec §8.1): `rolls` picks, each an entry by weight or nothing, its count
/// within its range; the same item twice is one stack, as far as its stack size lets it be.
pub(crate) fn roll_table(table: &Table, dice: &mut Dice, content: &Content) -> Vec<Stack> {
    let n = pick(dice, u16::from(table.rolls.0), u16::from(table.rolls.1));
    let total = table.nothing + table.items.iter().map(|e| e.weight).sum::<f32>();
    let mut out: Vec<Stack> = Vec::new();
    if !(total > 0.0) {
        return out;
    }
    for _ in 0..n {
        let mut x = dice.unit() * total - table.nothing;
        if x < 0.0 {
            continue;
        }
        let Some(e) = table.items.iter().find(|e| {
            if x < e.weight {
                true
            } else {
                x -= e.weight;
                false
            }
        }) else {
            continue;
        };
        let mut count = pick(dice, e.count.0, e.count.1);
        let max = content.items[e.item].stack.max(1);
        for s in out.iter_mut().filter(|s| usize::from(s.item) == e.item) {
            let n = (max - s.count).min(count);
            s.count += n;
            count -= n;
        }
        while count > 0 {
            let n = count.min(max);
            out.push(Stack::new(e.item, n));
            count -= n;
        }
    }
    out
}
```

- `Raid::new` becomes:

```rust
    /// A raid as `roll` has it: every container of the content's loot placed, each filled from
    /// its table by dice of its own from the raid's seed.
    pub(crate) fn new(content: &Content, roll: RaidRoll) -> Self {
        let mut raid = Self {
            roll,
            next_id: 1,
            ..Self::default()
        };
        for (i, p) in content.loot.containers.iter().enumerate() {
            let mut dice = Dice::new(seed_for(raid.roll.seed, i as u64));
            let stacks = roll_table(&content.loot.tables[p.table], &mut dice, content);
            let slots = usize::from(content.loot.kinds[p.kind].slots);
            raid.add_container(
                Holder::Placed(p.kind as u16),
                p.at,
                p.z,
                stacks,
                slots,
                false,
                p.lock.map(|l| l as u16),
            );
        }
        raid
    }
```

- Add:

```rust
impl World {
    /// The raid's part of a step, after the fight's: searches, then bodies.
    pub(crate) fn step_raid(&mut self) {
        self.step_searches();
        self.step_bodies();
    }

    /// Searches move on (spec §8.2): a searcher who stays by the container and up finishes it,
    /// heard every `noise_every` seconds while it lasts (§5.2), and then has it open; one who
    /// walks off or falls stops. What is open closes when its hero is no longer by it.
    pub(crate) fn step_searches(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.run;
        let mut sounds: Vec<(PlayerId, V3, Loudness)> = Vec::new();
        let mut finished: Vec<(PlayerId, u32)> = Vec::new();
        {
            let heroes = &self.heroes;
            let Raid {
                searches,
                containers,
                ..
            } = &mut self.raid;
            searches.retain_mut(|s| {
                let Some(h) = heroes.iter().find(|h| h.player == s.player) else {
                    return false;
                };
                let Some(c) = containers.iter().find(|c| c.id == s.id) else {
                    return false;
                };
                let by = h.fight.up()
                    && h.pos.distance(s.from) <= SEARCH_STRAY
                    && in_reach(h.pos, h.z, c.at, c.z, r.reach);
                if !by {
                    return false;
                }
                s.noise_in -= DT;
                if s.noise_in <= 0.0 {
                    s.noise_in += r.noise_every;
                    if let (_, Some(sound)) = search_of(c.holder, &content) {
                        sounds.push((s.player, V3::at(c.at, c.z.unwrap_or(h.z)), sound));
                    }
                }
                s.left -= DT;
                if s.left <= 1e-4 {
                    finished.push((s.player, s.id));
                    return false;
                }
                true
            });
        }
        for (player, id) in finished {
            if let Some(c) = self.raid.containers.iter_mut().find(|c| c.id == id) {
                c.searched = true;
            }
            self.raid.close(player);
            self.raid.opened.push((player, id));
        }
        for (by, at, sound) in sounds {
            self.make_noise(by, at, sound);
        }
        let heroes = &self.heroes;
        let Raid {
            opened, containers, ..
        } = &mut self.raid;
        opened.retain(|(p, id)| {
            let Some(h) = heroes.iter().find(|h| h.player == *p) else {
                return false;
            };
            containers
                .iter()
                .find(|c| c.id == *id)
                .is_some_and(|c| h.fight.up() && in_reach(h.pos, h.z, c.at, c.z, r.reach))
        });
    }

    /// Every enemy dead since the last step leaves a body (spec §8.2) holding a roll of its
    /// kind's table, when it has one and the roll holds something.
    pub(crate) fn step_bodies(&mut self) {
        let content = self.content().clone();
        let fresh: Vec<(u32, usize, V2, f32)> = self
            .enemies
            .iter()
            .filter(|e| e.hp <= 0.0 && !self.raid.bodied.contains(&e.id))
            .map(|e| (e.id, e.kind, e.pos, e.z))
            .collect();
        for (id, kind, at, z) in fresh {
            self.raid.bodied.push(id);
            let Some(&(_, table)) = content.loot.bodies.iter().find(|(k, _)| *k == kind) else {
                continue;
            };
            let mut dice = Dice::new(seed_for(self.raid.roll.seed ^ BODY_SEED, u64::from(id)));
            let stacks = roll_table(&content.loot.tables[table], &mut dice, &content);
            if !stacks.is_empty() {
                let slots = usize::from(content.rules.run.container_slots);
                self.raid
                    .add_container(Holder::Body(id), at, Some(z), stacks, slots, false, None);
            }
        }
    }
}
```

In `crates/nightfall-sim/src/world.rs`'s `step`, right after `self.step_fighters();`, add:

```rust
        // The raid after the fight: a search goes on or stops, the dead leave bodies; before the
        // enemies step, so the noises made here are heard this step.
        self.step_raid();
```

- [ ] **Step 3: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 8 new tests with every earlier one).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: containers rolled from their tables by the raid's seed, searched aloud; bodies; piles

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Exits and extracting, deaths and bodies, the raid's end and its outcomes

**Files:**
- Create: `crates/nightfall-sim/src/raid/extract.rs`
- Modify: `crates/nightfall-sim/src/raid/mod.rs` (`mod extract;`, `Raid::counted`), `crates/nightfall-sim/src/raid/containers.rs` (`step_raid`), `crates/nightfall-sim/src/world.rs` (`step`), `crates/nightfall-sim/src/combat/mod.rs` (`drain_events`), `crates/nightfall-sim/src/raid/tests.rs`

**Interfaces:**
- Consumes: `Raid`, `Exiting`, `Holder`, `HeroOutcome`, `Fate`, `Recovery`, `RaidRoll`, `add_container`, `take_pack`, `take_recovered`, `kills_of`, `add_kill`, `close` (Task 3); `Inventory::{split_for_death, clear_found}` (Task 2); `RunRules::{extract_for, death_loss, container_slots}`, `ExitDef`, `DeathLoss` (Task 1); `Event::EnemyDied`, `World::{events, drain_events, hurt_hero, kill_enemy, remove_hero, make_noise}`, `Life`, `Loudness::Extract` (earlier plans).
- Produces:
  - `World::{take_outcomes(&mut self) -> Vec<HeroOutcome>, outcomes(&self) -> &[HeroOutcome], is_over(&self) -> bool, abandon(&mut self)}`.
  - Crate-only `exit_at(&Content, &RaidRoll, now: f32, pos: V2) -> Option<usize>`; `World::{tally_kills, step_exits, step_deaths, step_over}`; `Raid::counted: usize`.
  - A hero up in an open exit's ring counts `Carry::extracting` up, heard every second (§5.2); a blow, leaving the ring or falling starts it over; at `extract_for` they are out: their outcome is final and they leave the world. A hero dead since the last step leaves a `Holder::Hero` body holding what `death_loss` takes. With every hero out or dead the raid is over and the dead's outcomes are settled (with `everything_one_recovery`, a `Recovery` of what is left on the body). `abandon` ends the raid now: the dead are settled as dead, everyone else is missing.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/raid/tests.rs`:

```rust
fn body_of(w: &World, player: PlayerId) -> &Container {
    w.raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(player))
        .unwrap()
}

fn die(w: &mut World, player: PlayerId) {
    let hi = w.hero_slot(player).unwrap();
    w.heroes[hi].fight.life = Life::Dead;
    w.step();
}

#[test]
fn ten_seconds_in_an_open_exit_gets_a_hero_out() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2)], &[], &[]));
    steps(&mut w, (5.0 / DT) as usize);
    assert!((w.hero(0).unwrap().carry.extracting - 5.0).abs() < 0.1);
    steps(&mut w, (5.1 / DT) as usize);
    assert!(w.hero(0).is_none());
    let out = w.take_outcomes();
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].fate,
        Fate::Extracted {
            exit: "The Bridge".into()
        }
    );
    assert_eq!(out[0].carried.count_of(item(&w, "bread")), 2);
    assert!(out[0].time > 9.9);
    assert!(w.is_over());
}

#[test]
fn extracting_is_heard_every_second() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    steps(&mut w, 3);
    assert!(w.hero(0).unwrap().noise >= 25.0);
}

#[test]
fn a_blow_starts_the_count_over() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    steps(&mut w, (9.0 / DT) as usize);
    w.hurt_hero(0, 10.0);
    w.step();
    assert!(w.hero(0).unwrap().carry.extracting < 0.1);
    steps(&mut w, (9.0 / DT) as usize);
    assert!(w.hero(0).is_some());
    steps(&mut w, (1.2 / DT) as usize);
    assert!(w.hero(0).is_none());
}

#[test]
fn the_cart_opens_late() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 11.0);
    steps(&mut w, (12.0 / DT) as usize);
    assert_eq!(w.hero(0).unwrap().carry.extracting, 0.0);
    let cart = w.loot_view(Some(0)).exits[1];
    assert!(!cart.open);
    assert!((cart.opens_in.unwrap() - 8.0).abs() < 0.2);
    steps(&mut w, (18.5 / DT) as usize);
    assert!(w.hero(0).is_none(), "out once the cart opened and ten seconds passed");
}

#[test]
fn the_downed_do_not_extract() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.heroes[0].fight.life = Life::Downed {
        left: 40.0,
        revive: 0.0,
    };
    steps(&mut w, (11.0 / DT) as usize);
    let h = w.hero(0).unwrap();
    assert_eq!(h.carry.extracting, 0.0);
}

#[test]
fn death_takes_the_gear_to_the_body_and_keeps_the_safe_slot() {
    let mut w = world_with("death_loss: everything", "");
    join(&mut w, 0, "rogue", 20.0, 20.0);
    w.give_loadout(
        0,
        &loadout(&w, &[("bread", 1)], &[("knife", EquipSlot::MainHand)], &[("potion", 1)]),
    );
    die(&mut w, 0);
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(
        body,
        vec![Stack::new(item(&w, "knife"), 1), Stack::new(item(&w, "bread"), 1)]
    );
    assert!(w.is_over());
    let out = w.take_outcomes();
    assert_eq!(out[0].fate, Fate::Died);
    assert_eq!(
        out[0].carried.stacks(),
        vec![(Place::Safe(0), Stack::new(item(&w, "potion"), 1))]
    );
    assert_eq!(out[0].recovery, None);
}

#[test]
fn everything_one_recovery_leaves_the_body_for_the_next_raid() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    w.give_loadout(0, &loadout(&w, &[("gem", 3)], &[("knife", EquipSlot::MainHand)], &[]));
    die(&mut w, 0);
    let out = w.take_outcomes();
    let r = out[0].recovery.clone().unwrap();
    assert_eq!(r.map, "test");
    assert!(r.at.ground().distance(V2::new(20.0, 20.0)) < 1e-4);
    assert_eq!(r.items.len(), 2);
    // The next raid: the remains lie where the hero fell, seen only by their player.
    let mut next = world();
    join(&mut next, 0, "rogue", 19.0, 20.0);
    next.give_loadout(
        0,
        &Loadout {
            inventory: Inventory::new(next.content()),
            recovery: Some(r.clone()),
        },
    );
    let mine = next.loot_view(Some(0));
    let remains = mine
        .marks
        .iter()
        .find(|m| m.holder == Holder::Remains(0))
        .unwrap()
        .id;
    assert!(next
        .loot_view(Some(1))
        .marks
        .iter()
        .all(|m| m.holder != Holder::Remains(0)));
    act(&mut next, 0, 1, Act::Open(remains));
    next.step();
    assert!(next.raid.containers.iter().all(|c| c.id != remains));
    stand(&mut next, 0, 50.0, 49.0);
    steps(&mut next, (10.3 / DT) as usize);
    let out = next.take_outcomes();
    assert_eq!(out[0].recovered.len(), 2);
    assert!(out[0].recovered.iter().all(|s| !s.found));
    assert_eq!(out[0].recovery, None);
}

#[test]
fn found_only_loses_only_what_was_found() {
    let mut w = world_with("death_loss: found_only", "");
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    act(&mut w, 0, 1, Act::Open(chest(&w)));
    steps(&mut w, (2.2 / DT) as usize);
    act(&mut w, 0, 2, Act::Take(0));
    w.step();
    die(&mut w, 0);
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(body, vec![Stack::found(item(&w, "gem"), 6)]);
    let out = w.take_outcomes();
    assert_eq!(out[0].carried.count_of(item(&w, "bread")), 1);
    assert_eq!(out[0].carried.count_of(item(&w, "gem")), 0);
    assert_eq!(out[0].recovery, None);
}

/// Every stack of `key` in the world's packs and containers.
fn held(w: &World, key: &str) -> u32 {
    let i = item(w, key);
    let packs: u32 = w.raid.packs.iter().map(|(_, inv)| inv.count_of(i)).sum();
    let boxes: u32 = w
        .raid
        .containers
        .iter()
        .flat_map(|c| c.stacks.iter().flatten())
        .filter(|s| usize::from(s.item) == i)
        .map(|s| u32::from(s.count))
        .sum();
    packs + boxes
}

#[test]
fn a_friend_loots_a_fallen_heros_body() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    join(&mut w, 1, "rogue", 21.0, 20.0);
    stand(&mut w, 0, 20.0, 20.0);
    stand(&mut w, 1, 21.0, 20.0);
    w.step();
    w.give_loadout(0, &loadout(&w, &[("gem", 6)], &[("knife", EquipSlot::MainHand)], &[]));
    w.give_loadout(1, &loadout(&w, &[], &[], &[]));
    die(&mut w, 0);
    let body = body_of(&w, 0).id;
    act(&mut w, 1, 1, Act::Open(body));
    steps(&mut w, (1.7 / DT) as usize);
    // The body holds the worn knife first, then the backpack's gems.
    act(&mut w, 1, 2, Act::Take(1));
    w.step();
    assert_eq!(w.pack(1).unwrap().count_of(item(&w, "gem")), 6);
    stand(&mut w, 1, 50.0, 49.0);
    steps(&mut w, (10.3 / DT) as usize);
    assert!(w.is_over());
    let out = w.take_outcomes();
    let friend = out.iter().find(|o| o.player == 1).unwrap();
    let fallen = out.iter().find(|o| o.player == 0).unwrap();
    assert_eq!(friend.carried.count_of(item(&w, "gem")), 6);
    assert_eq!(fallen.fate, Fate::Died);
    let left = &fallen.recovery.as_ref().unwrap().items;
    assert_eq!(left, &vec![Stack::new(item(&w, "knife"), 1)]);
}

#[test]
fn nothing_is_made_or_lost_between_the_body_the_packs_and_the_outcomes() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    join(&mut w, 1, "rogue", 21.0, 20.0);
    stand(&mut w, 0, 20.0, 20.0);
    stand(&mut w, 1, 21.0, 20.0);
    w.step();
    w.give_loadout(
        0,
        &loadout(&w, &[("gem", 6), ("rock", 2)], &[("knife", EquipSlot::MainHand)], &[("bread", 1)]),
    );
    w.give_loadout(1, &loadout(&w, &[("bread", 2)], &[], &[]));
    let before: Vec<u32> = ["gem", "rock", "knife", "bread"].iter().map(|k| held(&w, k)).collect();
    die(&mut w, 0);
    let body = body_of(&w, 0).id;
    act(&mut w, 1, 1, Act::Open(body));
    steps(&mut w, (1.7 / DT) as usize);
    act(&mut w, 1, 2, Act::TakeAll);
    w.step();
    act(&mut w, 1, 3, Act::Drop(Place::Pack(0)));
    w.step();
    let after: Vec<u32> = ["gem", "rock", "knife", "bread"].iter().map(|k| held(&w, k)).collect();
    assert_eq!(before, after);
}

#[test]
fn each_outcome_is_taken_once() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    join(&mut w, 1, "rogue", 20.0, 20.0);
    stand(&mut w, 0, 50.0, 49.0);
    stand(&mut w, 1, 20.0, 20.0);
    w.step();
    die(&mut w, 1);
    assert!(!w.is_over());
    assert!(w.take_outcomes().is_empty(), "the dead are settled when the raid is over");
    steps(&mut w, (10.3 / DT) as usize);
    let out = w.take_outcomes();
    assert_eq!(out.len(), 2);
    assert!(matches!(out[0].fate, Fate::Extracted { .. }));
    assert_eq!(out[1].fate, Fate::Died);
    assert!(w.take_outcomes().is_empty());
    assert_eq!(w.outcomes().len(), 2);
}

#[test]
fn abandoning_counts_everyone_still_in_as_missing() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 20.0, 20.0);
    stand(&mut w, 0, 5.0, 5.0);
    stand(&mut w, 1, 20.0, 20.0);
    w.step();
    w.give_loadout(0, &loadout(&w, &[], &[("knife", EquipSlot::MainHand)], &[("bread", 1)]));
    w.give_loadout(1, &loadout(&w, &[("gem", 2)], &[], &[]));
    die(&mut w, 1);
    let bodies = w.raid.containers.len();
    w.abandon();
    assert!(w.is_over());
    assert_eq!(w.raid.containers.len(), bodies, "the missing leave no body");
    let out = w.take_outcomes();
    let missing = out.iter().find(|o| o.player == 0).unwrap();
    assert_eq!(missing.fate, Fate::Missing);
    assert_eq!(missing.carried.stacks().len(), 1);
    assert_eq!(missing.carried.count_of(item(&w, "bread")), 1);
    assert_eq!(missing.recovery, None);
    let dead = out.iter().find(|o| o.player == 1).unwrap();
    assert_eq!(dead.fate, Fate::Died);
    assert!(dead.recovery.is_some());
}

#[test]
fn a_kill_counts_for_its_hero() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    w.kill_enemy(0, Some(0));
    w.step();
    w.abandon();
    assert_eq!(w.take_outcomes()[0].kills, 1);
}
```

Run: `cargo test -p nightfall-sim raid`
Expected: FAIL to compile (`no method named take_outcomes`).

- [ ] **Step 2: Write the end of a hero's raid**

`crates/nightfall-sim/src/raid/extract.rs`:

```rust
//! The end of a hero's raid (spec §8.1, §8.2): out by an exit after `extract_for` seconds in its
//! ring, or dead with their body left holding what `death_loss` takes; the raid over once every
//! hero is out or dead, each hero's outcome settled once and handed out once.

use super::{Exiting, Fate, HeroOutcome, Holder, Raid, RaidRoll, Recovery};
use crate::{
    Content, DeathLoss, Event, Inventory, Life, Loudness, PlayerId, Stack, World, DT, V2, V3,
};

/// The exit (an index into the map's exits) whose open ring `pos` stands in now. Rings are flat:
/// no exit stands over another floor.
pub(crate) fn exit_at(content: &Content, roll: &RaidRoll, now: f32, pos: V2) -> Option<usize> {
    content
        .map
        .exits
        .iter()
        .enumerate()
        .find(|(i, e)| {
            roll.exits
                .get(*i)
                .is_some_and(|p| p.open && now >= p.opens_at)
                && pos.distance(e.at) <= e.radius
        })
        .map(|(i, _)| i)
}

impl World {
    /// Every enemy killed since the last tally counts for the hero who killed it. Last in a step,
    /// so every kill of the step counts; [`World::drain_events`] starts the count over.
    pub(crate) fn tally_kills(&mut self) {
        let from = self.raid.counted.min(self.events.len());
        let by: Vec<PlayerId> = self.events[from..]
            .iter()
            .filter_map(|e| match e {
                Event::EnemyDied { by: Some(p), .. } => Some(*p),
                _ => None,
            })
            .collect();
        self.raid.counted = self.events.len();
        for p in by {
            self.raid.add_kill(p);
        }
    }

    /// Heroes up in an open exit's ring count to `extract_for` (spec §8.1), heard as they step in
    /// and every second after (§5.2); a blow, stepping out or falling starts the count over.
    /// At the end of the count they are out.
    pub(crate) fn step_exits(&mut self) {
        let content = self.content().clone();
        let r = &content.rules.run;
        let now = self.elapsed;
        let mut ticks: Vec<(PlayerId, V3)> = Vec::new();
        let mut out: Vec<(PlayerId, usize)> = Vec::new();
        {
            let Raid { exiting, roll, .. } = &mut self.raid;
            for h in self.heroes.iter_mut() {
                let i = match exiting.iter().position(|e| e.player == h.player) {
                    Some(i) => i,
                    None => {
                        exiting.push(Exiting {
                            player: h.player,
                            t: 0.0,
                            last_hp: h.hp,
                        });
                        exiting.len() - 1
                    }
                };
                let e = &mut exiting[i];
                let hurt = h.hp < e.last_hp - 1e-4;
                e.last_hp = h.hp;
                let ring = if h.fight.up() {
                    exit_at(&content, roll, now, h.pos)
                } else {
                    None
                };
                match ring {
                    Some(x) if !hurt => {
                        if e.t == 0.0 || (e.t + DT).floor() > e.t.floor() {
                            ticks.push((h.player, V3::at(h.pos, h.z)));
                        }
                        e.t += DT;
                        if e.t + 1e-4 >= r.extract_for {
                            out.push((h.player, x));
                        }
                    }
                    _ => e.t = 0.0,
                }
                h.carry.extracting = e.t;
            }
        }
        for (p, at) in ticks {
            self.make_noise(p, at, Loudness::Extract);
        }
        for (p, x) in out {
            self.extract(p, x);
        }
    }

    /// `player` is out by exit `exit`: everything they carry is theirs to keep, and they leave
    /// the world.
    fn extract(&mut self, player: PlayerId, exit: usize) {
        let content = self.content().clone();
        let mut carried = self
            .raid
            .take_pack(player)
            .unwrap_or_else(|| Inventory::new(&content));
        carried.clear_found();
        let fate = Fate::Extracted {
            exit: content.map.exits[exit].name.clone(),
        };
        let o = self.outcome(player, fate, carried, None);
        self.raid.outcomes.push(o);
        self.remove_hero(player);
    }

    /// Every hero dead since the last step leaves their body where they fell (spec §8.2),
    /// holding what `death_loss` takes; friends can search it this raid.
    pub(crate) fn step_deaths(&mut self) {
        let content = self.content().clone();
        let mode = content.rules.run.death_loss;
        let dead: Vec<(PlayerId, V2, f32)> = self
            .heroes
            .iter()
            .filter(|h| {
                h.fight.life == Life::Dead && !self.raid.fallen.iter().any(|(p, _)| *p == h.player)
            })
            .map(|h| (h.player, h.pos, h.z))
            .collect();
        for (player, at, z) in dead {
            let lost = self
                .raid
                .pack_mut(player, &content)
                .split_for_death(mode, &content);
            let body = if lost.is_empty() {
                0
            } else {
                let slots = usize::from(content.rules.run.container_slots);
                self.raid
                    .add_container(Holder::Hero(player), at, Some(z), lost, slots, false, None)
            };
            self.raid.fallen.push((player, body));
            self.raid.close(player);
            if let Some(hi) = self.hero_slot(player) {
                self.heroes[hi].carry.lantern = None;
            }
            self.refresh_carry(player);
        }
    }

    /// With every hero out or dead the raid is over (spec §8.1), and each dead hero's outcome is
    /// settled now: what is left on their body by then is what a recovery holds.
    pub(crate) fn step_over(&mut self) {
        if !self.heroes.is_empty() {
            self.raid.had_heroes = true;
        }
        if self.raid.over || !self.raid.had_heroes {
            return;
        }
        if !self.heroes.iter().all(|h| h.fight.life == Life::Dead) {
            return;
        }
        self.raid.over = true;
        let dead: Vec<PlayerId> = self.heroes.iter().map(|h| h.player).collect();
        for p in dead {
            self.settle_dead(p);
        }
    }

    /// Dead `player`'s outcome: what death left them, and under `everything_one_recovery` their
    /// body's remaining things as the recovery their next raid holds.
    fn settle_dead(&mut self, player: PlayerId) {
        let content = self.content().clone();
        let mut carried = self
            .raid
            .take_pack(player)
            .unwrap_or_else(|| Inventory::new(&content));
        carried.clear_found();
        let body = self
            .raid
            .fallen
            .iter()
            .find(|(p, _)| *p == player)
            .map_or(0, |(_, id)| *id);
        let recovery = if content.rules.run.death_loss == DeathLoss::EverythingOneRecovery {
            self.raid
                .containers
                .iter()
                .find(|c| c.id == body)
                .and_then(|c| {
                    let items: Vec<Stack> = c
                        .stacks
                        .iter()
                        .flatten()
                        .map(|s| Stack { found: false, ..*s })
                        .collect();
                    (!items.is_empty()).then(|| Recovery {
                        map: content.map.key.clone(),
                        at: V3::at(c.at, c.z.unwrap_or(0.0)),
                        items,
                    })
                })
        } else {
            None
        };
        let o = self.outcome(player, Fate::Died, carried, recovery);
        self.raid.outcomes.push(o);
    }

    fn outcome(
        &mut self,
        player: PlayerId,
        fate: Fate,
        carried: Inventory,
        recovery: Option<Recovery>,
    ) -> HeroOutcome {
        let name = self.hero(player).map(|h| h.name.clone()).unwrap_or_default();
        let mut recovered = self.raid.take_recovered(player);
        for s in &mut recovered {
            s.found = false;
        }
        HeroOutcome {
            player,
            name,
            fate,
            time: self.elapsed,
            kills: self.raid.kills_of(player),
            carried,
            recovered,
            recovery,
        }
    }

    /// The outcomes settled since the last call: each handed out once, for the host to give to
    /// its player.
    pub fn take_outcomes(&mut self) -> Vec<HeroOutcome> {
        let new = self.raid.outcomes[self.raid.taken..].to_vec();
        self.raid.taken = self.raid.outcomes.len();
        new
    }

    /// Every outcome so far, for the end screen.
    pub fn outcomes(&self) -> &[HeroOutcome] {
        &self.raid.outcomes
    }

    /// Every hero is out or dead, or the raid was abandoned.
    pub fn is_over(&self) -> bool {
        self.raid.over
    }

    /// The raid ends now (its host abandoned it): every hero still in it is missing, losing what
    /// a death would and leaving no body; the dead are settled as dead.
    pub fn abandon(&mut self) {
        if self.raid.over {
            return;
        }
        let content = self.content().clone();
        self.raid.over = true;
        self.raid.had_heroes = true;
        let heroes: Vec<(PlayerId, bool)> = self
            .heroes
            .iter()
            .map(|h| (h.player, h.fight.life == Life::Dead))
            .collect();
        for (p, dead) in heroes {
            if dead {
                self.settle_dead(p);
                continue;
            }
            let mut carried = self
                .raid
                .take_pack(p)
                .unwrap_or_else(|| Inventory::new(&content));
            let _ = carried.split_for_death(content.rules.run.death_loss, &content);
            carried.clear_found();
            let o = self.outcome(p, Fate::Missing, carried, None);
            self.raid.outcomes.push(o);
        }
    }
}
```

In `crates/nightfall-sim/src/raid/mod.rs`, add `mod extract;` beside `mod acts;`, and `Raid` gains:

```rust
    /// Events already tallied for kills ([`World::tally_kills`]).
    pub(crate) counted: usize,
```

In `crates/nightfall-sim/src/raid/containers.rs`, `step_raid` becomes:

```rust
    /// The raid's part of a step, after the fight's: searches, bodies, deaths, then exits.
    pub(crate) fn step_raid(&mut self) {
        self.step_searches();
        self.step_bodies();
        self.step_deaths();
        self.step_exits();
    }
```

In `crates/nightfall-sim/src/world.rs`'s `step`, before the `// Noises are heard the step they are made, or the next; none waits longer.` comment, add:

```rust
        // Last: every kill of the step counted, then whether the raid is over.
        self.tally_kills();
        self.step_over();
```

In `crates/nightfall-sim/src/combat/mod.rs`, `World::drain_events` sets `self.raid.counted = 0;` before it takes the events (the tally's cursor goes with them).

- [ ] **Step 3: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 13 new tests with every earlier one).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: exits and the 10-second ring; bodies by death_loss; the raid's end, outcomes and abandoning

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The profile: stash, gold, loadout, the free kit, settling raids

**Files:**
- Create: `crates/nightfall-sim/src/profile.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `Inventory`, `Stack`, `Place`, `MAX_BAGS` (Task 2); `HeroOutcome`, `Fate`, `Loadout`, `Recovery` (Task 3); `Content::{item, class, items, classes, map, rules.run}`, `ClassDef::free_items`, `ItemKind`, `EquipSlot` (Task 1); `V3`.
- Produces:
  - `nightfall_sim::{Profile, ProfileError, StashError, SavedStack, SavedInventory, SavedRecovery, PROFILE_VERSION, free_kit, saved_inventory, inventory_of}`.
  - `PROFILE_VERSION: u32 = 1`; `SavedStack { item: String, count: u16 }`; `SavedInventory { equip: Vec<(EquipSlot, SavedStack)>, bags: Vec<(u8, SavedStack)>, pack: Vec<SavedStack>, safe: Vec<(u8, SavedStack)> }` with `is_empty()`; `SavedRecovery { map: String, at: (f32, f32, f32), items: Vec<SavedStack> }`.
  - `Profile { version, class: String, gold: u32, stash: Vec<SavedStack>, loadout: SavedInventory, in_raid: Option<SavedInventory>, recovery: Option<SavedRecovery>, raids: u32, extracted: u32 }` with `from_ron(&str) -> Result<Profile, ProfileError>`, `to_ron(&self) -> String`, `class_index(&self, &Content) -> Option<usize>`, `raid_loadout(&self, class: usize, &Content) -> Loadout`, `begin_raid(&mut self, class: usize, &Content)`, `settle(&mut self, &HeroOutcome, &Content)`, `settle_missing(&mut self, &Content) -> bool`, `stash_capacity(&self, &Content) -> usize`, `take_from_stash(&mut self, index: usize, &Content) -> Result<(), StashError>`, `put_in_stash(&mut self, Place, &Content) -> Result<(), StashError>`, `sell(&mut self, index: usize, &Content) -> Result<u32, StashError>`, `stash_add(&mut self, SavedStack, &Content)`, `loadout_move(&mut self, from: Place, to: Place, &Content) -> Result<(), StashError>`.
  - `ProfileError::{Parse(String), Newer { found: u32, known: u32 }}` (`Display`); `StashError::{InRaid, NoSuchSlot, Unknown, StashFull, LoadoutFull, WrongSlot, BagInUse, Worthless}` with `message(self) -> &'static str`.
  - `free_kit(class: usize, &Content) -> Inventory`; `saved_inventory(&Inventory, &Content) -> SavedInventory`; `inventory_of(&SavedInventory, &Content) -> (Inventory, Vec<SavedStack>)`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/profile.rs` ends with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentSources, Fate, Recovery};

    fn content(run: &str) -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0, free_kit: [("knife", 1), ("bread", 2)])]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: &format!(
                r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                run: (backpack_slots: 3, safe_slots: 1, stash_slots: 2, {run}))"#
            ),
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand)),
                (key: "helm", name: "Helm", kind: Armour, icon: "a", weight: 2.0, slot: Some(Head),
                 value: 10),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 5),
                (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10,
                 value: 40),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 2),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    const KNIFE: usize = 0;
    const HELM: usize = 1;
    const BREAD: usize = 2;
    const GEM: usize = 3;

    fn stack(item: &str, count: u16) -> SavedStack {
        SavedStack {
            item: item.into(),
            count,
        }
    }

    fn outcome(c: &Content, carried: Inventory, recovered: Vec<Stack>, fate: Fate) -> HeroOutcome {
        let _ = c;
        HeroOutcome {
            player: 0,
            name: "A".into(),
            fate,
            time: 100.0,
            kills: 2,
            carried,
            recovered,
            recovery: None,
        }
    }

    #[test]
    fn a_profile_round_trips() {
        let mut p = Profile::default();
        p.gold = 7;
        p.class = "rogue".into();
        p.stash.push(stack("gem", 2));
        p.loadout.equip.push((EquipSlot::Head, stack("helm", 1)));
        p.recovery = Some(SavedRecovery {
            map: "test".into(),
            at: (1.0, 2.0, 3.0),
            items: vec![stack("bread", 1)],
        });
        assert_eq!(Profile::from_ron(&p.to_ron()).unwrap(), p);
    }

    #[test]
    fn an_old_file_without_a_version_loads() {
        let old = Profile::from_ron(r#"(gold: 12, stash: [(item: "bread", count: 3)])"#).unwrap();
        assert_eq!(old.version, PROFILE_VERSION);
        assert_eq!(old.gold, 12);
        assert_eq!(old.stash, vec![stack("bread", 3)]);
    }

    #[test]
    fn a_newer_profile_is_refused_not_misread() {
        assert_eq!(
            Profile::from_ron("(version: 99, gold: 5)"),
            Err(ProfileError::Newer {
                found: 99,
                known: PROFILE_VERSION
            })
        );
        assert!(matches!(Profile::from_ron("(gold: "), Err(ProfileError::Parse(_))));
        assert!(matches!(Profile::from_ron("(gold: \"lots\")"), Err(ProfileError::Parse(_))));
    }

    #[test]
    fn an_empty_loadout_raids_with_the_free_kit() {
        let c = content("");
        let mut p = Profile::default();
        let lo = p.raid_loadout(0, &c);
        assert_eq!(
            lo.inventory.get(Place::Equip(EquipSlot::MainHand)),
            Some(&Stack::new(KNIFE, 1))
        );
        assert_eq!(lo.inventory.count_of(BREAD), 2);
        assert_eq!(lo.recovery, None);
        p.begin_raid(0, &c);
        assert!(p.in_raid.is_some());
        assert!(p.loadout.is_empty());
        assert_eq!(p.raids, 1);
        assert_eq!(p.raid_loadout(0, &c).inventory, lo.inventory);
        p.begin_raid(0, &c);
        assert_eq!(p.raids, 1, "a raid under way does not begin again");
    }

    #[test]
    fn beginning_a_raid_moves_the_loadout_out_and_settling_brings_back_what_came_out() {
        let c = content("");
        let mut p = Profile::default();
        p.loadout.pack.push(stack("gem", 2));
        p.loadout.equip.push((EquipSlot::Head, stack("helm", 1)));
        p.begin_raid(0, &c);
        assert!(p.loadout.is_empty());
        assert_eq!(p.in_raid.as_ref().unwrap().pack, vec![stack("gem", 2)]);
        let mut carried = Inventory::new(&c);
        carried.add(Stack::new(GEM, 5), &c);
        let o = outcome(
            &c,
            carried,
            vec![Stack::new(BREAD, 3)],
            Fate::Extracted { exit: "x".into() },
        );
        p.settle(&o, &c);
        assert_eq!(p.in_raid, None);
        assert_eq!(p.loadout.pack, vec![stack("gem", 5)]);
        assert_eq!(p.stash, vec![stack("bread", 3)]);
        assert_eq!(p.extracted, 1);
    }

    #[test]
    fn a_raid_that_never_ended_counts_as_missing() {
        let brought = |p: &mut Profile| {
            p.loadout.equip.push((EquipSlot::MainHand, stack("knife", 1)));
            p.loadout.pack.push(stack("gem", 4));
            p.loadout.safe.push((0, stack("bread", 1)));
        };
        let c = content("");
        let mut p = Profile::default();
        brought(&mut p);
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.loadout.safe, vec![(0, stack("bread", 1))]);
        assert!(p.loadout.equip.is_empty() && p.loadout.pack.is_empty());
        assert!(!p.settle_missing(&c), "nothing under way: nothing to settle");
        let c = content("death_loss: found_only");
        let mut p = Profile::default();
        brought(&mut p);
        p.begin_raid(0, &c);
        assert!(p.settle_missing(&c));
        assert_eq!(p.loadout.pack, vec![stack("gem", 4)], "found_only keeps what was brought");
    }

    #[test]
    fn recovered_things_go_to_the_stash_and_the_new_body_replaces_the_old() {
        let c = content("");
        let mut p = Profile::default();
        p.recovery = Some(SavedRecovery {
            map: "test".into(),
            at: (1.0, 2.0, 3.0),
            items: vec![stack("gem", 1)],
        });
        let r = p.raid_loadout(0, &c).recovery.unwrap();
        assert_eq!(r.items, vec![Stack::new(GEM, 1)]);
        assert_eq!(r.at, V3::new(1.0, 2.0, 3.0));
        p.begin_raid(0, &c);
        let mut died = outcome(&c, Inventory::new(&c), Vec::new(), Fate::Died);
        died.recovery = Some(Recovery {
            map: "test".into(),
            at: V3::new(5.0, 6.0, 7.0),
            items: vec![Stack::new(KNIFE, 1)],
        });
        p.settle(&died, &c);
        let saved = p.recovery.clone().unwrap();
        assert_eq!(saved.at, (5.0, 6.0, 7.0));
        assert_eq!(saved.items, vec![stack("knife", 1)]);
        // Out without reaching it: its one chance is gone.
        p.begin_raid(0, &c);
        let out = outcome(&c, Inventory::new(&c), Vec::new(), Fate::Extracted { exit: "x".into() });
        p.settle(&out, &c);
        assert_eq!(p.recovery, None);
        // A body on another map waits for that map.
        p.recovery = Some(SavedRecovery {
            map: "elsewhere".into(),
            at: (0.0, 0.0, 0.0),
            items: vec![stack("gem", 1)],
        });
        p.begin_raid(0, &c);
        p.settle(&out, &c);
        assert_eq!(p.recovery.as_ref().unwrap().map, "elsewhere");
    }

    #[test]
    fn stash_moves_respect_room_and_selling_pays() {
        let c = content("");
        let mut p = Profile::default();
        p.stash = vec![stack("gem", 3), stack("knife", 1)];
        p.take_from_stash(1, &c).unwrap();
        assert_eq!(p.loadout.equip, vec![(EquipSlot::MainHand, stack("knife", 1))]);
        assert_eq!(p.stash, vec![stack("gem", 3)]);
        p.loadout.pack.push(stack("bread", 2));
        p.loadout.pack.push(stack("helm", 1));
        p.put_in_stash(Place::Equip(EquipSlot::MainHand), &c).unwrap();
        assert_eq!(p.stash.len(), 2);
        assert_eq!(p.put_in_stash(Place::Pack(0), &c), Err(StashError::StashFull));
        assert_eq!(p.sell(0, &c), Ok(120));
        assert_eq!(p.gold, 120);
        assert_eq!(p.sell(0, &c), Err(StashError::Worthless));
        assert_eq!(p.stash, vec![stack("knife", 1)]);
        p.begin_raid(0, &c);
        assert_eq!(p.take_from_stash(0, &c), Err(StashError::InRaid));
    }

    #[test]
    fn the_loadout_moves_between_its_own_slots() {
        let c = content("");
        let mut p = Profile::default();
        p.loadout.pack.push(stack("bread", 2));
        p.loadout_move(Place::Pack(0), Place::Safe(0), &c).unwrap();
        assert_eq!(p.loadout.safe, vec![(0, stack("bread", 2))]);
        assert!(p.loadout.pack.is_empty());
        assert_eq!(
            p.loadout_move(Place::Safe(0), Place::Equip(EquipSlot::Head), &c),
            Err(StashError::WrongSlot)
        );
        assert_eq!(p.loadout_move(Place::Pack(2), Place::Safe(0), &c), Err(StashError::NoSuchSlot));
    }

    #[test]
    fn unknown_or_overflowing_things_go_back_to_the_stash() {
        let c = content("");
        let mut p = Profile::default();
        p.loadout.pack = vec![stack("dragon_egg", 1), stack("bread", 5), stack("gem", 25)];
        p.begin_raid(0, &c);
        // Three backpack slots: the bread and two full stacks of gems; the egg the content does
        // not know and the five gems with no room go back to the stash, never away.
        assert_eq!(p.stash, vec![stack("dragon_egg", 1), stack("gem", 5)]);
        let raid = p.in_raid.as_ref().unwrap();
        assert_eq!(raid.pack, vec![stack("bread", 5), stack("gem", 10), stack("gem", 10)]);
        assert_eq!(p.take_from_stash(0, &c), Err(StashError::InRaid));
        let _ = HELM;
    }
}
```

Add `mod profile;` and `pub use profile::{free_kit, inventory_of, saved_inventory, Profile, ProfileError, SavedInventory, SavedRecovery, SavedStack, StashError, PROFILE_VERSION};` to `lib.rs`.

Run: `cargo test -p nightfall-sim profile`
Expected: FAIL to compile (`cannot find type Profile`).

- [ ] **Step 2: Write the profile**

Above the tests in `profile.rs`:

```rust
//! A player's things between raids (spec §8.3): the stash, gold, the loadout they will bring,
//! the raid under way and the body left for the next one. Pure and versioned: the client keeps it
//! in `benilla-config/nightfall/stash.ron`. Items are named by key, so a content change never
//! shuffles a stash, and a key the content no longer knows is kept, not dropped.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    Content, EquipSlot, Fate, HeroOutcome, InvError, Inventory, ItemKind, Loadout, Place,
    Recovery, Stack, MAX_BAGS, V3,
};

/// Bumped whenever the file changes shape; [`Profile::from_ron`] brings older files up to it.
pub const PROFILE_VERSION: u32 = 1;

/// Some of one item, by key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedStack {
    pub item: String,
    pub count: u16,
}

/// An inventory, by key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedInventory {
    pub equip: Vec<(EquipSlot, SavedStack)>,
    pub bags: Vec<(u8, SavedStack)>,
    pub pack: Vec<SavedStack>,
    pub safe: Vec<(u8, SavedStack)>,
}

impl SavedInventory {
    pub fn is_empty(&self) -> bool {
        self.equip.is_empty() && self.bags.is_empty() && self.pack.is_empty() && self.safe.is_empty()
    }
}

/// A body waiting for its player's next raid on its map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedRecovery {
    pub map: String,
    pub at: (f32, f32, f32),
    pub items: Vec<SavedStack>,
}

/// A player's things between raids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// The file's shape; a file written before it had one reads as 0.
    #[serde(default)]
    pub version: u32,
    /// The class picked last.
    pub class: String,
    pub gold: u32,
    pub stash: Vec<SavedStack>,
    /// What the next raid brings.
    pub loadout: SavedInventory,
    /// What the raid under way brought: present from its start to its end, so a raid that never
    /// ends (a crash, a lost connection) is settled as missing, never kept twice.
    pub in_raid: Option<SavedInventory>,
    pub recovery: Option<SavedRecovery>,
    pub raids: u32,
    pub extracted: u32,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: PROFILE_VERSION,
            class: String::new(),
            gold: 0,
            stash: Vec::new(),
            loadout: SavedInventory::default(),
            in_raid: None,
            recovery: None,
            raids: 0,
            extracted: 0,
        }
    }
}

/// A profile that does not load, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// Not a profile: cut short, or written by hand wrongly.
    Parse(String),
    /// Written by a newer build: read wrongly it would be saved back wrong, so it is not read.
    Newer { found: u32, known: u32 },
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::Parse(why) => write!(f, "not a stash file: {why}"),
            ProfileError::Newer { found, known } => write!(
                f,
                "a stash from a newer Nightfall (version {found}; this one knows {known})"
            ),
        }
    }
}

/// Why a stash move was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashError {
    InRaid,
    NoSuchSlot,
    Unknown,
    StashFull,
    LoadoutFull,
    WrongSlot,
    BagInUse,
    Worthless,
}

impl StashError {
    /// The line the stash panel shows.
    pub fn message(self) -> &'static str {
        match self {
            StashError::InRaid => "Your things are in a raid.",
            StashError::NoSuchSlot => "Nothing there.",
            StashError::Unknown => "This game does not know that item any more.",
            StashError::StashFull => "The stash is full.",
            StashError::LoadoutFull => "No room left to carry it.",
            StashError::WrongSlot => "That does not go there.",
            StashError::BagInUse => "Empty the bag first.",
            StashError::Worthless => "Nobody would pay for that.",
        }
    }
}

fn saved(s: &Stack, content: &Content) -> Option<SavedStack> {
    Some(SavedStack {
        item: content.items.get(usize::from(s.item))?.key.clone(),
        count: s.count,
    })
}

/// An inventory by key.
pub fn saved_inventory(inv: &Inventory, content: &Content) -> SavedInventory {
    let mut out = SavedInventory::default();
    for (place, s) in inv.stacks() {
        let Some(st) = saved(&s, content) else {
            continue;
        };
        match place {
            Place::Equip(slot) => out.equip.push((slot, st)),
            Place::Bag(i) => out.bags.push((i, st)),
            Place::Pack(_) => out.pack.push(st),
            Place::Safe(i) => out.safe.push((i, st)),
        }
    }
    out
}

/// A saved inventory for this content, and what of it the content does not know or has no room
/// for: kept for the stash, never dropped.
pub fn inventory_of(saved: &SavedInventory, content: &Content) -> (Inventory, Vec<SavedStack>) {
    let mut inv = Inventory::new(content);
    let mut aside: Vec<SavedStack> = Vec::new();
    let mut loose: Vec<Stack> = Vec::new();
    // A stack bigger than its stack size keeps the rest as more stacks, never clipped away.
    let mut known = |s: &SavedStack| -> Option<(Stack, Vec<Stack>)> {
        let Some(i) = content.item(&s.item) else {
            aside.push(s.clone());
            return None;
        };
        let max = content.items[i].stack.max(1);
        let mut rest = Vec::new();
        let mut left = s.count.saturating_sub(max);
        while left > 0 {
            let n = left.min(max);
            rest.push(Stack::new(i, n));
            left -= n;
        }
        Some((Stack::new(i, s.count.min(max)), rest))
    };
    for (slot, s) in &saved.equip {
        if let Some((st, rest)) = known(s) {
            if inv.equip[slot.index()].is_none() {
                inv.equip[slot.index()] = Some(st);
            } else {
                loose.push(st);
            }
            loose.extend(rest);
        }
    }
    for (i, s) in &saved.bags {
        if let Some((st, rest)) = known(s) {
            let b = usize::from(*i);
            if b < MAX_BAGS && inv.bags[b].is_none() {
                inv.bags[b] = Some(st);
            } else {
                loose.push(st);
            }
            loose.extend(rest);
        }
    }
    for s in &saved.pack {
        if let Some((st, rest)) = known(s) {
            loose.push(st);
            loose.extend(rest);
        }
    }
    for (i, s) in &saved.safe {
        if let Some((st, rest)) = known(s) {
            let k = usize::from(*i);
            if k < inv.safe.len() && inv.safe[k].is_none() {
                inv.safe[k] = Some(st);
            } else {
                loose.push(st);
            }
            loose.extend(rest);
        }
    }
    inv.pack = loose.into_iter().map(Some).collect();
    for s in inv.sanitize(content) {
        aside.extend(saved(&s, content));
    }
    (inv, aside)
}

/// The free basic kit of class `class` (spec §8.3): worn where it goes, the rest in the
/// backpack.
pub fn free_kit(class: usize, content: &Content) -> Inventory {
    let mut inv = Inventory::new(content);
    for &(item, count) in &content.classes[class].free_items {
        let stack = Stack::new(item, count);
        match content.items[item].slot {
            Some(s) if inv.equip[s.index()].is_none() => inv.equip[s.index()] = Some(stack),
            _ => {
                let _ = inv.add(stack, content);
            }
        }
    }
    inv
}

impl Profile {
    /// A profile from its file's text: an older shape brought up to this one; a newer one refused,
    /// so it is never saved back wrong.
    pub fn from_ron(text: &str) -> Result<Profile, ProfileError> {
        #[derive(Deserialize)]
        struct Head {
            #[serde(default)]
            version: u32,
        }
        let head: Head = ron::from_str(text).map_err(|e| ProfileError::Parse(e.to_string()))?;
        if head.version > PROFILE_VERSION {
            return Err(ProfileError::Newer {
                found: head.version,
                known: PROFILE_VERSION,
            });
        }
        let mut p: Profile =
            ron::from_str(text).map_err(|e| ProfileError::Parse(e.to_string()))?;
        // Version 0 (no version written) has this shape: nothing to move.
        p.version = PROFILE_VERSION;
        Ok(p)
    }

    /// The file's text.
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .expect("a profile is always writable")
    }

    /// The class picked last, when the content has it.
    pub fn class_index(&self, content: &Content) -> Option<usize> {
        content.class(&self.class)
    }

    /// What a hero of `class` brings into a raid now: the raid under way's, else the loadout,
    /// else the class's free kit when the loadout is empty (spec §8.3); and the body from their
    /// last raid on this map.
    pub fn raid_loadout(&self, class: usize, content: &Content) -> Loadout {
        let inventory = match &self.in_raid {
            Some(s) => inventory_of(s, content).0,
            None if self.loadout.is_empty() => free_kit(class, content),
            None => inventory_of(&self.loadout, content).0,
        };
        let recovery = self
            .recovery
            .as_ref()
            .filter(|r| r.map == content.map.key)
            .map(|r| Recovery {
                map: r.map.clone(),
                at: V3::new(r.at.0, r.at.1, r.at.2),
                items: r
                    .items
                    .iter()
                    .filter_map(|s| content.item(&s.item).map(|i| Stack::new(i, s.count)))
                    .collect(),
            });
        Loadout {
            inventory,
            recovery,
        }
    }

    /// A raid begins: the loadout (or the free kit) leaves for it, so a raid that never ends is
    /// never a copy. What the content no longer knows, or has no room for, goes back to the stash.
    pub fn begin_raid(&mut self, class: usize, content: &Content) {
        if self.in_raid.is_some() {
            return;
        }
        let inv = if self.loadout.is_empty() {
            free_kit(class, content)
        } else {
            let (inv, aside) = inventory_of(&self.loadout, content);
            for s in aside {
                self.stash_add(s, content);
            }
            inv
        };
        self.in_raid = Some(saved_inventory(&inv, content));
        self.loadout = SavedInventory::default();
        self.raids += 1;
    }

    /// The raid ended for this player: what came out is the loadout again, what they recovered
    /// goes to the stash, and a body left behind waits for the next raid on the map. A body from
    /// before on this map had its one chance and is gone.
    pub fn settle(&mut self, o: &HeroOutcome, content: &Content) {
        self.in_raid = None;
        self.loadout = saved_inventory(&o.carried, content);
        for s in &o.recovered {
            if let Some(st) = saved(s, content) {
                self.stash_add(st, content);
            }
        }
        let older = self
            .recovery
            .take()
            .filter(|r| r.map != content.map.key);
        self.recovery = match &o.recovery {
            Some(r) => Some(SavedRecovery {
                map: r.map.clone(),
                at: (r.at.x, r.at.y, r.at.z),
                items: r.items.iter().filter_map(|s| saved(s, content)).collect(),
            }),
            None => older,
        };
        if matches!(o.fate, Fate::Extracted { .. }) {
            self.extracted += 1;
        }
    }

    /// A raid that never told this player its end (the game closed, the host went): settled as a
    /// death with no body, so `death_loss` takes what it would and no recovery is left. Whether
    /// there was one to settle.
    pub fn settle_missing(&mut self, content: &Content) -> bool {
        let Some(brought) = self.in_raid.take() else {
            return false;
        };
        let (mut inv, aside) = inventory_of(&brought, content);
        for s in aside {
            self.stash_add(s, content);
        }
        let _ = inv.split_for_death(content.rules.run.death_loss, content);
        inv.clear_found();
        self.loadout = saved_inventory(&inv, content);
        if self.recovery.as_ref().is_some_and(|r| r.map == content.map.key) {
            self.recovery = None;
        }
        true
    }

    pub fn stash_capacity(&self, content: &Content) -> usize {
        usize::from(content.rules.run.stash_slots)
    }

    /// The stash stack at `index` into the loadout: worn when its slot is free, a bag in a free
    /// bag slot, else into the backpack.
    pub fn take_from_stash(&mut self, index: usize, content: &Content) -> Result<(), StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let s = self.stash.get(index).cloned().ok_or(StashError::NoSuchSlot)?;
        let item = content.item(&s.item).ok_or(StashError::Unknown)?;
        let (mut inv, aside) = inventory_of(&self.loadout, content);
        let def = &content.items[item];
        let stack = Stack::new(item, s.count.min(def.stack.max(1)));
        let rest = s.count - stack.count;
        let worn = def.slot.filter(|slot| inv.equip[slot.index()].is_none());
        let bag = if def.kind == ItemKind::Bag {
            inv.bags.iter().position(Option::is_none)
        } else {
            None
        };
        if let Some(slot) = worn {
            inv.equip[slot.index()] = Some(stack);
        } else if let Some(b) = bag {
            inv.bags[b] = Some(stack);
            let _ = inv.sanitize(content);
        } else if inv.add(stack, content).is_some() {
            return Err(StashError::LoadoutFull);
        }
        if rest > 0 {
            self.stash[index].count = rest;
        } else {
            self.stash.remove(index);
        }
        for a in aside {
            self.stash.push(a);
        }
        self.loadout = saved_inventory(&inv, content);
        Ok(())
    }

    /// The loadout's stack at `at` into the stash.
    pub fn put_in_stash(&mut self, at: Place, content: &Content) -> Result<(), StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let (mut inv, aside) = inventory_of(&self.loadout, content);
        let stack = match at {
            Place::Bag(i) => {
                let s = inv
                    .bags
                    .get_mut(usize::from(i))
                    .and_then(Option::take)
                    .ok_or(StashError::NoSuchSlot)?;
                if !inv.sanitize(content).is_empty() {
                    return Err(StashError::BagInUse);
                }
                s
            }
            _ => inv.take(at).ok_or(StashError::NoSuchSlot)?,
        };
        let s = saved(&stack, content).ok_or(StashError::Unknown)?;
        if !self.stash_has_room(&s, content) {
            return Err(StashError::StashFull);
        }
        self.stash_add(s, content);
        for a in aside {
            self.stash.push(a);
        }
        self.loadout = saved_inventory(&inv, content);
        Ok(())
    }

    /// The loadout's stack at `from` moved to `to` (worn, a bag slot, the backpack, a safe slot),
    /// by the same rules as in a raid.
    pub fn loadout_move(&mut self, from: Place, to: Place, content: &Content) -> Result<(), StashError> {
        if self.in_raid.is_some() {
            return Err(StashError::InRaid);
        }
        let (mut inv, aside) = inventory_of(&self.loadout, content);
        inv.move_to(from, to, content).map_err(|e| match e {
            InvError::BagInUse => StashError::BagInUse,
            InvError::Empty | InvError::NoSuchPlace => StashError::NoSuchSlot,
            InvError::WrongSlot => StashError::WrongSlot,
            InvError::Full => StashError::LoadoutFull,
        })?;
        for a in aside {
            self.stash.push(a);
        }
        self.loadout = saved_inventory(&inv, content);
        Ok(())
    }

    /// The stash stack at `index` sold: its value in gold.
    pub fn sell(&mut self, index: usize, content: &Content) -> Result<u32, StashError> {
        let s = self.stash.get(index).ok_or(StashError::NoSuchSlot)?;
        let item = content.item(&s.item).ok_or(StashError::Unknown)?;
        let value = content.items[item].value.saturating_mul(u32::from(s.count));
        if value == 0 {
            return Err(StashError::Worthless);
        }
        self.stash.remove(index);
        self.gold = self.gold.saturating_add(value);
        Ok(value)
    }

    /// Whether `s` fits the stash: onto stacks of it with room, then into free slots.
    fn stash_has_room(&self, s: &SavedStack, content: &Content) -> bool {
        let max = content
            .item(&s.item)
            .map_or(u16::MAX, |i| content.items[i].stack.max(1));
        let free: u32 = self
            .stash
            .iter()
            .filter(|o| o.item == s.item)
            .map(|o| u32::from(max.saturating_sub(o.count)))
            .sum();
        let need = u32::from(s.count).saturating_sub(free);
        let slots = need.div_ceil(u32::from(max)) as usize;
        self.stash.len() + slots <= self.stash_capacity(content)
    }

    /// `s` into the stash, onto stacks of it first. Never refused: what a raid brings back is
    /// kept even past the stash's room (the panel then refuses more moves in).
    pub fn stash_add(&mut self, s: SavedStack, content: &Content) {
        let Some(i) = content.item(&s.item) else {
            self.stash.push(s);
            return;
        };
        let max = content.items[i].stack.max(1);
        let mut count = s.count;
        for o in self.stash.iter_mut().filter(|o| o.item == s.item) {
            let n = max.saturating_sub(o.count).min(count);
            o.count += n;
            count -= n;
        }
        while count > 0 {
            let n = count.min(max);
            self.stash.push(SavedStack {
                item: s.item.clone(),
                count: n,
            });
            count -= n;
        }
    }
}
```

- [ ] **Step 3: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim profile`
Expected: PASS (10 tests).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/profile.rs crates/nightfall-sim/src/lib.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the profile: stash, gold, loadout and free kit; raids begun and settled, never copied

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: On the wire: loadouts in, each player's pack and loot, outcomes out; acts in the input

**Files:**
- Modify: `crates/nightfall-sim/src/messages.rs`, `crates/nightfall-sim/src/inventory.rs` (`Default`), `crates/nightfall-sim/src/raid/mod.rs` (`Loadout: Default`)
- Modify: `crates/benilla-app/src/nightfall/session.rs` (`own_pick`, `lobby`, `send_outcome`, `broadcast_run_over`, `Heard`, `client_exchange`, `client_left_run`, the tests' `pick`), `crates/benilla-app/src/nightfall/run.rs` (`Sim`, `Sim::hosted`, `LocalInput`, `start_run`, `step_sim`, `advance_acts`, `deliver`, `raid_seed`), `crates/benilla-app/src/nightfall/mod.rs` (the run's chain)

**Interfaces:**
- Consumes: `Loadout`, `HeroOutcome`, `Act`, `ActPress`, `Carry::acted`, `RaidRoll::standard`, `World::{begin_raid, give_loadout, take_outcomes, outcomes, is_over}`, `free_kit` (Tasks 3, 6, 7); `HeroPick`, `LobbyPlayer`, `ClientMsg`, `ServerMsg::{State, Ended}`, `PROTOCOL = 4` (earlier plans); client `session::{Net, Role, HostSide, Peer, Heard, host_send, client_exchange, client_left_run, lobby, own_pick}`, `run::{Sim, LocalInput, start_run, step_sim, feed_pose}` (earlier plans).
- Produces:
  - `HeroPick { name, class, race, sex, loadout: Loadout }` (and `LobbyPlayer`) derive `Clone, Debug, PartialEq, Serialize, Deserialize`, no longer `Eq`.
  - `ServerMsg::Outcome(HeroOutcome)` (to its player) and `ServerMsg::RunOver { outcomes: Vec<HeroOutcome> }` (to everyone); `PROTOCOL = 5`.
  - `Inventory: Default` (no slots at all), `Loadout: Default`.
  - Client: `session::own_pick(&Selection, Loadout) -> HeroPick`; `session::{send_outcome(&mut Net, &HeroOutcome), broadcast_run_over(&mut Net, &[HeroOutcome])}`; `Heard::{Outcome(HeroOutcome), RunOver(Vec<HeroOutcome>)}`.
  - Client: `run::Sim::{mine: Option<HeroOutcome>, over: Option<Vec<HeroOutcome>>}`; `Sim::hosted(&[LobbyPlayer], seed: u64)`; `run::LocalInput::{act: Option<ActPress>, acts: VecDeque<Act>, queue(&mut self, Act), advance(&mut self, acted: Option<u8>), new_run(&mut self)}`; systems `run::advance_acts`; `run::{deliver(&mut Sim, &mut Net), raid_seed(&Time<Real>) -> u64}`; `NIGHTFALL_SEED` fixes the raid's seed.

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/messages.rs`'s tests:

- `messages_survive_the_wire`'s `HeroPick { … }` gains `loadout: Loadout::default(),` (add `use crate::Loadout;` to the test module).
- Add:

```rust
    #[test]
    fn a_loadout_and_an_outcome_survive_the_wire() {
        use crate::{Fate, HeroOutcome, Inventory, Loadout, Recovery, Stack, V3};
        let mut inventory = Inventory::default();
        inventory.pack = vec![Some(Stack::found(3, 7)), None];
        let pick = ClientMsg::Pick(HeroPick {
            name: "Valeera".into(),
            class: "rogue".into(),
            race: 1,
            sex: 0,
            loadout: Loadout {
                inventory: inventory.clone(),
                recovery: Some(Recovery {
                    map: "duskwood".into(),
                    at: V3::new(1.0, 2.0, 3.0),
                    items: vec![Stack::new(2, 1)],
                }),
            },
        });
        let bytes = postcard::to_stdvec(&pick).unwrap();
        assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).unwrap(), pick);
        let out = ServerMsg::RunOver {
            outcomes: vec![HeroOutcome {
                player: 1,
                name: "Jaina".into(),
                fate: Fate::Extracted {
                    exit: "The bridge".into(),
                },
                time: 1200.0,
                kills: 4,
                carried: inventory,
                recovered: Vec::new(),
                recovery: None,
            }],
        };
        let bytes = postcard::to_stdvec(&out).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), out);
    }
```

- In `a_full_partys_state_is_small`, right after `let mut snapshot = w.snapshot();`, add:

```rust
        // Our own full backpack, twenty containers around us and every exit; four heroes in
        // full gear.
        let items = w.content().items.len();
        let mut pack = crate::Inventory::new(w.content());
        for (i, s) in pack.pack.iter_mut().enumerate() {
            *s = Some(crate::Stack::new(i % items, 3));
        }
        snapshot.pack = Some((0, pack));
        snapshot.loot.marks = (1..=20)
            .map(|id| crate::ContainerMark {
                id,
                holder: crate::Holder::Placed(0),
                at: V2::new(-10051.8, -792.5),
                z: None,
                searched: false,
                empty: false,
                locked: false,
            })
            .collect();
        snapshot.loot.exits = (0..4)
            .map(|exit| crate::ExitMark {
                exit,
                open: true,
                opens_in: Some(0.0),
            })
            .collect();
        for h in &mut snapshot.heroes {
            h.carry.worn = crate::EquipSlot::ALL.iter().map(|&s| (s, 3)).collect();
            h.carry.load = 30.0;
        }
```

and its bound and comment become:

```rust
        // Four heroes with their senses, fight and gear, the twenty enemies near one of them, all
        // marked, a busy step's events, our backpack and the loot around us, 15 times a second,
        // stay near 48 KB a second.
        assert!(bytes < 3200, "{bytes} bytes");
```

Run: `cargo test -p nightfall-sim messages`
Expected: FAIL to compile (`struct HeroPick has no field named loadout`, `no variant RunOver`).

- [ ] **Step 2: The messages**

In `crates/nightfall-sim/src/inventory.rs`, `Inventory`'s derive gains `Default` (an inventory with no slots at all, for messages and tests; [`Inventory::new`] is the one with the rules' slots). In `crates/nightfall-sim/src/raid/mod.rs`, `Loadout`'s derive gains `Default`.

In `crates/nightfall-sim/src/messages.rs`:

- `use crate::{PlayerId, PlayerInput, Snapshot};` becomes `use crate::{HeroOutcome, Loadout, PlayerId, PlayerInput, Snapshot};`.
- `PROTOCOL` becomes `5` (its doc unchanged).
- `HeroPick` and `LobbyPlayer` derive `Clone, Debug, PartialEq, Serialize, Deserialize` (a loadout holds a position, so not `Eq`); `HeroPick` gains:

```rust
    /// What they bring into the raid (spec §8.3: the host trusts it).
    pub loadout: Loadout,
```

- `ServerMsg` gains, before `Ended`:

```rust
    /// Your raid is over: what you keep, sent once, as soon as it is settled.
    Outcome(HeroOutcome),
    /// Everyone is out or dead: every hero's outcome, for the end screen.
    RunOver { outcomes: Vec<HeroOutcome> },
```

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 3: The client's session**

In `crates/benilla-app/src/nightfall/session.rs`:

- Add `HeroOutcome` and `Loadout` to its `nightfall_sim` imports.
- `own_pick` becomes:

```rust
/// The local pick as the wire carries it, bringing `loadout`.
pub(super) fn own_pick(selection: &Selection, loadout: Loadout) -> HeroPick {
    let content = content();
    let class = &content.classes[selection.class];
    HeroPick {
        name: hero_name(),
        class: class.key.clone(),
        race: class.race,
        sex: class.sex,
        loadout,
    }
}
```

and its callers (`lobby`, and `run::start_run`) pass, until the stash comes in Task 9, the class's free kit: `Loadout { inventory: nightfall_sim::free_kit(selection.class, &content()), recovery: None }`.
- In `lobby`, the client's match arm `ClientEvent::Message(ServerMsg::State { .. } | ServerMsg::Ended) => {}` becomes `ClientEvent::Message(ServerMsg::State { .. } | ServerMsg::Ended | ServerMsg::Outcome(_) | ServerMsg::RunOver { .. }) => {}`.
- Add after `host_send`:

```rust
/// The host: `outcome` to its player, once.
pub(super) fn send_outcome(net: &mut Net, outcome: &HeroOutcome) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    if let Some(p) = h.peers.iter().find(|p| p.player == outcome.player) {
        h.host
            .lock()
            .unwrap()
            .send(p.conn, &ServerMsg::Outcome(outcome.clone()));
    }
}

/// The host: the raid is over; everyone's outcome to everyone, for the end screen.
pub(super) fn broadcast_run_over(net: &mut Net, outcomes: &[HeroOutcome]) {
    if let Role::Host(h) = &mut net.role {
        h.host.lock().unwrap().broadcast(&ServerMsg::RunOver {
            outcomes: outcomes.to_vec(),
        });
    }
}
```

- `Heard` gains:

```rust
    /// Our raid's outcome.
    Outcome(HeroOutcome),
    /// Every hero's, at the raid's end.
    RunOver(Vec<HeroOutcome>),
```

- `client_exchange`'s match gains `ClientEvent::Message(ServerMsg::Outcome(o)) => heard.push(Heard::Outcome(o)),` and `ClientEvent::Message(ServerMsg::RunOver { outcomes }) => heard.push(Heard::RunOver(outcomes)),` before its catch-all arm.
- `client_left_run`'s match arm `Heard::State(..) => {}` becomes `Heard::State(..) | Heard::Outcome(_) | Heard::RunOver(_) => {}`.
- The tests' `pick` gains `loadout: Loadout::default(),`.

- [ ] **Step 4: The client's run**

In `crates/benilla-app/src/nightfall/run.rs`:

- Add to its `nightfall_sim` imports: `Act, ActPress, HeroOutcome, Loadout, RaidRoll`; add `use std::collections::VecDeque;` if it is not there (it is, for the inbox).
- `Sim` gains:

```rust
    /// Our outcome, once the host settled it, for the stash to take ([`super::stash`]).
    pub mine: Option<HeroOutcome>,
    /// Every hero's outcome, once the raid is over: the end screen shows while it is here.
    pub over: Option<Vec<HeroOutcome>>,
```

and `Sim::mirror`'s literal gains `mine: None, over: None,`.
- `Sim::hosted` takes `(roster: &[LobbyPlayer], seed: u64)`, and its start becomes:

```rust
        let content = content();
        let mut world = World::new(content.clone());
        // The raid first, so the heroes start at its entry.
        let roll = RaidRoll::standard(&content, seed);
        let _ = world.begin_raid(roll);
        for p in roster {
            // A class the content lacks (a mismatched client) is left out rather than crash.
            if world
                .add_hero(p.player, &p.hero.class, &p.hero.name, p.hero.race, p.hero.sex)
                .is_ok()
            {
                let left = world.give_loadout(p.player, &p.hero.loadout);
                if !left.is_empty() {
                    warn!(
                        "nightfall: {} brought {} stacks with no room for them; left out",
                        p.hero.name,
                        left.len()
                    );
                }
            }
        }
```

in place of its `let mut world = World::new(content());` and roster loop; its `Self { … }` literal gains `mine: None, over: None,`.
- Add:

```rust
/// Acts waiting their turn at most: a click storm never grows without end.
const MAX_QUEUED: usize = 32;

/// The raid's seed: `NIGHTFALL_SEED` when set (a test run rolls the same loot every time), else
/// the clock.
pub(super) fn raid_seed(time: &Time<Real>) -> u64 {
    std::env::var("NIGHTFALL_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| time.elapsed().as_nanos() as u64 ^ 0x5EED_0F01_0000_0001)
}
```
- `LocalInput` gains:

```rust
    /// The act the host has not taken yet, repeated in every input ([`PlayerInput::act`]).
    pub act: Option<ActPress>,
    /// Acts waiting their turn, oldest first.
    pub acts: VecDeque<Act>,
    /// The last act's number.
    act_seq: u8,
```

and `with_pose`'s literal gets `act: self.act,` (in place of the `act: None,` of Task 3). Add to `impl LocalInput`:

```rust
    /// `act`, after the ones already waiting.
    pub fn queue(&mut self, act: Act) {
        if self.acts.len() < MAX_QUEUED {
            self.acts.push_back(act);
        }
    }

    /// The host has taken our act when our hero's `acted` (`Carry::acted`) says its number: the
    /// next one goes. One at a time, so an input the host replaced loses none and none is taken
    /// twice.
    pub fn advance(&mut self, acted: Option<u8>) {
        if let Some(p) = self.act {
            if acted != Some(p.seq) {
                return;
            }
            self.act = None;
        }
        if let Some(act) = self.acts.pop_front() {
            // Numbers go 1 to 255 and round: 0 is a hero who has taken nothing yet.
            self.act_seq = if self.act_seq == u8::MAX { 1 } else { self.act_seq + 1 };
            self.act = Some(ActPress {
                seq: self.act_seq,
                act,
            });
        }
    }

    /// A new run: nothing in flight, numbering from the start, as a new world has taken nothing.
    pub fn new_run(&mut self) {
        self.act = None;
        self.acts.clear();
        self.act_seq = 0;
    }
```

- Add:

```rust
/// Before the pose is read: the act in flight is done once our hero says so, and the next goes.
pub(super) fn advance_acts(sim: Res<Sim>, mut input: ResMut<LocalInput>) {
    let acted = sim.world.hero(sim.me).map(|h| h.carry.acted);
    input.advance(acted);
}

/// Alone or hosting: the outcomes the world settled, ours kept for the stash, each friend's sent
/// to them; and once the raid is over, everyone's to everyone, for the end screen.
pub(super) fn deliver(sim: &mut Sim, net: &mut Net) {
    for o in sim.world.take_outcomes() {
        if o.player == sim.me {
            sim.mine = Some(o);
        } else {
            session::send_outcome(net, &o);
        }
    }
    if sim.world.is_over() && sim.over.is_none() {
        let all = sim.world.outcomes().to_vec();
        session::broadcast_run_over(net, &all);
        sim.over = Some(all);
    }
}
```

- `start_run` gains the parameter `mut input: ResMut<LocalInput>`; before each of its `next.set(Phase::Playing);` lines add `input.new_run();`; `Sim::hosted(&roster)` becomes `Sim::hosted(&roster, raid_seed(&time))`.
- In `step_sim`, the client loop's `if let Heard::State(snapshot, seen, marks, events) = h { … }` becomes:

```rust
            match h {
                Heard::State(snapshot, seen, marks, events) => {
                    sim.inbox.push_back((snapshot, seen, marks, events));
                }
                Heard::Outcome(o) => sim.mine = Some(o),
                Heard::RunOver(all) => sim.over = Some(all),
                Heard::Ended | Heard::Lost(_) => {}
            }
```

- In `step_sim`'s host half, everything from `let Some(pose) = input.pose else {` to the end of its `while sim.acc >= DT { … }` loop becomes:

```rust
    // Until the controller has our hero the world waits (the terrain is still streaming); once
    // our hero is out of the raid (extracted), it goes on for the others without us.
    let ours = sim.world.hero(sim.me).is_some();
    if ours && input.pose.is_none() {
        return;
    }
    if loading.covering() {
        return;
    }
    // A long hitch would otherwise replay seconds of play in one frame.
    sim.acc = (sim.acc + time.delta_secs()).min(0.25);
    while sim.acc >= DT {
        sim.acc -= DT;
        if ours {
            sim.world.set_input(sim.me, input.with_pose(input.pose));
        }
        let from = sim.world.events().len();
        sim.world.step();
        session::host_send(&mut net, &sim.world, from);
        sim.stepped = true;
    }
    deliver(sim, &mut net);
```

- Add to the tests:

```rust
    #[test]
    fn acts_go_one_at_a_time_and_only_once_taken() {
        let mut input = LocalInput::default();
        input.queue(Act::TakeAll);
        input.queue(Act::Close);
        input.advance(Some(0));
        assert_eq!(input.act, Some(ActPress { seq: 1, act: Act::TakeAll }));
        // Not taken yet: the same act, the same number.
        input.advance(Some(0));
        assert_eq!(input.act, Some(ActPress { seq: 1, act: Act::TakeAll }));
        input.advance(Some(1));
        assert_eq!(input.act, Some(ActPress { seq: 2, act: Act::Close }));
        input.advance(Some(2));
        assert_eq!(input.act, None);
        input.new_run();
        input.queue(Act::Close);
        input.advance(None);
        assert_eq!(input.act.unwrap().seq, 1, "a new run numbers from the start");
        input.act_seq = u8::MAX;
        input.act = None;
        input.queue(Act::TakeAll);
        input.advance(None);
        assert_eq!(input.act.unwrap().seq, 1, "the numbers round past 255 without 0");
    }
```

In `crates/benilla-app/src/nightfall/mod.rs`, the run's chain `(run::feed_pose, run::step_sim, …)` gains `run::advance_acts` before `run::feed_pose`.

- [ ] **Step 5: Run the tests and build**

Run: `cargo test -p nightfall-sim`
Expected: PASS.
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (the new `acts_go_one_at_a_time_and_only_once_taken` with every earlier one).
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: loadouts on the wire, each player's pack and loot in their state, outcomes sent; protocol 5

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The stash on disk: versioned, backed up, never lost silently; raids begun and settled

**Files:**
- Create: `crates/benilla-app/src/nightfall/stash.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (`mod stash;`, the resource, systems), `crates/benilla-app/src/nightfall/menu.rs` (`Selection::new`, `menu_clicks`, the status line), `crates/benilla-app/src/nightfall/session.rs` (`lobby`), `crates/benilla-app/src/nightfall/run.rs` (`start_run`)

**Interfaces:**
- Consumes: `Profile::{from_ron, to_ron, raid_loadout, begin_raid, settle, settle_missing, stash_add, class_index}`, `ProfileError`, `SavedStack` (Task 7); `run::Sim::mine` (Task 8); `session::own_pick(&Selection, Loadout)` (Task 8); `crate::local_state::{home, read_to_string, write_atomic}`; `menu::{Selection, MenuAction}`, `Phase` (earlier plans).
- Produces:
  - `stash::{load(&Path) -> Loaded, save(&Path, &Profile) -> std::io::Result<()>}`; `stash::Loaded { profile: Profile, note: Option<String>, writable: bool }`.
  - `stash::Stash` (a `Resource`) with `pub profile: Profile`, `pub note: Option<String>`, `open() -> Stash`, `save(&mut self)`; systems `stash::{settle_run, after_raid}`.
  - `menu::Selection::new(&Content, &Stash)`: `NIGHTFALL_CLASS`, else the class picked last, else the first.
  - `NIGHTFALL_GIVE=<item>:<count>,…` adds things to the stash at startup (a test hook; the test home is `BENILLA_HOME=target/nf-home`).

- [ ] **Step 1: Write the file's handling with its tests**

`crates/benilla-app/src/nightfall/stash.rs`:

```rust
//! The player's things between raids, on disk: `benilla-config/nightfall/stash.ron`, a
//! [`Profile`] in RON (spec §8.3: each player's stash on their own machine). Never lost silently:
//! every save keeps the last good file as `stash.ron.bak`; a file that does not read is kept as
//! `stash.ron.broken` (numbered when there is one already) and the backup tried; a file from a
//! newer Nightfall is left untouched, and this session does not save over it. Whatever happened
//! is said on the menu ([`Stash::note`]).

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use nightfall_sim::{Profile, ProfileError, SavedStack};

use super::content;
use super::run::Sim;

/// The stash as read, and what to say about it.
pub(super) struct Loaded {
    pub profile: Profile,
    pub note: Option<String>,
    /// Whether this session may save over the file.
    pub writable: bool,
}

fn backup_of(path: &Path) -> PathBuf {
    path.with_extension("ron.bak")
}

/// Where a broken file is set aside: `stash.ron.broken`, or the first `stash.ron.broken-N` free.
fn broken_of(path: &Path) -> PathBuf {
    let free = |p: &Path| {
        matches!(crate::local_state::read_to_string(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    };
    let first = path.with_extension("ron.broken");
    if free(&first) {
        return first;
    }
    (2..1000)
        .map(|n| path.with_extension(format!("ron.broken-{n}")))
        .find(|p| free(p))
        .unwrap_or(first)
}

/// The backup's profile, if it reads.
fn from_backup(path: &Path) -> Option<Profile> {
    let text = crate::local_state::read_to_string(&backup_of(path)).ok()?;
    Profile::from_ron(&text).ok()
}

/// The stash at `path`.
pub(super) fn load(path: &Path) -> Loaded {
    let fresh = |note: Option<String>, writable: bool| Loaded {
        profile: Profile::default(),
        note,
        writable,
    };
    let text = match crate::local_state::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return match from_backup(path) {
                Some(profile) => Loaded {
                    profile,
                    note: Some("Your stash file was missing; it came back from its backup.".into()),
                    writable: true,
                },
                None => fresh(None, true),
            };
        }
        Err(e) => {
            return fresh(
                Some(format!(
                    "Your stash could not be read ({e}); it is left as it is, and this session does not save."
                )),
                false,
            );
        }
    };
    match Profile::from_ron(&text) {
        Ok(profile) => Loaded {
            profile,
            note: None,
            writable: true,
        },
        Err(ProfileError::Newer { .. }) => fresh(
            Some(
                "Your stash is from a newer Nightfall: it is left untouched, and this session's raids are not saved."
                    .into(),
            ),
            false,
        ),
        Err(ProfileError::Parse(why)) => {
            let aside = broken_of(path);
            if crate::local_state::write_atomic(&aside, &text).is_err() {
                return fresh(
                    Some(format!(
                        "Your stash file is damaged ({why}) and could not be set aside: it is left as it is, and this session does not save."
                    )),
                    false,
                );
            }
            let kept = aside
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match from_backup(path) {
                Some(profile) => Loaded {
                    profile,
                    note: Some(format!(
                        "Your stash file was damaged ({why}): it was kept as {kept}, and your stash came back from its backup."
                    )),
                    writable: true,
                },
                None => fresh(
                    Some(format!(
                        "Your stash file was damaged ({why}) and had no good backup: it was kept as {kept}, and a new stash begins."
                    )),
                    true,
                ),
            }
        }
    }
}

/// `profile` written to `path`, the file it replaces kept as the backup when it was a good one
/// (a damaged file never overwrites a good backup).
pub(super) fn save(path: &Path, profile: &Profile) -> std::io::Result<()> {
    if let Ok(old) = crate::local_state::read_to_string(path) {
        if Profile::from_ron(&old).is_ok() {
            crate::local_state::write_atomic(&backup_of(path), &old)?;
        }
    }
    crate::local_state::write_atomic(path, &profile.to_ron())
}

/// The player's stash, as this session has it.
#[derive(Resource)]
pub(super) struct Stash {
    pub profile: Profile,
    path: Option<PathBuf>,
    writable: bool,
    /// What the menu says about the stash: a file that did not read, a raid counted missing, a
    /// move refused.
    pub note: Option<String>,
}

impl Stash {
    /// The stash from `benilla-config/nightfall/stash.ron`; a raid that never ended settled as
    /// missing; and `NIGHTFALL_GIVE`'s things added (`item:count,…`, a test hook).
    pub fn open() -> Self {
        let path = crate::local_state::home().map(|h| h.join("nightfall").join("stash.ron"));
        let loaded = match &path {
            Some(p) => load(p),
            None => Loaded {
                profile: Profile::default(),
                note: None,
                writable: false,
            },
        };
        let mut stash = Stash {
            profile: loaded.profile,
            path,
            writable: loaded.writable,
            note: loaded.note,
        };
        let content = content();
        if stash.profile.settle_missing(&content) {
            stash.note = Some("Your last raid ended without you: you were counted missing.".into());
            stash.save();
        }
        if let Ok(give) = std::env::var("NIGHTFALL_GIVE") {
            for part in give.split(',') {
                let Some((item, n)) = part.split_once(':') else {
                    continue;
                };
                let (item, n) = (item.trim(), n.trim().parse::<u16>());
                if let (Some(_), Ok(count)) = (content.item(item), n) {
                    stash.profile.stash_add(
                        SavedStack {
                            item: item.into(),
                            count,
                        },
                        &content,
                    );
                }
            }
            stash.save();
        }
        stash
    }

    /// Written to disk, unless this session may not (a newer or unreadable file).
    pub fn save(&mut self) {
        let (Some(path), true) = (&self.path, self.writable) else {
            return;
        };
        if let Err(e) = save(path, &self.profile) {
            warn!("nightfall: stash not saved: {e}");
            self.note = Some(format!("Your stash could not be saved: {e}"));
        }
    }
}

/// Our outcome, once it came, into the stash and onto the disk at once: a crash after it loses
/// nothing.
pub(super) fn settle_run(mut sim: ResMut<Sim>, mut stash: ResMut<Stash>) {
    let Some(o) = sim.mine.take() else {
        return;
    };
    stash.profile.settle(&o, &content());
    stash.save();
}

/// Back from a raid without its outcome (we left it, the host went): counted missing.
pub(super) fn after_raid(mut stash: ResMut<Stash>) {
    if stash.profile.settle_missing(&content()) {
        stash.note = Some("You left the raid before its end: you were counted missing.".into());
        stash.save();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// An empty folder of its own for each test.
    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nightfall-stash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn with_gold(gold: u32) -> Profile {
        Profile {
            gold,
            ..Profile::default()
        }
    }

    #[test]
    fn a_missing_file_is_a_fresh_stash() {
        let path = dir("missing").join("stash.ron");
        let l = load(&path);
        assert_eq!(l.profile, Profile::default());
        assert!(l.note.is_none() && l.writable);
    }

    #[test]
    fn saving_keeps_the_last_good_file_as_the_backup() {
        let path = dir("backup").join("stash.ron");
        save(&path, &with_gold(1)).unwrap();
        save(&path, &with_gold(2)).unwrap();
        assert_eq!(load(&path).profile.gold, 2);
        let bak = std::fs::read_to_string(backup_of(&path)).unwrap();
        assert_eq!(Profile::from_ron(&bak).unwrap().gold, 1);
        // A damaged main file never replaces a good backup.
        std::fs::write(&path, "(gold: ").unwrap();
        save(&path, &with_gold(3)).unwrap();
        let bak = std::fs::read_to_string(backup_of(&path)).unwrap();
        assert_eq!(Profile::from_ron(&bak).unwrap().gold, 1);
    }

    #[test]
    fn a_corrupt_file_falls_back_to_the_backup_and_is_kept() {
        let path = dir("corrupt").join("stash.ron");
        save(&path, &with_gold(5)).unwrap();
        save(&path, &with_gold(9)).unwrap();
        // Cut short, as a crash mid-write on a file system without atomic renames would.
        let good = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, &good[..good.len() / 2]).unwrap();
        let l = load(&path);
        assert_eq!(l.profile.gold, 5);
        assert!(l.writable);
        assert!(l.note.unwrap().contains("stash.ron.broken"));
        let kept = std::fs::read_to_string(path.with_extension("ron.broken")).unwrap();
        assert_eq!(kept, &good[..good.len() / 2]);
        // A second break is kept beside the first.
        std::fs::write(&path, "nonsense").unwrap();
        let _ = load(&path);
        assert!(path.with_extension("ron.broken-2").exists());
    }

    #[test]
    fn a_corrupt_file_with_no_backup_is_kept_and_a_new_stash_begins() {
        let path = dir("lone").join("stash.ron");
        std::fs::write(&path, "(gold: \"lots\")").unwrap();
        let l = load(&path);
        assert_eq!(l.profile, Profile::default());
        assert!(l.writable);
        assert!(l.note.unwrap().contains("a new stash begins"));
        assert!(path.with_extension("ron.broken").exists());
    }

    #[test]
    fn a_newer_file_is_never_overwritten() {
        let path = dir("newer").join("stash.ron");
        std::fs::write(&path, "(version: 99, gold: 77)").unwrap();
        let l = load(&path);
        assert!(!l.writable);
        assert!(l.note.as_ref().unwrap().contains("newer"));
        let mut stash = Stash {
            profile: l.profile,
            path: Some(path.clone()),
            writable: l.writable,
            note: l.note,
        };
        stash.profile.gold = 1;
        stash.save();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "(version: 99, gold: 77)");
    }

    #[test]
    fn a_lost_file_comes_back_from_its_backup() {
        let path = dir("lost").join("stash.ron");
        save(&path, &with_gold(4)).unwrap();
        save(&path, &with_gold(6)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let l = load(&path);
        assert_eq!(l.profile.gold, 4);
        assert!(l.note.unwrap().contains("backup"));
    }
}
```

In `nightfall/mod.rs` add `mod stash;`.

Run: `cargo test -p benilla-app --lib nightfall::stash -j 2`
Expected: PASS (6 tests).

- [ ] **Step 2: The stash in the game**

In `crates/benilla-app/src/nightfall/mod.rs`, in `NightfallPlugin::build`, replace `.insert_resource(menu::Selection::new(&content()))` with:

```rust
            .insert_resource(menu::Selection::new(&content(), &stash))
            .insert_resource(stash)
```

with `let stash = stash::Stash::open();` as the first line of `build`; and add:

```rust
            .add_systems(OnEnter(Phase::Menu), stash::after_raid)
            .add_systems(
                Update,
                stash::settle_run
                    .after(run::step_sim)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

In `crates/benilla-app/src/nightfall/menu.rs`:

- `Selection::new` becomes:

```rust
    /// `NIGHTFALL_CLASS`, else the class picked last (the stash keeps it), else the first.
    pub(super) fn new(content: &Content, stash: &super::stash::Stash) -> Self {
        let class = super::class_key()
            .and_then(|k| content.class(&k))
            .or_else(|| stash.profile.class_index(content))
            .unwrap_or(0);
        Self { class }
    }
```

- `menu_clicks` gains the parameter `mut stash: ResMut<super::stash::Stash>`, and its `MenuAction::Class(i) => selection.class = i,` arm becomes:

```rust
        MenuAction::Class(i) => {
            selection.class = i;
            stash.profile.class = content().classes[i].key.clone();
            stash.save();
        }
```

- `refresh_menu` gains `stash: Res<super::stash::Stash>`, and its status body becomes the lobby's text with the stash's note under it: `let body = match &stash.note { Some(n) => format!("{}\n{n}", net.lobby_text()), None => net.lobby_text() };`.

In `crates/benilla-app/src/nightfall/session.rs`, `lobby` gains `stash: Res<super::stash::Stash>`, and its `let own = own_pick(&selection, …);` becomes:

```rust
    let own = own_pick(
        &selection,
        stash.profile.raid_loadout(selection.class, &content()),
    );
```

In `crates/benilla-app/src/nightfall/run.rs`, `start_run` gains `mut stash: ResMut<super::stash::Stash>`. Its `let own = session::own_pick(&selection, …);` becomes the same `raid_loadout` call; and before each `input.new_run();` (the client's and the host's) add:

```rust
    // The loadout leaves the stash for the raid, on disk at once: a raid that never ends is
    // counted missing at the next start, never kept twice.
    stash.profile.begin_raid(selection.class, &content());
    stash.save();
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: A raid from the stash and back**

```bash
rm -rf target/nf-home/nightfall/stash.ron*
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_GIVE=ruby_shard:3,hooded_lantern:1 WOW_LIVE_SHOT=target/stash-run.png WOW_LIVE_SHOT_AT=30 ./target/release/azeroth-together.exe > target/stash-run.log 2>&1 &
```

Expected, once `target/stash-run.png` exists (then `kill` the PID): `target/nf-home/nightfall/stash.ron` holds `ruby_shard` and `hooded_lantern` in its `stash`, an empty `loadout`, `raids: 1` and an `in_raid` with the rogue's free kit (the run started from an empty loadout). Start it again without `NIGHTFALL_GIVE` and kill it before 12 s (before the autoplay starts a run): the stash file now has no `in_raid`, the kit gone (counted missing under `everything_one_recovery`), and `stash.ron.bak` is the file before.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the stash on disk, versioned and backed up; raids begin from it and settle into it

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The menu's stash panel: stash, loadout, gold, selling

**Files:**
- Create: `crates/benilla-app/src/nightfall/widgets.rs`, `crates/benilla-app/src/nightfall/stash_panel.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (modules, resources, systems), `crates/benilla-app/src/nightfall/menu.rs` (`MenuAction::Stash`, its button, `menu_clicks`, `refresh_menu`)

**Interfaces:**
- Consumes: `Stash::{profile, note, save}` (Task 9); `Profile::{take_from_stash, put_in_stash, sell, stash_capacity, loadout, stash, gold}`, `inventory_of`, `StashError::message`, `ItemDef`, `EquipSlot`, `Place`, `MAX_BAGS` (Tasks 1, 2, 7); `menu::{Selection, MenuAction, menu_clicks, refresh_menu, build_menu}`, `Phase`, `GameMode`, `crate::char_select::wow_font`, `benilla_assets::WorldAssets::sprite_texture` (earlier plans).
- Produces:
  - `widgets::{SLOT, GOLD, GREY, RED, PANEL, EDGE, ItemIcon, IconDone, fill_icons, quality_color(u8) -> Color, text(&Handle<Font>, impl Into<String>, f32, Color) -> impl Bundle, slot<T: Component>(&mut ChildSpawnerCommands, &Handle<Font>, T, Option<(&ItemDef, u16)>), text_button<T: Component>(&mut ChildSpawnerCommands, &Handle<Font>, T, &str), describe(&ItemDef, u16) -> String}`.
  - `stash_panel::{StashOpen, StashRoot, StashSlot, spawn_root, show, rebuild, clicks, hover}`; `NIGHTFALL_STASH=1` opens it at start (a test hook).
  - `MenuAction::Stash` toggles the panel.

- [ ] **Step 1: Write the widgets, with a test**

`crates/benilla-app/src/nightfall/widgets.rs`:

```rust
//! The small parts of Nightfall's own panels (spec §8.4: our own glue panels first): an item's
//! icon in a slot with its count, a text button, a label, WoW's quality colours, and a line that
//! says what an item is.

use bevy::prelude::*;
use nightfall_sim::ItemDef;

/// A slot's side, in the UI's units.
pub(super) const SLOT: f32 = 34.0;
pub(super) const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
pub(super) const GREY: Color = Color::srgb(0.7, 0.7, 0.7);
pub(super) const RED: Color = Color::srgb(1.0, 0.35, 0.3);
pub(super) const PANEL: Color = Color::srgba(0.03, 0.03, 0.05, 0.92);
pub(super) const EDGE: Color = Color::srgb(0.55, 0.47, 0.3);

/// An icon to load into this image: `Interface\Icons\<name>.blp`.
#[derive(Component)]
pub(super) struct ItemIcon(pub String);

/// An [`ItemIcon`] already loaded (or tried).
#[derive(Component)]
pub(super) struct IconDone;

/// Load every new [`ItemIcon`] once the asset reader is up.
pub(super) fn fill_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &ItemIcon), Without<IconDone>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(assets) = assets.as_deref_mut() else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let path = format!(r"Interface\Icons\{}.blp", icon.0);
        if let Some(h) = assets.sprite_texture(&path, &mut images) {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(IconDone);
    }
}

/// WoW's colour for an item of `quality`.
pub(super) fn quality_color(quality: u8) -> Color {
    match quality {
        0 => Color::srgb(0.62, 0.62, 0.62),
        1 => Color::WHITE,
        2 => Color::srgb(0.12, 1.0, 0.0),
        3 => Color::srgb(0.0, 0.44, 0.87),
        _ => Color::srgb(0.64, 0.21, 0.93),
    }
}

/// A line of text in WoW's font.
pub(super) fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow::default(),
    )
}

/// A slot carrying `tag`: an empty square, or `item`'s icon edged in its quality's colour, its
/// count in the corner above 1.
pub(super) fn slot<T: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    tag: T,
    item: Option<(&ItemDef, u16)>,
) {
    let edge = item.map_or(Color::srgb(0.3, 0.3, 0.3), |(d, _)| quality_color(d.quality));
    let mut cell = parent.spawn((
        Button,
        tag,
        Interaction::default(),
        Node {
            width: Val::Px(SLOT),
            height: Val::Px(SLOT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(edge),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
    ));
    if let Some((def, count)) = item {
        cell.with_children(|s| {
            s.spawn((
                ImageNode {
                    color: Color::srgba(0.2, 0.2, 0.2, 1.0),
                    ..default()
                },
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ItemIcon(def.icon.clone()),
            ));
            if count > 1 {
                s.spawn((
                    text(font, count.to_string(), 10.0, Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(2.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                ));
            }
        });
    }
}

/// A plain text button carrying `tag`.
pub(super) fn text_button<T: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    tag: T,
    caption: &str,
) {
    parent
        .spawn((
            Button,
            tag,
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(EDGE),
            BackgroundColor(Color::srgba(0.12, 0.08, 0.03, 0.9)),
        ))
        .with_children(|b| {
            b.spawn(text(font, caption, 12.0, GOLD));
        });
}

/// What `count` of `def` is, in one line: name, kind, weight, value and what it adds.
pub(super) fn describe(def: &ItemDef, count: u16) -> String {
    let mut parts = vec![if count > 1 {
        format!("{} ×{count}", def.name)
    } else {
        def.name.clone()
    }];
    parts.push(format!("{:?}", def.kind));
    parts.push(format!("weight {:.1}", def.weight * f32::from(count)));
    if def.value > 0 {
        parts.push(format!("{} gold", def.value * u32::from(count)));
    }
    if def.bonus.health > 0.0 {
        parts.push(format!("+{:.0} health", def.bonus.health));
    }
    if def.bonus.damage > 0.0 {
        parts.push(format!("+{:.0}% damage", def.bonus.damage * 100.0));
    }
    if def.bag_slots > 0 {
        parts.push(format!("{} slots", def.bag_slots));
    }
    if !def.text.is_empty() {
        parts.push(def.text.clone());
    }
    parts.join("  ·  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{Bonus, ItemKind};

    #[test]
    fn an_item_says_what_it_is() {
        let blade = ItemDef {
            key: "blade".into(),
            name: "Night Watch Blade".into(),
            kind: ItemKind::Weapon,
            icon: "INV_Sword_05".into(),
            quality: 2,
            weight: 3.0,
            stack: 1,
            value: 40,
            slot: Some(nightfall_sim::EquipSlot::MainHand),
            look: None,
            bonus: Bonus {
                health: 0.0,
                damage: 0.1,
            },
            bag_slots: 0,
            restore: 1.0,
            on_use: None,
            light: None,
            text: String::new(),
        };
        assert_eq!(
            describe(&blade, 1),
            "Night Watch Blade  ·  Weapon  ·  weight 3.0  ·  40 gold  ·  +10% damage"
        );
        assert!(describe(&blade, 2).starts_with("Night Watch Blade ×2  ·  Weapon  ·  weight 6.0"));
    }
}
```

In `nightfall/mod.rs` add `mod widgets;` and register `widgets::fill_icons.in_set(NightfallSet)` in `Update`.

Run: `cargo test -p benilla-app --lib nightfall::widgets -j 2`
Expected: PASS.

- [ ] **Step 2: Write the stash panel**

`crates/benilla-app/src/nightfall/stash_panel.rs`:

```rust
//! The menu's stash panel (spec §8.3): the stash on the left; on the right the loadout the next
//! raid brings (worn, bags, backpack, safe slots), its weight, and what the free kit would give
//! when it is empty. A click moves a stack between them; Shift and a click sells a stash stack.
//! Our own panel; in sub-project 5b the inn's banker opens it.

use bevy::prelude::*;
use nightfall_sim::{inventory_of, EquipSlot, Place, Stack};

use super::menu::Selection;
use super::stash::Stash;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL, RED, SLOT};
use super::{content, Phase};
use crate::game_pick::GameMode;

/// Whether the panel is open. `NIGHTFALL_STASH` opens it, for a test shot.
#[derive(Resource)]
pub(super) struct StashOpen(pub bool);

impl Default for StashOpen {
    fn default() -> Self {
        Self(std::env::var_os("NIGHTFALL_STASH").is_some())
    }
}

#[derive(Component)]
pub(super) struct StashRoot;

/// The part rebuilt whenever the stash changes.
#[derive(Component)]
struct StashBody;

/// The line under the panel: what the mouse is over, or what just happened.
#[derive(Component)]
struct StashInfo;

#[derive(Component)]
struct StashClose;

/// Which stack a slot shows.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StashSlot {
    Stash(usize),
    Loadout(Place),
}

#[derive(Resource)]
pub(super) struct StashFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            StashRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },
            GlobalZIndex(420),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                StashBody,
                Node {
                    column_gap: Val::Px(24.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ));
            root.spawn((StashInfo, text(&font, "", 12.0, GREY)));
            widgets::text_button(root, &font, StashClose, "Close");
        });
    commands.insert_resource(StashFont(font));
}

/// Shown at Nightfall's menu while open.
pub(super) fn show(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    open: Res<StashOpen>,
    mut roots: Query<&mut Visibility, With<StashRoot>>,
) {
    let on = *mode.get() == GameMode::Nightfall && *phase.get() == Phase::Menu && open.0;
    let want = if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// The stash and the loadout, built again whenever either (or the class) changes.
pub(super) fn rebuild(
    mut commands: Commands,
    stash: Res<Stash>,
    open: Res<StashOpen>,
    selection: Res<Selection>,
    font: Option<Res<StashFont>>,
    body: Query<Entity, With<StashBody>>,
) {
    if !(stash.is_changed() || open.is_changed() || selection.is_changed()) || !open.0 {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    let content = content();
    let p = &stash.profile;
    let (inv, _) = inventory_of(&p.loadout, &content);
    let item_of = |s: Option<Stack>| s.map(|s| (&content.items[usize::from(s.item)], s.count));
    let grid = |cols: usize| Node {
        flex_wrap: FlexWrap::Wrap,
        width: Val::Px(cols as f32 * (SLOT + 3.0)),
        column_gap: Val::Px(3.0),
        row_gap: Val::Px(3.0),
        ..default()
    };
    let column = || Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        ..default()
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        b.spawn(column()).with_children(|col| {
            col.spawn(text(
                font,
                format!("Stash  {}/{}", p.stash.len(), p.stash_capacity(&content)),
                14.0,
                GOLD,
            ));
            col.spawn(grid(10)).with_children(|g| {
                for (i, s) in p.stash.iter().enumerate() {
                    let def = content.item(&s.item).map(|k| (&content.items[k], s.count));
                    widgets::slot(g, font, StashSlot::Stash(i), def);
                }
            });
            col.spawn(text(font, format!("{} gold", p.gold), 13.0, GOLD));
            col.spawn(text(font, "Click: take  ·  Shift-click: sell", 11.0, GREY));
        });
        b.spawn(column()).with_children(|col| {
            let class = &content.classes[selection.class];
            col.spawn(text(font, format!("Next raid  ·  {}", class.name), 14.0, GOLD));
            col.spawn(text(font, "Worn", 11.0, GREY));
            col.spawn(grid(EquipSlot::COUNT)).with_children(|g| {
                for s in EquipSlot::ALL {
                    let at = Place::Equip(s);
                    widgets::slot(g, font, StashSlot::Loadout(at), item_of(inv.get(at).copied()));
                }
            });
            col.spawn(text(font, "Bags", 11.0, GREY));
            col.spawn(grid(nightfall_sim::MAX_BAGS)).with_children(|g| {
                for i in 0..nightfall_sim::MAX_BAGS {
                    let at = Place::Bag(i as u8);
                    widgets::slot(g, font, StashSlot::Loadout(at), item_of(inv.get(at).copied()));
                }
            });
            col.spawn(text(font, format!("Backpack  ({} slots)", inv.pack.len()), 11.0, GREY));
            col.spawn(grid(8)).with_children(|g| {
                for i in 0..inv.pack.len() {
                    let at = Place::Pack(i as u16);
                    widgets::slot(g, font, StashSlot::Loadout(at), item_of(inv.get(at).copied()));
                }
            });
            col.spawn(text(font, "Safe slots: always kept", 11.0, GREY));
            col.spawn(grid(inv.safe.len().max(1))).with_children(|g| {
                for i in 0..inv.safe.len() {
                    let at = Place::Safe(i as u8);
                    widgets::slot(g, font, StashSlot::Loadout(at), item_of(inv.get(at).copied()));
                }
            });
            let r = &content.rules.run;
            let weight = inv.weight(&content);
            col.spawn(text(
                font,
                format!("Weight {weight:.1}  (no slowing up to {:.0})", r.weight_free),
                11.0,
                if weight > r.weight_free { RED } else { GREY },
            ));
            if p.loadout.is_empty() {
                let kit: Vec<String> = class
                    .free_items
                    .iter()
                    .map(|&(i, n)| format!("{} ×{n}", content.items[i].name))
                    .collect();
                col.spawn(text(
                    font,
                    format!("Nothing chosen: you go in with the free kit ({}).", kit.join(", ")),
                    11.0,
                    GREY,
                ));
            }
        });
    });
}

/// A click moves a stack, Shift and a click sells it; Close closes. What went wrong (or the gold
/// a sale made) goes in the stash's note.
pub(super) fn clicks(
    slots: Query<(&Interaction, &StashSlot), Changed<Interaction>>,
    close: Query<&Interaction, (Changed<Interaction>, With<StashClose>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stash: ResMut<Stash>,
    mut open: ResMut<StashOpen>,
) {
    if close.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = false;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let content = content();
    for (i, slot) in &slots {
        if *i != Interaction::Pressed {
            continue;
        }
        let done = match *slot {
            StashSlot::Stash(n) if shift => stash
                .profile
                .sell(n, &content)
                .map(|g| format!("Sold for {g} gold.")),
            StashSlot::Stash(n) => stash.profile.take_from_stash(n, &content).map(|_| String::new()),
            StashSlot::Loadout(at) if shift => {
                // Between the backpack and the safe slots, as Shift does in a raid.
                let (inv, _) = inventory_of(&stash.profile.loadout, &content);
                let to = match at {
                    Place::Safe(_) => inv.first_empty(false),
                    _ => inv.first_empty(true),
                };
                match to {
                    Some(to) => stash
                        .profile
                        .loadout_move(at, to, &content)
                        .map(|_| String::new()),
                    None => Err(nightfall_sim::StashError::LoadoutFull),
                }
            }
            StashSlot::Loadout(at) => stash.profile.put_in_stash(at, &content).map(|_| String::new()),
        };
        match done {
            Ok(line) => {
                stash.note = (!line.is_empty()).then_some(line);
                stash.save();
            }
            Err(e) => stash.note = Some(e.message().into()),
        }
    }
}

/// The line under the panel: the stack under the mouse, else the stash's note.
pub(super) fn hover(
    slots: Query<(&Interaction, &StashSlot)>,
    stash: Res<Stash>,
    mut info: Query<&mut Text, With<StashInfo>>,
) {
    let content = content();
    let hovered = slots
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .map(|(_, s)| *s);
    let line = match hovered {
        Some(StashSlot::Stash(n)) => stash.profile.stash.get(n).map(|s| match content.item(&s.item) {
            Some(k) => format!("{}  (click: take · shift-click: sell)", widgets::describe(&content.items[k], s.count)),
            None => format!("{} ×{}: this game no longer knows it; it is kept", s.item, s.count),
        }),
        Some(StashSlot::Loadout(at)) => {
            let (inv, _) = inventory_of(&stash.profile.loadout, &content);
            inv.get(at).map(|s| {
                format!(
                    "{}  (click: back to the stash · shift-click: safe slot)",
                    widgets::describe(&content.items[usize::from(s.item)], s.count)
                )
            })
        }
        None => None,
    }
    .or_else(|| stash.note.clone())
    .unwrap_or_default();
    for mut t in &mut info {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
}
```

In `crates/benilla-app/src/nightfall/menu.rs`:
- `MenuAction` gains `Stash`.
- In `build_menu`'s button row, after `(MenuAction::PlaySolo, "Play Solo"),` add `(MenuAction::Stash, "Stash"),`.
- `menu_clicks` gains `mut stash_open: ResMut<super::stash_panel::StashOpen>` and the arm `MenuAction::Stash => stash_open.0 = !stash_open.0,`.
- In `refresh_menu`, `matches!(action, MenuAction::Class(_) | MenuAction::Back)` becomes `matches!(action, MenuAction::Class(_) | MenuAction::Back | MenuAction::Stash)`.

In `crates/benilla-app/src/nightfall/mod.rs` add `mod stash_panel;` and in `NightfallPlugin::build`:

```rust
            .init_resource::<stash_panel::StashOpen>()
            .add_systems(Startup, stash_panel::spawn_root)
            // Ungated: hides the panel whenever another game is up.
            .add_systems(Update, stash_panel::show)
            .add_systems(
                Update,
                (stash_panel::clicks, stash_panel::rebuild, stash_panel::hover)
                    .chain()
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Menu)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: See the panel**

```bash
rm -rf target/nf-home/nightfall/stash.ron*
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_STASH=1 NIGHTFALL_GIVE=ruby_shard:3,night_watch_blade:1,duskwood_satchel:1,tough_bread:6 WOW_LIVE_SHOT=target/stash-panel.png WOW_LIVE_SHOT_AT=25 ./target/release/azeroth-together.exe > target/stash-panel.log 2>&1 &
```

Expected, once `target/stash-panel.png` exists (then `kill` the PID): over the menu, the stash panel shows four stacks on the left (the shard ×3, the blade, the satchel and the bread ×6 with WoW's icons; the shards, the blade and the satchel edged green, uncommon), `0 gold`, and on the right the empty loadout (nine worn slots, four bag slots, sixteen backpack slots, two safe slots) with "Nothing chosen: you go in with the free kit (Worn Shortsword ×1, Worn Dagger ×1, Tough Hunk of Bread ×4, Refreshing Spring Water ×2)." The clicks need a hand: list "take the blade, sell the shards" in the hand-off for the user to try.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the menu's stash panel: stash, loadout, weight, the free kit, selling

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: In the raid: containers drawn and searched, the backpack and loot panels, gear worn, lanterns

**Files:**
- Create: `crates/benilla-app/src/nightfall/loot_view.rs`, `crates/benilla-app/src/nightfall/backpack.rs`, `crates/benilla-app/src/nightfall/gear.rs`
- Modify: `crates/benilla-app/src/nightfall/units.rs` (`spawn_object`, `object_guid`, `set_go_open`, the GameObject constants), `crates/benilla-app/src/nightfall/dress.rs` (`Piece: Clone`, `visible_entry_field`), `crates/benilla-app/src/nightfall/run.rs` (`Autoplay`, `autowalk_to`, `feed_pose`), `crates/benilla-app/src/nightfall/mod.rs` (modules, resources, systems)

**Interfaces:**
- Consumes: `World::{pack, loot_view, hero}`, `LootView`, `ContainerMark`, `OpenView`, `Holder`, `Act`, `Carry::{worn, lantern, load}`, `burden`, `Inventory::{get, first_empty, bags, pack, safe}`, `Place`, `EquipSlot`, `ItemKind`, `Look`, `ItemDef::light` (Tasks 1–3); `run::{Sim, LocalInput::queue, autoplay, feed_pose, autowalk}`, `view::Views`, `enemies::EnemyViews`, `units::{ground_to_bevy, wow_to_bevy}`, `dress::{Piece, pieces, wear}`, `NightfallEntity`, `Phase`, `NightfallSet` (earlier plans); `widgets::*` (Task 10); engine: `benilla_world::interact::WorldRightClick`, `crate::target::PressPick` (`object.target`, `hovered.any()`), `crate::net::{Guid, NetEntity, ObjectStore, GuidIndex}`, `crate::entities::CharCreate`, `crate::items::Items`, `benilla_world::lighting::WorldPointLight`, `benilla_world::world_point::WorldPoint`, `benilla_ui::script::UiScript::{run, has_keyboard_focus}`.
- Produces:
  - `units::{GO_TYPE_CHEST, GO_TYPE_GENERIC, spawn_object(&mut Commands, serial: u64, display: u32, scale: f32, pos: Vec3, go_type: u32) -> Entity, object_guid(serial: u64) -> u64, set_go_open(&mut ObjectStore, bool)}`.
  - `dress::visible_entry_field(slot: u8) -> u16`; `dress::Piece: Clone + Debug + PartialEq`.
  - `run::Autoplay { hold: bool, goal: Option<V2> }` (a `Resource`); `run::autowalk_to(goal: V2, from: V2, speed: f32, dt: f32) -> V2`.
  - `loot_view::{ContainerViews, CONTAINER_SERIAL_BASE, look_of(&ContainerMark, PlayerId, &Content) -> Option<(u32, f32)>}`; systems `loot_view::{draw, right_click, autoloot, forget}`.
  - `backpack::{BackpackOpen, smart_act(&Inventory, Place, open: bool, shift: bool, ctrl: bool, &Content) -> Option<Act>, holder_name(Holder, &Content, &World) -> String}`; systems `backpack::{spawn_root, toggle, show, rebuild, clicks, hover, quiet_wow_bags}`; `NIGHTFALL_BACKPACK=1` opens it (a test hook).
  - `gear::{piece_of, pieces_worn}`; systems `gear::{wear, lanterns, spawn_line, load_line}`.

- [ ] **Step 1: Write the pure parts with their tests**

In `crates/benilla-app/src/nightfall/dress.rs`, `Piece`'s derive becomes `#[derive(Clone, Debug, PartialEq)]` (add it if it has none), and add:

```rust
/// The `PLAYER_VISIBLE_ITEM_*` field holding equipment slot `slot`'s entry: 0 there shows nothing.
pub(super) fn visible_entry_field(slot: u8) -> u16 {
    FIELD_VISIBLE_ITEM_BASE + 2 + VISIBLE_ITEM_STRIDE * u16::from(slot)
}
```

`crates/benilla-app/src/nightfall/gear.rs`:

```rust
//! What heroes carry, seen: the gear every hero wears (stand-in item templates, as
//! [`super::dress`] files the starting outfits), a lit lantern's light, and our load under the
//! player frame.

use std::collections::HashMap;
use std::f32::consts::PI;

use benilla_protocol::messages::ObjectFields;
use benilla_world::lighting::WorldPointLight;
use bevy::prelude::*;
use nightfall_sim::{Content, EquipSlot, ItemDef, Look, PlayerId};

use super::dress::{self, Piece};
use super::run::Sim;
use super::view::Views;
use super::widgets::{text, GREY, RED};
use super::{content, Phase};
use crate::net::ObjectStore;

/// The piece `def` shows worn in WoW equipment slot `wow`; `outfit_of(class_id)` is the starting
/// outfit a `Look::Outfit` borrows from, in the hero's race and sex.
pub(super) fn piece_of(def: &ItemDef, wow: u8, outfit_of: impl Fn(u8) -> Vec<Piece>) -> Option<Piece> {
    match def.look? {
        Look::Item {
            display,
            inventory_type,
            class,
            subclass,
            material,
            sheath,
        } => Some(Piece {
            slot: wow,
            display,
            inventory_type: u32::from(inventory_type),
            class: u32::from(class),
            subclass: u32::from(subclass),
            material: u32::from(material),
            sheath: u32::from(sheath),
        }),
        Look::Outfit { class_id } => outfit_of(class_id).into_iter().find(|p| p.slot == wow),
    }
}

/// What a hero wearing `worn` shows in each of our equipment slots: the worn piece, else the
/// starting outfit's (`base`), else nothing. The slots that show nothing come back apart, to be
/// written 0, which takes a class weapon out of an empty hand.
pub(super) fn pieces_worn(
    worn: &[(EquipSlot, u16)],
    content: &Content,
    base: &[Piece],
    outfit_of: impl Fn(u8) -> Vec<Piece>,
) -> (Vec<Piece>, Vec<u8>) {
    let mut pieces = Vec::new();
    let mut empty = Vec::new();
    for slot in EquipSlot::ALL {
        let wow = slot.wow_slot();
        let worn_piece = worn
            .iter()
            .find(|(s, _)| *s == slot)
            .and_then(|&(_, i)| content.items.get(usize::from(i)))
            .and_then(|d| piece_of(d, wow, &outfit_of));
        match worn_piece.or_else(|| base.iter().find(|p| p.slot == wow).cloned()) {
            Some(p) => pieces.push(p),
            None => empty.push(wow),
        }
    }
    (pieces, empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::ContentSources;

    fn content() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (9.0, 0.0), (9.0, 9.0), (0.0, 9.0)],
                entries: [(1.0, 1.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "blade", name: "Blade", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand),
                 look: Some(Item(display: 7492, inventory_type: 13, class: 2, subclass: 7,
                                 material: 1, sheath: 3))),
                (key: "tunic", name: "Tunic", kind: Armour, icon: "a", weight: 1.0,
                 slot: Some(Chest), look: Some(Outfit(class_id: 1))),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    fn cloth(slot: u8, display: u32) -> Piece {
        Piece {
            slot,
            display,
            inventory_type: 5,
            class: 4,
            subclass: 1,
            material: 7,
            sheath: 0,
        }
    }

    #[test]
    fn worn_gear_covers_the_outfit_and_an_empty_hand_shows_nothing() {
        let c = content();
        let base = vec![cloth(4, 100), cloth(6, 101)];
        let warrior = |class_id: u8| {
            if class_id == 1 {
                vec![cloth(4, 900)]
            } else {
                Vec::new()
            }
        };
        let (pieces, empty) = pieces_worn(
            &[(EquipSlot::MainHand, 0), (EquipSlot::Chest, 1)],
            &c,
            &base,
            warrior,
        );
        assert!(pieces.contains(&cloth(4, 900)), "the tunic borrows the warrior's chest");
        assert!(pieces.contains(&cloth(6, 101)), "the legs stay the starting outfit's");
        assert!(pieces.iter().any(|p| p.slot == 15 && p.display == 7492));
        assert!(empty.contains(&16), "nothing in the off hand");
        assert!(!empty.contains(&15));
        let (_, bare) = pieces_worn(&[], &c, &base, |_| Vec::new());
        assert!(bare.contains(&15) && bare.contains(&16));
    }
}
```

`crates/benilla-app/src/nightfall/backpack.rs` starts with:

```rust
//! The raid's backpack and loot panels (spec §8.2): what our hero wears, their bags, backpack and
//! safe slots, and the container they have open. B opens the backpack (WoW's own bag windows stay
//! shut: their functions are quieted, since our things are not in WoW's bags); searching or
//! opening a container opens the loot panel. A click does the likely thing ([`smart_act`]): wear,
//! take off, light, drink, or put into the open container; Shift-click moves to or from a safe
//! slot; Ctrl-click drops on the ground. The host does it ([`Act`]).

use bevy::prelude::*;
use benilla_ui::script::UiScript;
use nightfall_sim::{Act, Content, EquipSlot, Holder, Inventory, ItemKind, OpenView, Place, World};

use super::run::{LocalInput, Sim};
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL, SLOT};
use super::{content, Phase};

/// What a click on our stack at `at` does: Ctrl drops it, Shift moves it to or from a safe slot,
/// with a container open it goes in; else gear is worn (or taken off into the backpack), a bag is
/// worn in a free bag slot, a lantern lit, a potion drunk. Food and drink are for Eat and Drink.
pub(super) fn smart_act(
    inv: &Inventory,
    at: Place,
    container_open: bool,
    shift: bool,
    ctrl: bool,
    content: &Content,
) -> Option<Act> {
    let s = inv.get(at)?;
    let def = &content.items[usize::from(s.item)];
    let to = |to: Place| Some(Act::Move { from: at, to });
    if ctrl {
        return (!matches!(at, Place::Bag(_))).then_some(Act::Drop(at));
    }
    if shift {
        return match at {
            Place::Safe(_) => inv.first_empty(false).and_then(to),
            Place::Bag(_) => None,
            _ => inv.first_empty(true).and_then(to),
        };
    }
    if container_open && !matches!(at, Place::Bag(_)) {
        return Some(Act::Put(at));
    }
    match at {
        Place::Equip(_) | Place::Bag(_) => inv.first_empty(false).and_then(to),
        Place::Pack(_) | Place::Safe(_) => {
            if let Some(slot) = def.slot {
                return to(Place::Equip(slot));
            }
            match def.kind {
                ItemKind::Bag => {
                    let free = inv.bags.iter().position(Option::is_none).unwrap_or(0);
                    to(Place::Bag(free as u8))
                }
                ItemKind::Lantern | ItemKind::Potion => Some(Act::Use(at)),
                _ => None,
            }
        }
    }
}

/// What the container a mark's `holder` names is called.
pub(super) fn holder_name(holder: Holder, content: &Content, world: &World) -> String {
    match holder {
        Holder::Placed(k) => content
            .loot
            .kinds
            .get(usize::from(k))
            .map_or_else(|| "Container".into(), |k| k.name.clone()),
        Holder::Body(_) => "A body".into(),
        Holder::Hero(p) => world
            .hero(p)
            .map_or_else(|| "A fallen hero".into(), |h| format!("{}'s body", h.name)),
        Holder::Remains(_) => "Your remains".into(),
        Holder::Pile => "On the ground".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{ContentSources, Stack};

    fn content() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (9.0, 0.0), (9.0, 9.0), (0.0, 9.0)],
                entries: [(1.0, 1.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                run: (backpack_slots: 3, safe_slots: 1))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0,
                 slot: Some(MainHand)),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 5),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 2),
                (key: "lantern", name: "Lantern", kind: Lantern, icon: "a", weight: 1.0,
                 light: Some((range: 18.0, intensity: 1.4))),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    #[test]
    fn a_click_does_the_likely_thing() {
        let c = content();
        let mut inv = Inventory::new(&c);
        for (item, n) in [(0, 1), (1, 3), (2, 1)] {
            inv.add(Stack::new(item, n), &c);
        }
        let (knife, bread, pouch) = (Place::Pack(0), Place::Pack(1), Place::Pack(2));
        assert_eq!(
            smart_act(&inv, knife, false, false, false, &c),
            Some(Act::Move { from: knife, to: Place::Equip(EquipSlot::MainHand) })
        );
        assert_eq!(smart_act(&inv, bread, false, false, false, &c), None);
        assert_eq!(
            smart_act(&inv, pouch, false, false, false, &c),
            Some(Act::Move { from: pouch, to: Place::Bag(0) })
        );
        assert_eq!(smart_act(&inv, bread, true, false, false, &c), Some(Act::Put(bread)));
        assert_eq!(
            smart_act(&inv, bread, false, true, false, &c),
            Some(Act::Move { from: bread, to: Place::Safe(0) })
        );
        assert_eq!(smart_act(&inv, bread, false, false, true, &c), Some(Act::Drop(bread)));
        inv.move_to(knife, Place::Equip(EquipSlot::MainHand), &c).unwrap();
        let worn = Place::Equip(EquipSlot::MainHand);
        assert_eq!(
            smart_act(&inv, worn, false, false, false, &c),
            Some(Act::Move { from: worn, to: Place::Pack(0) })
        );
        assert_eq!(smart_act(&inv, Place::Pack(0), false, false, false, &c), None);
    }

    #[test]
    fn a_lantern_is_lit_by_a_click() {
        let c = content();
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(3, 1), &c);
        assert_eq!(
            smart_act(&inv, Place::Pack(0), false, false, false, &c),
            Some(Act::Use(Place::Pack(0)))
        );
    }
}
```

In `crates/benilla-app/src/nightfall/run.rs`, add with a test:

```rust
/// `NIGHTFALL_AUTOPLAY`'s orders from the loot and the exits: stand still (searching), or walk to
/// a spot instead of the circle.
#[derive(Resource, Default)]
pub(super) struct Autoplay {
    pub hold: bool,
    pub goal: Option<V2>,
}

/// `NIGHTFALL_AUTOPLAY`: a step from `from` toward `goal`, at half speed, never past it.
pub(super) fn autowalk_to(goal: V2, from: V2, speed: f32, dt: f32) -> V2 {
    let to = goal - from;
    from + to.normalize_or_zero() * (speed * 0.5 * dt).min(to.length())
}
```

```rust
    #[test]
    fn autowalk_to_a_goal_stops_on_it() {
        let goal = V2::new(10.0, 0.0);
        let next = autowalk_to(goal, V2::ZERO, 7.0, 1.0);
        assert!((next.x - 3.5).abs() < 1e-4);
        assert_eq!(autowalk_to(goal, V2::new(9.9, 0.0), 7.0, 1.0), goal);
    }
```

and in `feed_pose` (which gains `auto: Res<Autoplay>`), the block

```rust
    if autoplay() {
        let speed = sim.world.hero(sim.me).map_or(7.0, |h| h.speed);
        pos = autowalk(V2::new(entry[0], entry[1]), pos, sim.world.elapsed, speed, time.delta_secs());
    }
```

becomes:

```rust
    if autoplay() && !auto.hold {
        let speed = sim.world.hero(sim.me).map_or(7.0, |h| h.speed);
        pos = match auto.goal {
            Some(goal) => autowalk_to(goal, pos, speed, time.delta_secs()),
            None => autowalk(V2::new(entry[0], entry[1]), pos, sim.world.elapsed, speed, time.delta_secs()),
        };
    }
```

Add `mod gear;`, `mod backpack;` and `mod loot_view;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (`worn_gear_covers_the_outfit_and_an_empty_hand_shows_nothing`, `a_click_does_the_likely_thing`, `a_lantern_is_lit_by_a_click`, `autowalk_to_a_goal_stops_on_it`), with warnings for the imports Steps 2 to 4 use.

- [ ] **Step 2: Containers in the world**

In `crates/benilla-app/src/nightfall/units.rs` (add `NetEntity` to its `crate::net` import if it is not there):

```rust
const HIGH_GAMEOBJECT: u64 = 0xF110 << 48;
const FIELD_GO_DISPLAY: u16 = 8;
const FIELD_GO_STATE: u16 = 14;
const FIELD_GO_TYPE: u16 = 21;
/// `GO_STATE_ACTIVE`: open, a chest's lid up.
const GO_STATE_ACTIVE: u32 = 0;
const GO_STATE_READY: u32 = 1;
/// `GAMEOBJECT_TYPE_CHEST`: WoW shows its loot cursor over it, and a right-click reaches us.
pub(super) const GO_TYPE_CHEST: u32 = 3;
/// `GAMEOBJECT_TYPE_GENERIC`: a model and nothing to use (an exit's ring).
pub(super) const GO_TYPE_GENERIC: u32 = 5;

/// A GameObject's guid from its serial.
pub(super) fn object_guid(serial: u64) -> u64 {
    HIGH_GAMEOBJECT | serial
}

/// A GameObject showing `GameObjectDisplayInfo` row `display`, of WoW type `go_type`.
pub(super) fn spawn_object(
    commands: &mut Commands,
    serial: u64,
    display: u32,
    scale: f32,
    pos: Vec3,
    go_type: u32,
) -> Entity {
    let guid = object_guid(serial);
    let pairs = [
        (0, guid as u32),
        (1, (guid >> 32) as u32),
        (2, 0x21),
        (4, scale.to_bits()),
        (FIELD_GO_DISPLAY, display),
        (FIELD_GO_STATE, GO_STATE_READY),
        (FIELD_GO_TYPE, go_type),
    ];
    commands
        .spawn((
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::GameObject,
                display_id: Some(display),
                scale,
            },
            ObjectStore(ObjectFields::from_pairs(&pairs)),
            Transform::from_translation(pos),
            Visibility::default(),
            NightfallEntity,
        ))
        .id()
}

/// A container drawn open (searched) or shut.
pub(super) fn set_go_open(store: &mut ObjectStore, open: bool) {
    let state = if open { GO_STATE_ACTIVE } else { GO_STATE_READY };
    store
        .0
        .merge(ObjectFields::from_pairs(&[(FIELD_GO_STATE, state)]));
}
```

`crates/benilla-app/src/nightfall/loot_view.rs`:

```rust
//! The raid's containers drawn and searched (spec §8.2): a chest, crate or grave as its WoW model,
//! open once searched; things dropped on the ground; our own remains from the last raid. A
//! right-click on one, on a dead enemy or on a fallen friend searches it. `NIGHTFALL_AUTOPLAY`
//! loots what it walks past, standing still while it searches.

use std::collections::{HashMap, HashSet};

use benilla_world::interact::WorldRightClick;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{Act, ContainerMark, Content, Holder, PlayerId};

use super::enemies::EnemyViews;
use super::run::{autoplay, Autoplay, LocalInput, Sim};
use super::view::Views;
use super::{content, units};
use crate::net::{GuidIndex, ObjectStore};

/// Containers' GameObject serials start here, above every other object Nightfall draws.
pub(super) const CONTAINER_SERIAL_BASE: u64 = 0x20_0000;

/// Which entity draws which container, and whether it was drawn open.
#[derive(Resource, Default)]
pub(super) struct ContainerViews {
    pub drawn: HashMap<u32, (Entity, bool)>,
}

/// How a container looks: its display and scale; `None` for a body, which is its unit.
pub(super) fn look_of(mark: &ContainerMark, me: PlayerId, content: &Content) -> Option<(u32, f32)> {
    match mark.holder {
        Holder::Placed(k) => content
            .loot
            .kinds
            .get(usize::from(k))
            .map(|k| (k.display, k.scale)),
        Holder::Pile => Some((content.loot.pile_display, 0.6)),
        Holder::Remains(p) if p == me => Some((content.loot.remains_display, 0.8)),
        _ => None,
    }
}

/// Every container near us drawn, open once searched; those gone from the view go. One placed
/// without a height waits for the terrain under it to stream.
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut views: ResMut<ContainerViews>,
    mut index: ResMut<GuidIndex>,
    mut stores: Query<&mut ObjectStore>,
) {
    let content = content();
    let loot = sim.world.loot_view(Some(sim.me));
    let mut seen = HashSet::new();
    for m in &loot.marks {
        let Some((display, scale)) = look_of(m, sim.me, &content) else {
            continue;
        };
        seen.insert(m.id);
        if let Some((e, open)) = views.drawn.get_mut(&m.id) {
            if *open != m.searched {
                if let Ok(mut store) = stores.get_mut(*e) {
                    units::set_go_open(&mut store, m.searched);
                }
                *open = m.searched;
            }
            continue;
        }
        let mut at = units::ground_to_bevy(m.at, m.z.unwrap_or(0.0));
        if m.z.is_none() {
            let Some(h) = world_point.terrain_height_under(at) else {
                continue;
            };
            at.y = h;
        }
        let serial = CONTAINER_SERIAL_BASE + u64::from(m.id);
        let e = units::spawn_object(&mut commands, serial, display, scale, at, units::GO_TYPE_CHEST);
        index.0.insert(units::object_guid(serial), e);
        views.drawn.insert(m.id, (e, false));
    }
    views.drawn.retain(|id, (e, _)| {
        let keep = seen.contains(id);
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// A right-click on a container, a dead enemy or a fallen hero: search it.
pub(super) fn right_click(
    mut clicks: MessageReader<WorldRightClick>,
    press: Res<crate::target::PressPick>,
    drawn: Res<ContainerViews>,
    enemies: Res<EnemyViews>,
    views: Res<Views>,
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
) {
    if clicks.read().last().is_none() {
        return;
    }
    let Some(target) = press.object.target.or(press.hovered.any()) else {
        return;
    };
    let loot = sim.world.loot_view(Some(sim.me));
    let mark_of = |holder: Holder| loot.marks.iter().find(|m| m.holder == holder).map(|m| m.id);
    let id = drawn
        .drawn
        .iter()
        .find(|(_, (e, _))| *e == target)
        .map(|(id, _)| *id)
        .or_else(|| {
            let enemy = enemies.drawn.iter().find(|(_, e)| **e == target)?.0;
            mark_of(Holder::Body(*enemy))
        })
        .or_else(|| {
            let player = views.heroes.iter().find(|(_, e)| **e == target)?.0;
            mark_of(Holder::Hero(*player))
        });
    if let Some(id) = id {
        input.queue(Act::Open(id));
    }
}

/// `NIGHTFALL_AUTOPLAY`: the nearest container within reach searched (standing still for it) and
/// emptied into the backpack, each once.
pub(super) fn autoloot(
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
    mut auto: ResMut<Autoplay>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
    mut done: Local<HashSet<u32>>,
) {
    if !autoplay() {
        return;
    }
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let content = content();
    let loot = sim.world.loot_view(Some(sim.me));
    let reach = content.rules.run.reach;
    let near = loot
        .marks
        .iter()
        .filter(|m| !m.empty && !m.locked && !done.contains(&m.id))
        .filter(|m| !matches!(m.holder, Holder::Remains(_) | Holder::Hero(_)))
        .filter(|m| m.at.distance(hero.pos) <= reach)
        .min_by(|a, b| a.at.distance(hero.pos).total_cmp(&b.at.distance(hero.pos)));
    auto.hold = near.is_some() || loot.open.is_some();
    if input.act.is_some() || !input.acts.is_empty() {
        return;
    }
    let now = real.elapsed_secs();
    if now - *last < 0.5 {
        return;
    }
    *last = now;
    match (&loot.open, near) {
        (Some(open), _) if open.search.is_some() => {}
        (Some(open), _) => {
            if open.stacks.iter().any(Option::is_some) {
                info!("nightfall: autoplay takes what is in container {}", open.id);
                input.queue(Act::TakeAll);
            }
            done.insert(open.id);
            input.queue(Act::Close);
        }
        (None, Some(m)) => input.queue(Act::Open(m.id)),
        (None, None) => {}
    }
}

/// A run ended: nothing of its containers is drawn any more (its entities went with the run).
pub(super) fn forget(mut views: ResMut<ContainerViews>) {
    *views = ContainerViews::default();
}
```

- [ ] **Step 3: The panels**

Append to `crates/benilla-app/src/nightfall/backpack.rs`, above its tests:

```rust
/// Whether the backpack is open. `NIGHTFALL_BACKPACK` opens it, for a test shot.
#[derive(Resource)]
pub(super) struct BackpackOpen(pub bool);

impl Default for BackpackOpen {
    fn default() -> Self {
        Self(std::env::var_os("NIGHTFALL_BACKPACK").is_some())
    }
}

#[derive(Component)]
pub(super) struct BackpackRoot;

#[derive(Component)]
struct LootBody;

#[derive(Component)]
struct PackBody;

#[derive(Component)]
struct PackInfo;

/// Which stack a slot shows: ours at a place, or the open container's in a slot.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PackSlot {
    Mine(Place),
    Loot(u16),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PackButton {
    TakeAll,
    Close,
}

#[derive(Resource)]
pub(super) struct PackFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            BackpackRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(120.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(8.0),
                ..default()
            },
            GlobalZIndex(400),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            for panel in [0, 1] {
                let node = Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                };
                let looks = (BackgroundColor(PANEL), BorderColor::all(EDGE));
                if panel == 0 {
                    root.spawn((LootBody, node, looks));
                } else {
                    root.spawn((PackBody, node, looks));
                }
            }
            root.spawn((PackInfo, text(&font, "", 11.0, GREY)));
        });
    commands.insert_resource(PackFont(font));
}

/// B opens and closes the backpack, unless a text box has the keyboard.
pub(super) fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    script: Option<NonSend<UiScript>>,
    mut open: ResMut<BackpackOpen>,
) {
    let typing = script.as_ref().is_some_and(|s| s.has_keyboard_focus());
    if keys.just_pressed(KeyCode::KeyB) && !typing {
        open.0 = !open.0;
    }
}

/// WoW's bag windows kept shut while Nightfall plays (our things are in our own panel), every
/// two seconds in case the interface was loaded again.
pub(super) fn quiet_wow_bags(
    script: Option<NonSend<UiScript>>,
    real: Res<Time<Real>>,
    mut last: Local<Option<f32>>,
) {
    let Some(script) = script else {
        return;
    };
    let now = real.elapsed_secs();
    if last.is_some_and(|t| now - t < 2.0) {
        return;
    }
    *last = Some(now);
    let chunk = "OpenAllBags = function() end\n\
                 CloseAllBags = function() end\n\
                 ToggleBackpack = function() end\n\
                 OpenBackpack = function() end\n\
                 ToggleBag = function() end\n";
    if let Err(e) = script.run(chunk) {
        warn!("nightfall: could not quiet WoW's bags: {e}");
    }
}

/// The panels show during a run: the loot panel while a container is open (or searched), the
/// backpack while open or beside it.
pub(super) fn show(
    phase: Res<State<Phase>>,
    open: Res<BackpackOpen>,
    sim: Option<Res<Sim>>,
    mut roots: Query<&mut Visibility, With<BackpackRoot>>,
    mut loot: Query<&mut Node, (With<LootBody>, Without<PackBody>)>,
    mut pack: Query<&mut Node, (With<PackBody>, Without<LootBody>)>,
) {
    let playing = *phase.get() == Phase::Playing;
    let looting = sim
        .as_ref()
        .is_some_and(|s| s.world.loot_view(Some(s.me)).open.is_some());
    let on = playing && (open.0 || looting);
    let want = if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
    let display = |shown: bool| if shown { Display::Flex } else { Display::None };
    for mut n in &mut loot {
        if n.display != display(looting) {
            n.display = display(looting);
        }
    }
    for mut n in &mut pack {
        let d = display(open.0 || looting);
        if n.display != d {
            n.display = d;
        }
    }
}

/// Both panels built again whenever our things or the open container change.
pub(super) fn rebuild(
    mut commands: Commands,
    sim: Res<Sim>,
    font: Option<Res<PackFont>>,
    loot_body: Query<Entity, With<LootBody>>,
    pack_body: Query<Entity, With<PackBody>>,
    mut shown: Local<Option<(Option<Inventory>, Option<OpenView>)>>,
) {
    let content = content();
    let inv = sim.world.pack(sim.me).cloned();
    let view = sim.world.loot_view(Some(sim.me));
    let now = (inv.clone(), view.open.clone());
    if shown.as_ref() == Some(&now) {
        return;
    }
    let (Ok(loot_body), Ok(pack_body), Some(font)) = (loot_body.single(), pack_body.single(), font)
    else {
        return;
    };
    *shown = Some(now);
    let font = &font.0;
    let row = |cols: usize| Node {
        flex_wrap: FlexWrap::Wrap,
        width: Val::Px(cols as f32 * (SLOT + 3.0)),
        column_gap: Val::Px(3.0),
        row_gap: Val::Px(3.0),
        ..default()
    };
    commands.entity(loot_body).despawn_related::<Children>();
    if let Some(open) = &view.open {
        let holder = view
            .marks
            .iter()
            .find(|m| m.id == open.id)
            .map(|m| m.holder)
            .unwrap_or(Holder::Pile);
        let name = holder_name(holder, &content, &sim.world);
        commands.entity(loot_body).with_children(|b| {
            b.spawn(text(font, name, 13.0, GOLD));
            match open.search {
                Some((left, _)) => {
                    b.spawn(text(font, format!("Searching… {left:.1} s"), 12.0, GREY));
                }
                None => {
                    b.spawn(row(6)).with_children(|g| {
                        for (i, s) in open.stacks.iter().enumerate() {
                            let item = s.map(|s| (&content.items[usize::from(s.item)], s.count));
                            widgets::slot(g, font, PackSlot::Loot(i as u16), item);
                        }
                    });
                    b.spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|r| {
                        widgets::text_button(r, font, PackButton::TakeAll, "Take All");
                        widgets::text_button(r, font, PackButton::Close, "Close");
                    });
                }
            }
        });
    }
    commands.entity(pack_body).despawn_related::<Children>();
    let Some(inv) = inv else {
        return;
    };
    let item_at = |at: Place| inv.get(at).map(|s| (&content.items[usize::from(s.item)], s.count));
    commands.entity(pack_body).with_children(|b| {
        b.spawn(text(font, "Worn", 11.0, GREY));
        b.spawn(row(EquipSlot::COUNT)).with_children(|g| {
            for s in EquipSlot::ALL {
                widgets::slot(g, font, PackSlot::Mine(Place::Equip(s)), item_at(Place::Equip(s)));
            }
        });
        b.spawn(text(font, "Bags", 11.0, GREY));
        b.spawn(row(nightfall_sim::MAX_BAGS)).with_children(|g| {
            for i in 0..nightfall_sim::MAX_BAGS as u8 {
                widgets::slot(g, font, PackSlot::Mine(Place::Bag(i)), item_at(Place::Bag(i)));
            }
        });
        b.spawn(text(font, format!("Backpack  ({} slots)", inv.pack.len()), 11.0, GREY));
        b.spawn(row(8)).with_children(|g| {
            for i in 0..inv.pack.len() as u16 {
                widgets::slot(g, font, PackSlot::Mine(Place::Pack(i)), item_at(Place::Pack(i)));
            }
        });
        b.spawn(text(font, "Safe slots: always kept", 11.0, GREY));
        b.spawn(row(inv.safe.len().max(1))).with_children(|g| {
            for i in 0..inv.safe.len() as u8 {
                widgets::slot(g, font, PackSlot::Mine(Place::Safe(i)), item_at(Place::Safe(i)));
            }
        });
        b.spawn(text(
            font,
            "B: close  ·  Shift-click: safe slot  ·  Ctrl-click: drop",
            10.0,
            GREY,
        ));
    });
}

/// Clicks into acts for the host.
pub(super) fn clicks(
    slots: Query<(&Interaction, &PackSlot), Changed<Interaction>>,
    buttons: Query<(&Interaction, &PackButton), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
) {
    let content = content();
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let open = sim.world.loot_view(Some(sim.me)).open.is_some();
    for (i, slot) in &slots {
        if *i != Interaction::Pressed {
            continue;
        }
        match *slot {
            PackSlot::Loot(n) => input.queue(Act::Take(n)),
            PackSlot::Mine(at) => {
                let act = sim
                    .world
                    .pack(sim.me)
                    .and_then(|inv| smart_act(inv, at, open, shift, ctrl, &content));
                if let Some(act) = act {
                    input.queue(act);
                }
            }
        }
    }
    for (i, b) in &buttons {
        if *i == Interaction::Pressed {
            input.queue(match b {
                PackButton::TakeAll => Act::TakeAll,
                PackButton::Close => Act::Close,
            });
        }
    }
}

/// The line under the panels: the stack under the mouse.
pub(super) fn hover(
    slots: Query<(&Interaction, &PackSlot)>,
    sim: Res<Sim>,
    mut info: Query<&mut Text, With<PackInfo>>,
) {
    let content = content();
    let line = slots
        .iter()
        .find(|(i, _)| **i == Interaction::Hovered)
        .and_then(|(_, s)| match *s {
            PackSlot::Mine(at) => sim.world.pack(sim.me)?.get(at).copied(),
            PackSlot::Loot(n) => sim
                .world
                .loot_view(Some(sim.me))
                .open?
                .stacks
                .get(usize::from(n))
                .copied()
                .flatten(),
        })
        .map(|s| widgets::describe(&content.items[usize::from(s.item)], s.count))
        .unwrap_or_default();
    for mut t in &mut info {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
}
```

- [ ] **Step 4: Gear, lanterns and the load line**

Append to `crates/benilla-app/src/nightfall/gear.rs`, above its tests:

```rust
/// Every hero dressed in what they wear, written again whenever it changes.
pub(super) fn wear(
    sim: Res<Sim>,
    views: Res<Views>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut stores: Query<&mut ObjectStore>,
    mut shown: Local<HashMap<PlayerId, Vec<(EquipSlot, u16)>>>,
) {
    let Some(cc) = char_create.as_deref() else {
        return;
    };
    let content = content();
    shown.retain(|p, _| views.heroes.contains_key(p));
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        if shown.get(&hero.player) == Some(&hero.carry.worn) {
            continue;
        }
        let Ok(mut store) = stores.get_mut(e) else {
            continue;
        };
        let class = &content.classes[hero.class];
        let outfit_of =
            |class_id: u8| dress::pieces(cc.0.start_outfit(hero.race, class_id, hero.sex), &[]);
        let base = outfit_of(class.class_id);
        let (pieces, empty) = pieces_worn(&hero.carry.worn, &content, &base, outfit_of);
        let mut pairs = dress::wear(&mut items, &pieces);
        pairs.extend(empty.into_iter().map(|s| (dress::visible_entry_field(s), 0)));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        shown.insert(hero.player, hero.carry.worn.clone());
    }
}

/// A lit lantern's light around its hero (spec §10), the small glow otherwise.
pub(super) fn lanterns(
    sim: Res<Sim>,
    views: Res<Views>,
    children: Query<&Children>,
    mut lights: Query<&mut WorldPointLight>,
) {
    let content = content();
    let base = &content.rules.atmosphere.hero_light;
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let (range, intensity) = match hero
            .carry
            .lantern
            .and_then(|i| content.items.get(usize::from(i)))
            .and_then(|d| d.light)
        {
            Some(l) => (l.range, l.intensity),
            None => (base.range, base.intensity),
        };
        // `4π × intensity`, the point-light convention the packer undoes.
        let watts = 4.0 * PI * intensity;
        let Ok(kids) = children.get(e) else {
            continue;
        };
        for k in kids.iter() {
            if let Ok(mut l) = lights.get_mut(k) {
                if l.range != range || l.intensity != watts {
                    l.range = range;
                    l.intensity = watts;
                }
            }
        }
    }
}

/// Our load, under the player frame.
#[derive(Component)]
pub(super) struct LoadLine;

pub(super) fn spawn_line(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        LoadLine,
        text(&font, "", 12.0, GREY),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(100.0),
            ..default()
        },
        GlobalZIndex(350),
        Visibility::Hidden,
    ));
}

/// "Load 34.0: 89% speed, louder steps", red once it slows us; the lantern, when lit.
pub(super) fn load_line(
    sim: Option<Res<Sim>>,
    phase: Res<State<Phase>>,
    mut lines: Query<(&mut Text, &mut TextColor, &mut Visibility), With<LoadLine>>,
) {
    let content = content();
    let hero = sim
        .as_ref()
        .filter(|_| *phase.get() == Phase::Playing)
        .and_then(|s| s.world.hero(s.me));
    for (mut t, mut c, mut v) in &mut lines {
        let Some(h) = hero else {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
            continue;
        };
        let r = &content.rules.run;
        let (pace, _) = nightfall_sim::burden(h.carry.load, r);
        let mut line = if pace < 1.0 {
            format!("Load {:.1}: {:.0}% speed, louder steps", h.carry.load, pace * 100.0)
        } else {
            format!("Load {:.1} of {:.0}", h.carry.load, r.weight_free)
        };
        if h.carry.lantern.is_some() {
            line.push_str("  ·  lantern lit");
        }
        let color = if pace < 1.0 { RED } else { GREY };
        if t.0 != line {
            t.0 = line;
        }
        if c.0 != color {
            c.0 = color;
        }
        if *v != Visibility::Inherited {
            *v = Visibility::Inherited;
        }
    }
}
```

- [ ] **Step 5: Register them**

In `NightfallPlugin::build`:

```rust
            .init_resource::<run::Autoplay>()
            .init_resource::<loot_view::ContainerViews>()
            .init_resource::<backpack::BackpackOpen>()
            .add_systems(Startup, (backpack::spawn_root, gear::spawn_line))
            .add_systems(OnExit(Phase::Playing), loot_view::forget)
            // Ungated: hides the load line when no run is up.
            .add_systems(Update, gear::load_line)
            .add_systems(
                Update,
                (
                    loot_view::draw,
                    loot_view::right_click,
                    loot_view::autoloot,
                    gear::wear,
                    gear::lanterns,
                    backpack::toggle,
                    backpack::quiet_wow_bags,
                    backpack::clicks,
                    backpack::rebuild,
                    backpack::hover,
                )
                    .chain()
                    .after(view::sync)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            // Ungated by the run: hides the panels outside one.
            .add_systems(Update, backpack::show.in_set(NightfallSet))
```

- [ ] **Step 6: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 7: Loot a crate in a run**

```bash
rm -rf target/nf-home/nightfall/stash.ron*
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_BACKPACK=1 NIGHTFALL_SEED=7 WOW_LIVE_SHOT=target/loot-run.png WOW_LIVE_SHOT_AT=25 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/loot-run.log 2>&1 &
```

The fresh stash raids with the free kit. Expected, once `target/loot-run-005.png` exists (then `kill` the PID):
- The log has `nightfall: autoplay takes what is in container …` at least once (the crates on the autoplay's circle).
- The shots show the two crates by the entry as WoW models (the first opens its lid once searched), the backpack panel at the right with the free kit worn (sword and dagger in the hands, on the hero too), and the loot panel with "Searching… 1.x s" in one shot and the crate's stacks or nothing in a later one; the load line under the player frame reads `Load …`.
- No `ContainerFrame` (WoW's own bag window) opens.

- [ ] **Step 8: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: containers drawn and searched by right-click; backpack and loot panels; gear worn; lanterns

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Exits on the ground, the extract count, the end screen, abandoning a raid

**Files:**
- Create: `crates/benilla-app/src/nightfall/exits.rs`, `crates/benilla-app/src/nightfall/end_screen.rs`
- Modify: `crates/benilla-app/src/nightfall/run.rs` (`leave_run`), `crates/benilla-app/src/nightfall/menu.rs` (the run bar's caption), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `LootView::exits`, `ExitMark`, `ExitDef`, `Holder::Remains`, `Carry::extracting`, `HeroOutcome`, `Fate`, `World::{abandon, hero, heroes}`, `RunRules::extract_for` (Tasks 1, 3, 6); `run::{Sim::{over, mine}, deliver, Autoplay, EndRun, autoplay}`, `stash::Stash`, `units::{spawn_object, object_guid, GO_TYPE_GENERIC, ground_to_bevy}`, `widgets::*` (Tasks 8–11); `session::{Net, Role}`, `menu::LeaveRun`, `Phase`, `NightfallSet`, `GameMode` (earlier plans); engine: `crate::poi_marker::PoiMarker::set`, `benilla_protocol::messages::GossipPoi`, `crate::player::Player::active`, `crate::glue::GlueClicks`, `WorldPoint`.
- Produces:
  - `exits::{ExitViews, clock(f32) -> String, exit_list(&[ExitMark], &Content) -> String}`; systems `exits::{draw, spawn_lines, lines, remains_marker, autoexit, forget}`; `NIGHTFALL_AUTOPLAY_EXTRACT=1` walks the autoplay to the nearest open exit.
  - `end_screen::{fate_line(&HeroOutcome) -> String, carried_line(&HeroOutcome, &Content) -> String}`; systems `end_screen::{spawn_root, show, continue_click, out_of_raid}`.
  - The run bar's button is Abandon Raid: alone or hosting it ends the raid for everyone (`World::abandon`), the end screen shows; joined, the player leaves and is counted missing; once the raid is over it goes back to the menu.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/exits.rs` starts with:

```rust
//! The ways out (spec §8.1): each open exit's ring on the ground (WoW's green rune), the exits
//! listed under the run bar with how long a late one has to go, the count while we stand in a
//! ring, our remains from the last raid marked on the minimap and the world map, and
//! `NIGHTFALL_AUTOPLAY_EXTRACT`'s walk to the nearest way out.

use std::collections::HashMap;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{Content, ExitMark, Holder};

use super::run::{Autoplay, Sim};
use super::widgets::{text, GOLD, GREY};
use super::{content, units, Phase};
use crate::net::GuidIndex;

/// Exits' GameObject serials start here.
const EXIT_SERIAL_BASE: u64 = 0x30_0000;
/// `G_RuneGroundGreen01.mdx`, and its radius at scale 1 in yards (the survivors' circles).
const RUNE: (u32, f32) = (674, 2.8);
/// How often the remains' marker is set again (WoW's own clears after 8 minutes), seconds.
const MARK_EVERY: f32 = 60.0;

/// `secs` as `m:ss`.
pub(super) fn clock(secs: f32) -> String {
    let s = secs.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The exits as lines: open ones, and the late ones with their wait; the shut ones not at all.
pub(super) fn exit_list(exits: &[ExitMark], content: &Content) -> String {
    exits
        .iter()
        .filter_map(|e| {
            let def = content.map.exits.get(usize::from(e.exit))?;
            match (e.open, e.opens_in) {
                (true, _) => Some(format!("{}: open", def.name)),
                (false, Some(t)) => Some(format!("{}: opens in {}", def.name, clock(t))),
                (false, None) => None,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::ContentSources;

    #[test]
    fn the_exits_say_when_they_open() {
        let c = nightfall_sim::Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (99.0, 0.0), (99.0, 99.0), (0.0, 99.0)],
                entries: [(1.0, 1.0, 0.0)],
                exits: [(key: "a", name: "The Bridge", at: (5.0, 5.0)),
                        (key: "b", name: "The Cart", at: (9.0, 9.0), late: true),
                        (key: "c", name: "The Road", at: (20.0, 9.0))])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: "[]",
            loot: "()",
        })
        .unwrap();
        let marks = [
            ExitMark { exit: 0, open: true, opens_in: Some(0.0) },
            ExitMark { exit: 1, open: false, opens_in: Some(754.4) },
            ExitMark { exit: 2, open: false, opens_in: None },
        ];
        assert_eq!(exit_list(&marks, &c), "The Bridge: open\nThe Cart: opens in 12:34");
        assert_eq!(clock(9.6), "0:10");
        assert_eq!(clock(-3.0), "0:00");
    }
}
```

`crates/benilla-app/src/nightfall/end_screen.rs` starts with:

```rust
//! The end of a raid on screen (spec §8.1): once we are out, a line that we wait for the others;
//! once every hero is out or dead, each hero's outcome, time and kills and what they carried out,
//! and Continue (alone or hosting) back to the menu. A joined player waits for their host.

use bevy::prelude::*;
use nightfall_sim::{Content, Fate, HeroOutcome};

use super::exits::clock;
use super::run::{EndRun, Sim};
use super::session::{Net, Role};
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, Phase};

/// What became of a hero, in a line.
pub(super) fn fate_line(o: &HeroOutcome) -> String {
    let kills = match o.kills {
        1 => "1 kill".to_string(),
        n => format!("{n} kills"),
    };
    match &o.fate {
        Fate::Extracted { exit } => {
            format!("{}: out by {exit} at {}, {kills}", o.name, clock(o.time))
        }
        Fate::Died => format!("{}: died at {}, {kills}", o.name, clock(o.time)),
        Fate::Missing => format!("{}: missing at {}, {kills}", o.name, clock(o.time)),
    }
}

/// What a hero keeps, in a line: what they carried out, what they took back from their remains,
/// and whether a body waits for them.
pub(super) fn carried_line(o: &HeroOutcome, content: &Content) -> String {
    let name = |item: u16, count: u16| {
        let n = content
            .items
            .get(usize::from(item))
            .map_or("something", |d| d.name.as_str());
        if count > 1 {
            format!("{n} ×{count}")
        } else {
            n.to_string()
        }
    };
    let kept: Vec<String> = o.carried.stacks().iter().map(|(_, s)| name(s.item, s.count)).collect();
    let mut line = if kept.is_empty() {
        "Kept nothing".to_string()
    } else {
        format!("Kept: {}", kept.join(", "))
    };
    if !o.recovered.is_empty() {
        let back: Vec<String> = o.recovered.iter().map(|s| name(s.item, s.count)).collect();
        line.push_str(&format!("  ·  Recovered: {}", back.join(", ")));
    }
    if o.recovery.is_some() {
        line.push_str("  ·  Their body waits for them in their next raid");
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::Inventory;

    fn outcome(fate: Fate, kills: u32) -> HeroOutcome {
        HeroOutcome {
            player: 0,
            name: "Valeera".into(),
            fate,
            time: 754.0,
            kills,
            carried: Inventory::default(),
            recovered: Vec::new(),
            recovery: None,
        }
    }

    #[test]
    fn a_fate_reads_as_a_line() {
        let out = outcome(Fate::Extracted { exit: "the bridge".into() }, 3);
        assert_eq!(fate_line(&out), "Valeera: out by the bridge at 12:34, 3 kills");
        assert_eq!(fate_line(&outcome(Fate::Died, 1)), "Valeera: died at 12:34, 1 kill");
        assert_eq!(fate_line(&outcome(Fate::Missing, 0)), "Valeera: missing at 12:34, 0 kills");
    }
}
```

Add `mod exits;` and `mod end_screen;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (`the_exits_say_when_they_open`, `a_fate_reads_as_a_line` with the rest).

- [ ] **Step 2: The exits' systems**

Append to `exits.rs`, above its tests:

```rust
/// Which entity draws which exit's ring.
#[derive(Resource, Default)]
pub(super) struct ExitViews {
    drawn: HashMap<u8, Entity>,
}

/// Every open exit's ring on the ground, sized to it; one that is not open (yet) is not drawn.
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut views: ResMut<ExitViews>,
    mut index: ResMut<GuidIndex>,
) {
    let content = content();
    let loot = sim.world.loot_view(Some(sim.me));
    for e in &loot.exits {
        let Some(def) = content.map.exits.get(usize::from(e.exit)) else {
            continue;
        };
        let drawn = views.drawn.get(&e.exit).copied();
        match (e.open, drawn) {
            (true, None) => {
                let mut at = units::ground_to_bevy(def.at, 0.0);
                let Some(h) = world_point.terrain_height_under(at) else {
                    continue;
                };
                at.y = h + 0.05;
                let serial = EXIT_SERIAL_BASE + u64::from(e.exit);
                let scale = def.radius / RUNE.1;
                let id =
                    units::spawn_object(&mut commands, serial, RUNE.0, scale, at, units::GO_TYPE_GENERIC);
                index.0.insert(units::object_guid(serial), id);
                views.drawn.insert(e.exit, id);
            }
            (false, Some(id)) => {
                index.0.retain(|_, v| *v != id);
                commands.entity(id).despawn();
                views.drawn.remove(&e.exit);
            }
            _ => {}
        }
    }
}

/// The exits' list under the run bar, and the count in the middle of the screen.
#[derive(Component)]
pub(super) struct ExitList;

#[derive(Component)]
pub(super) struct ExtractLine;

pub(super) fn spawn_lines(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        ExitList,
        text(&font, "", 12.0, GREY),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(56.0),
            ..default()
        },
        GlobalZIndex(350),
        Visibility::Hidden,
    ));
    commands.spawn((
        ExtractLine,
        text(&font, "", 18.0, GOLD),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(38.0),
            top: Val::Percent(30.0),
            ..default()
        },
        GlobalZIndex(350),
        Visibility::Hidden,
    ));
}

/// "The bridge to Elwynn: open …", and "Extracting… 6 / 10 s" while we count.
#[allow(clippy::type_complexity)]
pub(super) fn lines(
    sim: Option<Res<Sim>>,
    phase: Res<State<Phase>>,
    mut list: Query<(&mut Text, &mut Visibility), (With<ExitList>, Without<ExtractLine>)>,
    mut count: Query<(&mut Text, &mut Visibility), (With<ExtractLine>, Without<ExitList>)>,
) {
    let content = content();
    let sim = sim.filter(|_| *phase.get() == Phase::Playing);
    let exits = sim
        .as_ref()
        .map(|s| exit_list(&s.world.loot_view(Some(s.me)).exits, &content));
    let counting = sim
        .as_ref()
        .and_then(|s| s.world.hero(s.me))
        .map(|h| h.carry.extracting)
        .filter(|t| *t > 0.0)
        .map(|t| {
            format!(
                "Extracting… {:.0} / {:.0} s  ·  stay in the ring",
                t.floor(),
                content.rules.run.extract_for
            )
        });
    let set = |text: &mut Text, vis: &mut Visibility, line: Option<String>| {
        let want = if line.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        if let Some(line) = line {
            if text.0 != line {
                text.0 = line;
            }
        }
    };
    for (mut t, mut v) in &mut list {
        set(&mut *t, &mut *v, exits.clone());
    }
    for (mut t, mut v) in &mut count {
        set(&mut *t, &mut *v, counting.clone());
    }
}

/// Our remains from the last raid, marked on the minimap and the world map as WoW marks a guard's
/// directions; set again every minute while they wait.
pub(super) fn remains_marker(
    sim: Res<Sim>,
    real: Res<Time<Real>>,
    mut marker: ResMut<crate::poi_marker::PoiMarker>,
    mut last: Local<Option<f32>>,
) {
    let now = real.elapsed_secs();
    if last.is_some_and(|t| now - t < MARK_EVERY) {
        return;
    }
    let loot = sim.world.loot_view(Some(sim.me));
    let Some(m) = loot.marks.iter().find(|m| m.holder == Holder::Remains(sim.me)) else {
        return;
    };
    *last = Some(now);
    let poi = benilla_protocol::messages::GossipPoi {
        flags: 99,
        pos: [m.at.x, m.at.y],
        icon: 6,
        data: 0,
        name: "Your remains".into(),
    };
    marker.set(&poi, content().map.map, f64::from(now));
}

/// `NIGHTFALL_AUTOPLAY_EXTRACT`: the autoplay walks to the nearest open exit.
pub(super) fn autoexit(sim: Res<Sim>, mut auto: ResMut<Autoplay>) {
    if std::env::var_os("NIGHTFALL_AUTOPLAY_EXTRACT").is_none() {
        return;
    }
    let content = content();
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let loot = sim.world.loot_view(Some(sim.me));
    auto.goal = loot
        .exits
        .iter()
        .filter(|e| e.open)
        .filter_map(|e| content.map.exits.get(usize::from(e.exit)))
        .map(|d| d.at)
        .min_by(|a, b| a.distance(hero.pos).total_cmp(&b.distance(hero.pos)));
}

/// A run ended: nothing of its exits is drawn any more.
pub(super) fn forget(mut views: ResMut<ExitViews>) {
    *views = ExitViews::default();
}
```

(`V2` has public `x` and `y`, which `pos: [m.at.x, m.at.y]` reads; WoW's marker takes world x and y.)

- [ ] **Step 3: The end screen**

Append to `end_screen.rs`, above its tests:

```rust
#[derive(Component)]
pub(super) struct EndRoot;

#[derive(Component)]
struct EndBody;

#[derive(Component)]
struct EndContinue;

#[derive(Resource)]
pub(super) struct EndFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            EndRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(430),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                EndBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ));
        });
    commands.insert_resource(EndFont(font));
}

/// What shows: every outcome once the raid is over; "waiting" once we are out and others are not;
/// nothing while we play.
#[derive(Clone, Debug, PartialEq)]
enum Screen {
    None,
    Waiting(Vec<String>),
    Over(Vec<HeroOutcome>, bool),
}

/// The screen as the raid stands, built again whenever that changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    mut commands: Commands,
    sim: Option<Res<Sim>>,
    phase: Res<State<Phase>>,
    net: Res<Net>,
    font: Option<Res<EndFont>>,
    mut roots: Query<&mut Visibility, With<EndRoot>>,
    body: Query<Entity, With<EndBody>>,
    mut shown: Local<Option<Screen>>,
) {
    let content = content();
    let screen = match sim.as_ref().filter(|_| *phase.get() == Phase::Playing) {
        None => Screen::None,
        Some(s) => match &s.over {
            Some(all) => Screen::Over(all.clone(), matches!(net.role, Role::Client(_))),
            None if s.world.hero(s.me).is_none() && !s.world.heroes.is_empty() => {
                Screen::Waiting(s.world.heroes.iter().map(|h| h.name.clone()).collect())
            }
            None => Screen::None,
        },
    };
    if shown.as_ref() == Some(&screen) {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let want = if screen == Screen::None {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
    let font = &font.0;
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| match &screen {
        Screen::None => {}
        Screen::Waiting(names) => {
            b.spawn(text(font, "You got out.", 22.0, GOLD));
            b.spawn(text(
                font,
                format!("Waiting for the others: {}", names.join(", ")),
                13.0,
                GREY,
            ));
        }
        Screen::Over(all, joined) => {
            b.spawn(text(font, "The raid is over", 22.0, GOLD));
            for o in all {
                b.spawn(text(font, fate_line(o), 14.0, Color::WHITE));
                b.spawn(text(font, carried_line(o, &content), 11.0, GREY));
            }
            if *joined {
                b.spawn(text(font, "Waiting for the host…", 12.0, GREY));
            } else {
                widgets::text_button(b, font, EndContinue, "Continue");
            }
        }
    });
    *shown = Some(screen);
}

/// Continue: alone or hosting, back to the menu, the party with us.
pub(super) fn continue_click(
    buttons: Query<&Interaction, (Changed<Interaction>, With<EndContinue>)>,
    mut net: ResMut<Net>,
    mut end: ResMut<EndRun>,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        net.broadcast_ended();
        end.0 = true;
    }
}

/// Out of the raid (extracted, the others still in): WoW's controller lets go of the body that
/// is no more. Before the first state a joined mirror has no heroes at all, which is not "out".
pub(super) fn out_of_raid(sim: Res<Sim>, mut player: ResMut<crate::player::Player>) {
    let out = sim.world.hero(sim.me).is_none() && !sim.world.heroes.is_empty();
    if out && player.active {
        player.active = false;
    }
}
```

- [ ] **Step 4: Abandon Raid**

In `crates/benilla-app/src/nightfall/menu.rs`, the run bar's `glue_button(c, &art, &font, LeaveRun, "Leave Run", …)` caption becomes `"Abandon Raid"`.

In `crates/benilla-app/src/nightfall/run.rs`, `leave_run` becomes:

```rust
/// Abandon Raid: alone or hosting, the raid ends now for everyone (spec §8.1: those still in are
/// missing) and the end screen shows; once it is over, the button goes back to the menu. Joined,
/// we walk out of the session, counted missing (no outcome will come for us).
pub(super) fn leave_run(
    clicks: Res<GlueClicks>,
    buttons: Query<Entity, With<LeaveRun>>,
    mut net: ResMut<Net>,
    mut end: ResMut<EndRun>,
    mut sim: ResMut<Sim>,
    mut stash: ResMut<super::stash::Stash>,
) {
    if !buttons.iter().any(|e| clicks.hit(e)) {
        return;
    }
    if net.is_client() {
        stash.profile.settle_missing(&content());
        stash.save();
        net.leave("You left the raid");
        end.0 = true;
        return;
    }
    if sim.over.is_some() {
        net.broadcast_ended();
        end.0 = true;
        return;
    }
    sim.world.abandon();
    deliver(&mut sim, &mut net);
}
```

- [ ] **Step 5: Register them**

In `NightfallPlugin::build`:

```rust
            .init_resource::<exits::ExitViews>()
            .add_systems(Startup, (exits::spawn_lines, end_screen::spawn_root))
            .add_systems(OnExit(Phase::Playing), exits::forget)
            // Ungated by the run: hide when none is up.
            .add_systems(Update, (exits::lines, end_screen::show).in_set(NightfallSet))
            .add_systems(
                Update,
                (
                    exits::draw,
                    exits::remains_marker,
                    exits::autoexit,
                    end_screen::continue_click,
                    end_screen::out_of_raid,
                )
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

and the systems that read `Sim` with `Option<Res<Sim>>` (`exits::lines`, `end_screen::show`) need no gate. `end_screen::continue_click` must run before `run::back_to_menu`: add `.before(run::back_to_menu)` to that set.

- [ ] **Step 6: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 7: Get out, and see the end screen**

A test content folder with a way out at the first entry and no enemies (a blow would start the count over, and this shot is about the count):

```bash
rm -rf target/nf-exit && mkdir -p target/nf-exit && cp content/nightfall/*.ron target/nf-exit/
sed -i 's/^    exits: \[/    exits: [\n        (key: "test_exit", name: "a test way out", at: (-10051.8, -792.5)),/' target/nf-exit/map.ron
echo "[]" > target/nf-exit/spawns.ron
grep -n "test_exit" target/nf-exit/map.ron
rm -rf target/nf-home/nightfall/stash.ron*
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home NIGHTFALL_CONTENT=target/nf-exit WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_EXTRACT=1 NIGHTFALL_SEED=7 WOW_LIVE_SHOT=target/extract.png WOW_LIVE_SHOT_AT=14 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=3 ./target/release/azeroth-together.exe > target/extract.log 2>&1 &
```

Expected, once `target/extract-005.png` exists (then `kill` the PID):
- The early shots show the green rune under the hero, the exit list at the top right ("a test way out: open", the cart "opens in 17:5x") and "Extracting… n / 10 s · stay in the ring" counting up.
- The last shots show the end screen: "The raid is over", "<name>: out by a test way out at 0:1x, 0 kills", "Kept: Worn Shortsword, Worn Dagger, …" and Continue.
- `target/nf-home/nightfall/stash.ron` has no `in_raid`, its `loadout` holds what was kept, and `extracted: 1`.

And a death, its body left for the next raid:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_FRAIL=0 WOW_LIVE_SHOT=target/died.png WOW_LIVE_SHOT_AT=80 ./target/release/azeroth-together.exe > target/died.log 2>&1 &
```

Expected, once `target/died.png` exists (then `kill` the PID): the end screen reads "died at 1:0x" and "Their body waits for them in their next raid"; `stash.ron` has a `recovery: Some((map: "duskwood", …))`. Start the same command with `WOW_LIVE_SHOT_AT=30` (and no `NIGHTFALL_FRAIL`): the shot shows the red flag of "Your remains" on the minimap at the body's place, and the remains' model there when the hero is within sight of it.

- [ ] **Step 8: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: exits on the ground, the extract count, the end screen, abandoning a raid, remains marked

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Docs, the full check, and a raid in co-op

**Files:**
- Modify: `CLAUDE.md` (the Nightfall section), `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line), `scripts/package.sh` (only if it copies `content/` file by file)

- [ ] **Step 1: Write the docs**

In `CLAUDE.md`'s Nightfall section, add:

```markdown
- Nightfall's run loop (sub-project 5a): items are `content/nightfall/items.ron` (WoW icons; worn
  looks as item displays or a class's starting-outfit piece), containers, loot tables and enemies'
  bodies `loot.ron`, the exits `map.ron`'s `exits`, the numbers `rules.ron`'s `run:` (`death_loss`,
  `safe_slots`, `weight_*`, `extract_for`, `cart_opens_at`, `free_meal`). `nightfall-sim`'s
  `inventory.rs`, `raid/` (acts, containers, extracting, deaths, outcomes, `RaidRoll`) and
  `profile.rs` are the rules; the client keeps the stash in `benilla-config/nightfall/stash.ron`
  (versioned; `.bak` is the last good file, a damaged one is kept as `.broken`). In a raid B opens
  the backpack, a right-click searches a container, a body or a fallen friend. Test hooks:
  `NIGHTFALL_GIVE=<item>:<n>,…` (into the stash at start), `NIGHTFALL_STASH=1` and
  `NIGHTFALL_BACKPACK=1` (open the panels), `NIGHTFALL_SEED=<n>` (the raid's loot),
  `NIGHTFALL_AUTOPLAY_EXTRACT=1` (the autoplay walks out by the nearest exit). Use
  `BENILLA_HOME=target/nf-home` for test runs, never the player's own stash.
```

In the spec's status line, after the sentence sub-project 4 added, add: `Sub-project 5a built (plan docs/superpowers/plans/2026-10-17-nightfall-run-loop.md); 5b, the inn, is docs/superpowers/plans/2026-10-17-nightfall-inn.md.`

If `scripts/package.sh` copies `content/` file by file rather than whole, add `items.ron` and `loot.ron` to its Nightfall list; if it copies the folder, leave it.

- [ ] **Step 2: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS (the Darkshire bake test skips without `WOW_DATA`).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds with no new warnings in `nightfall-sim` or `nightfall/`.

- [ ] **Step 3: A raid in co-op on one PC**

Start the host in the background, and a few seconds later the client, each with its own home (each player's stash is their own):

```bash
rm -rf target/nf-home-host target/nf-home-client
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-host NIGHTFALL_CONTENT=target/nf-exit WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_EXTRACT=1 NIGHTFALL_NAME=Host NIGHTFALL_GIVE=ruby_shard:2 WOW_LIVE_SHOT=target/host-raid.png WOW_LIVE_SHOT_AT=45 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/host-raid.log 2>&1 &
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-client NIGHTFALL_CONTENT=target/nf-exit WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend WOW_LIVE_SHOT=target/client-raid.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=8 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/client-raid.log 2>&1 &
```

Expected, once both last shots exist (then `kill` both PIDs):
- The host walks out by the test exit; the client's shots show the host's hero gone and the friend still in, and the host's shots "You got out. Waiting for the others: Friend".
- `target/nf-home-host/nightfall/stash.ron` has `extracted: 1`; `target/nf-home-client/nightfall/stash.ron` is the client's own (the mage's kit in `in_raid` while the run lasts, nothing of the host's).
- Killing the client mid-raid and starting it again (without `NIGHTFALL_JOIN`) shows "Your last raid ended without you: you were counted missing." on its menu.
- Neither log has a panic or a `Refused` loop.

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md scripts/package.sh
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: document the run loop; sub-project 5a built

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review against the spec

| Spec | Where |
|---|---|
| §8.1 exits (3 or 4: the bridge, the Westfall road, the Deadwind road, the late Night Watch cart), 10 s in the ring, a blow restarts it, extracting makes noise | Task 1 (`map.ron`'s exits, `cart_opens_at`), Task 6 (`step_exits`), Task 12 (rings, the count) |
| §8.1 Darkshire guarded and lit, not a safe zone | Not here: the cart is placed there; Darkshire's guards and lamps are sub-projects 3 (lit areas) and 6 (risk zones); see the rulings |
| §8.1 the run ends when every hero has extracted or died; the end screen with outcome, time, kills, what was carried out | Task 6 (`step_over`, outcomes, kills), Task 12 (end screen) |
| §8.1 every raid a little different | Task 3's `RaidRoll` seam; loot rolled by its seed (Task 5); the rest sub-project 6 |
| §8.2 items in `items.ron` with WoW displays, kinds (weapons, armour, potions, food and water, valuables, keys, bags) | Task 1 |
| §8.2 16-slot backpack, bags add slots | Tasks 1 (`backpack_slots`), 2 (`Inventory`), 11 (panel) |
| §8.2 weight: slower walking and sneaking to `weight_slow_floor`, louder footsteps to `weight_noise_max`, never stops fighting | Tasks 1 (`run:`), 3 (`burden` in the senses' pace and steps), 11 (load line) |
| §8.2 `death_loss` three modes, the same everywhere; `everything_one_recovery`'s body in the next raid, marked on the map, one chance | Tasks 2 (`split_for_death`), 6 (bodies, recovery), 7 (`settle`), 12 (marker) |
| §8.2 teammates loot a fallen hero's body | Task 6 (`a_friend_loots_a_fallen_heros_body`), Task 11 (right-click on the body) |
| §8.2 `safe_slots` (2) always kept | Tasks 1, 2, 6 |
| §8.2 containers: chests, crates, bodies, graves dug (loud) | Tasks 1 (`loot.ron`), 5 (search times and sounds) |
| §8.2 the companion | Sub-project 5b (`2026-10-17-nightfall-inn.md`) |
| §8.3 stash per player in `benilla-config/nightfall/stash.ron`, gold | Tasks 7, 9, 10 |
| §8.3 the free basic kit, always available | Tasks 1 (`free_kit`), 7 (`free_kit`, `raid_loadout`), 10 (the panel's line) |
| §8.3 co-op: each stash on its own machine; the host trusts the loadout | Tasks 3 (`give_loadout` sanitizes, never refuses), 8 (`HeroPick::loadout`), 13 (two homes) |
| §8.4, §8.5 the inn, contracts, stations | Sub-project 5b |
| §11 `death_loss` (default `everything_one_recovery`), `safe_slots`, `weight_free`, `weight_slow_floor`, `weight_noise_max`, `cart_opens_at` | Task 1 |
| §6.1 eat and drink, now with food and water as items; §6.2 Conjure Water | Task 4 |
| §5.1 the lantern's ×1.6 | Task 3 (lit lantern in the senses), Task 11 (its light) |
| §12 tests | sim unit tests in Tasks 1–8; client tests in Tasks 8–12; shots in 9–13 |
