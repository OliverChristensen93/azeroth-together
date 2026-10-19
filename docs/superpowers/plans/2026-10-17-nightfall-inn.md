# Nightfall Sub-project 5b (The Inn) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A reason to go back in: between raids the party walks Goldshire's Lion's Pride Inn, its own scene at a warm evening light; its NPCs stand at their places (a banker keeping the stash, a trader, Duskwood's contract givers, the stations' keepers) and open our own panels; contracts pay gold and unlock stations; stations, upgraded with gold and found materials, give stash room, recipes, better lanterns, intel on the next raid and a companion that carries one item never lost; the inn's door opens the raid board (the map, the contracts taken, everyone's Ready); friends who join walk the inn with you. A timeboxed spike writes down whether WoW's own gossip, quest, merchant and bank windows could be fed without a server.

**Architecture:** `nightfall-sim` gains the inn as content of its own (`inn.rs`: `hub.ron`, `contracts.ron`, `stations.ron`, loaded beside the raid's `Content` and checked against it), contracts' and stations' pure logic on the `Profile` (version 2: `InnProgress`), the companion's slot in the `Inventory` (never lost, never weighed), kills by kind and place in each outcome, and lanterns that set their own visibility. The lobby becomes the inn: each player's inn pose and Ready ride the lobby messages, and the host plans the next raid's `RaidRoll` there. The client's `Phase::Menu` becomes `Phase::Inn`: the hero walks the hub, kept in its bounds, friends drawn where they stand, NPCs drawn as dressed WoW characters, a right-click on one opening its panel, and the door's ring opening the raid board.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content and the stash file), postcard (wire).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md`: §8.4 (the inn, its own scene, the door and raid board, the co-op lobby, NPCs, our own panels first, the spike), §8.5 (contracts, stations), §8.2's companion, §1.1's "a reason for every raid". This plan starts after every task of `docs/superpowers/plans/2026-10-17-nightfall-run-loop.md` (5a) is done, and of the four plans before it; it uses their names. Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-inn-rulings.md`.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call. Other agents commit in this worktree: stage only the files a task names, never `git add -A`.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`. Build `benilla-app` and `azeroth-together` with `-j 2` (`-j 1` if a build is killed).
- Every game launch sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data` and a test home (`BENILLA_HOME=target/nf-home`). The exe does not exit after its shots: start it in the background, wait for the last shot file, then `kill` its PID.
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats`. Contracts, stations, recipes, the trader and the companion are pure sim logic with unit tests.
- Every number is content: `content/nightfall/hub.ron` (the hub's place, NPCs, trader, companions, contracts at once), `contracts.ron`, `stations.ron`. **The hub's place (map, spawn, bounds, door, NPC spots, light) lives only in `hub.ron`, so moving it to Darkshire's Scarlet Raven Tavern is a content edit** (a commented alternative ships in the file).
- Co-op: each client keeps its own stash, contracts and stations; the inn is the lobby; the host trusts what a client brings.
- No engine crate changes. New files are preferred; edits to files earlier plans wrote are spelled out.
- Our own panels first (spec §8.4); the spike's code is thrown away and only its findings doc is committed.
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Contract progress counted twice, or from the wrong raid** (an outcome settled once but applied twice, a contract not taken, a raid that went missing, a Bring contract turned in twice). Expect progress once per outcome and only for taken contracts, nothing from a missing raid, and a Bring's items taken exactly once. Pinned in Task 4 (`progress_comes_once_per_outcome_and_only_to_taken_contracts`, `bringing_takes_the_items_once`).
2. **An upgrade or a craft half done** (gold paid, materials short; the gift given without the price). Expect both or neither. Pinned in Task 5 (`an_upgrade_pays_and_gives_or_does_neither`, `a_craft_pays_and_makes_or_does_neither`).
3. **The companion's stack lost** under any `death_loss`, a missing raid, or a loadout made without the companion. Expect it kept everywhere, never weighed. Pinned in Task 2 (`the_companion_keeps_its_stack_through_every_death`) and Task 3 (`the_companion_comes_with_its_stack`).
4. **The inn's lobby flooding the wire or starting a raid not everyone readied** (poses every frame, a friend who leaves while ready, a ready friend who walks off). Expect the lobby sent only when it changed and at most ten times a second, and a start only when everyone present is ready. Pinned in Task 6 (`the_lobby_goes_when_it_changes_and_no_faster`, `all_ready_counts_everyone_present`).
5. **A 5a stash file opened by 5b, and a 5b file opened by an older build.** Expect the old file to load with an empty inn and nothing lost, and the newer one refused by the older build (5a's own guard). Pinned in Task 3 (`a_version_1_stash_loads_with_an_empty_inn`) and 5a's `a_newer_profile_is_refused_not_misread`.

**Not in this plan** (see the rulings file): the Stitches contract (Stitches is sub-project 6's; the contract waits, commented, in `contracts.ron`); the map table's intel on signature monsters and weather (sub-project 6 rolls them; the table shows what the `RaidRoll` holds now: which exits open when); Stormwind's Trade District as a hub; WoW's own windows (the spike decides whether a later plan switches to them).

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/inn.rs` (new) | `InnContent` and its parts: the hub's place, NPCs, trader, companions, contracts, stations, recipes; loading and checking |
| `crates/nightfall-sim/src/contracts.rs` (new) | Taking, dropping, progress from outcomes, turning in |
| `crates/nightfall-sim/src/stations.rs` (new) | Station levels and upgrades, crafting, buying and selling at the trader |
| `crates/nightfall-sim/src/{inventory,items,profile,messages}.rs`, `raid/{mod,extract}.rs`, `senses/mod.rs` | The companion's slot; lanterns' own visibility; `InnProgress` (profile v2); kills by kind and place; inn poses, Ready and the planned raid on the wire; protocol 6 |
| `content/nightfall/{hub,contracts,stations}.ron` (new), `items.ron`, `loot.ron` | The inn, its contracts and stations; contract items and lanterns; the tome's grave |
| `crates/benilla-app/src/nightfall/inn.rs` (new) | The inn scene: our hero walking it, kept in bounds, friends drawn, the evening light |
| `crates/benilla-app/src/nightfall/npcs.rs` (new) | NPCs drawn at their places; a right-click opens their panel |
| `crates/benilla-app/src/nightfall/trader.rs` (new) | The trader's panel |
| `crates/benilla-app/src/nightfall/inn_panels.rs` (new) | Contract givers' and station keepers' panels |
| `crates/benilla-app/src/nightfall/board.rs` (new) | The door and the raid board: map, contracts, intel, party, Ready |
| `crates/benilla-app/src/nightfall/companion.rs` (new) | The companion following its hero in a raid |
| `crates/benilla-app/src/nightfall/{mod,menu,session,run,stash,stash_panel,backpack,atmosphere}.rs` | `Phase::Inn`, the inn bar, the lobby's poses and Ready, the planned raid, contracts settled, the stash panel at the banker, the companion's slot, the hub's light |
| `docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md` (new) | The spike's findings |

---

### Task 1: The inn as content: the hub, NPCs, trader, companions, contracts, stations, recipes

**Files:**
- Create: `crates/nightfall-sim/src/inn.rs`, `content/nightfall/hub.ron`, `content/nightfall/contracts.ron`, `content/nightfall/stations.ron`
- Modify: `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`, `content/nightfall/items.ron` (the tome, two lanterns), `content/nightfall/loot.ron` (Mor'Ladim's grave)
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (`prepare` reads the inn; `inn()`; `content_texts` grows to 11)

**Interfaces:**
- Consumes: `Content::{item, enemy, items, enemies, map.exits}`, `ContentError`, `Bounds`, `V2`, `V3` (earlier plans).
- Produces:
  - `nightfall_sim::{InnSources, InnContent, HubPlace, NpcDef, NpcRole, Body, TraderDef, CompanionDef, ContractDef, Objective, Reward, StationDef, StationLevel, Gift, RecipeDef}`.
  - `InnSources<'a> { hub, contracts, stations: &'a str }`; `InnContent::from_ron(InnSources, &Content) -> Result<InnContent, ContentError>`; `InnContent::{npc, contract, station, recipe, companion}(&self, key: &str) -> Option<usize>`; `InnContent::banker(&self) -> usize`.
  - `HubPlace { name, map: u32, spawn: V3, facing: f32, bounds: Bounds, door: V2, door_radius: f32, minute: u32, view_distance: f32 }`.
  - `NpcDef { key, name, title, role: NpcRole, body: Body, at: V3, facing: f32 }`; `NpcRole::{Banker, Trader, Contracts, Station(usize)}`; `Body { race: u8, sex: u8, class_id: u8 }`.
  - `TraderDef { stock: Vec<(usize, u32)>, sells_for: f32 }`; `CompanionDef { key, name, display: u32, scale: f32 }`; `InnContent::contracts_at_once: u8`.
  - `ContractDef { key, giver: usize, name, text, objectives: Vec<Objective>, reward: Reward, needs: Vec<usize>, repeatable: bool }`; `Objective::{Kill { enemy: usize, count: u32, near: Option<(V2, f32)> }, Bring { item: usize, count: u32 }, Extract { exit: Option<usize> }}`; `Reward { gold: u32, unlocks: Vec<usize> }`.
  - `StationDef { key, name, keeper: usize, open: bool, levels: Vec<StationLevel> }`; `StationLevel { gold: u32, items: Vec<(usize, u32)>, gives: Vec<Gift> }`; `Gift::{StashSlots(u16), Recipe(usize), Intel, Companion(usize)}`; `RecipeDef { key, station: usize, makes: (usize, u16), needs: Vec<(usize, u32)>, gold: u32 }`.
  - Client: `nightfall::inn() -> Arc<InnContent>`; `content_texts() -> &'static [String; 11]` (the three inn files last).

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/inn.rs` ends with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentSources;

    fn raid() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                entries: [(50.0, 50.0, 0.0)],
                exits: [(key: "road", name: "The Road", at: (90.0, 90.0))])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: r#"[(key: "worgen", name: "Worgen", display: 657, level: 27, hp: 900.0,
                walk_speed: 2.5, run_speed: 8.0)]"#,
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "tome", name: "Tome", kind: Valuable, icon: "a", weight: 1.0),
                (key: "cloth", name: "Cloth", kind: Material, icon: "a", weight: 0.1, stack: 20),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 4),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.2, stack: 20),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    const HUB: &str = r#"(
        place: (name: "Inn", map: 0, spawn: (10.0, 10.0, 0.0), facing: 0.0,
                bounds: [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
                door: (at: (19.0, 10.0), radius: 2.0), minute: 1170, view_distance: 200.0),
        npcs: [
            (key: "banker", name: "Farley", role: Banker, body: (race: 1, sex: 0, class_id: 1),
             at: (5.0, 5.0, 0.0)),
            (key: "trader", name: "Brog", role: Trader, body: (race: 1, sex: 0, class_id: 1),
             at: (5.0, 15.0, 0.0)),
            (key: "watch", name: "Ebonlock", role: Contracts, body: (race: 1, sex: 1, class_id: 2),
             at: (15.0, 5.0, 0.0)),
            (key: "smith", name: "Smith", role: Station("bench"), body: (race: 1, sex: 0, class_id: 1),
             at: (15.0, 15.0, 0.0)),
        ],
        contracts_at_once: 2,
        trader: (stock: [(item: "bread", price: 2)], sells_for: 0.5),
        companions: [(key: "hound", name: "Hound", display: 1052, scale: 0.6)],
    )"#;
    const CONTRACTS: &str = r#"[
        (key: "pack", giver: "watch", name: "Thin the pack",
         objectives: [Kill(enemy: "worgen", count: 3, near: Some((50.0, 50.0, 20.0)))],
         reward: (gold: 60, unlocks: ["bench"]), repeatable: true),
        (key: "tome", giver: "watch", name: "The tome", needs: ["pack"],
         objectives: [Bring(item: "tome", count: 1), Extract(exit: Some("road"))],
         reward: (gold: 100)),
    ]"#;
    const STATIONS: &str = r#"(
        stations: [
            (key: "stash", name: "The Stash", keeper: "banker", open: true,
             levels: [(gold: 50, items: [("cloth", 10)], gives: [StashSlots(20)])]),
            (key: "bench", name: "Workbench", keeper: "smith",
             levels: [(gold: 40, gives: [Recipe("pouch")]),
                      (gold: 90, gives: [Intel, Companion("hound")])]),
        ],
        recipes: [(key: "pouch", station: "bench", makes: ("pouch", 1), needs: [("cloth", 6)],
                   gold: 5)],
    )"#;

    fn load(hub: &str, contracts: &str, stations: &str) -> Result<InnContent, ContentError> {
        InnContent::from_ron(
            InnSources {
                hub,
                contracts,
                stations,
            },
            &raid(),
        )
    }

    #[test]
    fn the_inn_loads() {
        let inn = load(HUB, CONTRACTS, STATIONS).unwrap();
        assert_eq!(inn.banker(), 0);
        assert_eq!(inn.npcs[3].role, NpcRole::Station(1));
        assert_eq!(inn.trader.stock, vec![(3, 2)]);
        assert_eq!(inn.contracts[1].needs, vec![0]);
        assert_eq!(
            inn.contracts[1].objectives,
            vec![
                Objective::Bring { item: 0, count: 1 },
                Objective::Extract { exit: Some(0) }
            ]
        );
        assert_eq!(inn.contracts[0].reward.unlocks, vec![1]);
        assert_eq!(inn.stations[1].levels[1].gives, vec![Gift::Intel, Gift::Companion(0)]);
        assert_eq!(inn.recipes[0].station, 1);
        assert_eq!(inn.recipe("pouch"), Some(0));
        assert_eq!(inn.place.door, V2::new(19.0, 10.0));
        assert_eq!(inn.place.minute, 1170);
    }

    #[test]
    fn the_hub_is_wherever_its_content_puts_it() {
        // Darkshire instead of Goldshire: only hub.ron changes.
        let moved = HUB
            .replace("(10.0, 10.0, 0.0), facing", "(60.0, 60.0, 5.0), facing")
            .replace(
                "[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]",
                "[(50.0, 50.0), (70.0, 50.0), (70.0, 70.0), (50.0, 70.0)]",
            )
            .replace("(19.0, 10.0)", "(69.0, 60.0)")
            .replace("(5.0, 5.0, 0.0)", "(55.0, 55.0, 5.0)")
            .replace("(5.0, 15.0, 0.0)", "(55.0, 65.0, 5.0)")
            .replace("(15.0, 5.0, 0.0)", "(65.0, 55.0, 5.0)")
            .replace("(15.0, 15.0, 0.0)", "(65.0, 65.0, 5.0)");
        let inn = load(&moved, CONTRACTS, STATIONS).unwrap();
        assert_eq!(inn.place.spawn, V3::new(60.0, 60.0, 5.0));
        assert!(inn.place.bounds.contains(V2::new(60.0, 60.0)));
    }

    #[test]
    fn a_hub_with_its_people_outside_it_is_refused() {
        for bad in [
            HUB.replace("(5.0, 5.0, 0.0)", "(50.0, 5.0, 0.0)"),
            HUB.replace("(19.0, 10.0)", "(29.0, 10.0)"),
            HUB.replace("(10.0, 10.0, 0.0), facing", "(30.0, 10.0, 0.0), facing"),
            HUB.replace("minute: 1170", "minute: 1440"),
            HUB.replace("role: Banker", "role: Trader"),
        ] {
            assert!(
                matches!(load(&bad, CONTRACTS, STATIONS), Err(ContentError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn contracts_and_stations_naming_what_is_not_there_are_refused() {
        for (contracts, stations) in [
            (CONTRACTS.replace("enemy: \"worgen\"", "enemy: \"dragon\""), STATIONS.to_string()),
            (CONTRACTS.replace("giver: \"watch\", name: \"Thin", "giver: \"smith\", name: \"Thin"), STATIONS.to_string()),
            (CONTRACTS.replace("needs: [\"pack\"]", "needs: [\"nothing\"]"), STATIONS.to_string()),
            (CONTRACTS.replace("exit: Some(\"road\")", "exit: Some(\"sea\")"), STATIONS.to_string()),
            (CONTRACTS.replace("count: 3", "count: 0"), STATIONS.to_string()),
            (CONTRACTS.to_string(), STATIONS.replace("Recipe(\"pouch\")", "Recipe(\"cake\")")),
            (CONTRACTS.to_string(), STATIONS.replace("keeper: \"smith\"", "keeper: \"watch\"")),
            (CONTRACTS.to_string(), STATIONS.replace("station: \"bench\"", "station: \"stash\"")),
        ] {
            assert!(
                matches!(load(HUB, &contracts, &stations), Err(ContentError::Invalid(_))),
                "{contracts}\n{stations}"
            );
        }
    }
}
```

In `lib.rs` add `mod inn;` and:

```rust
pub use inn::{
    Body, CompanionDef, ContractDef, Gift, HubPlace, InnContent, InnSources, NpcDef, NpcRole,
    Objective, RecipeDef, Reward, StationDef, StationLevel, TraderDef,
};
```

Run: `cargo test -p nightfall-sim inn`
Expected: FAIL to compile (`cannot find type InnContent`).

- [ ] **Step 2: Write the inn's content**

Above the tests in `inn.rs`:

