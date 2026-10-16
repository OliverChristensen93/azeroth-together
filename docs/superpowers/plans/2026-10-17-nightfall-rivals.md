# Nightfall Sub-project 7 (Rivals) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Others out there with goals of their own: parties of two or three bot adventurers (grave robbers, Defias scavengers, Night Watch deserters) who come into the raid, walk to points of interest and loot them, fight the monsters on their way, and then extract or hunt the heroes as their party's aggression says. They follow the heroes' own sight, noise and awareness rules both ways (heroes can sneak past them, overhear them and ambush them; they notice heroes as enemies do), fight with abilities from the same ability system (picked by tag: opener, nuke, control, escape, heal, and one telegraphed big move), carry gear and bags, and drop everything when killed.

**Architecture:** A rival is a **hero of another side**: it sits in `World::heroes` with `Hero::side = Side::Rival(n)`, a class of its own (each rival kind in `rivals.ron` becomes a `ClassDef` appended to `Content::classes`, marked `rival`), a pack, a fight and a stance like any hero, and it is moved by a `PlayerInput` its bot sets each step exactly as a player's client sets its own: poses, gait, Stealth presses, ability presses, dodges and acts. So the enemies see, hear, chase and strike rivals with no change; rivals' footsteps, searches and casts make the same noises; their bodies are the run loop's bodies, their extracts the exits' rings. What is new is in `nightfall-sim/src/rivals/`: their content (`content.rs`), the pure AI (ability choice by tag in `kit.rs`; party goals, points of interest, formation and walking in `goals.rs`), each rival's `Mind` of the party's heroes fed by the senses' own `sense` (`mind.rs`), the bots that turn goals into inputs (`drive.rs`), and parties rolled with the raid and brought in out of sight (`arrivals.rs`). The fight gains one file, `combat/foes.rs`: a hero's ability aimed at a hostile hero (checks, effects, holds and bleeds on heroes, area hits, auto-attack) and a hero's telegraphed big move. Each player's snapshot carries a small `RivalView` (which rivals their hero sees, and the rivals' "?" and "!" for them). The client draws rivals with its hero code, as hostile WoW players (a red nameplate), concealed when unseen, with their marks over them.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content), postcard (wire).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md`: §7.4 (rivals, binding), §1.1 (Single Player Tarkov's questing bots as the model), §4.2 and §4.3 (rival sight; "which enemies and rivals that hero can see"), §5 (the senses rivals follow both ways), §6.1 ("rivals' big moves show a ground circle or cone"), §8.1 ("rivals can follow them in" to Darkshire), §8.2 ("rivals' bags" among the containers), §13 (rival kits: simplified or full, playtest both). Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-rivals-rulings.md`. This plan starts after every task of the foundation, world-knowledge, senses, combat, run-loop (5a), inn (5b) and night-threats (6) plans is done, and edits files those plans wrote; every name it consumes from them is the one those plans give.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call. Other agents commit in this worktree: stage only the files a task names, never `git add -A`.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`. This machine runs out of memory on wide builds: build `benilla-app` and `azeroth-together` with `-j 2` (`-j 1` if a build is killed).
- The worktree has no `WoW/Data`: every game launch sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data` and `BENILLA_HOME=target/nf-home` (a test home). The exe does not exit after its shots: start it in the background, wait for the last shot file, then `kill` its PID.
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats` (spec §3.1). The rival AI (goals, party coordination, ability choice by tag, the extract-or-hunt decision by aggression, looting) is pure sim logic with deterministic unit tests: the choice and goal functions on plain values, whole worlds on hand-made maps (`heightmap::testmap`).
- Rivals use the shared systems, not copies: the senses' `sense`, `Mind`, `notice_range` and noises both ways; the fight's checks, effects, casts, telegraphs, dodge and falling; the run loop's packs, acts, searches, bodies and exits. A rival's body moves only through its `PlayerInput`.
- Every number is content: `content/nightfall/rivals.ron` (kinds, kits, gear, parties and their aggression, points of interest, entries), `rules.ron`'s `rivals:` (every number the bots go by, defaults in `RivalRules::default`), `abilities.ron` (the rivals' abilities).
- The host decides (spec §3.1): clients get rivals as heroes in each snapshot, plus their `RivalView`, and draw what those say.
- No engine file changes. Client changes are in `crates/benilla-app/src/nightfall/` (one new file, `rivals_view.rs`); edits to files earlier plans wrote are spelled out.
- Nothing Blizzard-derived goes into `content/` or a commit: content names WoW ids and icon names only; ids not yet checked in the data are marked `measure:`.
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Rivals leaking into the party's raid** (the raid never ending while a rival lives, a rival's outcome settled or sent, a rival reviving a hero or a hero a rival, a rival's eyes counted as a teammate's for the seen set). Expect the raid to end with the party alone, no outcome for any rival, revives only within a side, and a hero's seen set blind to what only a rival sees. Pinned in Task 4 (`the_raid_ends_with_the_party_whatever_the_rivals_do`, `rivals_never_revive_heroes_nor_lend_them_their_eyes`).
2. **One-sided senses** (rivals seeing heroes while enemies ignore rivals, a sneaking hero noticed by a rival as if running, a rival that cannot be ambushed, a stealthed rival plainly shown far off). Expect the same rules both ways. Pinned in Task 7 (`a_sneaking_rogue_passes_behind_a_rival_and_a_running_one_is_heard`, `a_sentry_hears_a_rival_walk_past_and_strikes_it_once_it_is_found`, `an_unaware_rival_takes_an_ambush_and_its_party_hears_of_it`, `the_hero_sees_rivals_in_the_open_not_behind_walls_nor_far_in_stealth`).
3. **Bot inputs through the press seams** (a press lost to the first-input baseline, a press repeated every step, an act never acknowledged so a rival stands at a crate forever, Stealth toggled twice). Expect each choice pressed once, refusals rare, a crate searched and emptied through the acts. Pinned in Task 8 (`a_party_walks_around_a_house_to_its_point_and_loots_the_crate`, `a_rival_in_a_fight_presses_once_per_choice`).
4. **A party stuck for the rest of the night** (a point of interest no path reaches, an exit no path reaches, a hunt with no one to find, every exit shut). Expect a goal without headway given up within `give_up` seconds and every party gone (out or dead) before the night's last stage. Pinned in Task 8 (`a_party_with_no_way_in_gives_the_point_up_and_leaves`) and Task 10 (`every_party_leaves_or_dies_within_the_night`).
5. **Arrivals and determinism** (a party appearing in a hero's face, rival player numbers colliding with players', two rolls of one seed differing, a world with rivals playing differently twice). Expect arrivals out of sight, numbers from `RIVAL_BASE` up, the same seed bringing the same parties, and the same inputs giving the same world. Pinned in Task 9 (`rivals_arrive_late_and_unseen_with_numbers_of_their_own`, `the_same_seed_brings_the_same_rivals_and_seeds_differ`) and Task 10 (`a_world_with_rivals_plays_the_same_twice`).

**Not in this plan** (see the rulings file): rival parties fighting each other; rivals eating, using potions or lanterns; rivals looting bodies outside their points of interest; Darkshire's guards against rivals; measuring the real points of interest, entries and outfits in the game (they are `measure:` placeholders, as every earlier plan's placements are).

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/rivals/mod.rs` (new) | `Side`, `RIVAL_BASE`, `RivalView`, `RivalMark`, `Rivals`, `RivalBody`, `Party`, `Way`; `World::{hostile, side_of, add_rival_hero, rival_struck, rival_leaves, add_party, rival_debug}` |
| `crates/nightfall-sim/src/rivals/content.rs` (new) | `RivalRules`, `KitDef`, `RivalKindDef`, `PartyDef`, `PoiDef`, `RivalContent::load` (kinds made classes), `Content::{playable, rival_kind, party}` |
| `crates/nightfall-sim/src/rivals/kit.rs` (new) | `KitSlot`, `Moment`, `Choice`, `kit_of`, `ready`, `choose`: abilities by tag |
| `crates/nightfall-sim/src/rivals/goals.rs` (new) | `Goal`, `PartyView`, `FoeSeen`, `decide`, `next_poi`, `formation_spot`, `walk_along`, `away_from`, `WALK_RATIO` |
| `crates/nightfall-sim/src/rivals/mind.rs` (new) | Rivals sensing heroes (`perceive_rivals`, the rumour), alerts and party shouts, `rivals_lose`, each player's view of rivals (`rival_view`: seen and marks) |
| `crates/nightfall-sim/src/rivals/drive.rs` (new) | `step_rivals`, `plan_party`, `drive`: goals into each rival's `PlayerInput` (walking, formation, looting acts, fight presses, dodges, Stealth) |
| `crates/nightfall-sim/src/rivals/arrivals.rs` (new) | `RivalOut`, `pick_rivals`, `step_rival_arrivals`, `add_party` |
| `crates/nightfall-sim/src/rivals/tests.rs` (new) | Content fixtures and whole worlds with rivals on hand-made maps |
| `crates/nightfall-sim/src/combat/foes.rs` (new) | Heroes against hostile heroes: `check_foe`, `land_foe`, holds, slows and bleeds on heroes, `swing_foe`, `foe_pace`, a hero's telegraph |
| `crates/nightfall-sim/src/{content,abilities,lib,world,sight,messages}.rs` | `ContentSources::rivals`, `Content::rivals`, `ClassDef::rival`, `Rules::rivals`; `Effect::Heal`; `Hero::side`, `World::rivals`, the step; seen sets by side; `Snapshot::rivals`; protocol 8 |
| `crates/nightfall-sim/src/combat/{mod,effects,heroes,falling,strikes}.rs` | `Telegraph::by`, `HeroFight::{combo_hero, held}`, `HeroCombat` holds, `Refusal::Held`, `Event::{Healed, RivalDied, RivalOut}`; landing on foes, Heal, a rival's Blink and Vanish; foe targets in presses and casts; revives by side, a rival dying at once, `wound_by`; a rival's telegraph landing |
| `crates/nightfall-sim/src/senses/{mod,react}.rs` | `sense` shared (by position and facing), its kin re-exported |
| `crates/nightfall-sim/src/raid/{mod,extract}.rs` | `RaidRoll::rivals`; rivals extracting without an outcome, their bodies holding everything, kills of rivals, the raid's end by the party |
| `crates/nightfall-sim/src/night/roll.rs` | The roll brings rivals |
| `content/nightfall/rivals.ron` (new), `rules.ron`, `abilities.ron` | The rivals' content |
| `crates/benilla-app/src/nightfall/rivals_view.rs` (new) | Hostile faction, concealment and marks of rivals, the map table's line, `NIGHTFALL_RIVALS*` hooks, `NIGHTFALL_TRACE_RIVALS` |
| `crates/benilla-app/src/nightfall/{mod,menu,view,fight,night}.rs` | `rivals.ron` read, playable classes only, walking rivals drawn walking, heal numbers and the held refusal, the planned raid's rivals |

---

### Task 1: Content: rivals.ron, their rules and abilities, each kind a class

**Files:**
- Create: `crates/nightfall-sim/src/rivals/mod.rs`, `crates/nightfall-sim/src/rivals/content.rs`, `crates/nightfall-sim/src/rivals/tests.rs`, `content/nightfall/rivals.ron`
- Modify: `crates/nightfall-sim/src/content.rs`, `crates/nightfall-sim/src/abilities.rs` (`Effect::Heal`), `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`
- Modify (every `ContentSources { … }` literal gains `rivals`): the fixtures in `crates/nightfall-sim/src` and `crates/benilla-app/src/nightfall` (`grep -rn "ContentSources {" crates/nightfall-sim crates/benilla-app/src/nightfall`)
- Modify: `content/nightfall/rules.ron`, `content/nightfall/abilities.ron`
- Modify (so the client builds and shows only heroes' classes): `crates/benilla-app/src/nightfall/mod.rs` (`prepare`, `CONTENT_TEXTS`, `content_texts`), `crates/benilla-app/src/nightfall/menu.rs`

**Interfaces:**
- Consumes: `Content::from_ron`, `ContentSources`, `ContentError`, `ClassDef::{key, name, description, class_id, race, sex, base_hp, stealth, bar, kit, free_kit, free_items}`, `Content::{class, ability, item, stage, abilities, loot.tables, map.bounds}`, `Rules`, `Effect`, `AbilityDef::{telegraph, has}`, `Tag` (earlier plans).
- Produces:
  - `nightfall_sim::{RivalRules, KitDef, RivalKindDef, PartyDef, PoiDef, RivalContent}`.
  - `RivalRules { parties_out: (u8, u8), arrive: (f32, f32), cone, engage_at, hunt_at, hunt_for, flee_health, flee_for, avoid_within, heal_below, rest_below, full_load, pois: (u8, u8), leave_by: Option<String>, gap, paths_per_step: u32, dodge, dodge_at, finisher_at: u8, downed: bool, loot_reach, give_up }` (`#[serde(default)]`, `Default` with the plan's starting values); `Rules::rivals: RivalRules`.
  - `KitDef::{Abilities(Vec<String>), Class(String)}`; `RivalKindDef { key, name, names, base, class_id: Option<u8>, race, sex, hp: Option<f32>, kit, big: Option<String>, stand_off, sneaks, gear: Vec<(String, u16)>, carry: Option<String>, class: usize, carry_table: Option<usize>, big_ability: Option<usize> }` (the last three resolved); `PartyDef { key, name, members: Vec<String>, aggression: (f32, f32), odds, kinds: Vec<usize> }`; `PoiDef { key, name, at: (f32, f32), radius, worth }`; `RivalContent { kinds, parties, pois, entries: Vec<V2>, leave_by: Option<usize> }` (`Default`), crate-only `RivalContent::load(&str, &mut Content) -> Result<RivalContent, ContentError>`.
  - `ClassDef::rival: bool` (`#[serde(skip)]`); `ContentSources::rivals: &str`; `Content::rivals: RivalContent`; `Content::{playable(&self) -> impl Iterator<Item = (usize, &ClassDef)>, rival_kind(&self, &str) -> Option<usize>, party(&self, &str) -> Option<usize>}`.
  - `Effect::Heal { min: f32, max: f32 }`.
  - Test-only `rivals::tests::{CLASSES, MAP, ENEMIES, ABILITIES, ITEMS, LOOT, RIVALS, rules, content, content_with}`.
  - Client: `nightfall::content_texts() -> &'static [String; 13]` (`rivals.ron` last).

- [ ] **Step 1: Write the test kit and the failing tests**

`crates/nightfall-sim/src/rivals/tests.rs`:

```rust
//! Rivals in whole worlds on small hand-made maps: their content, the fight between heroes and
//! rivals, their minds, their bots, their arrivals and whole raids with them.

use crate::{Content, ContentSources, KitDef, V2};

pub(super) const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0,
     swing: Some((min: 30.0, max: 30.0, every: 2.0)),
     bar: [(1, "stealth"), (2, "strike"), (3, "finish"), (74, "ambush")]),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 800.0,
     speed: 7.0, power_max: 1000.0, bar: [(1, "bolt"), (4, "nova")]),
]"#;
/// 120 yards square: two points of interest east of the entry, three exits.
pub(super) const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (120.0, 0.0), (120.0, 120.0), (0.0, 120.0)],
    entries: [(5.0, 60.0, 0.0)],
    exits: [(key: "east", name: "The East Gate", at: (115.0, 60.0)),
            (key: "north", name: "The North Gate", at: (60.0, 115.0)),
            (key: "cart", name: "The Cart", at: (60.0, 5.0), late: true)])"#;
pub(super) const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 600.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 30.0, max: 30.0, every: 2.0))),
]"#;
pub(super) const ABILITIES: &str = r#"[
    (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0, keeps_stealth: true,
     effects: [Stealth], tags: [Utility]),
    (key: "strike", name: "Strike", spell: 1752, aim: Enemy, range: 5.0, cost: 40.0, gcd: 1.0,
     facing: Front, attack: true, effects: [Weapon(percent: 100.0, bonus: 20.0), Combo(1)],
     tags: [Builder]),
    (key: "finish", name: "Finish", spell: 6762, aim: Enemy, range: 5.0, cost: 35.0, gcd: 1.0,
     facing: Front, needs: (combo: true),
     effects: [Damage(min: 50.0, max: 50.0, per_combo: 60.0)], tags: [Finisher]),
    (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     needs: (stealth: true, unaware: true), effects: [Weapon(percent: 250.0, bonus: 70.0)],
     tags: [Opener]),
    (key: "bolt", name: "Bolt", spell: 116, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
     gcd: 1.5, moving: Slowed, facing: Front, effects: [Damage(min: 100.0, max: 100.0)],
     tags: [Nuke]),
    (key: "nova", name: "Nova", spell: 122, aim: Me, radius: 10.0, cost: 50.0, cooldown: 25.0,
     gcd: 1.5, effects: [Damage(min: 10.0, max: 10.0), Control(kind: Root, secs: 8.0)],
     tags: [Control]),
    (key: "hack", name: "Hack", spell: 285, aim: Enemy, range: 5.0, cost: 30.0, gcd: 1.0,
     facing: Front, attack: true, effects: [Weapon(percent: 100.0, bonus: 30.0)], tags: [Nuke]),
    (key: "dirt", name: "Dirt", spell: 2094, aim: Enemy, range: 8.0, cost: 20.0, cooldown: 20.0,
     gcd: 1.0, sound: Quiet, effects: [Control(kind: Incapacitate, secs: 4.0)], tags: [Control]),
    (key: "bandage", name: "Bandage", spell: 3276, aim: Me, channel: 3.0, ticks: 3,
     cooldown: 30.0, moving: Rooted, sound: Quiet, effects: [Heal(min: 60.0, max: 60.0)],
     tags: [Heal]),
    (key: "dig", name: "Dig", spell: 6343, aim: Me, radius: 6.0, cost: 30.0, cooldown: 20.0,
     gcd: 1.0, moving: Rooted, sound: Loud,
     telegraph: Some((shape: Circle(radius: 6.0), windup: 1.2, on_self: true)),
     effects: [Damage(min: 150.0, max: 150.0)], tags: [Nuke]),
    (key: "shot", name: "Shot", spell: 3018, aim: Enemy, range: 30.0, cost: 20.0, cast: 1.0,
     gcd: 1.0, moving: Slowed, facing: Front, effects: [Damage(min: 60.0, max: 60.0)],
     tags: [Nuke]),
    (key: "net", name: "Net", spell: 6533, aim: Enemy, range: 15.0, cost: 20.0, cooldown: 25.0,
     gcd: 1.0, effects: [Control(kind: Root, secs: 5.0)], tags: [Control]),
    (key: "bleed", name: "Bleed", spell: 8632, aim: Enemy, range: 5.0, cost: 20.0, gcd: 1.0,
     sound: Silent, effects: [Dot(amount: 10.0, every: 1.0, secs: 5.0)], tags: [Nuke]),
    (key: "slow", name: "Slow", spell: 6136, aim: Enemy, range: 20.0, cost: 20.0, gcd: 1.0,
     effects: [Slow(by: 0.5, secs: 6.0)], tags: [Control]),
    (key: "dash", name: "Dash", spell: 2983, aim: Me, cooldown: 60.0, keeps_stealth: true,
     effects: [Sprint(by: 0.7, secs: 8.0)], tags: [Escape]),
]"#;
pub(super) const ITEMS: &str = r#"[
    (key: "shovel", name: "Shovel", kind: Weapon, icon: "INV_Pick_02", weight: 3.0,
     slot: Some(MainHand)),
    (key: "tunic", name: "Tunic", kind: Armour, icon: "INV_Chest_Cloth_17", weight: 2.0,
     slot: Some(Chest)),
    (key: "sack", name: "Sack", kind: Bag, icon: "INV_Misc_Bag_09", weight: 0.5, bag_slots: 4),
    (key: "gem", name: "Gem", kind: Valuable, icon: "INV_Misc_Gem_Ruby_01", weight: 1.0,
     stack: 10, value: 40),
]"#;
/// A crate at each point of interest; a rival's carried find is one gem.
pub(super) const LOOT: &str = r#"(
    kinds: [(key: "crate", name: "Crate", display: 1387, search: 1.0, sound: Open, slots: 6)],
    tables: [(key: "one", rolls: (1, 1), items: [(item: "gem", weight: 1.0)]),
             (key: "found", rolls: (1, 1), items: [(item: "gem", weight: 1.0)])],
    containers: [(kind: "crate", table: "one", at: (60.0, 60.0)),
                 (kind: "crate", table: "one", at: (90.0, 30.0))],
    bodies: [],
)"#;
/// Three kinds (a melee digger, a shooter, and a blade with the whole rogue kit) in three
/// parties; two points of interest; one entry for rivals in the west.
pub(super) const RIVALS: &str = r#"(
    kinds: [
        (key: "digger", name: "Digger", names: ["Silas", "Mott"], base: "rogue", class_id: Some(1),
         race: 1, sex: 0, hp: Some(900.0), kit: Abilities(["hack", "dirt", "bandage"]),
         big: Some("dig"), gear: [("shovel", 1), ("tunic", 1), ("sack", 1)], carry: Some("found")),
        (key: "shooter", name: "Shooter", base: "rogue", class_id: Some(1), race: 1, sex: 1,
         kit: Abilities(["shot", "net", "bandage"]), stand_off: 20.0, gear: [("tunic", 1)]),
        (key: "blade", name: "Blade", base: "rogue", race: 1, sex: 0, kit: Class("rogue"),
         sneaks: true),
    ],
    parties: [
        (key: "diggers", name: "Diggers", members: ["digger", "digger"], aggression: (0.1, 0.2),
         odds: 1.0),
        (key: "watch", name: "The Watch", members: ["shooter", "digger"], aggression: (0.8, 0.9),
         odds: 1.0),
        (key: "blades", name: "Blades", members: ["blade", "blade", "blade"],
         aggression: (0.5, 0.5), odds: 0.0),
    ],
    pois: [(key: "yard", name: "The Yard", at: (60.0, 60.0), radius: 6.0, worth: 2.0),
           (key: "barn", name: "The Barn", at: (90.0, 30.0), radius: 6.0)],
    entries: [(10.0, 60.0)],
)"#;

/// The tests' rules with `rivals` inside `rivals: ( … )` (the defaults when empty).
pub(super) fn rules(rivals: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 200.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0),
        rivals: ({rivals}))"#
    )
}

/// The tests' content with these rivals' rules and `rivals.ron`; `spawns` are RON list items,
/// each ending in a comma.
pub(super) fn content_with(rivals_rules: &str, rivals: &str, spawns: &str) -> Content {
    Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(rivals_rules),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: "[]",
        rivals,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// [`content_with`] the tests' `rivals.ron`.
pub(super) fn content(rivals_rules: &str, spawns: &str) -> Content {
    content_with(rivals_rules, RIVALS, spawns)
}

#[test]
fn every_kind_becomes_a_rival_class_after_the_heroes() {
    let c = content("", "");
    assert_eq!(c.classes.len(), 5);
    assert_eq!(c.playable().map(|(i, _)| i).collect::<Vec<_>>(), vec![0, 1]);
    let digger = &c.rivals.kinds[c.rival_kind("digger").unwrap()];
    let class = &c.classes[digger.class];
    assert!(class.rival);
    assert_eq!(class.key, "rival:digger");
    assert_eq!(class.name, "Digger");
    assert_eq!((class.class_id, class.race, class.sex), (1, 1, 0));
    assert_eq!(class.base_hp, 900.0);
    // The base's numbers stay: energy, its swing.
    assert_eq!(class.power_max, 100.0);
    assert!(class.swing.is_some());
    // The simplified kit from action 1, the big move put after it.
    let keys: Vec<(u8, &str)> = class
        .kit
        .iter()
        .map(|&(s, a)| (s, c.abilities[a].key.as_str()))
        .collect();
    assert_eq!(keys, vec![(1, "hack"), (2, "dirt"), (3, "bandage"), (4, "dig")]);
    assert_eq!(digger.big_ability, c.ability("dig"));
    assert_eq!(
        class.free_items,
        vec![
            (c.item("shovel").unwrap(), 1),
            (c.item("tunic").unwrap(), 1),
            (c.item("sack").unwrap(), 1)
        ]
    );
    assert_eq!(digger.carry_table, Some(1));
    assert_eq!(c.rivals.entries, vec![V2::new(10.0, 60.0)]);
    let watch = &c.rivals.parties[c.party("watch").unwrap()];
    assert_eq!(
        watch.kinds,
        vec![c.rival_kind("shooter").unwrap(), c.rival_kind("digger").unwrap()]
    );
}

#[test]
fn a_full_class_kit_takes_the_whole_bar() {
    let c = content("", "");
    let blade = &c.rivals.kinds[c.rival_kind("blade").unwrap()];
    assert!(matches!(blade.kit, KitDef::Class(_)));
    assert_eq!(c.classes[blade.class].kit, c.classes[0].kit);
    assert!(c.classes[blade.class].stealth);
    assert_eq!(blade.big_ability, None);
}

#[test]
fn no_rivals_file_is_no_rivals() {
    let c = content_with("", "", "");
    assert!(c.rivals.kinds.is_empty() && c.rivals.parties.is_empty());
    assert_eq!(c.classes.len(), 2);
}

#[test]
fn rivals_content_mistakes_are_refused() {
    let bad = |rivals_rules: &str, rivals: &str| {
        Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: &rules(rivals_rules),
            enemies: ENEMIES,
            spawns: "[]",
            abilities: ABILITIES,
            items: ITEMS,
            loot: LOOT,
            monsters: "[]",
            rivals,
        })
        .is_err()
    };
    assert!(bad("", &RIVALS.replace(r#"["hack", "dirt""#, r#"["hacks", "dirt""#)));
    assert!(bad("", &RIVALS.replace(r#"big: Some("dig")"#, r#"big: Some("hack")"#)));
    assert!(bad("", &RIVALS.replace(r#"["digger", "digger"]"#, r#"["digger"]"#)));
    assert!(bad("", &RIVALS.replace("(0.1, 0.2)", "(0.1, 1.5)")));
    assert!(bad("", &RIVALS.replace("(90.0, 30.0), radius", "(190.0, 30.0), radius")));
    assert!(bad("", &RIVALS.replace(r#"base: "rogue", race: 1, sex: 0, kit: Class"#, r#"base: "mage", race: 1, sex: 0, kit: Class"#)));
    assert!(bad("", &RIVALS.replace(r#"("shovel", 1)"#, r#"("spade", 1)"#)));
    assert!(bad("", &RIVALS.replace("entries: [(10.0, 60.0)]", "entries: []")));
    assert!(bad(r#"leave_by: Some("dawn")"#, RIVALS));
    assert!(bad("engage_at: 1.5", RIVALS));
}
```

(The sixth `bad` line gives the blade a mage base: `sneaks: true` needs a class that can stealth.)

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`rivals` in `ContentSources`, `KitDef`, `playable` and the rest do not exist).

- [ ] **Step 2: The rivals' content types and their loading**

`crates/nightfall-sim/src/rivals/content.rs`:

```rust
//! Rivals as content (spec §7.4): `content/nightfall/rivals.ron` lists the kinds of rival (who
//! they look like, what they wear and carry, and their kit from the same ability system heroes
//! use), the parties they go about in (two or three of them, with an aggression), the points of
//! interest they know of, and where they come into the map; `rules.ron`'s `rivals:` holds the
//! numbers their bots go by. Each kind becomes a class of its own, appended to
//! [`Content::classes`] and marked [`ClassDef::rival`], so a rival is a hero to every system that
//! reads a class: its health, power, swing, bar and free kit. Giving a kind a whole hero class's
//! bar (`kit: Class("rogue")`) is a content change, for playtesting full kits against simplified
//! ones (spec §13).

use serde::Deserialize;

use crate::content::parse;
use crate::{ClassDef, Content, ContentError, Effect, V2};

fn one() -> f32 {
    1.0
}

/// The numbers rivals go by (`rules.ron`'s `rivals:`); each one left out takes its default.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RivalRules {
    /// How many parties come into a raid, fewest and most.
    pub parties_out: (u8, u8),
    /// Seconds into the raid the parties come, earliest and latest.
    pub arrive: (f32, f32),
    /// A rival's front cone, degrees (an enemy's `cone`).
    pub cone: f32,
    /// A party this aggressive or more fights a hero it is alerted to; below, it slips away.
    pub engage_at: f32,
    /// A party this aggressive or more hunts heroes once its looting is done; below, it leaves.
    pub hunt_at: f32,
    /// Seconds a hunt goes on before the party leaves.
    pub hunt_for: f32,
    /// A member below this share of health makes the party break off and flee.
    pub flee_health: f32,
    /// Seconds a flight lasts.
    pub flee_for: f32,
    /// Yards: a hero this near is fought whatever the party's aggression (it is cornered).
    pub avoid_within: f32,
    /// A member below this share of health uses a heal when it has one ready.
    pub heal_below: f32,
    /// A party whose weakest member is below this share of health does not go hunting.
    pub rest_below: f32,
    /// A member carrying this share of the run's `weight_full` or more counts as full.
    pub full_load: f32,
    /// How many points of interest a party visits, fewest and most.
    pub pois: (u8, u8),
    /// The night's stage (by key) from which every party heads out, or hunts.
    pub leave_by: Option<String>,
    /// Yards between members walking behind their leader.
    pub gap: f32,
    /// Path searches all rivals together may begin in one step.
    pub paths_per_step: u32,
    /// The chance a rival dodges a telegraph it stands in.
    pub dodge: f32,
    /// Seconds before a telegraph lands that a rival dodges it.
    pub dodge_at: f32,
    /// A finisher once this many combo points are on the target.
    pub finisher_at: u8,
    /// Rivals at 0 health go down as heroes do; when false they die at once.
    pub downed: bool,
    /// Yards from a container a rival searches it from (within the run's `reach`).
    pub loot_reach: f32,
    /// Seconds a party makes no headway toward a place before it gives that place up.
    pub give_up: f32,
}

impl Default for RivalRules {
    fn default() -> Self {
        Self {
            parties_out: (1, 2),
            arrive: (0.0, 300.0),
            cone: 120.0,
            engage_at: 0.5,
            hunt_at: 0.7,
            hunt_for: 180.0,
            flee_health: 0.3,
            flee_for: 8.0,
            avoid_within: 10.0,
            heal_below: 0.5,
            rest_below: 0.7,
            full_load: 0.8,
            pois: (2, 4),
            leave_by: Some("hunt".into()),
            gap: 2.5,
            paths_per_step: 2,
            dodge: 0.5,
            dodge_at: 0.6,
            finisher_at: 3,
            downed: false,
            loot_reach: 2.0,
            give_up: 15.0,
        }
    }
}

impl RivalRules {
    /// Why these numbers cannot work, if they cannot.
    fn check(&self) -> Result<(), String> {
        let shares = [
            self.engage_at,
            self.hunt_at,
            self.flee_health,
            self.heal_below,
            self.rest_below,
            self.full_load,
            self.dodge,
        ];
        if shares.iter().any(|s| !(0.0..=1.0).contains(s)) {
            return Err(
                "engage_at, hunt_at, flee_health, heal_below, rest_below, full_load and dodge are \
                 between 0 and 1"
                    .into(),
            );
        }
        let more = [
            self.hunt_for,
            self.flee_for,
            self.avoid_within,
            self.gap,
            self.dodge_at,
            self.loot_reach,
            self.give_up,
        ];
        if more.iter().any(|n| !(n.is_finite() && *n > 0.0)) {
            return Err(
                "hunt_for, flee_for, avoid_within, gap, dodge_at, loot_reach and give_up are more \
                 than 0"
                    .into(),
            );
        }
        if self.parties_out.0 > self.parties_out.1 {
            return Err("parties_out's fewest is more than its most".into());
        }
        if !(self.arrive.0 >= 0.0 && self.arrive.0 <= self.arrive.1) {
            return Err("arrive is two times from 0, the earliest first".into());
        }
        if self.pois.0 == 0 || self.pois.0 > self.pois.1 {
            return Err("pois is at least 1, the fewest first".into());
        }
        if !(self.cone > 0.0 && self.cone <= 360.0) {
            return Err("cone is more than 0 and at most 360 degrees".into());
        }
        if self.paths_per_step == 0 {
            return Err("paths_per_step is at least 1".into());
        }
        Ok(())
    }
}

/// Where a kind of rival's kit comes from.
#[derive(Clone, Debug, Deserialize)]
pub enum KitDef {
    /// A few abilities by key, put on the bar from action 1 in order: a simplified kit.
    Abilities(Vec<String>),
    /// The whole bar of a hero class, by key (spec §7.4: "a full Rogue or Mage kit").
    Class(String),
}

/// A kind of rival (spec §7.4).
#[derive(Clone, Debug, Deserialize)]
pub struct RivalKindDef {
    pub key: String,
    pub name: String,
    /// Names one of them goes by, picked by its party's dice; the kind's name when empty.
    #[serde(default)]
    pub names: Vec<String>,
    /// The hero class whose numbers it starts from: power, speed, swing, stealth.
    pub base: String,
    /// `ChrClasses.dbc` id for its look (the starting outfit it wears); the base's when absent.
    #[serde(default)]
    pub class_id: Option<u8>,
    pub race: u8,
    pub sex: u8,
    /// Its health; the base's when absent.
    #[serde(default)]
    pub hp: Option<f32>,
    pub kit: KitDef,
    /// Its telegraphed big move by key (spec §7.4), put on its bar after the kit when not on it.
    #[serde(default)]
    pub big: Option<String>,
    /// Yards it keeps from its target in a fight; 0 for melee.
    #[serde(default)]
    pub stand_off: f32,
    /// It goes in stealth when not fighting (its class must stealth).
    #[serde(default)]
    pub sneaks: bool,
    /// What it wears and carries by item key: worn where it goes, a bag in its first bag slot,
    /// the rest in its backpack.
    #[serde(default)]
    pub gear: Vec<(String, u16)>,
    /// A loot table by key, rolled into its backpack as it comes: what it found before.
    #[serde(default)]
    pub carry: Option<String>,
    /// Its class, an index into [`Content::classes`], made as content loads.
    #[serde(skip)]
    pub class: usize,
    /// `carry` as an index into the loot tables.
    #[serde(skip)]
    pub carry_table: Option<usize>,
    /// `big` as an index into the abilities.
    #[serde(skip)]
    pub big_ability: Option<usize>,
}

/// A kind of party (spec §7.4: grave robbers, Defias scavengers, Night Watch deserters).
#[derive(Clone, Debug, Deserialize)]
pub struct PartyDef {
    pub key: String,
    pub name: String,
    /// Its members by kind key: two or three, the first leading.
    pub members: Vec<String>,
    /// Its aggression from 0 to 1, rolled between these for each raid.
    pub aggression: (f32, f32),
    /// The chance it is out in a raid.
    pub odds: f32,
    /// `members` as indices into [`RivalContent::kinds`].
    #[serde(skip)]
    pub kinds: Vec<usize>,
}

/// A place rivals go to loot (spec §7.4, §9.2).
#[derive(Clone, Debug, Deserialize)]
pub struct PoiDef {
    pub key: String,
    pub name: String,
    pub at: (f32, f32),
    /// Yards around `at` a party searches.
    pub radius: f32,
    /// How much it draws a party; nearer draws more too.
    #[serde(default = "one")]
    pub worth: f32,
}

#[derive(Deserialize)]
struct RivalFile {
    kinds: Vec<RivalKindDef>,
    parties: Vec<PartyDef>,
    #[serde(default)]
    pois: Vec<PoiDef>,
    #[serde(default)]
    entries: Vec<(f32, f32)>,
}

/// The rivals' content, resolved.
#[derive(Clone, Debug, Default)]
pub struct RivalContent {
    pub kinds: Vec<RivalKindDef>,
    pub parties: Vec<PartyDef>,
    pub pois: Vec<PoiDef>,
    /// Where parties come into the map.
    pub entries: Vec<V2>,
    /// [`RivalRules::leave_by`] as a stage index.
    pub leave_by: Option<usize>,
}

impl RivalContent {
    /// `rivals.ron`'s text, read into `content`: every kind made a class appended to its
    /// classes. An empty text is no rivals.
    pub(crate) fn load(text: &str, content: &mut Content) -> Result<RivalContent, ContentError> {
        let invalid = |why: String| ContentError::Invalid(format!("rivals.ron: {why}"));
        content
            .rules
            .rivals
            .check()
            .map_err(|why| ContentError::Invalid(format!("rules.ron: rivals: {why}")))?;
        let leave_by = match content.rules.rivals.leave_by.clone() {
            Some(k) => Some(content.stage(&k).ok_or_else(|| {
                ContentError::Invalid(format!("rules.ron: rivals: leave_by names no stage \"{k}\""))
            })?),
            None => None,
        };
        if text.trim().is_empty() {
            return Ok(RivalContent {
                leave_by,
                ..RivalContent::default()
            });
        }
        let file: RivalFile = parse("rivals.ron", text)?;
        let mut kinds = file.kinds;
        for i in 0..kinds.len() {
            if kinds[..i].iter().any(|k| k.key == kinds[i].key) {
                return Err(invalid(format!("kind \"{}\" is listed twice", kinds[i].key)));
            }
            let class = class_of(&mut kinds[i], content).map_err(invalid)?;
            kinds[i].class = content.classes.len();
            content.classes.push(class);
        }
        let mut parties = file.parties;
        for i in 0..parties.len() {
            let p = &parties[i];
            if parties[..i].iter().any(|q| q.key == p.key) {
                return Err(invalid(format!("party \"{}\" is listed twice", p.key)));
            }
            if !(2..=3).contains(&p.members.len()) {
                return Err(invalid(format!("party \"{}\" has 2 or 3 members", p.key)));
            }
            let (lo, hi) = p.aggression;
            if !((0.0..=1.0).contains(&lo) && (0.0..=1.0).contains(&hi) && lo <= hi) {
                return Err(invalid(format!(
                    "party \"{}\": aggression is two numbers from 0 to 1, the lower first",
                    p.key
                )));
            }
            if !(0.0..=1.0).contains(&p.odds) {
                return Err(invalid(format!("party \"{}\": odds is from 0 to 1", p.key)));
            }
            let found: Result<Vec<usize>, String> = p
                .members
                .iter()
                .map(|m| {
                    kinds
                        .iter()
                        .position(|k| &k.key == m)
                        .ok_or_else(|| format!("party \"{}\": no kind \"{m}\"", p.key))
                })
                .collect();
            parties[i].kinds = found.map_err(invalid)?;
        }
        let bounds = &content.map.bounds;
        for (i, poi) in file.pois.iter().enumerate() {
            if file.pois[..i].iter().any(|q| q.key == poi.key) {
                return Err(invalid(format!("point \"{}\" is listed twice", poi.key)));
            }
            if !(poi.radius > 0.0 && poi.worth > 0.0) {
                return Err(invalid(format!("point \"{}\": radius and worth are more than 0", poi.key)));
            }
            if !bounds.contains(V2::new(poi.at.0, poi.at.1)) {
                return Err(invalid(format!("point \"{}\" is outside the map", poi.key)));
            }
        }
        let entries: Vec<V2> = file.entries.iter().map(|&(x, y)| V2::new(x, y)).collect();
        if let Some(e) = entries.iter().find(|e| !bounds.contains(**e)) {
            return Err(invalid(format!("entry ({}, {}) is outside the map", e.x, e.y)));
        }
        if !parties.is_empty() && entries.is_empty() {
            return Err(invalid("parties need at least one entry".into()));
        }
        Ok(RivalContent {
            kinds,
            parties,
            pois: file.pois,
            entries,
            leave_by,
        })
    }
}

/// The class kind `k` plays as: its base's, with its own name, look, health, bar and gear; and
/// its resolved parts filled in.
fn class_of(k: &mut RivalKindDef, content: &Content) -> Result<ClassDef, String> {
    let hero_class = |key: &str| content.class(key).filter(|&c| !content.classes[c].rival);
    let base = hero_class(&k.base)
        .ok_or_else(|| format!("kind \"{}\": no hero class \"{}\"", k.key, k.base))?;
    let mut class = content.classes[base].clone();
    class.key = format!("rival:{}", k.key);
    class.name = k.name.clone();
    class.description = String::new();
    if let Some(id) = k.class_id {
        class.class_id = id;
    }
    class.race = k.race;
    class.sex = k.sex;
    if let Some(hp) = k.hp {
        if !(hp > 0.0) {
            return Err(format!("kind \"{}\": hp is more than 0", k.key));
        }
        class.base_hp = hp;
    }
    class.bar = match &k.kit {
        KitDef::Abilities(keys) if keys.is_empty() => {
            return Err(format!("kind \"{}\": a kit has at least one ability", k.key));
        }
        KitDef::Abilities(keys) => keys
            .iter()
            .enumerate()
            .map(|(i, key)| ((i + 1) as u8, key.clone()))
            .collect(),
        KitDef::Class(key) => {
            let c = hero_class(key)
                .ok_or_else(|| format!("kind \"{}\": no hero class \"{key}\"", k.key))?;
            content.classes[c].bar.clone()
        }
    };
    k.big_ability = match &k.big {
        None => None,
        Some(key) => {
            let a = content
                .ability(key)
                .ok_or_else(|| format!("kind \"{}\": no ability \"{key}\"", k.key))?;
            if content.abilities[a].telegraph.is_none() {
                return Err(format!(
                    "kind \"{}\": its big move \"{key}\" needs a telegraph (spec §7.4)",
                    k.key
                ));
            }
            if !class.bar.iter().any(|(_, b)| b == key) {
                let last = class.bar.iter().map(|(s, _)| *s).filter(|s| *s <= 12).max();
                class.bar.push((last.unwrap_or(0) + 1, key.clone()));
            }
            Some(a)
        }
    };
    let kit: Result<Vec<(u8, usize)>, String> = class
        .bar
        .iter()
        .map(|(slot, key)| {
            content
                .ability(key)
                .map(|a| (*slot, a))
                .ok_or_else(|| format!("kind \"{}\": no ability \"{key}\"", k.key))
        })
        .collect();
    class.kit = kit?;
    if k.sneaks {
        let can = class.stealth
            && class
                .kit
                .iter()
                .any(|&(_, a)| content.abilities[a].has(|e| matches!(e, Effect::Stealth)));
        if !can {
            return Err(format!("kind \"{}\" sneaks, so its class and kit stealth", k.key));
        }
    }
    if !(k.stand_off.is_finite() && k.stand_off >= 0.0) {
        return Err(format!("kind \"{}\": stand_off is 0 or more", k.key));
    }
    let items: Result<Vec<(usize, u16)>, String> = k
        .gear
        .iter()
        .map(|(key, n)| {
            content
                .item(key)
                .map(|i| (i, *n))
                .ok_or_else(|| format!("kind \"{}\": no item \"{key}\"", k.key))
        })
        .collect();
    class.free_kit = k.gear.clone();
    class.free_items = items?;
    k.carry_table = match &k.carry {
        None => None,
        Some(t) => Some(
            content
                .loot
                .tables
                .iter()
                .position(|x| &x.key == t)
                .ok_or_else(|| format!("kind \"{}\": no loot table \"{t}\"", k.key))?,
        ),
    };
    class.rival = true;
    Ok(class)
}

impl Content {
    /// The classes a player may pick: every class but the rivals', with its index.
    pub fn playable(&self) -> impl Iterator<Item = (usize, &ClassDef)> + '_ {
        self.classes.iter().enumerate().filter(|(_, c)| !c.rival)
    }

    /// The kind of rival with this key, by index.
    pub fn rival_kind(&self, key: &str) -> Option<usize> {
        self.rivals.kinds.iter().position(|k| k.key == key)
    }

    /// The kind of party with this key, by index.
    pub fn party(&self, key: &str) -> Option<usize> {
        self.rivals.parties.iter().position(|p| p.key == key)
    }
}
```

`crates/nightfall-sim/src/rivals/mod.rs`:

```rust
//! Rivals (spec §7.4): parties of two or three bot adventurers with goals of their own, in the
//! manner of Single Player Tarkov's questing bots (§1.1). This task holds their content; the
//! later ones make each a hero of another side, driven by its bot.

mod content;
#[cfg(test)]
mod tests;

pub use content::{KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalRules};
```

- [ ] **Step 3: Wire it into the content**

In `crates/nightfall-sim/src/content.rs`:

- Add `use crate::rivals::{RivalContent, RivalRules};` beside the other imports.
- `fn parse` becomes `pub(crate) fn parse` (the rivals' file is read the same way).
- `ClassDef` gains, as its last field:

```rust
    /// A rival kind's class (spec §7.4), appended after the heroes' as `rivals.ron` loads: never
    /// offered to a player.
    #[serde(skip)]
    pub rival: bool,
```

- `Rules` gains, as its last field:

```rust
    /// The rivals' numbers (spec §7.4).
    #[serde(default)]
    pub rivals: RivalRules,
```

- `ContentSources` gains, as its last field, `pub rivals: &'a str,` (doc: `` `rivals.ron`; an empty text is no rivals. ``).
- `Content` gains, as its last field, `pub rivals: RivalContent,`.
- At the end of `Content::from_ron`, the closing `Ok(Self { … })` becomes `let mut content = Self { …, rivals: RivalContent::default() };` (the same fields, plus that one), followed by:

```rust
        // Last: the rivals' kinds become classes, built from the heroes' classes, abilities,
        // items, loot tables and the night's stages above.
        let rivals = RivalContent::load(src.rivals, &mut content)?;
        content.rivals = rivals;
        Ok(content)
```

In `crates/nightfall-sim/src/abilities.rs`, `Effect` gains after `ConjureWater`:

```rust
    /// Health back to the caster, `min` to `max` (on each tick of a channel): a rival's
    /// bandage, the heal its bot picks by tag (spec §7.4).
    Heal { min: f32, max: f32 },
```

and `AbilityDef::check`'s match on effects gains the arm `Effect::Heal { min, max } => min >= 0.0 && max >= min && max > 0.0,`.

In `crates/nightfall-sim/src/lib.rs`: add `mod rivals;` beside the other modules and `pub use rivals::{KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalRules};`.

Every other `ContentSources { … }` literal (see Files) gains `rivals: "",`.

In `crates/nightfall-sim/tests/shipped_content.rs`, `shipped()`'s `ContentSources` gains `rivals: include_str!("../../../content/nightfall/rivals.ron"),`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim rivals::`
Expected: PASS (4 tests). The shipped-content test fails until Step 5 writes `rivals.ron`.

- [ ] **Step 5: The shipped rivals**

Append to `content/nightfall/abilities.ron`, inside its list, after the last ability:

```ron
    // Rivals' abilities (spec §7.4), picked by their bots by tag. Spell ids are for the look and
    // the tooltip only; measure: each against the trimmed data's Spell.dbc.
    (key: "robber_hack", name: "Shovel Hack", spell: 285, aim: Enemy, range: 5.0, cost: 30.0,
     gcd: 1.0, facing: Front, attack: true, effects: [Weapon(percent: 100.0, bonus: 25.0)],
     tags: [Nuke]),
    (key: "robber_dirt", name: "Grave Dirt", spell: 2094, aim: Enemy, range: 8.0, cost: 25.0,
     cooldown: 25.0, gcd: 1.0, sound: Quiet,
     effects: [Control(kind: Incapacitate, secs: 4.0)], tags: [Control]),
    (key: "rival_bandage", name: "Linen Bandage", spell: 3276, aim: Me, channel: 6.0, ticks: 6,
     cooldown: 60.0, moving: Rooted, sound: Quiet, effects: [Heal(min: 45.0, max: 55.0)],
     tags: [Heal]),
    (key: "robber_shovel_smash", name: "Shovel Smash", spell: 6343, aim: Me, radius: 6.0,
     cost: 30.0, cooldown: 20.0, gcd: 1.0, moving: Rooted, sound: Loud,
     telegraph: Some((shape: Circle(radius: 6.0), windup: 1.2, on_self: true)),
     effects: [Damage(min: 140.0, max: 180.0), Slow(by: 0.3, secs: 6.0)], tags: [Nuke]),
    (key: "defias_stab", name: "Defias Stab", spell: 1752, aim: Enemy, range: 5.0, cost: 40.0,
     gcd: 1.0, facing: Front, attack: true, effects: [Weapon(percent: 120.0, bonus: 20.0)],
     tags: [Nuke]),
    (key: "defias_flurry", name: "Blade Flurry", spell: 1680, aim: Me, radius: 5.0, cost: 35.0,
     cooldown: 18.0, gcd: 1.0, moving: Rooted, sound: Normal,
     telegraph: Some((shape: Circle(radius: 5.0), windup: 1.2, on_self: true)),
     effects: [Weapon(percent: 160.0, bonus: 30.0)], tags: [Nuke]),
    (key: "deserter_shot", name: "Shoot", spell: 3018, aim: Enemy, range: 30.0, cost: 20.0,
     cast: 1.5, gcd: 1.0, moving: Slowed, facing: Front, effects: [Damage(min: 55.0, max: 75.0)],
     tags: [Nuke]),
    (key: "deserter_net", name: "Net", spell: 6533, aim: Enemy, range: 15.0, cost: 20.0,
     cooldown: 25.0, gcd: 1.0, effects: [Control(kind: Root, secs: 5.0)], tags: [Control]),
    (key: "deserter_volley", name: "Volley", spell: 1510, aim: Enemy, range: 30.0, cost: 40.0,
     cooldown: 22.0, gcd: 1.0, moving: Rooted, sound: Loud,
     telegraph: Some((shape: Circle(radius: 5.0), windup: 1.5)),
     effects: [Damage(min: 150.0, max: 190.0)], tags: [Nuke]),
```

Append to `content/nightfall/rules.ron`, inside its outer parentheses, after the last section:

```ron
    // The rivals' bots (spec §7.4). Starting values to playtest; see RivalRules for each.
    rivals: (
        parties_out: (1, 2),
        arrive: (0.0, 300.0),
        engage_at: 0.5,
        hunt_at: 0.7,
        hunt_for: 180.0,
        flee_health: 0.3,
        pois: (2, 4),
        leave_by: Some("hunt"),
        dodge: 0.5,
        downed: false,
    ),
```

`content/nightfall/rivals.ron`:

```ron
// Nightfall's rivals (spec §7.4): bot adventurers with goals of their own. Each kind becomes a
// class of its own ("rival:<key>") built on `base`, with a kit from abilities.ron that its bot
// picks from by tag (opener, nuke, control, escape, heal) and one telegraphed `big` move.
// `kit: Class("rogue")` gives a kind the rogue's whole bar instead, to playtest full kits
// (spec §13): the "cutthroats" party has those, and odds 0, so only NIGHTFALL_RIVALS brings it.
// Gear and the `carry` tables are items.ron's and loot.ron's own; killed, a rival drops it all.
// Every place here is a measure: placeholder near the first entry (-10051.8, -792.5) until
// Duskwood's points of interest are walked with NIGHTFALL_TRACE_POS=1.
(
    kinds: [
        (key: "grave_robber", name: "Grave Robber", names: ["Silas Dunmore", "Old Mott",
         "Jeb Crowley"], base: "rogue", class_id: Some(1), race: 1, sex: 0, hp: Some(900.0),
         kit: Abilities(["robber_hack", "robber_dirt", "rival_bandage"]),
         big: Some("robber_shovel_smash"),
         gear: [("worn_shortsword", 1), ("gravediggers_boots", 1), ("linen_pouch", 1),
                ("tough_bread", 2)],
         carry: Some("grave")),
        (key: "defias_scavenger", name: "Defias Scavenger", names: ["Defias Scavenger"],
         base: "rogue", race: 1, sex: 0, hp: Some(850.0),
         kit: Abilities(["stealth", "ambush", "defias_stab", "sprint"]),
         big: Some("defias_flurry"), sneaks: true,
         gear: [("raven_hill_dirk", 1), ("worn_dagger", 1), ("scavengers_leggings", 1),
                ("duskwood_satchel", 1)],
         carry: Some("crate")),
        (key: "watch_deserter", name: "Night Watch Deserter", names: ["Deserter Harlan",
         "Deserter Bree"], base: "rogue", class_id: Some(1), race: 1, sex: 1, hp: Some(950.0),
         kit: Abilities(["deserter_shot", "deserter_net", "rival_bandage"]),
         big: Some("deserter_volley"), stand_off: 20.0,
         gear: [("night_watch_blade", 1), ("watchmans_tunic", 1), ("linen_pouch", 1)],
         carry: Some("chest")),
        (key: "defias_cutthroat", name: "Defias Cutthroat", base: "rogue", race: 1, sex: 0,
         kit: Class("rogue"), sneaks: true,
         gear: [("raven_hill_dirk", 1), ("worn_dagger", 1), ("duskwood_satchel", 1)],
         carry: Some("crate")),
    ],
    parties: [
        (key: "grave_robbers", name: "Grave robbers", members: ["grave_robber", "grave_robber"],
         aggression: (0.1, 0.4), odds: 0.6),
        (key: "defias_scavengers", name: "Defias scavengers",
         members: ["defias_scavenger", "defias_scavenger", "grave_robber"],
         aggression: (0.5, 0.9), odds: 0.5),
        (key: "watch_deserters", name: "Night Watch deserters",
         members: ["watch_deserter", "watch_deserter"], aggression: (0.3, 0.7), odds: 0.5),
        (key: "cutthroats", name: "Defias cutthroats",
         members: ["defias_cutthroat", "defias_cutthroat"], aggression: (0.6, 0.9), odds: 0.0),
    ],
    // measure: Raven Hill, the Rotting Orchard, Brightwood Grove, Addle's Stead, Yorgen Farmstead.
    pois: [
        (key: "raven_hill", name: "Raven Hill cemetery", at: (-10040.0, -760.0), radius: 12.0,
         worth: 2.0),
        (key: "rotting_orchard", name: "The Rotting Orchard", at: (-10070.0, -830.0),
         radius: 10.0),
        (key: "brightwood", name: "Brightwood Grove", at: (-10020.0, -820.0), radius: 10.0),
        (key: "addles_stead", name: "Addle's Stead", at: (-10090.0, -770.0), radius: 10.0),
        (key: "yorgen", name: "Yorgen Farmstead", at: (-10010.0, -780.0), radius: 10.0,
         worth: 1.5),
    ],
    // measure: the west road, the southern woods and the Deadwind road.
    entries: [(-10110.0, -800.0), (-10060.0, -870.0), (-9990.0, -820.0)],
)
```

Append to `crates/nightfall-sim/tests/shipped_content.rs`:

```rust
#[test]
fn the_rivals_load_with_simplified_kits_and_a_full_one() {
    use nightfall_sim::KitDef;

    let c = shipped();
    assert!(c.rivals.kinds.len() >= 3 && c.rivals.parties.len() >= 3);
    assert!(!c.rivals.entries.is_empty() && !c.rivals.pois.is_empty());
    assert_eq!(c.playable().count(), 2, "only the rogue and the mage are offered");
    for k in &c.rivals.kinds {
        let class = &c.classes[k.class];
        assert!(class.rival);
        if let KitDef::Abilities(keys) = &k.kit {
            assert!((3..=4).contains(&keys.len()), "{}: 3 or 4 abilities", k.key);
            let big = k.big_ability.expect("a simplified kit has its big move");
            assert!(c.abilities[big].telegraph.is_some());
        }
    }
    assert!(c.rivals.kinds.iter().any(|k| matches!(k.kit, KitDef::Class(_))));
    for p in &c.rivals.parties {
        assert!((2..=3).contains(&p.kinds.len()));
    }
}
```

- [ ] **Step 6: The client reads rivals.ron and offers only heroes' classes**

In `crates/benilla-app/src/nightfall/mod.rs`'s `prepare`, read `rivals.ron` as the other files are (`let rivals = read("rivals.ron", include_str!("../../../../content/nightfall/rivals.ron"))?;`), pass `rivals: &rivals` in its `ContentSources`, and put it last in `CONTENT_TEXTS`, whose array (and `content_texts`' return type) grows from 12 to 13 strings; the doc above `CONTENT_TEXTS` gains `rivals` at the end of its list of files.

In `crates/benilla-app/src/nightfall/menu.rs`, the class buttons' loop `for (i, class) in content.classes.iter().enumerate()` becomes `for (i, class) in content.playable()`. If the inn's bar (`inn.rs`, sub-project 5b) has a loop of its own over `content.classes` for its class buttons, it takes the same change (`grep -rn "classes.iter().enumerate()" crates/benilla-app/src/nightfall`).

In `crates/benilla-app/src/nightfall/night.rs`'s test `shipped()`, its `ContentSources` gains `rivals: include_str!("../../../../content/nightfall/rivals.ron"),`.

- [ ] **Step 7: Run every test**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 4 new rivals tests, the new shipped-content test, every earlier one).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim content/nightfall/rivals.ron content/nightfall/rules.ron content/nightfall/abilities.ron crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: rivals as content; each kind a class of its own; a heal effect

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Abilities by tag: a rival's choice in a fight

**Files:**
- Create: `crates/nightfall-sim/src/rivals/kit.rs`
- Modify: `crates/nightfall-sim/src/rivals/mod.rs`, `crates/nightfall-sim/src/rivals/tests.rs`, `crates/nightfall-sim/src/combat/mod.rs` (re-exports)

**Interfaces:**
- Consumes: `AbilityDef::{aim, range, radius, telegraph, facing, needs, tags, cost, gcd, has}`, `Aim`, `Facing`, `Effect::Stealth`, `Shape::reach`, `Tag`, `HeroFight::{up, casting, cooldowns, gcd, power}`, `ClassDef::kit`, `heroes::NEEDS_SIGHT` (combat plan); `RivalRules::{heal_below, flee_health, finisher_at}`, `RivalKindDef::big_ability` (Task 1).
- Produces:
  - Crate-only `rivals::kit::{KitSlot { slot, ability, tags, reach, big, behind, opener, sight }, Moment { distance, size, health, stealthed, unaware, held, behind, in_sight, combo }, Choice { slot, ability, why }}`.
  - Crate-only `kit_of(class: usize, big: Option<usize>, content: &Content) -> Vec<KitSlot>`; `ready(fight: &HeroFight, ability: usize, a: &AbilityDef) -> bool`; `choose(kit: &[KitSlot], m: &Moment, r: &RivalRules, ready: impl Fn(usize) -> bool) -> Option<Choice>`.
  - Crate-only re-export `crate::combat::{Aimed, MELEE, NEEDS_SIGHT}`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/rivals/kit.rs` (its tests now, the code in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn slot(slot: u8, ability: usize, tags: &[Tag], reach: f32) -> KitSlot {
        KitSlot {
            slot,
            ability,
            tags: tags.to_vec(),
            reach,
            big: false,
            behind: false,
            opener: false,
            sight: false,
        }
    }

    /// A simplified kit: a nuke, a control, a heal, the big move and an escape.
    fn kit() -> Vec<KitSlot> {
        vec![
            slot(1, 0, &[Tag::Nuke], 5.0),
            slot(2, 1, &[Tag::Control], 8.0),
            slot(3, 2, &[Tag::Heal], 0.0),
            KitSlot {
                big: true,
                ..slot(4, 3, &[Tag::Nuke], 6.0)
            },
            slot(5, 4, &[Tag::Escape], 0.0),
        ]
    }

    fn near() -> Moment {
        Moment {
            distance: 2.0,
            size: 1.5,
            health: 1.0,
            in_sight: true,
            ..Moment::default()
        }
    }

    fn r() -> RivalRules {
        RivalRules::default()
    }

    fn picked(kit: &[KitSlot], m: Moment, ready: &[usize]) -> Option<u8> {
        choose(kit, &m, &r(), |a| ready.contains(&a)).map(|c| c.slot)
    }

    #[test]
    fn a_hurt_rival_heals_and_one_worse_off_without_a_heal_runs() {
        let all = [0, 1, 2, 3, 4];
        let hurt = Moment { health: 0.4, ..near() };
        assert_eq!(picked(&kit(), hurt, &all), Some(3));
        let worse = Moment { health: 0.2, ..near() };
        assert_eq!(picked(&kit(), worse, &[0, 1, 3, 4]), Some(5));
        assert_eq!(
            choose(&kit(), &worse, &r(), |a| a == 4).map(|c| c.why),
            Some(Tag::Escape)
        );
    }

    #[test]
    fn a_free_target_is_held_and_a_held_one_takes_the_big_move() {
        let all = [0, 1, 2, 3, 4];
        assert_eq!(picked(&kit(), near(), &all), Some(2));
        let held = Moment { held: true, ..near() };
        assert_eq!(picked(&kit(), held, &all), Some(4));
        assert_eq!(picked(&kit(), held, &[0, 1, 2, 4]), Some(1));
    }

    #[test]
    fn nothing_out_of_reach_is_pressed() {
        // 10 yards: past the control's 8 + 1.5, the big move's 6 + 1.5 and the nuke's 5 + 1.5.
        let far = Moment { distance: 10.0, ..near() };
        assert_eq!(picked(&kit(), far, &[0, 1, 2, 3, 4]), None);
        let closer = Moment { distance: 9.0, ..near() };
        assert_eq!(picked(&kit(), closer, &[0, 1, 2, 3, 4]), Some(2));
    }

    #[test]
    fn nothing_ready_is_nothing_pressed() {
        assert_eq!(picked(&kit(), near(), &[]), None);
    }

    #[test]
    fn an_opener_only_from_stealth_on_the_unaware() {
        let mut k = kit();
        k.push(KitSlot {
            opener: true,
            ..slot(74, 5, &[Tag::Opener], 5.0)
        });
        let ready = [0, 1, 2, 3, 4, 5];
        let sneaking = Moment {
            stealthed: true,
            unaware: true,
            ..near()
        };
        assert_eq!(picked(&k, sneaking, &ready), Some(74));
        let seen = Moment {
            stealthed: true,
            ..near()
        };
        assert_ne!(picked(&k, seen, &ready), Some(74));
        let open = Moment {
            unaware: true,
            ..near()
        };
        assert_ne!(picked(&k, open, &ready), Some(74));
    }

    #[test]
    fn a_finisher_on_enough_combo_points_else_a_builder() {
        let k = vec![
            slot(2, 0, &[Tag::Builder], 5.0),
            slot(3, 1, &[Tag::Finisher], 5.0),
        ];
        assert_eq!(picked(&k, Moment { combo: 2, ..near() }, &[0, 1]), Some(2));
        assert_eq!(picked(&k, Moment { combo: 3, ..near() }, &[0, 1]), Some(3));
    }

    #[test]
    fn behind_only_waits_for_the_back_and_far_needs_a_clear_line() {
        let k = vec![
            KitSlot {
                behind: true,
                ..slot(1, 0, &[Tag::Nuke], 5.0)
            },
            KitSlot {
                sight: true,
                ..slot(2, 1, &[Tag::Nuke], 30.0)
            },
        ];
        assert_eq!(picked(&k, near(), &[0, 1]), Some(2));
        assert_eq!(picked(&k, Moment { behind: true, ..near() }, &[0, 1]), Some(1));
        let blind = Moment {
            in_sight: false,
            distance: 20.0,
            ..near()
        };
        assert_eq!(picked(&k, blind, &[0, 1]), None);
    }
}
```

Append to `crates/nightfall-sim/src/rivals/tests.rs`:

```rust
#[test]
fn the_kit_leaves_stealth_out_and_marks_the_big_move() {
    use super::kit::kit_of;
    use crate::Tag;

    let c = content("", "");
    let digger = &c.rivals.kinds[c.rival_kind("digger").unwrap()];
    let kit = kit_of(digger.class, digger.big_ability, &c);
    assert_eq!(kit.iter().map(|k| k.slot).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    assert!(kit[3].big && kit[3].tags.contains(&Tag::Nuke));
    assert_eq!(kit[3].reach, 6.0, "an on-self telegraph reaches its own size");
    assert!(!kit[0].sight && kit[0].reach == 5.0);
    let blade = &c.rivals.kinds[c.rival_kind("blade").unwrap()];
    let kit = kit_of(blade.class, blade.big_ability, &c);
    let keys: Vec<&str> = kit.iter().map(|k| c.abilities[k.ability].key.as_str()).collect();
    assert_eq!(keys, vec!["strike", "finish", "ambush"], "Stealth is pressed by count");
    assert!(kit[2].opener);
}
```

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`kit` and its items do not exist).

- [ ] **Step 2: Export the fight's constants to the rivals**

In `crates/nightfall-sim/src/combat/mod.rs`, beside its other `use` lines, add `pub(crate) use heroes::{Aimed, MELEE, NEEDS_SIGHT};` (leave out any of the three it already re-exports).

- [ ] **Step 3: Write the choice**

Put above the tests in `crates/nightfall-sim/src/rivals/kit.rs`:

```rust
//! A rival's ability by tag (spec §7.4): its bot reads its kit by tag (opener, builder,
//! finisher, nuke, control, escape, heal) and at each moment of a fight presses the first that
//! fits, in a fixed order: a heal when hurt, an escape when worse off, an opener from stealth on
//! the unaware, a hold on a free target, the big move when it reaches, a finisher on enough
//! combo points, else a nuke or a builder. Pure: the bot gives it the moment and what is ready,
//! and the fight's own checks still decide whether the press goes off. A full hero kit reads the
//! same way, so a kind given the whole rogue bar plays it by its tags.

use crate::combat::NEEDS_SIGHT;
use crate::{AbilityDef, Aim, Content, Effect, Facing, HeroFight, RivalRules, Tag};

/// One ability on a rival's bar, as its bot reads it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KitSlot {
    /// The WoW action id it sits on, which a press names.
    pub(crate) slot: u8,
    /// Index into the content's abilities.
    pub(crate) ability: usize,
    pub(crate) tags: Vec<Tag>,
    /// Yards it reaches its target from: its range, or for one aimed at the rival itself its
    /// radius (its telegraph's reach when it has one).
    pub(crate) reach: f32,
    /// The kind's telegraphed big move.
    pub(crate) big: bool,
    /// Only from behind the target.
    pub(crate) behind: bool,
    /// Only from stealth, or only on a target not alerted to the rival.
    pub(crate) opener: bool,
    /// Its range needs a clear line (past the fight's `NEEDS_SIGHT`).
    pub(crate) sight: bool,
}

/// What a rival knows of its moment in a fight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Moment {
    /// Yards to the target, centre to centre.
    pub(crate) distance: f32,
    /// The target's size, added to every reach (`CombatRules::reach`).
    pub(crate) size: f32,
    /// Its own share of health.
    pub(crate) health: f32,
    pub(crate) stealthed: bool,
    /// The target is not alerted to it, so an opener may land (spec §5.4).
    pub(crate) unaware: bool,
    /// The target is held already.
    pub(crate) held: bool,
    /// It stands behind the target.
    pub(crate) behind: bool,
    /// A clear line to the target.
    pub(crate) in_sight: bool,
    /// Combo points on the target.
    pub(crate) combo: u8,
}

/// What a rival presses: the bar slot, its ability, and the tag it was picked by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Choice {
    pub(crate) slot: u8,
    pub(crate) ability: usize,
    pub(crate) why: Tag,
}

/// The kit of `class`: every ability on its bar but Stealth's toggle (its bot presses Stealth by
/// count, as a player does), in bar order; `big` marks the big move.
pub(crate) fn kit_of(class: usize, big: Option<usize>, content: &Content) -> Vec<KitSlot> {
    content.classes[class]
        .kit
        .iter()
        .filter(|&&(_, a)| !content.abilities[a].has(|e| matches!(e, Effect::Stealth)))
        .map(|&(slot, a)| {
            let def = &content.abilities[a];
            KitSlot {
                slot,
                ability: a,
                tags: def.tags.clone(),
                reach: reach_of(def),
                big: Some(a) == big,
                behind: def.facing == Facing::Behind,
                opener: def.needs.stealth || def.needs.unaware,
                sight: def.range > NEEDS_SIGHT,
            }
        })
        .collect()
}

/// How far an ability reaches its target from.
fn reach_of(a: &AbilityDef) -> f32 {
    match a.aim {
        Aim::Me => a.telegraph.map_or(a.radius, |t| t.shape.reach()),
        Aim::Enemy | Aim::Ground => a.range,
    }
}

/// Whether `fight` may press `ability` now as far as its clocks and power go: up, no cast under
/// way, its own cooldown and the global one done, and power enough. The fight's checks (range,
/// facing, sight) come after, as for a player's press.
pub(crate) fn ready(fight: &HeroFight, ability: usize, a: &AbilityDef) -> bool {
    fight.up()
        && fight.casting.is_none()
        && !fight.cooldowns.iter().any(|c| usize::from(c.ability) == ability)
        && !(a.gcd > 0.0 && fight.gcd > 0.0)
        && fight.power + 1e-3 >= a.cost
}

/// The ability to press now, if any fits (see the module's order).
pub(crate) fn choose(
    kit: &[KitSlot],
    m: &Moment,
    r: &RivalRules,
    ready: impl Fn(usize) -> bool,
) -> Option<Choice> {
    let reaches = |k: &KitSlot| {
        m.distance <= k.reach + m.size && (!k.sight || m.in_sight) && (!k.behind || m.behind)
    };
    let pick = |want: &dyn Fn(&KitSlot) -> bool, why: Tag| {
        kit.iter()
            .find(|k| want(k) && ready(k.ability))
            .map(|k| Choice {
                slot: k.slot,
                ability: k.ability,
                why,
            })
    };
    let has = |k: &KitSlot, t: Tag| k.tags.contains(&t);
    if m.health < r.heal_below {
        if let Some(c) = pick(&|k| has(k, Tag::Heal), Tag::Heal) {
            return Some(c);
        }
    }
    if m.health < r.flee_health {
        if let Some(c) = pick(&|k| has(k, Tag::Escape), Tag::Escape) {
            return Some(c);
        }
    }
    if m.stealthed && m.unaware {
        if let Some(c) = pick(&|k| has(k, Tag::Opener) && reaches(k), Tag::Opener) {
            return Some(c);
        }
    }
    if !m.held {
        if let Some(c) = pick(&|k| has(k, Tag::Control) && !k.opener && reaches(k), Tag::Control)
        {
            return Some(c);
        }
    }
    if let Some(c) = pick(&|k| k.big && reaches(k), Tag::Nuke) {
        return Some(c);
    }
    if m.combo >= r.finisher_at {
        if let Some(c) = pick(&|k| has(k, Tag::Finisher) && reaches(k), Tag::Finisher) {
            return Some(c);
        }
    }
    if let Some(c) = pick(&|k| has(k, Tag::Nuke) && !k.big && !k.opener && reaches(k), Tag::Nuke) {
        return Some(c);
    }
    pick(&|k| has(k, Tag::Builder) && !k.opener && reaches(k), Tag::Builder)
}
```