```rust
//! The inn (spec §8.4, §8.5) as content of its own: the hub's place (its map, where heroes
//! appear, the rooms' bounds, the door, the evening's light), the NPCs at their places, the
//! trader's stock, the companions, the contracts Duskwood's characters give, and the stations
//! and their recipes. Read from `content/nightfall/hub.ron`, `contracts.ron` and `stations.ron`
//! beside the raid's content and checked against it, so the hub can move (to Darkshire, say) by
//! editing `hub.ron` alone.

use serde::Deserialize;

use crate::{Bounds, Content, ContentError, V2, V3};

/// The hub: a place on a map the heroes walk between raids.
#[derive(Clone, Debug, PartialEq)]
pub struct HubPlace {
    pub name: String,
    /// `Map.dbc` id.
    pub map: u32,
    /// Where heroes appear, WoW `(x, y, z)`.
    pub spawn: V3,
    /// Degrees from north toward west.
    pub facing: f32,
    /// The rooms: heroes are kept inside, as a raid keeps them inside its map.
    pub bounds: Bounds,
    /// The door's ring: stepping in opens the raid board.
    pub door: V2,
    pub door_radius: f32,
    /// Minute of the day the light stands at.
    pub minute: u32,
    /// How far the world is drawn, yards.
    pub view_distance: f32,
}

/// What an NPC does when clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NpcRole {
    /// Keeps the stash: the stash panel.
    Banker,
    /// Buys and sells.
    Trader,
    /// Gives their contracts.
    Contracts,
    /// Keeps a station, by index.
    Station(usize),
}

/// An NPC's body: a character of this race and sex in this class's starting outfit (always in
/// the data, as the items' `Outfit` looks are).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Body {
    pub race: u8,
    pub sex: u8,
    pub class_id: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NpcDef {
    pub key: String,
    pub name: String,
    /// A line under the name.
    pub title: String,
    pub role: NpcRole,
    pub body: Body,
    pub at: V3,
    pub facing: f32,
}

/// The trader: what they sell and for how much, and the share of an item's value they pay.
#[derive(Clone, Debug, PartialEq)]
pub struct TraderDef {
    /// (item, gold each).
    pub stock: Vec<(usize, u32)>,
    pub sells_for: f32,
}

/// A companion (spec §8.2): a small pet that follows its hero and carries one stack never lost.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CompanionDef {
    pub key: String,
    pub name: String,
    /// `CreatureDisplayInfo.dbc` id.
    pub display: u32,
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

/// One thing a contract asks.
#[derive(Clone, Debug, PartialEq)]
pub enum Objective {
    /// `count` enemies of kind `enemy` killed (within `near`'s circle when it has one), over any
    /// number of raids.
    Kill {
        enemy: usize,
        count: u32,
        near: Option<(V2, f32)>,
    },
    /// `count` of `item` brought to the giver: taken from the stash and loadout as the contract
    /// is turned in.
    Bring { item: usize, count: u32 },
    /// Out alive by `exit` (any exit for `None`), while the contract is taken.
    Extract { exit: Option<usize> },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Reward {
    pub gold: u32,
    /// Stations unlocked, by index.
    pub unlocks: Vec<usize>,
}

/// A contract (spec §8.5): a route and a reason from one of Duskwood's characters.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractDef {
    pub key: String,
    /// The NPC who gives it, by index.
    pub giver: usize,
    pub name: String,
    pub text: String,
    pub objectives: Vec<Objective>,
    pub reward: Reward,
    /// Contracts finished before this one is offered, by index.
    pub needs: Vec<usize>,
    pub repeatable: bool,
}

/// What a station's level gives.
#[derive(Clone, Debug, PartialEq)]
pub enum Gift {
    /// More stash room.
    StashSlots(u16),
    /// A recipe, by index.
    Recipe(usize),
    /// The map table's look at the next raid.
    Intel,
    /// A companion, by index.
    Companion(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StationLevel {
    pub gold: u32,
    /// Materials, from the stash: (item, count).
    pub items: Vec<(usize, u32)>,
    pub gives: Vec<Gift>,
}

/// A station in the inn (spec §8.5): upgraded level by level, kept forever.
#[derive(Clone, Debug, PartialEq)]
pub struct StationDef {
    pub key: String,
    pub name: String,
    /// Its keeper, by NPC index.
    pub keeper: usize,
    /// Upgradable from the start, not only once a contract unlocks it.
    pub open: bool,
    pub levels: Vec<StationLevel>,
}

/// Something a station makes.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeDef {
    pub key: String,
    /// Where it is made, by station index.
    pub station: usize,
    /// (item, count).
    pub makes: (usize, u16),
    pub needs: Vec<(usize, u32)>,
    pub gold: u32,
}

/// The inn, resolved.
#[derive(Clone, Debug)]
pub struct InnContent {
    pub place: HubPlace,
    pub npcs: Vec<NpcDef>,
    /// How many contracts a player may have taken at once.
    pub contracts_at_once: u8,
    pub trader: TraderDef,
    pub companions: Vec<CompanionDef>,
    pub contracts: Vec<ContractDef>,
    pub stations: Vec<StationDef>,
    pub recipes: Vec<RecipeDef>,
}

/// The text of each of the inn's files.
pub struct InnSources<'a> {
    pub hub: &'a str,
    pub contracts: &'a str,
    pub stations: &'a str,
}

#[derive(Deserialize)]
struct DoorFile {
    at: (f32, f32),
    radius: f32,
}

#[derive(Deserialize)]
struct PlaceFile {
    name: String,
    map: u32,
    spawn: (f32, f32, f32),
    #[serde(default)]
    facing: f32,
    bounds: Vec<(f32, f32)>,
    door: DoorFile,
    minute: u32,
    view_distance: f32,
}

#[derive(Deserialize)]
enum RoleFile {
    Banker,
    Trader,
    Contracts,
    Station(String),
}

#[derive(Deserialize)]
struct NpcFile {
    key: String,
    name: String,
    #[serde(default)]
    title: String,
    role: RoleFile,
    body: Body,
    at: (f32, f32, f32),
    #[serde(default)]
    facing: f32,
}

#[derive(Deserialize)]
struct StockFile {
    item: String,
    price: u32,
}

#[derive(Deserialize)]
struct TraderFile {
    stock: Vec<StockFile>,
    sells_for: f32,
}

#[derive(Deserialize)]
struct HubFile {
    place: PlaceFile,
    npcs: Vec<NpcFile>,
    contracts_at_once: u8,
    trader: TraderFile,
    #[serde(default)]
    companions: Vec<CompanionDef>,
}

#[derive(Deserialize)]
enum ObjectiveFile {
    Kill {
        enemy: String,
        count: u32,
        #[serde(default)]
        near: Option<(f32, f32, f32)>,
    },
    Bring {
        item: String,
        count: u32,
    },
    Extract {
        #[serde(default)]
        exit: Option<String>,
    },
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RewardFile {
    gold: u32,
    unlocks: Vec<String>,
}

#[derive(Deserialize)]
struct ContractFile {
    key: String,
    giver: String,
    name: String,
    #[serde(default)]
    text: String,
    objectives: Vec<ObjectiveFile>,
    #[serde(default)]
    reward: RewardFile,
    #[serde(default)]
    needs: Vec<String>,
    #[serde(default)]
    repeatable: bool,
}

#[derive(Deserialize)]
enum GiftFile {
    StashSlots(u16),
    Recipe(String),
    Intel,
    Companion(String),
}

#[derive(Deserialize)]
struct LevelFile {
    gold: u32,
    #[serde(default)]
    items: Vec<(String, u32)>,
    gives: Vec<GiftFile>,
}

#[derive(Deserialize)]
struct StationFile {
    key: String,
    name: String,
    keeper: String,
    #[serde(default)]
    open: bool,
    levels: Vec<LevelFile>,
}

#[derive(Deserialize)]
struct RecipeFile {
    key: String,
    station: String,
    makes: (String, u16),
    needs: Vec<(String, u32)>,
    #[serde(default)]
    gold: u32,
}

#[derive(Deserialize)]
struct StationsFile {
    stations: Vec<StationFile>,
    #[serde(default)]
    recipes: Vec<RecipeFile>,
}

fn parse<T: for<'de> Deserialize<'de>>(file: &'static str, text: &str) -> Result<T, ContentError> {
    ron::from_str(text).map_err(|e| ContentError::Parse {
        file,
        error: e.to_string(),
    })
}

/// The first key listed twice, if any.
fn twice<'a>(keys: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen: Vec<&str> = Vec::new();
    for k in keys {
        if seen.contains(&k) {
            return Some(k);
        }
        seen.push(k);
    }
    None
}

impl InnContent {
    pub fn from_ron(src: InnSources, content: &Content) -> Result<Self, ContentError> {
        let hub: HubFile = parse("hub.ron", src.hub)?;
        let contracts: Vec<ContractFile> = parse("contracts.ron", src.contracts)?;
        let stations: StationsFile = parse("stations.ron", src.stations)?;
        let bad = |why: String| ContentError::Invalid(why);
        let item = |file: &str, key: &str| {
            content
                .item(key)
                .ok_or_else(|| bad(format!("{file}: no item \"{key}\" in items.ron")))
        };

        let p = &hub.place;
        let bounds = Bounds::new(p.bounds.iter().map(|&(x, y)| V2::new(x, y)).collect())
            .map_err(|why| bad(format!("hub.ron: {why}")))?;
        let spawn = V3::new(p.spawn.0, p.spawn.1, p.spawn.2);
        let door = V2::new(p.door.at.0, p.door.at.1);
        if !bounds.contains(spawn.ground()) || !bounds.contains(door) {
            return Err(bad("hub.ron: the spawn and the door are inside the bounds".into()));
        }
        if p.minute >= 1440 || !(p.door.radius > 0.0) || !(p.view_distance > 0.0) {
            return Err(bad(
                "hub.ron: minute below 1440, a door radius and a view distance above 0".into(),
            ));
        }
        let place = HubPlace {
            name: p.name.clone(),
            map: p.map,
            spawn,
            facing: p.facing,
            bounds,
            door,
            door_radius: p.door.radius,
            minute: p.minute,
            view_distance: p.view_distance,
        };

        let names = |v: &[&str]| v.join(", ");
        if let Some(k) = twice(hub.npcs.iter().map(|n| n.key.as_str())) {
            return Err(bad(format!("hub.ron: NPC \"{k}\" is listed twice")));
        }
        if let Some(k) = twice(stations.stations.iter().map(|s| s.key.as_str())) {
            return Err(bad(format!("stations.ron: station \"{k}\" is listed twice")));
        }
        if let Some(k) = twice(stations.recipes.iter().map(|r| r.key.as_str())) {
            return Err(bad(format!("stations.ron: recipe \"{k}\" is listed twice")));
        }
        if let Some(k) = twice(contracts.iter().map(|c| c.key.as_str())) {
            return Err(bad(format!("contracts.ron: contract \"{k}\" is listed twice")));
        }
        if let Some(k) = twice(hub.companions.iter().map(|c| c.key.as_str())) {
            return Err(bad(format!("hub.ron: companion \"{k}\" is listed twice")));
        }
        let station_of =
            |key: &str| stations.stations.iter().position(|s| s.key == key);
        let mut npcs = Vec::new();
        for n in &hub.npcs {
            let at = V3::new(n.at.0, n.at.1, n.at.2);
            if !place.bounds.contains(at.ground()) {
                return Err(bad(format!("hub.ron: \"{}\" stands outside the bounds", n.key)));
            }
            let role = match &n.role {
                RoleFile::Banker => NpcRole::Banker,
                RoleFile::Trader => NpcRole::Trader,
                RoleFile::Contracts => NpcRole::Contracts,
                RoleFile::Station(k) => NpcRole::Station(station_of(k).ok_or_else(|| {
                    bad(format!("hub.ron: \"{}\" keeps no station \"{k}\"", n.key))
                })?),
            };
            npcs.push(NpcDef {
                key: n.key.clone(),
                name: n.name.clone(),
                title: n.title.clone(),
                role,
                body: n.body,
                at,
                facing: n.facing,
            });
        }
        if npcs.iter().filter(|n| n.role == NpcRole::Banker).count() != 1 {
            return Err(bad("hub.ron: one NPC is the banker".into()));
        }
        if npcs.iter().filter(|n| n.role == NpcRole::Trader).count() > 1 {
            return Err(bad("hub.ron: at most one NPC is the trader".into()));
        }
        let npc_of = |file: &str, key: &str| {
            npcs.iter()
                .position(|n| n.key == key)
                .ok_or_else(|| bad(format!("{file}: no NPC \"{key}\" in hub.ron")))
        };

        let mut stock = Vec::new();
        for s in &hub.trader.stock {
            if s.price == 0 {
                return Err(bad(format!("hub.ron: \"{}\" sells for at least 1", s.item)));
            }
            stock.push((item("hub.ron", &s.item)?, s.price));
        }
        if !(hub.trader.sells_for >= 0.0 && hub.trader.sells_for <= 1.0) {
            return Err(bad("hub.ron: the trader pays a share of value, 0 to 1".into()));
        }
        for c in &hub.companions {
            if !(c.scale > 0.0) {
                return Err(bad(format!("hub.ron: companion \"{}\" has a size", c.key)));
            }
        }
        let companion_of = |key: &str| {
            hub.companions
                .iter()
                .position(|c| c.key == key)
                .ok_or_else(|| bad(format!("stations.ron: no companion \"{key}\" in hub.ron")))
        };
        let recipe_of = |key: &str| {
            stations
                .recipes
                .iter()
                .position(|r| r.key == key)
                .ok_or_else(|| bad(format!("stations.ron: no recipe \"{key}\"")))
        };

        let mut station_defs = Vec::new();
        for (si, s) in stations.stations.iter().enumerate() {
            let keeper = npc_of("stations.ron", &s.keeper)?;
            let keeps = npcs[keeper].role == NpcRole::Station(si)
                || (npcs[keeper].role == NpcRole::Banker && s.open);
            if !keeps {
                return Err(bad(format!(
                    "stations.ron: \"{}\"'s keeper \"{}\" must keep it (or be the banker, for a station open from the start)",
                    s.key, s.keeper
                )));
            }
            if s.levels.is_empty() {
                return Err(bad(format!("stations.ron: \"{}\" has a level", s.key)));
            }
            let mut levels = Vec::new();
            for l in &s.levels {
                let mut items = Vec::new();
                for (k, n) in &l.items {
                    if *n == 0 {
                        return Err(bad(format!("stations.ron: \"{}\": a count is 1 or more", s.key)));
                    }
                    items.push((item("stations.ron", k)?, *n));
                }
                let mut gives = Vec::new();
                for g in &l.gives {
                    gives.push(match g {
                        GiftFile::StashSlots(n) => Gift::StashSlots(*n),
                        GiftFile::Recipe(k) => Gift::Recipe(recipe_of(k)?),
                        GiftFile::Intel => Gift::Intel,
                        GiftFile::Companion(k) => Gift::Companion(companion_of(k)?),
                    });
                }
                levels.push(StationLevel {
                    gold: l.gold,
                    items,
                    gives,
                });
            }
            station_defs.push(StationDef {
                key: s.key.clone(),
                name: s.name.clone(),
                keeper,
                open: s.open,
                levels,
            });
        }

        let mut recipes = Vec::new();
        for r in &stations.recipes {
            let station = station_of(&r.station)
                .ok_or_else(|| bad(format!("stations.ron: no station \"{}\"", r.station)))?;
            if r.makes.1 == 0 {
                return Err(bad(format!("stations.ron: recipe \"{}\" makes 1 or more", r.key)));
            }
            let mut needs = Vec::new();
            for (k, n) in &r.needs {
                needs.push((item("stations.ron", k)?, *n));
            }
            recipes.push(RecipeDef {
                key: r.key.clone(),
                station,
                makes: (item("stations.ron", &r.makes.0)?, r.makes.1),
                needs,
                gold: r.gold,
            });
        }
        // A recipe is made where it is learnt.
        for (si, s) in station_defs.iter().enumerate() {
            for g in s.levels.iter().flat_map(|l| &l.gives) {
                if let Gift::Recipe(r) = g {
                    if recipes[*r].station != si {
                        return Err(bad(format!(
                            "stations.ron: \"{}\" teaches \"{}\", which is made at another station",
                            s.key, recipes[*r].key
                        )));
                    }
                }
            }
        }

        let mut contract_defs = Vec::new();
        for c in &contracts {
            let giver = npc_of("contracts.ron", &c.giver)?;
            if npcs[giver].role != NpcRole::Contracts {
                return Err(bad(format!(
                    "contracts.ron: \"{}\"'s giver \"{}\" gives no contracts",
                    c.key, c.giver
                )));
            }
            if c.objectives.is_empty() {
                return Err(bad(format!("contracts.ron: \"{}\" asks for something", c.key)));
            }
            let mut objectives = Vec::new();
            for o in &c.objectives {
                objectives.push(match o {
                    ObjectiveFile::Kill { enemy, count, near } => {
                        if *count == 0 {
                            return Err(bad(format!("contracts.ron: \"{}\": a count is 1 or more", c.key)));
                        }
                        Objective::Kill {
                            enemy: content.enemy(enemy).ok_or_else(|| {
                                bad(format!("contracts.ron: no enemy \"{enemy}\" in enemies.ron"))
                            })?,
                            count: *count,
                            near: near.map(|(x, y, r)| (V2::new(x, y), r)),
                        }
                    }
                    ObjectiveFile::Bring { item: k, count } => {
                        if *count == 0 {
                            return Err(bad(format!("contracts.ron: \"{}\": a count is 1 or more", c.key)));
                        }
                        Objective::Bring {
                            item: item("contracts.ron", k)?,
                            count: *count,
                        }
                    }
                    ObjectiveFile::Extract { exit } => Objective::Extract {
                        exit: match exit {
                            None => None,
                            Some(k) => Some(
                                content.map.exits.iter().position(|e| e.key == *k).ok_or_else(
                                    || bad(format!("contracts.ron: no exit \"{k}\" in map.ron")),
                                )?,
                            ),
                        },
                    },
                });
            }
            let mut unlocks = Vec::new();
            for k in &c.reward.unlocks {
                unlocks.push(
                    station_of(k)
                        .ok_or_else(|| bad(format!("contracts.ron: no station \"{k}\"")))?,
                );
            }
            contract_defs.push(ContractDef {
                key: c.key.clone(),
                giver,
                name: c.name.clone(),
                text: c.text.clone(),
                objectives,
                reward: Reward {
                    gold: c.reward.gold,
                    unlocks,
                },
                needs: Vec::new(),
                repeatable: c.repeatable,
            });
        }
        for (i, c) in contracts.iter().enumerate() {
            for k in &c.needs {
                let n = contracts.iter().position(|o| o.key == *k).ok_or_else(|| {
                    bad(format!(
                        "contracts.ron: \"{}\" needs \"{k}\", which is not a contract ({})",
                        c.key,
                        names(&contracts.iter().map(|o| o.key.as_str()).collect::<Vec<_>>())
                    ))
                })?;
                contract_defs[i].needs.push(n);
            }
        }

        Ok(Self {
            place,
            npcs,
            contracts_at_once: hub.contracts_at_once.max(1),
            trader: TraderDef {
                stock,
                sells_for: hub.trader.sells_for,
            },
            companions: hub.companions,
            contracts: contract_defs,
            stations: station_defs,
            recipes,
        })
    }

    pub fn npc(&self, key: &str) -> Option<usize> {
        self.npcs.iter().position(|n| n.key == key)
    }

    pub fn contract(&self, key: &str) -> Option<usize> {
        self.contracts.iter().position(|c| c.key == key)
    }

    pub fn station(&self, key: &str) -> Option<usize> {
        self.stations.iter().position(|s| s.key == key)
    }

    pub fn recipe(&self, key: &str) -> Option<usize> {
        self.recipes.iter().position(|r| r.key == key)
    }

    pub fn companion(&self, key: &str) -> Option<usize> {
        self.companions.iter().position(|c| c.key == key)
    }

    /// The banker, by NPC index (the content has exactly one).
    pub fn banker(&self) -> usize {
        self.npcs
            .iter()
            .position(|n| n.role == NpcRole::Banker)
            .expect("checked as it loaded")
    }
}
```

`Content::enemy(&self, key) -> Option<usize>` is the world-knowledge plan's (Task 5).

Run: `cargo test -p nightfall-sim inn`
Expected: PASS (4 tests).

- [ ] **Step 3: Write the shipped inn**

`content/nightfall/hub.ron`:

```ron
// The inn between raids (spec §8.4): Goldshire's Lion's Pride Inn, on map 0 (Elwynn). Everything
// about the place is here and only here: where heroes appear, the rooms they are kept inside, the
// door whose ring opens the raid board, the evening's light, and where each NPC stands. Moving the
// hub is a content edit: the Scarlet Raven Tavern block at the end of this file is Darkshire's.
//
// measure: every position and the bounds, in the game (NIGHTFALL_TRACE_POS=1 logs where the hero
// stands); the numbers below are the inn's ground floor as far as known.
//
// NPCs are characters of a race and sex in a class's starting outfit (`body`), so they always
// look like someone; `role` is what a right-click opens: Banker (the stash), Trader, Contracts
// (the giver's contracts), Station("key") (that station). `facing` is degrees from north toward
// west.
(
    place: (
        name: "The Lion's Pride Inn",
        map: 0,
        spawn: (-9459.0, 22.0, 57.6),
        facing: 90.0,
        bounds: [(-9474.0, 6.0), (-9445.0, 6.0), (-9445.0, 38.0), (-9474.0, 38.0)],
        door: (at: (-9447.5, 19.0), radius: 2.0),
        // 19:30, a warm evening, against the night outside.
        minute: 1170,
        view_distance: 250.0,
    ),
    npcs: [
        (key: "farley", name: "Innkeeper Farley", title: "Keeps your things",
         role: Banker, body: (race: 1, sex: 0, class_id: 1), at: (-9462.7, 16.2, 57.0), facing: 90.0),
        (key: "brog", name: "Brog Hamfist", title: "Trades in anything",
         role: Trader, body: (race: 3, sex: 0, class_id: 1), at: (-9467.0, 30.0, 57.0), facing: 0.0),
        (key: "ebonlock", name: "Commander Althea Ebonlock", title: "The Night Watch",
         role: Contracts, body: (race: 1, sex: 1, class_id: 2), at: (-9452.0, 30.0, 57.0), facing: 180.0),
        (key: "eva", name: "Madame Eva", title: "Sees what waits in the dark",
         role: Contracts, body: (race: 1, sex: 1, class_id: 8), at: (-9470.0, 20.0, 57.0), facing: 270.0),
        (key: "abercrombie", name: "Abercrombie", title: "Wants what the dead leave",
         role: Contracts, body: (race: 1, sex: 0, class_id: 9), at: (-9470.0, 12.0, 57.0), facing: 270.0),
        (key: "smith", name: "Argus the Smith", title: "Workbench",
         role: Station("workbench"), body: (race: 3, sex: 0, class_id: 1), at: (-9449.0, 9.0, 57.0)),
        (key: "alchemist", name: "Tilda Brew", title: "Alchemy",
         role: Station("alchemy"), body: (race: 7, sex: 1, class_id: 8), at: (-9455.0, 9.0, 57.0)),
        (key: "cook", name: "Old Ruth", title: "The cooking fire",
         role: Station("cooking"), body: (race: 1, sex: 1, class_id: 5), at: (-9460.0, 34.0, 57.0)),
        (key: "lamplighter", name: "Wick", title: "Lanterns",
         role: Station("lantern_maker"), body: (race: 7, sex: 0, class_id: 9), at: (-9466.0, 9.0, 57.0)),
        (key: "cartographer", name: "Mara Lines", title: "The map table",
         role: Station("map_table"), body: (race: 1, sex: 1, class_id: 4), at: (-9452.0, 24.0, 57.0)),
        (key: "kennelmaster", name: "Hobb", title: "The kennel",
         role: Station("kennel"), body: (race: 3, sex: 0, class_id: 3), at: (-9449.0, 34.0, 57.0)),
    ],
    // Contracts a player may have taken at once (spec §8.5: one or two).
    contracts_at_once: 2,
    // What the trader sells, gold each; and the share of an item's value they pay for it.
    trader: (
        stock: [
            (item: "tough_bread", price: 2),
            (item: "spring_water", price: 2),
            (item: "minor_healing_potion", price: 15),
            (item: "minor_mana_potion", price: 15),
            (item: "linen_pouch", price: 12),
            (item: "hooded_lantern", price: 25),
        ],
        sells_for: 1.0,
    ),
    // The kennel's companions (spec §8.2). measure: a hound's CreatureDisplayInfo id (a Stormwind
    // guard dog); the scale makes it small.
    companions: [
        (key: "watch_wolf", name: "Watch Wolf", display: 903, scale: 0.6),  // as built (Task 1 rulings)
    ],
)

// Darkshire's Scarlet Raven Tavern instead (measure: every number): replace `place` above with
//     place: (
//         name: "The Scarlet Raven Tavern",
//         map: 0,
//         spawn: (-10516.0, -1163.0, 41.3),
//         facing: 0.0,
//         bounds: [(-10528.0, -1175.0), (-10504.0, -1175.0), (-10504.0, -1150.0), (-10528.0, -1150.0)],
//         door: (at: (-10506.0, -1162.0), radius: 2.0),
//         minute: 1170,
//         view_distance: 250.0,
//     ),
// and move each NPC's `at` inside those bounds.
```

`content/nightfall/contracts.ron`:

```ron
// Contracts (spec §8.5): Duskwood's characters give them in the inn, one or two taken at a time.
// Objectives: Kill(enemy, count, near: Some((x, y, radius))) over any number of raids; Bring(item,
// count) to the giver, taken from the stash and loadout as it is turned in; Extract(exit) out
// alive by that exit (any for None) while it is taken. Rewards pay gold and unlock stations.
[
    (key: "brightwood_pack", giver: "ebonlock", name: "Thin the Brightwood pack",
     text: "The Nightbane hunt the Brightwood road. Kill four of them there, and the Watch will pay, and lend the inn its smith.",
     // measure: Brightwood Grove's centre and reach.
     objectives: [Kill(enemy: "nightbane_worgen", count: 4, near: Some((-10280.0, -1250.0, 150.0)))],
     reward: (gold: 60, unlocks: ["workbench"]), repeatable: true),
    (key: "raven_hill_remains", giver: "abercrombie", name: "Remains from Raven Hill",
     text: "Bring me bones from Raven Hill. Ten will do. Do not ask what for.",
     objectives: [Bring(item: "bone_fragments", count: 10)],
     reward: (gold: 40, unlocks: ["alchemy", "cooking"]), repeatable: true),
    (key: "ladims_tome", giver: "eva", name: "The tome in Mor'Ladim's grave",
     text: "Mor'Ladim was buried with a book. Dig it up, and get out by the Westfall road before he notices.",
     needs: ["raven_hill_remains"],
     objectives: [Bring(item: "ladims_tome", count: 1), Extract(exit: Some("westfall_road"))],
     reward: (gold: 120, unlocks: ["lantern_maker", "map_table"])),
    (key: "the_watchs_hound", giver: "ebonlock", name: "A hound for the Watch",
     text: "Take four pelts off the worgen and come back alive. Hobb will give you one of his hounds.",
     needs: ["brightwood_pack"],
     objectives: [Bring(item: "worgen_pelt", count: 4), Extract(exit: None)],
     reward: (gold: 50, unlocks: ["kennel"])),
    // Stitches walks the road once the night's clock reaches its last stage (sub-project 6);
    // this contract waits for him in enemies.ron:
    // (key: "stitches", giver: "ebonlock", name: "Stitches",
    //  text: "It walks from Raven Hill to Darkshire. Stop it.",
    //  objectives: [Kill(enemy: "stitches", count: 1)],
    //  reward: (gold: 500)),
]
```

`content/nightfall/stations.ron`:

```ron
// The inn's stations (spec §8.5): each upgraded level by level with gold and materials from the
// stash, kept forever. A level gives StashSlots(n), Recipe("key") (made at that station), Intel
// (the map table's look at the next raid) or Companion("key") (hub.ron's companions). `open`
// stations can be upgraded from the start; the others once a contract unlocks them. A station's
// keeper is the NPC whose role is Station(its key); the stash's is the banker.
(
    stations: [
        (key: "stash", name: "The Stash", keeper: "farley", open: true, levels: [
            (gold: 50, items: [("linen_cloth", 10)], gives: [StashSlots(20)]),
            (gold: 150, items: [("iron_scrap", 10), ("linen_cloth", 10)], gives: [StashSlots(30)]),
        ]),
        (key: "workbench", name: "Workbench", keeper: "smith", levels: [
            (gold: 40, items: [("iron_scrap", 5)],
             gives: [Recipe("linen_pouch"), Recipe("scavengers_leggings")]),
            (gold: 120, items: [("worgen_pelt", 6), ("iron_scrap", 8)],
             gives: [Recipe("duskwood_satchel"), Recipe("watchmans_tunic")]),
        ]),
        (key: "alchemy", name: "Alchemy", keeper: "alchemist", levels: [
            (gold: 40, items: [("bone_fragments", 5)], gives: [Recipe("minor_healing_potion")]),
            (gold: 90, items: [("spider_silk", 8)], gives: [Recipe("minor_mana_potion")]),
        ]),
        (key: "cooking", name: "The cooking fire", keeper: "cook", levels: [
            (gold: 25, items: [("linen_cloth", 4)], gives: [Recipe("smoked_venison")]),
        ]),
        (key: "lantern_maker", name: "Lanterns", keeper: "lamplighter", levels: [
            (gold: 60, items: [("iron_scrap", 6)], gives: [Recipe("shuttered_lantern")]),
            (gold: 140, items: [("iron_scrap", 10), ("spider_silk", 6)], gives: [Recipe("bright_lantern")]),
        ]),
        (key: "map_table", name: "The map table", keeper: "cartographer", levels: [
            (gold: 80, items: [("linen_cloth", 6)], gives: [Intel]),
        ]),
        (key: "kennel", name: "The kennel", keeper: "kennelmaster", levels: [
            (gold: 100, items: [("worgen_pelt", 2)], gives: [Companion("watch_wolf")]),
        ]),
    ],
    recipes: [
        (key: "linen_pouch", station: "workbench", makes: ("linen_pouch", 1), needs: [("linen_cloth", 6)], gold: 3),
        (key: "scavengers_leggings", station: "workbench", makes: ("scavengers_leggings", 1), needs: [("linen_cloth", 4), ("worgen_pelt", 2)], gold: 10),
        (key: "duskwood_satchel", station: "workbench", makes: ("duskwood_satchel", 1), needs: [("linen_cloth", 8), ("worgen_pelt", 3)], gold: 20),
        (key: "watchmans_tunic", station: "workbench", makes: ("watchmans_tunic", 1), needs: [("iron_scrap", 8), ("worgen_pelt", 2)], gold: 30),
        (key: "minor_healing_potion", station: "alchemy", makes: ("minor_healing_potion", 2), needs: [("bone_fragments", 3), ("spider_silk", 1)], gold: 4),
        (key: "minor_mana_potion", station: "alchemy", makes: ("minor_mana_potion", 2), needs: [("spider_silk", 3)], gold: 4),
        (key: "smoked_venison", station: "cooking", makes: ("smoked_venison", 4), needs: [("worgen_pelt", 1)], gold: 1),
        (key: "shuttered_lantern", station: "lantern_maker", makes: ("shuttered_lantern", 1), needs: [("iron_scrap", 3), ("linen_cloth", 2)], gold: 10),
        (key: "bright_lantern", station: "lantern_maker", makes: ("bright_lantern", 1), needs: [("iron_scrap", 4), ("spider_silk", 2)], gold: 20),
    ],
)
```

In `content/nightfall/items.ron`, before the closing `]`:

```ron
    // Contract items and the lantern-maker's lanterns (sub-project 5b).
    (key: "ladims_tome", name: "Mor'Ladim's Tome", kind: Valuable, icon: "INV_Misc_Book_09",
     quality: 3, weight: 1.5, text: "Madame Eva wants it. It is cold to the touch."),
    (key: "shuttered_lantern", name: "Shuttered Lantern", kind: Lantern, icon: "INV_Misc_Lantern_01",
     quality: 2, weight: 1.5, value: 20, light: Some((range: 13.0, intensity: 1.0))),
    (key: "bright_lantern", name: "Bright Lantern", kind: Lantern, icon: "INV_Misc_Lantern_01",
     quality: 2, weight: 2.0, value: 35, light: Some((range: 26.0, intensity: 1.8))),
```

(Task 2 gives the shuttered lantern its quieter light.)

In `content/nightfall/loot.ron`, `tables` gains `(key: "ladims_grave", rolls: (1, 1), items: [(item: "ladims_tome", weight: 1.0)]),` and `containers` gains:

```ron
        // measure: Mor'Ladim's grave in Raven Hill cemetery (Madame Eva's contract).
        (kind: "grave", table: "ladims_grave", at: (-10740.0, 330.0), place: "Mor'Ladim's grave"),
```

In `tests/shipped_content.rs` add:

```rust
#[test]
fn the_shipped_inn_loads_against_the_shipped_raid() {
    use nightfall_sim::{InnContent, InnSources};
    let c = shipped();
    let inn = InnContent::from_ron(
        InnSources {
            hub: include_str!("../../../content/nightfall/hub.ron"),
            contracts: include_str!("../../../content/nightfall/contracts.ron"),
            stations: include_str!("../../../content/nightfall/stations.ron"),
        },
        &c,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(inn.place.map, 0);
    assert!(inn.contracts.len() >= 3);
    assert!(inn.stations.iter().any(|s| s.open));
    assert!(!inn.companions.is_empty());
    assert!(inn.trader.stock.iter().all(|&(i, p)| i < c.items.len() && p > 0));
}
```

- [ ] **Step 4: The client reads the inn**

In `crates/benilla-app/src/nightfall/mod.rs`:

- `CONTENT_TEXTS` and `content_texts()` hold `[String; 11]`: the three inn files after `loot`.
- Add `static INN: OnceLock<Arc<InnContent>> = OnceLock::new();` and:

```rust
/// The inn's content, as [`prepare`] loaded it.
pub(super) fn inn() -> Arc<InnContent> {
    INN.get().expect("nightfall::prepare ran").clone()
}
```

- In `prepare`, after the raid's `Content` is made, read `hub.ron`, `contracts.ron` and `stations.ron` the same way (`read(name, include_str!("../../../../content/nightfall/<name>"))`), and:

```rust
    let inn = InnContent::from_ron(
        InnSources {
            hub: &hub,
            contracts: &contracts,
            stations: &stations,
        },
        &content,
    )
    .map_err(|e| e.to_string())?;
    let _ = INN.set(Arc::new(inn));
```

with `InnContent, InnSources` added to its `nightfall_sim` import; the `CONTENT_TEXTS.set([...])` array gains `hub, contracts, stations` at its end (the content hash covers them: the companion's index travels on the wire).

- [ ] **Step 5: Run the tests and check the client**

Run: `cargo test -p nightfall-sim`
Expected: PASS.
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim content/nightfall crates/benilla-app/src/nightfall/mod.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the inn as content: the hub's place, NPCs, trader, companions, contracts, stations

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 2: The companion's slot, lanterns that set their own visibility, kills by kind and place

**Files:**
- Modify: `crates/nightfall-sim/src/inventory.rs` (`Inventory::pet`, `Place::Pet`), `crates/nightfall-sim/src/items.rs` (`LightDef::seen`), `crates/nightfall-sim/src/raid/mod.rs` (`Loadout::pet`, `Carry::pet`, `HeroOutcome::slain`, `Raid::slain`, `give_loadout`), `crates/nightfall-sim/src/raid/extract.rs` (`tally_kills`, `outcome`), `crates/nightfall-sim/src/senses/mod.rs` (`step_heroes`), `crates/nightfall-sim/src/profile.rs` (`saved_inventory`'s match), `crates/nightfall-sim/src/raid/tests.rs`
- Modify (exhaustive matches and literals): `crates/benilla-app/src/nightfall/backpack.rs` (`smart_act`), every `HeroOutcome { … }` literal (`messages.rs`, `profile.rs` and `end_screen.rs` tests), `content/nightfall/items.ron` (the shuttered lantern's light)

**Interfaces:**
- Consumes: `Inventory`, `Place`, `Stack`, `split_for_death`, `sanitize`, `weight`, `clear_found` (5a Task 2); `Loadout`, `Carry`, `HeroOutcome`, `give_loadout`, `tally_kills`, `outcome`, `raid::tests::*` (5a Tasks 3, 6); `LightDef`, `check_items` (5a Task 1); `senses::step_heroes`, `visibility`, `SenseRules::lantern` (senses plan); `Event::EnemyDied { id, by }`, `Enemy::{id, kind, pos}`.
- Produces:
  - `Inventory::pet: Option<Option<Stack>>` (`None` without a companion); `Place::Pet`. The companion's stack is never lost (no death mode, no missing raid takes it) and never weighed.
  - `Loadout::pet: Option<u8>` and `Carry::pet: Option<u8>` (the companion's kind, an index into `InnContent::companions`); `give_loadout` gives the slot with the companion and takes a pet's stack out (returned) without one.
  - `LightDef::seen: Option<f32>` (at least 1): a lit lantern's own visibility factor, in place of `SenseRules::lantern`.
  - `HeroOutcome::slain: Vec<(u16, V2)>` (each kill's enemy kind and where it died); crate-only `Raid::{slain, add_slain, slain_of}`.

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/inventory.rs`'s tests, add:

```rust
    #[test]
    fn the_companion_keeps_its_stack_through_every_death() {
        let c = content("");
        for mode in [
            DeathLoss::Everything,
            DeathLoss::EverythingOneRecovery,
            DeathLoss::FoundOnly,
        ] {
            let mut inv = Inventory::new(&c);
            inv.pet = Some(Some(Stack::found(GEM, 4)));
            inv.add(Stack::found(BREAD, 1), &c);
            let lost = inv.split_for_death(mode, &c);
            assert_eq!(inv.get(Place::Pet), Some(&Stack::found(GEM, 4)), "{mode:?}");
            assert!(lost.iter().all(|s| s.item != GEM as u16));
        }
        let mut inv = Inventory::new(&c);
        inv.pet = Some(Some(Stack::new(GEM, 10)));
        assert_eq!(inv.weight(&c), 0.0, "the companion carries it, not the hero");
        inv.add(Stack::new(BREAD, 2), &c);
        inv.move_to(Place::Pack(0), Place::Pet, &c).unwrap();
        assert_eq!(inv.get(Place::Pet), Some(&Stack::new(BREAD, 2)));
        assert_eq!(inv.get(Place::Pack(0)), Some(&Stack::new(GEM, 10)));
        assert!(inv.sanitize(&c).is_empty());
        assert_eq!(inv.get(Place::Pet), Some(&Stack::new(BREAD, 2)));
        let mut none = Inventory::new(&c);
        none.add(Stack::new(BREAD, 1), &c);
        assert_eq!(none.move_to(Place::Pack(0), Place::Pet, &c), Err(InvError::NoSuchPlace));
    }
```

Append to `crates/nightfall-sim/src/raid/tests.rs`'s `ITEMS` (before its closing `]`):

```rust
    (key: "shutter", name: "Shuttered Lantern", kind: Lantern, icon: "a", weight: 1.0,
     light: Some((range: 10.0, intensity: 1.0, seen: Some(1.2)))),
```

and add:

```rust
#[test]
fn the_companion_comes_into_the_raid_and_out_of_any_death() {
    let mut w = world_with("death_loss: everything", "");
    join(&mut w, 0, "rogue", 20.0, 20.0);
    let mut lo = loadout(&w, &[("bread", 1)], &[], &[]);
    lo.pet = Some(0);
    lo.inventory.pet = Some(Some(Stack::found(item(&w, "gem"), 3)));
    w.give_loadout(0, &lo);
    assert_eq!(w.hero(0).unwrap().carry.pet, Some(0));
    assert!((w.hero(0).unwrap().carry.load - 0.5).abs() < 1e-5);
    die(&mut w, 0);
    let out = w.take_outcomes();
    assert_eq!(out[0].carried.get(Place::Pet), Some(&Stack::new(item(&w, "gem"), 3)));
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(body, vec![Stack::new(item(&w, "bread"), 1)]);
    // Without the companion, a stack in its slot comes back as left over, not into the raid.
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    let mut lo = loadout(&w, &[], &[], &[]);
    lo.inventory.pet = Some(Some(Stack::new(item(&w, "gem"), 3)));
    assert_eq!(w.give_loadout(0, &lo), vec![Stack::new(item(&w, "gem"), 3)]);
    assert_eq!(w.pack(0).unwrap().pet, None);
}

#[test]
fn a_lantern_sets_how_easy_it_makes_its_hero_to_see() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1), ("shutter", 1)], &[], &[]));
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    steps(&mut w, 2);
    // Standing still: 35, times the rules' lantern 1.6.
    assert!((w.hero(0).unwrap().visibility - 56.0).abs() < 1e-3);
    act(&mut w, 0, 2, Act::Use(Place::Pack(1)));
    steps(&mut w, 2);
    assert!((w.hero(0).unwrap().visibility - 42.0).abs() < 1e-3);
}

#[test]
fn a_kill_says_what_and_where() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    w.kill_enemy(0, Some(0));
    w.step();
    w.abandon();
    assert_eq!(w.take_outcomes()[0].slain, vec![(0, V2::new(5.0, 55.0))]);
}
```

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`no field pet`, `no variant Pet`, `no field slain`).

- [ ] **Step 2: The companion's slot**

In `crates/nightfall-sim/src/inventory.rs`:

- `Place` gains:

```rust
    /// The companion's one stack (spec §8.2).
    Pet,
```

- `Inventory` gains:

```rust
    /// The companion's one stack: `None` without a companion. Never lost, never weighed: the
    /// companion carries it.
    pub pet: Option<Option<Stack>>,
```

and `Inventory::new`'s literal gains `pet: None,`.
- `get` gains `Place::Pet => self.pet.as_ref()?.as_ref(),`; `slot` gains `Place::Pet => self.pet.as_mut(),`; in `fits`, `Place::Pack(_) | Place::Safe(_) => true,` becomes `Place::Pack(_) | Place::Safe(_) | Place::Pet => true,`.
- `stacks` gains, after the safe slots:

```rust
        if let Some(Some(st)) = self.pet {
            out.push((Place::Pet, st));
        }
```

- `weight` skips it: its `.iter()` becomes `.iter().filter(|(p, _)| *p != Place::Pet)`.
- `clear_found`'s chain gains `.chain(self.pet.iter_mut())` before `.flatten()`.
- In `sanitize`, before `let pack: Vec<Stack> = …`, add:

```rust
        if let Some(slot) = self.pet.as_mut() {
            *slot = slot.take().and_then(clean);
        }
```

(`split_for_death` walks the worn gear, bags and backpack only, so the companion's stack stays.)

In `crates/nightfall-sim/src/profile.rs`, `saved_inventory`'s match gains `Place::Pet => {}` for now (Task 3 saves it).

In `crates/benilla-app/src/nightfall/backpack.rs`, `smart_act`'s `match at` arm `Place::Equip(_) | Place::Bag(_) => …` becomes `Place::Equip(_) | Place::Bag(_) | Place::Pet => …` (a click takes the companion's stack back into the backpack).

In `crates/nightfall-sim/src/raid/mod.rs`:

- `Loadout` gains:

```rust
    /// The companion along, by kind (an index into `InnContent::companions`).
    pub pet: Option<u8>,
```

- `Carry` gains:

```rust
    /// The companion following them, by kind, for every client to draw.
    pub pet: Option<u8>,
```

- In `give_loadout`, `let left = inv.sanitize(&content);` becomes:

```rust
        // The companion's slot comes with the companion; without one, a stack there is left out.
        let mut extra = Vec::new();
        match loadout.pet {
            Some(_) if inv.pet.is_none() => inv.pet = Some(None),
            None => extra.extend(inv.pet.take().flatten()),
            Some(_) => {}
        }
        let mut left = inv.sanitize(&content);
        left.extend(extra);
```

and its last `if let Some(hi) = self.hero_slot(player) { … }` block gains `h.carry.pet = loadout.pet;`.

- [ ] **Step 3: Lanterns' own light, and kills by kind and place**

In `crates/nightfall-sim/src/items.rs`, `LightDef` gains:

```rust
    /// How much easier this lantern lit makes its hero to see (at least 1), in place of the
    /// senses' `lantern` factor: the lantern-maker's shuttered lantern is seen less.
    #[serde(default)]
    pub seen: Option<f32>,
```

and `check_items` gains, beside the lantern's check:

```rust
        if it.light.is_some_and(|l| l.seen.is_some_and(|s| !(s >= 1.0))) {
            return Err(format!("items.ron: \"{k}\": a lantern's `seen` is 1 or more"));
        }
```

In `crates/nightfall-sim/src/senses/mod.rs`'s `step_heroes`, `lantern: hero.carry.lantern.is_some(),` becomes `lantern: false,`, and right after `hero.visibility = visibility(hero.stance, light, r);` add:

```rust
        // A lit lantern (spec §5.1): its own factor, else the rules' (×1.6). Every factor is 1
        // or more, so clamping last equals clamping once.
        if let Some(def) = hero
            .carry
            .lantern
            .and_then(|i| content.items.get(usize::from(i)))
        {
            let factor = def.light.and_then(|l| l.seen).unwrap_or(r.lantern);
            hero.visibility = (hero.visibility * factor).min(100.0);
        }
```

In `content/nightfall/items.ron`, the shuttered lantern's `light: Some((range: 13.0, intensity: 1.0))` becomes `light: Some((range: 13.0, intensity: 1.0, seen: Some(1.2)))`.

In `crates/nightfall-sim/src/raid/mod.rs`:

- `HeroOutcome` gains:

```rust
    /// Each kill: the enemy's kind (an index into the content's enemies) and where it died, for
    /// the inn's contracts.
    pub slain: Vec<(u16, V2)>,
```

- `Raid` gains `pub(crate) slain: Vec<(PlayerId, Vec<(u16, V2)>)>,` and `impl Raid` gains:

```rust
    pub(crate) fn add_slain(&mut self, player: PlayerId, kind: u16, at: V2) {
        match self.slain.iter_mut().find(|(p, _)| *p == player) {
            Some(s) => s.1.push((kind, at)),
            None => self.slain.push((player, vec![(kind, at)])),
        }
    }

    pub(crate) fn slain_of(&self, player: PlayerId) -> Vec<(u16, V2)> {
        self.slain
            .iter()
            .find(|(p, _)| *p == player)
            .map_or_else(Vec::new, |(_, s)| s.clone())
    }
```

In `crates/nightfall-sim/src/raid/extract.rs`, `tally_kills` becomes:

```rust
    /// Every enemy killed since the last tally counts for the hero who killed it, with its kind
    /// and where it fell. Last in a step, so every kill of the step counts; [`World::drain_events`]
    /// starts the count over.
    pub(crate) fn tally_kills(&mut self) {
        let from = self.raid.counted.min(self.events.len());
        let by: Vec<(PlayerId, u32)> = self.events[from..]
            .iter()
            .filter_map(|e| match e {
                Event::EnemyDied { id, by: Some(p) } => Some((*p, *id)),
                _ => None,
            })
            .collect();
        self.raid.counted = self.events.len();
        for (p, id) in by {
            self.raid.add_kill(p);
            if let Some(e) = self.enemies.iter().find(|e| e.id == id) {
                self.raid.add_slain(p, e.kind as u16, e.pos);
            }
        }
    }
```

and `outcome`'s literal gains `slain: self.raid.slain_of(player),`. Every other `HeroOutcome { … }` literal (`grep -rn "HeroOutcome {" crates/nightfall-sim crates/benilla-app/src/nightfall`: `messages.rs`'s, `profile.rs`'s and `end_screen.rs`'s tests) gains `slain: Vec::new(),`; every `Loadout { … }` literal outside `raid/mod.rs` gains `pet: None,`.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 4 new tests with every earlier one).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim content/nightfall/items.ron crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the companion's stack, never lost; lanterns' own visibility; kills by kind and place

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The profile, version 3 (as built): the inn's progress, and the companion along

**Files:**
- Modify: `crates/nightfall-sim/src/profile.rs`, `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `Profile`, `SavedInventory`, `saved_inventory`, `inventory_of`, `raid_loadout`, `begin_raid`, `stash_capacity` (5a Task 7); `Place::Pet`, `Inventory::pet` (Task 2).
- Produces:
  - `PROFILE_VERSION = 2`; `Profile::inn: InnProgress`; `InnProgress { contracts: Vec<ContractState>, stations: Vec<(String, u8)>, recipes: Vec<String>, stash_bonus: u16, intel: bool, companion: Option<String>, applied: u32 }`; `ContractState { key: String, taken: bool, progress: Vec<u32>, finished: u32 }`.
  - `SavedInventory::pet: Option<SavedStack>`; `saved_inventory` and `inventory_of` keep the companion's stack.
  - `Profile::raid_loadout` and `begin_raid` give the loadout the companion's slot while `inn.companion` is set (its kind index is the client's to fill in `Loadout::pet`); `stash_capacity` adds `inn.stash_bonus`.

- [ ] **Step 1: Write the failing tests**

In `profile.rs`'s tests, add:

```rust
    #[test]
    fn a_version_1_stash_loads_with_an_empty_inn() {
        let v1 = r#"(version: 1, class: "rogue", gold: 9, stash: [(item: "gem", count: 2)],
            loadout: (pack: [(item: "bread", count: 1)]), raids: 3, extracted: 2)"#;
        let p = Profile::from_ron(v1).unwrap();
        assert_eq!(p.version, PROFILE_VERSION);
        assert_eq!((p.gold, p.raids, p.extracted), (9, 3, 2));
        assert_eq!(p.stash, vec![stack("gem", 2)]);
        assert_eq!(p.loadout.pack, vec![stack("bread", 1)]);
        assert_eq!(p.inn, InnProgress::default());
        assert_eq!(Profile::from_ron(&p.to_ron()).unwrap(), p);
    }

    #[test]
    fn the_companion_comes_with_its_stack() {
        let c = content("");
        let mut p = Profile::default();
        p.inn.companion = Some("hound".into());
        p.loadout.pet = Some(stack("gem", 3));
        p.loadout.pack.push(stack("bread", 1));
        let lo = p.raid_loadout(0, &c);
        assert_eq!(lo.inventory.get(Place::Pet), Some(&Stack::new(GEM, 3)));
        p.begin_raid(0, &c);
        assert_eq!(p.in_raid.as_ref().unwrap().pet, Some(stack("gem", 3)));
        assert!(p.settle_missing(&c));
        assert_eq!(p.loadout.pet, Some(stack("gem", 3)), "missing, and the companion's stack kept");
        // With no companion, an empty loadout is still the free kit, with no slot for one.
        let mut q = Profile::default();
        assert_eq!(q.raid_loadout(0, &c).inventory.pet, None);
        q.inn.companion = Some("hound".into());
        assert_eq!(q.raid_loadout(0, &c).inventory.pet, Some(None));
    }

    #[test]
    fn stash_room_grows_with_the_stash_station() {
        let c = content("");
        let mut p = Profile::default();
        assert_eq!(p.stash_capacity(&c), 2);
        p.inn.stash_bonus = 20;
        assert_eq!(p.stash_capacity(&c), 22);
    }
```

Run: `cargo test -p nightfall-sim profile`
Expected: FAIL to compile (`no field inn`, `no field pet on SavedInventory`).

- [ ] **Step 2: Version 2**

In `profile.rs`:

- `PROFILE_VERSION` becomes `2`, and its doc gains: "Version 1 (sub-project 5a) had no inn: it reads as version 2 with an empty one."
- Add:

```rust
/// A contract as the player has it (sub-project 5b).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractState {
    pub key: String,
    pub taken: bool,
    /// One number per objective: kills so far, or 1 once an extract has been made.
    pub progress: Vec<u32>,
    /// Times turned in.
    pub finished: u32,
}

/// What the player has built up at the inn (spec §8.5): kept forever.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InnProgress {
    pub contracts: Vec<ContractState>,
    /// Stations unlocked, by key, and the levels built (0: unlocked, nothing built yet).
    pub stations: Vec<(String, u8)>,
    /// Recipes known, by key.
    pub recipes: Vec<String>,
    /// Stash room the stash station added.
    pub stash_bonus: u16,
    /// The map table's look at the next raid.
    pub intel: bool,
    /// The companion, by key.
    pub companion: Option<String>,
    /// The raid (by `Profile::raids`) whose outcome last moved the contracts on: an outcome
    /// applied twice moves them once.
    pub applied: u32,
}
```