In `crates/nightfall-sim/src/rivals/mod.rs` add `mod kit;` below `mod content;`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim rivals::`
Expected: PASS (7 new tests in `rivals::kit::tests`, 1 new in `rivals::tests`). A warning that `ready` is unused goes when Task 8 drives rivals.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/rivals crates/nightfall-sim/src/combat/mod.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: a rival's ability by tag

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: A party's goals, its points of interest, and walking

**Files:**
- Create: `crates/nightfall-sim/src/rivals/goals.rs`
- Modify: `crates/nightfall-sim/src/rivals/mod.rs`

**Interfaces:**
- Consumes: `PoiDef`, `RivalRules` (Task 1); `V2`, `V3`.
- Produces: crate-only `rivals::goals::{Goal::{Wait, Loot { poi }, Hold { face }, Check { at }, Fight, Flee { from, left }, Hunt { at, left }, Extract { exit }}, FoeSeen { at, distance }, PartyView { aggression, health, full, late, pois_left, looting, next_poi, foe, hurt_by_hero, monsters, wary, lead, hunted, exit }, WALK_RATIO, decide(Goal, &PartyView, &RivalRules) -> Goal, next_poi(&[PoiDef], skip: &[usize], at: V2, roll: f32) -> Option<usize>, formation_spot(leader: V2, facing: V2, rank: usize, gap: f32) -> V2, walk_along(from: V3, path: &[V3], next: &mut usize, step: f32) -> V3, away_from(at: V2, from: V2, distance: f32) -> V2}`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/rivals/goals.rs` (its tests now, the code in Step 2):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> RivalRules {
        RivalRules::default()
    }

    /// A healthy party with points left and nothing about.
    fn calm(aggression: f32) -> PartyView {
        PartyView {
            aggression,
            health: 1.0,
            pois_left: 2,
            next_poi: Some(1),
            exit: Some(0),
            ..PartyView::default()
        }
    }

    fn foe(distance: f32) -> Option<FoeSeen> {
        Some(FoeSeen {
            at: V3::new(distance, 0.0, 0.0),
            distance,
        })
    }

    #[test]
    fn a_cautious_party_slips_away_from_a_hero_and_a_bold_one_fights() {
        let shy = PartyView { foe: foe(30.0), ..calm(0.2) };
        assert!(matches!(decide(Goal::Wait, &shy, &r()), Goal::Flee { .. }));
        let bold = PartyView { foe: foe(30.0), ..calm(0.8) };
        assert_eq!(decide(Goal::Wait, &bold, &r()), Goal::Fight);
        let cornered = PartyView { foe: foe(5.0), ..calm(0.2) };
        assert_eq!(decide(Goal::Wait, &cornered, &r()), Goal::Fight);
        let struck = PartyView {
            foe: foe(30.0),
            hurt_by_hero: true,
            ..calm(0.2)
        };
        assert_eq!(decide(Goal::Wait, &struck, &r()), Goal::Fight);
    }

    #[test]
    fn a_hurt_party_flees_and_a_flight_runs_its_course() {
        let hurt = PartyView {
            foe: foe(10.0),
            health: 0.2,
            ..calm(0.9)
        };
        let Goal::Flee { left, .. } = decide(Goal::Fight, &hurt, &r()) else {
            panic!("a party this hurt breaks off");
        };
        assert_eq!(left, r().flee_for);
        let fleeing = Goal::Flee {
            from: V2::ZERO,
            left: 3.0,
        };
        assert_eq!(decide(fleeing, &calm(0.9), &r()), fleeing);
        let over = Goal::Flee {
            from: V2::ZERO,
            left: 0.0,
        };
        assert_eq!(decide(over, &calm(0.9), &r()), Goal::Loot { poi: 1 });
    }

    #[test]
    fn monsters_are_fought_and_a_wary_party_stops_or_checks() {
        let beset = PartyView { monsters: true, ..calm(0.1) };
        assert_eq!(decide(Goal::Loot { poi: 1 }, &beset, &r()), Goal::Fight);
        let at = V3::new(4.0, 5.0, 0.0);
        let wary = PartyView { wary: Some((at, false)), ..calm(0.1) };
        assert_eq!(
            decide(Goal::Loot { poi: 1 }, &wary, &r()),
            Goal::Hold { face: at.ground() }
        );
        let checking = PartyView { wary: Some((at, true)), ..calm(0.1) };
        assert_eq!(decide(Goal::Loot { poi: 1 }, &checking, &r()), Goal::Check { at });
    }

    #[test]
    fn a_party_loots_until_it_is_full_late_or_done() {
        let looting = PartyView { looting: Some(0), ..calm(0.1) };
        assert_eq!(decide(Goal::Wait, &looting, &r()), Goal::Loot { poi: 0 });
        assert_eq!(decide(Goal::Wait, &calm(0.1), &r()), Goal::Loot { poi: 1 });
        for done in [
            PartyView { full: true, ..calm(0.1) },
            PartyView { late: true, ..calm(0.1) },
            PartyView { pois_left: 0, ..calm(0.1) },
            PartyView { next_poi: None, ..calm(0.1) },
        ] {
            assert_eq!(decide(Goal::Wait, &done, &r()), Goal::Extract { exit: 0 });
        }
    }

    #[test]
    fn a_bold_party_hunts_once_then_leaves() {
        let lead = Some(V3::new(50.0, 50.0, 0.0));
        let done = PartyView {
            pois_left: 0,
            lead,
            ..calm(0.9)
        };
        assert_eq!(
            decide(Goal::Wait, &done, &r()),
            Goal::Hunt {
                at: lead.unwrap(),
                left: r().hunt_for
            }
        );
        let newer = V3::new(60.0, 40.0, 0.0);
        let on = Goal::Hunt {
            at: lead.unwrap(),
            left: 10.0,
        };
        let heard = PartyView {
            lead: Some(newer),
            hunted: true,
            ..done
        };
        assert_eq!(decide(on, &heard, &r()), Goal::Hunt { at: newer, left: 10.0 });
        let spent = Goal::Hunt {
            at: newer,
            left: 0.0,
        };
        assert_eq!(decide(spent, &heard, &r()), Goal::Extract { exit: 0 });
        let no_lead = PartyView { lead: None, ..done };
        assert_eq!(decide(Goal::Wait, &no_lead, &r()), Goal::Extract { exit: 0 });
        let weak = PartyView { health: 0.5, ..done };
        assert_eq!(decide(Goal::Wait, &weak, &r()), Goal::Extract { exit: 0 });
        let shy = PartyView { aggression: 0.3, ..done };
        assert_eq!(decide(Goal::Wait, &shy, &r()), Goal::Extract { exit: 0 });
    }

    #[test]
    fn with_no_exit_open_a_done_party_waits() {
        let stuck = PartyView {
            pois_left: 0,
            exit: None,
            ..calm(0.1)
        };
        assert_eq!(decide(Goal::Wait, &stuck, &r()), Goal::Wait);
    }

    fn poi(x: f32, worth: f32) -> PoiDef {
        PoiDef {
            key: format!("p{x}"),
            name: "P".into(),
            at: (x, 0.0),
            radius: 5.0,
            worth,
        }
    }

    #[test]
    fn the_next_point_is_the_best_by_worth_and_distance_not_yet_seen() {
        let pois = [poi(10.0, 1.0), poi(40.0, 3.0), poi(100.0, 1.0)];
        // 1/60 against 3/90 against 1/150: the rich one 40 yards off.
        assert_eq!(next_poi(&pois, &[], V2::ZERO, 0.9), Some(1));
        assert_eq!(next_poi(&pois, &[1], V2::ZERO, 0.9), Some(0));
        assert_eq!(next_poi(&pois, &[], V2::ZERO, 0.1), Some(0), "the dice take the second");
        assert_eq!(next_poi(&pois, &[0, 1, 2], V2::ZERO, 0.9), None);
        assert_eq!(next_poi(&pois, &[0, 1], V2::ZERO, 0.1), Some(2));
    }

    #[test]
    fn followers_walk_behind_their_leader_left_and_right() {
        let leader = V2::new(10.0, 10.0);
        let east = V2::new(1.0, 0.0);
        let a = formation_spot(leader, east, 1, 2.5);
        let b = formation_spot(leader, east, 2, 2.5);
        assert!(a.x < leader.x && b.x < leader.x, "behind");
        assert!(a.y > leader.y && b.y < leader.y, "one to each side");
        assert!((a.distance(leader) - b.distance(leader)).abs() < 1e-4);
    }

    #[test]
    fn walking_a_path_turns_its_corners_and_stops_at_its_end() {
        let path = [V3::new(4.0, 0.0, 0.0), V3::new(4.0, 4.0, 1.0)];
        let mut next = 0;
        let a = walk_along(V3::new(0.0, 0.0, 0.0), &path, &mut next, 3.0);
        assert!((a.x - 3.0).abs() < 1e-4 && a.y.abs() < 1e-4);
        assert_eq!(next, 0);
        let b = walk_along(a, &path, &mut next, 3.0);
        assert!((b.x - 4.0).abs() < 1e-4 && (b.y - 2.0).abs() < 1e-4, "round the corner");
        assert_eq!((next, b.z), (1, 1.0));
        let c = walk_along(b, &path, &mut next, 10.0);
        assert_eq!((c.ground(), next), (V2::new(4.0, 4.0), 2));
        let d = walk_along(c, &path, &mut next, 10.0);
        assert_eq!(d, c, "at the end it stays");
    }

    #[test]
    fn away_is_straight_off_from_the_threat() {
        let p = away_from(V2::new(10.0, 0.0), V2::ZERO, 20.0);
        assert!((p.x - 30.0).abs() < 1e-4 && p.y.abs() < 1e-4);
        let q = away_from(V2::ZERO, V2::ZERO, 5.0);
        assert!((q.length() - 5.0).abs() < 1e-4, "on top of it, any way off");
    }
}
```

Run: `cargo test -p nightfall-sim rivals::goals`
Expected: FAIL to compile.

- [ ] **Step 2: Write the goals**

Put above the tests in `crates/nightfall-sim/src/rivals/goals.rs`:

```rust
//! A rival party's goals (spec §7.4): visit points of interest for loot, fight what comes on
//! the way, then extract or hunt the heroes as its aggression says. Like Single Player Tarkov's
//! questing bots (§1.1), a party has an errand of its own and drops it when the night gets in
//! the way: a hero it is alerted to is fought by a bold party and slipped away from by a
//! cautious one (unless the hero is too near, or struck first); monsters are fought; a wary
//! member stops the party, a checking one leads it to look; a party too hurt breaks off. Pure:
//! the bots build a [`PartyView`] each step and walk where the goal says.

use crate::{PoiDef, RivalRules, V2, V3};

/// WoW's walk speed as a share of its run speed (the stock 2.5 against 7), for a calm party.
pub(crate) const WALK_RATIO: f32 = 2.5 / 7.0;
/// Yards from a path's point at which it counts as reached.
const ARRIVE: f32 = 0.4;
/// Below this roll a party goes to its second-best point, so parties of one map vary.
const SECOND_BEST: f32 = 0.25;
/// Yards added to every point's distance when weighing it: a near poor point should not always
/// beat a far rich one.
const POI_NEAR: f32 = 50.0;

/// What a party is about.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Goal {
    /// Nothing to do yet (no exit open): it stands.
    #[default]
    Wait,
    /// To point of interest `poi` (an index into the rivals' points), and its loot.
    Loot { poi: usize },
    /// A member is wary: the party stops and faces `face`.
    Hold { face: V2 },
    /// A member is checking: the party goes to look at `at`.
    Check { at: V3 },
    /// In a fight: each member at its own target.
    Fight,
    /// Breaking off from `from`, for `left` seconds more.
    Flee { from: V2, left: f32 },
    /// After the heroes, toward `at`, for `left` seconds more.
    Hunt { at: V3, left: f32 },
    /// Out by exit `exit` (an index into the map's exits).
    Extract { exit: usize },
}

/// A hero the party is alerted to: where it was last seen, and how far from the leader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FoeSeen {
    pub(crate) at: V3,
    pub(crate) distance: f32,
}

/// What a party knows this step, for [`decide`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PartyView {
    pub(crate) aggression: f32,
    /// The weakest member's share of health.
    pub(crate) health: f32,
    /// A member's bag is full, or its load past `full_load`.
    pub(crate) full: bool,
    /// The night has reached `leave_by`.
    pub(crate) late: bool,
    /// Points of interest still to visit.
    pub(crate) pois_left: u8,
    /// The point being looted now, while it holds loot.
    pub(crate) looting: Option<usize>,
    /// The next point to go to.
    pub(crate) next_poi: Option<usize>,
    pub(crate) foe: Option<FoeSeen>,
    /// A hero has struck one of them this raid.
    pub(crate) hurt_by_hero: bool,
    /// An enemy is after one of them, or guards the point being looted.
    pub(crate) monsters: bool,
    /// A member is wary of a place (`false`) or checking it (`true`).
    pub(crate) wary: Option<(V3, bool)>,
    /// Where a hero was last heard or noticed, for a hunt.
    pub(crate) lead: Option<V3>,
    /// The party has hunted already this raid.
    pub(crate) hunted: bool,
    /// The nearest exit open now.
    pub(crate) exit: Option<usize>,
}

/// The party's goal this step, from the one it had and what it knows (see the module).
pub(crate) fn decide(now: Goal, v: &PartyView, r: &RivalRules) -> Goal {
    // A flight runs its course: a party that turns round at once is caught.
    if let Goal::Flee { left, .. } = now {
        if left > 0.0 {
            return now;
        }
    }
    if let Some(f) = v.foe {
        let from = f.at.ground();
        if v.health < r.flee_health {
            return Goal::Flee {
                from,
                left: r.flee_for,
            };
        }
        if v.aggression >= r.engage_at || v.hurt_by_hero || f.distance <= r.avoid_within {
            return Goal::Fight;
        }
        return Goal::Flee {
            from,
            left: r.flee_for,
        };
    }
    if v.monsters {
        return Goal::Fight;
    }
    if let Some((at, checking)) = v.wary {
        return if checking {
            Goal::Check { at }
        } else {
            Goal::Hold { face: at.ground() }
        };
    }
    let done = v.full || v.late || v.pois_left == 0;
    if !done {
        if let Some(poi) = v.looting.or(v.next_poi) {
            return Goal::Loot { poi };
        }
    }
    if v.aggression >= r.hunt_at && v.health >= r.rest_below && !v.full {
        match now {
            Goal::Hunt { at, left } if left > 0.0 => {
                return Goal::Hunt {
                    at: v.lead.unwrap_or(at),
                    left,
                };
            }
            _ if !v.hunted => {
                if let Some(at) = v.lead {
                    return Goal::Hunt {
                        at,
                        left: r.hunt_for,
                    };
                }
            }
            _ => {}
        }
    }
    v.exit.map_or(Goal::Wait, |exit| Goal::Extract { exit })
}

/// The point of interest a party at `at` goes to next, of those not in `skip`: the best by
/// worth over distance (plus [`POI_NEAR`]); a `roll` (0 to 1) under [`SECOND_BEST`] takes the
/// second best instead. Ties go to the lower index.
pub(crate) fn next_poi(pois: &[PoiDef], skip: &[usize], at: V2, roll: f32) -> Option<usize> {
    let mut scored: Vec<(f32, usize)> = pois
        .iter()
        .enumerate()
        .filter(|(i, _)| !skip.contains(i))
        .map(|(i, p)| (p.worth / (at.distance(V2::new(p.at.0, p.at.1)) + POI_NEAR), i))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    match scored.len() {
        0 => None,
        1 => Some(scored[0].1),
        _ => Some(scored[usize::from(roll < SECOND_BEST)].1),
    }
}

/// Where member `rank` (1 and 2; the leader is 0) walks: `gap` yards behind its leader, one to
/// the left and one to the right.
pub(crate) fn formation_spot(leader: V2, facing: V2, rank: usize, gap: f32) -> V2 {
    let f = match facing.normalize_or_zero() {
        f if f == V2::ZERO => V2::new(1.0, 0.0),
        f => f,
    };
    let left = V2::new(-f.y, f.x);
    let side = if rank % 2 == 1 { 1.0 } else { -1.0 };
    let back = gap * rank.div_ceil(2) as f32;
    leader + f * -back + left * (side * gap * 0.6)
}

/// `step` yards along `path` from `from`: past each point reached (within [`ARRIVE`]), `next`
/// counting them; at its end it stays where it is. The height is the point walked toward.
pub(crate) fn walk_along(from: V3, path: &[V3], next: &mut usize, step: f32) -> V3 {
    let mut at = from.ground();
    let mut z = from.z;
    let mut left = step;
    while let Some(p) = path.get(*next) {
        let to = p.ground();
        let d = at.distance(to);
        z = p.z;
        if d <= left.max(ARRIVE) {
            at = to;
            left -= d;
            *next += 1;
            if left <= 0.0 {
                break;
            }
            continue;
        }
        at = at + (to - at) * (left / d);
        break;
    }
    V3::at(at, z)
}

/// The spot `distance` yards from `at`, straight away from `from` (any way off when on it).
pub(crate) fn away_from(at: V2, from: V2, distance: f32) -> V2 {
    let way = match (at - from).normalize_or_zero() {
        w if w == V2::ZERO => V2::new(1.0, 0.0),
        w => w,
    };
    at + way * distance
}
```

In `crates/nightfall-sim/src/rivals/mod.rs` add `mod goals;` below `mod content;`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-sim rivals::goals`
Expected: PASS (10 tests). Unused-item warnings go in Task 8.

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/rivals
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: a rival party's goals, its points of interest, formation and walking

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Rivals as heroes of another side: the raid stays the party's

**Files:**
- Modify: `crates/nightfall-sim/src/rivals/mod.rs`, `crates/nightfall-sim/src/rivals/tests.rs`
- Modify: `crates/nightfall-sim/src/world.rs` (`Hero::side`, `World::rivals`, `new`, `add_hero`, `remove_hero`), `crates/nightfall-sim/src/sight.rs` (`seen_by`), `crates/nightfall-sim/src/combat/mod.rs` (`Event`), `crates/nightfall-sim/src/combat/falling.rs` (`wound`, `down`, `step_fall`), `crates/nightfall-sim/src/raid/extract.rs` (`tally_kills`, `extract`, `step_deaths`, `step_over`, `abandon`), `crates/nightfall-sim/src/lib.rs`
- Modify (every other `Hero { … }` literal gains `side`): `grep -rn "Hero {" crates/nightfall-sim crates/benilla-app/src/nightfall`

**Interfaces:**
- Consumes: `World::{add_hero, remove_hero, hero, hero_slot, give_loadout, set_input, map, content, heroes, events}`, `Hero::{player, pos, z, facing, fight, side}`, `HeroPose`, `PlayerInput`, `Loadout`, `free_kit`, `Inventory::{first_of, move_to}`, `Place::Bag`, `ItemKind::Bag`, `Event::{Died, EnemyDied}`, `Life`, `Stop`, `UnitRef`, `DeathLoss::Everything`, `Raid::{add_kill, kills_of, fallen, had_heroes, over}`, `Mind::{new, forget}`, `Dice::new` (earlier plans); `kit_of`, `KitSlot` (Task 2); `Goal` (Task 3); `RivalRules::{cone, downed}` (Task 1).
- Produces:
  - `nightfall_sim::{Side::{Party, Rival(u8)}, RIVAL_BASE}`; `Side::is_rival(self) -> bool`; `Hero::side: Side`.
  - `World::{hostile(&self, PlayerId, PlayerId) -> bool, side_of(&self, PlayerId) -> Side}`.
  - Crate-only `World::{rivals: Rivals, add_rival_hero(&mut self, PlayerId, kind: usize, Side, at: V2, name: &str) -> Result<(), WorldError>, rival_struck(&mut self, PlayerId, by: PlayerId), rival_leaves(&mut self, PlayerId, exit: usize)}`.
  - Crate-only `rivals::{Rivals { bodies, parties, arrived, try_in, next, paths_left, mirror }, RivalBody { player, party, kind, mind, kit, struck_by, way, cast_seq, cast, press_at, stealth_presses, stealth_at, dodge_presses, dodged, act_seq, act, loot, done }, Party { number, def, aggression, members, goal, visited, pois_left, next_poi, looting, hurt_by, hunted, shut, best, stalled, dice }, Way { path, next, toward, failed }}`; `Rivals::{body(&self, PlayerId) -> Option<&RivalBody>, body_mut, forget(&mut self, PlayerId), struck_by(&self, PlayerId) -> Option<PlayerId>}`.
  - `Event::{RivalDied { player: PlayerId, by: Option<PlayerId> }, RivalOut { player: PlayerId, exit: u8 }}`.
  - Test-only `rivals::tests::{open, world_on, world, join, put, rival, steps, secs, events_of}`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs`, merging its `use` lines into one at the top:

```rust
use std::sync::Arc;

use crate::heightmap::testmap::flat;
use crate::heightmap::HeightMap;
use crate::{
    Event, Gait, HeroPose, Holder, Life, PlayerId, PlayerInput, RaidRoll, Side, UnitRef, World,
    RIVAL_BASE, TICK_HZ,
};

/// Flat ground 120 yards across.
pub(super) fn open() -> HeightMap {
    flat(120.0).finish()
}

/// A world on `map` with the tests' content, its raid begun from `roll`, then its map attached.
pub(super) fn world_on(
    map: HeightMap,
    rivals_rules: &str,
    spawns: &str,
    roll: impl FnOnce(&Content) -> RaidRoll,
) -> World {
    let c = content(rivals_rules, spawns);
    let r = roll(&c);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(r).unwrap();
    w.attach_map(Arc::new(map));
    w
}

/// [`world_on`] open ground, the standard roll (no rivals come by themselves).
pub(super) fn world(rivals_rules: &str, spawns: &str) -> World {
    world_on(open(), rivals_rules, spawns, |c| RaidRoll::standard(c, 1))
}

/// `player`'s next input: their hero at `(x, y)` facing `facing`, moving at `gait`.
pub(super) fn put(w: &mut World, player: PlayerId, x: f32, y: f32, facing: V2, gait: Gait) {
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing,
                z: 0.0,
            }),
            gait,
            ..Default::default()
        },
    );
}

/// `player`'s rogue standing at `(x, y)`, facing +x, its first input taken.
pub(super) fn join(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
    put(w, player, x, y, V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

/// A rival of `kind` in party `party` at `(x, y)`, numbered `RIVAL_BASE + n`, its first input
/// (a baseline) taken. No bot drives it: it is in no party's list, so a test moves it.
pub(super) fn rival(w: &mut World, n: u8, kind: &str, party: u8, x: f32, y: f32) -> PlayerId {
    let player = RIVAL_BASE + n;
    let k = w.content().rival_kind(kind).unwrap();
    w.add_rival_hero(player, k, Side::Rival(party), V2::new(x, y), kind)
        .unwrap();
    w.step();
    player
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

/// Steps in `s` seconds.
pub(super) fn secs(s: f32) -> usize {
    (s * TICK_HZ as f32).round() as usize
}

/// The events since the last drain that `pick` keeps.
pub(super) fn events_of<T>(w: &mut World, pick: impl Fn(&Event) -> Option<T>) -> Vec<T> {
    w.drain_events().iter().filter_map(pick).collect()
}

#[test]
fn a_rival_is_a_hero_of_its_own_side_in_its_own_gear() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    join(&mut w, 1, 22.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    let b = rival(&mut w, 1, "shooter", 1, 50.0, 60.0);
    let h = w.hero(a).unwrap();
    assert_eq!(h.side, Side::Rival(0));
    assert_eq!(w.content().classes[h.class].key, "rival:digger");
    assert_eq!(h.pos, V2::new(40.0, 60.0));
    assert_eq!(h.hp, 900.0);
    assert!(w.hostile(0, a) && w.hostile(a, 0));
    assert!(!w.hostile(0, 1), "the party are friends");
    assert!(!w.hostile(a, b), "rival parties leave one another be");
    assert!(!w.hostile(0, 200), "no hero, no fight");
    // It wears its gear, its sack sits in a bag slot.
    let pack = w.pack(a).unwrap();
    let c = w.content().clone();
    assert!(pack.equip.iter().flatten().any(|s| usize::from(s.item) == c.item("shovel").unwrap()));
    assert_eq!(
        pack.bags[0].map(|s| usize::from(s.item)),
        Some(c.item("sack").unwrap())
    );
    assert!(w.rivals.body(a).is_some());
}

#[test]
fn the_raid_ends_with_the_party_whatever_the_rivals_do() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    // The hero dies; the rival is up and well: the raid is over, with one outcome.
    w.hurt_hero(0, 5000.0);
    let down = w.content().rules.combat.downed_for;
    steps(&mut w, secs(down + 1.0));
    assert!(w.is_over());
    let outcomes = w.take_outcomes();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].player, 0);
    assert!(w.hero(a).is_some_and(|h| h.fight.up()));
}

#[test]
fn a_rival_out_by_an_exit_leaves_no_outcome() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 115.0, 60.0);
    w.drain_events();
    let extract = w.content().rules.run.extract_for;
    steps(&mut w, secs(extract + 0.5));
    assert!(w.hero(a).is_none(), "out and gone");
    assert!(w.take_outcomes().is_empty());
    assert!(!w.is_over(), "the party is still in");
    let out = events_of(&mut w, |e| match e {
        Event::RivalOut { player, exit } => Some((*player, *exit)),
        _ => None,
    });
    assert_eq!(out, vec![(a, 0)]);
}

#[test]
fn rivals_never_revive_heroes_nor_lend_them_their_eyes() {
    // A sentry at (60, 100): 37 yards from (60, 63), inside the 40-yard fog; 45 from (60, 55),
    // outside it. The two spots are 8 yards apart, inside the 10 yards sight is shared over.
    let mut w = world("", &format!("{}\n", sentry_line(60.0, 100.0)));
    join(&mut w, 0, 60.0, 55.0);
    join(&mut w, 1, 60.0, 63.0);
    assert!(!w.seen_by(0).is_empty(), "a teammate's eyes count");
    put(&mut w, 1, 20.0, 20.0, V2::new(1.0, 0.0), Gait::Still);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 63.0);
    put(&mut w, 0, 60.0, 55.0, V2::new(0.0, 1.0), Gait::Still);
    w.step();
    assert!(w.seen_by(0).is_empty(), "a rival's eyes are not a teammate's");
    // Downed with an up rival beside them, the hero is never revived.
    put(&mut w, a, 61.0, 55.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    w.hurt_hero(0, 5000.0);
    let revive = w.content().rules.combat.revive_for;
    steps(&mut w, secs(revive + 1.0));
    assert!(matches!(w.hero(0).unwrap().fight.life, Life::Downed { .. }));
    assert!(events_of(&mut w, |e| matches!(e, Event::Revived { .. }).then_some(())).is_empty());
}

#[test]
fn a_slain_rival_drops_everything_and_counts_as_a_kill() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    assert_eq!(w.hero(a).unwrap().fight.life, Life::Dead, "rivals die at once");
    w.step();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(a))
        .expect("its body holds what it had");
    let held: Vec<usize> = body.stacks.iter().flatten().map(|s| usize::from(s.item)).collect();
    let c = w.content().clone();
    for item in ["shovel", "tunic", "sack", "gem"] {
        assert!(held.contains(&c.item(item).unwrap()), "{item} dropped");
    }
    assert_eq!(w.raid.kills_of(0), 1);
    let died = events_of(&mut w, |e| match e {
        Event::RivalDied { player, by } => Some((*player, *by)),
        _ => None,
    });
    assert_eq!(died, vec![(a, Some(0))]);
}

#[test]
fn with_downed_set_a_rival_goes_down_and_its_friend_revives_it() {
    let mut w = world("downed: true", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 31.0, 60.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    assert!(matches!(w.hero(a).unwrap().fight.life, Life::Downed { .. }));
    let revive = w.content().rules.combat.revive_for;
    steps(&mut w, secs(revive + 0.5));
    assert!(w.hero(a).unwrap().fight.up(), "its own party revives it");
    assert!(w.hero(b).unwrap().fight.up());
}

/// A sentry of the tests' brute at `(x, y)` facing south (-y), as a spawn line.
pub(super) fn sentry_line(x: f32, y: f32) -> String {
    format!(
        r#"(place: "Post", kind: "brute", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: 270.0)),"#
    )
}
```

The digger's carried gem comes in Task 9 with its party; the slain-rival test checks a gem too, so `add_rival_hero` rolls the kind's `carry` table as well (Step 2).

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`Side`, `add_rival_hero`, `hostile`, `Event::RivalOut` and the rest do not exist).

- [ ] **Step 2: Sides, the rivals' host state, and adding a rival**

Replace `crates/nightfall-sim/src/rivals/mod.rs` with:

```rust
//! Rivals (spec §7.4): parties of two or three bot adventurers with goals of their own, in the
//! manner of Single Player Tarkov's questing bots (§1.1). A rival is a hero of another side:
//! in [`World::heroes`] with [`Hero::side`] a [`Side::Rival`], a class of its own (its kind's,
//! [`RivalContent::load`]), a pack, a fight and a stance, and moved only by the input its bot
//! sets each step as a player's client would. So the enemies see, hear and fight rivals as they
//! do heroes; the raid's packs, searches, bodies and exits are theirs too. The raid itself stays
//! the party's: it ends with the party, rivals settle no outcome, and only a side revives its
//! own.

mod content;
mod goals;
mod kit;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

pub use content::{KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalRules};
use goals::Goal;
use kit::{kit_of, KitSlot};

use crate::combat::Dice;
use crate::senses::Mind;
use crate::{
    free_kit, ActPress, CastPress, Content, Event, HeroPose, ItemKind, Loadout, Place, PlayerId,
    PlayerInput, World, WorldError, V2, V3,
};

/// Rivals' player numbers start here, past any party's (a host gives 0 to 3).
pub const RIVAL_BASE: PlayerId = 64;

/// Whose side a hero is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// The players' party.
    #[default]
    Party,
    /// A rival party, by its number in the raid.
    Rival(u8),
}

impl Side {
    pub fn is_rival(self) -> bool {
        matches!(self, Side::Rival(_))
    }
}

/// The rivals' host-only state: never sent (a client's mirror keeps only [`Rivals::mirror`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct Rivals {
    /// One per rival in the raid, the dead included.
    pub(crate) bodies: Vec<RivalBody>,
    /// The parties a roll brought ([`World::step_rival_arrivals`]).
    pub(crate) parties: Vec<Party>,
    /// Which of the roll's parties have come.
    pub(crate) arrived: Vec<bool>,
    /// Seconds until arrivals are tried again.
    pub(crate) try_in: f32,
    /// Rival numbers handed out: the next is `RIVAL_BASE + next`.
    pub(crate) next: u8,
    /// Path searches left to the rivals this step.
    pub(crate) paths_left: u32,
    /// A client's mirror: the view of rivals its host sent.
    pub(crate) mirror: Option<RivalView>,
}

/// What a client is told of the rivals: which its hero sees, and the marks they show it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RivalView {
    pub seen: Vec<PlayerId>,
    pub marks: Vec<RivalMark>,
}

/// One rival's mark for one player (spec §5.5: "?" and "!").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalMark {
    pub rival: PlayerId,
    pub alert: crate::Alert,
}

/// One rival's mind and bot.
#[derive(Clone, Debug)]
pub(crate) struct RivalBody {
    pub(crate) player: PlayerId,
    /// Its party's number ([`Side::Rival`]).
    pub(crate) party: u8,
    /// Index into the rivals' kinds.
    pub(crate) kind: usize,
    /// What it makes of each hero of the party (spec §7.4: "notice heroes the way enemies do").
    pub(crate) mind: Mind,
    pub(crate) kit: Vec<KitSlot>,
    /// The last hero of the party to strike it.
    pub(crate) struck_by: Option<PlayerId>,
    pub(crate) way: Way,
    /// Its presses, numbered as a client numbers a player's.
    pub(crate) cast_seq: u8,
    pub(crate) cast: Option<CastPress>,
    /// When it last pressed: a bot presses at most every so often.
    pub(crate) press_at: f32,
    pub(crate) stealth_presses: u8,
    pub(crate) stealth_at: f32,
    pub(crate) dodge_presses: u8,
    /// Telegraphs it has rolled to dodge.
    pub(crate) dodged: Vec<u32>,
    pub(crate) act_seq: u8,
    pub(crate) act: Option<ActPress>,
    /// The container it is looting.
    pub(crate) loot: Option<u32>,
    /// Containers it is done with.
    pub(crate) done: Vec<u32>,
}

/// Where a rival is walking: its path and the goal it was searched for.
#[derive(Clone, Debug, Default)]
pub(crate) struct Way {
    pub(crate) path: Option<Arc<[V3]>>,
    pub(crate) next: usize,
    pub(crate) toward: Option<V2>,
    /// The last search found no way.
    pub(crate) failed: bool,
}

/// One rival party in the raid.
#[derive(Clone, Debug)]
pub(crate) struct Party {
    pub(crate) number: u8,
    /// Index into the rivals' parties.
    pub(crate) def: usize,
    pub(crate) aggression: f32,
    /// Its members, the first living one leading.
    pub(crate) members: Vec<PlayerId>,
    pub(crate) goal: Goal,
    /// Points of interest visited or given up.
    pub(crate) visited: Vec<usize>,
    pub(crate) pois_left: u8,
    pub(crate) next_poi: Option<usize>,
    pub(crate) looting: Option<usize>,
    /// A hero of the party struck one of them.
    pub(crate) hurt_by: Option<PlayerId>,
    pub(crate) hunted: bool,
    /// Exits given up (no way there).
    pub(crate) shut: Vec<usize>,
    /// The leader's nearest yet to its goal's place, and the seconds since it got nearer.
    pub(crate) best: f32,
    pub(crate) stalled: f32,
    pub(crate) dice: Dice,
}

impl Rivals {
    pub(crate) fn body(&self, player: PlayerId) -> Option<&RivalBody> {
        self.bodies.iter().find(|b| b.player == player)
    }

    pub(crate) fn body_mut(&mut self, player: PlayerId) -> Option<&mut RivalBody> {
        self.bodies.iter_mut().find(|b| b.player == player)
    }

    /// `player` left the world: a rival's body and place in its party go; a hero's are
    /// forgotten by every rival's mind.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.bodies.retain(|b| b.player != player);
        for p in &mut self.parties {
            p.members.retain(|&m| m != player);
        }
        for b in &mut self.bodies {
            b.mind.forget(player);
        }
    }

    /// The last hero of the party to strike rival `player`.
    pub(crate) fn struck_by(&self, player: PlayerId) -> Option<PlayerId> {
        self.body(player).and_then(|b| b.struck_by)
    }
}

impl RivalBody {
    fn new(player: PlayerId, party: u8, kind: usize, content: &Content) -> Self {
        let k = &content.rivals.kinds[kind];
        Self {
            player,
            party,
            kind,
            mind: Mind::new(content.rules.rivals.cone, false),
            kit: kit_of(k.class, k.big_ability, content),
            struck_by: None,
            way: Way::default(),
            cast_seq: 0,
            cast: None,
            press_at: f32::NEG_INFINITY,
            stealth_presses: 0,
            stealth_at: f32::NEG_INFINITY,
            dodge_presses: 0,
            dodged: Vec::new(),
            act_seq: 0,
            act: None,
            loot: None,
            done: Vec::new(),
        }
    }
}

impl World {
    /// Whether `a` and `b` fight each other: the party against any rival; rival parties leave
    /// one another be. Nobody is hostile to a player with no hero.
    pub fn hostile(&self, a: PlayerId, b: PlayerId) -> bool {
        match (self.hero(a), self.hero(b)) {
            (Some(x), Some(y)) => {
                x.side != y.side && (x.side == Side::Party || y.side == Side::Party)
            }
            _ => false,
        }
    }

    /// `player`'s side: the party's for a player with no hero.
    pub fn side_of(&self, player: PlayerId) -> Side {
        self.hero(player).map_or(Side::Party, |h| h.side)
    }

    /// A rival of `kind` (an index into the rivals' kinds) on `side` at `at`, as `player`: a hero
    /// of the kind's class named `name`, standing on the floor there, wearing its gear (its bag in
    /// its first bag slot) and carrying its kind's `carry` roll. Its first input is set, the
    /// baseline every hero's first input is.
    pub(crate) fn add_rival_hero(
        &mut self,
        player: PlayerId,
        kind: usize,
        side: Side,
        at: V2,
        name: &str,
    ) -> Result<(), WorldError> {
        let content = self.content().clone();
        let def = &content.rivals.kinds[kind];
        let class_key = content.classes[def.class].key.clone();
        self.add_hero(player, &class_key, name, def.race, def.sex)?;
        let at = content.map.bounds.clamp(at);
        let z = self.map().and_then(|m| m.ground_at(at)).unwrap_or(0.0);
        if let Some(hi) = self.hero_slot(player) {
            let h = &mut self.heroes[hi];
            h.side = side;
            h.pos = at;
            h.z = z;
        }
        let mut inventory = free_kit(def.class, &content);
        if let Some(bag) = inventory.first_of(ItemKind::Bag, &content) {
            let _ = inventory.move_to(bag, Place::Bag(0), &content);
        }
        if let Some(t) = def.carry_table {
            let mut dice = Dice::new(u64::from(player) ^ 0x5249_5641_4c53);
            for stack in crate::raid::roll_table(&content.loot.tables[t], &mut dice, &content) {
                let _ = inventory.add(stack, &content);
            }
        }
        self.give_loadout(
            player,
            &Loadout {
                inventory,
                ..Loadout::default()
            },
        );
        let party = match side {
            Side::Rival(n) => n,
            Side::Party => 0,
        };
        self.rivals
            .bodies
            .push(RivalBody::new(player, party, kind, &content));
        self.set_input(
            player,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: at,
                    facing: V2::new(1.0, 0.0),
                    z,
                }),
                ..PlayerInput::default()
            },
        );
        Ok(())
    }

    /// Rival `player` was struck by `by`: a hero of the party, remembered as the one who slew
    /// it if it dies of it.
    pub(crate) fn rival_struck(&mut self, player: PlayerId, by: PlayerId) {
        if !(self.side_of(player).is_rival() && self.hostile(player, by)) {
            return;
        }
        if let Some(b) = self.rivals.body_mut(player) {
            b.struck_by = Some(by);
        }
    }

    /// Rival `player` is out by `exit`: everything it carries goes with it, and nobody settles
    /// an outcome for it (spec §7.4: it extracts, as a hero would, but the raid is not its).
    pub(crate) fn rival_leaves(&mut self, player: PlayerId, exit: usize) {
        self.events.push(Event::RivalOut {
            player,
            exit: exit as u8,
        });
        self.remove_hero(player);
    }
}
```

Fields the later tasks read (`way`, the presses, `parties`, `mirror`) warn as unused until Task 8; leave the warnings, they go then.

If `roll_table` is private to `raid::containers`, make it `pub(crate)` and re-export it from `raid/mod.rs` as `pub(crate) use containers::roll_table;` (the run loop's Task 5 made it crate-only). If `senses::Mind` is not yet re-exported from `senses/mod.rs`, add `pub(crate) use awareness::{Mind, Sensed, Shout, State};` there (keep only what it lacks).

In `crates/nightfall-sim/src/lib.rs`, the `rivals` exports become `pub use rivals::{KitDef, PartyDef, PoiDef, RivalContent, RivalKindDef, RivalMark, RivalRules, RivalView, Side, RIVAL_BASE};`.

- [ ] **Step 3: The world keeps sides**

In `crates/nightfall-sim/src/world.rs`:

- Add `use crate::rivals::{Rivals, Side};`.
- `Hero` gains, as its last field:

```rust
    /// The party's, or a rival party's (spec §7.4).
    pub side: Side,
```

- The `Hero { … }` literal in `add_hero` gains `side: Side::Party,`; so does every other `Hero { … }` literal (see Files).
- `World` gains the field `pub(crate) rivals: Rivals,` and `World::new`'s literal `rivals: Rivals::default(),`.
- `remove_hero` gains, after the forgets the earlier plans put in it, `self.rivals.forget(player);`.

In `crates/nightfall-sim/src/sight.rs`'s `seen_by`, the watchers' filter `.filter(|h| h.player == player || h.pos.distance(me.pos) <= share)` becomes `.filter(|h| h.player == player || (h.side == me.side && h.pos.distance(me.pos) <= share))`, and the doc gains: `Only the party's own eyes count: a rival standing by is not a teammate.`

In `crates/nightfall-sim/src/combat/mod.rs`, `Event` gains after `EnemyDied`:

```rust
    /// A rival died, by `by`'s hand when a hero of the party's (spec §7.4).
    RivalDied { player: PlayerId, by: Option<PlayerId> },
    /// A rival got out by exit `exit`, everything it carried with it.
    RivalOut { player: PlayerId, exit: u8 },
```

In `crates/nightfall-sim/src/combat/falling.rs`:

- `wound` gains, right after its first `if !self.heroes[hi].fight.up() || !(amount > 0.0) { return; }`:

```rust
        if let Some(UnitRef::Hero(p)) = by {
            self.rival_struck(player, p);
        }
```

- `down` begins with:

```rust
        // A rival dies at 0 health unless the rules let it go down as heroes do (spec §7.4:
        // "killed, they drop" their bags; a 45-second crawl would only delay it).
        if self.heroes[hi].side.is_rival() && !self.content.rules.rivals.downed {
            self.stop_cast(hi, Stop::Interrupted);
            let h = &mut self.heroes[hi];
            let player = h.player;
            h.hp = 0.0;
            h.stealthed = false;
            h.fight.life = Life::Dead;
            h.fight.attacking = None;
            h.fight.sprint = 0.0;
            self.events.push(Event::Died { player });
            return;
        }
```

- In `step_fall`, `let (player, pos, z) = …;` is followed by `let side = self.heroes[hi].side;`, and the reviver's filter gains `&& o.side == side` (a side revives only its own).

In `crates/nightfall-sim/src/raid/extract.rs`:

- `tally_kills`' `filter_map` arm `Event::EnemyDied { by: Some(p), .. } => Some(*p),` becomes `Event::EnemyDied { by: Some(p), .. } | Event::RivalDied { by: Some(p), .. } => Some(*p),`.
- `extract` begins with:

```rust
        if self.side_of(player).is_rival() {
            return self.rival_leaves(player, exit);
        }
```

- `step_deaths` becomes:

```rust
    /// Every hero dead since the last step leaves their body where they fell (spec §8.2),
    /// holding what `death_loss` takes; friends can search it this raid. A rival's body holds
    /// everything it had (spec §7.4), and its death is told.
    pub(crate) fn step_deaths(&mut self) {
        let content = self.content().clone();
        let dead: Vec<(PlayerId, V2, f32, bool)> = self
            .heroes
            .iter()
            .filter(|h| {
                h.fight.life == Life::Dead && !self.raid.fallen.iter().any(|(p, _)| *p == h.player)
            })
            .map(|h| (h.player, h.pos, h.z, h.side.is_rival()))
            .collect();
        for (player, at, z, rival) in dead {
            let mode = if rival {
                DeathLoss::Everything
            } else {
                content.rules.run.death_loss
            };
            let lost = self
                .raid
                .pack_mut(player, &content)
                .split_for_death(mode, &content);
            let body = if lost.is_empty() {
                0
            } else {
                let slots = usize::from(content.rules.run.container_slots).max(lost.len());
                self.raid
                    .add_container(Holder::Hero(player), at, Some(z), lost, slots, false, None)
            };
            self.raid.fallen.push((player, body));
            self.raid.close(player);
            if let Some(hi) = self.hero_slot(player) {
                self.heroes[hi].carry.lantern = None;
            }
            self.refresh_carry(player);
            if rival {
                let by = self.rivals.struck_by(player);
                self.events.push(Event::RivalDied { player, by });
            }
        }
    }
```

  (`DeathLoss` joins the file's `crate::{…}` import if it is not in it. The body's slots grow to what it holds, so a rival's whole kit fits; a hero's body is as before unless it held more than `container_slots`.)

- `step_over` becomes:

```rust
    /// With every hero of the party out or dead the raid is over (spec §8.1), and each dead
    /// hero's outcome is settled now: what is left on their body by then is what a recovery
    /// holds. Rivals do not count (spec §7.4): the raid is the party's.
    pub(crate) fn step_over(&mut self) {
        let party: Vec<(PlayerId, bool)> = self
            .heroes
            .iter()
            .filter(|h| h.side == Side::Party)
            .map(|h| (h.player, h.fight.life == Life::Dead))
            .collect();
        if !party.is_empty() {
            self.raid.had_heroes = true;
        }
        if self.raid.over || !self.raid.had_heroes {
            return;
        }
        if !party.iter().all(|&(_, dead)| dead) {
            return;
        }
        self.raid.over = true;
        for (p, _) in party {
            self.settle_dead(p);
        }
    }
```

- In `abandon`, the `heroes` collection gains `.filter(|h| h.side == Side::Party)` before its `.map`.
- `Side` joins the file's `crate::{…}` import.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 6 new rivals tests and every earlier one: no earlier world has a rival, so sides change nothing there).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: rivals are heroes of another side; the raid, revives and sight stay the party's

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The fight between heroes and rivals

**Files:**
- Create: `crates/nightfall-sim/src/combat/foes.rs`
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (`mod foes;`, `HeroFight::{combo_hero, held}`, `HeroCombat::{held, slow, burns, swing_at, foe_swing_in}`, `Refusal::Held`, `Event::Healed`, `CastState::foe`), `crates/nightfall-sim/src/combat/heroes.rs` (`Aimed::foe`, `check`, `begin`, `go`, `step_cast`, `step_fighters`), `crates/nightfall-sim/src/combat/effects.rs` (`land`, `land_on`, `land_self`), `crates/nightfall-sim/src/combat/falling.rs` (`wound_by`), `crates/nightfall-sim/src/rivals/tests.rs`
- Modify (the client's exhaustive match on `Refusal`): `crates/benilla-app/src/nightfall/fight.rs` (`refusal_key`)

**Interfaces:**
- Consumes: `World::{check, begin, pay, go, land, land_on, land_self, stop_cast, make_noise, hero_slot, hostile, clear_line, wound}`, `Aimed`, `CastState`, `HeroCombat`, `Held`, `Slowed`, `Burn`, `ControlKind::{out, freed_by_damage}`, `Creature::Humanoid`, `in_range`, `stands_right`, `Facing`, `Sound`, `Effect`, `Refusal`, `Stop`, `Event::{Damage, Swing}`, `CombatRules::{crit_chance, crit_times, combat_linger, front, reach}`, `SenseRules::ambush`, `MELEE`, `NEEDS_SIGHT`, `EYE_HEIGHT`, `CHEST_HEIGHT`, `Loudness::Hit`, `DT` (earlier plans; Task 4).
- Produces:
  - `HeroFight::{combo_hero: Option<PlayerId>, held: Option<ControlKind>}`; `Refusal::Held`; `Event::Healed { player: PlayerId, amount: f32 }`.
  - Crate-only `Aimed::foe: Option<PlayerId>`, `CastState::foe: Option<PlayerId>`; `HeroCombat::{held: Option<Held>, slow: Option<Slowed>, burns: Vec<Burn>, swing_at: Option<PlayerId>, foe_swing_in: f32}`.
  - Crate-only on `World` (`combat/foes.rs`): `foes_within(&self, PlayerId, V2, f32) -> Vec<PlayerId>`, `check_foe(&self, hi, ability, foe: PlayerId) -> Result<Aimed, Refusal>`, `foe_in_sight(&self, hi, Option<PlayerId>, range) -> bool`, `foe_unaware(&self, target: PlayerId, by: PlayerId) -> bool`, `foe_ambush(&self, target, by) -> f32`, `land_at_foe(&mut self, PlayerId, ability, foe: PlayerId) -> Vec<UnitRef>`, `land_foe(&mut self, PlayerId, ability, foe: PlayerId, combo: u8)`, `strike_foe(&mut self, by, foe, amount, ambush, silent, may_crit)`, `tick_foe_holds(&mut self, hi)`, `foe_pace(&mut self, hi)`, `swing_foe(&mut self, hi)`.
  - Crate-only `World::wound_by(&mut self, PlayerId, f32, Option<UnitRef>, crit: bool, ambush: bool)` (`wound` calls it with neither).
  - `Effect::Heal` lands on its caster; a rival's Blink moves it on the host.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs` (merge `CastPress`, `Refusal` and `ControlKind` into its `crate::{…}` import):

```rust
/// `player` presses action `slot` at `target`, standing still; numbered `seq` (a new press
/// needs a new number, as from a client).
pub(super) fn press(w: &mut World, player: PlayerId, seq: u8, slot: u8, target: Option<UnitRef>) {
    let h = w.hero(player).unwrap();
    let pose = HeroPose {
        pos: h.pos,
        facing: h.facing,
        z: h.z,
    };
    w.set_input(
        player,
        PlayerInput {
            pose: Some(pose),
            cast: Some(CastPress {
                seq,
                slot,
                target,
                at: None,
            }),
            ..Default::default()
        },
    );
}

/// The damage `to` took in these events.
pub(super) fn damage_to(events: &[Event], to: UnitRef) -> Vec<f32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Damage { to: t, amount, .. } if *t == to => Some(*amount),
            _ => None,
        })
        .collect()
}

/// A hero at (20, 60) facing +x and a digger 3 yards in front of it facing the hero.
fn duel() -> (World, PlayerId) {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 23.0, 60.0);
    put(&mut w, a, 23.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    w.drain_events();
    (w, a)
}

#[test]
fn a_hero_strikes_a_rival_and_the_rival_strikes_back() {
    let (mut w, a) = duel();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let events = w.drain_events();
    let hit = damage_to(&events, UnitRef::Hero(a));
    assert_eq!(hit.len(), 1, "{events:?}");
    assert!(w.hero(a).unwrap().hp < 900.0);
    assert_eq!(w.hero(0).unwrap().fight.combo_hero, Some(a), "a combo point on the rival");
    // The digger's hack (action 1) back at the hero.
    press(&mut w, a, 1, 1, Some(UnitRef::Hero(0)));
    w.step();
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(0)).len(), 1, "{events:?}");
    assert!(w.hero(0).unwrap().hp < 1000.0);
}

#[test]
fn friends_and_rivals_of_another_party_cannot_be_targeted() {
    let (mut w, a) = duel();
    join(&mut w, 1, 21.0, 61.0);
    let b = rival(&mut w, 1, "digger", 1, 24.0, 61.0);
    w.drain_events();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(1)));
    press(&mut w, a, 1, 1, Some(UnitRef::Hero(b)));
    w.step();
    let refused = events_of(&mut w, |e| match e {
        Event::Refused { player, why } => Some((*player, *why)),
        _ => None,
    });
    assert!(refused.contains(&(0, Refusal::BadTarget)));
    assert!(refused.contains(&(a, Refusal::BadTarget)));
}

#[test]
fn a_hold_roots_the_hero_and_refuses_presses_until_a_blow_frees_it() {
    let (mut w, a) = duel();
    // Grave dirt (action 2): incapacitated 4 seconds, freed by damage.
    press(&mut w, a, 1, 2, Some(UnitRef::Hero(0)));
    w.step();
    let h = w.hero(0).unwrap();
    assert_eq!(h.fight.held, Some(ControlKind::Incapacitate));
    assert!(h.fight.rooted, "held feet");
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let refused = events_of(&mut w, |e| match e {
        Event::Refused { player: 0, why } => Some(*why),
        _ => None,
    });
    assert_eq!(refused, vec![Refusal::Held]);
    // A blow from the rival frees it.
    steps(&mut w, secs(1.0));
    press(&mut w, a, 2, 1, Some(UnitRef::Hero(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.held, None);
    assert!(!w.hero(0).unwrap().fight.rooted);
}

#[test]
fn a_hold_runs_out_by_itself() {
    let (mut w, a) = duel();
    press(&mut w, a, 1, 2, Some(UnitRef::Hero(0)));
    w.step();
    assert!(w.hero(0).unwrap().fight.held.is_some());
    steps(&mut w, secs(4.2));
    assert_eq!(w.hero(0).unwrap().fight.held, None);
}

#[test]
fn an_area_ability_reaches_the_hostile_heroes_in_it_and_spares_friends() {
    let mut w = world("", "");
    w.add_hero(0, "mage", "Mage", 1, 1).unwrap();
    put(&mut w, 0, 20.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    join(&mut w, 1, 22.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 25.0, 60.0);
    let far = rival(&mut w, 1, "digger", 0, 40.0, 60.0);
    w.drain_events();
    // The mage's nova (action 4): 10 yards around them.
    press(&mut w, 0, 1, 4, None);
    w.step();
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(a)).len(), 1);
    assert!(damage_to(&events, UnitRef::Hero(1)).is_empty(), "a friend is spared");
    assert!(damage_to(&events, UnitRef::Hero(far)).is_empty(), "out of reach");
    assert!(events.iter().any(|e| matches!(e, Event::CastGo { hits, .. } if hits.contains(&UnitRef::Hero(a)))));
    assert_eq!(w.hero(a).unwrap().fight.held, Some(ControlKind::Root));
}

#[test]
fn a_bandage_heals_its_rival_tick_by_tick() {
    let (mut w, a) = duel();
    w.wound(a, 400.0, Some(UnitRef::Hero(0)));
    let before = w.hero(a).unwrap().hp;
    press(&mut w, a, 1, 3, None);
    steps(&mut w, secs(3.2));
    let healed: f32 = events_of(&mut w, |e| match e {
        Event::Healed { player, amount } if *player == a => Some(*amount),
        _ => None,
    })
    .iter()
    .sum();
    assert!((healed - 180.0).abs() < 1e-3, "three ticks of 60: {healed}");
    assert!((w.hero(a).unwrap().hp - (before + 180.0)).abs() < 1e-3);
}

#[test]
fn a_slow_and_a_bleed_on_a_hero_run_their_course() {
    let (mut w, a) = duel();
    // No test kit puts a bleed or a slow on a bar: they are laid on the hero as `land_foe`
    // lays them, from the digger.
    let speed = w.hero(0).unwrap().speed;
    let ci = w.combat.hero_index(0);
    w.combat.heroes[ci].slow = Some(crate::combat::Slowed {
        by: 0.5,
        left: 1.0,
        spell: 1,
    });
    w.combat.heroes[ci].burns.push(crate::combat::Burn {
        by: a,
        amount: 10.0,
        every: 1.0,
        next: 1.0,
        left: 3.0,
        spell: 2,
        silent: true,
    });
    w.step();
    assert!(w.hero(0).unwrap().speed < speed * 0.6, "slowed by half");
    steps(&mut w, secs(3.2));
    let ticks = damage_to(&w.drain_events(), UnitRef::Hero(0));
    assert_eq!(ticks, vec![10.0, 10.0, 10.0]);
    assert!((w.hero(0).unwrap().speed - speed).abs() < 1e-3, "the slow is gone");
}
```

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`combo_hero`, `held`, `Refusal::Held`, `Event::Healed`, the hero's `slow` and `burns` do not exist).

- [ ] **Step 2: The fight's state for hostile heroes**

In `crates/nightfall-sim/src/combat/mod.rs`:

- Add `mod foes;` beside the other modules.
- `HeroFight` gains, as its last fields:

```rust
    /// Combo points are on this hostile hero, not an enemy (a rival's or a hero's builder).
    pub combo_hero: Option<PlayerId>,
    /// What holds them (a rival's control, spec §7.4): their feet are held and they cannot
    /// press while it is one that takes them out of the fight.
    pub held: Option<ControlKind>,
```

- `HeroCombat` gains, as its last fields:

```rust
    /// A hostile hero's hold on them, its slow, and its bleeds.
    pub(crate) held: Option<Held>,
    pub(crate) slow: Option<Slowed>,
    pub(crate) burns: Vec<Burn>,
    /// The hostile hero their auto-attack swings at, and the seconds to its next blow.
    pub(crate) swing_at: Option<PlayerId>,
    pub(crate) foe_swing_in: f32,
```

- `CastState` gains `pub(crate) foe: Option<PlayerId>,` after `enemy` (doc: `Its hostile hero, by player.`).
- `Refusal` gains, after `Down`, `Held,` (doc: `Stunned, incapacitated or otherwise held by a rival's control.`).
- `Event` gains, after `Revived`:

```rust
    /// Health back to a hero from a heal of their own.
    Healed { player: PlayerId, amount: f32 },
```

- Add `pub(crate) use` of `Burn`, `Held` and `Slowed` if they are not visible to `crate::rivals` (they are used by the tests as `crate::combat::{Burn, Slowed}`).

In `crates/benilla-app/src/nightfall/fight.rs`'s `refusal_key`, add the arm `Refusal::Held => "SPELL_FAILED_STUNNED",` after `Refusal::Down`.

- [ ] **Step 3: Write the fight's arm for hostile heroes**

`crates/nightfall-sim/src/combat/foes.rs`:

```rust
//! Heroes against hostile heroes (spec §7.4): a rival is a hero of another side, so a hero's
//! ability may be aimed at a rival and a rival's at a hero. This is the fight's arm for a hero
//! target, the same abilities with the same numbers as on an enemy: the press's checks; what
//! each effect does to a hero (damage, bleeds, slows, holds, combo points); area abilities
//! reaching the hostile heroes in them; auto-attack at one; and the holds, slows and bleeds on
//! heroes running down. A hold on a hero roots them and, unless it is a root, keeps them from
//! pressing; a hero is never turned into a sheep (a sheep on a client-owned body would need its
//! own design), only held.

use super::{in_range, stands_right, Aimed, Burn, Held, Slowed, MELEE, NEEDS_SIGHT};
use crate::{
    Creature, Effect, Event, Facing, Loudness, PlayerId, Refusal, Sound, UnitRef, World,
    CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3,
};

impl World {
    /// The players of `player`'s hostile heroes up within `radius` of `at`, in the world's order.
    pub(crate) fn foes_within(&self, player: PlayerId, at: V2, radius: f32) -> Vec<PlayerId> {
        self.heroes
            .iter()
            .filter(|h| h.fight.up() && h.pos.distance(at) <= radius)
            .filter(|h| self.hostile(player, h.player))
            .map(|h| h.player)
            .collect()
    }

    /// Whether hero `hi` may begin `ability` at hostile hero `foe`, after the checks every press
    /// has passed: a hostile hero who is up and a humanoid where the ability wants one, combo
    /// points on them for a finisher, unaware of the caster for an opener, in range, the caster
    /// standing right, and a clear line for a long one.
    pub(crate) fn check_foe(&self, hi: usize, ability: usize, foe: PlayerId) -> Result<Aimed, Refusal> {
        let content = &self.content;
        let r = &content.rules.combat;
        let a = &content.abilities[ability];
        let hero = &self.heroes[hi];
        if !self.hostile(hero.player, foe) {
            return Err(Refusal::BadTarget);
        }
        let fi = self.hero_slot(foe).ok_or(Refusal::NoTarget)?;
        let t = &self.heroes[fi];
        let takes = a.needs.creatures.is_empty() || a.needs.creatures.contains(&Creature::Humanoid);
        if !t.fight.up() || !takes {
            return Err(Refusal::BadTarget);
        }
        if a.needs.combo && hero.fight.combo_hero != Some(foe) {
            return Err(Refusal::NeedCombo);
        }
        if a.needs.unaware && !self.foe_unaware(foe, hero.player) {
            return Err(Refusal::Aware);
        }
        if !in_range(hero.pos.distance(t.pos), a.range, r) {
            return Err(Refusal::OutOfRange);
        }
        if !stands_right(a.facing, hero.pos, hero.facing, t.pos, t.facing, r.front) {
            return Err(if a.facing == Facing::Behind {
                Refusal::Behind
            } else {
                Refusal::Facing
            });
        }
        if !self.foe_in_sight(hi, Some(foe), a.range) {
            return Err(Refusal::NotInSight);
        }
        Ok(Aimed {
            ability,
            enemy: None,
            foe: Some(foe),
            at: None,
        })
    }

    /// Whether hero `hi` has the clear line to hero `foe` an ability of `range` needs (melee
    /// needs none, nor does no foe).
    pub(crate) fn foe_in_sight(&self, hi: usize, foe: Option<PlayerId>, range: f32) -> bool {
        let Some(t) = foe.and_then(|p| self.hero(p)) else {
            return true;
        };
        let h = &self.heroes[hi];
        range <= NEEDS_SIGHT
            || self.clear_line(
                V3::at(h.pos, h.z + EYE_HEIGHT),
                V3::at(t.pos, t.z + CHEST_HEIGHT),
            )
    }

    /// Whether hero `target` is not aware of `by`: for now, not in combat (Task 7 asks a rival's
    /// mind instead).
    pub(crate) fn foe_unaware(&self, target: PlayerId, _by: PlayerId) -> bool {
        self.hero(target).is_some_and(|t| !t.fight.in_combat)
    }

    /// The ambush multiplier on a blow from `by` at hero `target` (spec §5.4): the senses'
    /// `ambush` while the target is unaware of them, else 1.
    pub(crate) fn foe_ambush(&self, target: PlayerId, by: PlayerId) -> f32 {
        if self.foe_unaware(target, by) {
            self.content.rules.senses.ambush
        } else {
            1.0
        }
    }

    /// `player`'s ability `ability`, aimed at hostile hero `foe`, goes off: a finisher spends its
    /// points, it lands on the foe, and its effects on the caster land. What it hit.
    pub(crate) fn land_at_foe(&mut self, player: PlayerId, ability: usize, foe: PlayerId) -> Vec<UnitRef> {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return Vec::new();
        };
        let combo = if a.needs.combo {
            let f = &mut self.heroes[hi].fight;
            f.combo_hero = None;
            f.combo_on = None;
            std::mem::take(&mut f.combo)
        } else {
            0
        };
        let centre = Some(self.heroes[hi].pos);
        self.land_foe(player, ability, foe, combo);
        for effect in &a.effects {
            self.land_self(player, effect, centre, a.radius);
        }
        vec![UnitRef::Hero(foe)]
    }

    /// Ability `ability` of `player` lands on hostile hero `foe`, a finisher's `combo` points
    /// behind it: the same effects as on an enemy, and the blow frees what damage frees.
    pub(crate) fn land_foe(&mut self, player: PlayerId, ability: usize, foe: PlayerId, combo: u8) {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if !self.hero(foe).is_some_and(|h| h.fight.up()) {
            return;
        }
        // Asked before the blow, which alerts it: only the first blow is an ambush.
        let ambush = self.foe_ambush(foe, player);
        let silent = a.sound == Sound::Silent;
        for effect in &a.effects {
            match *effect {
                Effect::Damage {
                    min,
                    max,
                    per_combo,
                } => {
                    let amount =
                        self.combat.dice.between(min, max) + per_combo * f32::from(combo);
                    self.strike_foe(player, foe, amount * ambush, ambush > 1.0, silent, true);
                }
                Effect::Weapon { percent, bonus } => {
                    let Some(blow) = content.classes[self.heroes[hi].class].swing else {
                        continue;
                    };
                    let amount =
                        self.combat.dice.between(blow.min, blow.max) * percent / 100.0 + bonus;
                    self.strike_foe(player, foe, amount * ambush, ambush > 1.0, silent, true);
                }
                Effect::Dot { amount, every, secs } => {
                    let ci = self.combat.hero_index(foe);
                    let burns = &mut self.combat.heroes[ci].burns;
                    burns.retain(|b| !(b.by == player && b.spell == a.spell));
                    burns.push(Burn {
                        by: player,
                        amount,
                        every,
                        next: every,
                        left: secs,
                        spell: a.spell,
                        silent,
                    });
                }
                Effect::Slow { by, secs } => {
                    let ci = self.combat.hero_index(foe);
                    self.combat.heroes[ci].slow = Some(Slowed {
                        by,
                        left: secs,
                        spell: a.spell,
                    });
                }
                Effect::Control { kind, secs } => {
                    let ci = self.combat.hero_index(foe);
                    self.combat.heroes[ci].held = Some(Held {
                        kind,
                        left: secs,
                        by: player,
                        spell: a.spell,
                    });
                    if let Some(fi) = self.hero_slot(foe) {
                        self.heroes[fi].fight.held = Some(kind);
                        if kind.out() {
                            self.stop_cast(fi, crate::Stop::Interrupted);
                        }
                    }
                    // Its caster stops swinging at what a blow would free.
                    let pi = self.combat.hero_index(player);
                    if kind.freed_by_damage() && self.combat.heroes[pi].swing_at == Some(foe) {
                        self.combat.heroes[pi].swing_at = None;
                    }
                }
                Effect::Combo(n) => {
                    let max = content.rules.combat.combo_max;
                    let f = &mut self.heroes[hi].fight;
                    if f.combo_hero != Some(foe) {
                        f.combo = 0;
                    }
                    f.combo = (f.combo + n).min(max);
                    f.combo_hero = Some(foe);
                    f.combo_on = None;
                }
                _ => {}
            }
            if !self.hero(foe).is_some_and(|h| h.fight.up()) {
                return;
            }
        }
        if a.attack && content.classes[self.heroes[hi].class].swing.is_some() {
            let pi = self.combat.hero_index(player);
            self.combat.heroes[pi].swing_at = Some(foe);
        }
    }

    /// `by`'s blow of `amount` on hero `foe`: a crit when `may_crit` and the dice say so; a blow
    /// frees what damage frees; it keeps `by` in combat; it is heard 15 yards off unless
    /// `silent`; and it wounds.
    pub(crate) fn strike_foe(
        &mut self,
        by: PlayerId,
        foe: PlayerId,
        amount: f32,
        ambush: bool,
        silent: bool,
        may_crit: bool,
    ) {
        let Some(fi) = self.hero_slot(foe).filter(|&fi| self.heroes[fi].fight.up()) else {
            return;
        };
        let (chance, times, linger) = {
            let r = &self.content.rules.combat;
            (r.crit_chance, r.crit_times, r.combat_linger)
        };
        let crit = may_crit && self.combat.dice.chance(chance);
        let amount = (if crit { amount * times } else { amount }).max(0.0);
        let at = V3::at(self.heroes[fi].pos, self.heroes[fi].z);
        let ci = self.combat.hero_index(foe);
        if self.combat.heroes[ci]
            .held
            .is_some_and(|h| h.kind.freed_by_damage())
        {
            self.combat.heroes[ci].held = None;
            self.heroes[fi].fight.held = None;
        }
        if self.hero_slot(by).is_some() {
            let bi = self.combat.hero_index(by);
            self.combat.heroes[bi].combat_for = linger;
        }
        if !silent {
            self.make_noise(by, at, Loudness::Hit);
        }
        self.wound_by(foe, amount, Some(UnitRef::Hero(by)), crit, ambush);
    }

    /// Hero `hi`'s holds, slow and bleeds run down a step; a bleed's tick strikes. The fallen
    /// lose them all.
    pub(crate) fn tick_foe_holds(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let up = self.heroes[hi].fight.up();
        let hc = &mut self.combat.heroes[ci];
        if !up {
            hc.held = None;
            hc.slow = None;
            hc.burns.clear();
            hc.swing_at = None;
            self.heroes[hi].fight.held = None;
            return;
        }
        if let Some(h) = &mut hc.held {
            h.left -= DT;
        }
        hc.held = hc.held.filter(|h| h.left > 1e-4);
        if let Some(s) = &mut hc.slow {
            s.left -= DT;
        }
        hc.slow = hc.slow.filter(|s| s.left > 1e-4);
        let mut ticks: Vec<(PlayerId, f32, bool)> = Vec::new();
        for b in &mut hc.burns {
            b.left -= DT;
            b.next -= DT;
            if b.next <= 1e-4 {
                b.next += b.every;
                ticks.push((b.by, b.amount, b.silent));
            }
        }
        hc.burns.retain(|b| b.left > 1e-4);
        self.heroes[hi].fight.held = self.combat.heroes[ci].held.map(|h| h.kind);
        for (by, amount, silent) in ticks {
            self.strike_foe(by, player, amount, false, silent, false);
        }
    }

    /// Hero `hi`'s pace under a hostile hero's slow and hold, after the fight's own: slower by
    /// the slow's share, feet held by any hold.
    pub(crate) fn foe_pace(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let Some(hc) = self.combat.hero(player) else {
            return;
        };
        let (held, slow) = (hc.held.is_some(), hc.slow.map(|s| s.by));
        let h = &mut self.heroes[hi];
        if let Some(by) = slow {
            h.speed *= (1.0 - by).max(0.0);
        }
        if held {
            h.fight.rooted = true;
        }
    }

    /// Hero `hi`'s auto-attack at the hostile hero it swings at: in melee reach, facing them,
    /// out of stealth, a blow of its class's swing every so often.
    pub(crate) fn swing_foe(&mut self, hi: usize) {
        let content = self.content.clone();
        let player = self.heroes[hi].player;
        let Some(blow) = content.classes[self.heroes[hi].class].swing else {
            return;
        };
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.foe_swing_in = (hc.foe_swing_in - DT).max(0.0);
        let Some(foe) = hc.swing_at else {
            return;
        };
        let target = self
            .hero(foe)
            .filter(|t| t.fight.up() && self.hostile(player, foe))
            .map(|t| (t.pos, t.facing));
        let Some((at, at_facing)) = target else {
            self.combat.heroes[ci].swing_at = None;
            return;
        };
        let h = &self.heroes[hi];
        let r = &content.rules.combat;
        let held = self.combat.heroes[ci].held.is_some_and(|x| x.kind.out());
        if !h.fight.up() || h.stealthed || held || h.fight.casting.is_some() {
            return;
        }
        if self.combat.heroes[ci].foe_swing_in > 0.0
            || !in_range(h.pos.distance(at), MELEE, r)
            || !stands_right(Facing::Front, h.pos, h.facing, at, at_facing, r.front)
        {
            return;
        }
        self.combat.heroes[ci].foe_swing_in = blow.every;
        let amount = self.combat.dice.between(blow.min, blow.max);
        self.events.push(Event::Swing {
            by: UnitRef::Hero(player),
            to: UnitRef::Hero(foe),
        });
        self.strike_foe(player, foe, amount, false, false, true);
    }
}
```

(Import `in_range` and `stands_right` the way `heroes.rs` imports them, if `combat/mod.rs` does not re-export them at `super::`. If the run loop's `strike` multiplies a hero's blow by their gear's damage share (`self.gear_damage(by)`), `strike_foe` does the same to its `amount` before the crit roll, so gear counts against rivals as it does against enemies.)

- [ ] **Step 4: Presses, casts and landings reach hostile heroes**

In `crates/nightfall-sim/src/combat/heroes.rs`:

- `Aimed` gains `pub(crate) foe: Option<PlayerId>,` after `enemy` (doc: `Its hostile hero.`); every `Aimed { … }` literal gains `foe: None,` except the one `step_cast` rebuilds from its `CastState`, which gains `foe: cs.foe,`.
- In `check`, right after `if !f.up() { return Err(Refusal::Down); }`, add:

```rust
        if self.combat.hero(hero.player).is_some_and(|hc| hc.held.is_some_and(|h| h.kind.out())) {
            return Err(Refusal::Held);
        }
```

  and in its `Aim::Enemy` arm `Some(UnitRef::Hero(_)) => return Err(Refusal::BadTarget),` becomes `Some(UnitRef::Hero(p)) => return self.check_foe(hi, ability, p),`.
- In `begin`, `let target = aimed.enemy.map(|ei| UnitRef::Enemy(self.enemies[ei].id));` becomes `let target = aimed.enemy.map(|ei| UnitRef::Enemy(self.enemies[ei].id)).or(aimed.foe.map(UnitRef::Hero));`, and its `CastState { … }` literal gains `foe: aimed.foe,`.
- `go` becomes:

```rust
    /// It goes off (or a channel ticks): it lands, and every client sees it.
    fn go(&mut self, hi: usize, aimed: Aimed) {
        let player = self.heroes[hi].player;
        let hits = match aimed.foe {
            Some(foe) => self.land_at_foe(player, aimed.ability, foe),
            None => self.land(player, aimed.ability, aimed.enemy, aimed.at),
        };
        self.events.push(Event::CastGo {
            by: UnitRef::Hero(player),
            ability: aimed.ability as u16,
            hits,
            at: aimed.at,
        });
    }
```

- In `step_cast`, after `if cs.enemy.is_some_and(|ei| self.enemies[ei].hp <= 0.0) { return self.stop_cast(hi, Stop::TargetLost); }` add:

```rust
        if cs.foe.is_some_and(|p| !self.hero(p).is_some_and(|h| h.fight.up())) {
            return self.stop_cast(hi, Stop::TargetLost);
        }
```

  and where it checks at a cast's end that the target is still seen (`self.in_sight_of(hi, cs.enemy, …)`), the condition gains `&& self.foe_in_sight(hi, cs.foe, a.range)`.
- In `step_fighters`' loop body, add `self.tick_foe_holds(hi);` right after `self.tick_hero(hi);`, `self.swing_foe(hi);` right after `self.swing(hi);`, and `self.foe_pace(hi);` right after `self.set_pace(hi);`.

In `crates/nightfall-sim/src/combat/effects.rs`:

- `fn land_on` and `fn land_self` become `pub(crate) fn`.
- In `land_on`'s `Effect::Combo(n)` arm, add `f.combo_hero = None;` after `f.combo_on = Some(id);`.
- `land` becomes:

```rust
    pub(crate) fn land(
        &mut self,
        player: PlayerId,
        ability: usize,
        enemy: Option<usize>,
        at: Option<V3>,
    ) -> Vec<UnitRef> {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return Vec::new();
        };
        let centre = match a.aim {
            Aim::Ground => at.map(V3::ground),
            Aim::Me | Aim::Enemy => Some(self.heroes[hi].pos),
        };
        let reaches = a.effects.iter().any(Effect::on_enemy);
        let targets: Vec<usize> = match (a.aim, centre) {
            (Aim::Enemy, _) => enemy.into_iter().collect(),
            (_, Some(c)) if reaches && a.radius > 0.0 => self.enemies_within(c, a.radius),
            _ => Vec::new(),
        };
        // Area abilities reach the hostile heroes in them too (spec §7.4).
        let foes: Vec<PlayerId> = match (a.aim, centre) {
            (Aim::Enemy, _) => Vec::new(),
            (_, Some(c)) if reaches && a.radius > 0.0 => self.foes_within(player, c, a.radius),
            _ => Vec::new(),
        };
        // A finisher spends every combo point at once.
        let combo = if a.needs.combo {
            let f = &mut self.heroes[hi].fight;
            f.combo_on = None;
            f.combo_hero = None;
            std::mem::take(&mut f.combo)
        } else {
            0
        };
        for &ei in &targets {
            self.land_on(player, ability, ei, combo);
        }
        for &p in &foes {
            self.land_foe(player, ability, p, combo);
        }
        for effect in &a.effects {
            self.land_self(player, effect, centre, a.radius);
        }
        let mut hits: Vec<UnitRef> = targets
            .iter()
            .map(|&ei| UnitRef::Enemy(self.enemies[ei].id))
            .collect();
        hits.extend(foes.into_iter().map(UnitRef::Hero));
        hits
    }
```

- `land_self`'s match gains, before its `_ => {}`:

```rust
            Effect::Heal { min, max } => {
                let amount = self.combat.dice.between(min, max);
                let h = &mut self.heroes[hi];
                let before = h.hp;
                h.hp = (h.hp + amount).min(h.max_hp);
                let healed = h.hp - before;
                if healed > 0.0 {
                    self.events.push(Event::Healed {
                        player,
                        amount: healed,
                    });
                }
            }
```

- `land_self`'s `Effect::Blink` arm gains, after its `self.events.push(Event::Blinked { player, to });`:

```rust
                // A rival's body is the host's to move: its bot walks on from there.
                if self.heroes[hi].side.is_rival() {
                    let h = &mut self.heroes[hi];
                    h.pos = to.ground();
                    h.z = to.z;
                }
```

In `crates/nightfall-sim/src/combat/falling.rs`, `wound` becomes a call to a fuller one:

```rust
    /// `player`'s hero takes `amount` damage `by` something (see [`World::wound_by`]).
    pub(crate) fn wound(&mut self, player: PlayerId, amount: f32, by: Option<UnitRef>) {
        self.wound_by(player, amount, by, false, false);
    }

    /// `player`'s hero takes `amount` damage `by` something, a crit or an ambush as shown: it
    /// breaks their stealth (spec §6.3), ends their meal, keeps them in combat, and at 0 health
    /// downs them. Nothing happens to a hero who is not up.
    pub(crate) fn wound_by(
        &mut self,
        player: PlayerId,
        amount: f32,
        by: Option<UnitRef>,
        crit: bool,
        ambush: bool,
    ) {
```

  with the old body of `wound` under it, its `Event::Damage { … }` literal's `crit: false, ambush: false,` becoming `crit, ambush,` (Task 4's `rival_struck` lines stay where they were).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 7 new rivals tests and every earlier one: no earlier ability lands on a hero).