- `Profile` gains `pub inn: InnProgress,` (and `inn: InnProgress::default(),` in its `Default`).
- `SavedInventory` gains `pub pet: Option<SavedStack>,`, and `is_empty` gains `&& self.pet.is_none()`.
- `saved_inventory`'s `Place::Pet => {}` arm becomes `Place::Pet => out.pet = Some(st),`.
- In `inventory_of`, after the safe slots' loop, add:

```rust
    if let Some(s) = &saved.pet {
        if let Some((st, rest)) = known(s) {
            inv.pet = Some(Some(st));
            loose.extend(rest);
        }
    }
```

- Add:

```rust
impl Profile {
    /// `inv` with the companion's slot when the player has a companion.
    fn with_pet(&self, mut inv: Inventory) -> Inventory {
        if self.inn.companion.is_some() && inv.pet.is_none() {
            inv.pet = Some(None);
        }
        inv
    }
}
```

and in `raid_loadout`, `let inventory = match …;` is followed by `let inventory = self.with_pet(inventory);`; in `begin_raid`, `self.in_raid = Some(saved_inventory(&inv, content));` becomes `self.in_raid = Some(saved_inventory(&self.with_pet(inv), content));`. In `loadout_move`, `let (mut inv, aside) = inventory_of(&self.loadout, content);` becomes `let (inv, aside) = inventory_of(&self.loadout, content);` followed by `let mut inv = self.with_pet(inv);`, so a stack can be moved into the companion's slot between raids.
- `stash_capacity` becomes `usize::from(content.rules.run.stash_slots) + usize::from(self.inn.stash_bonus)`.

In `lib.rs`, the `pub use profile::{ … }` gains `ContractState, InnProgress`.

- [ ] **Step 3: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 3 new tests, and 5a's `an_old_file_without_a_version_loads` and `a_newer_profile_is_refused_not_misread` unchanged).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/profile.rs crates/nightfall-sim/src/lib.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the profile, version 2: the inn's progress and the companion's stack

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Contracts: taken, moved on by raids, turned in

**Files:**
- Create: `crates/nightfall-sim/src/contracts.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `InnContent`, `ContractDef`, `Objective`, `Reward` (Task 1); `Profile`, `InnProgress`, `ContractState`, `SavedStack`, `stash_add` (Task 3, 5a Task 7); `HeroOutcome::{fate, slain}`, `Fate` (5a, Task 2); `Content::{items, map.exits}`.
- Produces:
  - `nightfall_sim::ContractError::{InRaid, Unknown, Taken, TooMany, Locked, Finished, NotTaken, NotReady}` with `message(self) -> &'static str`.
  - On `Profile`: `contract(&self, c: usize, &InnContent) -> Option<&ContractState>`, `taken(&self, &InnContent) -> Vec<usize>`, `offer(&self, c, &InnContent) -> Result<(), ContractError>`, `take_contract(&mut self, c, &InnContent) -> Result<(), ContractError>`, `drop_contract(&mut self, c, &InnContent)`, `objective_lines(&self, c, &InnContent, &Content) -> Vec<(String, bool)>`, `contract_ready(&self, c, &InnContent, &Content) -> bool`, `turn_in(&mut self, c, &InnContent, &Content) -> Result<u32, ContractError>`, `apply_contracts(&mut self, &HeroOutcome, &InnContent, &Content)`, `held(&self, item: &str) -> u32`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/contracts.rs` ends with:

```rust
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{ContentSources, Fate, InnSources, Inventory, V2};

    pub(crate) fn raid() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                entries: [(50.0, 50.0, 0.0)],
                exits: [(key: "road", name: "The Road", at: (90.0, 90.0)),
                        (key: "lane", name: "The Lane", at: (10.0, 90.0))])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: r#"[
                (key: "worgen", name: "Worgen", display: 657, level: 27, hp: 900.0,
                 walk_speed: 2.5, run_speed: 8.0),
                (key: "spider", name: "Spider", display: 955, level: 26, hp: 800.0,
                 walk_speed: 2.2, run_speed: 7.0),
            ]"#,
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "tome", name: "Tome", kind: Valuable, icon: "a", weight: 1.0),
                (key: "cloth", name: "Cloth", kind: Material, icon: "a", weight: 0.1, stack: 20,
                 value: 10),
                (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 4),
                (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.2, stack: 20),
            ]"#,
            loot: "()",
        })
        .unwrap()
    }

    pub(crate) fn inn(raid: &Content) -> InnContent {
        InnContent::from_ron(
            InnSources {
                hub: r#"(
                    place: (name: "Inn", map: 0, spawn: (10.0, 10.0, 0.0),
                            bounds: [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
                            door: (at: (19.0, 10.0), radius: 2.0), minute: 1170,
                            view_distance: 200.0),
                    npcs: [
                        (key: "banker", name: "Farley", role: Banker,
                         body: (race: 1, sex: 0, class_id: 1), at: (5.0, 5.0, 0.0)),
                        (key: "trader", name: "Brog", role: Trader,
                         body: (race: 1, sex: 0, class_id: 1), at: (5.0, 15.0, 0.0)),
                        (key: "watch", name: "Ebonlock", role: Contracts,
                         body: (race: 1, sex: 1, class_id: 2), at: (15.0, 5.0, 0.0)),
                        (key: "smith", name: "Smith", role: Station("bench"),
                         body: (race: 1, sex: 0, class_id: 1), at: (15.0, 15.0, 0.0)),
                    ],
                    contracts_at_once: 2,
                    trader: (stock: [(item: "bread", price: 2), (item: "pouch", price: 30)],
                             sells_for: 0.5),
                    companions: [(key: "hound", name: "Hound", display: 1052)],
                )"#,
                contracts: r#"[
                    (key: "pack", giver: "watch", name: "Thin the pack",
                     objectives: [Kill(enemy: "worgen", count: 3, near: Some((50.0, 50.0, 20.0)))],
                     reward: (gold: 60, unlocks: ["bench"]), repeatable: true),
                    (key: "tome", giver: "watch", name: "The tome", needs: ["pack"],
                     objectives: [Bring(item: "tome", count: 1), Extract(exit: Some("road"))],
                     reward: (gold: 100)),
                    (key: "out", giver: "watch", name: "Out alive",
                     objectives: [Extract(exit: None)], reward: (gold: 10)),
                ]"#,
                stations: r#"(
                    stations: [
                        (key: "stash", name: "The Stash", keeper: "banker", open: true,
                         levels: [(gold: 50, items: [("cloth", 10)], gives: [StashSlots(20)])]),
                        (key: "bench", name: "Workbench", keeper: "smith",
                         levels: [(gold: 40, gives: [Recipe("pouch")]),
                                  (gold: 90, items: [("cloth", 4)], gives: [Intel, Companion("hound")])]),
                    ],
                    recipes: [(key: "pouch", station: "bench", makes: ("pouch", 1),
                               needs: [("cloth", 6)], gold: 5)],
                )"#,
            },
            raid,
        )
        .unwrap()
    }

    fn outcome(fate: Fate, slain: Vec<(u16, V2)>) -> HeroOutcome {
        HeroOutcome {
            player: 0,
            name: "A".into(),
            fate,
            time: 600.0,
            kills: slain.len() as u32,
            carried: Inventory::default(),
            recovered: Vec::new(),
            recovery: None,
            slain,
        }
    }

    /// The profile as it stands after a raid: begun, then this outcome applied.
    fn after_raid(p: &mut Profile, o: &HeroOutcome, inn: &InnContent, c: &Content) {
        p.raids += 1;
        p.apply_contracts(o, inn, c);
    }

    #[test]
    fn a_contract_is_offered_once_what_it_needs_is_done() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        assert_eq!(p.offer(1, &inn), Err(ContractError::Locked));
        p.take_contract(0, &inn).unwrap();
        assert_eq!(p.take_contract(0, &inn), Err(ContractError::Taken));
        p.take_contract(2, &inn).unwrap();
        assert_eq!(p.taken(&inn), vec![0, 2]);
        p.drop_contract(2, &inn);
        assert_eq!(p.taken(&inn), vec![0]);
        assert_eq!(p.offer(9, &inn), Err(ContractError::Unknown));
    }

    #[test]
    fn progress_comes_once_per_outcome_and_only_to_taken_contracts() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.take_contract(0, &inn).unwrap();
        let near = V2::new(55.0, 50.0);
        let far = V2::new(95.0, 5.0);
        let o = outcome(Fate::Died, vec![(0, near), (0, near), (0, far), (1, near)]);
        after_raid(&mut p, &o, &inn, &c);
        // Applied again for the same raid: nothing more.
        p.apply_contracts(&o, &inn, &c);
        assert_eq!(p.contract(0, &inn).unwrap().progress, vec![2]);
        assert!(!p.contract_ready(0, &inn, &c));
        // A contract not taken moves on with nobody.
        assert!(p.contract(2, &inn).is_none_or(|s| s.progress.is_empty()));
        after_raid(&mut p, &outcome(Fate::Died, vec![(0, near), (0, near)]), &inn, &c);
        assert_eq!(p.contract(0, &inn).unwrap().progress, vec![3], "held at the count");
        assert!(p.contract_ready(0, &inn, &c));
        assert_eq!(
            p.objective_lines(0, &inn, &c),
            vec![("Kill Worgen near the place: 3 / 3".to_string(), true)]
        );
    }

    #[test]
    fn turning_in_pays_unlocks_and_lets_a_repeatable_one_be_taken_again() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.take_contract(0, &inn).unwrap();
        assert_eq!(p.turn_in(0, &inn, &c), Err(ContractError::NotReady));
        let near = V2::new(50.0, 50.0);
        after_raid(&mut p, &outcome(Fate::Died, vec![(0, near); 3]), &inn, &c);
        assert_eq!(p.turn_in(0, &inn, &c), Ok(60));
        assert_eq!(p.gold, 60);
        assert_eq!(p.inn.stations, vec![("bench".to_string(), 0)]);
        assert_eq!(p.contract(0, &inn).unwrap().finished, 1);
        assert!(p.taken(&inn).is_empty());
        assert!(p.offer(0, &inn).is_ok(), "repeatable");
        assert!(p.offer(1, &inn).is_ok(), "its follower is offered now");
        p.take_contract(2, &inn).unwrap();
        after_raid(&mut p, &outcome(Fate::Extracted { exit: "The Lane".into() }, vec![]), &inn, &c);
        assert_eq!(p.turn_in(2, &inn, &c), Ok(10));
        assert_eq!(p.offer(2, &inn), Err(ContractError::Finished), "not repeatable");
    }

    #[test]
    fn bringing_takes_the_items_once() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.inn.contracts.push(ContractState {
            key: "pack".into(),
            taken: false,
            progress: Vec::new(),
            finished: 1,
        });
        p.take_contract(1, &inn).unwrap();
        // Out by the wrong exit: the extract is not made.
        after_raid(&mut p, &outcome(Fate::Extracted { exit: "The Lane".into() }, vec![]), &inn, &c);
        assert!(!p.contract_ready(1, &inn, &c));
        after_raid(&mut p, &outcome(Fate::Extracted { exit: "The Road".into() }, vec![]), &inn, &c);
        assert!(!p.contract_ready(1, &inn, &c), "and the tome is not here");
        p.loadout.pack.push(SavedStack {
            item: "tome".into(),
            count: 1,
        });
        assert_eq!(p.held("tome"), 1);
        assert!(p.contract_ready(1, &inn, &c));
        assert_eq!(p.turn_in(1, &inn, &c), Ok(100));
        assert_eq!(p.held("tome"), 0);
        assert_eq!(p.turn_in(1, &inn, &c), Err(ContractError::NotTaken));
        assert_eq!(p.gold, 100);
    }

    #[test]
    fn no_more_than_the_inn_allows_at_once_and_nothing_during_a_raid() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.inn.contracts.push(ContractState {
            key: "pack".into(),
            taken: false,
            progress: Vec::new(),
            finished: 1,
        });
        p.take_contract(0, &inn).unwrap();
        p.take_contract(1, &inn).unwrap();
        assert_eq!(p.take_contract(2, &inn), Err(ContractError::TooMany));
        p.begin_raid(0, &c);
        p.drop_contract(0, &inn);
        assert_eq!(p.taken(&inn), vec![0, 1], "nothing changes while a raid is under way");
        assert_eq!(p.turn_in(0, &inn, &c), Err(ContractError::InRaid));
    }
}
```

In `lib.rs` add `mod contracts;` and `pub use contracts::ContractError;`.

Run: `cargo test -p nightfall-sim contracts`
Expected: FAIL to compile (`no method named take_contract`).

- [ ] **Step 2: Write the contracts**

Above the tests in `contracts.rs`:

```rust
//! Contracts (spec §8.5): Duskwood's characters give them in the inn, one or two taken at a time;
//! a raid's outcome moves them on (kills where they were asked for, an extract by the exit named);
//! things to bring are taken as the contract is turned in, for gold and stations unlocked. Pure,
//! on the player's [`Profile`].

use crate::{Content, ContractState, Fate, HeroOutcome, InnContent, Objective, Profile};

/// Why a contract could not be taken, dropped or turned in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractError {
    InRaid,
    Unknown,
    Taken,
    TooMany,
    /// A contract it needs is not finished yet.
    Locked,
    /// Finished, and not to be done again.
    Finished,
    NotTaken,
    NotReady,
}

impl ContractError {
    /// The line the panel shows.
    pub fn message(self) -> &'static str {
        match self {
            ContractError::InRaid => "Not while a raid is under way.",
            ContractError::Unknown => "No such contract.",
            ContractError::Taken => "You have taken it already.",
            ContractError::TooMany => "You have taken as many as you can.",
            ContractError::Locked => "Not yet: there is something to do first.",
            ContractError::Finished => "Done, and done for good.",
            ContractError::NotTaken => "You have not taken it.",
            ContractError::NotReady => "There is more to do.",
        }
    }
}

impl Profile {
    /// Contract `c`'s state, if the player ever took it.
    pub fn contract(&self, c: usize, inn: &InnContent) -> Option<&ContractState> {
        let key = &inn.contracts.get(c)?.key;
        self.inn.contracts.iter().find(|s| s.key == *key)
    }

    fn contract_mut(&mut self, c: usize, inn: &InnContent) -> &mut ContractState {
        let key = inn.contracts[c].key.clone();
        let i = match self.inn.contracts.iter().position(|s| s.key == key) {
            Some(i) => i,
            None => {
                self.inn.contracts.push(ContractState {
                    key,
                    taken: false,
                    progress: Vec::new(),
                    finished: 0,
                });
                self.inn.contracts.len() - 1
            }
        };
        &mut self.inn.contracts[i]
    }

    /// The contracts taken, by index.
    pub fn taken(&self, inn: &InnContent) -> Vec<usize> {
        (0..inn.contracts.len())
            .filter(|&c| self.contract(c, inn).is_some_and(|s| s.taken))
            .collect()
    }

    /// Whether contract `c` may be taken now.
    pub fn offer(&self, c: usize, inn: &InnContent) -> Result<(), ContractError> {
        let def = inn.contracts.get(c).ok_or(ContractError::Unknown)?;
        let state = self.contract(c, inn);
        if state.is_some_and(|s| s.taken) {
            return Err(ContractError::Taken);
        }
        if state.is_some_and(|s| s.finished > 0) && !def.repeatable {
            return Err(ContractError::Finished);
        }
        let done = |n: usize| self.contract(n, inn).is_some_and(|s| s.finished > 0);
        if !def.needs.iter().all(|&n| done(n)) {
            return Err(ContractError::Locked);
        }
        if self.taken(inn).len() >= usize::from(inn.contracts_at_once) {
            return Err(ContractError::TooMany);
        }
        Ok(())
    }

    pub fn take_contract(&mut self, c: usize, inn: &InnContent) -> Result<(), ContractError> {
        if self.in_raid.is_some() {
            return Err(ContractError::InRaid);
        }
        self.offer(c, inn)?;
        let n = inn.contracts[c].objectives.len();
        let s = self.contract_mut(c, inn);
        s.taken = true;
        s.progress = vec![0; n];
        Ok(())
    }

    /// Contract `c` dropped, its progress with it.
    pub fn drop_contract(&mut self, c: usize, inn: &InnContent) {
        if self.in_raid.is_some() || self.contract(c, inn).is_none_or(|s| !s.taken) {
            return;
        }
        let s = self.contract_mut(c, inn);
        s.taken = false;
        s.progress.clear();
    }

    /// How many of `item` the player holds between the stash and the loadout.
    pub fn held(&self, item: &str) -> u32 {
        let l = &self.loadout;
        self.stash
            .iter()
            .chain(l.pack.iter())
            .chain(l.equip.iter().map(|(_, s)| s))
            .chain(l.bags.iter().map(|(_, s)| s))
            .chain(l.safe.iter().map(|(_, s)| s))
            .chain(l.pet.iter())
            .filter(|s| s.item == item)
            .map(|s| u32::from(s.count))
            .sum()
    }

    /// `count` of `item` taken from the stash first, then the loadout's backpack and safe slots;
    /// nothing when there are not enough.
    fn take_items(&mut self, item: &str, count: u32) -> bool {
        let loose = |v: &Vec<crate::SavedStack>| {
            v.iter()
                .filter(|s| s.item == item)
                .map(|s| u32::from(s.count))
                .sum::<u32>()
        };
        let safe: u32 = self
            .loadout
            .safe
            .iter()
            .filter(|(_, s)| s.item == item)
            .map(|(_, s)| u32::from(s.count))
            .sum();
        if loose(&self.stash) + loose(&self.loadout.pack) + safe < count {
            return false;
        }
        let mut left = count;
        let mut take = |s: &mut crate::SavedStack| {
            if s.item == item && left > 0 {
                let n = left.min(u32::from(s.count));
                s.count -= n as u16;
                left -= n;
            }
        };
        self.stash.iter_mut().for_each(&mut take);
        self.loadout.pack.iter_mut().for_each(&mut take);
        self.loadout.safe.iter_mut().for_each(|(_, s)| take(s));
        self.stash.retain(|s| s.count > 0);
        self.loadout.pack.retain(|s| s.count > 0);
        self.loadout.safe.retain(|(_, s)| s.count > 0);
        true
    }

    /// Whether objective `o` of a taken contract with `progress` is met.
    fn met(&self, o: &Objective, progress: u32, content: &Content) -> bool {
        match o {
            Objective::Kill { count, .. } => progress >= *count,
            Objective::Extract { .. } => progress >= 1,
            Objective::Bring { item, count } => self.held(&content.items[*item].key) >= *count,
        }
    }

    /// Each objective of contract `c` as a line, and whether it is met.
    pub fn objective_lines(&self, c: usize, inn: &InnContent, content: &Content) -> Vec<(String, bool)> {
        let Some(def) = inn.contracts.get(c) else {
            return Vec::new();
        };
        let state = self.contract(c, inn).filter(|s| s.taken);
        def.objectives
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let p = state.and_then(|s| s.progress.get(i).copied()).unwrap_or(0);
                let line = match o {
                    Objective::Kill { enemy, count, near } => format!(
                        "Kill {}{}: {} / {count}",
                        content.enemies[*enemy].name,
                        if near.is_some() { " near the place" } else { "" },
                        p.min(*count)
                    ),
                    Objective::Bring { item, count } => {
                        let def = &content.items[*item];
                        format!("Bring {}: {} / {count}", def.name, self.held(&def.key).min(*count))
                    }
                    Objective::Extract { exit } => match exit {
                        Some(e) => format!("Get out by {}", content.map.exits[*e].name),
                        None => "Get out alive".to_string(),
                    },
                };
                (line, state.is_some() && self.met(o, p, content))
            })
            .collect()
    }

    /// Whether taken contract `c` can be turned in.
    pub fn contract_ready(&self, c: usize, inn: &InnContent, content: &Content) -> bool {
        let (Some(def), Some(s)) = (inn.contracts.get(c), self.contract(c, inn)) else {
            return false;
        };
        s.taken
            && def
                .objectives
                .iter()
                .enumerate()
                .all(|(i, o)| self.met(o, s.progress.get(i).copied().unwrap_or(0), content))
    }

    /// Contract `c` turned in: what it asks to be brought taken, its gold paid, its stations
    /// unlocked; it is finished once more (and may be taken again when repeatable). The gold.
    pub fn turn_in(&mut self, c: usize, inn: &InnContent, content: &Content) -> Result<u32, ContractError> {
        if self.in_raid.is_some() {
            return Err(ContractError::InRaid);
        }
        let def = inn.contracts.get(c).ok_or(ContractError::Unknown)?;
        if self.contract(c, inn).is_none_or(|s| !s.taken) {
            return Err(ContractError::NotTaken);
        }
        if !self.contract_ready(c, inn, content) {
            return Err(ContractError::NotReady);
        }
        for o in &def.objectives {
            if let Objective::Bring { item, count } = o {
                let taken = self.take_items(&content.items[*item].key, *count);
                debug_assert!(taken, "ready means held");
            }
        }
        self.gold = self.gold.saturating_add(def.reward.gold);
        for &s in &def.reward.unlocks {
            let key = &inn.stations[s].key;
            if !self.inn.stations.iter().any(|(k, _)| k == key) {
                self.inn.stations.push((key.clone(), 0));
            }
        }
        let state = self.contract_mut(c, inn);
        state.taken = false;
        state.progress.clear();
        state.finished += 1;
        Ok(def.reward.gold)
    }

    /// A raid's outcome moves the taken contracts on, once per raid: kills of the kind asked for
    /// (inside the circle, when there is one), an extract by the exit named. A raid that went
    /// missing has no outcome, and moves nothing.
    pub fn apply_contracts(&mut self, o: &HeroOutcome, inn: &InnContent, content: &Content) {
        if self.inn.applied >= self.raids {
            return;
        }
        self.inn.applied = self.raids;
        for c in self.taken(inn) {
            let def = &inn.contracts[c];
            let mut adds = vec![0u32; def.objectives.len()];
            for (i, ob) in def.objectives.iter().enumerate() {
                adds[i] = match ob {
                    Objective::Kill { enemy, near, .. } => o
                        .slain
                        .iter()
                        .filter(|(kind, at)| {
                            usize::from(*kind) == *enemy
                                && near.is_none_or(|(c, r)| c.distance(*at) <= r)
                        })
                        .count() as u32,
                    Objective::Extract { exit } => match &o.fate {
                        Fate::Extracted { exit: by } => {
                            let named = exit.is_none_or(|e| content.map.exits[e].name == *by);
                            u32::from(named)
                        }
                        _ => 0,
                    },
                    Objective::Bring { .. } => 0,
                };
            }
            let state = self.contract_mut(c, inn);
            state.progress.resize(def.objectives.len(), 0);
            for (i, ob) in def.objectives.iter().enumerate() {
                let cap = match ob {
                    Objective::Kill { count, .. } => *count,
                    _ => 1,
                };
                state.progress[i] = (state.progress[i] + adds[i]).min(cap);
            }
        }
    }
}
```

Run: `cargo test -p nightfall-sim contracts`
Expected: PASS (5 tests).

- [ ] **Step 3: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/contracts.rs crates/nightfall-sim/src/lib.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: contracts taken, moved on once per raid, turned in for gold and stations

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Stations, recipes and the trader

**Files:**
- Create: `crates/nightfall-sim/src/stations.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `InnContent::{stations, recipes, trader, companions}`, `StationDef`, `StationLevel`, `Gift`, `RecipeDef`, `TraderDef` (Task 1); `Profile::{stash, gold, inn, stash_add}`, `InnProgress` (Task 3); `contracts::tests::{raid, inn}` (Task 4).
- Produces:
  - `nightfall_sim::StationError::{InRaid, Unknown, Locked, MaxLevel, NoGold, Missing, NoRecipe, Worthless}` with `message(self) -> &'static str`.
  - On `Profile`: `station_level(&self, s: usize, &InnContent) -> Option<u8>` (`None` locked), `upgrade_station(&mut self, s, &InnContent, &Content) -> Result<u8, StationError>`, `knows(&self, r: usize, &InnContent) -> bool`, `craft(&mut self, r, &InnContent, &Content) -> Result<(), StationError>`, `buy(&mut self, stock: usize, &InnContent, &Content) -> Result<(), StationError>`, `sell_to_trader(&mut self, stash_index: usize, &InnContent, &Content) -> Result<u32, StationError>`, `stashed(&self, item: &str) -> u32`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/stations.rs` ends with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::tests::{inn, raid};
    use crate::SavedStack;

    fn stack(item: &str, count: u16) -> SavedStack {
        SavedStack {
            item: item.into(),
            count,
        }
    }

    #[test]
    fn a_station_is_locked_until_unlocked_unless_open() {
        let c = raid();
        let inn = inn(&c);
        let p = Profile::default();
        assert_eq!(p.station_level(0, &inn), Some(0), "the stash is open from the start");
        assert_eq!(p.station_level(1, &inn), None);
        let mut q = Profile::default();
        q.inn.stations.push(("bench".into(), 0));
        assert_eq!(q.station_level(1, &inn), Some(0));
        q.gold = 40;
        assert_eq!(q.upgrade_station(1, &inn, &c), Ok(1));
        assert!(q.knows(0, &inn));
    }

    #[test]
    fn an_upgrade_pays_and_gives_or_does_neither() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.gold = 100;
        p.stash = vec![stack("cloth", 9)];
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::Missing));
        assert_eq!((p.gold, p.stashed("cloth"), p.inn.stash_bonus), (100, 9, 0));
        p.stash = vec![stack("cloth", 12)];
        p.gold = 49;
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::NoGold));
        assert_eq!((p.gold, p.stashed("cloth")), (49, 12));
        p.gold = 50;
        assert_eq!(p.upgrade_station(0, &inn, &c), Ok(1));
        assert_eq!((p.gold, p.stashed("cloth"), p.inn.stash_bonus), (0, 2, 20));
        assert_eq!(p.upgrade_station(0, &inn, &c), Err(StationError::MaxLevel));
        let mut q = Profile::default();
        q.inn.stations.push(("bench".into(), 1));
        q.gold = 90;
        q.stash = vec![stack("cloth", 4)];
        assert_eq!(q.upgrade_station(1, &inn, &c), Ok(2));
        assert!(q.inn.intel);
        assert_eq!(q.inn.companion.as_deref(), Some("hound"));
    }

    #[test]
    fn a_craft_pays_and_makes_or_does_neither() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.gold = 10;
        p.stash = vec![stack("cloth", 6)];
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::NoRecipe));
        p.inn.recipes.push("pouch".into());
        p.stash = vec![stack("cloth", 5)];
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::Missing));
        p.stash = vec![stack("cloth", 6)];
        p.gold = 4;
        assert_eq!(p.craft(0, &inn, &c), Err(StationError::NoGold));
        assert_eq!(p.stashed("cloth"), 6);
        p.gold = 5;
        assert_eq!(p.craft(0, &inn, &c), Ok(()));
        assert_eq!((p.gold, p.stashed("cloth"), p.stashed("pouch")), (0, 0, 1));
    }

    #[test]
    fn the_trader_sells_and_buys_at_their_share() {
        let c = raid();
        let inn = inn(&c);
        let mut p = Profile::default();
        p.gold = 3;
        assert_eq!(p.buy(1, &inn, &c), Err(StationError::NoGold));
        assert_eq!(p.buy(0, &inn, &c), Ok(()));
        assert_eq!((p.gold, p.stashed("bread")), (1, 1));
        // Bread is worth nothing in gold.
        assert_eq!(p.sell_to_trader(0, &inn, &c), Err(StationError::Worthless));
        p.stash.push(stack("cloth", 4));
        // Four cloth worth 10 each, at the trader's half.
        assert_eq!(p.sell_to_trader(1, &inn, &c), Ok(20));
        assert_eq!(p.gold, 21);
        assert_eq!(p.buy(9, &inn, &c), Err(StationError::Unknown));
    }
}
```