Run: `cargo build -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall/fight.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: heroes and rivals fight with the same abilities; holds, slows and bleeds on heroes; heals

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: A rival's big move, telegraphed

**Files:**
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (`Telegraph::by`), `crates/nightfall-sim/src/combat/heroes.rs` (`begin`, `step_cast`, `stop_cast`), `crates/nightfall-sim/src/combat/strikes.rs` (`begin_move`'s literal, `telegraph_lands`), `crates/nightfall-sim/src/combat/effects.rs` (`kill_enemy`), `crates/nightfall-sim/src/combat/foes.rs`, `crates/nightfall-sim/src/rivals/tests.rs`

**Interfaces:**
- Consumes: `Telegraph { id, enemy, ability, shape, at, facing, left, total }`, `Combat::next_telegraph`, `AbilityDef::{windup, telegraph}`, `TelegraphDef { shape, windup, on_self }`, `Shape::{contains, Cone}`, `HeroCombat::immune`, `Event::{Evaded, CastGo, CastStart}`, `land_telegraphs`, `land_foe`, `land_on`, `pay` (earlier plans, Task 5).
- Produces: `Telegraph::by: Option<PlayerId>` (a hero's big move: its caster; `enemy` is 0 then); crate-only `World::{put_telegraph(&mut self, hi, &Aimed, TelegraphDef, windup: f32), foe_telegraph_lands(&mut self, by: PlayerId, Telegraph)}`. A hero's telegraphed ability casts for its windup, rooted where its `moving` says, and lands as its telegraph does: on every hostile hero inside (unless their dodge covers it, `Evaded`) and every enemy inside; a cast stopped early takes its telegraph away.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs`:

```rust
#[test]
fn a_rivals_big_move_shows_first_then_lands_on_its_foes_inside() {
    let mut w = world("", &format!("{}\n", sentry_line(43.0, 60.0)));
    join(&mut w, 0, 37.0, 60.0);
    join(&mut w, 1, 30.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    let friend = rival(&mut w, 1, "digger", 0, 41.0, 61.0);
    w.drain_events();
    let sentry_hp = w.enemies[0].hp;
    // The shovel smash (action 4): a 6-yard circle around the digger, 1.2 s to land.
    press(&mut w, a, 1, 4, None);
    w.step();
    let t = *w.telegraphs.iter().find(|t| t.by == Some(a)).expect("shown on the ground");
    assert!((t.total - 1.2).abs() < 1e-4);
    assert!(w.drain_events().iter().all(|e| !matches!(e, Event::Damage { .. })));
    steps(&mut w, secs(1.3));
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(0)).len(), 1, "inside: {events:?}");
    assert!(damage_to(&events, UnitRef::Hero(1)).is_empty(), "10 yards off: outside");
    assert!(damage_to(&events, UnitRef::Hero(friend)).is_empty(), "its friend is spared");
    assert!(w.enemies[0].hp < sentry_hp, "a monster inside is struck too");
    assert!(w.telegraphs.iter().all(|t| t.by != Some(a)));
}

#[test]
fn a_hero_who_dodges_sees_the_big_move_miss() {
    let mut w = world("", "");
    join(&mut w, 0, 37.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    w.drain_events();
    press(&mut w, a, 1, 4, None);
    w.step();
    steps(&mut w, secs(1.0));
    // The dodge, as a client counts it, just before the blow lands.
    w.set_input(
        0,
        PlayerInput {
            dodge_presses: 1,
            ..Default::default()
        },
    );
    steps(&mut w, secs(0.4));
    let events = w.drain_events();
    assert!(damage_to(&events, UnitRef::Hero(0)).is_empty(), "{events:?}");
    assert!(events.iter().any(|e| matches!(e, Event::Evaded { player: 0, .. })));
}

#[test]
fn a_big_move_cut_short_takes_its_telegraph_with_it() {
    let mut w = world("", "");
    join(&mut w, 0, 37.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    w.drain_events();
    press(&mut w, a, 1, 4, None);
    w.step();
    assert!(w.telegraphs.iter().any(|t| t.by == Some(a)));
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    assert!(w.telegraphs.iter().all(|t| t.by != Some(a)));
    steps(&mut w, secs(1.5));
    assert!(damage_to(&w.drain_events(), UnitRef::Hero(0)).is_empty());
}
```

(The dodge test's input has no pose: the hero stays where it was, and the host takes the dodge count; the second dodge count a host sees is the first press, since the hero's first input was its baseline.)

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`Telegraph::by` does not exist).

- [ ] **Step 2: A hero's telegraph**

In `crates/nightfall-sim/src/combat/mod.rs`, `Telegraph` gains after `enemy`:

```rust
    /// A hero's big move (a rival's, spec §7.4): its caster. `enemy` is 0 then.
    pub by: Option<PlayerId>,
```

In `crates/nightfall-sim/src/combat/strikes.rs`, `begin_move`'s `Telegraph { … }` literal gains `by: None,`, and `telegraph_lands` begins with:

```rust
        if let Some(by) = t.by {
            return self.foe_telegraph_lands(by, t);
        }
```

In `crates/nightfall-sim/src/combat/effects.rs`'s `kill_enemy`, `self.telegraphs.retain(|t| t.enemy != id);` becomes `self.telegraphs.retain(|t| t.by.is_some() || t.enemy != id);`.

In `crates/nightfall-sim/src/combat/heroes.rs`:

- In `begin`, `if a.cast <= 0.0 && !channel {` becomes `if a.windup() <= 0.0 && !channel {`, `let total = if channel { a.channel } else { a.cast };` becomes `let total = if channel { a.channel } else { a.windup() };`, and after the `self.combat.heroes[ci].cast = Some(CastState { … });` statement add:

```rust
        if let Some(t) = a.telegraph {
            self.put_telegraph(hi, &aimed, t, total);
        }
```

- In `step_cast`, the branch `} else if cs.elapsed + 1e-4 >= a.cast {` becomes `} else if cs.elapsed + 1e-4 >= a.windup() {`, and that branch's body begins with:

```rust
            // A telegraphed blow lands as its telegraph does ([`World::foe_telegraph_lands`]):
            // the cast only pays.
            if a.telegraph.is_some() {
                self.combat.heroes[ci].cast = None;
                self.heroes[hi].fight.casting = None;
                self.pay(hi, aimed);
                return;
            }
```

- `stop_cast` gains, after `self.combat.heroes[ci].cast = None;`:

```rust
        // A big move cut short does not land.
        if why != Stop::Finished {
            self.telegraphs.retain(|t| t.by != Some(player));
        }
```

Append to `crates/nightfall-sim/src/combat/foes.rs`'s `impl World` (adding `Telegraph`, `TelegraphDef`, `Shape` to its imports):

```rust
    /// Hero `hi`'s telegraph for `aimed`, landing in `windup` seconds: around them for one on
    /// themselves or a cone, else under their target as it stands now; turned toward the target.
    pub(crate) fn put_telegraph(&mut self, hi: usize, aimed: &Aimed, t: TelegraphDef, windup: f32) {
        let h = &self.heroes[hi];
        let (player, from) = (h.player, V3::at(h.pos, h.z));
        let target = aimed
            .foe
            .and_then(|p| self.hero(p))
            .map(|t| V3::at(t.pos, t.z))
            .or(aimed
                .enemy
                .map(|ei| V3::at(self.enemies[ei].pos, self.enemies[ei].z)))
            .or(aimed.at);
        let at = if t.on_self || matches!(t.shape, Shape::Cone { .. }) {
            from
        } else {
            target.unwrap_or(from)
        };
        let facing = target
            .map(|p| (p.ground() - h.pos).normalize_or_zero())
            .filter(|f| *f != V2::ZERO)
            .unwrap_or(h.facing);
        let id = self.combat.next_telegraph;
        self.combat.next_telegraph += 1;
        self.telegraphs.push(Telegraph {
            id,
            enemy: 0,
            by: Some(player),
            ability: aimed.ability as u16,
            shape: t.shape,
            at,
            facing,
            left: windup,
            total: windup,
        });
    }

    /// Hero `by`'s telegraph `t` lands: on every hostile hero up inside it (those whose dodge's
    /// moment covers it see it miss) and every enemy alive inside it, with the ability's own
    /// effects. Nothing lands from a caster no longer up.
    pub(crate) fn foe_telegraph_lands(&mut self, by: PlayerId, t: Telegraph) {
        if !self.hero(by).is_some_and(|h| h.fight.up()) {
            return;
        }
        let ability = usize::from(t.ability);
        let at = t.at.ground();
        let inside: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.fight.up() && t.shape.contains(at, t.facing, h.pos))
            .filter(|h| self.hostile(by, h.player))
            .map(|h| h.player)
            .collect();
        let mut hits = Vec::new();
        for p in inside {
            if self.combat.hero(p).is_some_and(|c| c.immune > 0.0) {
                self.events.push(Event::Evaded { player: p, enemy: 0 });
                continue;
            }
            self.land_foe(by, ability, p, 0);
            hits.push(UnitRef::Hero(p));
        }
        let struck: Vec<usize> = (0..self.enemies.len())
            .filter(|&ei| {
                self.enemies[ei].hp > 0.0 && t.shape.contains(at, t.facing, self.enemies[ei].pos)
            })
            .collect();
        for ei in struck {
            self.land_on(by, ability, ei, 0);
            hits.push(UnitRef::Enemy(self.enemies[ei].id));
        }
        self.events.push(Event::CastGo {
            by: UnitRef::Hero(by),
            ability: t.ability,
            hits,
            at: Some(t.at),
        });
    }
```

If any other `Telegraph { … }` literal exists (`grep -rn "Telegraph {" crates/nightfall-sim crates/benilla-app/src/nightfall`, tests included), it gains `by: None,`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 3 new tests and every earlier one: no hero's ability had a telegraph before).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: a rival's big move shows on the ground before it lands, and can be dodged

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Rivals' minds, both ways, and the hero's view of them

**Files:**
- Create: `crates/nightfall-sim/src/rivals/mind.rs`
- Modify: `crates/nightfall-sim/src/rivals/mod.rs` (`mod mind;`, `rival_struck`), `crates/nightfall-sim/src/rivals/tests.rs`
- Modify: `crates/nightfall-sim/src/senses/react.rs` (`sense` shared), `crates/nightfall-sim/src/senses/mod.rs` (re-exports), `crates/nightfall-sim/src/combat/foes.rs` (`foe_unaware`, `land_foe`'s alert), `crates/nightfall-sim/src/combat/effects.rs` (Vanish), `crates/nightfall-sim/src/world.rs` (`step`, `Snapshot::rivals`, `apply_snapshot`), `crates/nightfall-sim/src/sight.rs` (`snapshot_for`), `crates/nightfall-sim/src/messages.rs` (`PROTOCOL`, its tests' `Snapshot` literals)

**Interfaces:**
- Consumes: `react::sense`, `Mind::{think, alert, lose, mark, alerted_to, is_alerted, level, state, of}`, `Sensed`, `Shout`, `State`, `notice_range`, `AwarenessRules::{shout, suspicious}`, `SenseRules::ambush`, `World::{fog, notice_scale, clear_line, map, side_of, hostile, snapshot, snapshot_for, apply_snapshot}`, `Senses::{noises, lose}`, `WorldRules::share_sight_within`, `EYE_HEIGHT`, `CHEST_HEIGHT`, `PROTOCOL = 7` (earlier plans); `Rivals`, `RivalBody`, `RivalView`, `RivalMark`, `Side` (Task 4); `foe_unaware`, `land_foe` (Task 5).
- Produces:
  - `senses::sense(from: V2, facing: V2, mind: &Mind, eye: V3, h: &Hero, noises: &[Noise], map: &HeightMap, fog: f32, notice: f32, r: &SenseRules) -> Sensed` (crate-only, re-exported from `senses` with `Mind`, `Sensed`, `State`, `notice_range`).
  - Crate-only `World::{perceive_rivals(&mut self), alert_rival(&mut self, rival: PlayerId, by: PlayerId), rivals_lose(&mut self, PlayerId), rival_mind(&self, PlayerId) -> Option<&Mind>}`; `World::rival_view(&self, player: PlayerId) -> RivalView` (public).
  - `Snapshot::rivals: RivalView` (each player's own, from `snapshot_for`); `PROTOCOL = 8`.
  - `foe_unaware` asks a rival's mind (not alerted to the attacker); a hero's blow or a hostile ability that `alerts` alerts the rival and its party within `shout` yards, and marks the party as struck; Vanish loses rivals' minds too.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs` (merge `Alert` into its `crate::{…}` import, and `house` into its `testmap` import):

```rust
/// Walk `player` from `from` to `to` over `n` steps at `gait`, facing the way it goes, with
/// `stealth_presses` in every input.
fn walk(w: &mut World, player: PlayerId, from: V2, to: V2, n: usize, gait: Gait, presses: u8) {
    let way = (to - from).normalize_or_zero();
    for i in 1..=n {
        let at = from + (to - from) * (i as f32 / n as f32);
        w.set_input(
            player,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: at,
                    facing: way,
                    z: 0.0,
                }),
                gait,
                stealth_presses: presses,
                ..Default::default()
            },
        );
        w.step();
    }
}

#[test]
fn a_sneaking_rogue_passes_behind_a_rival_and_a_running_one_is_heard() {
    let mut w = world("", "");
    join(&mut w, 0, 54.0, 40.0);
    join(&mut w, 1, 54.0, 39.0);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 60.0);
    put(&mut w, a, 60.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    // Hero 0 steps into stealth, then sneaks past 6 yards behind the rival.
    walk(&mut w, 0, V2::new(54.0, 40.0), V2::new(54.0, 41.0), 3, Gait::Still, 1);
    assert!(w.hero(0).unwrap().stealthed);
    walk(&mut w, 0, V2::new(54.0, 41.0), V2::new(54.0, 80.0), secs(16.0), Gait::Walk, 1);
    let r = w.content().rules.senses.awareness.clone();
    assert!(w.rival_mind(a).unwrap().level(0) < r.suspicious, "unnoticed and unheard");
    // Hero 1 runs the same way: its footsteps carry 12 yards.
    walk(&mut w, 1, V2::new(54.0, 39.0), V2::new(54.0, 80.0), secs(6.0), Gait::Run, 0);
    assert!(w.rival_mind(a).unwrap().level(1) >= r.suspicious, "heard running past");
}

#[test]
fn a_sentry_hears_a_rival_walk_past_and_strikes_it_once_it_is_found() {
    let mut w = world("", &format!("{}\n", r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (60.0, 60.0), facing: 0.0)),"#));
    join(&mut w, 0, 10.0, 10.0);
    // Three yards behind the sentry: inside its hearing of a walking step (4 yards).
    let a = rival(&mut w, 0, "digger", 0, 57.0, 50.0);
    walk(&mut w, a, V2::new(57.0, 50.0), V2::new(57.0, 70.0), secs(8.0), Gait::Walk, 0);
    assert!(!w.marks_for(a).is_empty(), "the rival's footsteps were heard");
    // In front of it, the sentry finds the rival and goes for it as for a hero.
    walk(&mut w, a, V2::new(57.0, 70.0), V2::new(62.0, 60.0), secs(2.0), Gait::Walk, 0);
    steps(&mut w, secs(8.0));
    assert!(w.marks_for(a).iter().any(|m| m.alert == Alert::Alerted));
    let events = w.drain_events();
    assert!(!damage_to(&events, UnitRef::Hero(a)).is_empty(), "struck: {events:?}");
}

#[test]
fn an_unaware_rival_takes_an_ambush_and_its_party_hears_of_it() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 23.0, 60.0);
    let b = rival(&mut w, 1, "digger", 0, 30.0, 60.0);
    put(&mut w, a, 23.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    w.drain_events();
    press(&mut w, 0, 1, 2, Some(UnitRef::Hero(a)));
    w.step();
    let first = w.drain_events();
    let hit = first.iter().find_map(|e| match e {
        Event::Damage { to, amount, ambush, .. } if *to == UnitRef::Hero(a) => Some((*amount, *ambush)),
        _ => None,
    });
    assert_eq!(hit, Some((75.0, true)), "(30 + 20) × 1.5 from behind, unaware");
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    assert!(w.rival_mind(b).unwrap().alerted_to(0), "the shout reached its friend");
    steps(&mut w, secs(1.1));
    press(&mut w, 0, 2, 2, Some(UnitRef::Hero(a)));
    w.step();
    let second = w.drain_events();
    assert!(second.iter().any(|e| matches!(e, Event::Damage { to, ambush: false, .. } if *to == UnitRef::Hero(a))));
}

#[test]
fn the_hero_sees_rivals_in_the_open_not_behind_walls_nor_far_in_stealth() {
    let mut b = flat(120.0);
    house(&mut b, V2::new(30.0, 70.0), V2::new(35.0, 90.0), None, false);
    let mut w = world_on(b.finish(), "", "", |c| RaidRoll::standard(c, 1));
    join(&mut w, 0, 20.0, 60.0);
    let open = rival(&mut w, 0, "digger", 0, 40.0, 60.0);
    // The line from (20, 60) to (40, 85) crosses the house's west wall at y 72.5.
    let walled = rival(&mut w, 1, "digger", 0, 40.0, 85.0);
    let sneak = rival(&mut w, 2, "blade", 0, 30.0, 50.0);
    walk(&mut w, sneak, V2::new(30.0, 50.0), V2::new(30.0, 50.0), 3, Gait::Still, 1);
    assert!(w.hero(sneak).unwrap().stealthed);
    put(&mut w, 0, 20.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    let seen = w.snapshot_for(0).rivals.seen;
    assert!(seen.contains(&open));
    assert!(!seen.contains(&walled), "behind the house");
    assert!(!seen.contains(&sneak), "a still blade in stealth, 14 yards off");
    put(&mut w, 0, 29.0, 51.0, V2::new(1.0, 0.0), Gait::Still);
    w.step();
    assert!(w.snapshot_for(0).rivals.seen.contains(&sneak), "close enough to make it out");
}

#[test]
fn a_rival_marks_the_hero_it_suspects_then_is_alerted_to() {
    let mut w = world("", "");
    join(&mut w, 0, 40.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 60.0, 60.0);
    put(&mut w, a, 60.0, 60.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    // Running toward it, in its front cone.
    walk(&mut w, 0, V2::new(40.0, 60.0), V2::new(48.0, 60.0), secs(1.0), Gait::Run, 0);
    let marks = w.rival_view(0).marks;
    assert!(marks.iter().any(|m| m.rival == a), "{marks:?}");
    walk(&mut w, 0, V2::new(48.0, 60.0), V2::new(50.0, 60.0), secs(4.0), Gait::Run, 0);
    assert!(w
        .rival_view(0)
        .marks
        .contains(&crate::RivalMark { rival: a, alert: Alert::Alerted }));
}

#[test]
fn a_vanish_makes_rivals_lose_the_rogue() {
    let mut w = world("", "");
    join(&mut w, 0, 20.0, 60.0);
    let a = rival(&mut w, 0, "digger", 0, 30.0, 60.0);
    w.alert_rival(a, 0);
    assert!(w.rival_mind(a).unwrap().alerted_to(0));
    w.rivals_lose(0);
    assert!(!w.rival_mind(a).unwrap().is_alerted());
}
```

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`rival_mind`, `rival_view`, `alert_rival`, `rivals_lose`, `Snapshot::rivals` do not exist).

- [ ] **Step 2: The senses' `sense`, shared**

In `crates/nightfall-sim/src/senses/react.rs`:

- `fn sense(e: &Enemy, mind: &Mind, eye: V3, h: &Hero, …)` becomes `pub(crate) fn sense(from: V2, facing: V2, mind: &Mind, eye: V3, h: &Hero, …)` (the rest of its parameters as they are); in its body `e.pos` becomes `from` and `e.facing` becomes `facing`; its doc becomes: `What a looker at `from`, facing `facing`, with `mind`, sees and hears of hero `h` this step: an enemy, or a rival (spec §7.4: the same rules both ways).`
- Every call `sense(e, mind, …)` in the file becomes `sense(e.pos, e.facing, mind, …)` (the look-and-think loop's, and the Dark Riders' block's).

In `crates/nightfall-sim/src/senses/mod.rs`, beside its other `use` lines, add whichever of these it lacks: `pub(crate) use awareness::{Mind, Sensed, Shout, State};`, `pub(crate) use react::sense;`, `pub(crate) use visibility::notice_range;`.

- [ ] **Step 3: The rivals' minds**

`crates/nightfall-sim/src/rivals/mind.rs`:

```rust
//! Rivals' senses (spec §7.4: "the same sight, noise and awareness rules as heroes"), both
//! ways. Enemies see and hear rivals already: rivals are heroes to them. This file is the other
//! way and the heroes' view: each rival keeps a [`Mind`] of the party's heroes, fed by the
//! senses' own [`sense`] (the notice range by the hero's visibility and the rival's facing, line
//! of sight within the fog, noises heard through walls at half their radius); a rival that
//! becomes alerted shouts to its party; a hero's blow alerts it, and lands as an ambush while it
//! is unaware; a Vanish loses it. A hero sees a rival in the fog and in line of sight, a
//! stealthed one only within the notice range its visibility gives (spec §5.1), as an enemy
//! sees a stealthed hero; the rivals' marks ("?" and "!") are each player's, as enemies' are.

use super::{RivalMark, RivalView, Side};
use crate::senses::{notice_range, sense, Mind, Sensed};
use crate::{Hero, PlayerId, SenseRules, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

impl World {
    /// Every rival up and in the fight senses the party's heroes and thinks (spec §5.3); one that
    /// becomes alerted shouts to its party within `shout` yards. Last in a step, while the
    /// step's noises are still about. A rival held out of the fight neither looks nor thinks.
    pub(crate) fn perceive_rivals(&mut self) {
        let Some(map) = self.map().cloned() else {
            return;
        };
        let content = self.content().clone();
        let r = &content.rules.senses;
        let fog = self.fog().end;
        let notice = self.notice_scale();
        let mut shouts: Vec<(u8, V2, PlayerId, V3)> = Vec::new();
        for b in 0..self.rivals.bodies.len() {
            let player = self.rivals.bodies[b].player;
            let Some(me) = self.hero(player) else {
                continue;
            };
            let out = self
                .combat
                .hero(player)
                .is_some_and(|c| c.held.is_some_and(|h| h.kind.out()));
            if !me.fight.up() || out {
                continue;
            }
            let (pos, facing) = (me.pos, me.facing);
            let eye = V3::at(me.pos, me.z + EYE_HEIGHT);
            let party = self.rivals.bodies[b].party;
            let mind = &self.rivals.bodies[b].mind;
            let sensed: Vec<Sensed> = self
                .heroes
                .iter()
                .filter(|h| h.side == Side::Party)
                .map(|h| sense(pos, facing, mind, eye, h, &self.senses.noises, &map, fog, notice, r))
                .collect();
            if let Some(s) = self.rivals.bodies[b].mind.think(&sensed, DT, &r.awareness) {
                shouts.push((party, pos, s.player, s.at));
            }
        }
        for (party, from, player, at) in shouts {
            self.party_shout(party, from, player, at);
        }
    }

    /// Every member of rival party `party` up within `shout` yards of `from`, not yet alerted, is
    /// alerted to `player`, last seen at `at`. Shouts do not chain, as enemies' do not.
    fn party_shout(&mut self, party: u8, from: V2, player: PlayerId, at: V3) {
        let content = self.content().clone();
        let r = &content.rules.senses.awareness;
        let near: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.side == Side::Rival(party) && h.fight.up())
            .filter(|h| h.pos.distance(from) <= r.shout)
            .map(|h| h.player)
            .collect();
        for b in &mut self.rivals.bodies {
            if near.contains(&b.player) && !b.mind.is_alerted() {
                b.mind.alert(player, at, r);
            }
        }
    }

    /// Rival `rival` is alerted to `by` where `by` stands now (a blow, a hostile ability), its
    /// party is told within `shout` yards, and the party counts as struck by a hero.
    pub(crate) fn alert_rival(&mut self, rival: PlayerId, by: PlayerId) {
        let Side::Rival(party) = self.side_of(rival) else {
            return;
        };
        if !self.hostile(rival, by) {
            return;
        }
        let (Some(at), Some(from)) = (
            self.hero(by).map(|h| V3::at(h.pos, h.z)),
            self.hero(rival).map(|h| h.pos),
        ) else {
            return;
        };
        let content = self.content().clone();
        if let Some(b) = self.rivals.body_mut(rival) {
            b.mind.alert(by, at, &content.rules.senses.awareness);
        }
        self.party_shout(party, from, by, at);
        for p in &mut self.rivals.parties {
            if p.number == party {
                p.hurt_by = Some(by);
            }
        }
    }

    /// Every rival's mind loses `player` (their Vanish): an alert turns to a search where they
    /// were last seen.
    pub(crate) fn rivals_lose(&mut self, player: PlayerId) {
        let content = self.content().clone();
        for b in &mut self.rivals.bodies {
            b.mind.lose(player, &content.rules.senses.awareness);
        }
    }

    /// Rival `player`'s mind.
    pub(crate) fn rival_mind(&self, player: PlayerId) -> Option<&Mind> {
        self.rivals.body(player).map(|b| &b.mind)
    }

    /// What `player`'s client is told of the rivals: those their hero sees (spec §4.3) and the
    /// marks living rivals show them. On a client's mirror, what its host sent.
    pub fn rival_view(&self, player: PlayerId) -> RivalView {
        if let Some(v) = &self.rivals.mirror {
            return v.clone();
        }
        let r = &self.content().rules.senses.awareness;
        RivalView {
            seen: self.rivals_seen_by(player),
            marks: self
                .rivals
                .bodies
                .iter()
                .filter(|b| self.hero(b.player).is_some_and(|h| h.fight.up()))
                .filter_map(|b| {
                    b.mind
                        .mark(player, r)
                        .map(|alert| RivalMark { rival: b.player, alert })
                })
                .collect(),
        }
    }

    /// The rivals `player`'s hero sees, or a teammate of theirs within the shared-sight reach.
    fn rivals_seen_by(&self, player: PlayerId) -> Vec<PlayerId> {
        let Some(me) = self.hero(player) else {
            return Vec::new();
        };
        let content = self.content();
        let share = content.rules.world.share_sight_within;
        let fog = self.fog().end;
        let watchers: Vec<&Hero> = self
            .heroes
            .iter()
            .filter(|h| {
                h.player == player || (h.side == me.side && h.pos.distance(me.pos) <= share)
            })
            .collect();
        self.heroes
            .iter()
            .filter(|h| h.side.is_rival() && h.side != me.side)
            .filter(|h| {
                watchers
                    .iter()
                    .any(|w| self.sees_rival(w, h, fog, &content.rules.senses))
            })
            .map(|h| h.player)
            .collect()
    }

    /// Whether `w` sees rival `h`: within the fog (a stealthed one within the notice range its
    /// visibility gives, faced), and nothing solid between `w`'s eyes and its chest.
    fn sees_rival(&self, w: &Hero, h: &Hero, fog: f32, r: &SenseRules) -> bool {
        let range = if h.stealthed {
            notice_range(h.visibility, 1.0, fog, r)
        } else {
            fog
        };
        w.pos.distance(h.pos) <= range
            && self.clear_line(
                V3::at(w.pos, w.z + EYE_HEIGHT),
                V3::at(h.pos, h.z + CHEST_HEIGHT),
            )
    }
}
```

In `crates/nightfall-sim/src/rivals/mod.rs`: add `mod mind;` below `mod kit;`, and `rival_struck` becomes:

```rust
    /// Rival `player` was struck by `by`: if a hero of the party, it is remembered as the one who
    /// slew it should it die of it, and the rival and its party are alerted to them.
    pub(crate) fn rival_struck(&mut self, player: PlayerId, by: PlayerId) {
        if !(self.side_of(player).is_rival() && self.hostile(player, by)) {
            return;
        }
        if let Some(b) = self.rivals.body_mut(player) {
            b.struck_by = Some(by);
        }
        self.alert_rival(player, by);
    }
```

In `crates/nightfall-sim/src/combat/foes.rs`:

- `foe_unaware` becomes:

```rust
    /// Whether hero `target` is not aware of `by`: a rival whose mind is not alerted to them
    /// (spec §5.4, as an enemy's), a hero of the party not in combat.
    pub(crate) fn foe_unaware(&self, target: PlayerId, by: PlayerId) -> bool {
        match self.rival_mind(target) {
            Some(m) => !m.alerted_to(by),
            None => self.hero(target).is_some_and(|t| !t.fight.in_combat),
        }
    }
```

- At the end of `land_foe`, before its auto-attack lines, add:

```rust
        if a.alerts {
            self.alert_rival(foe, player);
        }
```

In `crates/nightfall-sim/src/combat/effects.rs`, `land_self`'s `Effect::Vanish` arm gains `self.rivals_lose(player);` after its `self.senses.lose(…)` line.

- [ ] **Step 4: Each player's view of the rivals in their snapshot**

In `crates/nightfall-sim/src/world.rs`:

- `Snapshot` gains, as its last field:

```rust
    /// The rivals this player's hero sees and their marks for them (spec §4.3, §5.5): each
    /// player's own, from [`World::snapshot_for`]; empty from [`World::snapshot`].
    pub rivals: RivalView,
```

- `World::snapshot`'s literal gains `rivals: RivalView::default(),`; `apply_snapshot` gains `self.rivals.mirror = Some(snapshot.rivals.clone());` before the snapshot's other fields are moved out (or take it first with `std::mem::take`).
- `step` gains, right before the night plan's `self.night_listen();` (itself right before `self.senses.noises.clear();`):

```rust
        // Rivals sense the step's noises before they go (Task 8 makes this their whole turn).
        self.perceive_rivals();
```

- `RivalView` joins the file's `crate::rivals::{…}` import.

In `crates/nightfall-sim/src/sight.rs`'s `snapshot_for`, its `Snapshot { … }` literal gains `rivals: self.rival_view(player),`. Every other `Snapshot { … }` literal (`grep -rn "Snapshot {" crates/nightfall-sim`, the messages tests included) gains `rivals: RivalView::default(),`.

In `crates/nightfall-sim/src/messages.rs`: `pub const PROTOCOL: u32 = 8;` (the snapshot changed shape; Task 9's `RaidRoll::rivals` rides on the same number, no build ships between them).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 6 new tests and every earlier one; the senses' tests unchanged by `sense`'s new parameters).

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: rivals sense heroes as enemies do and are sensed in turn; each player's view of them

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The bots: goals into inputs

**Files:**
- Create: `crates/nightfall-sim/src/rivals/drive.rs`
- Modify: `crates/nightfall-sim/src/rivals/mod.rs` (`mod drive;`, `Way::retry_at`, `Rivals::rumour`, `add_party`, `rival_debug`), `crates/nightfall-sim/src/rivals/mind.rs` (the rumour), `crates/nightfall-sim/src/rivals/tests.rs`, `crates/nightfall-sim/src/world.rs` (`nav` crate-visible; `step` calls `step_rivals`)

**Interfaces:**
- Consumes: `decide`, `next_poi`, `formation_spot`, `walk_along`, `away_from`, `WALK_RATIO`, `Goal`, `PartyView`, `FoeSeen` (Task 3); `choose`, `ready`, `Moment`, `KitSlot` (Task 2); `Rivals`, `RivalBody`, `Party`, `Way`, `add_rival_hero` (Task 4); `perceive_rivals`, `State` (Task 7); `Navigator::find_path`, `HeightMap::{walk_line, floor_near, floor_within}`, `PATROL_BUDGET`, `Senses::{target_of, noises}`, `World::{opener_allowed, may_stealth, clear_line, pack, set_input, elapsed, telegraphs, enemies}`, `Raid::{containers, searches, open_of, roll}`, `RaidRoll::exit_open`, `Container { id, holder, at, stacks, searched, lock }`, `Holder::Remains`, `Act::{Open, TakeAll, Close}`, `ActPress`, `Carry::{acted, load}`, `Inventory::first_empty`, `RunRules::weight_full`, `DodgeRules::distance`, `HeroFight::{dodge, casting, combo, combo_on, combo_hero}`, `CombatRules::{reach, front}`, `stands_right`, `Facing`, `Tag::Escape`, `Aim`, `Moving`, `ExitDef::{at, radius}`, `HuntRules::memory`, `Dice` (earlier plans).
- Produces:
  - Crate-only `World::{step_rivals(&mut self), add_party(&mut self, def: usize, number: u8, aggression: f32, at: V2, seed: u64) -> Vec<PlayerId>}`; `World::rival_debug(&self) -> Vec<String>` (public, the trace's lines).
  - Crate-only `Way::retry_at: f32`; `Rivals::rumour: Option<(V3, f32, f32)>` (the loudest noise of a hero of the party lately: where, its radius, when).
  - `World::step` gives the rivals their turn last (after the raid's end is known, before the step's noises go).

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs` (merge `Act`, `ActPress` into its `crate::{…}` import):

```rust
/// Party kind `key` brought in at `(x, y)` as party 0, at `aggression`, with dice `seed`.
pub(super) fn party(w: &mut World, key: &str, aggression: f32, x: f32, y: f32, seed: u64) -> Vec<PlayerId> {
    let def = w.content().party(key).unwrap();
    let members = w.add_party(def, 0, aggression, V2::new(x, y), seed);
    w.step();
    members
}

/// Whether the crate at `at` has been searched and emptied.
pub(super) fn emptied(w: &World, at: V2) -> bool {
    w.raid
        .containers
        .iter()
        .find(|c| c.at.distance(at) < 0.5 && matches!(c.holder, Holder::Placed(_)))
        .is_some_and(|c| c.searched && c.stacks.iter().all(Option::is_none))
}

#[test]
fn a_party_walks_around_a_house_to_its_point_and_loots_the_crate() {
    // A closed house stands between the rivals' entry and the yard.
    let mut b = flat(120.0);
    house(&mut b, V2::new(40.0, 50.0), V2::new(50.0, 70.0), None, false);
    let mut w = world_on(b.finish(), "pois: (1, 1)", "", |c| RaidRoll::standard(c, 1));
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    assert_eq!(members.len(), 2);
    let mut inside = false;
    for _ in 0..secs(80.0) {
        w.step();
        for &m in &members {
            if let Some(h) = w.hero(m) {
                inside |= (40.0..=50.0).contains(&h.pos.x) && (50.0..=70.0).contains(&h.pos.y);
            }
        }
    }
    assert!(!inside, "nobody walked through the house");
    let p = &w.rivals.parties[0];
    assert_eq!(p.visited.len(), 1);
    let poi = &w.content().rivals.pois[p.visited[0]];
    assert!(emptied(&w, V2::new(poi.at.0, poi.at.1)), "its crate searched and emptied");
}

#[test]
fn a_party_done_looting_walks_out_by_the_nearest_open_exit() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.drain_events();
    steps(&mut w, secs(120.0));
    let out = events_of(&mut w, |e| match e {
        Event::RivalOut { player, exit } => Some((*player, *exit)),
        _ => None,
    });
    assert_eq!(out.len(), 2, "{out:?}");
    for (p, _) in &out {
        assert!(members.contains(p));
    }
    assert!(members.iter().all(|&m| w.hero(m).is_none()));
}

#[test]
fn a_rival_in_a_fight_presses_once_per_choice() {
    let mut w = world("pois: (1, 1)", &format!("{}\n", r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (30.0, 60.0), facing: 180.0)),"#));
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.drain_events();
    steps(&mut w, secs(40.0));
    let events = w.drain_events();
    let refused: Vec<Refusal> = events
        .iter()
        .filter_map(|e| match e {
            Event::Refused { player, why } if members.contains(player) => Some(*why),
            _ => None,
        })
        .collect();
    let mashing = [Refusal::Busy, Refusal::NotReady, Refusal::NoPower, Refusal::Moving];
    assert!(refused.iter().all(|r| !mashing.contains(r)), "{refused:?}");
    assert!(refused.len() <= 2, "{refused:?}");
    let blows = events
        .iter()
        .filter(|e| matches!(e, Event::Damage { to: UnitRef::Enemy(_), by: Some(UnitRef::Hero(p)), .. } if members.contains(p)))
        .count();
    assert!(blows >= 5, "they fought the sentry on their way");
    assert!(events.iter().any(|e| matches!(e, Event::EnemyDied { by: Some(p), .. } if members.contains(p))));
}

#[test]
fn a_party_with_no_way_in_gives_the_point_up_and_leaves() {
    // The only point of interest is inside a sealed, roofed house.
    let rivals = RIVALS.replace(
        r#"(key: "barn", name: "The Barn", at: (90.0, 30.0), radius: 6.0)"#,
        r#"(key: "barn", name: "The Barn", at: (60.0, 60.0), radius: 3.0)"#,
    );
    let mut b = flat(120.0);
    house(&mut b, V2::new(55.0, 55.0), V2::new(65.0, 65.0), None, true);
    let c = content_with("pois: (2, 2), give_up: 5.0", &rivals, "");
    let roll = RaidRoll::standard(&c, 1);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(roll).unwrap();
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 10.0, 60.0, 7);
    w.drain_events();
    steps(&mut w, secs(90.0));
    assert_eq!(w.rivals.parties[0].visited.len(), 2, "both points given up");
    let out = events_of(&mut w, |e| matches!(e, Event::RivalOut { .. }).then_some(()));
    assert_eq!(out.len(), members.len(), "and out by an exit");
}

#[test]
fn followers_keep_behind_their_leader() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "watch", 0.8, 10.0, 60.0, 3);
    steps(&mut w, secs(3.0));
    for _ in 0..secs(10.0) {
        w.step();
        let (Some(lead), Some(follow)) = (w.hero(members[0]), w.hero(members[1])) else {
            panic!("both walk");
        };
        assert!(lead.pos.distance(follow.pos) <= 6.0, "{:?} {:?}", lead.pos, follow.pos);
    }
}

#[test]
fn a_rival_dodges_a_telegraph_it_stands_in() {
    let mut w = world(
        "dodge: 1.0, pois: (1, 1)",
        &format!("{}\n", r#"(place: "Post", kind: "smasher", behaviour: Sentry(at: (32.0, 60.0), facing: 180.0)),"#),
    );
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "diggers", 0.1, 30.0, 60.0, 7);
    let id = w.enemies[0].id;
    w.alert(id, members[0]);
    w.drain_events();
    steps(&mut w, secs(8.0));
    let events = w.drain_events();
    assert!(events.iter().any(|e| matches!(e, Event::Dodged { player } if members.contains(player))));
    for &m in &members {
        assert!(
            damage_to(&events, UnitRef::Hero(m)).iter().all(|&d| d < 100.0),
            "the smash missed: {:?}",
            damage_to(&events, UnitRef::Hero(m))
        );
    }
}

#[test]
fn a_sneaking_kind_goes_in_stealth_on_its_way() {
    let mut w = world("pois: (1, 1)", "");
    join(&mut w, 0, 110.0, 110.0);
    let members = party(&mut w, "blades", 0.5, 10.0, 60.0, 5);
    assert_eq!(members.len(), 3);
    steps(&mut w, secs(3.0));
    assert!(members.iter().all(|&m| w.hero(m).unwrap().stealthed));
}
```

Add to the fixture `ENEMIES`, inside its list, the smasher (a monster with an on-self telegraph, for the dodge):

```rust
    (key: "smasher", name: "Smasher", display: 200, level: 30, hp: 3000.0, walk_speed: 2.5,
     run_speed: 6.0, swing: Some((min: 10.0, max: 10.0, every: 2.0)),
     moves: [(ability: "dig", every: 6.0)]),
```

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`add_party` does not exist).

- [ ] **Step 2: Parties, the rumour, retries**

In `crates/nightfall-sim/src/rivals/mod.rs`:

- Add `mod drive;` below `mod content;`.
- `Way` gains `pub(crate) retry_at: f32,` (doc: `A search that found no way is not tried again for the same goal before this time.`).
- `Rivals` gains `pub(crate) rumour: Option<(V3, f32, f32)>,` (doc: `The loudest noise a hero of the party made lately: where, its radius and when. A hunting party goes toward it when its own ears have nothing, as the night's hunters go toward the loudest noise (spec §7.2).`).
- Add to `impl World`:

```rust
    /// Party kind `def` (an index into the rivals' parties) comes in at `at` as party `number`,
    /// at `aggression`, its dice from `seed`: each member a rival of its kind, numbered from
    /// [`RIVAL_BASE`] up, the first at `at` and the rest 1.5 yards around it, each named by the
    /// dice from its kind's names; the party visits between `pois` points. The members made.
    pub(crate) fn add_party(
        &mut self,
        def: usize,
        number: u8,
        aggression: f32,
        at: V2,
        seed: u64,
    ) -> Vec<PlayerId> {
        let content = self.content().clone();
        let d = &content.rivals.parties[def];
        let (lo, hi) = content.rules.rivals.pois;
        let mut dice = Dice::new(seed);
        let mut members = Vec::new();
        for (i, &kind) in d.kinds.iter().enumerate() {
            let Some(player) = RIVAL_BASE.checked_add(self.rivals.next) else {
                break;
            };
            self.rivals.next = self.rivals.next.saturating_add(1);
            let spot = if i == 0 {
                at
            } else {
                at + V2::from_angle(i as f32 * std::f32::consts::TAU / 3.0, 1.5)
            };
            let k = &content.rivals.kinds[kind];
            let name = match k.names.len() {
                0 => k.name.clone(),
                n => k.names[((dice.unit() * n as f32) as usize).min(n - 1)].clone(),
            };
            if self
                .add_rival_hero(player, kind, Side::Rival(number), spot, &name)
                .is_ok()
            {
                members.push(player);
            }
        }
        let span = f32::from(hi - lo + 1);
        let pois_left = lo + ((dice.unit() * span) as u8).min(hi - lo);
        self.rivals.parties.push(Party {
            number,
            def,
            aggression,
            members: members.clone(),
            goal: Goal::Wait,
            visited: Vec::new(),
            pois_left,
            next_poi: None,
            looting: None,
            hurt_by: None,
            hunted: false,
            shut: Vec::new(),
            best: f32::INFINITY,
            stalled: 0.0,
            dice,
        });
        members
    }

    /// One line per rival party, for `NIGHTFALL_TRACE_RIVALS`: its goal, aggression and members
    /// (with health and where they stand). Empty on a client's mirror.
    pub fn rival_debug(&self) -> Vec<String> {
        let content = self.content();
        self.rivals
            .parties
            .iter()
            .map(|p| {
                let members: Vec<String> = p
                    .members
                    .iter()
                    .filter_map(|&m| self.hero(m))
                    .map(|h| {
                        format!(
                            "{} {} {:.0}/{:.0} ({:.1}, {:.1})",
                            h.player, h.name, h.hp, h.max_hp, h.pos.x, h.pos.y
                        )
                    })
                    .collect();
                format!(
                    "party {} {}: {:?}, aggression {:.2}, {} points left [{}]",
                    p.number,
                    content.rivals.parties[p.def].name,
                    p.goal,
                    p.aggression,
                    p.pois_left,
                    members.join("; ")
                )
            })
            .collect()
    }
```

In `crates/nightfall-sim/src/rivals/mind.rs`, `perceive_rivals` begins (before its `let Some(map)` line) with the rumour kept:

```rust
        // The loudest noise a hero of the party made lately (the night's hunt memory).
        let memory = self.content().rules.night.hunt.memory;
        let now = self.elapsed;
        if self.rivals.rumour.is_some_and(|(_, _, t)| now - t > memory) {
            self.rivals.rumour = None;
        }
        let loudest = self
            .senses
            .noises
            .iter()
            .filter(|n| self.side_of(n.by) == Side::Party && self.hero(n.by).is_some())
            .max_by(|a, b| a.radius.total_cmp(&b.radius))
            .map(|n| (n.at, n.radius));
        if let Some((at, radius)) = loudest {
            if self.rivals.rumour.is_none_or(|(_, r, _)| radius >= r) {
                self.rivals.rumour = Some((at, radius, now));
            }
        }
```

In `crates/nightfall-sim/src/world.rs`: the field `nav` becomes `pub(crate) nav`; in `step`, Task 7's `self.perceive_rivals();` (and its comment) becomes:

```rust
        // Last of a step, with the step's noises still about: rivals sense, their parties pick
        // their goals, and each rival's input for the next step is set as a client sets its own.
        self.step_rivals();
```

- [ ] **Step 3: The bots**

`crates/nightfall-sim/src/rivals/drive.rs`:

```rust
//! The rivals' bots (spec §7.4): each step every party picks its goal ([`decide`]) from what its
//! members know, and every rival's input for the next step is set as a player's client sets its
//! own: a pose walked along a path at its pace, a gait, Stealth presses, an ability press picked
//! by tag ([`choose`]), a dodge, and the acts that search and empty a container. The host takes
//! those inputs the way it takes a player's, so nothing a rival does skips a rule: its presses
//! are checked, its searches heard, its pace its load's.

use super::goals::{
    away_from, decide, formation_spot, next_poi, walk_along, FoeSeen, Goal, PartyView, WALK_RATIO,
};
use super::kit::{choose, ready, Moment};
use super::Way;
use crate::combat::stands_right;
use crate::senses::State;
use crate::{
    Act, ActPress, Aim, CastPress, Facing, Gait, HeroPose, Holder, Moving, PlayerId, PlayerInput,
    Tag, UnitRef, World, CHEST_HEIGHT, DT, EYE_HEIGHT, PATROL_BUDGET, V2, V3,
};

/// Seconds between two presses of one rival: a bot does not mash its bar.
const PRESS_EVERY: f32 = 0.25;
/// Seconds between two Stealth presses, so one is taken before the next.
const STEALTH_EVERY: f32 = 1.0;
/// Yards a goal may move before the path to it is searched again.
const REPATH: f32 = 3.0;
/// Seconds before a search that found no way is tried again for the same goal.
const RETRY: f32 = 2.0;
/// Yards from a place at which a rival is there.
const THERE: f32 = 1.0;
/// Yards a melee rival stands from its target, inside every melee reach.
const MELEE_STAND: f32 = 1.8;
/// Yards a fleeing rival runs off.
const FLEE_RUN: f32 = 30.0;
/// Yards a leader must get nearer its goal's place to count as headway.
const HEADWAY: f32 = 0.5;
/// Yards past a point's radius at which a monster guards it.
const GUARD: f32 = 6.0;

/// One rival's input this step, as its bot builds it.
#[derive(Clone, Copy, Debug, Default)]
struct Plan {
    /// Where it stands next.
    to: Option<V3>,
    /// Which way it faces.
    face: Option<V2>,
    gait: Gait,
    /// A bar slot, its target and its spot.
    press: Option<(u8, Option<UnitRef>, Option<V3>)>,
    act: Option<Act>,
    /// It wants to be in stealth.
    stealth: bool,
    dodge: bool,
}

impl World {
    /// The rivals' turn, last in a step: they sense the step (they hear its noises), every
    /// party picks its goal, and every rival's input for the next step is set.
    pub(crate) fn step_rivals(&mut self) {
        if self.map().is_none() || self.rivals.bodies.is_empty() {
            return;
        }
        self.perceive_rivals();
        self.rivals.paths_left = self.content().rules.rivals.paths_per_step;
        for pi in 0..self.rivals.parties.len() {
            self.plan_party(pi);
        }
        let members: Vec<(usize, PlayerId)> = self
            .rivals
            .parties
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| p.members.iter().map(move |&m| (pi, m)))
            .collect();
        for (pi, player) in members {
            self.drive(pi, player);
        }
    }

    /// The members of party `pi` who are up, the first leading.
    fn living(&self, pi: usize) -> Vec<PlayerId> {
        self.rivals.parties[pi]
            .members
            .iter()
            .copied()
            .filter(|&m| self.hero(m).is_some_and(|h| h.fight.up()))
            .collect()
    }

    /// Party `pi`'s goal this step: its clocks run, a looted point is done when nothing is left
    /// in it, the next point is picked, [`decide`] says what it is about, and a goal it makes no
    /// headway toward for `give_up` seconds is given up.
    fn plan_party(&mut self, pi: usize) {
        let content = self.content().clone();
        let living = self.living(pi);
        let Some(&leader) = living.first() else {
            return;
        };
        let lpos = self.hero(leader).map_or(V2::ZERO, |h| h.pos);
        if let Goal::Flee { left, .. } | Goal::Hunt { left, .. } = &mut self.rivals.parties[pi].goal {
            *left -= DT;
        }
        if let Some(poi) = self.rivals.parties[pi].looting {
            let all_full = living.iter().all(|&m| self.pack_full(m));
            if all_full || !self.poi_has_loot(pi, poi) {
                self.poi_done(pi, poi);
            }
        }
        {
            let p = &mut self.rivals.parties[pi];
            if p.looting.is_none() && p.next_poi.is_none() && p.pois_left > 0 {
                let roll = p.dice.unit();
                p.next_poi = next_poi(&content.rivals.pois, &p.visited, lpos, roll);
            }
        }
        let view = self.party_view(pi, &living, lpos);
        let was = self.rivals.parties[pi].goal;
        let goal = decide(was, &view, &content.rules.rivals);
        let p = &mut self.rivals.parties[pi];
        if let Goal::Loot { poi } = goal {
            let at = poi_at(&content, poi);
            if p.looting.is_none() && lpos.distance(at) <= content.rivals.pois[poi].radius {
                p.looting = Some(poi);
                p.next_poi = None;
            }
        }
        if matches!(goal, Goal::Hunt { .. }) {
            p.hunted = true;
        }
        let before = place_of(&content, was, p.looting);
        let after = place_of(&content, goal, p.looting);
        let same = std::mem::discriminant(&was) == std::mem::discriminant(&goal)
            && match (before, after) {
                (Some(a), Some(b)) => a.distance(b) <= REPATH,
                (None, None) => true,
                _ => false,
            };
        if !same {
            p.best = f32::INFINITY;
            p.stalled = 0.0;
        }
        p.goal = goal;
        self.headway(pi, lpos);
    }

    /// Whether party `pi`'s leader, at `lpos`, gets nearer its goal's place; after `give_up`
    /// seconds without, the goal is given up: a point counts as visited, an exit as shut to
    /// them, a hunt ends, a check is dropped.
    fn headway(&mut self, pi: usize, lpos: V2) {
        let content = self.content().clone();
        let give_up = content.rules.rivals.give_up;
        let p = &mut self.rivals.parties[pi];
        let place = match p.goal {
            Goal::Extract { exit } => {
                let e = &content.map.exits[exit];
                (lpos.distance(e.at) > e.radius).then_some(e.at)
            }
            g => place_of(&content, g, p.looting),
        };
        let Some(place) = place else {
            p.best = f32::INFINITY;
            p.stalled = 0.0;
            return;
        };
        let d = lpos.distance(place);
        if d < p.best - HEADWAY {
            p.best = d;
            p.stalled = 0.0;
        } else {
            p.stalled += DT;
        }
        if p.stalled < give_up {
            return;
        }
        p.best = f32::INFINITY;
        p.stalled = 0.0;
        let goal = p.goal;
        match goal {
            Goal::Loot { poi } => self.poi_done(pi, poi),
            Goal::Extract { exit } => self.rivals.parties[pi].shut.push(exit),
            Goal::Hunt { at, .. } => self.rivals.parties[pi].goal = Goal::Hunt { at, left: 0.0 },
            Goal::Check { .. } => {
                let members = self.rivals.parties[pi].members.clone();
                for b in self.rivals.bodies.iter_mut().filter(|b| members.contains(&b.player)) {
                    if matches!(b.mind.state, State::Checking { .. } | State::Wary { .. } | State::Searching { .. }) {
                        b.mind.state = State::Calm;
                        b.mind.there = false;
                        for s in &mut b.mind.of {
                            s.level = 0.0;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Party `pi` is done with point `poi`: visited, one fewer to go, nothing being looted.
    fn poi_done(&mut self, pi: usize, poi: usize) {
        let p = &mut self.rivals.parties[pi];
        if !p.visited.contains(&poi) {
            p.visited.push(poi);
        }
        p.pois_left = p.pois_left.saturating_sub(1);
        p.looting = None;
        p.next_poi = None;
        let members = p.members.clone();
        for b in self.rivals.bodies.iter_mut().filter(|b| members.contains(&b.player)) {
            b.loot = None;
        }
    }

    /// What party `pi` knows this step (see [`PartyView`]).
    fn party_view(&self, pi: usize, living: &[PlayerId], lpos: V2) -> PartyView {
        let content = self.content();
        let p = &self.rivals.parties[pi];
        let mut v = PartyView {
            aggression: p.aggression,
            health: 1.0,
            pois_left: p.pois_left,
            looting: p.looting,
            next_poi: p.next_poi,
            hurt_by_hero: p.hurt_by.is_some(),
            hunted: p.hunted,
            late: content.rivals.leave_by.is_some_and(|s| self.night.stage >= s),
            exit: self.nearest_exit(lpos, &p.shut),
            ..PartyView::default()
        };
        let mut lead: Option<(f32, V3)> = None;
        for &m in living {
            let Some(h) = self.hero(m) else {
                continue;
            };
            v.health = v.health.min(h.hp / h.max_hp.max(1.0));
            v.full |= self.pack_full(m);
            v.monsters |= self.enemy_after(m).is_some();
            let Some(b) = self.rivals.body(m) else {
                continue;
            };
            match b.mind.state {
                State::Alerted { last_seen, .. } => {
                    let d = lpos.distance(last_seen.ground());
                    if v.foe.is_none_or(|f| d < f.distance) {
                        v.foe = Some(FoeSeen {
                            at: last_seen,
                            distance: d,
                        });
                    }
                }
                State::Checking { place, .. } | State::Searching { place, .. } => {
                    v.wary = Some((place, true));
                }
                State::Wary { place, .. } if v.wary.is_none() => v.wary = Some((place, false)),
                _ => {}
            }
            for s in &b.mind.of {
                if s.level > 0.0 && lead.is_none_or(|(l, _)| s.level > l) {
                    lead = Some((s.level, s.place));
                }
            }
        }
        if let Some(poi) = p.looting {
            v.monsters |= self.guard_of(poi).is_some();
        }
        v.lead = lead
            .map(|(_, at)| at)
            .or(self.rivals.rumour.map(|(at, _, _)| at));
        v
    }

    /// Whether `player`'s pack has no empty slot, or carries past `full_load` of `weight_full`.
    fn pack_full(&self, player: PlayerId) -> bool {
        let content = self.content();
        let load = self.hero(player).map_or(0.0, |h| h.carry.load);
        let full_at = content.rules.rivals.full_load * content.rules.run.weight_full;
        self.pack(player).is_none_or(|i| i.first_empty(false).is_none()) || load >= full_at
    }

    /// The nearest enemy alive alerted to `player` (after it), by index.
    fn enemy_after(&self, player: PlayerId) -> Option<usize> {
        let at = self.hero(player)?.pos;
        (0..self.enemies.len())
            .filter(|&ei| self.enemies[ei].hp > 0.0)
            .filter(|&ei| self.senses.target_of(ei).is_some_and(|(p, _)| p == player))
            .min_by(|&a, &b| {
                at.distance(self.enemies[a].pos)
                    .total_cmp(&at.distance(self.enemies[b].pos))
            })
    }

    /// An enemy alive guarding point `poi` (within its radius and [`GUARD`] yards more), by index.
    fn guard_of(&self, poi: usize) -> Option<usize> {
        let p = &self.content().rivals.pois[poi];
        let at = V2::new(p.at.0, p.at.1);
        (0..self.enemies.len()).find(|&ei| {
            self.enemies[ei].hp > 0.0 && self.enemies[ei].pos.distance(at) <= p.radius + GUARD
        })
    }

    /// The nearest exit open now to `at`, of those not in `shut`.
    fn nearest_exit(&self, at: V2, shut: &[usize]) -> Option<usize> {
        let now = self.elapsed;
        self.content()
            .map
            .exits
            .iter()
            .enumerate()
            .filter(|(i, _)| !shut.contains(i) && self.raid.roll.exit_open(*i, now))
            .min_by(|a, b| at.distance(a.1.at).total_cmp(&at.distance(b.1.at)))
            .map(|(i, _)| i)
    }

    /// Whether container `id` is one `player` would loot: unlocked, not someone's remains,
    /// unsearched or holding something, and not one it is done with.
    fn lootable(&self, id: u32, player: PlayerId) -> bool {
        let Some(c) = self.raid.containers.iter().find(|c| c.id == id) else {
            return false;
        };
        let done = self.rivals.body(player).is_some_and(|b| b.done.contains(&id));
        c.lock.is_none()
            && !matches!(c.holder, Holder::Remains(_))
            && (!c.searched || c.stacks.iter().any(Option::is_some))
            && !done
    }

    /// Whether point `poi` still holds something a member of party `pi` would loot.
    fn poi_has_loot(&self, pi: usize, poi: usize) -> bool {
        let p = &self.content().rivals.pois[poi];
        let at = V2::new(p.at.0, p.at.1);
        let living = self.living(pi);
        self.raid
            .containers
            .iter()
            .filter(|c| c.at.distance(at) <= p.radius)
            .any(|c| living.iter().any(|&m| self.lootable(c.id, m)))
    }

    /// Rival `player`'s turn in party `pi`: a dodge when a telegraph is about to land on it,
    /// else what its party's goal asks; then its input is set.
    fn drive(&mut self, pi: usize, player: PlayerId) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if !self.heroes[hi].fight.up() {
            return;
        }
        let ids: Vec<u32> = self.telegraphs.iter().map(|t| t.id).collect();
        if let Some(b) = self.rivals.body_mut(player) {
            b.dodged.retain(|id| ids.contains(id));
        }
        let goal = self.rivals.parties[pi].goal;
        let held = self.combat.hero(player).is_some_and(|c| c.held.is_some());
        let busy = self.heroes[hi].fight.casting.is_some();
        let mut plan = Plan::default();
        if !held && !busy {
            if let Some(spot) = self.dodge_spot(pi, player, hi) {
                plan.to = Some(spot);
                plan.gait = Gait::Run;
                plan.dodge = true;
            }
        }
        if !plan.dodge && !held {
            match goal {
                Goal::Fight => self.fight_plan(pi, player, hi, &mut plan),
                Goal::Loot { poi } => self.loot_plan(pi, player, hi, poi, &mut plan),
                Goal::Check { at } => self.go_with_party(pi, player, hi, at.ground(), false, &mut plan),
                Goal::Hunt { at, .. } => self.go_with_party(pi, player, hi, at.ground(), true, &mut plan),
                Goal::Hold { face } => plan.face = Some(face - self.heroes[hi].pos),
                Goal::Flee { from, .. } => self.flee_plan(player, hi, from, &mut plan),
                Goal::Extract { exit } => self.exit_plan(pi, player, hi, exit, &mut plan),
                Goal::Wait => {}
            }
        }
        if busy || held {
            plan.to = None;
            plan.gait = Gait::Still;
        }
        let kind = self.rivals.body(player).map_or(0, |b| b.kind);
        plan.stealth = self.content().rivals.kinds[kind].sneaks
            && !matches!(goal, Goal::Fight | Goal::Flee);
        self.send(player, hi, plan);
    }

    /// The leader of party `pi` and `player`'s rank among the living (0 leads).
    fn rank_of(&self, pi: usize, player: PlayerId) -> (PlayerId, usize) {
        let living = self.living(pi);
        let rank = living.iter().position(|&m| m == player).unwrap_or(0);
        (living.first().copied().unwrap_or(player), rank)
    }

    /// `player` goes toward `place` with its party: the leader there, the rest behind it in
    /// formation, catching up at a run when far behind.
    fn go_with_party(&mut self, pi: usize, player: PlayerId, hi: usize, place: V2, run: bool, plan: &mut Plan) {
        let gap = self.content().rules.rivals.gap;
        let (leader, rank) = self.rank_of(pi, player);
        let goal = match self.hero(leader).filter(|_| leader != player) {
            Some(l) => formation_spot(l.pos, l.facing, rank, gap),
            None => place,
        };
        let far = self.heroes[hi].pos.distance(goal);
        if far <= THERE {
            return;
        }
        let run = run || (leader != player && far > 8.0);
        plan.to = self.step_toward(player, hi, goal, run);
        plan.gait = if run { Gait::Run } else { Gait::Walk };
    }

    /// The next spot on `player`'s way to `goal` at its pace (running, or in stealth, at its
    /// speed; walking at [`WALK_RATIO`] of it): straight when nothing stands between, else along
    /// a path, searched within the rivals' allowance this step. `None` when it is there, waits
    /// for a search, or no way is known.
    fn step_toward(&mut self, player: PlayerId, hi: usize, goal: V2, run: bool) -> Option<V3> {
        let map = self.map()?.clone();
        let now = self.elapsed;
        let h = &self.heroes[hi];
        let from = V3::at(h.pos, h.z);
        let pace = if run || h.stealthed { h.speed } else { h.speed * WALK_RATIO };
        let step = pace * DT;
        if from.ground().distance(goal) <= THERE * 0.5 {
            return None;
        }
        let to = V3::at(goal, map.floor_near(goal, h.z).unwrap_or(h.z));
        if map.walk_line(from, to) {
            if let Some(b) = self.rivals.body_mut(player) {
                b.way = Way::default();
            }
            let d = goal - from.ground();
            let n = from.ground() + d * (step.min(d.length()) / d.length().max(1e-6));
            return Some(V3::at(n, map.floor_within(n, from.z, 1.0).unwrap_or(from.z)));
        }
        let (stale, waiting) = match self.rivals.body(player) {
            Some(b) => {
                let moved = b.way.toward.is_none_or(|t| t.distance(goal) > REPATH);
                (b.way.path.is_none() || moved, b.way.failed && !moved && now < b.way.retry_at)
            }
            None => return None,
        };
        if waiting {
            return None;
        }
        if stale {
            if self.rivals.paths_left == 0 {
                return None;
            }
            self.rivals.paths_left -= 1;
            let path = self.nav.as_mut()?.find_path(from, to, false, PATROL_BUDGET);
            let b = self.rivals.body_mut(player)?;
            b.way = Way {
                failed: path.is_none(),
                retry_at: now + RETRY,
                path,
                next: 0,
                toward: Some(goal),
            };
        }
        let b = self.rivals.body_mut(player)?;
        let path = b.way.path.clone()?;
        Some(walk_along(from, &path, &mut b.way.next, step))
    }

    /// Looting point `poi`: until the party is there, with it; there, each member to a container
    /// of its own, searched and emptied through the acts a player's client sends (open, take
    /// all, close).
    fn loot_plan(&mut self, pi: usize, player: PlayerId, hi: usize, poi: usize, plan: &mut Plan) {
        let content = self.content().clone();
        let centre = poi_at(&content, poi);
        if self.rivals.parties[pi].looting != Some(poi) {
            return self.go_with_party(pi, player, hi, centre, false, plan);
        }
        let mine = self
            .rivals
            .body(player)
            .and_then(|b| b.loot)
            .filter(|&id| self.lootable(id, player));
        let target = mine.or_else(|| self.pick_container(pi, player, hi, poi));
        if let Some(b) = self.rivals.body_mut(player) {
            b.loot = target;
        }
        let Some(id) = target else {
            return self.go_with_party(pi, player, hi, centre, false, plan);
        };
        let Some(c) = self.raid.containers.iter().find(|c| c.id == id) else {
            return;
        };
        let (at, searched, holds) = (c.at, c.searched, c.stacks.iter().any(Option::is_some));
        let here = self.heroes[hi].pos;
        plan.face = Some(at - here);
        if here.distance(at) > content.rules.rivals.loot_reach {
            plan.to = self.step_toward(player, hi, at, false);
            plan.gait = Gait::Walk;
            return;
        }
        let acted = self.heroes[hi].carry.acted;
        let open = self.raid.open_of(player) == Some(id);
        let searching = self
            .raid
            .searches
            .iter()
            .any(|s| s.player == player && s.id == id);
        let took = self
            .rivals
            .body(player)
            .and_then(|b| b.act)
            .is_some_and(|a| a.act == Act::TakeAll && a.seq == acted);
        // An act already sent repeats until the host takes it (`None` keeps the last one).
        plan.act = if searching {
            None
        } else if !open {
            Some(Act::Open(id))
        } else if !searched {
            None
        } else if holds && !took && !self.pack_full(player) {
            Some(Act::TakeAll)
        } else {
            if let Some(b) = self.rivals.body_mut(player) {
                b.done.push(id);
                b.loot = None;
            }
            Some(Act::Close)
        };
    }

    /// The nearest container in point `poi` that `player` would loot and no other member of its
    /// party is looting.
    fn pick_container(&self, pi: usize, player: PlayerId, hi: usize, poi: usize) -> Option<u32> {
        let p = &self.content().rivals.pois[poi];
        let centre = V2::new(p.at.0, p.at.1);
        let here = self.heroes[hi].pos;
        let taken: Vec<u32> = self.rivals.parties[pi]
            .members
            .iter()
            .filter(|&&m| m != player)
            .filter_map(|&m| self.rivals.body(m).and_then(|b| b.loot))
            .collect();
        self.raid
            .containers
            .iter()
            .filter(|c| c.at.distance(centre) <= p.radius && !taken.contains(&c.id))
            .filter(|c| self.lootable(c.id, player))
            .min_by(|a, b| here.distance(a.at).total_cmp(&here.distance(b.at)))
            .map(|c| c.id)
    }

    /// The unit `player` fights: a hero its mind (or a friend's) is alerted to, else the nearest
    /// enemy after one of the party or guarding the point being looted. Its unit, ground, floor
    /// and facing.
    fn rival_target(&self, pi: usize, player: PlayerId) -> Option<(UnitRef, V2, f32, V2)> {
        let party = &self.rivals.parties[pi];
        let alerted = |m: PlayerId| match self.rivals.body(m)?.mind.state {
            State::Alerted { player: p, .. } => Some(p),
            _ => None,
        };
        let hero = alerted(player).or_else(|| party.members.iter().find_map(|&m| alerted(m)));
        if let Some(t) = hero.and_then(|p| self.hero(p)).filter(|t| t.fight.up()) {
            return Some((UnitRef::Hero(t.player), t.pos, t.z, t.facing));
        }
        let at = self.hero(player)?.pos;
        let guard = party.looting.and_then(|poi| self.guard_of(poi));
        let ei = party
            .members
            .iter()
            .filter_map(|&m| self.enemy_after(m))
            .chain(guard)
            .min_by(|&a, &b| {
                at.distance(self.enemies[a].pos)
                    .total_cmp(&at.distance(self.enemies[b].pos))
            })?;
        let e = &self.enemies[ei];
        Some((UnitRef::Enemy(e.id), e.pos, e.z, e.facing))
    }

    /// A fight: at its target, it closes to melee (or holds its stand-off with a clear line, for
    /// a shooter), faces it, and presses what [`choose`] picks from its kit by tag; a cast that
    /// must be stood for is pressed standing.
    fn fight_plan(&mut self, pi: usize, player: PlayerId, hi: usize, plan: &mut Plan) {
        let content = self.content().clone();
        let Some((target, tpos, tz, tfacing)) = self.rival_target(pi, player) else {
            let lead = self.hero(self.rank_of(pi, player).0).map_or(self.heroes[hi].pos, |h| h.pos);
            return self.go_with_party(pi, player, hi, lead, true, plan);
        };
        let Some(b) = self.rivals.body(player) else {
            return;
        };
        let (kit, kind) = (b.kit.clone(), b.kind);
        let me = &self.heroes[hi];
        let r = &content.rules.combat;
        let d = me.pos.distance(tpos);
        let in_sight = self.clear_line(
            V3::at(me.pos, me.z + EYE_HEIGHT),
            V3::at(tpos, tz + CHEST_HEIGHT),
        );
        plan.face = Some(tpos - me.pos);
        let combo = match target {
            UnitRef::Enemy(id) if me.fight.combo_on == Some(id) => me.fight.combo,
            UnitRef::Hero(p) if me.fight.combo_hero == Some(p) => me.fight.combo,
            _ => 0,
        };
        let (unaware, held) = match target {
            UnitRef::Enemy(id) => (
                self.opener_allowed(id, player),
                self.enemies
                    .iter()
                    .position(|e| e.id == id)
                    .is_some_and(|ei| self.combat.enemies[ei].held.is_some()),
            ),
            UnitRef::Hero(p) => (
                self.foe_unaware(p, player),
                self.combat.hero(p).is_some_and(|c| c.held.is_some()),
            ),
        };
        let m = Moment {
            distance: d,
            size: r.reach,
            health: me.hp / me.max_hp.max(1.0),
            stealthed: me.stealthed,
            unaware,
            held,
            behind: stands_right(Facing::Behind, me.pos, me.facing, tpos, tfacing, r.front),
            in_sight,
            combo,
        };
        let fight = me.fight.clone();
        let choice = choose(&kit, &m, &content.rules.rivals, |a| {
            ready(&fight, a, &content.abilities[a])
        });
        let stand_off = content.rivals.kinds[kind].stand_off;
        let want = if stand_off > 0.0 { stand_off } else { MELEE_STAND };
        if d > want || !in_sight {
            plan.to = self.step_toward(player, hi, tpos, true);
            plan.gait = Gait::Run;
        }
        if let Some(c) = choice {
            let a = &content.abilities[c.ability];
            if a.moving == Moving::Rooted && (a.cast > 0.0 || a.channel > 0.0) {
                plan.to = None;
                plan.gait = Gait::Still;
            }
            let aimed = match a.aim {
                Aim::Me => (None, None),
                Aim::Enemy => (Some(target), None),
                Aim::Ground => (None, Some(V3::at(tpos, tz))),
            };
            plan.press = Some((c.slot, aimed.0, aimed.1));
        }
    }

    /// Breaking off: an escape pressed when ready, and a run straight away from `from`.
    fn flee_plan(&mut self, player: PlayerId, hi: usize, from: V2, plan: &mut Plan) {
        let content = self.content().clone();
        if let Some(b) = self.rivals.body(player) {
            let fight = &self.heroes[hi].fight;
            if let Some(k) = b
                .kit
                .iter()
                .find(|k| k.tags.contains(&Tag::Escape) && ready(fight, k.ability, &content.abilities[k.ability]))
            {
                plan.press = Some((k.slot, None, None));
            }
        }
        let spot = content
            .map
            .bounds
            .clamp(away_from(self.heroes[hi].pos, from, FLEE_RUN));
        plan.to = self.step_toward(player, hi, spot, true);
        plan.gait = Gait::Run;
    }

    /// Out by exit `exit`: each member to a spot of its own inside the ring, and it stands.
    fn exit_plan(&mut self, pi: usize, player: PlayerId, hi: usize, exit: usize, plan: &mut Plan) {
        let content = self.content().clone();
        let e = &content.map.exits[exit];
        let (_, rank) = self.rank_of(pi, player);
        let spot = e.at + V2::from_angle(rank as f32 * std::f32::consts::TAU / 3.0, e.radius * 0.4);
        if self.heroes[hi].pos.distance(spot) <= THERE {
            return;
        }
        let hunted = self.senses.hunted(player);
        plan.to = self.step_toward(player, hi, spot, hunted);
        plan.gait = if hunted { Gait::Run } else { Gait::Walk };
    }

    /// Where `player` dodges to, if a telegraph about to land holds it (within `dodge_at`
    /// seconds) and the dice say it sees it coming: its dodge's distance straight out from the
    /// telegraph's centre, where a floor is and nothing stands between. Each telegraph is rolled
    /// for once.
    fn dodge_spot(&mut self, pi: usize, player: PlayerId, hi: usize) -> Option<V3> {
        let content = self.content().clone();
        let r = &content.rules.rivals;
        let h = &self.heroes[hi];
        if h.fight.dodge.have == 0 {
            return None;
        }
        let (pos, z, facing) = (h.pos, h.z, h.facing);
        let rolled = self.rivals.body(player).map(|b| b.dodged.clone()).unwrap_or_default();
        let threat = *self.telegraphs.iter().find(|t| {
            t.left <= r.dodge_at
                && t.shape.contains(t.at.ground(), t.facing, pos)
                && t.by.is_none_or(|by| self.hostile(by, player))
                && !rolled.contains(&t.id)
        })?;
        let roll = self.rivals.parties[pi].dice.unit();
        self.rivals.body_mut(player)?.dodged.push(threat.id);
        if roll >= r.dodge {
            return None;
        }
        let way = match (pos - threat.at.ground()).normalize_or_zero() {
            w if w == V2::ZERO => facing * -1.0,
            w => w,
        };
        let to = content.map.bounds.clamp(pos + way * content.rules.combat.dodge.distance);
        let map = self.map()?;
        let end = V3::at(to, map.floor_within(to, z, 1.0)?);
        map.walk_line(V3::at(pos, z), end).then_some(end)
    }

    /// `player`'s input from `plan`, set as a client sets its own: the pose (where it walks, the
    /// way it faces), the gait it moves at, a new press at most every [`PRESS_EVERY`] (the last
    /// one repeated, as a client repeats it), Stealth pressed when it wants in and may, a dodge
    /// counted, and an act repeated until the host takes it.
    fn send(&mut self, player: PlayerId, hi: usize, plan: Plan) {
        let now = self.elapsed;
        let (pos, z, facing, stealthed, acted) = {
            let h = &self.heroes[hi];
            (h.pos, h.z, h.facing, h.stealthed, h.carry.acted)
        };
        let may_stealth = plan.stealth && !stealthed && self.may_stealth(player);
        let to = plan.to.unwrap_or(V3::at(pos, z));
        let moved = to.ground() - pos;
        let moving = moved.length() > 1e-4;
        let face = plan
            .face
            .or(moving.then_some(moved))
            .map(V2::normalize_or_zero)
            .filter(|f| *f != V2::ZERO)
            .unwrap_or(facing);
        let gait = match (moving, plan.gait) {
            (false, _) => Gait::Still,
            (true, Gait::Still) => Gait::Walk,
            (true, g) => g,
        };
        let Some(b) = self.rivals.body_mut(player) else {
            return;
        };
        if let Some((slot, target, at)) = plan.press {
            if now - b.press_at >= PRESS_EVERY {
                b.cast_seq = b.cast_seq.wrapping_add(1);
                b.cast = Some(CastPress {
                    seq: b.cast_seq,
                    slot,
                    target,
                    at,
                });
                b.press_at = now;
            }
        }
        if may_stealth && now - b.stealth_at >= STEALTH_EVERY {
            b.stealth_presses = b.stealth_presses.wrapping_add(1);
            b.stealth_at = now;
        }
        if plan.dodge {
            b.dodge_presses = b.dodge_presses.wrapping_add(1);
        }
        if let Some(act) = plan.act {
            let pending = b.act.is_some_and(|a| a.act == act && a.seq != acted);
            if !pending {
                b.act_seq = b.act_seq % 255 + 1;
                b.act = Some(ActPress {
                    seq: b.act_seq,
                    act,
                });
            }
        }
        let input = PlayerInput {
            pose: Some(HeroPose {
                pos: to.ground(),
                facing: face,
                z: to.z,
            }),
            gait,
            stealth_presses: b.stealth_presses,
            cast: b.cast,
            dodge_presses: b.dodge_presses,
            act: b.act,
            ..PlayerInput::default()
        };
        self.set_input(player, input);
    }
}

/// Point `poi`'s centre.
fn poi_at(content: &crate::Content, poi: usize) -> V2 {
    let p = &content.rivals.pois[poi];
    V2::new(p.at.0, p.at.1)
}

/// The place a goal walks to, if it has one: a point not yet reached, a check's or a hunt's
/// spot, an exit's centre.
fn place_of(content: &crate::Content, goal: Goal, looting: Option<usize>) -> Option<V2> {
    match goal {
        Goal::Loot { poi } if looting != Some(poi) => Some(poi_at(content, poi)),
        Goal::Check { at } | Goal::Hunt { at, .. } => Some(at.ground()),
        Goal::Extract { exit } => Some(content.map.exits[exit].at),
        _ => None,
    }
}

```

(If `stands_right` is not re-exported at `crate::combat`, add it to Task 2's `pub(crate) use` line there. If `Act` is not `Copy + PartialEq`, compare with `matches!` and clone; the run loop's plan derives both.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 7 new tests and every earlier one: the rivals the earlier tests make are in no party, so no bot drives them).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: rival bots walk to points of interest, loot, fight, dodge and leave through their inputs

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Rivals out tonight: rolled with the raid, arriving unseen

**Files:**
- Create: `crates/nightfall-sim/src/rivals/arrivals.rs`
- Modify: `crates/nightfall-sim/src/rivals/mod.rs` (`mod arrivals;`, exports), `crates/nightfall-sim/src/raid/mod.rs` (`RaidRoll::rivals`, `begin_raid`'s checks), `crates/nightfall-sim/src/night/roll.rs` (`RaidRoll::roll`), `crates/nightfall-sim/src/world.rs` (`step`), `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/src/rivals/tests.rs`

**Interfaces:**
- Consumes: `RaidRoll { seed, entry, exits, weather, monsters, closes }`, `RaidRoll::{standard, roll}`, `World::{begin_raid, map, elapsed}`, `NightRules::arrive_unseen`, `Dice::{new, unit, between}` (earlier plans); `RivalContent::{parties, entries}`, `PartyDef::{odds, aggression}`, `RivalRules::{parties_out, arrive}` (Task 1); `add_party`, `Rivals::{arrived, try_in}`, `Side` (Tasks 4, 8).
- Produces:
  - `nightfall_sim::RivalOut { party: u8, entry: u8, at: f32, aggression: f32, seed: u64 }` (`Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize`); `RaidRoll::rivals: Vec<RivalOut>` (`#[serde(default)]`; empty in `RaidRoll::standard`, rolled in `RaidRoll::roll`).
  - Crate-only `rivals::arrivals::pick_rivals(content: &Content, seed: u64) -> Vec<RivalOut>`; `World::step_rival_arrivals(&mut self)`.
  - `begin_raid` refuses a roll naming a party or entry the content has not.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs` (merge `RivalOut` into its `crate::{…}` import):

```rust
#[test]
fn the_same_seed_brings_the_same_rivals_and_seeds_differ() {
    let c = content("", "");
    assert_eq!(RaidRoll::roll(&c, 42).rivals, RaidRoll::roll(&c, 42).rivals);
    let rolls: Vec<Vec<RivalOut>> = (0..30).map(|s| RaidRoll::roll(&c, s).rivals).collect();
    assert!(rolls.iter().any(|r| r != &rolls[0]));
}

#[test]
fn parties_out_bounds_the_parties_and_a_party_at_odds_zero_never_comes() {
    let c = content("parties_out: (1, 2), arrive: (30.0, 90.0)", "");
    let blades = c.party("blades").unwrap() as u8;
    for seed in 0..200 {
        let out = RaidRoll::roll(&c, seed).rivals;
        assert!((1..=2).contains(&out.len()), "seed {seed}: {out:?}");
        for o in &out {
            assert_ne!(o.party, blades);
            assert!((30.0..=90.0).contains(&o.at));
            let (lo, hi) = c.rivals.parties[usize::from(o.party)].aggression;
            assert!((lo..=hi).contains(&o.aggression));
            assert!(usize::from(o.entry) < c.rivals.entries.len());
        }
        let mut parties: Vec<u8> = out.iter().map(|o| o.party).collect();
        parties.dedup();
        assert_eq!(parties.len(), out.len(), "a party comes once");
    }
}

#[test]
fn the_standard_roll_brings_none_and_a_roll_naming_a_missing_party_is_refused() {
    let c = Arc::new(content("", ""));
    assert!(RaidRoll::standard(&c, 1).rivals.is_empty());
    let mut bad = RaidRoll::standard(&c, 1);
    bad.rivals.push(RivalOut {
        party: 9,
        ..RivalOut::default()
    });
    assert!(World::new(c.clone()).begin_raid(bad).is_err());
    let mut far = RaidRoll::standard(&c, 1);
    far.rivals.push(RivalOut {
        entry: 5,
        ..RivalOut::default()
    });
    assert!(World::new(c).begin_raid(far).is_err());
}

#[test]
fn rivals_arrive_late_and_unseen_with_numbers_of_their_own() {
    let plan = RivalOut {
        party: 0,
        entry: 0,
        at: 5.0,
        aggression: 0.15,
        seed: 3,
    };
    let mut w = world_on(open(), "", "", |c| {
        let mut r = RaidRoll::standard(c, 1);
        r.rivals.push(plan);
        r
    });
    // A hero two yards from the rivals' entry: nobody comes while they stand there.
    join(&mut w, 0, 12.0, 60.0);
    steps(&mut w, secs(10.0));
    assert!(w.heroes.iter().all(|h| h.side == Side::Party), "not in their sight");
    put(&mut w, 0, 80.0, 60.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(2.0));
    let rivals: Vec<&crate::Hero> = w.heroes.iter().filter(|h| h.side.is_rival()).collect();
    assert_eq!(rivals.len(), 2);
    assert!(rivals.iter().all(|h| h.player >= RIVAL_BASE && h.side == Side::Rival(0)));
    assert_ne!(rivals[0].player, rivals[1].player);
    assert!(rivals.iter().all(|h| !h.carry.worn.is_empty()), "dressed in their gear");
    let gem = w.content().item("gem").unwrap();
    assert!(rivals
        .iter()
        .all(|h| w.pack(h.player).is_some_and(|p| p.count_of(gem) >= 1)));
    assert_eq!(w.rivals.parties[0].aggression, 0.15);
    // It comes once.
    steps(&mut w, secs(3.0));
    assert_eq!(w.heroes.iter().filter(|h| h.side.is_rival()).count(), 2);
}

#[test]
fn a_roll_with_rivals_crosses_the_wire() {
    let c = content("", "");
    let r = RaidRoll::roll(&c, 11);
    assert!(!r.rivals.is_empty());
    let bytes = postcard::to_stdvec(&r).unwrap();
    let back: RaidRoll = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(back, r);
}
```

Run: `cargo test -p nightfall-sim rivals::`
Expected: FAIL to compile (`RivalOut`, `RaidRoll::rivals` do not exist).

- [ ] **Step 2: The roll's rivals and their arrival**

`crates/nightfall-sim/src/rivals/arrivals.rs`:

```rust
//! Rivals out tonight (spec §7.4, and §8.1's "every raid a little different"): the raid's roll
//! says which parties come, when, by which entry and how bold each is; a party comes once no
//! hero of the party who is up stands within `arrive_unseen` yards of its entry, so nobody sees
//! it appear, as the night's newcomers come (sub-project 6).

use serde::{Deserialize, Serialize};

use super::Side;
use crate::combat::Dice;
use crate::{Content, World, DT};

/// Mixed into the raid's seed, so the rivals' dice never follow the loot's or the night's.
const RIVAL_SEED: u64 = 0x5249_5641_4C00_0007;
/// Seconds between two tries at bringing a waiting party in.
const TRY_EVERY: f32 = 1.0;

/// One rival party out in a raid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RivalOut {
    /// Index into the rivals' parties.
    pub party: u8,
    /// Index into the rivals' entries.
    pub entry: u8,
    /// Seconds into the raid it comes, at the earliest.
    pub at: f32,
    /// Its aggression this raid (spec §7.4), within its kind's.
    pub aggression: f32,
    /// Its dice.
    pub seed: u64,
}