In `lib.rs` add `mod stations;` and `pub use stations::StationError;`.

Run: `cargo test -p nightfall-sim stations`
Expected: FAIL to compile (`no method named station_level`).

- [ ] **Step 2: Write the stations**

Above the tests in `stations.rs`:

```rust
//! The inn's stations (spec §8.5), its recipes and its trader, on the player's [`Profile`]: a
//! station is locked until a contract unlocks it (or open from the start), then upgraded level by
//! level with gold and materials from the stash, its gifts kept forever (stash room, recipes, the
//! map table's intel, a companion). Every purchase pays and gives, or does neither.

use crate::{Content, Gift, InnContent, Profile};

/// Why an upgrade, a craft or a trade was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationError {
    InRaid,
    Unknown,
    Locked,
    MaxLevel,
    NoGold,
    Missing,
    NoRecipe,
    Worthless,
}

impl StationError {
    /// The line the panel shows.
    pub fn message(self) -> &'static str {
        match self {
            StationError::InRaid => "Not while a raid is under way.",
            StationError::Unknown => "No such thing here.",
            StationError::Locked => "Not yet: a contract opens it.",
            StationError::MaxLevel => "Built as far as it goes.",
            StationError::NoGold => "Not enough gold.",
            StationError::Missing => "Not enough of what it needs in the stash.",
            StationError::NoRecipe => "You do not know how to make that yet.",
            StationError::Worthless => "Nobody would pay for that.",
        }
    }
}

impl Profile {
    /// Station `s`'s level: `None` while locked, else the levels built.
    pub fn station_level(&self, s: usize, inn: &InnContent) -> Option<u8> {
        let def = inn.stations.get(s)?;
        match self.inn.stations.iter().find(|(k, _)| *k == def.key) {
            Some((_, level)) => Some(*level),
            None => def.open.then_some(0),
        }
    }

    /// How many of `item` the stash holds.
    pub fn stashed(&self, item: &str) -> u32 {
        self.stash
            .iter()
            .filter(|s| s.item == item)
            .map(|s| u32::from(s.count))
            .sum()
    }

    /// Whether `gold` and `items` (from the stash) can be paid.
    fn can_pay(&self, gold: u32, items: &[(usize, u32)], content: &Content) -> Result<(), StationError> {
        if items
            .iter()
            .any(|&(i, n)| self.stashed(&content.items[i].key) < n)
        {
            return Err(StationError::Missing);
        }
        if self.gold < gold {
            return Err(StationError::NoGold);
        }
        Ok(())
    }

    /// `gold` and `items` paid out of the stash; [`Profile::can_pay`] said they can be.
    fn pay(&mut self, gold: u32, items: &[(usize, u32)], content: &Content) {
        self.gold -= gold;
        for &(i, n) in items {
            let key = &content.items[i].key;
            let mut left = n;
            for s in self.stash.iter_mut().filter(|s| s.item == *key) {
                let k = left.min(u32::from(s.count));
                s.count -= k as u16;
                left -= k;
            }
        }
        self.stash.retain(|s| s.count > 0);
    }

    /// Station `s` up one level: its price paid, its gifts given. The new level.
    pub fn upgrade_station(&mut self, s: usize, inn: &InnContent, content: &Content) -> Result<u8, StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let def = inn.stations.get(s).ok_or(StationError::Unknown)?;
        let level = self.station_level(s, inn).ok_or(StationError::Locked)?;
        let next = def
            .levels
            .get(usize::from(level))
            .ok_or(StationError::MaxLevel)?;
        self.can_pay(next.gold, &next.items, content)?;
        self.pay(next.gold, &next.items, content);
        for g in &next.gives {
            match g {
                Gift::StashSlots(n) => self.inn.stash_bonus = self.inn.stash_bonus.saturating_add(*n),
                Gift::Recipe(r) => {
                    let key = &inn.recipes[*r].key;
                    if !self.inn.recipes.contains(key) {
                        self.inn.recipes.push(key.clone());
                    }
                }
                Gift::Intel => self.inn.intel = true,
                Gift::Companion(c) => self.inn.companion = Some(inn.companions[*c].key.clone()),
            }
        }
        let new = level + 1;
        match self.inn.stations.iter_mut().find(|(k, _)| *k == def.key) {
            Some(entry) => entry.1 = new,
            None => self.inn.stations.push((def.key.clone(), new)),
        }
        Ok(new)
    }

    pub fn knows(&self, r: usize, inn: &InnContent) -> bool {
        inn.recipes.get(r).is_some_and(|d| self.inn.recipes.contains(&d.key))
    }

    /// Recipe `r` made: its materials and gold paid, what it makes into the stash.
    pub fn craft(&mut self, r: usize, inn: &InnContent, content: &Content) -> Result<(), StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let def = inn.recipes.get(r).ok_or(StationError::Unknown)?;
        if !self.knows(r, inn) {
            return Err(StationError::NoRecipe);
        }
        self.can_pay(def.gold, &def.needs, content)?;
        self.pay(def.gold, &def.needs, content);
        let (item, count) = def.makes;
        self.stash_add(
            crate::SavedStack {
                item: content.items[item].key.clone(),
                count,
            },
            content,
        );
        Ok(())
    }

    /// One of the trader's stock line `stock`, into the stash, for its price.
    pub fn buy(&mut self, stock: usize, inn: &InnContent, content: &Content) -> Result<(), StationError> {
        if self.in_raid.is_some() {
            return Err(StationError::InRaid);
        }
        let &(item, price) = inn.trader.stock.get(stock).ok_or(StationError::Unknown)?;
        if self.gold < price {
            return Err(StationError::NoGold);
        }
        self.gold -= price;
        self.stash_add(
            crate::SavedStack {
                item: content.items[item].key.clone(),
                count: 1,
            },
            content,
        );
        Ok(())
    }

    /// The stash stack at `index` sold to the trader, at their share of its value. The gold.
    pub fn sell_to_trader(&mut self, index: usize, inn: &InnContent, content: &Content) -> Result<u32, StationError> {
        let s = self.stash.get(index).ok_or(StationError::Unknown)?;
        let item = content.item(&s.item).ok_or(StationError::Unknown)?;
        let worth = content.items[item].value.saturating_mul(u32::from(s.count));
        let gold = (worth as f32 * inn.trader.sells_for).floor() as u32;
        if gold == 0 {
            return Err(StationError::Worthless);
        }
        self.stash.remove(index);
        self.gold = self.gold.saturating_add(gold);
        Ok(gold)
    }
}
```

- [ ] **Step 3: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 4 new tests with every earlier one).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/stations.rs crates/nightfall-sim/src/lib.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: stations upgraded with gold and materials, recipes crafted, the trader's buying and selling

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The inn's lobby on the wire: poses, Ready, the planned raid

**Files:**
- Modify: `crates/nightfall-sim/src/messages.rs`
- Modify: `crates/benilla-app/src/nightfall/session.rs` (`InnState`, `Peer`, `HostSide`, `ClientSide`, `Net`, `lobby`, `host_receive`, `broadcast_ended`, `roster`, `plan_raid`, the pure helpers), `crates/benilla-app/src/nightfall/run.rs` (`Sim::hosted`, `start_run`), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `LobbyPlayer`, `ClientMsg`, `ServerMsg::Lobby`, `PROTOCOL = 5`, `HeroPose`, `RaidRoll::standard` (earlier plans); client `session::{Net, Role, HostSide, ClientSide, Peer, lobby, host_receive, broadcast_ended, roster, hero_name}`, `run::{Sim::hosted, start_run, raid_seed}`, `inn()` (Task 1).
- Produces:
  - `LobbyPlayer { player, hero, pose: Option<HeroPose>, ready: bool }`; `ClientMsg::{InnPose(HeroPose), Ready(bool)}`; `ServerMsg::Lobby { players: Vec<LobbyPlayer>, raid: RaidRoll }`; `PROTOCOL = 6`.
  - Client: `session::InnState { pose: Option<HeroPose>, ready: bool }` (a `Resource`); `Net::{solo_raid: Option<RaidRoll>, planned_raid(&self) -> Option<RaidRoll>, friends(&self) -> Vec<LobbyPlayer>, party_ready(&self, own: bool) -> bool}`; `session::{plan_raid() -> RaidRoll, all_ready(bool, impl IntoIterator<Item = bool>) -> bool}`; `session::hero_name` becomes `pub(super)`; crate-private `lobby_due(bool, Option<f32>) -> bool`, `moved(Option<HeroPose>, HeroPose) -> bool`.
  - Client: `Sim::hosted(&[LobbyPlayer], RaidRoll)`: a raid as the host planned it.

- [ ] **Step 1: Write the failing tests**

In `crates/nightfall-sim/src/messages.rs`'s tests, add:

```rust
    #[test]
    fn the_inns_lobby_survives_the_wire() {
        use crate::{ExitPlan, HeroPose, Loadout, RaidRoll, V2};
        let msg = ServerMsg::Lobby {
            players: vec![LobbyPlayer {
                player: 1,
                hero: HeroPick {
                    name: "Friend".into(),
                    class: "mage".into(),
                    race: 1,
                    sex: 1,
                    loadout: Loadout::default(),
                },
                pose: Some(HeroPose {
                    pos: V2::new(-9460.0, 20.0),
                    facing: V2::new(1.0, 0.0),
                    z: 57.6,
                }),
                ready: true,
            }],
            raid: RaidRoll {
                seed: 7,
                entry: 0,
                exits: vec![ExitPlan {
                    open: true,
                    opens_at: 1080.0,
                }],
            },
        };
        let bytes = postcard::to_stdvec(&msg).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), msg);
        let up = ClientMsg::Ready(true);
        let bytes = postcard::to_stdvec(&up).unwrap();
        assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).unwrap(), up);
    }
```

In `crates/benilla-app/src/nightfall/session.rs`'s tests, add:

```rust
    #[test]
    fn the_lobby_goes_when_it_changes_and_no_faster() {
        assert!(lobby_due(true, None));
        assert!(!lobby_due(false, None));
        assert!(!lobby_due(true, Some(0.05)));
        assert!(lobby_due(true, Some(0.1)));
        assert!(!lobby_due(false, Some(5.0)));
    }

    #[test]
    fn all_ready_counts_everyone_present() {
        assert!(all_ready(true, []));
        assert!(!all_ready(false, []));
        assert!(!all_ready(true, [true, false]));
        assert!(all_ready(true, [true, true]));
    }

    #[test]
    fn a_pose_goes_only_when_it_moved() {
        use nightfall_sim::V2;
        let at = |x: f32| HeroPose {
            pos: V2::new(x, 0.0),
            facing: V2::new(1.0, 0.0),
            z: 0.0,
        };
        assert!(moved(None, at(0.0)));
        assert!(!moved(Some(at(0.0)), at(0.01)));
        assert!(moved(Some(at(0.0)), at(0.2)));
    }
```

and the tests' `LobbyPlayer { … }` literals (in `the_roster_marks_us`) gain `pose: None, ready: false,`.

Run: `cargo test -p nightfall-sim messages`
Expected: FAIL to compile (`no field pose`, `no variant Ready`).

- [ ] **Step 2: The messages**

In `crates/nightfall-sim/src/messages.rs`:

- The `use crate::{…}` gains `HeroPose, RaidRoll`.
- `PROTOCOL` becomes `6`.
- `LobbyPlayer` gains:

```rust
    /// Where their hero stands in the inn.
    pub pose: Option<HeroPose>,
    /// Ready at the raid board.
    pub ready: bool,
```

- `ClientMsg` gains:

```rust
    /// Where our hero stands in the inn.
    InnPose(HeroPose),
    /// Ready, or not, at the raid board.
    Ready(bool),
```

- `ServerMsg::Lobby { players: Vec<LobbyPlayer> }` becomes:

```rust
    /// The lobby as it stands, and the next raid as the host plans it (the map table's intel
    /// reads it).
    Lobby {
        players: Vec<LobbyPlayer>,
        raid: RaidRoll,
    },
```

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 3: The client's lobby**

In `crates/benilla-app/src/nightfall/session.rs`:

- Imports gain `HeroPose` and `RaidRoll` (from `nightfall_sim`) and `super::inn`.
- Add:

```rust
/// The lobby's least gap between two sends, seconds: friends' poses at most ten times a second.
const LOBBY_EVERY: f32 = 0.1;

/// Our own part of the inn's lobby: where our hero stands, and whether we are ready.
#[derive(Resource, Default)]
pub(super) struct InnState {
    pub pose: Option<HeroPose>,
    pub ready: bool,
}

/// Whether the lobby goes out now: it changed, and the last went at least [`LOBBY_EVERY`] ago.
fn lobby_due(changed: bool, since_last: Option<f32>) -> bool {
    changed && since_last.is_none_or(|s| s >= LOBBY_EVERY)
}

/// Whether a raid may start: we are ready, and so is every friend present.
pub(super) fn all_ready(own: bool, friends: impl IntoIterator<Item = bool>) -> bool {
    own && friends.into_iter().all(|r| r)
}

/// A pose worth sending: moved or turned since the last one sent.
fn moved(last: Option<HeroPose>, now: HeroPose) -> bool {
    last.is_none_or(|l| {
        l.pos.distance(now.pos) > 0.05
            || l.facing.distance(now.facing) > 0.05
            || (l.z - now.z).abs() > 0.05
    })
}

/// The next raid: the map's standard roll from a fresh seed (`NIGHTFALL_SEED` for a test run).
/// Sub-project 6 rolls its exits, weather and monsters here.
pub(super) fn plan_raid() -> RaidRoll {
    let seed = std::env::var("NIGHTFALL_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
    RaidRoll::standard(&content(), seed)
}
```

- `Net` gains:

```rust
    /// Alone: the next raid, planned as the inn is entered.
    pub solo_raid: Option<RaidRoll>,
```

- `Peer` gains `pub pose: Option<HeroPose>,` and `pub ready: bool,` (the welcome's `Peer { … }` literal gains `pose: None, ready: false,`); `HostSide` gains `pub raid: RaidRoll,` and `sent_at: Option<Instant>,` (`host`'s literal gains `raid: plan_raid(), sent_at: None,`); `ClientSide` gains `pub raid: Option<RaidRoll>,`, `sent_pose: Option<(HeroPose, Instant)>,` and `sent_ready: Option<bool>,` (`join`'s literal gains `raid: None, sent_pose: None, sent_ready: None,`).
- `hero_name` becomes `pub(super)`.
- Add to `impl Net`:

```rust
    /// The next raid as planned: the host's, the one the host told us, or ours alone.
    pub fn planned_raid(&self) -> Option<RaidRoll> {
        match &self.role {
            Role::Solo => self.solo_raid.clone(),
            Role::Host(h) => Some(h.raid.clone()),
            Role::Client(c) => c.raid.clone(),
        }
    }

    /// The friends in the lobby (never us), where they stand and whether they are ready.
    pub fn friends(&self) -> Vec<LobbyPlayer> {
        match &self.role {
            Role::Solo => Vec::new(),
            Role::Host(h) => h
                .peers
                .iter()
                .map(|p| LobbyPlayer {
                    player: p.player,
                    hero: p.hero.clone(),
                    pose: p.pose,
                    ready: p.ready,
                })
                .collect(),
            Role::Client(c) => c
                .lobby
                .iter()
                .filter(|p| Some(p.player) != c.me)
                .cloned()
                .collect(),
        }
    }

    /// Whether the party may start: alone, when we are ready; hosting, when everyone present is.
    pub fn party_ready(&self, own: bool) -> bool {
        match &self.role {
            Role::Solo => own,
            Role::Host(h) => all_ready(own, h.peers.iter().map(|p| p.ready)),
            Role::Client(_) => false,
        }
    }
```

- `roster`'s `LobbyPlayer { … }` literals gain `pose: None, ready: false,`.
- `broadcast_ended` gains, inside its `if let Role::Host(h)`: `h.raid = plan_raid(); for p in &mut h.peers { p.ready = false; }`; and add after it:

```rust
    /// Back in the inn alone: the next raid planned anew.
    pub fn replan_alone(&mut self) {
        if matches!(self.role, Role::Solo) {
            self.solo_raid = Some(plan_raid());
        }
    }
```

- `lobby` gains the parameter `inn_state: Res<InnState>`. Its `Role::Solo => {}` arm becomes `Role::Solo => { if net.solo_raid.is_none() { net.solo_raid = Some(plan_raid()); } }`. In the host's event loop, add the arms:

```rust
                    HostEvent::Message(conn, ClientMsg::InnPose(pose)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            if pose.pos.is_finite() && pose.facing.is_finite() && pose.z.is_finite() {
                                // Kept inside the inn, as our own hero is.
                                let pos = inn().place.bounds.clamp(pose.pos);
                                p.pose = Some(HeroPose { pos, ..pose });
                            }
                        }
                    }
                    HostEvent::Message(conn, ClientMsg::Ready(ready)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            p.ready = ready;
                        }
                    }
```

and the host's lobby building and sending (from `let mut players = vec![LobbyPlayer { player: 0, hero: own }];` to the end of the `Role::Host(h)` arm) becomes:

```rust
            let mut players = vec![LobbyPlayer {
                player: 0,
                hero: own,
                pose: inn_state.pose,
                ready: inn_state.ready,
            }];
            players.extend(h.peers.iter().map(|p| LobbyPlayer {
                player: p.player,
                hero: p.hero.clone(),
                pose: p.pose,
                ready: p.ready,
            }));
            let changed = h.sent.as_ref() != Some(&players);
            if lobby_due(changed, h.sent_at.map(|t| t.elapsed().as_secs_f32())) {
                host.broadcast(&ServerMsg::Lobby {
                    players: players.clone(),
                    raid: h.raid.clone(),
                });
                drop(host);
                h.sent = Some(players);
                h.sent_at = Some(Instant::now());
            }
```

In the client's arm, `ClientEvent::Message(ServerMsg::Lobby { players }) => c.lobby = players,` becomes:

```rust
                    ClientEvent::Message(ServerMsg::Lobby { players, raid }) => {
                        c.lobby = players;
                        c.raid = Some(raid);
                    }
```

and after the `Pick` it sends when the pick changed, add:

```rust
            if c.me.is_some() {
                if let Some(pose) = inn_state.pose {
                    let last = c.sent_pose.map(|(p, _)| p);
                    let due = c
                        .sent_pose
                        .is_none_or(|(_, t)| t.elapsed().as_secs_f32() >= LOBBY_EVERY);
                    if due && moved(last, pose) {
                        c.client.lock().unwrap().send(&ClientMsg::InnPose(pose));
                        c.sent_pose = Some((pose, Instant::now()));
                    }
                }
                if c.sent_ready != Some(inn_state.ready) {
                    c.client
                        .lock()
                        .unwrap()
                        .send(&ClientMsg::Ready(inn_state.ready));
                    c.sent_ready = Some(inn_state.ready);
                }
            }
```

- In `host_receive`, the ignored arm `HostEvent::Message(_, ClientMsg::Pick(_)) | HostEvent::Joined(..) => {}` becomes `HostEvent::Message(_, ClientMsg::Pick(_) | ClientMsg::InnPose(_) | ClientMsg::Ready(_)) | HostEvent::Joined(..) => {}`.

In `crates/benilla-app/src/nightfall/run.rs`:

- `Sim::hosted` takes `(roster: &[LobbyPlayer], roll: RaidRoll)`; its `let roll = RaidRoll::standard(&content, seed);` line goes (the `roll` parameter is begun instead).
- In `start_run` (which gains `mut inn_state: ResMut<super::session::InnState>`), `Sim::hosted(&roster, raid_seed(&time))` becomes:

```rust
    // The raid the host (or we, alone) planned in the inn; one planned now if none was.
    let roll = net
        .planned_raid()
        .unwrap_or_else(|| RaidRoll::standard(&content(), raid_seed(&time)));
    net.solo_raid = None;
    commands.insert_resource(Sim::hosted(&roster, roll));
```

(in place of the `commands.insert_resource(Sim::hosted(…));` line), and beside each `input.new_run();` add `inn_state.ready = false;`.
- `back_to_menu` gains `net.replan_alone();` beside its `next.set(…)` (it has `net` already in the joined case; add `mut net: ResMut<Net>` if it does not).

In `crates/benilla-app/src/nightfall/mod.rs`, `NightfallPlugin::build` gains `.init_resource::<session::InnState>()`.

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p nightfall-sim`
Expected: PASS.
Run: `cargo test -p benilla-app --lib nightfall::session -j 2`
Expected: PASS (the 3 new tests and the earlier ones).
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/messages.rs crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the inn's lobby on the wire: where friends stand, who is ready, the planned raid; protocol 6

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The inn scene: our hero walking it, friends in it, the evening light

**Files:**
- Create: `crates/benilla-app/src/nightfall/inn.rs`
- Modify (rename): every `Phase::Menu` in `crates/benilla-app/src/nightfall/` becomes `Phase::Inn`
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (`Phase`, registration), `crates/benilla-app/src/nightfall/atmosphere.rs` (`follow_map`, `inn_light`, `menu_camera` gone), `crates/benilla-app/src/nightfall/menu.rs` (the inn bar), `crates/benilla-app/src/nightfall/run.rs` (`trace_pos` shared)

**Interfaces:**
- Consumes: `InnContent::place` (Task 1); `session::{InnState, Net::friends, hero_name}` (Task 6); `Stash`, `inventory_of`, `gear::pieces_worn`, `dress::{spawn_player_body, pieces, wear, visible_entry_field}`, `units::{unit_guid, UnitKind, wow_to_bevy, ground_to_bevy, yaw_of}`, `menu::{Selection, MenuAction, build_menu, menu_clicks, refresh_menu, shown}`, `atmosphere::{enter, night, follow_map, hero_lights, SavedView, restore}` (earlier plans); engine: `crate::entities::CharCreate`, `crate::items::Items`, `crate::names::NameCache`, `crate::net::{GuidIndex, ObjectStore, SelfGuid}`, `crate::player::{Player, PlayerControlSet}`, `crate::creature_anim::{move_flags, MovementState}`, `benilla_world::lighting::FogOverride`, `benilla_world::view::ViewDistance`, `benilla_world::dev_state::DebugState`.
- Produces:
  - `nightfall::Phase::{Inn, Playing}` (`Inn` the default: between raids, the inn).
  - `inn::{InnBody, NeedsDress, InnViews, glide(Vec3, Vec3, f32) -> Vec3}`; systems `inn::{sync, dress_new, feed, place_friends, leave}`.
  - `atmosphere::inn_light`; `follow_map` follows the phase's map.
  - `run::trace_pos()` becomes `pub(super)`.

- [ ] **Step 1: Rename the phase**

```bash
grep -rl "Phase::Menu" crates/benilla-app/src/nightfall | xargs sed -i 's/Phase::Menu/Phase::Inn/g'
grep -rn "Phase::Menu" crates/benilla-app/src/nightfall || echo "renamed"
```

In `nightfall/mod.rs`, the `Phase` enum's first variant becomes:

```rust
    /// Between raids: the inn (spec §8.4), which is also the lobby.
    #[default]
    Inn,
```

Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 2: Write the inn scene with a test**

`crates/benilla-app/src/nightfall/inn.rs`:

```rust
//! The inn between raids (spec §8.4): its own scene, the hub `hub.ron` places on a map, at a warm
//! evening light and no fog, a contrast to the night outside. Our hero walks it with WoW's own
//! controller, kept inside its rooms as a raid keeps them inside its map, dressed in the loadout
//! they will bring; friends who joined walk it too, drawn where their clients say they stand.

use std::collections::{HashMap, HashSet};

use benilla_protocol::messages::ObjectFields;
use bevy::prelude::*;
use nightfall_sim::{inventory_of, Content, EquipSlot, HeroPose, PlayerId, V2};

use super::gear::pieces_worn;
use super::menu::Selection;
use super::session::{self, InnState, Net};
use super::stash::Stash;
use super::units::{self, UnitKind};
use super::{content, dress, inn};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::NameCache;
use crate::net::{GuidIndex, ObjectStore, SelfGuid};

/// Inn bodies' guid serials: ours, then each friend's after it.
const INN_SERIAL_BASE: u64 = 0x40;

/// Everything the inn drew, gone when it is left.
#[derive(Component)]
pub(super) struct InnBody;

/// A body just made, its gear still to write once its store exists.
#[derive(Component)]
pub(super) struct NeedsDress(Vec<(u16, u32)>);

/// Which entities draw us and our friends in the inn.
#[derive(Resource, Default)]
pub(super) struct InnViews {
    me: Option<Entity>,
    /// The class and the worn gear ours was dressed in.
    dressed: Option<(usize, Vec<(EquipSlot, u16)>)>,
    friends: HashMap<PlayerId, Entity>,
}

/// A friend's body a step toward where they stand: quick enough to keep up at ten poses a
/// second, smooth between them.
pub(super) fn glide(from: Vec3, to: Vec3, dt: f32) -> Vec3 {
    from.lerp(to, (dt * 8.0).min(1.0))
}

/// The worn gear the loadout shows.
fn loadout_worn(stash: &Stash, content: &Content) -> Vec<(EquipSlot, u16)> {
    let (inv, _) = inventory_of(&stash.profile.loadout, content);
    EquipSlot::ALL
        .iter()
        .filter_map(|&s| inv.equip[s.index()].map(|st| (s, st.item)))
        .collect()
}

/// Our hero in the inn, made again whenever the class or the loadout's gear changes (where they
/// stand kept); each friend's body from the lobby, gone when they leave it.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync(
    mut commands: Commands,
    selection: Res<Selection>,
    stash: Res<Stash>,
    net: Res<Net>,
    mut views: ResMut<InnViews>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut self_guid: ResMut<SelfGuid>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut player: ResMut<crate::player::Player>,
) {
    let Some(cc) = char_create.as_deref() else {
        return;
    };
    let content = content();
    let inn = inn();
    let level = content.rules.hero_level;
    let worn = loadout_worn(&stash, &content);
    let want = (selection.class, worn.clone());
    if views.dressed.as_ref() != Some(&want) {
        let had = views.me.take();
        if let Some(e) = had {
            index.0.retain(|_, v| *v != e);
            commands.entity(e).despawn();
        }
        let class = &content.classes[selection.class];
        let s = inn.place.spawn;
        let at = if had.is_some() && player.active {
            player.pos
        } else {
            units::wow_to_bevy([s.x, s.y, s.z])
        };
        let yaw = units::yaw_of(V2::from_angle(inn.place.facing.to_radians(), 1.0));
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            cc,
            class,
            (class.race, class.sex),
            INN_SERIAL_BASE,
            level,
            at,
            yaw,
        ) else {
            return;
        };
        let outfit_of =
            |cid: u8| dress::pieces(cc.0.start_outfit(class.race, cid, class.sex), &[]);
        let base = outfit_of(class.class_id);
        let (pieces, empty) = pieces_worn(&worn, &content, &base, outfit_of);
        let mut pairs = dress::wear(&mut items, &pieces);
        pairs.extend(empty.into_iter().map(|w| (dress::visible_entry_field(w), 0)));
        commands.entity(e).insert((InnBody, NeedsDress(pairs)));
        let guid = units::unit_guid(UnitKind::Player, INN_SERIAL_BASE);
        index.0.insert(guid, e);
        names.insert_player(guid, session::hero_name(), None);
        self_guid.0 = Some(guid);
        player.active = true;
        player.pos = at;
        views.me = Some(e);
        views.dressed = Some(want);
    }
    let friends = net.friends();
    for f in &friends {
        if views.friends.contains_key(&f.player) {
            continue;
        }
        let (Some(pose), Some(ci)) = (f.pose, content.class(&f.hero.class)) else {
            continue;
        };
        let class = &content.classes[ci];
        let serial = INN_SERIAL_BASE + 1 + u64::from(f.player);
        let at = units::ground_to_bevy(pose.pos, pose.z);
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            cc,
            class,
            (f.hero.race, f.hero.sex),
            serial,
            level,
            at,
            units::yaw_of(pose.facing),
        ) else {
            continue;
        };
        commands.entity(e).insert(InnBody);
        let guid = units::unit_guid(UnitKind::Player, serial);
        index.0.insert(guid, e);
        names.insert_player(guid, f.hero.name.clone(), None);
        views.friends.insert(f.player, e);
    }
    views.friends.retain(|p, e| {
        let keep = friends.iter().any(|f| f.player == *p);
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// A body made last frame gets its gear now that its store exists.
pub(super) fn dress_new(
    mut commands: Commands,
    mut bodies: Query<(Entity, &NeedsDress, &mut ObjectStore)>,
) {
    for (e, d, mut store) in &mut bodies {
        store.0.merge(ObjectFields::from_pairs(&d.0));
        commands.entity(e).remove::<NeedsDress>();
    }
}

/// Where WoW's controller left our hero, kept inside the inn's rooms, as the lobby's pose.
pub(super) fn feed(
    views: Res<InnViews>,
    real: Res<Time<Real>>,
    mut bodies: Query<&mut Transform>,
    mut player: ResMut<crate::player::Player>,
    mut state: ResMut<InnState>,
    mut last_trace: Local<f32>,
) {
    let inn = inn();
    let Some(mut tf) = views
        .me
        .and_then(|e| bodies.get_mut(e).ok())
        .filter(|_| player.active)
    else {
        state.pose = None;
        return;
    };
    let at = V2::new(-tf.translation.z, -tf.translation.x);
    let pos = inn.place.bounds.clamp(at);
    if pos != at {
        let back = units::ground_to_bevy(pos, tf.translation.y);
        tf.translation = back;
        player.pos = back;
    }
    if super::run::trace_pos() && real.elapsed_secs() - *last_trace >= 2.0 {
        *last_trace = real.elapsed_secs();
        let t = tf.translation;
        info!("nightfall: hero in the inn at ({:.1}, {:.1}, {:.1})", -t.z, -t.x, t.y);
    }
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    state.pose = Some(HeroPose {
        pos,
        facing: V2::new(yaw.cos(), yaw.sin()),
        z: tf.translation.y,
    });
}

/// Each friend's body glides to where their client says they stand, turned their way, walking
/// while it moves.
pub(super) fn place_friends(
    net: Res<Net>,
    views: Res<InnViews>,
    time: Res<Time<Real>>,
    mut bodies: Query<(&mut Transform, &mut MovementState)>,
) {
    let dt = time.delta_secs().max(1e-3);
    for f in net.friends() {
        let (Some(pose), Some(&e)) = (f.pose, views.friends.get(&f.player)) else {
            continue;
        };
        let Ok((mut tf, mut mv)) = bodies.get_mut(e) else {
            continue;
        };
        let before = tf.translation;
        tf.translation = glide(before, units::ground_to_bevy(pose.pos, pose.z), dt);
        let speed = before.distance(tf.translation) / dt;
        let turn = Quat::from_rotation_y(units::yaw_of(pose.facing));
        tf.rotation = tf.rotation.slerp(turn, (dt * 8.0).min(1.0));
        let flags = if speed > 0.3 { move_flags::FORWARD } else { 0 };
        if mv.speed != speed || mv.flags != flags {
            mv.speed = speed;
            mv.flags = flags;
        }
    }
}

/// Leaving the inn (a raid begins, or Nightfall is left): everything it drew goes, and the
/// controller lets go.
pub(super) fn leave(
    mut commands: Commands,
    bodies: Query<Entity, With<InnBody>>,
    mut views: ResMut<InnViews>,
    mut index: ResMut<GuidIndex>,
    mut self_guid: ResMut<SelfGuid>,
    mut player: ResMut<crate::player::Player>,
) {
    let gone: HashSet<Entity> = bodies.iter().collect();
    for &e in &gone {
        commands.entity(e).despawn();
    }
    index.0.retain(|_, e| !gone.contains(e));
    *views = InnViews::default();
    self_guid.0 = None;
    player.active = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_friend_glides_toward_where_they_stand_and_stops_on_it() {
        let to = Vec3::new(8.0, 0.0, 0.0);
        let one = glide(Vec3::ZERO, to, 1.0 / 60.0);
        assert!(one.x > 0.0 && one.x < 8.0);
        assert_eq!(glide(Vec3::ZERO, to, 1.0), to);
    }
}
```

In `crates/benilla-app/src/nightfall/run.rs`, `fn trace_pos()` becomes `pub(super) fn trace_pos()`.

- [ ] **Step 3: The evening light and the inn bar**

In `crates/benilla-app/src/nightfall/atmosphere.rs`:

- Delete `menu_camera` (WoW's own camera follows our hero in the inn).
- `follow_map` gains `phase: Res<State<super::Phase>>` and its `let want = content().map.map;` becomes:

```rust
    // The inn's map between raids, the raid's during one (both are map 0 for Goldshire and
    // Duskwood, but the hub may move to another map).
    let want = if *phase.get() == super::Phase::Inn {
        super::inn().place.map
    } else {
        content().map.map
    };
```

- Add:

```rust
/// In the inn: the hub's evening light, the engine's own fog and view distance (the night's are
/// given back if a raid set them), a contrast to the night outside (spec §8.4).
pub(super) fn inn_light(
    mut commands: Commands,
    saved: Option<Res<SavedView>>,
    view: Option<ResMut<ViewDistance>>,
    fog: Option<Res<FogOverride>>,
    debug: Option<ResMut<benilla_world::dev_state::DebugState>>,
) {
    if let (Some(saved), Some(mut view)) = (saved, view) {
        restore(&saved, &mut view);
        commands.remove_resource::<SavedView>();
    }
    if fog.is_some() {
        commands.remove_resource::<FogOverride>();
    }
    let minute = super::inn().place.minute;
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = minute;
    }
}
```

In `nightfall/mod.rs`'s `NightfallPlugin::build`:
- Remove the `atmosphere::menu_camera` registration.
- The `(atmosphere::enter, atmosphere::night, atmosphere::follow_map, atmosphere::hero_lights).in_set(NightfallSet)` registration becomes:

```rust
            .add_systems(
                Update,
                (atmosphere::enter, atmosphere::night)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing)),
            )
            .add_systems(
                Update,
                atmosphere::inn_light.in_set(NightfallSet).run_if(in_state(Phase::Inn)),
            )
            .add_systems(
                Update,
                (atmosphere::follow_map, atmosphere::hero_lights).in_set(NightfallSet),
            )
```

- Add `mod inn;` and:

```rust
            .init_resource::<inn::InnViews>()
            .add_systems(OnExit(Phase::Inn), inn::leave)
            .add_systems(OnExit(GameMode::Nightfall), inn::leave)
            .add_systems(
                Update,
                (inn::sync, inn::dress_new, inn::feed, inn::place_friends)
                    .chain()
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
```

In `crates/benilla-app/src/nightfall/menu.rs` (the menu is the inn bar now):
- In `build_menu`, the panel's `Node` keeps its column but sits top left: `left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(40.0),` becomes `left: Val::Px(16.0), top: Val::Px(16.0),` and `align_items: AlignItems::Center,` becomes `align_items: AlignItems::FlexStart,`; the title `text("Nightfall", 32.0, …)` becomes `text(&super::inn().place.name, 22.0, …)`.
- The action row loses `(MenuAction::PlaySolo, "Play Solo")`, `(MenuAction::Stash, "Stash")` and `(MenuAction::StartRun, "Start Run")`: the raid board's Ready starts a raid (Task 10) and the banker opens the stash (Task 8).
- `MenuAction` loses `PlaySolo`, `StartRun` and `Stash`; `menu_clicks` loses their arms and the parameters only they used (`start`, `stash_open`); `refresh_menu`'s always-shown list goes back to `MenuAction::Class(_) | MenuAction::Back`.
- `shown` becomes:

```rust
/// Which actions show for the session's role: alone, hosting or joined.
fn shown(role: &Role) -> &'static [MenuAction] {
    match role {
        Role::Solo => &[MenuAction::Host, MenuAction::Join],
        Role::Host(_) => &[MenuAction::CopyCode, MenuAction::Leave],
        Role::Client(_) => &[MenuAction::Leave],
    }
}
```

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (`a_friend_glides_toward_where_they_stand_and_stops_on_it`, and `nightfall_screens_show_only_in_nightfall` with `Phase::Inn`).
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 5: See the inn**

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_TRACE_POS=1 WOW_LIVE_SHOT=target/inn.png WOW_LIVE_SHOT_AT=25 ./target/release/azeroth-together.exe > target/inn.log 2>&1 &
```

Expected, once `target/inn.png` exists (then `kill` the PID): our hero stands in Goldshire at the hub's spawn, in a warm evening light with no fog, WoW's player frame up, the inn bar at the top left ("The Lion's Pride Inn", the class buttons, Host Co-op, Join). The log has `nightfall: hero in the inn at (…)` lines. If the hero stands outside the inn or in a wall, the positions in `hub.ron` are off: walk the hero (by hand, in a window the user opens) to the inn's middle, door and the NPCs' spots, read the traced positions, and write them into `hub.ron` (the `measure:` comment); list this in the hand-off when it cannot be done here.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the inn scene: our hero walks Goldshire's inn at an evening light, friends beside; the menu is the inn bar

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: NPCs at their places; the banker's stash and the trader's panel

**Files:**
- Create: `crates/benilla-app/src/nightfall/npcs.rs`, `crates/benilla-app/src/nightfall/trader.rs`
- Modify: `crates/benilla-app/src/nightfall/stash_panel.rs` (selling moves to the trader), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `InnContent::{npcs, trader}`, `NpcRole`, `Body` (Task 1); `Profile::{buy, sell_to_trader}`, `StationError::message` (Task 5); `inn::{InnBody, leave}`, `session::InnState` (Tasks 6–7); `stash_panel::StashOpen`, `widgets::*`, `Stash` (5a); `units::{spawn_unit, UnitSpec, UnitKind, unit_guid, wow_to_bevy, yaw_of, FACTION_FRIENDLY}`, `dress::{pieces, wear}` (earlier plans); engine: `WorldRightClick`, `crate::target::PressPick`, `crate::entities::CharCreate::{body_display, start_outfit}`, `crate::names::NameCache`, `crate::net::GuidIndex`.
- Produces:
  - `npcs::{NpcViews, InnPanel, PANEL_REACH, npc_at(&[(usize, Entity)], Entity) -> Option<usize>}`; systems `npcs::{spawn, right_click, walk_away, forget}`.
  - `trader::{TraderRoot, spawn_root, show, rebuild, clicks}`.
  - Selling is the trader's: the stash panel's Shift-click on a stash stack no longer sells.

- [ ] **Step 1: Write the NPCs, with a test**

`crates/benilla-app/src/nightfall/npcs.rs`:

```rust
//! The inn's NPCs (spec §8.4) at their places: each a character of `hub.ron`'s race and sex in a
//! class's starting outfit, named, standing where the hub says. A right-click on one within reach
//! opens their panel ([`InnPanel`]): the banker's stash and stash station, the trader's goods, a
//! giver's contracts, a keeper's station. Walking away closes it.

use benilla_world::interact::WorldRightClick;
use bevy::prelude::*;
use nightfall_sim::{NpcRole, V2};

use super::inn::InnBody;
use super::session::InnState;
use super::stash_panel::StashOpen;
use super::units::{self, UnitKind, UnitSpec};
use super::{content, dress, inn};
use crate::names::NameCache;
use crate::net::GuidIndex;

/// NPCs' guid serials start here.
const NPC_SERIAL_BASE: u64 = 0x100;
/// How near an NPC we must stand for their panel to open and stay open, yards.
pub(super) const PANEL_REACH: f32 = 6.0;

/// Which entity draws which NPC (by index into the inn's NPCs).
#[derive(Resource, Default)]
pub(super) struct NpcViews {
    pub drawn: Vec<(usize, Entity)>,
}

/// The NPC whose panel is open.
#[derive(Resource, Default)]
pub(super) struct InnPanel(pub Option<usize>);

/// The NPC an entity draws.
pub(super) fn npc_at(drawn: &[(usize, Entity)], e: Entity) -> Option<usize> {
    drawn.iter().find(|(_, d)| *d == e).map(|(i, _)| *i)
}

/// Every NPC drawn once the body tables are in, once per visit to the inn.
pub(super) fn spawn(
    mut commands: Commands,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut views: ResMut<NpcViews>,
) {
    if !views.drawn.is_empty() {
        return;
    }
    let Some(cc) = char_create.as_deref() else {
        return;
    };
    let content = content();
    let inn = inn();
    for (i, n) in inn.npcs.iter().enumerate() {
        let Some(display) = cc.0.body_display(n.body.race, n.body.sex) else {
            continue;
        };
        let outfit = cc.0.start_outfit(n.body.race, n.body.class_id, n.body.sex);
        let worn = dress::wear(&mut items, &dress::pieces(outfit, &[]));
        let serial = NPC_SERIAL_BASE + i as u64;
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                kind: UnitKind::Player,
                serial,
                display,
                scale: 1.0,
                level: content.rules.hero_level,
                faction: units::FACTION_FRIENDLY,
                bytes0: u32::from(n.body.race)
                    | u32::from(n.body.class_id) << 8
                    | u32::from(n.body.sex) << 16,
                weapons: &[],
                sheath: 0,
                extra: &worn,
            },
            units::wow_to_bevy([n.at.x, n.at.y, n.at.z]),
            units::yaw_of(V2::from_angle(n.facing.to_radians(), 1.0)),
        );
        commands.entity(e).insert(InnBody);
        let guid = units::unit_guid(UnitKind::Player, serial);
        index.0.insert(guid, e);
        names.insert_player(guid, n.name.clone(), None);
        views.drawn.push((i, e));
    }
}

/// A right-click on an NPC within reach opens their panel (the banker's opens the stash too).
pub(super) fn right_click(
    mut clicks: MessageReader<WorldRightClick>,
    press: Res<crate::target::PressPick>,
    views: Res<NpcViews>,
    state: Res<InnState>,
    mut panel: ResMut<InnPanel>,
    mut stash_open: ResMut<StashOpen>,
) {
    if clicks.read().last().is_none() {
        return;
    }
    let Some(target) = press.hovered.any().or(press.object.target) else {
        return;
    };
    let Some(i) = npc_at(&views.drawn, target) else {
        return;
    };
    let npc = &inn().npcs[i];
    let near = state
        .pose
        .is_some_and(|p| p.pos.distance(npc.at.ground()) <= PANEL_REACH);
    if !near {
        return;
    }
    panel.0 = Some(i);
    stash_open.0 = npc.role == NpcRole::Banker;
}

/// Walking away from the NPC closes their panel.
pub(super) fn walk_away(
    state: Res<InnState>,
    mut panel: ResMut<InnPanel>,
    mut stash_open: ResMut<StashOpen>,
) {
    let (Some(i), Some(pose)) = (panel.0, state.pose) else {
        return;
    };
    if pose.pos.distance(inn().npcs[i].at.ground()) > PANEL_REACH {
        panel.0 = None;
        stash_open.0 = false;
    }
}

/// The inn left: its NPCs went with its bodies; nothing is open.
pub(super) fn forget(mut views: ResMut<NpcViews>, mut panel: ResMut<InnPanel>) {
    *views = NpcViews::default();
    panel.0 = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entity_names_its_npc() {
        let mut world = bevy::ecs::world::World::new();
        let (a, b, c) = (
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
        );
        let drawn = [(0, a), (3, b)];
        assert_eq!(npc_at(&drawn, b), Some(3));
        assert_eq!(npc_at(&drawn, c), None);
    }
}
```