/// The parties out for a raid of `seed`: each comes up at its odds, between `parties_out` of
/// them (the likeliest kept when trimming, the next likeliest added when short); each has an
/// entry, a time within `arrive`, an aggression within its kind's, and dice of its own. None
/// when the content has no parties.
pub(crate) fn pick_rivals(content: &Content, seed: u64) -> Vec<RivalOut> {
    let rc = &content.rivals;
    let (lo, hi) = content.rules.rivals.parties_out;
    let (early, late) = content.rules.rivals.arrive;
    if rc.parties.is_empty() || rc.entries.is_empty() {
        return Vec::new();
    }
    let mut dice = Dice::new(seed ^ RIVAL_SEED);
    // A key under 1 comes up at its odds; the lowest keys are the likeliest.
    let mut keyed: Vec<(f32, usize)> = rc
        .parties
        .iter()
        .enumerate()
        .filter(|(_, p)| p.odds > 0.0)
        .map(|(i, p)| (dice.unit() / p.odds, i))
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let rolled = keyed.iter().filter(|(k, _)| *k < 1.0).count();
    let take = rolled.clamp(usize::from(lo).min(keyed.len()), usize::from(hi).max(usize::from(lo).min(keyed.len())));
    let mut out: Vec<usize> = keyed.iter().take(take).map(|&(_, i)| i).collect();
    out.sort_unstable();
    out.iter()
        .enumerate()
        .map(|(n, &i)| {
            let p = &rc.parties[i];
            let entries = rc.entries.len();
            RivalOut {
                party: i as u8,
                entry: ((dice.unit() * entries as f32) as usize).min(entries - 1) as u8,
                at: dice.between(early, late),
                aggression: dice.between(p.aggression.0, p.aggression.1),
                seed: (seed ^ RIVAL_SEED).wrapping_add((n as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
            }
        })
        .collect()
}

impl World {
    /// The roll's parties come in: each once its time has come and, tried every second, once no
    /// hero of the party who is up stands within `arrive_unseen` yards of its entry. Only once a
    /// map is attached (parties walk it).
    pub(crate) fn step_rival_arrivals(&mut self) {
        let plans = self.raid.roll.rivals.clone();
        if plans.is_empty() || self.map().is_none() {
            return;
        }
        if self.rivals.arrived.len() < plans.len() {
            self.rivals.arrived.resize(plans.len(), false);
        }
        self.rivals.try_in -= DT;
        if self.rivals.try_in > 0.0 {
            return;
        }
        self.rivals.try_in = TRY_EVERY;
        let content = self.content().clone();
        let unseen = content.rules.night.arrive_unseen;
        for (n, o) in plans.iter().enumerate() {
            if self.rivals.arrived[n] || self.elapsed < o.at {
                continue;
            }
            let at = content.rivals.entries[usize::from(o.entry)];
            let seen = self
                .heroes
                .iter()
                .any(|h| h.side == Side::Party && h.fight.up() && h.pos.distance(at) <= unseen);
            if seen {
                continue;
            }
            self.rivals.arrived[n] = true;
            self.add_party(usize::from(o.party), n as u8, o.aggression, at, o.seed);
        }
    }
}
```

In `crates/nightfall-sim/src/rivals/mod.rs`: add `mod arrivals;` below `mod content;` and `pub use arrivals::RivalOut;` beside the content's `pub use`, and `pub(crate) use arrivals::pick_rivals;`. In `crates/nightfall-sim/src/lib.rs`, `RivalOut` joins the `rivals` exports.

In `crates/nightfall-sim/src/raid/mod.rs`:

- `RaidRoll` gains, as its last field:

```rust
    /// The rival parties out (spec §7.4), in the order they were rolled.
    #[serde(default)]
    pub rivals: Vec<RivalOut>,
```

  (`RivalOut` joins the file's `crate::{…}` import; `RaidRoll::standard`'s literal, if it lists every field, gains `rivals: Vec::new(),`.)
- `begin_raid`'s checks gain, after the night plan's weather and monsters checks:

```rust
        for o in &roll.rivals {
            if usize::from(o.party) >= content.rivals.parties.len()
                || usize::from(o.entry) >= content.rivals.entries.len()
            {
                return Err(format!(
                    "no rival party {} or entry {} in rivals.ron",
                    o.party, o.entry
                ));
            }
        }
```

In `crates/nightfall-sim/src/night/roll.rs`, `RaidRoll::roll`'s last lines `roll.monsters = pick_monsters(content, &mut dice); roll` become:

```rust
        roll.monsters = pick_monsters(content, &mut dice);
        // The rivals' own dice, so the rest of the roll stays as it was.
        roll.rivals = crate::rivals::pick_rivals(content, seed);
        roll
```

and its doc gains: `The rival parties out, by the rivals' rules (spec §7.4).`

In `crates/nightfall-sim/src/world.rs`'s `step`, the line `self.step_rivals();` (Task 8) gains before it `self.step_rival_arrivals();`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (the 5 new tests and every earlier one; the night's roll tests compare whole rolls of one seed, which still match, and `RaidRoll::standard` has no rivals).

- [ ] **Step 4: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: every raid rolls its rival parties, who come in out of sight

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Whole raids with rivals

**Files:**
- Modify: `crates/nightfall-sim/src/rivals/tests.rs`

**Interfaces:**
- Consumes: every rivals task; `World::{begin_raid, attach_map, step, snapshot, drain_events, hero, heroes, raid, rivals}`, `RaidRoll::{standard, roll}`, `Act`, `ActPress`, `Event` (earlier plans).
- Produces: the review's whole-raid pins; no new code outside the tests (a fix a test turns up goes in the file that owns the rule, named in the commit).

- [ ] **Step 1: Write the tests**

Append to `crates/nightfall-sim/src/rivals/tests.rs`:

```rust
/// A world on open ground whose roll brings `plans`, the party's hero far off at (110, 110).
fn raid_with(rivals_rules: &str, spawns: &str, plans: &[RivalOut]) -> World {
    let mut w = world_on(open(), rivals_rules, spawns, |c| {
        let mut r = RaidRoll::standard(c, 1);
        r.rivals.extend_from_slice(plans);
        r
    });
    join(&mut w, 0, 110.0, 110.0);
    w
}

fn plan(party: u8, aggression: f32) -> RivalOut {
    RivalOut {
        party,
        entry: 0,
        at: 0.0,
        aggression,
        seed: 21,
    }
}

/// `player` runs back and forth between `a` and `b` for `n` steps.
fn pace(w: &mut World, player: PlayerId, a: V2, b: V2, n: usize) {
    for i in 0..n {
        let t = ((i as f32 / secs(3.0) as f32) % 2.0 - 1.0).abs();
        let at = a + (b - a) * t;
        put(w, player, at.x, at.y, (b - a).normalize_or_zero(), Gait::Run);
        w.step();
    }
}

#[test]
fn a_cautious_party_loots_its_points_and_extracts() {
    let mut w = raid_with("pois: (2, 2)", "", &[plan(0, 0.1)]);
    w.drain_events();
    steps(&mut w, secs(240.0));
    assert!(emptied(&w, V2::new(60.0, 60.0)) && emptied(&w, V2::new(90.0, 30.0)));
    let out = events_of(&mut w, |e| matches!(e, Event::RivalOut { .. }).then_some(()));
    assert_eq!(out.len(), 2, "both out");
    assert!(w.raid.containers.iter().all(|c| !matches!(c.holder, Holder::Hero(_))), "no bodies");
    assert!(w.take_outcomes().is_empty());
}

#[test]
fn a_bold_party_done_looting_hunts_a_noisy_hero_and_fights_it() {
    let mut w = raid_with("pois: (1, 1), hunt_for: 150.0", "", &[plan(1, 0.9)]);
    w.drain_events();
    // While they loot, the hero runs about 50 yards off, loud.
    pace(&mut w, 0, V2::new(100.0, 100.0), V2::new(100.0, 90.0), secs(150.0));
    let p = &w.rivals.parties[0];
    assert!(p.hunted, "they went hunting: {:?}", p.goal);
    let events = w.drain_events();
    let struck = events.iter().any(|e| {
        matches!(e, Event::Damage { to: UnitRef::Hero(0), by: Some(UnitRef::Hero(r)), .. } if *r >= RIVAL_BASE)
    });
    assert!(struck || !w.hero(0).is_some_and(|h| h.fight.up()), "they found and fought the hero");
}

#[test]
fn a_party_fights_the_monster_guarding_its_point_then_loots_it() {
    let mut w = raid_with(
        "pois: (1, 1)",
        &format!("{}\n{}\n", sentry_line(62.0, 60.0), sentry_line(92.0, 30.0)),
        &[plan(0, 0.1)],
    );
    w.drain_events();
    steps(&mut w, secs(120.0));
    let events = w.drain_events();
    assert!(events.iter().any(|e| matches!(e, Event::EnemyDied { by: Some(p), .. } if *p >= RIVAL_BASE)));
    let poi = &w.content().rivals.pois[w.rivals.parties[0].visited[0]];
    assert!(emptied(&w, V2::new(poi.at.0, poi.at.1)));
}

#[test]
fn a_slain_rival_drops_its_bag_for_the_hero_to_take() {
    let mut w = raid_with("", "", &[]);
    let a = rival(&mut w, 0, "digger", 0, 105.0, 110.0);
    w.wound(a, 5000.0, Some(UnitRef::Hero(0)));
    w.step();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(a))
        .map(|c| c.id)
        .expect("a body");
    put(&mut w, 0, 106.0, 110.0, V2::new(-1.0, 0.0), Gait::Still);
    w.step();
    let act = |w: &mut World, seq: u8, act: Act| {
        let h = w.hero(0).unwrap();
        let pose = HeroPose {
            pos: h.pos,
            facing: h.facing,
            z: h.z,
        };
        w.set_input(
            0,
            PlayerInput {
                pose: Some(pose),
                act: Some(ActPress { seq, act }),
                ..Default::default()
            },
        );
        w.step();
    };
    act(&mut w, 1, Act::Open(body));
    let search = w.content().rules.run.search_body;
    steps(&mut w, secs(search + 0.5));
    act(&mut w, 2, Act::TakeAll);
    let sack = w.content().item("sack").unwrap();
    assert!(w.pack(0).unwrap().count_of(sack) >= 1, "its bag is the hero's now");
}

#[test]
fn every_party_leaves_or_dies_within_the_night() {
    for seed in [1, 2] {
        let c = content(r#"arrive: (0.0, 60.0), leave_by: Some("howling"), pois: (1, 2)"#, "");
        let roll = RaidRoll::roll(&c, seed);
        assert!(!roll.rivals.is_empty());
        let mut w = World::new(Arc::new(c));
        w.begin_raid(roll).unwrap();
        w.attach_map(Arc::new(open()));
        join(&mut w, 0, 110.0, 110.0);
        steps(&mut w, secs(600.0));
        let left: Vec<String> = w
            .heroes
            .iter()
            .filter(|h| h.side.is_rival() && h.fight.up())
            .map(|h| format!("{} at ({:.0}, {:.0})", h.name, h.pos.x, h.pos.y))
            .collect();
        assert!(left.is_empty(), "seed {seed}: {left:?}\n{:#?}", w.rival_debug());
    }
}

#[test]
fn a_world_with_rivals_plays_the_same_twice() {
    let run = || {
        let mut w = raid_with("pois: (1, 2)", &format!("{}\n", sentry_line(62.0, 60.0)), &[plan(0, 0.2), plan(1, 0.9)]);
        pace(&mut w, 0, V2::new(100.0, 100.0), V2::new(100.0, 80.0), secs(80.0));
        w.snapshot()
    };
    assert_eq!(run(), run());
}
```

The test `plan(1, 0.9)` brings the tests' `watch` party (shooter and digger, index 1); its entry is the only one, (10, 60). In `a_world_with_rivals_plays_the_same_twice` both parties come at second 0 from one entry, numbered 64 to 67.

Run: `cargo test -p nightfall-sim rivals:: -- --nocapture`
Expected: PASS (6 tests; `every_party_leaves_or_dies_within_the_night` runs 20 simulated minutes and takes some seconds). A failure here is a rule to fix where it lives (most likely `drive.rs` or `goals.rs`), not a test to loosen: a party standing somewhere at the end shows its goal in the `rival_debug` lines the assertion prints.

- [ ] **Step 2: Run every test**

Run: `cargo test -p nightfall-sim -p together-net -p together-sim`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: whole raids with rivals: cautious ones leave, bold ones hunt, all are gone by night's end

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The client: rivals as hostile players, concealed when unseen, their marks, the map table

**Files:**
- Create: `crates/benilla-app/src/nightfall/rivals_view.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (module, resource, systems), `crates/benilla-app/src/nightfall/view.rs` (`place`: walking shows walking), `crates/benilla-app/src/nightfall/fight.rs` (`Event::Healed`), `crates/benilla-app/src/nightfall/fx.rs` (`COLOR_HEAL`), `crates/benilla-app/src/nightfall/night.rs` (`intel_lines`, `plan_from_env`)

**Interfaces:**
- Consumes: `run::Sim { world, me }`, `view::Views::heroes`, `units::{FACTION_FRIENDLY, FACTION_MONSTER}`, `conceal::bytes_1`, `marks::{mark_face, mark_height, MarkText}`, `fx::Fx::number`, `night::{intel_lines, plan_from_env}`, `NightfallEntity`, `NightfallSet`, `Phase`, `content()` (earlier plans); engine: `crate::target::Concealed`, `crate::net::ObjectStore`, `ObjectFields::{unit_faction_template, unit_is_stealthed, unit_is_untrackable, merge, from_pairs}`, `benilla_protocol::field::FIELD_UNIT_BYTES_1`, `benilla_world::view::WorldCamera`, `crate::char_select::wow_font`; sim: `World::{rival_view, rival_debug, heroes}`, `Hero::side`, `Side`, `RaidRoll::rivals`, `RivalOut`, `Content::{party, rivals}`, `NightRules::arrive_unseen`, `Event::Healed` (Tasks 1–9).
- Produces:
  - `rivals_view::{faction_of(Side) -> u32, intel_lines(&RaidRoll, &Content) -> Vec<String>, force(&mut RaidRoll, &Content, keys: &str, at: f32) -> Vec<String>, force_from_env(&mut RaidRoll, &Content), RivalMarkViews}`; systems `rivals_view::{hostile, conceal, marks, trace}`.
  - `fx::COLOR_HEAL`; WoW's walk for any other hero (or rival) moving slower than `WALK_UNDER` (4 yards a second).
  - Test hooks: `NIGHTFALL_RIVALS=<party>,…` (the planned raid's rivals are these, entering out of the party's sight at `NIGHTFALL_RIVALS_AT` seconds, 5 by default, at the middle of their aggression), `NIGHTFALL_TRACE_RIVALS=1` (every 2 s: each party's goal and members on the host, and this player's view of the rivals).

- [ ] **Step 1: Write the pure parts and their tests**

`crates/benilla-app/src/nightfall/rivals_view.rs`:

```rust
//! Rivals on screen (spec §7.4). They are heroes of another side in the snapshot, so the hero
//! code draws them already, as WoW players in their own gear; this module makes them hostile
//! ones (the monster faction template, so WoW's plates, rings and target frame show them red,
//! as a hostile player is shown), conceals those our hero does not see (no Tab, plate or minimap
//! dot, as for enemies; their body is still drawn, faded when in stealth), floats their "?" and
//! "!" over them, names the parties out on the map table, and holds the rivals' test hooks.

use std::collections::{HashMap, HashSet};

use benilla_protocol::field::FIELD_UNIT_BYTES_1;
use benilla_protocol::messages::ObjectFields;
use benilla_world::view::WorldCamera;
use bevy::prelude::*;
use nightfall_sim::{Content, PlayerId, RaidRoll, RivalOut, Side, V2};

use super::conceal::bytes_1;
use super::marks::{mark_face, mark_height, MarkText};
use super::run::Sim;
use super::units::{FACTION_FRIENDLY, FACTION_MONSTER};
use super::view::Views;
use super::NightfallEntity;
use crate::net::ObjectStore;
use crate::target::Concealed;

/// `UNIT_FIELD_FACTIONTEMPLATE`.
const FIELD_FACTION: u16 = 35;
/// A mark's glyph size and placement, UI pixels, as the enemies' marks have them.
const SIZE: f32 = 30.0;
const HALF_W: f32 = 8.0;
const HIGH: f32 = 34.0;
/// Seconds into the raid a party a test hook names comes, when `NIGHTFALL_RIVALS_AT` is unset.
const HOOK_AT: f32 = 5.0;

/// The faction template a hero of `side` wears: the players' friendly one for the party, the
/// monsters' for a rival, which WoW's reaction rules make hostile to every player.
pub(super) fn faction_of(side: Side) -> u32 {
    match side {
        Side::Party => FACTION_FRIENDLY,
        Side::Rival(_) => FACTION_MONSTER,
    }
}

/// The map table's line about rivals (spec §8.5's intel): the parties the planned raid brings.
pub(super) fn intel_lines(roll: &RaidRoll, content: &Content) -> Vec<String> {
    let names: Vec<&str> = roll
        .rivals
        .iter()
        .filter_map(|o| content.rivals.parties.get(usize::from(o.party)))
        .map(|p| p.name.as_str())
        .collect();
    vec![if names.is_empty() {
        "Rivals: none heard of".into()
    } else {
        format!("Rivals: {}", names.join(", "))
    }]
}

/// The planned raid's rivals become the parties named in `keys` (comma-separated), each coming
/// `at` seconds in at the middle of its aggression, by the rivals' entry nearest the map's first
/// entry that is out of the party's sight there (`arrive_unseen`), else the farthest. The
/// warnings for keys the content has not.
pub(super) fn force(roll: &mut RaidRoll, content: &Content, keys: &str, at: f32) -> Vec<String> {
    let mut warnings = Vec::new();
    let entries = &content.rivals.entries;
    if entries.is_empty() {
        return vec!["NIGHTFALL_RIVALS: rivals.ron has no entries".into()];
    }
    let start = V2::new(content.map.entries[0][0], content.map.entries[0][1]);
    let unseen = content.rules.night.arrive_unseen;
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| start.distance(entries[a]).total_cmp(&start.distance(entries[b])));
    let entry = order
        .iter()
        .copied()
        .find(|&i| start.distance(entries[i]) >= unseen)
        .unwrap_or(*order.last().unwrap_or(&0));
    roll.rivals.clear();
    for (n, key) in keys.split(',').map(str::trim).filter(|k| !k.is_empty()).enumerate() {
        let Some(party) = content.party(key) else {
            warnings.push(format!("NIGHTFALL_RIVALS: no party \"{key}\" in rivals.ron"));
            continue;
        };
        let (lo, hi) = content.rivals.parties[party].aggression;
        roll.rivals.push(RivalOut {
            party: party as u8,
            entry: entry as u8,
            at,
            aggression: (lo + hi) * 0.5,
            seed: roll.seed ^ (n as u64 + 1),
        });
    }
    warnings
}

/// `NIGHTFALL_RIVALS` and `NIGHTFALL_RIVALS_AT` on the planned raid, warning of what they name
/// wrong.
pub(super) fn force_from_env(roll: &mut RaidRoll, content: &Content) {
    let Ok(keys) = std::env::var("NIGHTFALL_RIVALS") else {
        return;
    };
    let at = std::env::var("NIGHTFALL_RIVALS_AT")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|t: &f32| t.is_finite() && *t >= 0.0)
        .unwrap_or(HOOK_AT);
    for w in force(roll, content, &keys, at) {
        warn!("{w}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Content {
        Content::from_ron(nightfall_sim::ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
            rivals: include_str!("../../../../content/nightfall/rivals.ron"),
        })
        .unwrap()
    }

    #[test]
    fn rivals_wear_the_monsters_faction_and_the_party_the_players() {
        assert_eq!(faction_of(Side::Party), FACTION_FRIENDLY);
        assert_eq!(faction_of(Side::Rival(2)), FACTION_MONSTER);
    }

    #[test]
    fn the_map_table_names_the_parties_out() {
        let c = shipped();
        let mut roll = RaidRoll::standard(&c, 1);
        assert_eq!(intel_lines(&roll, &c), vec!["Rivals: none heard of".to_string()]);
        let warnings = force(&mut roll, &c, "grave_robbers, watch_deserters", 5.0);
        assert!(warnings.is_empty());
        assert_eq!(
            intel_lines(&roll, &c),
            vec!["Rivals: Grave robbers, Night Watch deserters".to_string()]
        );
    }

    #[test]
    fn the_hook_brings_the_named_parties_out_of_the_partys_sight() {
        let c = shipped();
        let mut roll = RaidRoll::standard(&c, 7);
        let warnings = force(&mut roll, &c, "cutthroats,nobody", 12.0);
        assert_eq!(warnings.len(), 1);
        assert_eq!(roll.rivals.len(), 1);
        let o = roll.rivals[0];
        assert_eq!(usize::from(o.party), c.party("cutthroats").unwrap());
        assert_eq!(o.at, 12.0);
        let start = V2::new(c.map.entries[0][0], c.map.entries[0][1]);
        let e = c.rivals.entries[usize::from(o.entry)];
        let far = c
            .rivals
            .entries
            .iter()
            .all(|x| start.distance(*x) < c.rules.night.arrive_unseen);
        assert!(start.distance(e) >= c.rules.night.arrive_unseen || far);
        let (lo, hi) = c.rivals.parties[usize::from(o.party)].aggression;
        assert!((o.aggression - (lo + hi) * 0.5).abs() < 1e-5);
    }
}
```

Add `mod rivals_view;` to `nightfall/mod.rs`.

Run: `cargo test -p benilla-app --lib nightfall::rivals_view -j 2`
Expected: PASS (3 tests).

- [ ] **Step 2: The systems**

Append to `rivals_view.rs`, above its tests:

```rust
/// Every hero's body wears its side's faction: a rival's turns hostile the frame it is drawn.
pub(super) fn hostile(sim: Res<Sim>, views: Res<Views>, mut stores: Query<&mut ObjectStore>) {
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let Ok(mut store) = stores.get_mut(e) else {
            continue;
        };
        let want = faction_of(hero.side);
        if store.0.unit_faction_template() != Some(want) {
            store
                .0
                .merge(ObjectFields::from_pairs(&[(FIELD_FACTION, want)]));
        }
    }
}

/// Every rival our hero does not see (this player's [`nightfall_sim::RivalView`]) is concealed
/// and untrackable; one they see is neither. Its body is still drawn: walls hide it on screen.
pub(super) fn conceal(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    mut bodies: Query<(&mut ObjectStore, Has<Concealed>)>,
) {
    let view = sim.world.rival_view(sim.me);
    for hero in sim.world.heroes.iter().filter(|h| h.side.is_rival()) {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let Ok((mut store, concealed)) = bodies.get_mut(e) else {
            continue;
        };
        let hide = !view.seen.contains(&hero.player);
        if hide != concealed {
            if hide {
                commands.entity(e).insert(Concealed);
            } else {
                commands.entity(e).remove::<Concealed>();
            }
        }
        let stealthed = store.0.unit_is_stealthed();
        let now = bytes_1(&store.0, None, stealthed, store.0.unit_is_untrackable());
        let want = bytes_1(&store.0, None, stealthed, hide);
        if now != want {
            store
                .0
                .merge(ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, want)]));
        }
    }
}

/// Which text node shows which rival's mark.
#[derive(Resource, Default)]
pub(super) struct RivalMarkViews {
    nodes: HashMap<PlayerId, Entity>,
}

/// Every rival's mark for us over its head, where the camera sees it, as the enemies' marks
/// are drawn; hidden while the rival is concealed or no longer marks us.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn marks(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    assets: Res<AssetServer>,
    ui_scale: Res<UiScale>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    bodies: Query<(&Transform, Has<Concealed>)>,
    mut nodes: ResMut<RivalMarkViews>,
    mut texts: Query<(&mut Node, &mut Text, &mut TextColor, &mut Visibility), With<MarkText>>,
) {
    let s = ui_scale.0;
    let cam = cams.single().ok();
    nodes.nodes.retain(|_, e| texts.contains(*e));
    let mut shown = HashSet::new();
    for m in sim.world.rival_view(sim.me).marks {
        let Some(&body) = views.heroes.get(&m.rival) else {
            continue;
        };
        let (Ok((tf, concealed)), Some((camera, cam_tf))) = (bodies.get(body), cam) else {
            continue;
        };
        if concealed {
            continue;
        }
        let head = tf.translation + Vec3::Y * mark_height(1.0);
        let Ok(at) = camera.world_to_viewport(&GlobalTransform::from(*cam_tf), head) else {
            continue;
        };
        let node = *nodes.nodes.entry(m.rival).or_insert_with(|| {
            commands
                .spawn((
                    MarkText,
                    NightfallEntity,
                    Text::new(""),
                    TextFont {
                        font: crate::char_select::wow_font(&assets),
                        font_size: SIZE,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    GlobalZIndex(340),
                    Visibility::Hidden,
                ))
                .id()
        });
        // A node spawned this frame is placed from the next.
        let Ok((mut n, mut text, mut color, mut vis)) = texts.get_mut(node) else {
            continue;
        };
        shown.insert(m.rival);
        let (glyph, tint) = mark_face(m.alert);
        n.left = Val::Px(at.x / s - HALF_W);
        n.top = Val::Px(at.y / s - HIGH);
        if text.0 != glyph {
            text.0 = glyph.into();
        }
        color.0 = tint;
        *vis = Visibility::Inherited;
    }
    for (rival, &node) in &nodes.nodes {
        if !shown.contains(rival) {
            if let Ok((_, _, _, mut vis)) = texts.get_mut(node) {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// `NIGHTFALL_TRACE_RIVALS=1`: every 2 seconds, each party's goal and members (on the host) and
/// what this player is told of the rivals.
pub(super) fn trace(sim: Res<Sim>, real: Res<Time<Real>>, mut last: Local<f32>) {
    if std::env::var_os("NIGHTFALL_TRACE_RIVALS").is_none() || real.elapsed_secs() - *last < 2.0
    {
        return;
    }
    *last = real.elapsed_secs();
    for line in sim.world.rival_debug() {
        info!("nightfall: rivals: {line}");
    }
    let v = sim.world.rival_view(sim.me);
    info!("nightfall: rivals seen {:?}, marks {:?}", v.seen, v.marks);
}
```

(If `ObjectFields::unit_faction_template` returns another integer type, compare through `u32::from`; the ring's reaction code is its one reader.)

- [ ] **Step 3: Wire it in**

In `crates/benilla-app/src/nightfall/mod.rs`'s `NightfallPlugin::build`, add `.init_resource::<rivals_view::RivalMarkViews>()` beside the other resources, and:

```rust
            .add_systems(
                Update,
                (
                    rivals_view::hostile,
                    rivals_view::conceal,
                    rivals_view::marks,
                    rivals_view::trace,
                )
                    .chain()
                    .after(view::sync)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

In `crates/benilla-app/src/nightfall/view.rs`'s `place`, the line `let flags = if p.speed > 0.1 { move_flags::FORWARD } else { 0 };` becomes:

```rust
        // Under WoW's walk-to-run line a hero (or a rival) is drawn walking.
        let flags = match p.speed {
            s if s <= 0.1 => 0,
            s if s < WALK_UNDER => move_flags::FORWARD | move_flags::WALK_MODE,
            _ => move_flags::FORWARD,
        };
```

with, above `sync`, `/// Yards a second under which another hero is drawn walking, not running. const WALK_UNDER: f32 = 4.0;`.

In `crates/benilla-app/src/nightfall/fx.rs`, beside the other colours: `/// A heal's number: green. pub(super) const COLOR_HEAL: u32 = 0xFF40_FF40;`.

In `crates/benilla-app/src/nightfall/fight.rs`'s `show_events`, add before its `_ => {}` arm:

```rust
            Event::Healed { player, amount } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    fx.number(e, *amount, false, COLOR_HEAL);
                }
            }
```

(`COLOR_HEAL` joins its `fx::{…}` import.)

In `crates/benilla-app/src/nightfall/night.rs`:

- `intel_lines`' last line `lines` becomes `lines.extend(super::rivals_view::intel_lines(roll, content)); lines`.
- `plan_from_env` becomes:

```rust
pub(super) fn plan_from_env(content: &Content, seed: u64) -> RaidRoll {
    let mut roll = plan(
        content,
        seed,
        std::env::var("NIGHTFALL_WEATHER").ok().as_deref(),
        std::env::var("NIGHTFALL_MONSTERS").ok().as_deref(),
    );
    super::rivals_view::force_from_env(&mut roll, content);
    roll
}
```

- Its test `shipped()` already reads `rivals.ron` (Task 1). If a test of `intel_lines` compares the whole list, it gains the `"Rivals: …"` line at the end.

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds with no new warnings in `nightfall/` or `nightfall-sim`.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: rivals drawn as hostile players, concealed when unseen, marked, and on the map table

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Docs, the full check, rivals on screen, and co-op

**Files:**
- Modify: `CLAUDE.md` (the Nightfall section), `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line), `scripts/package.sh` (only if it copies `content/` file by file), `docs/superpowers/plans/2026-10-17-nightfall-rivals-rulings.md` (what the shots show)

- [ ] **Step 1: Write the docs**

In `CLAUDE.md`'s Nightfall section, add:

```markdown
- Nightfall's rivals (sub-project 7): `content/nightfall/rivals.ron` has the rival kinds (each a
  class of its own, `rival:<key>`, built on a hero class `base`, with a simplified kit of
  abilities picked by tag and one telegraphed `big` move, or `kit: Class("rogue")` for a whole
  hero bar; gear and a `carry` loot table), the parties (2 or 3 members, an aggression range,
  odds), their points of interest and entries; `rules.ron`'s `rivals:` holds every number their
  bots go by (engage and hunt thresholds, fleeing, `pois`, `leave_by`, dodging, `give_up`,
  `downed`). A rival is a hero of another side (`Hero::side`, numbers from `RIVAL_BASE` 64) moved
  only by the `PlayerInput` its bot sets, so enemies, noise, searches, bodies and exits treat it
  as a hero; `nightfall-sim/src/rivals/` is the AI (`kit.rs` choice by tag, `goals.rs` party
  goals, `mind.rs` senses both ways, `drive.rs` the bots, `arrivals.rs` the roll's parties) and
  `combat/foes.rs` the fight between heroes and rivals. The client draws rivals as hostile WoW
  players (`nightfall/rivals_view.rs`). Test hooks: `NIGHTFALL_RIVALS=<party>,…` (the planned
  raid's rivals, out of the party's sight, at `NIGHTFALL_RIVALS_AT` seconds, 5 by default),
  `NIGHTFALL_TRACE_RIVALS=1` (each party's goal every 2 s). The `cutthroats` party has full rogue
  kits and odds 0, for playtesting full kits against simplified ones (spec §13).
```

In the spec's status line, after the sentence about sub-project 6, add: `Sub-project 7 built (plan docs/superpowers/plans/2026-10-17-nightfall-rivals.md).`

If `scripts/package.sh` copies `content/` file by file rather than whole, add `rivals.ron` to its Nightfall list; if it copies the folder, leave it.

- [ ] **Step 2: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS (the Darkshire bake test skips without `WOW_DATA`).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 3: Rivals near the entry, on screen**

A scratch copy of the content (never in `content/`) that brings a party in near the first entry (-10051.8, -792.5), in sight (`arrive_unseen` 0), with a point of interest 15 yards from it:

```bash
rm -rf target/nf-rivals && cp -r content/nightfall target/nf-rivals
sed -i 's/arrive_unseen: 50.0/arrive_unseen: 0.0/' target/nf-rivals/rules.ron
cat > target/nf-rivals/rivals.ron <<'EOF'
// Test only: two parties that come in 30 yards from the first entry and loot a point 15 yards
// from it, in the autoplay's sight.
(
    kinds: [
        (key: "grave_robber", name: "Grave Robber", names: ["Silas Dunmore"], base: "rogue",
         class_id: Some(1), race: 1, sex: 0, hp: Some(900.0),
         kit: Abilities(["robber_hack", "robber_dirt", "rival_bandage"]),
         big: Some("robber_shovel_smash"),
         gear: [("worn_shortsword", 1), ("gravediggers_boots", 1), ("linen_pouch", 1)],
         carry: Some("grave")),
        (key: "defias_scavenger", name: "Defias Scavenger", base: "rogue", race: 1, sex: 0,
         hp: Some(850.0), kit: Abilities(["stealth", "ambush", "defias_stab", "sprint"]),
         big: Some("defias_flurry"), sneaks: true,
         gear: [("raven_hill_dirk", 1), ("worn_dagger", 1), ("scavengers_leggings", 1),
                ("duskwood_satchel", 1)],
         carry: Some("crate")),
    ],
    parties: [
        (key: "grave_robbers", name: "Grave robbers", members: ["grave_robber", "grave_robber"],
         aggression: (0.1, 0.1), odds: 1.0),
        (key: "defias_scavengers", name: "Defias scavengers",
         members: ["defias_scavenger", "grave_robber"], aggression: (0.9, 0.9), odds: 1.0),
    ],
    pois: [(key: "near", name: "Near the entry", at: (-10040.0, -792.5), radius: 10.0)],
    entries: [(-10021.8, -792.5)],
)
EOF
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home NIGHTFALL_CONTENT=target/nf-rivals WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_SEED=7 NIGHTFALL_RIVALS=grave_robbers NIGHTFALL_RIVALS_AT=3 NIGHTFALL_TRACE_RIVALS=1 WOW_LIVE_SHOT=target/rivals-robbers.png WOW_LIVE_SHOT_AT=20 WOW_LIVE_SHOT_COUNT=8 WOW_LIVE_SHOT_EVERY=3 ./target/release/azeroth-together.exe > target/rivals-robbers.log 2>&1 &
```

Expected, once `target/rivals-robbers-007.png` exists (then `kill` the PID):
- Two players in a warrior's starting clothes and gravedigger's boots, a shortsword drawn, walk (WoW's walk, not a run) toward the point near the entry; their nameplates are red; one behind a building in a shot shows no plate.
- The log's `nightfall: rivals: party 0 Grave robbers:` lines go `Wait` → `Loot { poi: 0 }`; once the autoplay's hero is in their front cone a "?" shows over them, and the log shows the party fleeing (`Flee`, aggression 0.10) rather than fighting.
- `grep -c "panicked" target/rivals-robbers.log` prints `0`.

Then the bold party, which fights:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home NIGHTFALL_CONTENT=target/nf-rivals WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 NIGHTFALL_SEED=7 NIGHTFALL_RIVALS=defias_scavengers NIGHTFALL_RIVALS_AT=3 NIGHTFALL_TRACE_RIVALS=1 WOW_LIVE_SHOT=target/rivals-defias.png WOW_LIVE_SHOT_AT=20 WOW_LIVE_SHOT_COUNT=10 WOW_LIVE_SHOT_EVERY=2 ./target/release/azeroth-together.exe > target/rivals-defias.log 2>&1 &
```

Expected, once `target/rivals-defias-009.png` exists (then `kill` the PID): a Defias scavenger fades into stealth on its way (WoW's stealth look) and has no plate until near; once alerted a "!" shows over them and the log shows `Fight`; a red circle shows on the ground around one of them before its flurry or smash lands; yellow and red numbers rise as the hero and the rivals trade blows; WoW's target frame, when the autofight targets one, is red with its name; a slain rival leaves a body the loot cursor shows over. `grep -c "panicked" target/rivals-defias.log` prints `0`.

Note in the rulings' client lines which looks are wrong enough to fix first (the outfits, the walk, the stealth fade on a player unit).

- [ ] **Step 4: Rivals in co-op on one PC**

Start the host in the background, and a few seconds later the client:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-host NIGHTFALL_CONTENT=target/nf-rivals WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_NAME=Host NIGHTFALL_SEED=7 NIGHTFALL_RIVALS=grave_robbers NIGHTFALL_RIVALS_AT=10 WOW_LIVE_SHOT=target/host-rivals.png WOW_LIVE_SHOT_AT=45 WOW_LIVE_SHOT_COUNT=4 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/host-rivals.log 2>&1 &
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home-client NIGHTFALL_CONTENT=target/nf-rivals WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend NIGHTFALL_TRACE_RIVALS=1 WOW_LIVE_SHOT=target/client-rivals.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=5 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/client-rivals.log 2>&1 &
```

Expected, once both last shots exist (then `kill` both PIDs): both clients draw the two grave robbers as red-plated players in the same places; the client's log shows `nightfall: rivals seen [64, 65]` (or fewer when one is out of its hero's sight) and no `party` lines (the AI is the host's); neither log has a panic, and the client's log has no protocol refusal (both are protocol 8).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md scripts/package.sh docs/superpowers/plans/2026-10-17-nightfall-rivals-rulings.md
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: document the rivals; sub-project 7 built

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Spec coverage

| Spec | Where |
|---|---|
| §7.4 parties of 2 to 3 bots: grave robbers, Defias scavengers, Night Watch deserters | Task 1 (`rivals.ron`'s kinds and parties, 2 or 3 members checked), Task 9 (rolled), Task 11 (drawn) |
| §7.4 the same sight, noise and awareness rules as heroes: snuck past, overheard, ambushed; noticing heroes as enemies do | Task 4 (rivals are heroes to enemies, noises, scent), Task 7 (`sense` shared, `Mind`, shouts, ambush, a stealthed rival seen as a stealthed hero is), Task 11 (concealed when unseen, marks) |
| §7.4 goals: points of interest for loot, monsters fought on the way, then extract or hunt by aggression | Task 3 (`decide`, `next_poi`), Task 8 (looting through acts, fighting monsters after them or guarding a point, exits, hunting toward what they heard), Task 10 (whole raids) |
| §7.4 bags carried and dropped when killed | Task 4 (gear and bag worn, a `carry` roll; the body holds everything), Task 10 (a hero takes a slain rival's bag) |
| §7.4 kits from content by tag (opener, nuke, control, escape, heal); simplified kits of 3 or 4 and one telegraphed big move; a full kit as a content change | Task 1 (`KitDef`, `big` with a telegraph, `Effect::Heal`), Task 2 (`choose`), Task 5 (the same effects on heroes), Task 6 (the big move telegraphed and dodgeable), Task 1's `cutthroats` (full rogue kits) |
| §6.1 "rivals' big moves show a ground circle or cone 1 to 1.5 s before they land" | Task 6 (`put_telegraph`; the shipped big moves' windups are 1.2 and 1.5 s), Task 11 (drawn by the telegraph code already) |
| §4.2, §4.3 rival sight; which rivals a hero sees | Task 7 (`rival_view`, `Snapshot::rivals`), Task 11 (`Concealed`) |
| §8.1 "rivals can follow them in" to Darkshire; §8.2 rivals' bags among containers | Rivals walk anywhere a path goes (Task 8); their bodies are containers (Task 4) |
| §13 rival kits: simplified or full, playtest both | Task 1 (`kit: Class("rogue")`, the `cutthroats` party at odds 0), Task 11 (`NIGHTFALL_RIVALS`) |
| §1.1 bots with goals of their own (Single Player Tarkov's questing bots) | Tasks 3 and 8 |
| §11 settings in content | `rules.ron`'s `rivals:` (Task 1), every number in `RivalRules` |
| §12 deterministic tests on hand-made maps | Tasks 2–10 |

## Self-review

- Every interface consumed from an earlier plan is named as that plan gives it; where this plan edits a function an earlier plan wrote (`land`, `check`, `begin`, `go`, `step_cast`, `stop_cast`, `wound`, `down`, `step_fall`, `step_deaths`, `step_over`, `abandon`, `extract`, `tally_kills`, `telegraph_lands`, `kill_enemy`, `sense`, `seen_by`, `snapshot_for`, `apply_snapshot`, `step`, `RaidRoll::roll`, `begin_raid`, `intel_lines`, `plan_from_env`, `place`, `show_events`, `refusal_key`), the edit is spelled out against the code that plan shows.
- Names stay one across tasks: `Side`, `RIVAL_BASE`, `Rivals`, `RivalBody`, `Party`, `Way`, `RivalView`, `RivalMark`, `RivalOut`, `Goal`, `PartyView`, `KitSlot`, `Moment`, `choose`, `ready`, `kit_of`, `add_rival_hero`, `add_party`, `step_rivals`, `perceive_rivals`, `alert_rival`, `rival_struck`, `rival_leaves`, `rival_view`, `check_foe`, `land_foe`, `land_at_foe`, `strike_foe`, `foe_unaware`, `foe_ambush`, `put_telegraph`, `foe_telegraph_lands`.
- Each review risk is pinned by a named test in the task that builds the rule.
- The places in `rivals.ron` and the rivals' abilities' spell ids are `measure:` placeholders, as every earlier plan's are; the rulings say what they stand in for.