- [ ] **Step 2: The trader's panel**

`crates/benilla-app/src/nightfall/trader.rs`:

```rust
//! The trader's panel (spec §8.4): what they sell and for how much, and what of the stash they
//! would buy, at their share of its value. Our own panel, opened by a right-click on the trader.

use bevy::prelude::*;
use nightfall_sim::{NpcRole, StationError};

use super::npcs::InnPanel;
use super::stash::Stash;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, inn, Phase};

#[derive(Component)]
pub(super) struct TraderRoot;

#[derive(Component)]
struct TraderBody;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum TraderButton {
    Buy(usize),
    Sell(usize),
    Close,
}

#[derive(Resource)]
pub(super) struct TraderFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        TraderRoot,
        TraderBody,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(150.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
        GlobalZIndex(420),
        Visibility::Hidden,
    ));
    commands.insert_resource(TraderFont(font));
}

/// Whether the open panel is the trader's.
fn trading(panel: &InnPanel) -> bool {
    panel
        .0
        .is_some_and(|i| inn().npcs.get(i).is_some_and(|n| n.role == NpcRole::Trader))
}

pub(super) fn show(
    phase: Res<State<Phase>>,
    panel: Res<InnPanel>,
    mut roots: Query<&mut Visibility, With<TraderRoot>>,
) {
    let on = *phase.get() == Phase::Inn && trading(&panel);
    let want = if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// The goods and the stash's sellable stacks, built again whenever the stash or the panel
/// changes.
pub(super) fn rebuild(
    mut commands: Commands,
    stash: Res<Stash>,
    panel: Res<InnPanel>,
    font: Option<Res<TraderFont>>,
    body: Query<Entity, With<TraderBody>>,
) {
    if !(stash.is_changed() || panel.is_changed()) || !trading(&panel) {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    let content = content();
    let inn = inn();
    let npc = &inn.npcs[panel.0.expect("trading")];
    let row = || Node {
        column_gap: Val::Px(8.0),
        align_items: AlignItems::Center,
        ..default()
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        b.spawn(text(font, format!("{}: {}", npc.name, npc.title), 14.0, GOLD));
        b.spawn(text(font, format!("You have {} gold.", stash.profile.gold), 12.0, GREY));
        b.spawn(text(font, "For sale", 12.0, GREY));
        for (i, &(item, price)) in inn.trader.stock.iter().enumerate() {
            let def = &content.items[item];
            b.spawn(row()).with_children(|r| {
                widgets::slot(r, font, TraderButton::Buy(i), Some((def, 1)));
                r.spawn(text(font, format!("{}  ·  {price} gold", def.name), 12.0, Color::WHITE));
                widgets::text_button(r, font, TraderButton::Buy(i), "Buy");
            });
        }
        b.spawn(text(font, "From your stash", 12.0, GREY));
        for (n, s) in stash.profile.stash.iter().enumerate() {
            let Some(k) = content.item(&s.item) else {
                continue;
            };
            let def = &content.items[k];
            let gold =
                (def.value as f32 * f32::from(s.count) * inn.trader.sells_for).floor() as u32;
            if gold == 0 {
                continue;
            }
            b.spawn(row()).with_children(|r| {
                widgets::slot(r, font, TraderButton::Sell(n), Some((def, s.count)));
                r.spawn(text(font, format!("{}  ·  {gold} gold", def.name), 12.0, Color::WHITE));
                widgets::text_button(r, font, TraderButton::Sell(n), "Sell");
            });
        }
        widgets::text_button(b, font, TraderButton::Close, "Close");
    });
}

pub(super) fn clicks(
    buttons: Query<(&Interaction, &TraderButton), Changed<Interaction>>,
    mut stash: ResMut<Stash>,
    mut panel: ResMut<InnPanel>,
) {
    let content = content();
    let inn = inn();
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        let done: Result<String, StationError> = match *b {
            TraderButton::Buy(n) => stash.profile.buy(n, &inn, &content).map(|_| String::new()),
            TraderButton::Sell(n) => stash
                .profile
                .sell_to_trader(n, &inn, &content)
                .map(|g| format!("Sold for {g} gold.")),
            TraderButton::Close => {
                panel.0 = None;
                continue;
            }
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
```

In `crates/benilla-app/src/nightfall/stash_panel.rs`, `clicks`' arm `StashSlot::Stash(n) if shift => …sell…` goes (selling is the trader's now), `hover`'s stash line says `(click: take)`, and the stash column's footnote reads `Click: take  ·  sell at the trader`.

- [ ] **Step 3: Register them**

In `NightfallPlugin::build` (add `mod npcs;` and `mod trader;`):

```rust
            .init_resource::<npcs::NpcViews>()
            .init_resource::<npcs::InnPanel>()
            .add_systems(Startup, trader::spawn_root)
            .add_systems(OnExit(Phase::Inn), npcs::forget)
            .add_systems(Update, trader::show)
            .add_systems(
                Update,
                (
                    npcs::spawn,
                    npcs::right_click,
                    npcs::walk_away,
                    trader::clicks,
                    trader::rebuild,
                )
                    .chain()
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
```

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (`an_entity_names_its_npc` with the rest).
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 5: See the NPCs**

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 WOW_LIVE_SHOT=target/inn-npcs.png WOW_LIVE_SHOT_AT=25 ./target/release/azeroth-together.exe > target/inn-npcs.log 2>&1 &
```

Expected, once `target/inn-npcs.png` exists (then `kill` the PID): around our hero the inn's people stand, dressed (a dwarf trader, Madame Eva in a mage's robe, Ebonlock in a paladin's mail…), their names over them. A right-click needs a hand: list "right-click Pike (the stash opens), Coppermug (his goods; buy bread)" in the hand-off for the user to try.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the inn's NPCs at their places; the banker's stash and the trader's panel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The contract givers' and the station keepers' panels; raids move contracts on

**Files:**
- Create: `crates/benilla-app/src/nightfall/inn_panels.rs`
- Modify: `crates/benilla-app/src/nightfall/stash.rs` (`settle_run`), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Profile::{offer, take_contract, drop_contract, turn_in, objective_lines, contract_ready, contract, apply_contracts, station_level, upgrade_station, knows, craft, stashed}`, `ContractError`, `StationError` (Tasks 4–5); `InnContent::{contracts, stations, recipes, npcs}`, `NpcRole`, `Gift` (Task 1); `npcs::InnPanel` (Task 8); `Stash`, `widgets::*` (5a).
- Produces:
  - `inn_panels::{gift_line(&Gift, &InnContent, &Content) -> String, cost_line(u32, &[(usize, u32)], &Profile, &Content) -> String}`; systems `inn_panels::{spawn_root, show, rebuild, clicks}`.
  - `stash::settle_run` moves the contracts on with each outcome before settling it.

- [ ] **Step 1: Write the pure lines, with a test**

`crates/benilla-app/src/nightfall/inn_panels.rs` starts with:

```rust
//! The inn's own panels for contract givers and station keepers (spec §8.4, §8.5): a giver's
//! contracts with their objectives and what is done of them (Take, Drop, Turn In), and a keeper's
//! stations with their level, the next level's price and gifts (Upgrade), and the recipes known
//! there (Craft). Our own panels; the spike says whether WoW's gossip and quest windows could
//! show them instead.

use bevy::prelude::*;
use nightfall_sim::{Content, Gift, InnContent, NpcRole, Profile};

use super::npcs::InnPanel;
use super::stash::Stash;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, inn, Phase};

/// What a station level's gift is, in words.
pub(super) fn gift_line(g: &Gift, inn: &InnContent, content: &Content) -> String {
    match g {
        Gift::StashSlots(n) => format!("{n} more stash slots"),
        Gift::Recipe(r) => {
            let (item, n) = inn.recipes[*r].makes;
            let name = &content.items[item].name;
            if n > 1 {
                format!("Learn to make {name} ×{n}")
            } else {
                format!("Learn to make {name}")
            }
        }
        Gift::Intel => "The map table shows the next raid's exits".into(),
        Gift::Companion(c) => format!("{} joins you on raids", inn.companions[*c].name),
    }
}

/// A price in words, each material with what the stash holds of it: "40 gold, Iron Scrap 3/5".
pub(super) fn cost_line(gold: u32, items: &[(usize, u32)], p: &Profile, content: &Content) -> String {
    let mut parts = vec![format!("{gold} gold (you have {})", p.gold)];
    for &(i, n) in items {
        let def = &content.items[i];
        parts.push(format!("{} {}/{n}", def.name, p.stashed(&def.key).min(n)));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{ContentSources, InnSources, SavedStack};

    #[test]
    fn gifts_and_prices_read_as_lines() {
        let c = Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 100.0, speed: 7.0)]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (99.0, 0.0), (99.0, 99.0), (0.0, 99.0)],
                entries: [(1.0, 1.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
            items: r#"[
                (key: "scrap", name: "Iron Scrap", kind: Material, icon: "a", weight: 1.0, stack: 20),
                (key: "potion", name: "Potion", kind: Potion, icon: "a", weight: 0.2, stack: 5,
                 on_use: Some(Heal(100.0))),
            ]"#,
            loot: "()",
        })
        .unwrap();
        let inn = InnContent::from_ron(
            InnSources {
                hub: r#"(place: (name: "Inn", map: 0, spawn: (5.0, 5.0, 0.0),
                    bounds: [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                    door: (at: (9.0, 5.0), radius: 1.0), minute: 1170, view_distance: 100.0),
                    npcs: [(key: "b", name: "B", role: Banker, body: (race: 1, sex: 0, class_id: 1),
                            at: (2.0, 2.0, 0.0)),
                           (key: "a", name: "A", role: Station("lab"), body: (race: 1, sex: 0, class_id: 1),
                            at: (3.0, 3.0, 0.0))],
                    contracts_at_once: 2, trader: (stock: [], sells_for: 1.0),
                    companions: [(key: "dog", name: "Watch Hound", display: 1)])"#,
                contracts: "[]",
                stations: r#"(stations: [(key: "lab", name: "Lab", keeper: "a",
                    levels: [(gold: 40, items: [("scrap", 5)], gives: [Recipe("p"), Intel, Companion("dog"), StashSlots(20)])])],
                    recipes: [(key: "p", station: "lab", makes: ("potion", 2), needs: [("scrap", 1)])])"#,
            },
            &c,
        )
        .unwrap();
        let gives: Vec<String> = inn.stations[0].levels[0]
            .gives
            .iter()
            .map(|g| gift_line(g, &inn, &c))
            .collect();
        assert_eq!(
            gives,
            vec![
                "Learn to make Potion ×2",
                "The map table shows the next raid's exits",
                "Watch Hound joins you on raids",
                "20 more stash slots"
            ]
        );
        let mut p = Profile::default();
        p.gold = 12;
        p.stash.push(SavedStack {
            item: "scrap".into(),
            count: 3,
        });
        assert_eq!(cost_line(40, &[(0, 5)], &p, &c), "40 gold (you have 12), Iron Scrap 3/5");
    }
}
```

Add `mod inn_panels;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::inn_panels -j 2`
Expected: PASS.

- [ ] **Step 2: The panels**

Append to `inn_panels.rs`, above its tests:

```rust
#[derive(Component)]
pub(super) struct InnPanelRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PanelButton {
    Take(usize),
    Drop(usize),
    TurnIn(usize),
    Upgrade(usize),
    Craft(usize),
    Close,
}

#[derive(Resource)]
pub(super) struct PanelFont(Handle<Font>);

/// Whether the open panel is one of these: a giver's, a keeper's, or the banker's stations.
fn ours(panel: &InnPanel) -> bool {
    panel.0.is_some_and(|i| {
        inn()
            .npcs
            .get(i)
            .is_some_and(|n| n.role != NpcRole::Trader)
    })
}

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.spawn((
        InnPanelRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(150.0),
            max_width: Val::Px(460.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
        GlobalZIndex(420),
        Visibility::Hidden,
    ));
    commands.insert_resource(PanelFont(font));
}

pub(super) fn show(
    phase: Res<State<Phase>>,
    panel: Res<InnPanel>,
    mut roots: Query<&mut Visibility, With<InnPanelRoot>>,
) {
    let on = *phase.get() == Phase::Inn && ours(&panel);
    let want = if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// The open NPC's contracts, or the stations they keep, built again whenever the stash or the
/// panel changes.
pub(super) fn rebuild(
    mut commands: Commands,
    stash: Res<Stash>,
    panel: Res<InnPanel>,
    font: Option<Res<PanelFont>>,
    body: Query<Entity, With<InnPanelRoot>>,
) {
    if !(stash.is_changed() || panel.is_changed()) || !ours(&panel) {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    let content = content();
    let inn = inn();
    let p = &stash.profile;
    let who = panel.0.expect("ours");
    let npc = &inn.npcs[who];
    let row = || Node {
        column_gap: Val::Px(6.0),
        ..default()
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        b.spawn(text(font, format!("{}: {}", npc.name, npc.title), 14.0, GOLD));
        // A giver's contracts.
        for (c, def) in inn.contracts.iter().enumerate().filter(|(_, d)| d.giver == who) {
            let taken = p.contract(c, &inn).is_some_and(|s| s.taken);
            b.spawn(text(font, &def.name, 13.0, if taken { GOLD } else { Color::WHITE }));
            if !def.text.is_empty() {
                b.spawn(text(font, &def.text, 11.0, GREY));
            }
            for (line, met) in p.objective_lines(c, &inn, &content) {
                let color = if met { Color::srgb(0.3, 1.0, 0.3) } else { GREY };
                b.spawn(text(font, format!("  {line}"), 11.0, color));
            }
            b.spawn(text(
                font,
                format!("  Reward: {} gold", def.reward.gold),
                11.0,
                GREY,
            ));
            b.spawn(row()).with_children(|r| {
                if taken {
                    if p.contract_ready(c, &inn, &content) {
                        widgets::text_button(r, font, PanelButton::TurnIn(c), "Turn In");
                    }
                    widgets::text_button(r, font, PanelButton::Drop(c), "Drop");
                } else {
                    match p.offer(c, &inn) {
                        Ok(()) => widgets::text_button(r, font, PanelButton::Take(c), "Take"),
                        Err(e) => {
                            r.spawn(text(font, e.message(), 11.0, GREY));
                        }
                    }
                }
            });
        }
        // The stations this NPC keeps.
        for (s, def) in inn.stations.iter().enumerate().filter(|(_, d)| d.keeper == who) {
            let level = p.station_level(s, &inn);
            let status = match level {
                None => "Locked: a contract opens it".to_string(),
                Some(n) => format!("Level {n} of {}", def.levels.len()),
            };
            b.spawn(text(font, format!("{}  ·  {status}", def.name), 13.0, Color::WHITE));
            if let Some(next) = level.and_then(|n| def.levels.get(usize::from(n))) {
                b.spawn(text(font, format!("  Next: {}", cost_line(next.gold, &next.items, p, &content)), 11.0, GREY));
                for g in &next.gives {
                    b.spawn(text(font, format!("  · {}", gift_line(g, &inn, &content)), 11.0, GREY));
                }
                b.spawn(row()).with_children(|r| {
                    widgets::text_button(r, font, PanelButton::Upgrade(s), "Upgrade");
                });
            }
            for (r, recipe) in inn.recipes.iter().enumerate().filter(|(_, d)| d.station == s) {
                if !p.knows(r, &inn) {
                    continue;
                }
                let (item, n) = recipe.makes;
                b.spawn(text(
                    font,
                    format!(
                        "  Make {} ×{n}: {}",
                        content.items[item].name,
                        cost_line(recipe.gold, &recipe.needs, p, &content)
                    ),
                    11.0,
                    GREY,
                ));
                b.spawn(row()).with_children(|row| {
                    widgets::text_button(row, font, PanelButton::Craft(r), "Craft");
                });
            }
        }
        widgets::text_button(b, font, PanelButton::Close, "Close");
    });
}

pub(super) fn clicks(
    buttons: Query<(&Interaction, &PanelButton), Changed<Interaction>>,
    mut stash: ResMut<Stash>,
    mut panel: ResMut<InnPanel>,
) {
    let content = content();
    let inn = inn();
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        let p = &mut stash.profile;
        let done: Result<String, String> = match *b {
            PanelButton::Take(c) => p.take_contract(c, &inn).map(|_| String::new()).map_err(|e| e.message().into()),
            PanelButton::Drop(c) => {
                p.drop_contract(c, &inn);
                Ok(String::new())
            }
            PanelButton::TurnIn(c) => p
                .turn_in(c, &inn, &content)
                .map(|g| format!("Done: {g} gold."))
                .map_err(|e| e.message().into()),
            PanelButton::Upgrade(s) => p
                .upgrade_station(s, &inn, &content)
                .map(|n| format!("{} is level {n}.", inn.stations[s].name))
                .map_err(|e| e.message().into()),
            PanelButton::Craft(r) => p
                .craft(r, &inn, &content)
                .map(|_| "Made: in your stash.".to_string())
                .map_err(|e| e.message().into()),
            PanelButton::Close => {
                panel.0 = None;
                continue;
            }
        };
        match done {
            Ok(line) => {
                stash.note = (!line.is_empty()).then_some(line);
                stash.save();
            }
            Err(why) => stash.note = Some(why),
        }
    }
}
```

In `crates/benilla-app/src/nightfall/stash.rs`, `settle_run`'s `stash.profile.settle(&o, &content());` becomes:

```rust
    let content = content();
    // The contracts move on with the raid (once per raid), then its outcome is settled.
    stash.profile.apply_contracts(&o, &super::inn(), &content);
    stash.profile.settle(&o, &content);
```

In `NightfallPlugin::build`:

```rust
            .add_systems(Startup, inn_panels::spawn_root)
            .add_systems(Update, inn_panels::show)
            .add_systems(
                Update,
                (inn_panels::clicks, inn_panels::rebuild)
                    .chain()
                    .after(npcs::walk_away)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: contract givers' and station keepers' panels; each raid moves the contracts on

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The door and the raid board: the map, the contracts, intel, the party, Ready

**Files:**
- Create: `crates/benilla-app/src/nightfall/board.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `HubPlace::{door, door_radius}` (Task 1); `Profile::{taken, objective_lines, inn.intel}` (Tasks 3–4); `session::{InnState, Net::{planned_raid, friends, party_ready, is_client}}` (Task 6); `exits::exit_list`, `widgets::*`, `menu::StartRequest`, `Stash`, `RaidRoll`, `ExitMark` (5a, earlier plans).
- Produces:
  - `board::{BoardOpen, at_door(Option<HeroPose>, V2, f32) -> bool, intel_marks(&RaidRoll) -> Vec<ExitMark>}`; systems `board::{spawn_root, door, show, rebuild, clicks, start}`.
  - Stepping into the door's ring opens the board; Ready toggles `InnState::ready`; alone, Ready starts the raid; hosting, the raid starts once everyone present is ready; joined, the host starts it.

- [ ] **Step 1: Write the pure parts with a test**

`crates/benilla-app/src/nightfall/board.rs` starts with:

```rust
//! The inn's door and the raid board (spec §8.4): stepping into the door's ring opens it: which
//! map (Duskwood), the contracts taken and what is done of them, the map table's intel when it is
//! built (which exits open when, from the raid the host plans), the party with who is ready, and
//! Ready. Alone, Ready starts the raid; hosting, the raid starts once everyone present is ready;
//! joined, the host starts it.

use bevy::prelude::*;
use nightfall_sim::{ExitMark, HeroPose, RaidRoll, V2};

use super::exits::exit_list;
use super::menu::StartRequest;
use super::session::{InnState, Net};
use super::stash::Stash;
use super::widgets::{self, text, EDGE, GOLD, GREY, PANEL};
use super::{content, inn, Phase};

/// Whether a hero at `pose` stands in the door's ring.
pub(super) fn at_door(pose: Option<HeroPose>, door: V2, radius: f32) -> bool {
    pose.is_some_and(|p| p.pos.distance(door) <= radius)
}

/// The exits as a raid about to begin has them, for the map table.
pub(super) fn intel_marks(roll: &RaidRoll) -> Vec<ExitMark> {
    roll.exits
        .iter()
        .enumerate()
        .map(|(i, p)| ExitMark {
            exit: i as u8,
            open: p.open && p.opens_at <= 0.0,
            opens_in: p.open.then_some(p.opens_at),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::ExitPlan;

    #[test]
    fn the_door_and_the_tables_intel() {
        let at = |x: f32| {
            Some(HeroPose {
                pos: V2::new(x, 0.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            })
        };
        assert!(at_door(at(1.5), V2::ZERO, 2.0));
        assert!(!at_door(at(2.5), V2::ZERO, 2.0));
        assert!(!at_door(None, V2::ZERO, 2.0));
        let roll = RaidRoll {
            seed: 1,
            entry: 0,
            exits: vec![
                ExitPlan { open: true, opens_at: 0.0 },
                ExitPlan { open: true, opens_at: 1080.0 },
                ExitPlan { open: false, opens_at: 0.0 },
            ],
        };
        assert_eq!(
            intel_marks(&roll),
            vec![
                ExitMark { exit: 0, open: true, opens_in: Some(0.0) },
                ExitMark { exit: 1, open: false, opens_in: Some(1080.0) },
                ExitMark { exit: 2, open: false, opens_in: None },
            ]
        );
    }
}
```

Add `mod board;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::board -j 2`
Expected: PASS.

- [ ] **Step 2: The board**

Append to `board.rs`, above its tests:

```rust
/// Whether the board is open: our hero is in the door's ring.
#[derive(Resource, Default)]
pub(super) struct BoardOpen(pub bool);

#[derive(Component)]
pub(super) struct BoardRoot;

#[derive(Component)]
struct BoardBody;

#[derive(Component)]
struct ReadyButton;

#[derive(Resource)]
pub(super) struct BoardFont(Handle<Font>);

pub(super) fn spawn_root(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            BoardRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Px(120.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(420),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                BoardBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    min_width: Val::Px(380.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(EDGE),
            ));
        });
    commands.insert_resource(BoardFont(font));
}

/// The board opens as our hero steps into the door's ring and closes as they step out.
pub(super) fn door(state: Res<InnState>, mut open: ResMut<BoardOpen>) {
    let place = &inn().place;
    let here = at_door(state.pose, place.door, place.door_radius);
    if open.0 != here {
        open.0 = here;
    }
}

pub(super) fn show(
    phase: Res<State<Phase>>,
    open: Res<BoardOpen>,
    mut roots: Query<&mut Visibility, With<BoardRoot>>,
) {
    let on = *phase.get() == Phase::Inn && open.0;
    let want = if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// The board's lines, built again when they change (checked twice a second while it is open).
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild(
    mut commands: Commands,
    open: Res<BoardOpen>,
    stash: Res<Stash>,
    net: Res<Net>,
    state: Res<InnState>,
    real: Res<Time<Real>>,
    font: Option<Res<BoardFont>>,
    body: Query<Entity, With<BoardBody>>,
    mut last: Local<(f32, Vec<String>)>,
) {
    if !open.0 {
        return;
    }
    let now = real.elapsed_secs();
    if now - last.0 < 0.5 && !open.is_changed() {
        return;
    }
    last.0 = now;
    let content = content();
    let inn = inn();
    let p = &stash.profile;
    let mut lines = vec![format!("The raid: {}", content.map.name)];
    let taken = p.taken(&inn);
    if taken.is_empty() {
        lines.push("No contract taken: the inn's people have work.".into());
    }
    for c in taken {
        lines.push(format!("Contract: {}", inn.contracts[c].name));
        for (line, met) in p.objective_lines(c, &inn, &content) {
            lines.push(format!("  {}{line}", if met { "done: " } else { "" }));
        }
    }
    if p.inn.intel {
        if let Some(roll) = net.planned_raid() {
            lines.push("The map table:".into());
            for l in exit_list(&intel_marks(&roll), &content).lines() {
                lines.push(format!("  {l}"));
            }
        }
    }
    lines.push(format!(
        "You{}",
        if state.ready { "  ·  ready" } else { "" }
    ));
    for f in net.friends() {
        lines.push(format!(
            "{}  ·  {}{}",
            f.hero.name,
            f.hero.class,
            if f.ready { "  ·  ready" } else { "" }
        ));
    }
    if net.is_client() {
        lines.push("The host starts the raid once everyone is ready.".into());
    }
    if last.1 == lines {
        return;
    }
    let (Ok(body), Some(font)) = (body.single(), font) else {
        return;
    };
    let font = &font.0;
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        for (i, l) in lines.iter().enumerate() {
            let (size, color) = if i == 0 { (15.0, GOLD) } else { (12.0, GREY) };
            b.spawn(text(font, l, size, color));
        }
        widgets::text_button(
            b,
            font,
            ReadyButton,
            if state.ready { "Not Ready" } else { "Ready" },
        );
    });
    last.1 = lines;
}

/// Ready toggled.
pub(super) fn clicks(
    buttons: Query<&Interaction, (Changed<Interaction>, With<ReadyButton>)>,
    mut state: ResMut<InnState>,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        state.ready = !state.ready;
    }
}

/// Alone and ready, or hosting with everyone present ready: the raid starts. Stepping away from
/// the door un-readies us.
pub(super) fn start(
    open: Res<BoardOpen>,
    net: Res<Net>,
    mut state: ResMut<InnState>,
    mut start: ResMut<StartRequest>,
) {
    if !open.0 {
        if state.ready {
            state.ready = false;
        }
        return;
    }
    if !net.is_client() && net.party_ready(state.ready) {
        start.0 = true;
    }
}
```

In `NightfallPlugin::build`:

```rust
            .init_resource::<board::BoardOpen>()
            .add_systems(Startup, board::spawn_root)
            .add_systems(Update, board::show)
            .add_systems(
                Update,
                (board::door, board::clicks, board::start, board::rebuild)
                    .chain()
                    .after(inn::feed)
                    .before(run::start_run)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Inn)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the inn's door opens the raid board: contracts, the map table's intel, the party, Ready

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The companion in the raid, and its slot in the panels

**Files:**
- Create: `crates/benilla-app/src/nightfall/companion.rs`
- Modify: `crates/benilla-app/src/nightfall/stash.rs` (`raid_loadout`), `crates/benilla-app/src/nightfall/session.rs` (`lobby`), `crates/benilla-app/src/nightfall/run.rs` (`start_run`), `crates/benilla-app/src/nightfall/backpack.rs` (the companion's row; Shift to its slot), `crates/benilla-app/src/nightfall/stash_panel.rs` (the companion's slot), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Carry::pet`, `Loadout::pet`, `Inventory::pet`, `Place::Pet` (Task 2); `InnProgress::companion`, `Profile::{raid_loadout, loadout_move}` (Task 3, 5a); `InnContent::{companion, companions}` (Task 1); `run::Sim`, `view::Views`, `units::{spawn_unit, UnitSpec, UnitKind, FACTION_FRIENDLY}`, `backpack::{smart_act, rebuild}`, `stash_panel::rebuild`, `inn::glide` (earlier tasks).
- Produces:
  - `stash::raid_loadout(&Stash, class: usize) -> Loadout`: the profile's loadout (or free kit) with our companion along; `session::lobby` and `run::start_run` use it.
  - `companion::{PetViews, PetBody, follow_spot(Vec3, Quat) -> Vec3}`; systems `companion::{sync, forget}`.
  - `smart_act`'s Shift from the backpack goes to a free safe slot, else the companion's empty slot.

- [ ] **Step 1: Write the pure part with a test**

`crates/benilla-app/src/nightfall/companion.rs`:

```rust
//! The companion in a raid (spec §8.2): a small pet that follows its hero, a step behind and to
//! the side, carrying the one stack no death takes. Every client draws every hero's companion from
//! `Carry::pet`; it is a look only: the host's rules know the stack, not the animal.

use std::collections::HashMap;

use bevy::prelude::*;
use nightfall_sim::PlayerId;

use super::inn::glide;
use super::run::Sim;
use super::units::{self, UnitKind, UnitSpec};
use super::view::Views;
use super::inn;
use crate::creature_anim::{move_flags, MovementState};

/// Companions' guid serials start here.
const PET_SERIAL_BASE: u64 = 0x500;
/// How far behind and beside its hero a companion walks, yards.
const BEHIND: f32 = 1.6;
const BESIDE: f32 = 0.8;

/// Which entity draws which hero's companion, and of which kind.
#[derive(Resource, Default)]
pub(super) struct PetViews {
    drawn: HashMap<PlayerId, (u8, Entity)>,
}

#[derive(Component)]
pub(super) struct PetBody;

/// Where a companion walks for a hero at `at` turned `rotation`: behind and a little beside.
pub(super) fn follow_spot(at: Vec3, rotation: Quat) -> Vec3 {
    // Bevy's forward is -Z; the hero's own forward is its rotation of it.
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    at - forward * BEHIND + right * BESIDE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_companion_walks_behind_and_beside() {
        let spot = follow_spot(Vec3::ZERO, Quat::IDENTITY);
        assert!((spot.z - BEHIND).abs() < 1e-5, "behind is +Z for a hero facing -Z");
        assert!((spot.x - BESIDE).abs() < 1e-5);
    }
}
```

Add `mod companion;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::companion -j 2`
Expected: PASS.

- [ ] **Step 2: The companion drawn, and brought**

Append to `companion.rs`, above its tests:

```rust
/// Every hero's companion drawn beside them and following; gone with its hero.
#[allow(clippy::type_complexity)]
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    time: Res<Time<Real>>,
    mut pets: ResMut<PetViews>,
    heroes: Query<&Transform, Without<PetBody>>,
    mut bodies: Query<(&mut Transform, &mut MovementState), With<PetBody>>,
) {
    let inn = inn();
    let dt = time.delta_secs().max(1e-3);
    for hero in &sim.world.heroes {
        let (Some(kind), Some(&he)) = (hero.carry.pet, views.heroes.get(&hero.player)) else {
            continue;
        };
        let Some(def) = inn.companions.get(usize::from(kind)) else {
            continue;
        };
        let Ok(htf) = heroes.get(he) else {
            continue;
        };
        let spot = follow_spot(htf.translation, htf.rotation);
        match pets.drawn.get(&hero.player) {
            Some(&(k, e)) if k == kind => {
                if let Ok((mut tf, mut mv)) = bodies.get_mut(e) {
                    let before = tf.translation;
                    tf.translation = glide(before, spot, dt);
                    tf.rotation = tf.rotation.slerp(htf.rotation, (dt * 6.0).min(1.0));
                    let speed = before.distance(tf.translation) / dt;
                    let flags = if speed > 0.3 { move_flags::FORWARD } else { 0 };
                    if mv.speed != speed || mv.flags != flags {
                        mv.speed = speed;
                        mv.flags = flags;
                    }
                }
            }
            other => {
                if let Some(&(_, old)) = other {
                    commands.entity(old).despawn();
                }
                let serial = PET_SERIAL_BASE + u64::from(hero.player);
                let e = units::spawn_unit(
                    &mut commands,
                    &UnitSpec {
                        kind: UnitKind::Creature,
                        serial,
                        display: def.display,
                        scale: def.scale,
                        level: 1,
                        faction: units::FACTION_FRIENDLY,
                        bytes0: 0,
                        weapons: &[],
                        sheath: 0,
                        extra: &[],
                    },
                    spot,
                    htf.rotation.to_euler(EulerRot::YXZ).0,
                );
                commands.entity(e).insert(PetBody);
                pets.drawn.insert(hero.player, (kind, e));
            }
        }
    }
    pets.drawn.retain(|player, (_, e)| {
        let keep = sim
            .world
            .hero(*player)
            .is_some_and(|h| h.carry.pet.is_some());
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// A run ended: its companions went with its entities.
pub(super) fn forget(mut pets: ResMut<PetViews>) {
    *pets = PetViews::default();
}
```

In `crates/benilla-app/src/nightfall/stash.rs`, add:

```rust
/// What we bring into a raid now: the profile's loadout (or the free kit), with our companion
/// along when the kennel gave us one.
pub(super) fn raid_loadout(stash: &Stash, class: usize) -> nightfall_sim::Loadout {
    let mut lo = stash.profile.raid_loadout(class, &content());
    lo.pet = stash
        .profile
        .inn
        .companion
        .as_ref()
        .and_then(|k| super::inn().companion(k))
        .map(|i| i as u8);
    lo
}
```

and in `session::lobby` and `run::start_run`, `stash.profile.raid_loadout(selection.class, &content())` becomes `super::stash::raid_loadout(&stash, selection.class)`.

In `crates/benilla-app/src/nightfall/backpack.rs`:
- In `smart_act`'s Shift branch, `_ => inv.first_empty(true).and_then(to),` becomes:

```rust
            _ => inv
                .first_empty(true)
                .or_else(|| inv.pet.as_ref().filter(|p| p.is_none()).map(|_| Place::Pet))
                .and_then(to),
```

- In `rebuild`, after the safe slots' row, add:

```rust
        if inv.pet.is_some() {
            b.spawn(text(font, "Your companion carries: never lost", 11.0, GREY));
            b.spawn(row(1)).with_children(|g| {
                widgets::slot(g, font, PackSlot::Mine(Place::Pet), item_at(Place::Pet));
            });
        }
```

In `crates/benilla-app/src/nightfall/stash_panel.rs`'s `rebuild`, after the safe slots' grid, add:

```rust
            if p.inn.companion.is_some() {
                col.spawn(text(font, "Your companion carries: never lost", 11.0, GREY));
                col.spawn(grid(1)).with_children(|g| {
                    let at = Place::Pet;
                    widgets::slot(g, font, StashSlot::Loadout(at), item_of(inv.get(at).copied()));
                });
            }
```

and in `clicks`' Shift arm for the loadout, `_ => inv.first_empty(true),` becomes (Shift sends a backpack stack to a safe slot, else to the companion's empty slot):

```rust
                    _ => inv.first_empty(true).or_else(|| {
                        let free = stash.profile.inn.companion.is_some() && inv.pet.flatten().is_none();
                        free.then_some(Place::Pet)
                    }),
```

In `crates/benilla-app/src/nightfall/backpack.rs`'s tests, add to `a_click_does_the_likely_thing`'s end:

```rust
        // With the safe slot full, Shift sends a stack to the companion's empty slot.
        let mut pet = Inventory::new(&c);
        pet.pet = Some(None);
        pet.add(Stack::new(1, 1), &c);
        pet.add(Stack::new(0, 1), &c);
        pet.move_to(Place::Pack(0), Place::Safe(0), &c).unwrap();
        assert_eq!(
            smart_act(&pet, Place::Pack(1), false, true, false, &c),
            Some(Act::Move { from: Place::Pack(1), to: Place::Pet })
        );
```

In `NightfallPlugin::build`:

```rust
            .init_resource::<companion::PetViews>()
            .add_systems(OnExit(Phase::Playing), companion::forget)
            .add_systems(
                Update,
                companion::sync
                    .after(view::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: A companion along**

A test stash with the kennel built (a companion) and a stack for it:

```bash
mkdir -p target/nf-home-pet/nightfall
cat > target/nf-home-pet/nightfall/stash.ron <<'EOF'
(version: 3, class: "rogue", gold: 0, stash: [],
 loadout: (pet: Some((item: "ruby_shard", count: 3))),
 inn: (companion: Some("watch_wolf"), stations: [("kennel", 1)]))
EOF
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-pet WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_BACKPACK=1 WOW_LIVE_SHOT=target/pet.png WOW_LIVE_SHOT_AT=30 WOW_LIVE_SHOT_COUNT=3 WOW_LIVE_SHOT_EVERY=4 ./target/release/azeroth-together.exe > target/pet.log 2>&1 &
```

Expected, once `target/pet-002.png` exists (then `kill` the PID): in the raid a small wolf walks behind and beside the hero as the autoplay circles; the backpack panel has "Your companion carries: never lost" with the ruby shards in its slot. (The loadout is not empty, so the free kit does not come: the hero is unarmed but for the wolf; that is right for this stash.)

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the companion follows its hero into the raid, carrying its stack; its slot in the panels

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Spike: can WoW's own windows be fed without a server? (timeboxed: 3 hours)

**Files:**
- Create: `docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md`
- Create and then delete (never committed): `crates/benilla-app/src/nightfall/spike.rs` and its `mod`/registration lines in `nightfall/mod.rs`

**Interfaces:**
- Consumes: engine: `crate::ui_gossip::{GossipState, feed_gossip, drain_gossip}`, `crate::ui_quest::{QuestGiver, feed_quest, drain_quest}`, `crate::ui_merchant::{MerchantOpen, feed_merchant, drain_merchant}`, `crate::ui_bank` (its open banker and `crate::ui_items`' containers `-1` and `5..=10`), `crate::ui_items::{feed, drain}`, `crate::ui_loot::{LootState, feed_loot, drain_loot}`, `crate::ui_session::InteractNpc`, `crate::net` (what the drains send).
- Produces: the findings doc, and nothing else in the tree.

The spec asks (§8.4, §13) whether WoW's real gossip, quest, merchant and bank windows can be fed locally, without a server; if they can, a later plan switches the inn to them. The shipped path stays our own panels whatever this finds. Stop at the timebox and write down what is known.

- [ ] **Step 1: Read the seams**

Read each window's app side and note, for the doc: the resource that holds the window's data (`GossipState`, `QuestGiver`, `MerchantOpen`, the bank's), the system that pushes it into the Lua VM (`feed_*`) and the events it fires, what opens the window (which server message's handler fills the resource), and what the window's buttons send back (`drain_*`: which `ClientCommand` it writes, to which `crate::net` seam). Note also what `ui_items` needs to show a bag or the bank (the player's descriptor fields, the item objects in the `GuidIndex`, templates in `Items`).

- [ ] **Step 2: Try the gossip window on the banker**

Write `crates/benilla-app/src/nightfall/spike.rs`: behind `NIGHTFALL_SPIKE=gossip`, when the banker's panel would open, fill `GossipState` as `SMSG_GOSSIP_MESSAGE`'s handler would (the banker's guid, a greeting text in the NPC-text cache, two options "Show me my things" and "Goodbye") and mark the interact NPC (`ui_session::InteractNpc`) as WoW's own `CMSG_GOSSIP_HELLO` path would. Register it in the inn's chain. Build, then:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_SPIKE=gossip WOW_LIVE_SHOT=target/spike-gossip.png WOW_LIVE_SHOT_AT=30 ./target/release/azeroth-together.exe > target/spike-gossip.log 2>&1 &
```

(the spike opens the window on its own 25 s in, as if the banker were clicked). Record: does WoW's GossipFrame show with the greeting and the options? Does clicking an option (by hand, or by calling the drain in a test) reach a seam the spike can read instead of the net? What had to be faked?

- [ ] **Step 3: Try the merchant window on the trader**

As Step 2 with `NIGHTFALL_SPIKE=merchant`: fill `MerchantOpen` as `SMSG_LIST_INVENTORY`'s handler would, with the trader's stock as rows (item entries need templates in `Items`: use stand-in templates with the item's name, icon display and price). Record what shows, and what a Buy click writes (`CMSG_BUY_ITEM` to the net) and whether it can be intercepted before the net, as the combat plan intercepts `UseAction` (`UiScript::take_action_uses`).

- [ ] **Step 4: Note the quest and bank windows without building them**

From Step 1's reading: what QuestFrame needs (`QuestGiver`: quest ids, texts, objectives, rewards as item entries), and what the bank needs (the vault as descriptor fields and item objects `ui_items` reads). Estimate the work for each.

- [ ] **Step 5: Write the findings and throw the spike away**

`docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md`, with these sections filled from what was found:

```markdown
# Nightfall: can WoW's own windows be fed without a server? (spike)

Date: <the day>. Timebox: 3 hours; used: <hours>. Plan: docs/superpowers/plans/2026-10-17-nightfall-inn.md, Task 12.

## Answer

<One paragraph: yes, partly, or no, and for which windows.>

## Gossip (GossipFrame)

- Data and feed: <resource, feed system, events>.
- What opened it here: <what the spike filled and faked>.
- What its buttons send, and whether we can take it before the net: <findings>.
- Shot: target/spike-gossip.png <what it shows>.

## Merchant (MerchantFrame)

<the same headings>

## Quest (QuestFrame)

<what it needs; estimate>

## Bank (BankFrame) and bags (ContainerFrame)

<what it needs; estimate>

## Recommendation

<Keep our own panels, or switch which windows in which later plan, and what engine-side seam (if
any) a switch would need: an intercept like UiScript::take_action_uses for each drain.>
```

Then remove the spike's code and registration so none of it is committed:

```bash
rm crates/benilla-app/src/nightfall/spike.rs
git -c safe.directory=D:/Games/azeroth-together-stealth diff --stat crates/benilla-app/src/nightfall/mod.rs
```

and undo the `mod spike;` and registration lines in `mod.rs` by hand (do not `git checkout` the file: other work may be in it). Run `cargo check -p benilla-app -j 2` (Expected: builds).

- [ ] **Step 6: Commit the findings only**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the spike on feeding WoW's own windows without a server; findings

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Docs, the full check, and co-op in the inn

**Files:**
- Modify: `CLAUDE.md` (the Nightfall section), `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line; §13's spike question), `scripts/package.sh` (only if it copies `content/` file by file)

- [ ] **Step 1: Write the docs**

In `CLAUDE.md`'s Nightfall section, add:

```markdown
- Nightfall's inn (sub-project 5b): between raids the party walks the hub `content/nightfall/hub.ron`
  places (Goldshire's Lion's Pride Inn; its place, bounds, door, light and every NPC's spot live
  only there, and a commented Darkshire block shows the move). NPCs open our own panels with a
  right-click: the banker (stash, stash station), the trader, contract givers (`contracts.ron`),
  station keepers (`stations.ron`: levels, recipes, intel, the kennel's companion). The door's
  ring opens the raid board (contracts, the map table's intel, the party, Ready). `nightfall-sim`'s
  `inn.rs`, `contracts.rs` and `stations.rs` are the rules; the stash file is version 3
  (`inn:`). The spike on WoW's own windows: docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md.
```

In the spec's status line, after 5a's sentence, add: `Sub-project 5b built (plan docs/superpowers/plans/2026-10-17-nightfall-inn.md).` In §13, the "WoW's own windows in the inn" bullet becomes: `- **WoW's own windows in the inn:** spiked in sub-project 5b (docs/superpowers/notes/2026-10-17-nightfall-wow-windows-spike.md); the inn uses our own panels until a later plan says otherwise.`

If `scripts/package.sh` copies `content/` file by file, add `hub.ron`, `contracts.ron` and `stations.ron`.

- [ ] **Step 2: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS.
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds with no new warnings in `nightfall-sim` or `nightfall/`.

- [ ] **Step 3: Co-op in the inn on one PC**

```bash
rm -rf target/nf-home-host target/nf-home-client
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-host WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_NAME=Host WOW_LIVE_SHOT=target/host-inn.png WOW_LIVE_SHOT_AT=35 WOW_LIVE_SHOT_COUNT=3 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/host-inn.log 2>&1 &
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-client WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend WOW_LIVE_SHOT=target/client-inn.png WOW_LIVE_SHOT_AT=30 WOW_LIVE_SHOT_COUNT=3 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/client-inn.log 2>&1 &
```

Expected, once both last shots exist (then `kill` both PIDs): each shot shows both heroes in the inn (the friend's body standing where its client put it), the NPCs at their places, the evening light, and the inn bar's lobby lines ("Joined: Friend" on the host). Neither log has a panic; the host's log has no burst of lobby sends (the client's pose arrives at most ten times a second). Walking to the door, readying and the raid starting need hands: list "both walk into the door's ring, both Ready, the raid starts at Duskwood's entry; back in the inn after it, the contracts moved on" in the hand-off for the user to try.

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md scripts/package.sh
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: document the inn; sub-project 5 built

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review against the spec

| Spec | Where |
|---|---|
| §8.4 the inn: its own scene in Elwynn, warm evening light, no fog, no enemies; kept inside by the bounds clamp | Tasks 1 (`hub.ron`'s place), 7 (the scene, `inn_light`, the clamp in `inn::feed`) |
| §8.4 the door opens the raid board: the map, the contracts taken, Ready; the raid starts when everyone is ready | Task 10 |
| §8.4 the co-op lobby: friends appear in the inn and walk around; class and loadout picked there | Tasks 6 (poses, Ready on the wire), 7 (friends drawn), 8 (the banker's stash panel, the class on the inn bar) |
| §8.4 NPCs at their places: a contract giver per character, a banker, a trader, the stations' keepers; a click opens their panel | Tasks 1 (`npcs`), 8 (drawn, right-click, banker and trader), 9 (givers and keepers) |
| §8.4 our own glue panels first; a spike on WoW's windows | Tasks 8–10 (our panels), 12 (the spike and its doc) |
| §8.4 Stormwind's Trade District later | Not here (the hub moves by `hub.ron` alone, Task 1's `the_hub_is_wherever_its_content_puts_it`) |
| §8.5 contracts from Ebonlock, Madame Eva, Abercrombie (and Stitches), one or two at a time, paying gold and unlocking stations | Tasks 1 (`contracts.ron`; Stitches commented for sub-project 6), 4, 9 |
| §8.5 stations upgraded with gold and found materials, kept forever: stash, workbench, alchemy, cooking fire, lantern-maker (brighter, quieter), map table (intel), kennel (companion) | Tasks 1 (`stations.ron`), 2 (a lantern's own `seen`), 5, 9, 10 (intel on the board) |
| §8.2 the companion: unlocked at the inn, follows, carries one item never lost | Tasks 2 (the slot), 3 (the profile), 11 (drawn, brought, its slot in the panels) |
| §8.3 the stash per player, gold; co-op stashes on their own machines | Task 3 (version 3), Task 13 (two homes) |
| The user's note: the hub could be Darkshire's | Task 1 (`hub.ron`'s commented Scarlet Raven Tavern; every place number in content), rulings |
