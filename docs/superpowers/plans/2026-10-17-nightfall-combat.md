# Nightfall Sub-project 4 (Combat) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fights worth choosing: WoW tab-target combat with the real action bar, cast bar, global cooldown, ranges and facing, at a fixed level 30; the Mage's and the Rogue's kits with their noise; per-ability movement while casting (free, slowed, rooted); a dodge for everyone with charges and a moment in which telegraphed blows miss; throwing a stone, eating and drinking; the ambush and the openers; enemies that swing back, casters that cast from range and elites whose big moves show on the ground before they land; and heroes who fall, are revived or die.

**Architecture:** `nightfall-sim` gains abilities as content (`abilities.rs`, `content/nightfall/abilities.ron`) and a `combat` module: pure arithmetic (`rules.rs`: dice, arcs, ranges, the casting allowance, dodge charges), what abilities do (`effects.rs`), heroes' presses and casts (`heroes.rs`), enemies held and hurt over time (`hold.rs`), heroes falling (`falling.rs`), and enemies striking back with telegraphs and the dodge (`strikes.rs`). Every hero and enemy carries its fight in the snapshot (`HeroFight`, `EnemyFight`), telegraphs ride beside them, and each step's `Event`s travel in `State`. Clients own their position, so the host enforces movement while casting by lowering the hero's speed (WoW's controller takes it), rooting rooted casts, and breaking any cast whose hero moved further than allowed. The client takes WoW's own `UseAction` presses before the engine's cast path sees them, sends them as `CastPress`es aimed at the current target or the ground under the cursor, dodges with a hop on a WoW key binding of its own, and shows the fight through the engine's cast bar, cooldown sweeps, power and combo fields, auras, spell visuals, swings and combat text, plus telegraph shapes drawn on the terrain.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18 (client only), serde + ron (content), postcard (wire).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md`: §6 in full; §5.4 (the ×1.5 ambush, openers need an unaware target); §7.1's casters and elites fighting back; §8.1's falling (downed 45 s, revive 5 s, dead after); §11's `cast_move_speed` and `downed_for`; §13's dodge key. Loot, bags and the inn are sub-project 5 and stay out. Decisions the spec leaves open are in `docs/superpowers/plans/2026-10-17-nightfall-combat-rulings.md`. This plan starts after every task of `2026-10-16-nightfall-foundation.md`, `2026-10-17-nightfall-world-knowledge.md` and `2026-10-17-nightfall-senses.md` is done, and edits files those plans wrote; every name it consumes from them is the one those plans give.

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call (or the global `safe.directory` once). Other agents commit in this worktree: stage only the files a task names, never `git add -A`.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`. This machine runs out of memory on wide builds: build `benilla-app` and `azeroth-together` with `-j 2` (`-j 1` if a build is killed).
- The worktree has no `WoW/Data`: every game launch sets `WOW_DATA=D:/Games/azeroth-together/WoW/Data` and `BENILLA_HOME=target/nf-home`. The exe does not exit after its shots: start it in the background, wait for the last shot file, then `kill` its PID.
- `nightfall-sim` depends on neither `together-sim`, Bevy nor `benilla-formats` (spec §3.1). Every combat rule is pure sim logic with unit tests on hand-made maps (`heightmap::testmap`).
- Abilities are data: every number of an ability lives in `content/nightfall/abilities.ron`, starting from vanilla's level-30 rank (its `Spell.dbc` id, so WoW's own tooltip, icon and cast-bar name fit), and anyone (hero bar, enemy move, later a rival) names abilities by key. Every number of the fight's rules lives in `content/nightfall/rules.ron` under `combat:`, the spec's value as the default when left out.
- The host decides (spec §3.1): clients send presses (`PlayerInput::cast`, `dodge_presses`, the senses' `stealth_presses`) and draw what the host says. The one thing a client does at once is the dodge's hop and a Blink's teleport, because clients own their position.
- No engine crate changes: everything the client needs is already public to `benilla-app` (`UiScript::take_action_uses`, `register_addon_bindings`, `CastBarFeed`, `Cooldowns`, `KnockBackMessage`, `TeleportMessage`, `MoveModeMessage`, `Engaged`, the fx messages). New files are preferred; edits to files earlier plans wrote are spelled out.
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A press lost, doubled or replayed** (the same press repeated in every input until the next, two presses between steps, a new run whose first input still carries the last run's press, the dodge count wrapping past 255). Expect each press cast or refused exactly once and nothing at a run's start. Pinned in Task 5 (`a_press_repeated_in_every_input_casts_once`, `a_new_run_does_not_replay_the_last_press`) and Task 8 (`dodges_come_back_one_at_a_time_and_are_heard`, whose second half sends a count round past 255).
2. **Moving while casting, which the host cannot stop because clients own their position.** Expect a slowed cast to go on at the slowed pace (with a grace while the slow reaches a joined client) and break beyond it, a rooted cast refused on the move and broken by a step, and an instant never held. Pinned in Task 2 (`a_rooted_cast_allows_a_nudge_a_slowed_one_the_slowed_pace_after_a_grace`) and Task 5 (`a_slowed_cast_allows_the_slowed_pace_and_breaks_beyond_it`, `a_rooted_cast_cannot_begin_on_the_move_and_breaks_when_the_hero_moves`).
3. **The dodge's window at its edges** (a telegraph landing 9 steps after the press, and 10). Expect a miss inside 0.3 s and a hit after it, a charge spent only when there was one. Pinned in Task 8 (`a_dodge_makes_a_telegraph_miss_only_inside_its_window`).
4. **The ambush and the openers across heroes** (an enemy alerted to your friend, a second hit, Sap which must not alert). Expect ×1.5 on the first blow from each hero not yet noticed, openers refused only when the target is alerted to that hero. Pinned in Task 4 (`damage_lands_with_the_ambush_on_an_unaware_enemy_once`, `an_enemy_alerted_to_a_friend_is_still_caught_unaware_by_you`, `sap_holds_without_alerting`) and Task 5 (`a_rogues_presses_are_refused_for_what_is_wrong`).
5. **Falling with enemies around** (a hero downed beside the enemy that downed them, a reviver who walks away, a hero who stays down 45 s). Expect the enemy to lose interest, the revive count to start over, death at 45 s, and a downed or dead hero unable to press. Pinned in Task 7 (`an_enemy_leaves_a_downed_hero_alone`, `walking_away_starts_the_revive_over`, `unrevived_for_45_seconds_is_dead_and_cannot_press`).

**Not in this plan** (see the rulings file): loot, bags, weight, food and water as items, the run's end screen (sub-project 5); Stitches, ravens and the night's stages (sub-project 6); rivals (sub-project 7, whose kits will name these abilities); misses, parries, armour and resistances; crowd control on heroes.

## File Structure

| File | Responsibility |
|---|---|
| `crates/nightfall-sim/src/abilities.rs` (new) | `AbilityDef` and its parts (`Aim`, `Moving`, `Sound`, `Facing`, `Needs`, `Effect`, `ControlKind`, `Creature`, `Shape`, `TelegraphDef`, `Tag`), `Shape::contains`, checks |
| `crates/nightfall-sim/src/content.rs` | `CombatRules`, `DodgeRules`, `Swing`, `MoveDef`; classes' power, swing and bar; enemies' creature, elite, swing, reach and moves; `abilities.ron` loaded and checked |
| `crates/nightfall-sim/src/combat/mod.rs` (new) | The fight's wire types (`HeroFight`, `EnemyFight`, `Life`, `Casting`, `Cooldown`, `UnitRef`, `Telegraph`, `Event`, `Refusal`, `Stop`, `CastPress`), the host-only `Combat`, events on `World` |
| `crates/nightfall-sim/src/combat/rules.rs` (new) | Pure: `Dice`, arcs, `stands_right`, `in_range`, `allowance`, `Charges` |
| `crates/nightfall-sim/src/combat/effects.rs` (new) | `World::land`, `strike`, `kill_enemy`; holds, slows, bleeds, combos; Vanish, Sprint, Blink, Distract, eating, conjured water |
| `crates/nightfall-sim/src/combat/heroes.rs` (new) | Presses taken once, checks and refusals, GCD, cooldowns, power, casts and channels, the casting allowance, auto-attack, meals, regeneration, pace |
| `crates/nightfall-sim/src/combat/hold.rs` (new) | Holds, slows, distractions and burns ticking; the sheep; `Engage` for the senses |
| `crates/nightfall-sim/src/combat/falling.rs` (new) | `World::hurt_hero`, downed, crawling, revives, death |
| `crates/nightfall-sim/src/combat/strikes.rs` (new) | Enemy swings, casts and telegraphed blows; the dodge |
| `crates/nightfall-sim/src/combat/tests.rs` (new) | The fight in whole worlds on hand-made maps |
| `crates/nightfall-sim/src/senses/{mod,react,awareness}.rs` | `may_stealth`; `Engage` holds and reaches; downed heroes unsensed; `lose`, `alerted`, `hunted`, `target_of` |
| `crates/nightfall-sim/src/{world,enemies,sight,messages,lib}.rs` | Fight fields, telegraphs and events in the world, snapshot and `State`; protocol 4 |
| `content/nightfall/{abilities,classes,enemies,spawns,rules}.ron` | The kits, the bars, enemies' blows and moves, two rares, the combat rules |
| `crates/benilla-app/src/nightfall/bar.rs` (new) | The bar filled; `UseAction` presses into `CastPress`es; ground aim; `NIGHTFALL_AUTOFIGHT` |
| `crates/benilla-app/src/nightfall/dodge.rs` (new) | The `NIGHTFALL_DODGE` binding, the hop |
| `crates/benilla-app/src/nightfall/fx.rs` (new) | `Fx`: casts, impacts, swings and combat text as a server's packets would show them |
| `crates/benilla-app/src/nightfall/fight.rs` (new) | Events shown; power, combo, cooldown sweeps, auto-attack stance, roots |
| `crates/benilla-app/src/nightfall/auras.rs` (new) | Hero buffs and stand states (Stealth's bar page), enemy debuffs, the sheep |
| `crates/benilla-app/src/nightfall/telegraphs.rs` (new) | Telegraph shapes draped on the terrain |
| `crates/benilla-app/src/nightfall/fallen.rs` (new) | The downed or dead player's status line, the death popup, `NIGHTFALL_FRAIL` |
| `crates/benilla-app/src/nightfall/{mod,run,session,units,view,stance,conceal}.rs` | Registration, events through the session, the input's presses, `unit_of`, vitals |

---

### Task 1: Content: abilities, kits, enemies' blows and moves, the combat rules

**Files:**
- Create: `crates/nightfall-sim/src/abilities.rs`, `content/nightfall/abilities.ron`
- Modify: `crates/nightfall-sim/src/content.rs`, `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`
- Modify (every `ContentSources { … }` literal gains `abilities`): `crates/nightfall-sim/src/{world,enemies,sight,messages}.rs` and `senses/tests.rs` (test fixtures)
- Modify: `content/nightfall/classes.ron`, `content/nightfall/enemies.ron`, `content/nightfall/spawns.ron`, `content/nightfall/rules.ron`
- Modify (so the client still builds): `crates/benilla-app/src/nightfall/mod.rs` (`prepare`, `content_texts`)

**Interfaces:**
- Consumes: `Content::from_ron`, `ContentSources`, `ContentError`, `ClassDef`, `EnemyDef`, `Rules`, the test fixtures `CLASSES`, `MAP`, `RULES`, `load`, `load_all` (foundation Task 3, world knowledge Task 5); `Loudness` (senses Task 3); `V2`.
- Produces:
  - `nightfall_sim::{AbilityDef, Aim, Moving, Sound, Facing, Needs, Effect, ControlKind, Creature, Shape, TelegraphDef, Tag}`; `Sound::loudness(self) -> Option<Loudness>`; `ControlKind::{freed_by_damage, out}(self) -> bool`; `Effect::on_enemy(&self) -> bool`; `Shape::{contains(&self, at: V2, facing: V2, p: V2) -> bool, reach(&self) -> f32}`; `AbilityDef::{windup(&self) -> f32, has(&self, impl Fn(&Effect) -> bool) -> bool}`.
  - `Swing { min, max, every }`, `MoveDef { ability: String, every: f32 }`, `DodgeRules { charges: u8, recharge, immune, distance }` (`Copy`), `CombatRules { cast_move_speed, cast_slack, speed_grace, reach, front, crit_chance, crit_times, dodge, combat_linger, regen: (f32, f32), downed_for, revive_for, revive_reach, revive_health, crawl_speed, sheep_speed, combo_max: u8, max_drinks: u8 }`; `Rules::combat`.
  - `ClassDef` gains `power_max`, `power_regen`, `swing: Option<Swing>`, `bar: Vec<(u8, String)>`, `kit: Vec<(u8, usize)>` (resolved); `EnemyDef` gains `creature`, `elite`, `swing: Option<Swing>`, `reach`, `moves: Vec<MoveDef>`, `kit: Vec<(usize, f32)>` (resolved).
  - `ContentSources::abilities: &str`; `Content::abilities: Vec<AbilityDef>`; `Content::ability(&self, key: &str) -> Option<usize>`.
  - Client: `nightfall::content_texts() -> &'static [String; 6]`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/abilities.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circle_holds_what_is_within_its_radius() {
        let s = Shape::Circle { radius: 8.0 };
        let at = V2::new(10.0, 10.0);
        assert!(s.contains(at, V2::new(1.0, 0.0), V2::new(17.9, 10.0)));
        assert!(!s.contains(at, V2::new(1.0, 0.0), V2::new(18.1, 10.0)));
        assert_eq!(s.reach(), 8.0);
    }

    #[test]
    fn a_cone_holds_what_is_ahead_within_its_angle_and_length() {
        let s = Shape::Cone {
            length: 8.0,
            angle: 120.0,
        };
        let at = V2::new(0.0, 0.0);
        let north = V2::new(1.0, 0.0);
        assert!(s.contains(at, north, V2::new(5.0, 0.0)));
        // 50° off straight ahead: inside the 60° half-angle.
        assert!(s.contains(at, north, V2::from_angle(50f32.to_radians(), 5.0)));
        assert!(!s.contains(at, north, V2::from_angle(70f32.to_radians(), 5.0)));
        assert!(!s.contains(at, north, V2::new(9.0, 0.0)));
        assert!(!s.contains(at, north, V2::new(-3.0, 0.0)));
    }

    #[test]
    fn holds_and_their_freeing() {
        assert!(!ControlKind::Stun.freed_by_damage());
        assert!(ControlKind::Sap.freed_by_damage());
        assert!(ControlKind::Root.freed_by_damage());
        assert!(!ControlKind::Root.out());
        assert!(ControlKind::Polymorph.out());
        assert_eq!(Sound::Silent.loudness(), None);
        assert_eq!(Sound::Loud.loudness(), Some(Loudness::Loud));
        assert!(Effect::Combo(1).on_enemy());
        assert!(!Effect::Vanish.on_enemy());
    }
}
```

In `crates/nightfall-sim/src/content.rs`'s `tests` module, add below `SPAWNS`:

```rust
    const ABILITIES: &str = r#"[
        (key: "bolt", name: "Bolt", spell: 116, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
         gcd: 1.5, moving: Slowed, effects: [Damage(min: 100.0, max: 110.0)]),
        (key: "nova", name: "Nova", spell: 122, aim: Me, radius: 10.0, cooldown: 25.0,
         effects: [Control(kind: Root, secs: 8.0)]),
    ]"#;
    const KIT_CLASSES: &str = r#"[
        (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 880.0,
         speed: 7.0, power_max: 1300.0, bar: [(1, "bolt"), (4, "nova")]),
    ]"#;
    const KIT_ENEMIES: &str = r#"[
        (key: "weaver", name: "Weaver", display: 203, level: 28, hp: 900.0, walk_speed: 2.5,
         run_speed: 7.0, swing: Some((min: 20.0, max: 30.0, every: 2.0)),
         moves: [(ability: "bolt", every: 6.0)]),
    ]"#;

    /// Content with `abilities`, a bar and enemy moves, on the tests' map with no spawns.
    fn load_kit(
        classes: &str,
        abilities: &str,
        rules: &str,
        enemies: &str,
    ) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes,
            map: MAP,
            rules,
            enemies,
            spawns: "[]",
            abilities,
        })
    }

    #[test]
    fn a_bar_resolves_to_its_abilities_and_moves_to_theirs() {
        let c = load_kit(KIT_CLASSES, ABILITIES, RULES, KIT_ENEMIES).unwrap();
        assert_eq!(c.ability("nova"), Some(1));
        assert_eq!(c.classes[0].kit, vec![(1, 0), (4, 1)]);
        assert_eq!(c.classes[0].power_max, 1300.0);
        assert_eq!(c.enemies[0].kit, vec![(0, 6.0)]);
        assert_eq!(c.enemies[0].reach, 2.5);
        assert_eq!(c.enemies[0].creature, Creature::Humanoid);
        assert!(!c.enemies[0].elite);
        let bolt = &c.abilities[0];
        assert_eq!(bolt.moving, Moving::Slowed);
        assert_eq!(bolt.sound, Sound::Normal);
        assert_eq!(bolt.ticks, 1);
        assert!(bolt.alerts);
        assert_eq!(bolt.windup(), 2.0);
    }

    #[test]
    fn a_bar_or_a_move_naming_no_ability_is_refused() {
        let bad_bar = KIT_CLASSES.replace("(4, \"nova\")", "(4, \"fireball\")");
        assert!(matches!(
            load_kit(&bad_bar, ABILITIES, RULES, KIT_ENEMIES),
            Err(ContentError::Invalid(_))
        ));
        let twice = KIT_CLASSES.replace("(4, \"nova\")", "(1, \"nova\")");
        assert!(matches!(
            load_kit(&twice, ABILITIES, RULES, KIT_ENEMIES),
            Err(ContentError::Invalid(_))
        ));
        let bad_move = KIT_ENEMIES.replace("ability: \"bolt\"", "ability: \"cleave\"");
        assert!(matches!(
            load_kit(KIT_CLASSES, ABILITIES, RULES, &bad_move),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn abilities_that_cannot_work_are_refused() {
        for (from, to) in [
            ("cost: 50.0", "cost: -5.0"),
            ("range: 30.0, cost", "range: 0.0, cost"),
            ("Damage(min: 100.0, max: 110.0)", "Damage(min: 100.0, max: 90.0)"),
            ("Control(kind: Root, secs: 8.0)", "Control(kind: Root, secs: 0.0)"),
            ("Damage(min: 100.0, max: 110.0)", "Slow(by: 1.5, secs: 7.0)"),
            ("cast: 2.0,", "cast: 2.0, channel: 3.0,"),
        ] {
            let abilities = ABILITIES.replacen(from, to, 1);
            assert!(
                matches!(
                    load_kit(KIT_CLASSES, &abilities, RULES, KIT_ENEMIES),
                    Err(ContentError::Invalid(_))
                ),
                "{to}"
            );
        }
        let twice = ABILITIES.replace("key: \"nova\"", "key: \"bolt\"");
        assert!(matches!(
            load_kit(KIT_CLASSES, &twice, RULES, KIT_ENEMIES),
            Err(ContentError::Invalid(_))
        ));
    }

    #[test]
    fn the_combat_rules_default_to_the_spec_and_refuse_what_cannot_work() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        let r = &c.rules.combat;
        assert_eq!(r.cast_move_speed, 0.5);
        assert_eq!(r.downed_for, 45.0);
        assert_eq!(r.revive_for, 5.0);
        assert_eq!((r.dodge.charges, r.dodge.recharge, r.dodge.immune), (2, 8.0, 0.3));
        assert_eq!(r.dodge.distance, 6.0);
        assert_eq!(r.regen, (0.0, 0.0));
        for bad in [
            "cast_move_speed: 0.0",
            "front: 200.0",
            "crit_chance: 1.5",
            "dodge: (charges: 0)",
            "revive_health: 0.0",
            "combo_max: 9",
        ] {
            let rules = RULES.replacen(
                "(hero_level: 30,",
                &format!("(hero_level: 30, combat: ({bad}),"),
                1,
            );
            assert!(
                matches!(load(CLASSES, MAP, &rules), Err(ContentError::Invalid(_))),
                "{bad}"
            );
        }
    }
```

In `crates/nightfall-sim/tests/shipped_content.rs`, `shipped()`'s `ContentSources { … }` gains `abilities: include_str!("../../../content/nightfall/abilities.ron"),`, and add:

```rust
#[test]
fn the_shipped_kits_are_on_their_bars_and_enemies_fight_back() {
    use nightfall_sim::{Aim, ClassDef, Effect, Moving};

    let c = shipped();
    let rogue = &c.classes[c.class("rogue").unwrap()];
    let mage = &c.classes[c.class("mage").unwrap()];
    let on_bar = |class: &ClassDef, key: &str| {
        class.kit.iter().any(|&(_, a)| c.abilities[a].key == key)
    };
    for key in [
        "stealth", "sap", "cheap_shot", "garrote", "ambush", "sinister_strike", "eviscerate",
        "gouge", "sprint", "distract", "vanish", "throw_stone", "eat_and_drink",
    ] {
        assert!(on_bar(rogue, key), "rogue: {key}");
    }
    for key in [
        "frostbolt", "fireball", "fire_blast", "frost_nova", "arcane_missiles", "polymorph",
        "blink", "conjure_water", "throw_stone", "eat_and_drink",
    ] {
        assert!(on_bar(mage, key), "mage: {key}");
    }
    let a = |key: &str| &c.abilities[c.ability(key).unwrap()];
    assert_eq!(a("frostbolt").cast, 2.5);
    assert_eq!(a("frostbolt").moving, Moving::Slowed);
    assert_eq!(a("fireball").moving, Moving::Rooted);
    assert_eq!(a("arcane_missiles").moving, Moving::Rooted);
    assert_eq!(a("blink").cooldown, 15.0);
    assert_eq!(a("throw_stone").range, 25.0);
    assert_eq!(a("throw_stone").cooldown, 20.0);
    assert_eq!(a("throw_stone").aim, Aim::Ground);
    assert!(a("vanish").has(|e| matches!(e, Effect::Vanish)));
    assert_eq!(c.rules.combat.downed_for, 45.0);
    assert_eq!(rogue.power_max, 100.0);
    assert!(rogue.swing.is_some());
    assert!(mage.power_max > 0.0);
    let weaver = &c.enemies[c.enemy("nightbane_shadow_weaver").unwrap()];
    assert!(!weaver.kit.is_empty());
    assert!(c.enemies.iter().filter(|e| e.elite).count() >= 2);
    for e in c.enemies.iter().filter(|e| e.elite) {
        assert!(
            e.kit.iter().any(|&(m, _)| c.abilities[m].telegraph.is_some()),
            "{}",
            e.key
        );
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim content abilities`
Expected: FAIL to compile (`cannot find type Shape`, `struct ContentSources has no field named abilities`, `no field combat on type Rules`).

- [ ] **Step 3: Implement the abilities and the content**

At the top of `crates/nightfall-sim/src/abilities.rs`:

```rust
//! Abilities as content (`content/nightfall/abilities.ron`, spec §6): what each costs and takes,
//! how far it reaches and how loud it is, what its caster may do with their feet meanwhile, and
//! what it does where it lands. Heroes' bars, enemies' moves and (in sub-project 7) rivals' kits
//! all name abilities from this one table by key, so giving anyone any ability is a content
//! change, not a code change.

use serde::{Deserialize, Serialize};

use crate::{Loudness, V2};

/// What a caster may do with their feet while an ability is cast (spec §6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Moving {
    /// Instants: nothing holds the caster.
    #[default]
    Free,
    /// Cast-time spells: the caster moves at `cast_move_speed`.
    Slowed,
    /// Channels and the biggest nukes: the caster stands, and must be standing to begin.
    Rooted,
}

/// How loud an ability is where it goes off (spec §5.2's quiet, normal and loud), or silent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Sound {
    Silent,
    Quiet,
    #[default]
    Normal,
    Loud,
}

impl Sound {
    /// The noise it makes, if any.
    pub fn loudness(self) -> Option<Loudness> {
        match self {
            Sound::Silent => None,
            Sound::Quiet => Some(Loudness::Quiet),
            Sound::Normal => Some(Loudness::Normal),
            Sound::Loud => Some(Loudness::Loud),
        }
    }
}

/// What an ability is aimed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Aim {
    /// The caster's target, an enemy.
    Enemy,
    /// The caster, and every enemy within `radius` of them when it has one.
    Me,
    /// A spot on the ground within `range`, and every enemy within `radius` of it.
    Ground,
}

/// Where the caster must stand toward its target (WoW's facing rules, spec §6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Facing {
    #[default]
    Any,
    /// Facing the target.
    Front,
    /// Facing the target, from behind it.
    Behind,
}

/// A creature's kind, for abilities that take only some (Sap humanoids, Polymorph humanoids and
/// beasts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Creature {
    #[default]
    Humanoid,
    Beast,
    Undead,
}

/// What holds an enemy (spec §6.2, §6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ControlKind {
    /// No step, no blow (Cheap Shot); damage does not free it.
    Stun,
    /// No step, no blow (Gouge); freed by damage.
    Incapacitate,
    /// A sheep that wanders (Polymorph); freed by damage.
    Polymorph,
    /// Out of the fight and unaware (Sap); freed by damage.
    Sap,
    /// Held where it stands, still striking what it reaches (Frost Nova); freed by damage.
    Root,
}

impl ControlKind {
    /// Whether a blow frees the enemy.
    pub fn freed_by_damage(self) -> bool {
        self != ControlKind::Stun
    }

    /// Whether it keeps the enemy out of the fight altogether: no looking, thinking or blows.
    pub fn out(self) -> bool {
        self != ControlKind::Root
    }
}

/// What an ability does where it lands.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Effect {
    /// Damage from `min` to `max`, and `per_combo` more for each combo point a finisher spends.
    Damage {
        min: f32,
        max: f32,
        #[serde(default)]
        per_combo: f32,
    },
    /// The class weapon's blow, `percent` of it and `bonus` more.
    Weapon {
        #[serde(default = "hundred")]
        percent: f32,
        #[serde(default)]
        bonus: f32,
    },
    /// `amount` every `every` seconds for `secs` (a bleed, a burn).
    Dot { amount: f32, every: f32, secs: f32 },
    /// Slower by `by` (0.4 is 40% slower) for `secs`.
    Slow { by: f32, secs: f32 },
    /// Held (see [`ControlKind`]) for `secs`.
    Control { kind: ControlKind, secs: f32 },
    /// Combo points on the target (spec §6.3).
    Combo(u8),
    /// The Rogue's Stealth: its button counts a Stealth press (`PlayerInput::stealth_presses`).
    Stealth,
    /// Every enemy after the caster loses them, and they are in stealth again.
    Vanish,
    /// Faster by `by` for `secs`.
    Sprint { by: f32, secs: f32 },
    /// `distance` yards the way the caster faces, stopping short of walls.
    Blink { distance: f32 },
    /// Enemies within the ability's radius of the spot turn to it for `secs`.
    Distract { secs: f32 },
    /// Health (and mana, drinking a conjured drink) back over `secs`, sitting still.
    Eat { secs: f32 },
    /// `count` conjured drinks.
    ConjureWater { count: u8 },
}

impl Effect {
    /// Whether it does anything to the enemies it lands on.
    pub fn on_enemy(&self) -> bool {
        matches!(
            self,
            Effect::Damage { .. }
                | Effect::Weapon { .. }
                | Effect::Dot { .. }
                | Effect::Slow { .. }
                | Effect::Control { .. }
                | Effect::Combo(_)
        )
    }
}

fn hundred() -> f32 {
    100.0
}

fn one() -> u32 {
    1
}

fn yes() -> bool {
    true
}

/// What an ability needs before it may be used (WoW's own checks).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Needs {
    /// Only from stealth.
    pub stealth: bool,
    /// An opener: its target must not be alerted to the caster (spec §5.4).
    pub unaware: bool,
    /// A finisher: combo points on its target.
    pub combo: bool,
    pub out_of_combat: bool,
    /// Not while moving.
    pub still: bool,
    /// Only these kinds of creature; any when empty.
    pub creatures: Vec<Creature>,
}

/// The ground a telegraphed blow lands on (spec §6.1).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub enum Shape {
    Circle { radius: f32 },
    /// From the caster the way it faces, `angle` degrees wide.
    Cone { length: f32, angle: f32 },
}

impl Shape {
    /// Whether the ground point `p` is in this shape put at `at`, turned toward `facing`.
    pub fn contains(&self, at: V2, facing: V2, p: V2) -> bool {
        let d = p - at;
        match *self {
            Shape::Circle { radius } => d.length() <= radius,
            Shape::Cone { length, angle } => {
                if d.length() > length {
                    return false;
                }
                let (f, n) = (facing.normalize_or_zero(), d.normalize_or_zero());
                n == V2::ZERO || f.dot(n) >= (angle * 0.5).to_radians().cos() - 1e-5
            }
        }
    }

    /// How far from where it is put it reaches.
    pub fn reach(&self) -> f32 {
        match *self {
            Shape::Circle { radius } => radius,
            Shape::Cone { length, .. } => length,
        }
    }
}

/// A blow shown on the ground before it lands.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct TelegraphDef {
    pub shape: Shape,
    /// Seconds from the shape showing to the blow landing (spec §6.1: 1 to 1.5).
    pub windup: f32,
    /// A circle around the caster, not at its target.
    #[serde(default)]
    pub on_self: bool,
}

/// What an AI may use an ability for: enemies now, rivals' kits later (spec §7.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Tag {
    Opener,
    Builder,
    Finisher,
    Nuke,
    Control,
    Escape,
    Heal,
    Utility,
}

/// One ability.
#[derive(Clone, Debug, Deserialize)]
pub struct AbilityDef {
    pub key: String,
    pub name: String,
    /// `Spell.dbc` id: the button's icon and tooltip, the cast bar's name, the visual.
    pub spell: u32,
    /// The visual of each tick of a channel (Arcane Missiles' missile), when not `spell`'s.
    #[serde(default)]
    pub tick_spell: Option<u32>,
    pub aim: Aim,
    /// Yards to the target or spot; the target's size is added (`CombatRules::reach`).
    #[serde(default)]
    pub range: f32,
    /// Yards around the caster or the spot an area ability reaches.
    #[serde(default)]
    pub radius: f32,
    /// Mana or energy.
    #[serde(default)]
    pub cost: f32,
    /// Seconds to cast; 0 for an instant.
    #[serde(default)]
    pub cast: f32,
    /// Seconds a channel lasts, in `ticks` even ticks; 0 for none.
    #[serde(default)]
    pub channel: f32,
    #[serde(default = "one")]
    pub ticks: u32,
    #[serde(default)]
    pub cooldown: f32,
    /// The global cooldown it starts (a mage's 1.5, a rogue's 1); 0 for none.
    #[serde(default)]
    pub gcd: f32,
    #[serde(default)]
    pub moving: Moving,
    #[serde(default)]
    pub sound: Sound,
    /// A fire spell's flash on the caster (spec §5.1).
    #[serde(default)]
    pub flash: bool,
    #[serde(default)]
    pub facing: Facing,
    #[serde(default)]
    pub needs: Needs,
    /// Landing on an enemy alerts it to the caster (all but Sap and Distract).
    #[serde(default = "yes")]
    pub alerts: bool,
    /// The caster stays in stealth (Sap, Sprint, Distract, Vanish, Stealth itself).
    #[serde(default)]
    pub keeps_stealth: bool,
    /// Landing starts the caster's auto-attack on the target.
    #[serde(default)]
    pub attack: bool,
    /// An enemy's big move: shown on the ground before it lands.
    #[serde(default)]
    pub telegraph: Option<TelegraphDef>,
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

impl AbilityDef {
    /// Seconds from beginning to going off: a telegraph's windup, else its cast time.
    pub fn windup(&self) -> f32 {
        self.telegraph.map_or(self.cast, |t| t.windup)
    }

    /// Whether any of its effects is one `pick` picks.
    pub fn has(&self, pick: impl Fn(&Effect) -> bool) -> bool {
        self.effects.iter().any(pick)
    }

    /// Why it cannot work, if it cannot.
    pub(crate) fn check(&self) -> Result<(), String> {
        let k = &self.key;
        let numbers = [
            self.range,
            self.radius,
            self.cost,
            self.cast,
            self.channel,
            self.cooldown,
            self.gcd,
        ];
        if numbers.iter().any(|n| !(n.is_finite() && *n >= 0.0)) {
            return Err(format!(
                "\"{k}\": range, radius, cost, cast, channel, cooldown and gcd are 0 or more"
            ));
        }
        if matches!(self.aim, Aim::Enemy | Aim::Ground) && self.range <= 0.0 {
            return Err(format!(
                "\"{k}\": an ability aimed at an enemy or the ground needs a range"
            ));
        }
        if self.channel > 0.0 && (self.cast > 0.0 || self.ticks == 0) {
            return Err(format!("\"{k}\": a channel has ticks and no cast time"));
        }
        if let Some(t) = &self.telegraph {
            let sized = match t.shape {
                Shape::Circle { radius } => radius > 0.0,
                Shape::Cone { length, angle } => length > 0.0 && angle > 0.0 && angle <= 360.0,
            };
            if !(t.windup > 0.0 && sized) {
                return Err(format!("\"{k}\": a telegraph needs a windup and a size"));
            }
        }
        for e in &self.effects {
            let works = match *e {
                Effect::Damage { min, max, per_combo } => {
                    min >= 0.0 && max >= min && per_combo >= 0.0
                }
                Effect::Weapon { percent, bonus } => percent >= 0.0 && bonus >= 0.0,
                Effect::Dot { amount, every, secs } => {
                    amount >= 0.0 && every > 0.0 && secs >= every
                }
                Effect::Slow { by, secs } => by > 0.0 && by < 1.0 && secs > 0.0,
                Effect::Control { secs, .. } => secs > 0.0,
                Effect::Combo(n) => (1..=5).contains(&n),
                Effect::Sprint { by, secs } => by > 0.0 && secs > 0.0,
                Effect::Blink { distance } => distance > 0.0,
                Effect::Distract { secs } | Effect::Eat { secs } => secs > 0.0,
                Effect::ConjureWater { count } => count > 0,
                Effect::Stealth | Effect::Vanish => true,
            };
            if !works {
                return Err(format!("\"{k}\": {e:?} cannot work"));
            }
        }
        Ok(())
    }
}
```

In `crates/nightfall-sim/src/content.rs`:

- Add `use crate::{AbilityDef, Creature};` beside the other imports.
- Add above `ClassDef`:

```rust
/// A weapon's or a claw's blow: damage from `min` to `max`, one every `every` seconds.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Swing {
    pub min: f32,
    pub max: f32,
    pub every: f32,
}

impl Swing {
    fn works(&self) -> bool {
        self.min >= 0.0 && self.max >= self.min && self.every > 0.0
    }
}
```

- `ClassDef` gains, after `speed`:

```rust
    /// Mana for a mage, energy (100) for a rogue, at its fullest (spec §6).
    #[serde(default)]
    pub power_max: f32,
    /// Power back a second, in combat or not: a rogue's energy. Mana comes back by drinking.
    #[serde(default)]
    pub power_regen: f32,
    /// The weapon's blow, for auto-attack and weapon abilities; a class without one does not
    /// melee.
    #[serde(default)]
    pub swing: Option<Swing>,
    /// The action bar (spec §6.1): WoW action ids (1 to 12 the main bar, 73 to 84 the bar a
    /// stealthed rogue sees) and the ability on each, by key.
    #[serde(default)]
    pub bar: Vec<(u8, String)>,
    /// `bar` with each ability as its index into [`Content::abilities`], filled as content loads.
    #[serde(skip)]
    pub kit: Vec<(u8, usize)>,
```

- `EnemyDef` gains, after `run_speed`:

```rust
    #[serde(default)]
    pub creature: Creature,
    /// Guards the best loot and uses telegraphs (spec §7.1); shown with WoW's elite dragon.
    #[serde(default)]
    pub elite: bool,
    /// Its melee blow; without one it does not melee.
    #[serde(default)]
    pub swing: Option<Swing>,
    /// Yards it strikes from in melee: a little past the senses' `chase_reach`, where it stops.
    #[serde(default = "melee_reach")]
    pub reach: f32,
    /// The abilities it uses in a fight, each at most every so many seconds.
    #[serde(default)]
    pub moves: Vec<MoveDef>,
    /// `moves` as (ability index, seconds), filled as content loads.
    #[serde(skip)]
    pub kit: Vec<(usize, f32)>,
```

- Add after `EnemyDef`:

```rust
fn melee_reach() -> f32 {
    2.5
}

/// An ability an enemy uses, and how often at most.
#[derive(Clone, Debug, Deserialize)]
pub struct MoveDef {
    pub ability: String,
    pub every: f32,
}

/// The dodge (spec §6.1).
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct DodgeRules {
    pub charges: u8,
    /// Seconds for each charge to come back, one at a time.
    pub recharge: f32,
    /// Seconds after a dodge in which telegraphed blows miss.
    pub immune: f32,
    /// Yards the hop goes (the client's own).
    pub distance: f32,
}

impl Default for DodgeRules {
    fn default() -> Self {
        Self {
            charges: 2,
            recharge: 8.0,
            immune: 0.3,
            distance: 6.0,
        }
    }
}

/// The fight's rules (spec §6, §8.1, §11).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct CombatRules {
    /// Speed while casting a `Slowed` ability (§11's `cast_move_speed`).
    pub cast_move_speed: f32,
    /// Yards a cast may drift (a nudge, a slope) before moving breaks it.
    pub cast_slack: f32,
    /// Seconds a slowed cast allows full speed while the slow reaches the client.
    pub speed_grace: f32,
    /// Yards added to every range: the target's size.
    pub reach: f32,
    /// Degrees either side of straight ahead that count as in front (WoW's 180° arc).
    pub front: f32,
    pub crit_chance: f32,
    pub crit_times: f32,
    pub dodge: DodgeRules,
    /// Seconds a hero stays in combat after the last blow they dealt or took.
    pub combat_linger: f32,
    /// Health and mana back a second out of combat; eating and drinking do the rest.
    pub regen: (f32, f32),
    /// Seconds a downed hero has before they die (§11's `downed_for`).
    pub downed_for: f32,
    /// Seconds a teammate stands by a downed hero to get them up.
    pub revive_for: f32,
    /// Yards that teammate stands within.
    pub revive_reach: f32,
    /// The share of their health a revived hero gets up with.
    pub revive_health: f32,
    /// The share of their speed a downed hero crawls at.
    pub crawl_speed: f32,
    /// Yards a second a sheep wanders.
    pub sheep_speed: f32,
    pub combo_max: u8,
    /// Conjured drinks a hero may carry.
    pub max_drinks: u8,
}

impl Default for CombatRules {
    fn default() -> Self {
        Self {
            cast_move_speed: 0.5,
            cast_slack: 0.5,
            speed_grace: 0.3,
            reach: 1.5,
            front: 90.0,
            crit_chance: 0.05,
            crit_times: 1.5,
            dodge: DodgeRules::default(),
            combat_linger: 5.0,
            regen: (0.0, 0.0),
            downed_for: 45.0,
            revive_for: 5.0,
            revive_reach: 3.0,
            revive_health: 0.3,
            crawl_speed: 0.3,
            sheep_speed: 1.0,
            combo_max: 5,
            max_drinks: 20,
        }
    }
}

impl CombatRules {
    fn check(&self) -> Result<(), String> {
        let d = &self.dodge;
        let fine = self.cast_move_speed > 0.0
            && self.cast_move_speed <= 1.0
            && self.cast_slack >= 0.0
            && self.speed_grace >= 0.0
            && self.reach >= 0.0
            && self.front > 0.0
            && self.front <= 180.0
            && (0.0..=1.0).contains(&self.crit_chance)
            && self.crit_times >= 1.0
            && d.charges >= 1
            && d.recharge > 0.0
            && d.immune >= 0.0
            && d.distance > 0.0
            && self.combat_linger >= 0.0
            && self.regen.0 >= 0.0
            && self.regen.1 >= 0.0
            && self.downed_for > 0.0
            && self.revive_for > 0.0
            && self.revive_reach > 0.0
            && self.revive_health > 0.0
            && self.revive_health <= 1.0
            && (0.0..=1.0).contains(&self.crawl_speed)
            && self.sheep_speed >= 0.0
            && (1..=5).contains(&self.combo_max);
        if fine {
            Ok(())
        } else {
            Err("rules.ron: combat's numbers cannot work (see CombatRules)".into())
        }
    }
}
```

- `Rules` gains, after `senses`:

```rust
    /// The fight (spec §6); its defaults when `rules.ron` leaves it out.
    #[serde(default)]
    pub combat: CombatRules,
```

- `ContentSources` gains `pub abilities: &'a str,` after `spawns`, and `Content` gains `pub abilities: Vec<AbilityDef>,` after `spawns`.
- In `Content::from_ron`: `let classes: Vec<ClassDef>` becomes `let mut classes: Vec<ClassDef>` and `let enemies: Vec<EnemyDef>` becomes `let mut enemies: Vec<EnemyDef>`; after the `spawns` parse add `let abilities: Vec<AbilityDef> = parse("abilities.ron", src.abilities)?;`; and just before `Ok(Self {` add:

```rust
        for (i, a) in abilities.iter().enumerate() {
            if abilities[..i].iter().any(|b| b.key == a.key) {
                return invalid(format!("ability \"{}\" is listed twice", a.key));
            }
            a.check()
                .map_err(|why| ContentError::Invalid(format!("abilities.ron: {why}")))?;
        }
        let ability = |key: &str| abilities.iter().position(|a| a.key == key);
        for c in &mut classes {
            if !(c.power_max >= 0.0 && c.power_regen >= 0.0)
                || c.swing.is_some_and(|s| !s.works())
            {
                return invalid(format!(
                    "classes.ron: \"{}\" needs power_max and power_regen of 0 or more and a swing that works",
                    c.key
                ));
            }
            for (i, (slot, key)) in c.bar.iter().enumerate() {
                if !(1..=120).contains(slot) || c.bar[..i].iter().any(|(s, _)| s == slot) {
                    return invalid(format!(
                        "classes.ron: \"{}\"'s bar slot {slot} is not 1 to 120, or is used twice",
                        c.key
                    ));
                }
                let Some(a) = ability(key) else {
                    return invalid(format!(
                        "classes.ron: \"{}\"'s bar names no ability \"{key}\"",
                        c.key
                    ));
                };
                c.kit.push((*slot, a));
            }
        }
        for e in &mut enemies {
            if !(e.reach > 0.0) || e.swing.is_some_and(|s| !s.works()) {
                return invalid(format!(
                    "enemies.ron: \"{}\" needs a reach above 0 and a swing that works",
                    e.key
                ));
            }
            for m in &e.moves {
                let Some(a) = ability(&m.ability) else {
                    return invalid(format!(
                        "enemies.ron: \"{}\" moves with no ability \"{}\"",
                        e.key, m.ability
                    ));
                };
                if !(m.every > 0.0) {
                    return invalid(format!(
                        "enemies.ron: \"{}\"'s \"{}\" needs an every above 0",
                        e.key, m.ability
                    ));
                }
                e.kit.push((a, m.every));
            }
        }
        rules.combat.check().map_err(ContentError::Invalid)?;
```

and add `abilities,` to the `Self { … }` literal.

- Add to `impl Content`:

```rust
    /// The ability with this key, by index.
    pub fn ability(&self, key: &str) -> Option<usize> {
        self.abilities.iter().position(|a| a.key == key)
    }
```

- In the tests' `load_all`, the `ContentSources { … }` literal gains `abilities: "[]",`, and the test module adds `use crate::{Creature, Moving, Sound};`.

In `crates/nightfall-sim/src/lib.rs` add `mod abilities;` beside the other modules and:

```rust
pub use abilities::{
    AbilityDef, Aim, ControlKind, Creature, Effect, Facing, Moving, Needs, Shape, Sound, Tag,
    TelegraphDef,
};
```

and add `CombatRules, DodgeRules, MoveDef, Swing` to the `pub use content::{ … }` list.

Every other `ContentSources { … }` literal gains a line after its `spawns`: find them with `grep -rn "ContentSources {" crates/nightfall-sim/src`. The test fixtures in `world.rs`, `enemies.rs`, `sight.rs` and `senses/tests.rs` gain `abilities: "[]",`; `messages.rs`'s `a_full_partys_state_is_small`, which reads the shipped files, gains `abilities: include_str!("../../../content/nightfall/abilities.ron"),`.

- [ ] **Step 4: Write the shipped content**

`content/nightfall/abilities.ron`:

```ron
// Nightfall's abilities (spec §6): the Mage's and the Rogue's kits, the moves every hero has, and
// the moves enemies use. Heroes' bars (classes.ron) and enemies' moves (enemies.ron) name them by
// key, so anyone can be given any of them.
//
// Numbers start from vanilla's rank at level 30, and `spell` is that rank's Spell.dbc id, so the
// button's icon, WoW's own tooltip and the cast bar's name fit; where the spec says otherwise
// (Frostbolt's 2.5 s and Fireball's 3 s casts, Polymorph's 20 s and Sap's 30 s, Sprint's 8 s) the
// spec wins. `range` is yards to the target's centre (its size is added in rules.ron); `cast`,
// `channel`, `cooldown` and `gcd` are seconds; `cost` is mana or energy. `moving` is what the
// caster may do meanwhile: Free, Slowed (at rules.ron's cast_move_speed) or Rooted. `sound` is
// Silent, Quiet (8 yd), Normal (20 yd) or Loud (40 yd), heard where the ability goes off (a spot,
// for one aimed at the ground). `tags` are what an AI may use an ability for.
[
    // --- The Mage (spec §6.2) ---
    (key: "frostbolt", name: "Frostbolt", spell: 8406, aim: Enemy, range: 30.0, cost: 100.0,
     cast: 2.5, gcd: 1.5, moving: Slowed, sound: Normal, facing: Front,
     effects: [Damage(min: 126.0, max: 138.0), Slow(by: 0.4, secs: 7.0)], tags: [Nuke, Control]),
    (key: "fireball", name: "Fireball", spell: 8401, aim: Enemy, range: 35.0, cost: 185.0,
     cast: 3.0, gcd: 1.5, moving: Rooted, sound: Loud, flash: true, facing: Front,
     effects: [Damage(min: 199.0, max: 265.0), Dot(amount: 7.0, every: 2.0, secs: 8.0)],
     tags: [Nuke]),
    (key: "fire_blast", name: "Fire Blast", spell: 8412, aim: Enemy, range: 20.0, cost: 165.0,
     cooldown: 8.0, gcd: 1.5, sound: Loud, flash: true, facing: Front,
     effects: [Damage(min: 168.0, max: 202.0)], tags: [Nuke]),
    (key: "frost_nova", name: "Frost Nova", spell: 865, aim: Me, radius: 10.0, cost: 85.0,
     cooldown: 25.0, gcd: 1.5, sound: Normal,
     effects: [Damage(min: 33.0, max: 37.0), Control(kind: Root, secs: 8.0)],
     tags: [Control, Escape]),
    (key: "arcane_missiles", name: "Arcane Missiles", spell: 5145, tick_spell: Some(7270),
     aim: Enemy, range: 30.0, cost: 235.0, channel: 5.0, ticks: 5, gcd: 1.5, moving: Rooted,
     sound: Normal, facing: Front, effects: [Damage(min: 56.0, max: 56.0)], tags: [Nuke]),
    (key: "polymorph", name: "Polymorph", spell: 12824, aim: Enemy, range: 30.0, cost: 90.0,
     cast: 1.5, gcd: 1.5, moving: Slowed, sound: Quiet, facing: Front,
     needs: (creatures: [Humanoid, Beast]), effects: [Control(kind: Polymorph, secs: 20.0)],
     tags: [Control]),
    // 35% of a level-30 mage's base mana.
    (key: "blink", name: "Blink", spell: 1953, aim: Me, cost: 160.0, cooldown: 15.0, gcd: 1.5,
     sound: Quiet, effects: [Blink(distance: 20.0)], tags: [Escape]),
    (key: "conjure_water", name: "Conjure Water", spell: 6127, aim: Me, cost: 285.0, cast: 3.0,
     gcd: 1.5, moving: Rooted, sound: Quiet, needs: (out_of_combat: true),
     effects: [ConjureWater(count: 2)], tags: [Utility]),

    // --- The Rogue (spec §6.3) ---
    (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0, sound: Silent,
     keeps_stealth: true, needs: (out_of_combat: true), effects: [Stealth], tags: [Utility]),
    (key: "sap", name: "Sap", spell: 2070, aim: Enemy, range: 5.0, cost: 65.0, gcd: 1.0,
     sound: Quiet, alerts: false, keeps_stealth: true,
     needs: (stealth: true, unaware: true, creatures: [Humanoid]),
     effects: [Control(kind: Sap, secs: 30.0)], tags: [Opener, Control]),
    (key: "cheap_shot", name: "Cheap Shot", spell: 1833, aim: Enemy, range: 5.0, cost: 60.0,
     gcd: 1.0, sound: Quiet, attack: true, needs: (stealth: true, unaware: true),
     effects: [Control(kind: Stun, secs: 4.0), Combo(2)], tags: [Opener, Control]),
    (key: "garrote", name: "Garrote", spell: 8632, aim: Enemy, range: 5.0, cost: 50.0, gcd: 1.0,
     sound: Silent, facing: Behind, attack: true, needs: (stealth: true, unaware: true),
     effects: [Dot(amount: 47.0, every: 3.0, secs: 18.0), Combo(1)], tags: [Opener]),
    (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     sound: Quiet, facing: Behind, attack: true, needs: (stealth: true, unaware: true),
     effects: [Weapon(percent: 250.0, bonus: 40.0), Combo(1)], tags: [Opener, Nuke]),
    (key: "sinister_strike", name: "Sinister Strike", spell: 1760, aim: Enemy, range: 5.0,
     cost: 45.0, gcd: 1.0, sound: Normal, facing: Front, attack: true,
     effects: [Weapon(bonus: 22.0), Combo(1)], tags: [Builder]),
    (key: "eviscerate", name: "Eviscerate", spell: 6762, aim: Enemy, range: 5.0, cost: 35.0,
     gcd: 1.0, sound: Normal, facing: Front, attack: true, needs: (combo: true),
     effects: [Damage(min: 10.0, max: 30.0, per_combo: 31.0)], tags: [Finisher]),
    (key: "gouge", name: "Gouge", spell: 1777, aim: Enemy, range: 5.0, cost: 45.0,
     cooldown: 10.0, gcd: 1.0, sound: Normal, facing: Front,
     effects: [Damage(min: 20.0, max: 20.0), Control(kind: Incapacitate, secs: 4.0), Combo(1)],
     tags: [Control]),
    (key: "sprint", name: "Sprint", spell: 2983, aim: Me, cooldown: 300.0, sound: Silent,
     keeps_stealth: true, effects: [Sprint(by: 0.5, secs: 8.0)], tags: [Escape]),
    (key: "distract", name: "Distract", spell: 1725, aim: Ground, range: 30.0, radius: 10.0,
     cost: 30.0, cooldown: 30.0, gcd: 1.0, sound: Loud, alerts: false, keeps_stealth: true,
     effects: [Distract(secs: 10.0)], tags: [Utility]),
    (key: "vanish", name: "Vanish", spell: 1856, aim: Me, cooldown: 300.0, sound: Quiet,
     keeps_stealth: true, effects: [Vanish], tags: [Escape]),

    // --- Every hero (spec §6.1) ---
    (key: "throw_stone", name: "Throw a Stone", spell: 4164, aim: Ground, range: 25.0,
     cooldown: 20.0, sound: Normal, alerts: false, keeps_stealth: true, effects: [],
     tags: [Utility]),
    (key: "eat_and_drink", name: "Eat and Drink", spell: 2639, aim: Me, sound: Quiet,
     needs: (out_of_combat: true, still: true), effects: [Eat(secs: 15.0)], tags: [Heal]),

    // --- Enemies (spec §7.1) ---
    (key: "shadow_bolt", name: "Shadow Bolt", spell: 1106, aim: Enemy, range: 30.0, cast: 2.5,
     sound: Normal, facing: Front, effects: [Damage(min: 75.0, max: 95.0)], tags: [Nuke]),
    (key: "rending_cleave", name: "Rending Cleave", spell: 845, aim: Enemy, range: 8.0,
     sound: Loud, telegraph: Some((shape: Cone(length: 8.0, angle: 120.0), windup: 1.2)),
     effects: [Damage(min: 190.0, max: 230.0)], tags: [Nuke]),
    (key: "war_stomp", name: "War Stomp", spell: 45, aim: Me, radius: 8.0, sound: Loud,
     telegraph: Some((shape: Circle(radius: 8.0), windup: 1.4, on_self: true)),
     effects: [Damage(min: 150.0, max: 180.0)], tags: [Control]),
]
```

`content/nightfall/classes.ron` (replaced whole; the Rogue keeps sub-project 3's `stealth: true`):

```ron
// Nightfall's classes. `race` and `sex` are the body every hero of the class wears for now
// (ChrRaces ids: 1 human; sex 0 male, 1 female). `weapons` are the virtual item slots (slot 0 main
// hand, 1 off hand, 2 ranged), the same looks the survivors classes use. Heroes are level 30
// (rules.ron); `base_hp`, `power_max` (mana, or the rogue's 100 energy) and `swing` (the weapon's
// blow: damage from min to max, every so many seconds) are a level-30 hero's in modest gear.
// `bar` puts abilities (abilities.ron) on WoW's action bar: action ids 1 to 12 are the main bar
// (keys 1 to =), 73 to 84 the bar a stealthed rogue sees instead.
[
    (
        key: "rogue",
        name: "Rogue",
        description: "A blade in the dark: slips past what it cannot kill, and kills what it reaches unseen.",
        color: 0xFFF569,
        class_id: 4,
        power: 3,
        race: 1,
        sex: 0,
        weapons: [
            (slot: 0, display: 7492, class: 2, subclass: 7, material: 1, inventory_type: 13, sheath: 3),
            (slot: 1, display: 6443, class: 2, subclass: 15, material: 1, inventory_type: 13, sheath: 3),
        ],
        sheath: 1,
        base_hp: 1050.0,
        speed: 7.0,
        stealth: true,
        power_max: 100.0,
        // WoW's 20 energy every 2 seconds.
        power_regen: 10.0,
        swing: Some((min: 26.0, max: 46.0, every: 1.8)),
        bar: [
            (1, "stealth"), (2, "sinister_strike"), (3, "eviscerate"), (4, "gouge"),
            (5, "sprint"), (6, "distract"), (7, "vanish"), (11, "throw_stone"),
            (12, "eat_and_drink"),
            (73, "stealth"), (74, "ambush"), (75, "garrote"), (76, "cheap_shot"), (77, "sap"),
            (78, "sprint"), (79, "distract"), (80, "vanish"), (83, "throw_stone"),
            (84, "eat_and_drink"),
        ],
    ),
    (
        key: "mage",
        name: "Mage",
        description: "A scholar of frost and fire: deadly at range, and loud about it.",
        color: 0x69CCF0,
        class_id: 8,
        power: 0,
        race: 1,
        sex: 1,
        weapons: [
            (slot: 0, display: 1927, class: 2, subclass: 10, material: 2, inventory_type: 17, sheath: 2),
        ],
        sheath: 1,
        base_hp: 880.0,
        speed: 7.0,
        power_max: 1300.0,
        bar: [
            (1, "frostbolt"), (2, "fireball"), (3, "fire_blast"), (4, "frost_nova"),
            (5, "arcane_missiles"), (6, "polymorph"), (7, "blink"), (8, "conjure_water"),
            (11, "throw_stone"), (12, "eat_and_drink"),
        ],
    ),
]
```

`content/nightfall/enemies.ron` (replaced whole):

```ron
// Nightfall's enemy kinds: Duskwood's own creatures (spec §7.1). `display` is the
// CreatureDisplayInfo id, the same models the survivors content uses for them. Speeds are yards a
// second (WoW's creatures walk at about 2.5 and run at 7). `creature` is Humanoid (the default),
// Beast or Undead, for Sap and Polymorph. `swing` is its melee blow (damage from min to max, every
// so many seconds), struck from `reach` yards (2.5 when left out). `moves` are abilities from
// abilities.ron it uses in a fight, each at most every so many seconds: a caster's bolt, an
// elite's telegraphed blow (spec §6.1). `elite` marks the rares that guard the best loot.
[
    (key: "skeletal_warrior", name: "Skeletal Warrior", display: 200, scale: 1.2, level: 28,
     hp: 980.0, walk_speed: 2.5, run_speed: 7.0, creature: Undead,
     swing: Some((min: 38.0, max: 52.0, every: 2.0))),
    (key: "nightbane_worgen", name: "Nightbane Worgen", display: 657, level: 27,
     hp: 940.0, walk_speed: 2.5, run_speed: 8.0,
     swing: Some((min: 36.0, max: 50.0, every: 2.0))),
    (key: "nightbane_shadow_weaver", name: "Nightbane Shadow Weaver", display: 203, level: 28,
     hp: 900.0, walk_speed: 2.5, run_speed: 7.5,
     swing: Some((min: 22.0, max: 30.0, every: 2.0)),
     moves: [(ability: "shadow_bolt", every: 7.0)]),
    (key: "venom_web_spider", name: "Venom Web Spider", display: 955, level: 26,
     hp: 860.0, walk_speed: 2.2, run_speed: 7.0, creature: Beast,
     swing: Some((min: 32.0, max: 44.0, every: 1.8))),
    (key: "black_ravager", name: "Black Ravager", display: 741, level: 25,
     hp: 820.0, walk_speed: 3.0, run_speed: 8.0, creature: Beast,
     swing: Some((min: 30.0, max: 42.0, every: 1.6))),
    (key: "fetid_corpse", name: "Fetid Corpse", display: 559, scale: 1.3, level: 30,
     hp: 1100.0, walk_speed: 1.8, run_speed: 5.0, creature: Undead,
     swing: Some((min: 45.0, max: 60.0, every: 2.4))),
    // Duskwood's rares: elites with a telegraphed big move.
    (key: "fenros", name: "Fenros", display: 657, scale: 1.35, level: 32, hp: 2900.0,
     walk_speed: 2.5, run_speed: 8.0, elite: true,
     swing: Some((min: 70.0, max: 95.0, every: 2.2)),
     moves: [(ability: "rending_cleave", every: 9.0)]),
    (key: "naraxis", name: "Naraxis", display: 955, scale: 1.6, level: 30, hp: 3100.0,
     walk_speed: 2.2, run_speed: 7.0, creature: Beast, elite: true,
     swing: Some((min: 60.0, max: 85.0, every: 2.0)),
     moves: [(ability: "war_stomp", every: 11.0)]),
]
```

In `content/nightfall/spawns.ron`, before the closing `]`:

```ron
    // measure: a shadow weaver at Raven Hill's chapel door: a caster that fights from range.
    (place: "Near the entry: a caster", kind: "nightbane_shadow_weaver",
     behaviour: Sentry(at: (-10010.0, -830.0), facing: 0.0)),
    // measure: Fenros's den by the Brightwood road.
    (place: "Near the entry: Fenros", kind: "fenros",
     behaviour: Sentry(at: (-10100.0, -840.0), facing: 45.0)),
    // measure: Naraxis's lair in Twilight Grove.
    (place: "Near the entry: Naraxis", kind: "naraxis",
     behaviour: Wander(center: (-10110.0, -760.0), radius: 8.0)),
```

In `content/nightfall/rules.ron`, after the `senses: ( … ),` block and before the closing `)`:

```ron
    // The fight (spec §6, §8.1). A slowed cast moves at cast_move_speed, after speed_grace seconds
    // at full speed (while the slow reaches a joined client); a cast breaks past cast_slack yards
    // beyond that. Every range has `reach` yards (the target's size) added; `front` is degrees
    // either side of straight ahead. The dodge: charges, seconds each takes to come back, seconds
    // in which telegraphed blows miss, yards the hop goes. A hero is in combat while an enemy is
    // after them and combat_linger seconds after a blow; out of it, `regen` gives health and mana a
    // second (eating and drinking do the rest). Falling: downed_for seconds to be revived by a
    // teammate within revive_reach for revive_for seconds, getting up with revive_health of their
    // health; downed heroes crawl at crawl_speed.
    combat: (
        cast_move_speed: 0.5,
        cast_slack: 0.5,
        speed_grace: 0.3,
        reach: 1.5,
        front: 90.0,
        crit_chance: 0.05,
        crit_times: 1.5,
        dodge: (charges: 2, recharge: 8.0, immune: 0.3, distance: 6.0),
        combat_linger: 5.0,
        regen: (0.0, 0.0),
        downed_for: 45.0,
        revive_for: 5.0,
        revive_reach: 3.0,
        revive_health: 0.3,
        crawl_speed: 0.3,
        sheep_speed: 1.0,
        combo_max: 5,
        max_drinks: 20,
    ),
```

- [ ] **Step 5: Keep the client reading every file**

In `crates/benilla-app/src/nightfall/mod.rs`, `CONTENT_TEXTS` becomes `OnceLock<[String; 6]>` and `content_texts()` returns `&'static [String; 6]` (its doc names "classes, map, rules, enemies, spawns, abilities"); in `prepare`, after the `spawns` read add:

```rust
    let abilities = read(
        "abilities.ron",
        include_str!("../../../../content/nightfall/abilities.ron"),
    )?;
```

the `ContentSources { … }` literal gains `abilities: &abilities,`, and the `CONTENT_TEXTS.set(…)` line becomes `let _ = CONTENT_TEXTS.set([classes, map, rules, enemies, spawns, abilities]);`. `session::our_content_hash` already hashes every text, so a client with other abilities is refused.

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test; 3 in `abilities`, 4 new in `content`, and `the_shipped_kits_are_on_their_bars_and_enemies_fight_back`).
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 7: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add content/nightfall crates/nightfall-sim crates/benilla-app/src/nightfall/mod.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall content: abilities, the Rogue's and Mage's bars, enemies' blows and moves, combat rules

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The fight's arithmetic: dice, arcs, ranges, the casting allowance, dodge charges

**Files:**
- Create: `crates/nightfall-sim/src/combat/mod.rs` (a first, small version Task 3 replaces), `crates/nightfall-sim/src/combat/rules.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `CombatRules`, `DodgeRules`, `Facing`, `Moving` (Task 1); `V2`.
- Produces:
  - `nightfall_sim::Charges { have: u8, next_in: f32 }` (`Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize`) with `Charges::full(n: u8) -> Charges`, crate-only `spend(&mut self, &DodgeRules) -> bool`, `tick(&mut self, dt: f32, &DodgeRules)`.
  - Crate-only: `Dice` with `new(u64)`, `unit(&mut self) -> f32`, `between(&mut self, min: f32, max: f32) -> f32`, `chance(&mut self, p: f32) -> bool`; `within_arc(facing: V2, to: V2, half: f32) -> bool`; `stands_right(want: Facing, from: V2, facing: V2, at: V2, at_facing: V2, front: f32) -> bool`; `in_range(distance: f32, range: f32, &CombatRules) -> bool`; `allowance(Moving, run: f32, elapsed: f32, &CombatRules) -> f32`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/combat/rules.rs` (Step 3 adds the code above the tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dice_are_the_same_every_run_and_stay_in_bounds() {
        let roll = || {
            let mut d = Dice::new(7);
            (0..50).map(|_| d.between(10.0, 20.0)).collect::<Vec<_>>()
        };
        let a = roll();
        assert_eq!(a, roll());
        assert!(a.iter().all(|x| (10.0..20.0).contains(x)));
        let mut d = Dice::new(7);
        assert_eq!(d.between(5.0, 5.0), 5.0);
        assert!(!(0..100).any(|_| d.chance(0.0)));
        assert!((0..100).all(|_| d.chance(1.0)));
    }

    #[test]
    fn in_front_is_the_half_circle_ahead() {
        let north = V2::new(1.0, 0.0);
        assert!(within_arc(north, V2::new(3.0, 2.9), 90.0));
        assert!(within_arc(north, V2::new(0.0, 5.0), 90.0));
        assert!(!within_arc(north, V2::new(-0.5, 5.0), 90.0));
        assert!(within_arc(north, V2::ZERO, 90.0));
        let r = CombatRules::default();
        let at = V2::new(5.0, 0.0);
        assert!(stands_right(Facing::Front, V2::ZERO, north, at, north, r.front));
        assert!(!stands_right(Facing::Front, V2::ZERO, north * -1.0, at, north, r.front));
        assert!(stands_right(Facing::Any, V2::ZERO, north * -1.0, at, north, r.front));
    }

    #[test]
    fn behind_needs_the_targets_back_and_the_attacker_facing_it() {
        let north = V2::new(1.0, 0.0);
        let target = V2::new(5.0, 0.0);
        // The target faces north and we stand south of it, facing it: behind.
        assert!(stands_right(Facing::Behind, V2::ZERO, north, target, north, 90.0));
        // The target turned to face us: in front of it.
        assert!(!stands_right(Facing::Behind, V2::ZERO, north, target, north * -1.0, 90.0));
        // Behind it but looking away.
        assert!(!stands_right(Facing::Behind, V2::ZERO, north * -1.0, target, north, 90.0));
    }

    #[test]
    fn range_counts_the_targets_size() {
        let r = CombatRules::default();
        assert!(in_range(6.4, 5.0, &r));
        assert!(!in_range(6.6, 5.0, &r));
    }

    #[test]
    fn a_rooted_cast_allows_a_nudge_a_slowed_one_the_slowed_pace_after_a_grace() {
        let r = CombatRules::default();
        assert_eq!(allowance(Moving::Free, 7.0, 1.0, &r), f32::INFINITY);
        assert_eq!(allowance(Moving::Rooted, 7.0, 1.0, &r), 0.5);
        // 0.2 s in: still within the grace, at full speed.
        assert!((allowance(Moving::Slowed, 7.0, 0.2, &r) - (0.5 + 1.4)).abs() < 1e-4);
        // 1.3 s in: 0.3 s at full speed, then a second at half.
        assert!((allowance(Moving::Slowed, 7.0, 1.3, &r) - (0.5 + 2.1 + 3.5)).abs() < 1e-4);
    }

    #[test]
    fn charges_come_back_one_at_a_time() {
        let r = DodgeRules::default();
        let mut c = Charges::full(r.charges);
        assert!(c.spend(&r));
        assert!(c.spend(&r));
        assert!(!c.spend(&r));
        assert_eq!(c.have, 0);
        for _ in 0..79 {
            c.tick(0.1, &r);
        }
        assert_eq!(c.have, 0);
        for _ in 0..2 {
            c.tick(0.1, &r);
        }
        assert_eq!(c.have, 1);
        for _ in 0..81 {
            c.tick(0.1, &r);
        }
        assert_eq!(c.have, 2);
        assert_eq!(c.next_in, 0.0);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

`crates/nightfall-sim/src/combat/mod.rs`:

```rust
//! The fight (spec §6). This first part is its arithmetic ([`rules`]); the rest of the module
//! comes in the next tasks.

mod rules;

pub use rules::Charges;
#[allow(unused_imports)]
pub(crate) use rules::{allowance, in_range, stands_right, within_arc, Dice};
```

In `crates/nightfall-sim/src/lib.rs` add `mod combat;` beside the other modules and `pub use combat::Charges;`.

Run: `cargo test -p nightfall-sim combat::rules`
Expected: FAIL to compile (`cannot find type Dice`, `cannot find function within_arc`).

- [ ] **Step 3: Implement the arithmetic**

At the top of `crates/nightfall-sim/src/combat/rules.rs`:

```rust
//! The fight's arithmetic, pure: dice for damage and crits, which way a unit faces another, how
//! far an ability reaches, how far a caster may move before its cast breaks, and a hero's dodge
//! charges. The rest of [`super`] uses it on the world.

use serde::{Deserialize, Serialize};

use crate::{CombatRules, DodgeRules, Facing, Moving, V2};

/// A small random number generator (xorshift64*): the same fight every time with the same inputs.
#[derive(Clone, Debug)]
pub(crate) struct Dice(u64);

impl Dice {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..1`.
    pub(crate) fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number from `min` up to `max`.
    pub(crate) fn between(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.unit()
    }

    /// Whether a chance of `p` (0 to 1) comes up.
    pub(crate) fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
}

/// Whether the way `to` lies within `half` degrees either side of `facing` (ground directions;
/// a unit on top of another is taken to face it).
pub(crate) fn within_arc(facing: V2, to: V2, half: f32) -> bool {
    let (f, d) = (facing.normalize_or_zero(), to.normalize_or_zero());
    f == V2::ZERO || d == V2::ZERO || f.dot(d) >= half.to_radians().cos() - 1e-5
}

/// Whether an attacker at `from` facing `facing` stands as `want` asks toward a target at `at`
/// facing `at_facing`: in front, the target within `front` degrees of straight ahead; behind,
/// that, and the attacker in the target's rear half (WoW's facing rules).
pub(crate) fn stands_right(
    want: Facing,
    from: V2,
    facing: V2,
    at: V2,
    at_facing: V2,
    front: f32,
) -> bool {
    let ahead = within_arc(facing, at - from, front);
    match want {
        Facing::Any => true,
        Facing::Front => ahead,
        Facing::Behind => {
            ahead && at_facing.normalize_or_zero().dot((from - at).normalize_or_zero()) < 0.0
        }
    }
}

/// Whether a target `distance` yards off is within `range`, its size (`reach`) added.
pub(crate) fn in_range(distance: f32, range: f32, r: &CombatRules) -> bool {
    distance <= range + r.reach
}

/// How far (yards, straight from where it began) a caster running at `run` yards a second may
/// have gone `elapsed` seconds into a cast before it breaks: anywhere for an instant; a nudge for
/// a rooted cast; for a slowed one full speed during `speed_grace` (the slow reaching a joined
/// client), `cast_move_speed` after it, and the nudge.
pub(crate) fn allowance(moving: Moving, run: f32, elapsed: f32, r: &CombatRules) -> f32 {
    match moving {
        Moving::Free => f32::INFINITY,
        Moving::Rooted => r.cast_slack,
        Moving::Slowed => {
            let grace = elapsed.min(r.speed_grace);
            r.cast_slack + run * (grace + (elapsed - grace) * r.cast_move_speed)
        }
    }
}

/// A hero's dodge charges (spec §6.1): one comes back `recharge` seconds after the last came
/// back, or after the first was spent from full.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Charges {
    pub have: u8,
    /// Seconds until the next comes back (0 when full).
    pub next_in: f32,
}

impl Charges {
    /// `n` charges, all there.
    pub fn full(n: u8) -> Self {
        Self {
            have: n,
            next_in: 0.0,
        }
    }

    /// Spend one; whether there was one.
    pub(crate) fn spend(&mut self, r: &DodgeRules) -> bool {
        if self.have == 0 {
            return false;
        }
        if self.have >= r.charges {
            self.next_in = r.recharge;
        }
        self.have -= 1;
        true
    }

    /// `dt` seconds pass.
    pub(crate) fn tick(&mut self, dt: f32, r: &DodgeRules) {
        if self.have >= r.charges {
            self.next_in = 0.0;
            return;
        }
        self.next_in -= dt;
        if self.next_in <= 1e-4 {
            self.have += 1;
            self.next_in = if self.have < r.charges {
                self.next_in + r.recharge
            } else {
                0.0
            };
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim combat::rules`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/combat crates/nightfall-sim/src/lib.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: the fight's arithmetic: dice, arcs, ranges, the casting allowance, dodge charges

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The fight on the wire: each hero's and enemy's fight, telegraphs, events, presses

**Files:**
- Replace: `crates/nightfall-sim/src/combat/mod.rs`
- Modify: `crates/nightfall-sim/src/world.rs`, `crates/nightfall-sim/src/enemies.rs`, `crates/nightfall-sim/src/sight.rs`, `crates/nightfall-sim/src/messages.rs`, `crates/nightfall-sim/src/lib.rs`
- Modify (the wire changed): `crates/benilla-app/src/nightfall/session.rs` (`Peer`, `host_send`, `Heard`, `client_exchange`), `crates/benilla-app/src/nightfall/run.rs` (`Sim`, `LocalInput::with_pose`, `step_sim`, `take_events`), `crates/benilla-app/src/nightfall/mod.rs` (registration)

**Interfaces:**
- Consumes: `Charges`, `Dice` (Task 2); `ClassDef`, `CombatRules`, `ControlKind`, `Shape` (Task 1); `World`, `Hero`, `Enemy`, `Snapshot`, `PlayerInput`, `World::{attach_map, remove_hero, snapshot, snapshot_for, apply_snapshot}`, `Senses::place` (earlier plans); `ServerMsg::State { snapshot, seen, marks }`, `PROTOCOL = 3`, `Heard::State`, `Sim { inbox, seen, marks }`, `LocalInput::with_pose` (senses Tasks 7, 9).
- Produces:
  - `nightfall_sim::{UnitRef, Life, Casting, Cooldown, HeroFight, EnemyFight, Telegraph, Stop, Refusal, CastPress, Event}` (all `Clone, Debug, PartialEq, Serialize, Deserialize`; all but `Event`, `HeroFight` and `EnemyFight` `Copy`); `HeroFight::{new(&ClassDef, &CombatRules) -> HeroFight, up(&self) -> bool}`.
  - `Hero::fight: HeroFight`; `Enemy::fight: EnemyFight`; `Snapshot::telegraphs: Vec<Telegraph>`; `PlayerInput` gains `cast: Option<CastPress>` and `dodge_presses: u8`.
  - `World` gains `pub telegraphs: Vec<Telegraph>`, `pub(crate) events: Vec<Event>`, `pub(crate) combat: Combat`; `World::{events(&self) -> &[Event], drain_events(&mut self) -> Vec<Event>, push_events(&mut self, Vec<Event>)}`; crate-only `World::{hero_slot(&self, PlayerId) -> Option<usize>, enemies_within(&self, V2, f32) -> Vec<usize>}`.
  - Crate-only `Combat { heroes, enemies, dice, next_telegraph }` with `hero_index`, `hero`, `forget`, `place`; `HeroCombat`, `Meal`, `CastState`, `EnemyCombat`, `Held`, `Slowed`, `Burn`, `EnemyCast`, `Hold::{Free, Still, Out, Facing(V2)}`, `Engage { hold, reach, speed }` with `Engage::free(f32)`.
  - `ServerMsg::State { snapshot, seen, marks, events: Vec<Event> }`; `PROTOCOL = 4`.
  - Client: `run::Sim::events: Vec<Event>` (this frame's), system `run::take_events`; `session::host_send(&mut Net, &World, events_from: usize)`; `session::Heard::State(Snapshot, Vec<u32>, Vec<Mark>, Vec<Event>)`.

- [ ] **Step 1: Write the failing tests**

Replace `crates/nightfall-sim/src/combat/mod.rs`'s test-free first version with the file in Step 3, whose tests (a module of their own, `state_tests`, since Task 4 adds `combat/tests.rs` as `tests`) are:

```rust
#[cfg(test)]
mod state_tests {
    use std::sync::Arc;

    use super::*;
    use crate::{ContentSources, HeroPose, PlayerInput};

    fn world() -> World {
        let content = crate::Content::from_ron(ContentSources {
            classes: r#"[(key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1,
                base_hp: 880.0, speed: 7.0, power_max: 1300.0)]"#,
            map: r#"(key: "test", name: "Test", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: "[]",
        })
        .unwrap();
        World::new(Arc::new(content))
    }

    #[test]
    fn a_new_hero_is_up_with_full_power_and_both_dodges() {
        let mut w = world();
        w.add_hero(0, "mage", "Jaina", 1, 1).unwrap();
        let f = &w.hero(0).unwrap().fight;
        assert!(f.up());
        assert_eq!((f.power, f.max_power), (1300.0, 1300.0));
        assert_eq!(f.dodge, Charges::full(2));
        assert!(f.casting.is_none() && f.cooldowns.is_empty());
    }

    #[test]
    fn a_mirror_takes_the_telegraphs_and_the_events() {
        let mut host = world();
        host.add_hero(0, "mage", "Jaina", 1, 1).unwrap();
        host.telegraphs.push(Telegraph {
            id: 1,
            enemy: 9,
            ability: 0,
            shape: Shape::Circle { radius: 8.0 },
            at: V3::new(10.0, 10.0, 0.0),
            facing: V2::new(1.0, 0.0),
            left: 1.0,
            total: 1.4,
        });
        host.events.push(Event::Dodged { player: 0 });
        let mut mirror = world();
        mirror.apply_snapshot(host.snapshot());
        mirror.push_events(host.drain_events());
        assert_eq!(mirror.telegraphs, host.telegraphs);
        assert_eq!(mirror.drain_events(), vec![Event::Dodged { player: 0 }]);
        assert!(host.events().is_empty());
    }

    #[test]
    fn an_input_carries_its_press_and_dodges() {
        let input = PlayerInput {
            pose: Some(HeroPose::default()),
            cast: Some(CastPress {
                seq: 3,
                slot: 2,
                target: Some(UnitRef::Enemy(7)),
                at: None,
            }),
            dodge_presses: 4,
            ..Default::default()
        };
        let bytes = postcard::to_stdvec(&input).unwrap();
        assert_eq!(postcard::from_bytes::<PlayerInput>(&bytes).unwrap(), input);
    }
}
```

In `crates/nightfall-sim/src/messages.rs`'s tests, add:

```rust
    #[test]
    fn a_state_with_events_survives_the_wire() {
        use crate::{Event, Refusal, Snapshot, Stop, UnitRef, V3};
        let msg = ServerMsg::State {
            snapshot: Snapshot {
                elapsed: 1.0,
                heroes: Vec::new(),
                enemies: Vec::new(),
                telegraphs: Vec::new(),
            },
            seen: vec![3],
            marks: Vec::new(),
            events: vec![
                Event::CastGo {
                    by: UnitRef::Hero(0),
                    ability: 2,
                    hits: vec![UnitRef::Enemy(3)],
                    at: Some(V3::new(1.0, 2.0, 3.0)),
                },
                Event::CastStop {
                    by: UnitRef::Enemy(3),
                    ability: 21,
                    why: Stop::TargetLost,
                },
                Event::Refused {
                    player: 1,
                    why: Refusal::OutOfRange,
                },
            ],
        };
        let bytes = postcard::to_stdvec(&msg).unwrap();
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).unwrap(), msg);
    }
```

and in `a_full_partys_state_is_small`, the `ServerMsg::State { … }` literal gains

```rust
            events: (0..12)
                .map(|i| crate::Event::Damage {
                    to: crate::UnitRef::Enemy(i),
                    by: Some(crate::UnitRef::Hero(0)),
                    amount: 130.0,
                    crit: false,
                    ambush: false,
                })
                .collect(),
```

and its bound and comment become:

```rust
        // Four heroes with their senses and their fight, the twenty enemies near one of them, all
        // marked, and a busy step's events, 15 times a second, stay near 35 KB a second.
        assert!(bytes < 2400, "{bytes} bytes");
```

(That test's `Enemy { … }` literal gains `fight: Default::default(),`.)

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile (`cannot find type Telegraph`, `no field telegraphs on type Snapshot`, `struct PlayerInput has no field named cast`).

- [ ] **Step 3: Implement the fight's state**

`crates/nightfall-sim/src/combat/mod.rs` (the tests above at its end):

```rust
//! The fight (spec §6, §5.4, §8.1's falling): heroes' presses and casts, what abilities do where
//! they land, enemies held and hurt over time, heroes falling, revived or dead, and enemies
//! striking back with plain swings and telegraphed blows that a dodge makes miss. The host's: a
//! client's mirror takes each hero's and enemy's fight from the snapshot, the telegraphs beside
//! them, and what happened from the [`Event`]s that travel with it.

mod rules;

pub use rules::Charges;
#[allow(unused_imports)]
pub(crate) use rules::{allowance, in_range, stands_right, within_arc, Dice};

use serde::{Deserialize, Serialize};

use crate::{ClassDef, CombatRules, Content, ControlKind, Enemy, PlayerId, Shape, World, V2, V3};

/// The seed of the fight's dice: every run rolls the same with the same inputs.
const DICE_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// A unit an ability or a blow is aimed at, or comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitRef {
    Hero(PlayerId),
    Enemy(u32),
}

/// How a hero stands (spec §8.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Life {
    #[default]
    Up,
    /// At 0 health and crawling: `left` seconds from death, `revive` seconds into a revive.
    Downed { left: f32, revive: f32 },
    /// The body stays where it fell.
    Dead,
}

/// A cast or channel under way, as every client sees it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Casting {
    /// Index into the content's abilities.
    pub ability: u16,
    pub left: f32,
    pub total: f32,
    pub channel: bool,
    pub target: Option<UnitRef>,
}

/// An ability waiting out its own cooldown.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cooldown {
    pub ability: u16,
    pub left: f32,
}

/// A hero's side of the fight, as every client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroFight {
    pub life: Life,
    /// Mana or energy, as the class has.
    pub power: f32,
    pub max_power: f32,
    /// Combo points, on enemy `combo_on`.
    pub combo: u8,
    pub combo_on: Option<u32>,
    pub casting: Option<Casting>,
    /// Seconds of global cooldown left.
    pub gcd: f32,
    pub cooldowns: Vec<Cooldown>,
    pub dodge: Charges,
    /// In combat (spec §6.1: nothing regenerates): an enemy is after them, or they dealt or took
    /// a blow lately.
    pub in_combat: bool,
    /// The enemy their auto-attack swings at.
    pub attacking: Option<u32>,
    /// Their feet are held: a rooted cast, or dead.
    pub rooted: bool,
    /// Seconds of Sprint left.
    pub sprint: f32,
    /// Seconds of eating and drinking left.
    pub eating: f32,
    /// Conjured drinks carried (spec §6.2).
    pub drinks: u8,
}

impl HeroFight {
    /// A fresh hero of `class`: up, full power, every dodge.
    pub fn new(class: &ClassDef, r: &CombatRules) -> Self {
        Self {
            power: class.power_max,
            max_power: class.power_max,
            dodge: Charges::full(r.dodge.charges),
            ..Self::default()
        }
    }

    /// Up and fighting.
    pub fn up(&self) -> bool {
        self.life == Life::Up
    }
}

/// An enemy's side of the fight, as every client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EnemyFight {
    pub control: Option<ControlKind>,
    pub slowed: bool,
    pub casting: Option<Casting>,
    /// The spells of what is on it (a hold, a slow, a bleed), for the target frame's debuffs.
    pub auras: Vec<u32>,
}

/// A telegraphed blow about to land (spec §6.1): where, which way, and how soon.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Telegraph {
    pub id: u32,
    pub enemy: u32,
    pub ability: u16,
    pub shape: Shape,
    pub at: V3,
    pub facing: V2,
    pub left: f32,
    pub total: f32,
}

/// Why a cast stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    /// A channel ran its course.
    Finished,
    /// Its caster moved further than it allows.
    Moved,
    /// Its target died, or went out of sight.
    TargetLost,
    /// Its caster dodged.
    Cancelled,
    /// Its caster fell, or was held.
    Interrupted,
}

/// Why the host turned a press down: WoW's own refusals, shown in its red error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    Down,
    Busy,
    NotReady,
    NoPower,
    Moving,
    NeedStealth,
    InCombat,
    NeedCombo,
    NoTarget,
    BadTarget,
    Aware,
    OutOfRange,
    Facing,
    Behind,
    NotInSight,
}

/// An ability press: the latest, repeated in every input until the next, so an input replaced
/// before the host steps cannot lose it; `seq` tells a new press from the same one again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CastPress {
    pub seq: u8,
    /// The WoW action id pressed (1 to 120), which the class's bar maps to an ability.
    pub slot: u8,
    pub target: Option<UnitRef>,
    /// The spot, for an ability aimed at the ground.
    pub at: Option<V3>,
}

/// What happened in a step, for every client to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A cast began: the caster's hands glow, its player's cast bar fills.
    CastStart {
        by: UnitRef,
        ability: u16,
        target: Option<UnitRef>,
        secs: f32,
    },
    /// A channel began.
    ChannelStart {
        by: UnitRef,
        ability: u16,
        target: Option<UnitRef>,
        secs: f32,
    },
    /// An ability went off (or a channel ticked): its visual, at what it hit and where.
    CastGo {
        by: UnitRef,
        ability: u16,
        hits: Vec<UnitRef>,
        at: Option<V3>,
    },
    /// A cast stopped without going off, or a channel ended.
    CastStop { by: UnitRef, ability: u16, why: Stop },
    Damage {
        to: UnitRef,
        by: Option<UnitRef>,
        amount: f32,
        crit: bool,
        ambush: bool,
    },
    /// A melee blow's swing (its damage is a `Damage` of its own).
    Swing { by: UnitRef, to: UnitRef },
    /// A telegraphed blow missed a hero who dodged.
    Evaded { player: PlayerId, enemy: u32 },
    /// A press the host turned down: shown to its player.
    Refused { player: PlayerId, why: Refusal },
    Dodged { player: PlayerId },
    /// A Blink's end, which its player's client moves the hero to.
    Blinked { player: PlayerId, to: V3 },
    Downed { player: PlayerId },
    Revived { player: PlayerId, by: PlayerId },
    Died { player: PlayerId },
    EnemyDied { id: u32, by: Option<PlayerId> },
}

/// The fight's host-only state: never sent.
#[derive(Clone, Debug)]
pub(crate) struct Combat {
    pub(crate) heroes: Vec<HeroCombat>,
    /// One per enemy, beside the world's enemies.
    pub(crate) enemies: Vec<EnemyCombat>,
    pub(crate) dice: Dice,
    pub(crate) next_telegraph: u32,
}

impl Default for Combat {
    fn default() -> Self {
        Self {
            heroes: Vec::new(),
            enemies: Vec::new(),
            dice: Dice::new(DICE_SEED),
            next_telegraph: 1,
        }
    }
}

impl Combat {
    /// `player`'s state, made if need be.
    pub(crate) fn hero_index(&mut self, player: PlayerId) -> usize {
        match self.heroes.iter().position(|h| h.player == player) {
            Some(i) => i,
            None => {
                self.heroes.push(HeroCombat {
                    player,
                    ..HeroCombat::default()
                });
                self.heroes.len() - 1
            }
        }
    }

    pub(crate) fn hero(&self, player: PlayerId) -> Option<&HeroCombat> {
        self.heroes.iter().find(|h| h.player == player)
    }

    /// A player left: nothing of their hero's fight stays.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
    }

    /// A fresh fight for each enemy placed: its moves half ready, so none opens with its big
    /// move at once.
    pub(crate) fn place(&mut self, content: &Content, enemies: &[Enemy]) {
        let chase = content.rules.senses.chase_reach;
        self.enemies = enemies
            .iter()
            .map(|e| EnemyCombat {
                moves_in: content.enemies[e.kind]
                    .kit
                    .iter()
                    .map(|&(_, every)| every * 0.5)
                    .collect(),
                engage: Engage::free(chase),
                ..EnemyCombat::default()
            })
            .collect();
    }
}

/// One hero's host-only fight.
#[derive(Clone, Debug, Default)]
pub(crate) struct HeroCombat {
    pub(crate) player: PlayerId,
    /// An input has been seen: the first is a baseline, so a new run casts nothing old.
    pub(crate) started: bool,
    /// The last ability press's `seq`.
    pub(crate) seq: Option<u8>,
    /// The last dodge count.
    pub(crate) dodges: u8,
    /// This step's input moves the hero.
    pub(crate) moving: bool,
    pub(crate) cast: Option<CastState>,
    /// Seconds until auto-attack may swing again.
    pub(crate) swing_in: f32,
    /// Seconds in which telegraphed blows miss them.
    pub(crate) immune: f32,
    /// Seconds left in combat after their last blow dealt or taken.
    pub(crate) combat_for: f32,
    /// In stealth at the last step: Stealth's cooldown starts as stealth ends.
    pub(crate) was_stealthed: bool,
    pub(crate) meal: Option<Meal>,
    /// How much faster Sprint makes them while it lasts.
    pub(crate) sprint_by: f32,
}

/// A meal under way: health and mana back a second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Meal {
    pub(crate) health: f32,
    pub(crate) mana: f32,
}

/// A hero's cast under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CastState {
    pub(crate) ability: usize,
    /// Its enemy, an index into the world's enemies.
    pub(crate) enemy: Option<usize>,
    pub(crate) at: Option<V3>,
    /// Where the hero stood as it began.
    pub(crate) from: V2,
    pub(crate) elapsed: f32,
    /// A channel's ticks so far.
    pub(crate) ticks: u32,
}

/// One enemy's host-only fight.
#[derive(Clone, Debug, Default)]
pub(crate) struct EnemyCombat {
    pub(crate) held: Option<Held>,
    pub(crate) slow: Option<Slowed>,
    pub(crate) burns: Vec<Burn>,
    /// Turned to a Distract's spot, for so many seconds more.
    pub(crate) distracted: Option<(V2, f32)>,
    /// A sheep's wandering: where it was turned, and where it walks to.
    pub(crate) wander: Option<(V2, V2)>,
    /// Seconds until its melee swing.
    pub(crate) swing_in: f32,
    /// Seconds until each of its kind's moves is ready.
    pub(crate) moves_in: Vec<f32>,
    pub(crate) cast: Option<EnemyCast>,
    /// What its senses are told this step.
    pub(crate) engage: Engage,
}

/// A hold on an enemy, and who put it there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Held {
    pub(crate) kind: ControlKind,
    pub(crate) left: f32,
    pub(crate) by: PlayerId,
    pub(crate) spell: u32,
}

/// A slow on an enemy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slowed {
    pub(crate) by: f32,
    pub(crate) left: f32,
    pub(crate) spell: u32,
}

/// A bleed or a burn: `amount` every `every` seconds, the next in `next`, for `left` more.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Burn {
    pub(crate) by: PlayerId,
    pub(crate) amount: f32,
    pub(crate) every: f32,
    pub(crate) next: f32,
    pub(crate) left: f32,
    pub(crate) spell: u32,
    /// Its ticks make no noise (Garrote).
    pub(crate) silent: bool,
}

/// An enemy's cast or telegraphed windup under way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EnemyCast {
    pub(crate) ability: usize,
    pub(crate) player: PlayerId,
    pub(crate) left: f32,
    pub(crate) total: f32,
    pub(crate) telegraph: Option<u32>,
}

/// What an enemy's senses may do this step, from what holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Hold {
    /// Look, think and move as its senses say.
    Free,
    /// Look and think, but stand turned to its hero (rooted, or casting).
    Still,
    /// Out of the fight (stunned, sapped, a sheep, dead): neither look, think nor move.
    Out,
    /// Unless something worries it more, stand turned to this spot (Distract).
    Facing(V2),
}

/// What an enemy's senses are told each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Engage {
    pub(crate) hold: Hold,
    /// Yards short of its hero a chase stops: its melee reach, or a caster's range.
    pub(crate) reach: f32,
    /// Its speed's share (under a slow, less than 1).
    pub(crate) speed: f32,
}

impl Engage {
    /// Free, stopping `reach` yards short, at full speed.
    pub(crate) fn free(reach: f32) -> Self {
        Self {
            hold: Hold::Free,
            reach,
            speed: 1.0,
        }
    }
}

impl Default for Engage {
    fn default() -> Self {
        Self::free(2.0)
    }
}

impl World {
    /// What happened in the steps since the last drain (host) or push (mirror), oldest first: a
    /// host sends each step's slice of them.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The events so far, taken.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// A mirror takes its host's events beside its snapshot.
    pub fn push_events(&mut self, events: Vec<Event>) {
        self.events.extend(events);
    }

    /// `player`'s hero, by index.
    pub(crate) fn hero_slot(&self, player: PlayerId) -> Option<usize> {
        self.heroes.iter().position(|h| h.player == player)
    }

    /// The living enemies within `radius` of `at`, by index.
    pub(crate) fn enemies_within(&self, at: V2, radius: f32) -> Vec<usize> {
        self.enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0.0 && e.pos.distance(at) <= radius)
            .map(|(i, _)| i)
            .collect()
    }
}
```

In `crates/nightfall-sim/src/world.rs`:

- Add `use crate::combat::{CastPress, Combat, Event, HeroFight, Telegraph};`.
- `PlayerInput` gains, after `stealth_presses`:

```rust
    /// The latest ability press, repeated until the next (spec §6.1).
    pub cast: Option<CastPress>,
    /// How many times the player has pressed dodge, counting round past 255.
    pub dodge_presses: u8,
```

- `Hero` gains, after `noise`:

```rust
    /// Their side of the fight (spec §6).
    pub fight: HeroFight,
```

- `Snapshot` gains, after `enemies`:

```rust
    /// The telegraphed blows about to land; a player's own snapshot has those near their hero.
    pub telegraphs: Vec<Telegraph>,
```

- `World` gains, after `senses`:

```rust
    /// The telegraphed blows about to land (spec §6.1), as every client draws them.
    pub telegraphs: Vec<Telegraph>,
    /// What happened in the steps since they were last drained (host) or pushed (mirror).
    pub(crate) events: Vec<Event>,
    /// The fight's host-only state.
    pub(crate) combat: Combat,
```

and `World::new` adds `telegraphs: Vec::new(), events: Vec::new(), combat: Combat::default(),`.
- In `add_hero`'s `Hero { … }` literal, after `noise: 0.0,` add `fight: HeroFight::new(def, &self.content.rules.combat),`.
- `remove_hero` gains `self.combat.forget(player);`.
- In `attach_map`, after the senses' `place` call, add `self.combat.place(&self.content, &self.enemies);`.
- `snapshot()`'s literal gains `telegraphs: self.telegraphs.clone(),`; `apply_snapshot` gains `self.telegraphs = snapshot.telegraphs;`.

In `crates/nightfall-sim/src/sight.rs`, `snapshot_for`'s literal gains:

```rust
            telegraphs: self
                .telegraphs
                .iter()
                .filter(|t| near.is_none_or(|p| p.distance(t.at.ground()) <= reach))
                .copied()
                .collect(),
```

In `crates/nightfall-sim/src/enemies.rs`, add `use crate::combat::EnemyFight;`; `Enemy` gains, after `asleep`:

```rust
    /// Its side of the fight (spec §6).
    pub fight: EnemyFight,
```

and `spawn_all`'s `Enemy { … }` literal gains `fight: EnemyFight::default(),`. Every other `Enemy { … }` literal (`grep -rn "Enemy {" crates/nightfall-sim/src` finds those in `sight.rs`'s and `messages.rs`'s tests) gains `fight: Default::default(),`; every other `Hero { … }` literal (`grep -rn "Hero {" crates/nightfall-sim/src`) gains `fight: Default::default(),`; every `Snapshot { … }` literal outside `world.rs` and `sight.rs` gains `telegraphs: Vec::new(),`; every `PlayerInput { … }` literal that names all of its fields (the senses' `put` in `senses/tests.rs`) gains `..Default::default()`.

In `crates/nightfall-sim/src/messages.rs`: `use crate::{Event, Mark, PlayerId, PlayerInput, Snapshot};`, `pub const PROTOCOL: u32 = 4;`, and the `State` variant becomes:

```rust
    /// The world after a step, as this player gets it: the enemies and telegraphs near their
    /// hero, which enemies their hero sees (spec §4.3), what each thinks of their hero (§5.5), and
    /// what happened since the last state this player got (§6).
    State {
        snapshot: Snapshot,
        seen: Vec<u32>,
        marks: Vec<Mark>,
        events: Vec<Event>,
    },
```

In `crates/nightfall-sim/src/lib.rs`, replace `pub use combat::Charges;` with:

```rust
pub use combat::{
    CastPress, Casting, Charges, Cooldown, EnemyFight, Event, HeroFight, Life, Refusal, Stop,
    Telegraph, UnitRef,
};
```

- [ ] **Step 4: Run the sim's tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test, the 3 in `combat::state_tests`, `a_state_with_events_survives_the_wire`, and `a_full_partys_state_is_small` under its new bound).

- [ ] **Step 5: Carry the events through the client's session and run**

In `crates/benilla-app/src/nightfall/session.rs`:

- Add `Event` to its `nightfall_sim` import, and below `MAX_BACKLOG`:

```rust
/// Events a client that keeps falling behind may have waiting; past it the oldest go, so a stalled
/// connection never grows the host's memory without end.
const MAX_PENDING: usize = 4096;
```

- `Peer` gains:

```rust
    /// The events since this player's last state, sent with the next one it keeps up for.
    pending: Vec<Event>,
```

and the one `Peer { … }` literal (in `lobby`, where a hello is welcomed) gains `pending: Vec::new(),`.
- Replace `host_send` with:

```rust
/// The host, after a step: the step's events (from `events_from` on) kept for every client, and
/// every [`SEND_EVERY`] steps each client whose connection keeps up gets the world as its player
/// sees it, with the events since its last state.
pub(super) fn host_send(net: &mut Net, world: &World, events_from: usize) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    if h.peers.is_empty() {
        return;
    }
    let events = &world.events()[events_from..];
    for p in &mut h.peers {
        p.pending.extend_from_slice(events);
        let over = p.pending.len().saturating_sub(MAX_PENDING);
        p.pending.drain(..over);
    }
    h.steps += 1;
    if h.steps % SEND_EVERY != 0 {
        return;
    }
    let host = h.host.lock().unwrap();
    for p in &mut h.peers {
        if host.backlog(p.conn) > MAX_BACKLOG {
            continue;
        }
        host.send(
            p.conn,
            &ServerMsg::State {
                snapshot: world.snapshot_for(p.player),
                seen: world.seen_by(p.player),
                marks: world.marks_for(p.player),
                events: std::mem::take(&mut p.pending),
            },
        );
    }
}
```

- `Heard`'s `State(Snapshot, Vec<u32>, Vec<Mark>),` becomes:

```rust
    /// A state, which enemies in it our hero sees, what they think of our hero, and what
    /// happened since the last.
    State(Snapshot, Vec<u32>, Vec<Mark>, Vec<Event>),
```

- In `client_exchange`, the `State` arm becomes `ClientEvent::Message(ServerMsg::State { snapshot, seen, marks, events }) => heard.push(Heard::State(snapshot, seen, marks, events)),`.

In `crates/benilla-app/src/nightfall/run.rs`:

- Add `Event` to its `nightfall_sim` import.
- The `inbox` field becomes `inbox: VecDeque<(Snapshot, Vec<u32>, Vec<Mark>, Vec<Event>)>,` (its doc adds "and what happened since the last").
- `Sim` gains, after `marks`:

```rust
    /// What happened in the steps taken (or the states played) this frame, oldest first, for
    /// the fight's look ([`super::fight`]).
    pub events: Vec<Event>,
```

and `Sim::hosted` and `Sim::mirror` add `events: Vec::new(),`.
- `apply_next` becomes:

```rust
    fn apply_next(&mut self) {
        if let Some((snapshot, seen, marks, events)) = self.inbox.pop_front() {
            self.world.apply_snapshot(snapshot);
            self.world.push_events(events);
            self.seen = seen;
            self.marks = marks;
            self.stepped = true;
        }
    }
```

- `LocalInput::with_pose`'s `PlayerInput { … }` literal gains `..Default::default()` (Task 9 fills the press fields).
- In `step_sim`, the client loop's `if let Heard::State(snapshot, seen, marks) = h { sim.inbox.push_back((snapshot, seen, marks)); }` becomes `if let Heard::State(snapshot, seen, marks, events) = h { sim.inbox.push_back((snapshot, seen, marks, events)); }`; in the host loop, `sim.world.step();` gains `let from = sim.world.events().len();` before it, and `session::host_send(&mut net, &sim.world);` becomes `session::host_send(&mut net, &sim.world, from);`.
- Add after `step_sim`:

```rust
/// This frame's events out of the world, so they are shown once and never pile up.
pub(super) fn take_events(mut sim: ResMut<Sim>) {
    sim.events = sim.world.drain_events();
}
```

In `crates/benilla-app/src/nightfall/mod.rs`, in the run's chain `(run::feed_pose, run::step_sim, view::sync, …)`, add `run::take_events` right after `run::step_sim`.

- [ ] **Step 6: Run the client's tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo check -p benilla-app -j 2`
Expected: builds.

- [ ] **Step 7: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim crates/benilla-app/src/nightfall/session.rs crates/benilla-app/src/nightfall/run.rs crates/benilla-app/src/nightfall/mod.rs
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: each hero's and enemy's fight, telegraphs and events on the wire; protocol 4

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: What abilities do where they land

**Files:**
- Create: `crates/nightfall-sim/src/combat/effects.rs`, `crates/nightfall-sim/src/combat/tests.rs`
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (modules), `crates/nightfall-sim/src/senses/mod.rs`, `crates/nightfall-sim/src/senses/awareness.rs`

**Interfaces:**
- Consumes: `Combat`, `Burn`, `Held`, `Slowed`, `Meal`, `Event`, `UnitRef`, `World::{hero_slot, enemies_within}` (Task 3); `Dice` (Task 2); `AbilityDef`, `Aim`, `Effect`, `ControlKind`, `Sound`, `Swing` (Task 1); `World::{ambush, alert, make_noise, map}`, `Mind`, `State`, `AwarenessRules`, `Loudness` (senses Tasks 3, 5–7); `HeightMap::{floor_within, walk_line}`, `testmap::{flat, house}` (world knowledge).
- Produces:
  - Crate-only on `World`: `land(&mut self, player: PlayerId, ability: usize, enemy: Option<usize>, at: Option<V3>) -> Vec<UnitRef>`; `strike(&mut self, by: PlayerId, ei: usize, amount: f32, ambush: bool, silent: bool, may_crit: bool)`; `kill_enemy(&mut self, ei: usize, by: Option<PlayerId>)`; `effects::MANA`.
  - Crate-only on the senses: `Mind::lose(&mut self, PlayerId, &AwarenessRules)`; `Senses::{lose(&mut self, PlayerId, &AwarenessRules), alerted(&self, usize) -> bool}`.
  - Test-only in `combat::tests`: `world(&str) -> World`, `world_with(combat: &str, spawns: &str) -> World`, `open()`, `post(kind, x, y, facing) -> String`, `join(&mut World, PlayerId, class, x, y)`, `stand(&mut World, PlayerId, V2, V2, Gait)`, `press(&mut World, PlayerId, seq, slot, Option<UnitRef>, Option<V3>)`, `steps`, `ability(&World, &str) -> usize`, `foe(&World, usize) -> Option<UnitRef>`, `damage_to(&[Event], UnitRef) -> Vec<f32>`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/combat/tests.rs`:

```rust
//! The fight in whole worlds on small hand-made maps: what abilities do, presses and casts,
//! enemies held and hurt, heroes falling, and enemies striking back at heroes who dodge.

use std::sync::Arc;

use super::{Slowed, Telegraph};
use crate::heightmap::testmap::{flat, house};
use crate::heightmap::HeightMap;
use crate::{
    Alert, CastPress, Content, ContentSources, ControlKind, Event, Gait, HeroPose, Life, Mark,
    PlayerId, PlayerInput, Refusal, Shape, Stop, UnitRef, World, DT, V2, V3,
};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0,
     swing: Some((min: 30.0, max: 30.0, every: 2.0)),
     bar: [(1, "stealth"), (2, "strike"), (3, "finish"), (4, "gouge"), (5, "sprint"),
           (6, "distract"), (7, "vanish"), (11, "stone"), (12, "eat"), (74, "ambush"),
           (75, "garrote"), (76, "cheap"), (77, "sap")]),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 800.0,
     speed: 7.0, power_max: 1000.0,
     bar: [(1, "bolt"), (2, "nuke"), (3, "blast"), (4, "nova"), (5, "missiles"), (6, "sheep"),
           (7, "blink"), (8, "water"), (11, "stone"), (12, "eat")]),
]"#;
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)],
    entries: [(5.0, 5.0, 0.0)])"#;
const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 1000.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 40.0, max: 40.0, every: 2.0))),
    (key: "beast", name: "Beast", display: 955, level: 26, hp: 800.0, walk_speed: 2.2,
     run_speed: 7.0, creature: Beast),
    (key: "bones", name: "Bones", display: 200, level: 28, hp: 900.0, walk_speed: 2.5,
     run_speed: 7.0, creature: Undead),
    (key: "caster", name: "Caster", display: 203, level: 28, hp: 900.0, walk_speed: 2.5,
     run_speed: 7.0, moves: [(ability: "smite", every: 5.0)]),
    (key: "elite", name: "Elite", display: 657, level: 31, hp: 3000.0, walk_speed: 2.5,
     run_speed: 8.0, elite: true, moves: [(ability: "cleave", every: 8.0)]),
    (key: "stomper", name: "Stomper", display: 955, level: 30, hp: 3000.0, walk_speed: 2.2,
     run_speed: 7.0, elite: true, moves: [(ability: "stomp", every: 8.0)]),
]"#;
const ABILITIES: &str = r#"[
    (key: "bolt", name: "Bolt", spell: 116, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
     gcd: 1.5, moving: Slowed, facing: Front, effects: [Damage(min: 100.0, max: 100.0)]),
    (key: "nuke", name: "Nuke", spell: 133, aim: Enemy, range: 30.0, cost: 50.0, cast: 2.0,
     gcd: 1.5, moving: Rooted, sound: Loud, flash: true, facing: Front,
     effects: [Damage(min: 100.0, max: 100.0)]),
    (key: "blast", name: "Blast", spell: 2136, aim: Enemy, range: 20.0, cost: 50.0,
     cooldown: 8.0, gcd: 1.5, sound: Loud, effects: [Damage(min: 100.0, max: 100.0)]),
    (key: "missiles", name: "Missiles", spell: 5143, aim: Enemy, range: 30.0, cost: 50.0,
     channel: 3.0, ticks: 3, gcd: 1.5, moving: Rooted,
     effects: [Damage(min: 10.0, max: 10.0)]),
    (key: "nova", name: "Nova", spell: 122, aim: Me, radius: 10.0, cost: 50.0, cooldown: 25.0,
     gcd: 1.5, effects: [Damage(min: 10.0, max: 10.0), Control(kind: Root, secs: 8.0)]),
    (key: "sheep", name: "Sheep", spell: 118, aim: Enemy, range: 30.0, cost: 50.0, cast: 1.5,
     gcd: 1.5, moving: Slowed, needs: (creatures: [Humanoid, Beast]),
     effects: [Control(kind: Polymorph, secs: 20.0)]),
    (key: "blink", name: "Blink", spell: 1953, aim: Me, cooldown: 15.0, gcd: 1.5,
     effects: [Blink(distance: 20.0)]),
    (key: "water", name: "Water", spell: 6127, aim: Me, cost: 50.0, cast: 3.0, gcd: 1.5,
     moving: Rooted, needs: (out_of_combat: true), effects: [ConjureWater(count: 2)]),
    (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0, sound: Silent,
     keeps_stealth: true, needs: (out_of_combat: true), effects: [Stealth]),
    (key: "sap", name: "Sap", spell: 2070, aim: Enemy, range: 5.0, cost: 65.0, gcd: 1.0,
     sound: Quiet, alerts: false, keeps_stealth: true,
     needs: (stealth: true, unaware: true, creatures: [Humanoid]),
     effects: [Control(kind: Sap, secs: 30.0)]),
    (key: "cheap", name: "Cheap", spell: 1833, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     attack: true, needs: (stealth: true, unaware: true),
     effects: [Control(kind: Stun, secs: 4.0), Combo(2)]),
    (key: "garrote", name: "Garrote", spell: 8632, aim: Enemy, range: 5.0, cost: 50.0, gcd: 1.0,
     sound: Silent, facing: Behind, attack: true, needs: (stealth: true, unaware: true),
     effects: [Dot(amount: 30.0, every: 3.0, secs: 18.0), Combo(1)]),
    (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0, cost: 60.0, gcd: 1.0,
     facing: Behind, attack: true, needs: (stealth: true, unaware: true),
     effects: [Weapon(percent: 250.0, bonus: 40.0), Combo(1)]),
    (key: "strike", name: "Strike", spell: 1760, aim: Enemy, range: 5.0, cost: 45.0, gcd: 1.0,
     facing: Front, attack: true, effects: [Weapon(bonus: 20.0), Combo(1)]),
    (key: "finish", name: "Finish", spell: 6762, aim: Enemy, range: 5.0, cost: 35.0, gcd: 1.0,
     facing: Front, needs: (combo: true),
     effects: [Damage(min: 10.0, max: 10.0, per_combo: 30.0)]),
    (key: "gouge", name: "Gouge", spell: 1777, aim: Enemy, range: 5.0, cost: 45.0,
     cooldown: 10.0, gcd: 1.0, facing: Front,
     effects: [Damage(min: 20.0, max: 20.0), Control(kind: Incapacitate, secs: 4.0), Combo(1)]),
    (key: "sprint", name: "Sprint", spell: 2983, aim: Me, cooldown: 300.0, sound: Silent,
     keeps_stealth: true, effects: [Sprint(by: 0.5, secs: 8.0)]),
    (key: "distract", name: "Distract", spell: 1725, aim: Ground, range: 30.0, radius: 10.0,
     cost: 30.0, cooldown: 30.0, gcd: 1.0, sound: Loud, alerts: false, keeps_stealth: true,
     effects: [Distract(secs: 10.0)]),
    (key: "vanish", name: "Vanish", spell: 1856, aim: Me, cooldown: 300.0, sound: Quiet,
     keeps_stealth: true, effects: [Vanish]),
    (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0, cooldown: 20.0,
     alerts: false, keeps_stealth: true, effects: []),
    (key: "eat", name: "Eat", spell: 2639, aim: Me, sound: Quiet,
     needs: (out_of_combat: true, still: true), effects: [Eat(secs: 15.0)]),
    (key: "smite", name: "Smite", spell: 1106, aim: Enemy, range: 30.0, cast: 2.0,
     facing: Front, effects: [Damage(min: 50.0, max: 50.0)]),
    (key: "cleave", name: "Cleave", spell: 845, aim: Enemy, range: 8.0, sound: Loud,
     telegraph: Some((shape: Cone(length: 8.0, angle: 120.0), windup: 1.2)),
     effects: [Damage(min: 200.0, max: 200.0)]),
    (key: "stomp", name: "Stomp", spell: 45, aim: Me, radius: 8.0,
     telegraph: Some((shape: Circle(radius: 8.0), windup: 1.5, on_self: true)),
     effects: [Damage(min: 150.0, max: 150.0)]),
]"#;

/// The tests' rules: crits off so damage is exact, with `combat` (more `combat:` fields) on top.
fn rules(combat: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0, {combat}))"#
    )
}

/// A world on flat ground 60 yards across, its map attached, with `spawns` (RON list items, each
/// ending in a comma) and `combat` rules on top of the tests' own.
pub(super) fn world_with(combat: &str, spawns: &str) -> World {
    let content = Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(combat),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
    })
    .unwrap();
    let mut w = World::new(Arc::new(content));
    w.attach_map(open());
    w
}

pub(super) fn world(spawns: &str) -> World {
    world_with("", spawns)
}

/// Flat ground 60 yards across.
pub(super) fn open() -> Arc<HeightMap> {
    Arc::new(flat(60.0).finish())
}

/// A `kind` sentry at `(x, y)` facing `facing` degrees (0 north, 90 west), as a spawn line.
pub(super) fn post(kind: &str, x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "{kind}", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// `player`'s hero of `class` at `(x, y)` facing north and standing, its first input (the
/// presses' baseline) taken.
pub(super) fn join(w: &mut World, player: PlayerId, class: &str, x: f32, y: f32) {
    w.add_hero(player, class, "Hero", 1, 0).unwrap();
    stand(w, player, V2::new(x, y), V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

/// `player`'s next input: their hero at `at`, facing `facing`, moving at `gait`.
pub(super) fn stand(w: &mut World, player: PlayerId, at: V2, facing: V2, gait: Gait) {
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: at,
                facing,
                z: 0.0,
            }),
            gait,
            ..Default::default()
        },
    );
}

/// `player`'s next input presses action `slot` (press number `seq`) at `target` or the spot
/// `at`, the hero standing where they are.
pub(super) fn press(
    w: &mut World,
    player: PlayerId,
    seq: u8,
    slot: u8,
    target: Option<UnitRef>,
    at: Option<V3>,
) {
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
                at,
            }),
            ..Default::default()
        },
    );
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

pub(super) fn ability(w: &World, key: &str) -> usize {
    w.content().ability(key).unwrap()
}

/// Enemy `i` as a target.
pub(super) fn foe(w: &World, i: usize) -> Option<UnitRef> {
    Some(UnitRef::Enemy(w.enemies[i].id))
}

/// The damage `to` took in `events`, in order.
pub(super) fn damage_to(events: &[Event], to: UnitRef) -> Vec<f32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Damage { to: t, amount, .. } if *t == to => Some(*amount),
            _ => None,
        })
        .collect()
}

#[test]
fn damage_lands_with_the_ambush_on_an_unaware_enemy_once() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let bolt = ability(&w, "bolt");
    w.land(0, bolt, Some(0), None);
    w.land(0, bolt, Some(0), None);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, foe(&w, 0).unwrap()), vec![150.0, 100.0]);
    assert!(events.iter().any(|e| matches!(e, Event::Damage { ambush: true, .. })));
    assert_eq!(w.enemies[0].hp, 750.0);
}

#[test]
fn an_enemy_alerted_to_a_friend_is_still_caught_unaware_by_you() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    join(&mut w, 1, "mage", 20.0, 35.0);
    let id = w.enemies[0].id;
    w.alert(id, 1);
    w.land(0, ability(&w, "bolt"), Some(0), None);
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Enemy(id)), vec![150.0]);
}

#[test]
fn sap_holds_without_alerting() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let id = w.enemies[0].id;
    w.land(0, ability(&w, "sap"), Some(0), None);
    assert_eq!(
        w.combat.enemies[0].held.map(|h| h.kind),
        Some(ControlKind::Sap)
    );
    assert!(w.opener_allowed(id, 0));
    assert!(w.marks_for(0).is_empty());
}

#[test]
fn a_blow_frees_a_sheep_and_a_sapped_enemy_but_not_a_stunned_one() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    for (key, freed) in [("sap", true), ("sheep", true), ("cheap", false)] {
        w.combat.enemies[0].held = None;
        w.land(0, ability(&w, key), Some(0), None);
        w.strike(0, 0, 10.0, false, false, false);
        assert_eq!(w.combat.enemies[0].held.is_none(), freed, "{key}");
    }
    // Gouge's own damage comes before its hold, so it holds.
    w.combat.enemies[0].held = None;
    w.land(0, ability(&w, "gouge"), Some(0), None);
    assert_eq!(
        w.combat.enemies[0].held.map(|h| h.kind),
        Some(ControlKind::Incapacitate)
    );
}

#[test]
fn combo_points_build_on_one_target_and_a_finisher_spends_them() {
    let spawns = format!("{}{}", post("brute", 30.0, 30.0, 0.0), post("brute", 30.0, 40.0, 0.0));
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let (strike, finish) = (ability(&w, "strike"), ability(&w, "finish"));
    let (a, b) = (w.enemies[0].id, w.enemies[1].id);
    w.land(0, strike, Some(0), None);
    w.land(0, strike, Some(0), None);
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.combo, f.combo_on), (2, Some(a)));
    w.drain_events();
    w.land(0, finish, Some(0), None);
    assert_eq!(damage_to(&w.drain_events(), UnitRef::Enemy(a)), vec![70.0]);
    assert_eq!(w.hero(0).unwrap().fight.combo, 0);
    w.land(0, strike, Some(0), None);
    w.land(0, strike, Some(1), None);
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.combo, f.combo_on), (1, Some(b)));
}

#[test]
fn an_area_ability_reaches_only_within_its_radius() {
    let spawns = format!("{}{}", post("brute", 30.0, 38.0, 0.0), post("brute", 30.0, 45.0, 0.0));
    let mut w = world(&spawns);
    join(&mut w, 0, "mage", 30.0, 30.0);
    let hits = w.land(0, ability(&w, "nova"), None, None);
    assert_eq!(hits, vec![UnitRef::Enemy(w.enemies[0].id)]);
    assert_eq!(
        w.combat.enemies[0].held.map(|h| h.kind),
        Some(ControlKind::Root)
    );
    assert!(w.combat.enemies[1].held.is_none());
}

#[test]
fn a_kill_is_heard_and_ends_attacks_and_combos_on_the_dead() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    let id = w.enemies[0].id;
    w.land(0, ability(&w, "strike"), Some(0), None);
    assert_eq!(w.hero(0).unwrap().fight.attacking, Some(id));
    w.strike(0, 0, 5000.0, false, false, false);
    assert_eq!(w.enemies[0].hp, 0.0);
    assert!(w.drain_events().contains(&Event::EnemyDied { id, by: Some(0) }));
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.attacking, f.combo, f.combo_on), (None, 0, None));
    w.step();
    assert_eq!(w.hero(0).unwrap().noise, 15.0);
}

#[test]
fn vanish_turns_a_chase_into_a_search() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 20.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: id,
            alert: Alert::Alerted
        }]
    );
    w.land(0, ability(&w, "vanish"), None, None);
    assert!(w.hero(0).unwrap().stealthed);
    assert_eq!(
        w.marks_for(0),
        vec![Mark {
            enemy: id,
            alert: Alert::Suspicious
        }]
    );
}

#[test]
fn blink_stops_short_of_a_wall() {
    let mut w = world("");
    let mut b = flat(60.0);
    house(&mut b, V2::new(19.0, 0.0), V2::new(30.0, 60.0), None, false);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "blink"), None, None);
    let to = w
        .drain_events()
        .into_iter()
        .find_map(|e| match e {
            Event::Blinked { player: 0, to } => Some(to),
            _ => None,
        })
        .unwrap();
    assert!(to.x > 14.0 && to.x < 19.0, "{to:?}");
    assert!((to.y - 30.0).abs() < 1e-3);
    // In the open it goes the whole way.
    let mut w = world("");
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "blink"), None, None);
    assert!(w.drain_events().contains(&Event::Blinked {
        player: 0,
        to: V3::new(30.0, 30.0, 0.0)
    }));
}

#[test]
fn distract_turns_only_the_unalerted_within_reach() {
    let spawns = format!(
        "{}{}{}",
        post("brute", 40.0, 30.0, 0.0),
        post("brute", 44.0, 33.0, 0.0),
        post("brute", 55.0, 30.0, 0.0)
    );
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 10.0, 30.0);
    let alerted = w.enemies[1].id;
    w.alert(alerted, 0);
    w.land(0, ability(&w, "distract"), None, Some(V3::new(42.0, 30.0, 0.0)));
    let d = |i: usize| w.combat.enemies[i].distracted.map(|(at, _)| at);
    assert_eq!(d(0), Some(V2::new(42.0, 30.0)));
    assert_eq!(d(1), None);
    assert_eq!(d(2), None);
}

#[test]
fn eating_needs_a_drink_for_mana_and_water_gives_two() {
    let mut w = world("");
    join(&mut w, 0, "mage", 10.0, 30.0);
    let eat = ability(&w, "eat");
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert!((meal.health - 800.0 / 15.0).abs() < 1e-3);
    assert_eq!(meal.mana, 0.0);
    w.land(0, ability(&w, "water"), None, None);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 2);
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert!((meal.mana - 1000.0 / 15.0).abs() < 1e-3);
    let f = &w.hero(0).unwrap().fight;
    assert_eq!((f.drinks, f.eating), (1, 15.0));
}
```

In `crates/nightfall-sim/src/combat/mod.rs`, below `mod rules;` add:

```rust
mod effects;
#[cfg(test)]
mod tests;

pub(crate) use effects::MANA;
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim combat::tests`
Expected: FAIL to compile (`no method named land`, `no method named strike`, `file not found for module effects`).

- [ ] **Step 3: Let the senses lose a hero**

In `crates/nightfall-sim/src/senses/awareness.rs`, add to `impl Mind`, after `alert`:

```rust
    /// `player` slipped away (Vanish, spec §6.3): alerted to them, it searches where it last saw
    /// them instead, as if it had lost them; seeing them again finds them again.
    pub(crate) fn lose(&mut self, player: PlayerId, r: &AwarenessRules) {
        if let State::Alerted {
            player: p,
            last_seen,
            ..
        } = self.state
        {
            if p == player {
                self.state = State::Searching {
                    player,
                    place: last_seen,
                    left: r.search_for,
                };
                self.there = false;
                self.entry(player).level = r.check;
            }
        }
    }
```

In `crates/nightfall-sim/src/senses/mod.rs`, add `AwarenessRules` to its `crate::{…}` import and to `impl Senses`:

```rust
    /// `player` slipped away (Vanish): every enemy after them searches instead.
    pub(crate) fn lose(&mut self, player: PlayerId, r: &AwarenessRules) {
        for m in &mut self.minds {
            m.lose(player, r);
        }
    }

    /// Whether enemy `i` is alerted to anyone.
    pub(crate) fn alerted(&self, i: usize) -> bool {
        self.minds.get(i).is_some_and(Mind::is_alerted)
    }
```

- [ ] **Step 4: Implement what abilities do**

`crates/nightfall-sim/src/combat/effects.rs`:

```rust
//! What abilities do where they land (spec §5.4, §6): damage, with the ambush on an enemy caught
//! unaware of its attacker and a chance to crit; the enemy alerted to whoever hit it (all but Sap
//! and Distract) and the 15-yard noise of the hit and of a death; holds, slows, bleeds and burns;
//! combo points and the finishers that spend them; auto-attack begun; and the moves a hero makes
//! on themselves: Vanish, Sprint, Blink, Distract, eating and drinking, conjuring water. Casts
//! going off and channels ticking ([`super::heroes`]) call [`World::land`]; auto-attack, bleeds
//! and burns call [`World::strike`].

use super::{Burn, Event, Held, Meal, Slowed, UnitRef};
use crate::{Aim, ControlKind, Effect, Loudness, PlayerId, Sound, World, V2, V3};

/// Yards up or down a Blink may end from where it began.
const BLINK_CLIMB: f32 = 3.0;
/// Yards a Blink shortens by, each try, until nothing stands in its way.
const BLINK_STEP: f32 = 0.5;
/// `ClassDef::power` of a class whose power is mana.
pub(crate) const MANA: u8 = 0;

impl World {
    /// `player`'s ability `ability` lands: on enemy `enemy` (an index into the enemies), on every
    /// enemy within its radius of the hero or of the spot `at`, and on the hero themselves. The
    /// enemies it reached, for the cast's visual.
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
        // A finisher spends every combo point at once.
        let combo = if a.needs.combo {
            let f = &mut self.heroes[hi].fight;
            f.combo_on = None;
            std::mem::take(&mut f.combo)
        } else {
            0
        };
        for &ei in &targets {
            self.land_on(player, ability, ei, combo);
        }
        for effect in &a.effects {
            self.land_self(player, effect, centre, a.radius);
        }
        targets
            .iter()
            .map(|&ei| UnitRef::Enemy(self.enemies[ei].id))
            .collect()
    }

    /// Ability `ability` lands on enemy `ei`, a finisher's `combo` points behind it.
    fn land_on(&mut self, player: PlayerId, ability: usize, ei: usize, combo: u8) {
        let content = self.content.clone();
        let a = &content.abilities[ability];
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if self.enemies[ei].hp <= 0.0 {
            return;
        }
        let id = self.enemies[ei].id;
        // Asked before the blow, which alerts it: only each hero's first blow is an ambush.
        let ambush = self.ambush(id, player);
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
                    self.strike(player, ei, amount * ambush, ambush > 1.0, silent, true);
                }
                Effect::Weapon { percent, bonus } => {
                    let Some(blow) = content.classes[self.heroes[hi].class].swing else {
                        continue;
                    };
                    let amount =
                        self.combat.dice.between(blow.min, blow.max) * percent / 100.0 + bonus;
                    self.strike(player, ei, amount * ambush, ambush > 1.0, silent, true);
                }
                Effect::Dot { amount, every, secs } => {
                    let burns = &mut self.combat.enemies[ei].burns;
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
                    self.combat.enemies[ei].slow = Some(Slowed {
                        by,
                        left: secs,
                        spell: a.spell,
                    });
                }
                Effect::Control { kind, secs } => {
                    let here = self.enemies[ei].pos;
                    let ec = &mut self.combat.enemies[ei];
                    ec.held = Some(Held {
                        kind,
                        left: secs,
                        by: player,
                        spell: a.spell,
                    });
                    ec.wander = (kind == ControlKind::Polymorph).then_some((here, here));
                    // Its caster stops swinging at what a blow would free.
                    if kind.freed_by_damage() && self.heroes[hi].fight.attacking == Some(id) {
                        self.heroes[hi].fight.attacking = None;
                    }
                }
                Effect::Combo(n) => {
                    let max = content.rules.combat.combo_max;
                    let f = &mut self.heroes[hi].fight;
                    if f.combo_on != Some(id) {
                        f.combo = 0;
                    }
                    f.combo = (f.combo + n).min(max);
                    f.combo_on = Some(id);
                }
                _ => {}
            }
            if self.enemies[ei].hp <= 0.0 {
                return;
            }
        }
        if a.alerts {
            self.alert(id, player);
        }
        if a.attack && content.classes[self.heroes[hi].class].swing.is_some() {
            self.heroes[hi].fight.attacking = Some(id);
        }
    }

    /// `by`'s blow of `amount` on enemy `ei`: a crit when `may_crit` and the dice say so; a blow
    /// frees what damage frees; it keeps `by` in combat; it is heard 15 yards off unless
    /// `silent`; and at 0 health the enemy dies.
    pub(crate) fn strike(
        &mut self,
        by: PlayerId,
        ei: usize,
        amount: f32,
        ambush: bool,
        silent: bool,
        may_crit: bool,
    ) {
        if self.enemies[ei].hp <= 0.0 {
            return;
        }
        let (chance, times, linger) = {
            let r = &self.content.rules.combat;
            (r.crit_chance, r.crit_times, r.combat_linger)
        };
        let crit = may_crit && self.combat.dice.chance(chance);
        let amount = (if crit { amount * times } else { amount }).max(0.0);
        let e = &mut self.enemies[ei];
        e.hp = (e.hp - amount).max(0.0);
        let (id, at, dead) = (e.id, V3::at(e.pos, e.z), e.hp <= 0.0);
        self.events.push(Event::Damage {
            to: UnitRef::Enemy(id),
            by: Some(UnitRef::Hero(by)),
            amount,
            crit,
            ambush,
        });
        let ec = &mut self.combat.enemies[ei];
        if ec.held.is_some_and(|h| h.kind.freed_by_damage()) {
            ec.held = None;
            ec.wander = None;
        }
        if self.hero_slot(by).is_some() {
            let ci = self.combat.hero_index(by);
            self.combat.heroes[ci].combat_for = linger;
        }
        if !silent {
            self.make_noise(by, at, Loudness::Hit);
        }
        if dead {
            self.kill_enemy(ei, Some(by));
        }
    }

    /// Enemy `ei` dies (by `by`'s hand, when a hero's): what held it and what it was about to do
    /// go, heroes stop swinging at it and lose their combo points on it, and the death is heard
    /// 15 yards off. Its body stays where it fell, for sub-project 5 to search.
    pub(crate) fn kill_enemy(&mut self, ei: usize, by: Option<PlayerId>) {
        let e = &mut self.enemies[ei];
        e.hp = 0.0;
        e.speed = 0.0;
        let (id, at) = (e.id, V3::at(e.pos, e.z));
        let ec = &mut self.combat.enemies[ei];
        ec.held = None;
        ec.slow = None;
        ec.burns.clear();
        ec.cast = None;
        ec.distracted = None;
        ec.wander = None;
        self.telegraphs.retain(|t| t.enemy != id);
        for h in &mut self.heroes {
            if h.fight.attacking == Some(id) {
                h.fight.attacking = None;
            }
            if h.fight.combo_on == Some(id) {
                h.fight.combo = 0;
                h.fight.combo_on = None;
            }
        }
        self.events.push(Event::EnemyDied { id, by });
        if let Some(by) = by {
            self.make_noise(by, at, Loudness::Hit);
        }
    }

    /// What `effect` does to `player`'s hero themselves; `spot` is where a ground ability went,
    /// `radius` how far around it reaches.
    fn land_self(&mut self, player: PlayerId, effect: &Effect, spot: Option<V2>, radius: f32) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let content = self.content.clone();
        let ci = self.combat.hero_index(player);
        match *effect {
            Effect::Vanish => {
                let hero = &mut self.heroes[hi];
                hero.stealthed = true;
                hero.fight.attacking = None;
                self.combat.heroes[ci].combat_for = 0.0;
                self.senses.lose(player, &content.rules.senses.awareness);
            }
            Effect::Sprint { by, secs } => {
                self.heroes[hi].fight.sprint = secs;
                self.combat.heroes[ci].sprint_by = by;
            }
            Effect::Blink { distance } => {
                let to = self.blink_to(hi, distance);
                self.events.push(Event::Blinked { player, to });
            }
            Effect::Distract { secs } => {
                let Some(spot) = spot else {
                    return;
                };
                for ei in self.enemies_within(spot, radius) {
                    if !self.senses.alerted(ei) {
                        self.combat.enemies[ei].distracted = Some((spot, secs));
                    }
                }
            }
            Effect::Eat { secs } => {
                let hero = &mut self.heroes[hi];
                let drinks = content.classes[hero.class].power == MANA && hero.fight.drinks > 0;
                if drinks {
                    hero.fight.drinks -= 1;
                }
                hero.fight.eating = secs;
                let mana = if drinks {
                    hero.fight.max_power / secs
                } else {
                    0.0
                };
                self.combat.heroes[ci].meal = Some(Meal {
                    health: hero.max_hp / secs,
                    mana,
                });
            }
            Effect::ConjureWater { count } => {
                let f = &mut self.heroes[hi].fight;
                f.drinks = f.drinks.saturating_add(count).min(content.rules.combat.max_drinks);
            }
            _ => {}
        }
    }

    /// Where a Blink of `distance` yards ends for hero `hi`: straight the way they face, shortened
    /// until no wall stands between and a floor lies near their height; where they stand when no
    /// such spot is found. Without a map (a mirror) nothing stands in the way.
    fn blink_to(&self, hi: usize, distance: f32) -> V3 {
        let h = &self.heroes[hi];
        let from = V3::at(h.pos, h.z);
        let way = h.facing.normalize_or_zero();
        let mut d = distance;
        while d > 0.0 {
            let to = self.content.map.bounds.clamp(h.pos + way * d);
            match self.map() {
                None => return V3::at(to, h.z),
                Some(map) => {
                    if let Some(z) = map.floor_within(to, h.z, BLINK_CLIMB) {
                        let end = V3::at(to, z);
                        if map.walk_line(from, end) {
                            return end;
                        }
                    }
                }
            }
            d -= BLINK_STEP;
        }
        from
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim combat`
Expected: PASS (the 11 tests in `combat::tests`, and the earlier `combat` ones).
Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test).

- [ ] **Step 6: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim/src/combat crates/nightfall-sim/src/senses
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: what abilities do: damage, ambush, holds, slows, bleeds, combos, Vanish, Blink, Distract, meals

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Presses and casts: checks, the global cooldown, power, channels, feet, auto-attack

**Files:**
- Create: `crates/nightfall-sim/src/combat/heroes.rs`
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (module), `crates/nightfall-sim/src/combat/tests.rs`, `crates/nightfall-sim/src/world.rs` (`step`), `crates/nightfall-sim/src/senses/mod.rs` (`take_input`, `hunted`)

**Interfaces:**
- Consumes: `World::{land, strike, kill_enemy}`, `MANA` (Task 4); `HeroCombat`, `CastState`, `Casting`, `Cooldown`, `CastPress`, `Event`, `Refusal`, `Stop`, `UnitRef` (Task 3); `allowance`, `in_range`, `stands_right`, `within_arc` (Task 2); `AbilityDef`, `Aim`, `Effect`, `Facing`, `Moving` (Task 1); `World::{opener_allowed, ambush, alert, make_noise, flash, break_stealth, map}`, `HeightMap::line_of_sight`, `senses::{take_input, step_heroes}`, `Gait`, `EYE_HEIGHT`, `CHEST_HEIGHT` (earlier plans).
- Produces:
  - Crate-only on `World`: `take_presses(&mut self, PlayerId, &PlayerInput)`; `check(&self, hi: usize, ability: usize, Option<UnitRef>, Option<V3>) -> Result<Aimed, Refusal>`; `clear_line(&self, V3, V3) -> bool`; `stop_cast(&mut self, hi: usize, Stop)`; `stop_eating(&mut self, hi: usize)`; `step_fighters(&mut self)`; `may_stealth(&self, PlayerId) -> bool`; `heroes::{MELEE, NEEDS_SIGHT, Aimed}`.
  - `senses::take_input(&mut Senses, &mut Hero, &PlayerInput, &Content, may_stealth: bool)`; `Senses::hunted(&self, PlayerId) -> bool`.
  - `World::step` takes the presses after the poses and steps every hero's fight after their senses.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/combat/tests.rs`:

```rust
/// The last thing the host turned `player` 0 down for, if anything, taking the events.
fn refusal(w: &mut World) -> Option<Refusal> {
    w.drain_events().into_iter().rev().find_map(|e| match e {
        Event::Refused { player: 0, why } => Some(why),
        _ => None,
    })
}

/// Player 0 presses `slot` as a new press, the world steps, and what was refused (if anything).
fn attempt(
    w: &mut World,
    seq: &mut u8,
    slot: u8,
    target: Option<UnitRef>,
    at: Option<V3>,
) -> Option<Refusal> {
    *seq = seq.wrapping_add(1);
    press(w, 0, *seq, slot, target, at);
    w.step();
    refusal(w)
}

/// Player 0's next input: standing where they are, Stealth pressed `presses` times so far.
fn sneak(w: &mut World, presses: u8) {
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
            stealth_presses: presses,
            ..Default::default()
        },
    );
}

#[test]
fn a_press_repeated_in_every_input_casts_once() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 20.0, 30.0);
    let target = foe(&w, 0);
    for _ in 0..6 {
        press(&mut w, 0, 1, 3, target, None);
        w.step();
    }
    let gos = w
        .drain_events()
        .iter()
        .filter(|e| matches!(e, Event::CastGo { by: UnitRef::Hero(0), .. }))
        .count();
    assert_eq!(gos, 1);
    assert_eq!(w.hero(0).unwrap().fight.cooldowns.len(), 1);
    assert_eq!(w.enemies[0].hp, 850.0);
    assert_eq!(w.hero(0).unwrap().fight.power, 950.0);
}

#[test]
fn a_new_run_does_not_replay_the_last_press() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    w.add_hero(0, "mage", "Hero", 1, 0).unwrap();
    let target = foe(&w, 0);
    // The first input this host sees still carries a press from before: a baseline.
    press(&mut w, 0, 9, 3, target, None);
    w.step();
    press(&mut w, 0, 9, 3, target, None);
    w.step();
    assert!(w.drain_events().is_empty());
    stand(&mut w, 0, V2::new(20.0, 30.0), V2::new(1.0, 0.0), Gait::Still);
    w.step();
    press(&mut w, 0, 10, 3, target, None);
    w.step();
    assert!(w
        .drain_events()
        .iter()
        .any(|e| matches!(e, Event::CastGo { by: UnitRef::Hero(0), .. })));
}

/// A bolt (2 s, slowed) begun 20 yards from a brute, the hero walking sideways at `speed`.
fn bolt_while_moving(speed: f32) -> Vec<Event> {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 1, target, None);
    w.step();
    for k in 1..=75 {
        let at = V2::new(10.0, 30.0 + speed * k as f32 * DT);
        stand(&mut w, 0, at, V2::new(1.0, 0.0), Gait::Run);
        w.step();
    }
    w.drain_events()
}

#[test]
fn a_slowed_cast_allows_the_slowed_pace_and_breaks_beyond_it() {
    // 3 yards a second stays under the slowed pace (3.5 after the grace): the bolt goes off.
    let slow = bolt_while_moving(3.0);
    assert!(slow
        .iter()
        .any(|e| matches!(e, Event::CastGo { by: UnitRef::Hero(0), .. })));
    // 5 yards a second outruns it about a second in.
    let fast = bolt_while_moving(5.0);
    assert!(fast.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::Moved
    }));
    assert!(!fast.iter().any(|e| matches!(e, Event::CastGo { .. })));
}

#[test]
fn a_rooted_cast_cannot_begin_on_the_move_and_breaks_when_the_hero_moves() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(10.0, 30.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            gait: Gait::Run,
            cast: Some(CastPress {
                seq: 1,
                slot: 2,
                target,
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(refusal(&mut w), Some(Refusal::Moving));
    press(&mut w, 0, 2, 2, target, None);
    w.step();
    assert!(w.hero(0).unwrap().fight.rooted);
    stand(&mut w, 0, V2::new(10.0, 31.0), V2::new(1.0, 0.0), Gait::Run);
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 1,
        why: Stop::Moved
    }));
    assert!(!w.hero(0).unwrap().fight.rooted);
}

#[test]
fn a_rogues_presses_are_refused_for_what_is_wrong() {
    let spawns = format!("{}{}", post("brute", 30.0, 30.0, 0.0), post("bones", 30.0, 33.0, 0.0));
    let mut w = world(&spawns);
    join(&mut w, 0, "rogue", 27.0, 30.0);
    let (brute, bones) = (foe(&w, 0), foe(&w, 1));
    let north = V2::new(1.0, 0.0);
    let mut seq = 0;
    assert_eq!(attempt(&mut w, &mut seq, 2, None, None), Some(Refusal::NoTarget));
    assert_eq!(attempt(&mut w, &mut seq, 3, brute, None), Some(Refusal::NeedCombo));
    assert_eq!(attempt(&mut w, &mut seq, 74, brute, None), Some(Refusal::NeedStealth));
    assert_eq!(attempt(&mut w, &mut seq, 11, None, None), Some(Refusal::NoTarget));
    let far = Some(V3::new(27.0, 59.0, 0.0));
    assert_eq!(attempt(&mut w, &mut seq, 11, None, far), Some(Refusal::OutOfRange));
    w.heroes[0].stealthed = true;
    assert_eq!(attempt(&mut w, &mut seq, 77, bones, None), Some(Refusal::BadTarget));
    stand(&mut w, 0, V2::new(27.0, 30.0), north * -1.0, Gait::Still);
    w.step();
    assert_eq!(attempt(&mut w, &mut seq, 75, brute, None), Some(Refusal::Behind));
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), Some(Refusal::Facing));
    stand(&mut w, 0, V2::new(10.0, 30.0), north, Gait::Still);
    w.step();
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), Some(Refusal::OutOfRange));
    seq += 1;
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(27.0, 30.0),
                facing: north,
                z: 0.0,
            }),
            gait: Gait::Run,
            cast: Some(CastPress {
                seq,
                slot: 12,
                target: None,
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(refusal(&mut w), Some(Refusal::Moving));
    // A blow that lands, then another inside the global cooldown.
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), None);
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), Some(Refusal::NotReady));
    steps(&mut w, 40);
    assert_eq!(attempt(&mut w, &mut seq, 12, None, None), Some(Refusal::InCombat));
    w.heroes[0].stealthed = true;
    assert_eq!(attempt(&mut w, &mut seq, 74, brute, None), Some(Refusal::Aware));
    w.heroes[0].fight.power = 0.0;
    assert_eq!(attempt(&mut w, &mut seq, 2, brute, None), Some(Refusal::NoPower));
}

#[test]
fn a_mages_presses_are_refused_for_what_is_wrong() {
    let spawns = format!("{}{}", post("brute", 30.0, 30.0, 0.0), post("beast", 30.0, 50.0, 0.0));
    let mut w = world(&spawns);
    let mut b = flat(60.0);
    house(&mut b, V2::new(15.0, 45.0), V2::new(25.0, 55.0), None, true);
    w.attach_map(Arc::new(b.finish()));
    join(&mut w, 0, "mage", 10.0, 50.0);
    let (brute, beast) = (foe(&w, 0), foe(&w, 1));
    let mut seq = 0;
    assert_eq!(attempt(&mut w, &mut seq, 1, beast, None), Some(Refusal::NotInSight));
    stand(&mut w, 0, V2::new(10.0, 30.0), V2::new(1.0, 0.0), Gait::Still);
    w.step();
    assert_eq!(attempt(&mut w, &mut seq, 1, brute, None), None);
    assert_eq!(attempt(&mut w, &mut seq, 3, brute, None), Some(Refusal::Busy));
    steps(&mut w, 70);
    let me = Some(UnitRef::Hero(0));
    assert_eq!(attempt(&mut w, &mut seq, 6, me, None), Some(Refusal::BadTarget));
}

#[test]
fn a_channel_pays_up_front_and_ticks() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let target = foe(&w, 0);
    press(&mut w, 0, 1, 5, target, None);
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.power, 950.0);
    steps(&mut w, 95);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, target.unwrap()), vec![15.0, 10.0, 10.0]);
    assert!(events.contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 3,
        why: Stop::Finished
    }));
    assert!(w.hero(0).unwrap().fight.casting.is_none());
}

#[test]
fn a_cast_whose_target_dies_stops_and_costs_nothing() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    press(&mut w, 0, 1, 1, foe(&w, 0), None);
    w.step();
    w.kill_enemy(0, None);
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::TargetLost
    }));
    assert_eq!(w.hero(0).unwrap().fight.power, 1000.0);
}

#[test]
fn auto_attack_follows_a_melee_blow_and_stops_in_stealth() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 27.0, 30.0);
    press(&mut w, 0, 1, 2, foe(&w, 0), None);
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.attacking, Some(w.enemies[0].id));
    steps(&mut w, 70);
    let swings = w
        .drain_events()
        .iter()
        .filter(|e| matches!(e, Event::Swing { by: UnitRef::Hero(0), .. }))
        .count();
    assert!(swings >= 2, "{swings}");
    w.heroes[0].stealthed = true;
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.attacking, None);
}

#[test]
fn nothing_comes_back_in_combat_but_energy_does() {
    let mut w = world_with("regen: (10.0, 10.0)", &post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.heroes[0].hp = 500.0;
    w.heroes[0].fight.power = 0.0;
    steps(&mut w, 30);
    assert!((w.heroes[0].hp - 510.0).abs() < 0.5, "{}", w.heroes[0].hp);
    assert!((w.heroes[0].fight.power - 10.0).abs() < 0.5);
    w.alert(w.enemies[0].id, 0);
    w.step();
    let (hp, power) = (w.heroes[0].hp, w.heroes[0].fight.power);
    steps(&mut w, 30);
    assert!(w.heroes[0].hp <= hp);
    assert!(w.heroes[0].fight.power > power + 9.0);
    assert!(w.hero(0).unwrap().fight.in_combat);
}

#[test]
fn eating_heals_while_still_and_a_step_ends_it() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.heroes[0].hp = 400.0;
    press(&mut w, 0, 1, 12, None, None);
    w.step();
    steps(&mut w, 29);
    let healed = w.heroes[0].hp;
    assert!((healed - (400.0 + 1000.0 / 15.0)).abs() < 2.0, "{healed}");
    stand(&mut w, 0, V2::new(11.0, 30.0), V2::new(1.0, 0.0), Gait::Walk);
    w.step();
    assert_eq!(w.hero(0).unwrap().fight.eating, 0.0);
    let after = w.heroes[0].hp;
    steps(&mut w, 30);
    assert_eq!(w.heroes[0].hp, after);
}

#[test]
fn sprint_and_a_slowed_cast_set_the_pace() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    press(&mut w, 0, 1, 5, None, None);
    w.step();
    assert!((w.heroes[0].speed - 10.5).abs() < 1e-4);
    steps(&mut w, 241);
    assert!((w.heroes[0].speed - 7.0).abs() < 1e-4);
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    press(&mut w, 0, 1, 1, foe(&w, 0), None);
    w.step();
    assert!((w.heroes[0].speed - 3.5).abs() < 1e-4);
}

#[test]
fn stealth_cools_down_once_it_ends() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    sneak(&mut w, 1);
    w.step();
    assert!(w.heroes[0].stealthed);
    w.break_stealth(0);
    w.step();
    assert!(!w.heroes[0].stealthed);
    assert!(!w.may_stealth(0));
    sneak(&mut w, 2);
    w.step();
    assert!(!w.heroes[0].stealthed);
    steps(&mut w, 300);
    sneak(&mut w, 3);
    w.step();
    assert!(w.heroes[0].stealthed);
}
```

In `crates/nightfall-sim/src/combat/mod.rs`, below `mod effects;` add `pub(crate) mod heroes;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim combat::tests`
Expected: FAIL to compile (`no method named may_stealth`, `file not found for module heroes`).

- [ ] **Step 3: Implement heroes in the fight**

`crates/nightfall-sim/src/combat/heroes.rs`:

```rust
//! Heroes in the fight (spec §6.1): each new ability press taken once, the checks it must pass
//! (WoW's own refusals, sent back to its player), the global cooldown, cooldowns and power, casts
//! that take time and channels that tick, what a caster may do with their feet meanwhile,
//! auto-attack, eating, regeneration out of combat, Sprint, and Stealth's own cooldown.
//!
//! Clients own their position, so the host cannot hold a caster's feet: it lowers a slowed
//! caster's speed (which their controller takes, as stealth's does), roots a rooted one (the
//! client's `HeroFight::rooted`), and breaks any cast whose hero went further from where it began
//! than [`allowance`] allows.

use super::{
    allowance, in_range, stands_right, within_arc, CastPress, CastState, Casting, Cooldown,
    Event, Refusal, Stop, UnitRef, MANA,
};
use crate::{
    Aim, Effect, Facing, Gait, Moving, PlayerId, PlayerInput, World, CHEST_HEIGHT, DT, EYE_HEIGHT,
    V3,
};

/// Melee abilities' and auto-attack's range, yards (WoW's 5).
pub(crate) const MELEE: f32 = 5.0;
/// An ability reaching further than this needs a clear line to its target.
pub(crate) const NEEDS_SIGHT: f32 = 8.0;

/// A press that passed its checks: the ability, its enemy (an index), its spot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Aimed {
    pub(crate) ability: usize,
    pub(crate) enemy: Option<usize>,
    pub(crate) at: Option<V3>,
}

impl World {
    /// `player`'s presses in this step's input: whether they move (a rooted cast cannot begin on
    /// the move), and the latest ability press, once. The first input a host sees of a hero is a
    /// baseline, so a new run never casts the last run's press.
    pub(crate) fn take_presses(&mut self, player: PlayerId, input: &PlayerInput) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.moving = input.gait != Gait::Still;
        let first = !hc.started;
        hc.started = true;
        let last = hc.seq;
        if let Some(p) = input.cast {
            hc.seq = Some(p.seq);
        }
        if first {
            return;
        }
        if let Some(p) = input.cast.filter(|p| last != Some(p.seq)) {
            self.press(hi, p);
        }
    }

    /// A new press on hero `hi`'s bar: begun if it passes its checks, else refused to its player.
    fn press(&mut self, hi: usize, p: CastPress) {
        let player = self.heroes[hi].player;
        let content = self.content.clone();
        let kit = &content.classes[self.heroes[hi].class].kit;
        let Some(&(_, ability)) = kit.iter().find(|(s, _)| *s == p.slot) else {
            return;
        };
        // Stealth's button counts presses of its own (`PlayerInput::stealth_presses`).
        if content.abilities[ability].has(|e| matches!(e, Effect::Stealth)) {
            return;
        }
        match self.check(hi, ability, p.target, p.at) {
            Ok(aimed) => self.begin(hi, aimed),
            Err(why) => self.events.push(Event::Refused { player, why }),
        }
    }

    /// Whether hero `hi` may begin `ability` at `target` or `at`, in WoW's order of checks.
    pub(crate) fn check(
        &self,
        hi: usize,
        ability: usize,
        target: Option<UnitRef>,
        at: Option<V3>,
    ) -> Result<Aimed, Refusal> {
        let content = &self.content;
        let r = &content.rules.combat;
        let a = &content.abilities[ability];
        let hero = &self.heroes[hi];
        let f = &hero.fight;
        let moving = self.combat.hero(hero.player).is_some_and(|hc| hc.moving);
        if !f.up() {
            return Err(Refusal::Down);
        }
        if f.casting.is_some() {
            return Err(Refusal::Busy);
        }
        let cooling = f.cooldowns.iter().any(|c| usize::from(c.ability) == ability);
        if cooling || (a.gcd > 0.0 && f.gcd > 0.0) {
            return Err(Refusal::NotReady);
        }
        if f.power + 1e-3 < a.cost {
            return Err(Refusal::NoPower);
        }
        let rooted = a.moving == Moving::Rooted && (a.cast > 0.0 || a.channel > 0.0);
        if moving && (a.needs.still || rooted) {
            return Err(Refusal::Moving);
        }
        if a.needs.stealth && !hero.stealthed {
            return Err(Refusal::NeedStealth);
        }
        if a.needs.out_of_combat && f.in_combat {
            return Err(Refusal::InCombat);
        }
        if a.needs.combo && f.combo == 0 {
            return Err(Refusal::NeedCombo);
        }
        match a.aim {
            Aim::Me => Ok(Aimed {
                ability,
                enemy: None,
                at: None,
            }),
            Aim::Ground => {
                let at = at.filter(|p| p.is_finite()).ok_or(Refusal::NoTarget)?;
                if !in_range(hero.pos.distance(at.ground()), a.range, r) {
                    return Err(Refusal::OutOfRange);
                }
                Ok(Aimed {
                    ability,
                    enemy: None,
                    at: Some(at),
                })
            }
            Aim::Enemy => {
                let id = match target {
                    Some(UnitRef::Enemy(id)) => id,
                    Some(UnitRef::Hero(_)) => return Err(Refusal::BadTarget),
                    None => return Err(Refusal::NoTarget),
                };
                let ei = self
                    .enemies
                    .iter()
                    .position(|e| e.id == id)
                    .ok_or(Refusal::NoTarget)?;
                let e = &self.enemies[ei];
                let kind = content.enemies[e.kind].creature;
                if e.hp <= 0.0
                    || !(a.needs.creatures.is_empty() || a.needs.creatures.contains(&kind))
                {
                    return Err(Refusal::BadTarget);
                }
                if a.needs.combo && f.combo_on != Some(id) {
                    return Err(Refusal::NeedCombo);
                }
                if a.needs.unaware && !self.opener_allowed(id, hero.player) {
                    return Err(Refusal::Aware);
                }
                if !in_range(hero.pos.distance(e.pos), a.range, r) {
                    return Err(Refusal::OutOfRange);
                }
                if !stands_right(a.facing, hero.pos, hero.facing, e.pos, e.facing, r.front) {
                    return Err(if a.facing == Facing::Behind {
                        Refusal::Behind
                    } else {
                        Refusal::Facing
                    });
                }
                if !self.in_sight_of(hi, Some(ei), a.range) {
                    return Err(Refusal::NotInSight);
                }
                Ok(Aimed {
                    ability,
                    enemy: Some(ei),
                    at: None,
                })
            }
        }
    }

    /// Nothing solid between `a` and `b` (always, without a map).
    pub(crate) fn clear_line(&self, a: V3, b: V3) -> bool {
        self.map().is_none_or(|m| m.line_of_sight(a, b))
    }

    /// Whether hero `hi` has the clear line to enemy `ei` an ability of `range` needs: melee
    /// abilities need none.
    fn in_sight_of(&self, hi: usize, ei: Option<usize>, range: f32) -> bool {
        let Some(ei) = ei else {
            return true;
        };
        let (h, e) = (&self.heroes[hi], &self.enemies[ei]);
        range <= NEEDS_SIGHT
            || self.clear_line(
                V3::at(h.pos, h.z + EYE_HEIGHT),
                V3::at(e.pos, e.z + CHEST_HEIGHT),
            )
    }

    /// A checked press begins: the global cooldown, a meal ended, and then it goes off at once, or
    /// a cast or a channel starts (a channel pays as it starts).
    fn begin(&mut self, hi: usize, aimed: Aimed) {
        let content = self.content.clone();
        let a = &content.abilities[aimed.ability];
        let player = self.heroes[hi].player;
        let target = aimed.enemy.map(|ei| UnitRef::Enemy(self.enemies[ei].id));
        let ability = aimed.ability as u16;
        if a.gcd > 0.0 {
            self.heroes[hi].fight.gcd = a.gcd;
        }
        self.stop_eating(hi);
        let channel = a.channel > 0.0;
        if a.cast <= 0.0 && !channel {
            self.pay(hi, aimed);
            self.go(hi, aimed);
            return;
        }
        let total = if channel { a.channel } else { a.cast };
        let from = self.heroes[hi].pos;
        self.heroes[hi].fight.casting = Some(Casting {
            ability,
            left: total,
            total,
            channel,
            target,
        });
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].cast = Some(CastState {
            ability: aimed.ability,
            enemy: aimed.enemy,
            at: aimed.at,
            from,
            elapsed: 0.0,
            ticks: 0,
        });
        let by = UnitRef::Hero(player);
        self.events.push(if channel {
            Event::ChannelStart {
                by,
                ability,
                target,
                secs: total,
            }
        } else {
            Event::CastStart {
                by,
                ability,
                target,
                secs: total,
            }
        });
        if channel {
            self.pay(hi, aimed);
        }
    }

    /// What going off costs and makes: the power, the ability's own cooldown, its sound (at the
    /// spot, for a ground ability), a fire spell's flash, and stealth unless it keeps it.
    fn pay(&mut self, hi: usize, aimed: Aimed) {
        let content = self.content.clone();
        let a = &content.abilities[aimed.ability];
        let player = self.heroes[hi].player;
        let here = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
        let f = &mut self.heroes[hi].fight;
        f.power = (f.power - a.cost).max(0.0);
        if a.cooldown > 0.0 {
            f.cooldowns.push(Cooldown {
                ability: aimed.ability as u16,
                left: a.cooldown,
            });
        }
        if let Some(loud) = a.sound.loudness() {
            self.make_noise(player, aimed.at.unwrap_or(here), loud);
        }
        if a.flash {
            self.flash(player);
        }
        if !a.keeps_stealth {
            self.break_stealth(player);
        }
    }

    /// It goes off (or a channel ticks): it lands, and every client sees it.
    fn go(&mut self, hi: usize, aimed: Aimed) {
        let player = self.heroes[hi].player;
        let hits = self.land(player, aimed.ability, aimed.enemy, aimed.at);
        self.events.push(Event::CastGo {
            by: UnitRef::Hero(player),
            ability: aimed.ability as u16,
            hits,
            at: aimed.at,
        });
    }

    /// Hero `hi`'s cast ends without going off, or their channel ends.
    pub(crate) fn stop_cast(&mut self, hi: usize, why: Stop) {
        let player = self.heroes[hi].player;
        let Some(c) = self.heroes[hi].fight.casting.take() else {
            return;
        };
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].cast = None;
        self.events.push(Event::CastStop {
            by: UnitRef::Hero(player),
            ability: c.ability,
            why,
        });
    }

    /// Hero `hi` stops eating and drinking.
    pub(crate) fn stop_eating(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        self.heroes[hi].fight.eating = 0.0;
        if let Some(c) = self.combat.heroes.iter_mut().find(|c| c.player == player) {
            c.meal = None;
        }
    }

    /// Whether `player`'s hero may step into stealth now: up, out of combat, and Stealth not
    /// cooling down. Leaving stealth is always allowed.
    pub(crate) fn may_stealth(&self, player: PlayerId) -> bool {
        let Some(h) = self.hero(player) else {
            return false;
        };
        let cooling = self
            .stealth_of(h.class)
            .is_some_and(|s| h.fight.cooldowns.iter().any(|c| usize::from(c.ability) == s));
        h.fight.up() && !h.fight.in_combat && !cooling
    }

    /// The class's Stealth ability, if its bar has one.
    fn stealth_of(&self, class: usize) -> Option<usize> {
        self.content.classes[class]
            .kit
            .iter()
            .map(|&(_, a)| a)
            .find(|&a| self.content.abilities[a].has(|e| matches!(e, Effect::Stealth)))
    }

    /// How much faster Sprint makes hero `hi` this step.
    fn sprint_by(&self, hi: usize) -> f32 {
        let h = &self.heroes[hi];
        if h.fight.sprint > 0.0 {
            self.combat.hero(h.player).map_or(0.0, |c| c.sprint_by)
        } else {
            0.0
        }
    }

    /// Every hero's fight moves on a step: their clocks and power, their cast, their swing, their
    /// meal, and their pace.
    pub(crate) fn step_fighters(&mut self) {
        for hi in 0..self.heroes.len() {
            self.tick_hero(hi);
            self.step_cast(hi);
            self.swing(hi);
            self.step_meal(hi);
            self.set_pace(hi);
        }
    }

    /// The global cooldown, cooldowns, Sprint and power; in combat or not (an enemy after them,
    /// or a blow lately); Stealth's cooldown once stealth ends; regeneration out of combat.
    fn tick_hero(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let player = self.heroes[hi].player;
        let hunted = self.senses.hunted(player);
        let stealth = self.stealth_of(self.heroes[hi].class);
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.combat_for = (hc.combat_for - DT).max(0.0);
        let lingering = hc.combat_for > 0.0;
        let was_stealthed = std::mem::replace(&mut hc.was_stealthed, self.heroes[hi].stealthed);
        let hero = &mut self.heroes[hi];
        let class = &content.classes[hero.class];
        let up = hero.fight.up();
        let f = &mut hero.fight;
        f.gcd = (f.gcd - DT).max(0.0);
        for c in &mut f.cooldowns {
            c.left -= DT;
        }
        f.cooldowns.retain(|c| c.left > 1e-4);
        f.sprint = (f.sprint - DT).max(0.0);
        f.in_combat = up && (hunted || lingering);
        f.power = (f.power + class.power_regen * DT).min(f.max_power);
        // Stealth's own cooldown starts as stealth ends (WoW's 10 seconds).
        if let (true, false, Some(s)) = (was_stealthed, hero.stealthed, stealth) {
            let cd = content.abilities[s].cooldown;
            f.cooldowns.retain(|c| usize::from(c.ability) != s);
            if cd > 0.0 {
                f.cooldowns.push(Cooldown {
                    ability: s as u16,
                    left: cd,
                });
            }
        }
        if up && !f.in_combat {
            if class.power == MANA {
                f.power = (f.power + r.regen.1 * DT).min(f.max_power);
            }
            hero.hp = (hero.hp + r.regen.0 * DT).min(hero.max_hp);
        }
    }

    /// Hero `hi`'s cast moves on: broken by moving further than it allows, or by its target dying;
    /// a channel ticks; a cast goes off at its end, if its target is still in sight.
    fn step_cast(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let Some(mut cs) = self.combat.heroes[ci].cast else {
            return;
        };
        let content = self.content.clone();
        let a = &content.abilities[cs.ability];
        cs.elapsed += DT;
        let run = self.heroes[hi].speed * (1.0 + self.sprint_by(hi));
        let allowed = allowance(a.moving, run, cs.elapsed, &content.rules.combat);
        if self.heroes[hi].pos.distance(cs.from) > allowed {
            return self.stop_cast(hi, Stop::Moved);
        }
        if cs.enemy.is_some_and(|ei| self.enemies[ei].hp <= 0.0) {
            return self.stop_cast(hi, Stop::TargetLost);
        }
        if let Some(c) = &mut self.heroes[hi].fight.casting {
            c.left = (c.total - cs.elapsed).max(0.0);
        }
        let aimed = Aimed {
            ability: cs.ability,
            enemy: cs.enemy,
            at: cs.at,
        };
        if a.channel > 0.0 {
            let every = a.channel / a.ticks as f32;
            while cs.ticks < a.ticks && cs.elapsed + 1e-4 >= every * (cs.ticks + 1) as f32 {
                cs.ticks += 1;
                self.go(hi, aimed);
            }
            self.combat.heroes[ci].cast = Some(cs);
            if cs.ticks >= a.ticks {
                self.stop_cast(hi, Stop::Finished);
            }
        } else if cs.elapsed + 1e-4 >= a.cast {
            // WoW checks range only as a cast begins; at its end the target must still be seen.
            if !self.in_sight_of(hi, aimed.enemy, a.range) {
                return self.stop_cast(hi, Stop::TargetLost);
            }
            self.heroes[hi].fight.casting = None;
            self.combat.heroes[ci].cast = None;
            self.pay(hi, aimed);
            self.go(hi, aimed);
        } else {
            self.combat.heroes[ci].cast = Some(cs);
        }
    }

    /// Hero `hi`'s auto-attack: a blow every `swing.every` seconds at the enemy they attack, while
    /// it lives, stands in melee range in front of them, and they are out of stealth and not
    /// casting. Like any blow, the first from an unseen hero is an ambush.
    fn swing(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.swing_in = (hc.swing_in - DT).max(0.0);
        let ready = hc.swing_in <= 0.0;
        let hero = &self.heroes[hi];
        let Some(id) = hero.fight.attacking else {
            return;
        };
        let target = self.enemies.iter().position(|e| e.id == id && e.hp > 0.0);
        let blow = content.classes[hero.class].swing;
        let keep = blow.is_some() && target.is_some() && !hero.stealthed && hero.fight.up();
        if !keep {
            self.heroes[hi].fight.attacking = None;
            return;
        }
        let (Some(blow), Some(ei)) = (blow, target) else {
            return;
        };
        let (hero, e) = (&self.heroes[hi], &self.enemies[ei]);
        let close = in_range(hero.pos.distance(e.pos), MELEE, r)
            && within_arc(hero.facing, e.pos - hero.pos, r.front);
        if !ready || !close || hero.fight.casting.is_some() {
            return;
        }
        self.combat.heroes[ci].swing_in = blow.every;
        let ambush = self.ambush(id, player);
        let amount = self.combat.dice.between(blow.min, blow.max) * ambush;
        self.events.push(Event::Swing {
            by: UnitRef::Hero(player),
            to: UnitRef::Enemy(id),
        });
        self.strike(player, ei, amount, ambush > 1.0, false, true);
        if self.enemies[ei].hp > 0.0 {
            self.alert(id, player);
        }
    }

    /// Hero `hi`'s meal: health and mana back a second while they sit still out of combat, until
    /// it is eaten.
    fn step_meal(&mut self, hi: usize) {
        let player = self.heroes[hi].player;
        let ci = self.combat.hero_index(player);
        let Some(meal) = self.combat.heroes[ci].meal else {
            return;
        };
        let moving = self.combat.heroes[ci].moving;
        let hero = &mut self.heroes[hi];
        hero.fight.eating = (hero.fight.eating - DT).max(0.0);
        if moving || hero.fight.in_combat || hero.fight.eating <= 0.0 || !hero.fight.up() {
            return self.stop_eating(hi);
        }
        hero.hp = (hero.hp + meal.health * DT).min(hero.max_hp);
        hero.fight.power = (hero.fight.power + meal.mana * DT).min(hero.fight.max_power);
    }

    /// Hero `hi`'s pace this step: the senses' speed (stealth's) times Sprint, times
    /// `cast_move_speed` while casting a slowed ability; feet held for a rooted one.
    fn set_pace(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let sprint = 1.0 + self.sprint_by(hi);
        let hero = &mut self.heroes[hi];
        let moving = hero
            .fight
            .casting
            .map(|c| content.abilities[usize::from(c.ability)].moving);
        let cast = if moving == Some(Moving::Slowed) {
            r.cast_move_speed
        } else {
            1.0
        };
        hero.speed *= sprint * cast;
        hero.fight.rooted = moving == Some(Moving::Rooted);
    }
}
```

In `crates/nightfall-sim/src/senses/mod.rs`:

- `take_input` gains a last parameter `may_stealth: bool` (its doc adds: "Stepping into stealth also needs `may_stealth`: not in combat, Stealth not cooling down; stepping out is always allowed."), and its toggle becomes:

```rust
    if pressed % 2 == 1 && content.classes[hero.class].stealth && (hero.stealthed || may_stealth) {
        hero.stealthed = !hero.stealthed;
    }
```

- Add to `impl Senses`:

```rust
    /// Whether any enemy is alerted to `player`'s hero: they are in combat (spec §6.1).
    pub(crate) fn hunted(&self, player: PlayerId) -> bool {
        self.minds.iter().any(|m| m.alerted_to(player))
    }
```

In `crates/nightfall-sim/src/world.rs`, replace `step` with:

```rust
    /// One step: each hero takes its player's input (the gait and Stealth presses first, then the
    /// pose, kept inside the map, a pose that is not a number dropped so a broken client cannot
    /// poison the world), then the presses of the fight; every hero's senses are worked out and
    /// their fight moves on; once a map is attached, every enemy goes about its business.
    pub fn step(&mut self) {
        let content = self.content.clone();
        let inputs: Vec<(PlayerId, PlayerInput)> = self.inputs.drain(..).collect();
        for (player, input) in &inputs {
            let may_stealth = self.may_stealth(*player);
            let Some(hero) = self.heroes.iter_mut().find(|h| h.player == *player) else {
                continue;
            };
            senses::take_input(&mut self.senses, hero, input, &content, may_stealth);
            let Some(pose) = input.pose else { continue };
            if !pose.pos.is_finite() || !pose.facing.is_finite() {
                continue;
            }
            hero.pos = content.map.bounds.clamp(pose.pos);
            if pose.z.is_finite() {
                // On the host, the floor nearest the reported height, so a hero on a stair sees
                // from where they stand; a height with no floor near it (a jump) is kept as it is.
                let pos = hero.pos;
                hero.z = self
                    .nav
                    .as_ref()
                    .and_then(|nav| nav.map().floor_within(pos, pose.z, HERO_FLOOR_REACH))
                    .unwrap_or(pose.z);
            }
            let facing = pose.facing.normalize_or_zero();
            if facing != V2::ZERO {
                hero.facing = facing;
            }
        }
        for (player, input) in &inputs {
            self.take_presses(*player, input);
        }
        senses::step_heroes(&mut self.senses, &mut self.heroes, &self.content);
        self.step_fighters();
        if let Some(nav) = &mut self.nav {
            senses::step_enemies(
                &mut self.senses,
                &mut self.enemies,
                &mut self.brains,
                &self.heroes,
                nav,
                &self.content,
            );
            enemies::step(
                &mut self.enemies,
                &mut self.brains,
                nav,
                &self.content,
                &mut self.warnings,
                &mut self.plan_cursor,
            );
        }
        // Noises are heard the step they are made, or the next; none waits longer.
        self.senses.noises.clear();
        self.elapsed += DT;
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test, the senses' Stealth tests unchanged, and the 13 new ones in `combat::tests`).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: presses taken once, WoW's refusals, GCD, power, casts and channels held to their feet, auto-attack

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Enemies held and hurt over time; what their senses may do

**Files:**
- Create: `crates/nightfall-sim/src/combat/hold.rs`
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (module), `crates/nightfall-sim/src/combat/tests.rs`, `crates/nightfall-sim/src/senses/mod.rs` (`target_of`), `crates/nightfall-sim/src/senses/react.rs` (`step_enemies`, `react`), `crates/nightfall-sim/src/world.rs` (`step`)

**Interfaces:**
- Consumes: `EnemyCombat`, `Held`, `Slowed`, `Burn`, `EnemyCast`, `Hold`, `Engage`, `EnemyFight`, `Casting` (Task 3); `World::{strike, clear_line}`, `heroes::NEEDS_SIGHT` (Tasks 4–5); `enemies::follow`, `HeightMap::walk_line`, `Mind`, `State`, `react::{stand, go}` (earlier plans).
- Produces:
  - Crate-only `World::hold_enemies(&mut self)`: every enemy's holds, slows, distractions and burns tick; a sheep wanders; `Enemy::fight` is written; `EnemyCombat::engage` is set.
  - Crate-only `Senses::target_of(&self, i: usize) -> Option<(PlayerId, V3)>`.
  - `senses::step_enemies(…, engage: &[Engage])`: `Out` enemies neither look nor think nor move; `Still` ones stand turned to their hero; `Facing(spot)` ones stand turned to it while calm or wary; a chase stops `engage.reach` short at `engage.speed` of its pace.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/combat/tests.rs`:

```rust
#[test]
fn a_stunned_enemy_stands_and_a_rooted_one_turns_but_stays() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 20.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    w.land(0, ability(&w, "cheap"), Some(0), None);
    w.heroes[0].fight.attacking = None;
    steps(&mut w, 30);
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
    assert_eq!(w.enemies[0].fight.control, Some(ControlKind::Stun));
    steps(&mut w, 100);
    assert_eq!(w.enemies[0].fight.control, None);
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 30.0, 22.0);
    w.alert(w.enemies[0].id, 0);
    w.land(0, ability(&w, "nova"), None, None);
    steps(&mut w, 30);
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
    let f = w.enemies[0].facing;
    assert!(f.y < -0.9, "{f:?}");
}

#[test]
fn a_sapped_enemy_neither_sees_nor_hears_until_it_wakes() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.land(0, ability(&w, "sap"), Some(0), None);
    // Running loudly about in front of it.
    let run_in_front = |w: &mut World, k: usize| {
        let at = V2::new(35.0 + (k % 20) as f32 * 0.2, 30.0);
        stand(w, 0, at, V2::new(1.0, 0.0), Gait::Run);
        w.step();
    };
    for k in 0..300 {
        run_in_front(&mut w, k);
    }
    assert!(w.marks_for(0).is_empty());
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
    for k in 0..700 {
        run_in_front(&mut w, k);
    }
    assert!(!w.marks_for(0).is_empty());
}

#[test]
fn bleeds_and_burns_tick_their_count_and_a_silent_one_is_not_heard() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.land(0, ability(&w, "garrote"), Some(0), None);
    w.heroes[0].fight.attacking = None;
    steps(&mut w, 18 * 30 + 5);
    let ticks = damage_to(&w.drain_events(), foe(&w, 0).unwrap());
    assert_eq!(ticks, vec![30.0; 6]);
    assert_eq!(w.enemies[0].hp, 1000.0 - 180.0);
    assert_eq!(w.hero(0).unwrap().noise, 0.0);
    assert!(w.enemies[0].fight.auras.is_empty());
}

#[test]
fn a_sheep_wanders_near_where_it_was_turned() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.land(0, ability(&w, "sheep"), Some(0), None);
    let start = w.enemies[0].pos;
    let mut furthest: f32 = 0.0;
    for _ in 0..300 {
        w.step();
        furthest = furthest.max(w.enemies[0].pos.distance(start));
    }
    assert!(furthest > 0.5 && furthest <= 4.5, "{furthest}");
    assert_eq!(w.enemies[0].fight.control, Some(ControlKind::Polymorph));
    assert_eq!(w.enemies[0].fight.auras, vec![118]);
}

#[test]
fn a_slowed_enemy_chases_slower() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    w.combat.enemies[0].slow = Some(Slowed {
        by: 0.4,
        left: 7.0,
        spell: 116,
    });
    steps(&mut w, 10);
    assert!((w.enemies[0].speed - 7.0 * 0.6).abs() < 0.05, "{}", w.enemies[0].speed);
    assert!(w.enemies[0].fight.slowed);
    assert_eq!(w.enemies[0].fight.auras, vec![116]);
}

#[test]
fn a_distracted_enemy_faces_the_spot() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.land(0, ability(&w, "distract"), None, Some(V3::new(30.0, 38.0, 0.0)));
    steps(&mut w, 5);
    let f = w.enemies[0].facing;
    assert!(f.y > 0.9, "{f:?}");
    assert_eq!(w.enemies[0].pos, V2::new(30.0, 30.0));
}

#[test]
fn a_caster_keeps_its_distance_while_it_sees_its_hero() {
    let mut w = world(&post("caster", 50.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 150);
    let d = w.enemies[0].pos.distance(V2::new(10.0, 30.0));
    assert!((27.0..=29.0).contains(&d), "{d}");
}

#[test]
fn a_dead_enemy_stays_where_it_fell() {
    let patrol =
        r#"(place: "Road", kind: "brute", behaviour: Patrol(route: [(20.0, 20.0), (40.0, 20.0)])),"#;
    let mut w = world(patrol);
    join(&mut w, 0, "mage", 5.0, 50.0);
    steps(&mut w, 30);
    w.kill_enemy(0, Some(0));
    let at = w.enemies[0].pos;
    steps(&mut w, 60);
    assert_eq!(w.enemies[0].pos, at);
    assert_eq!(w.enemies[0].speed, 0.0);
}
```

In `crates/nightfall-sim/src/combat/mod.rs`, below `pub(crate) mod heroes;` add `mod hold;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim combat::tests`
Expected: FAIL to compile (`file not found for module hold`); once it compiles, the new tests fail (a stunned enemy walks, a slowed one runs at full speed).

- [ ] **Step 3: Implement the holds**

`crates/nightfall-sim/src/combat/hold.rs`:

```rust
//! Enemies held and hurt over time (spec §6.2, §6.3): holds running out (a stun, a sap, a sheep, a
//! root), slows and distractions, bleeds and burns ticking, a sheep wandering where it was
//! turned, what is on each enemy written for the clients ([`EnemyFight`]), and what its senses may
//! do each step ([`Engage`]): look, think and move, stand, or nothing at all; how close it closes
//! on its hero (a caster keeps its distance while it can see them); and how fast.

use super::heroes::NEEDS_SIGHT;
use super::{Casting, EnemyFight, Engage, Event, Hold, Stop, UnitRef};
use crate::{Aim, Content, ControlKind, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

/// Yards inside its longest spell's range a caster stops.
const KEEP_INSIDE: f32 = 2.0;
/// Yards around where it was turned a sheep wanders.
const SHEEP_ROAM: f32 = 4.0;
/// Within this many yards of its spot a sheep picks another.
const SHEEP_THERE: f32 = 0.3;

impl World {
    /// Every enemy's holds, slows, distractions, bleeds and burns move on a step; a sheep
    /// wanders; what is on each is written for the clients; and what its senses may do is set.
    pub(crate) fn hold_enemies(&mut self) {
        let content = self.content.clone();
        for ei in 0..self.enemies.len().min(self.combat.enemies.len()) {
            self.tick_holds(ei);
            self.tick_burns(ei);
            self.wander(ei, &content);
            let engage = self.engage(ei, &content);
            if engage.hold == Hold::Out && !self.is_sheep(ei) {
                self.enemies[ei].speed = 0.0;
            }
            self.combat.enemies[ei].engage = engage;
            let fight = self.enemy_fight(ei);
            self.enemies[ei].fight = fight;
        }
    }

    fn is_sheep(&self, ei: usize) -> bool {
        self.combat.enemies[ei]
            .held
            .is_some_and(|h| h.kind == ControlKind::Polymorph)
    }

    /// Holds, slows and distractions run down; a distraction ends once the enemy is after
    /// someone; a hold that takes it out of the fight stops its cast, and the blow it was about to
    /// land on the ground.
    fn tick_holds(&mut self, ei: usize) {
        let alerted = self.senses.alerted(ei);
        let id = self.enemies[ei].id;
        let ec = &mut self.combat.enemies[ei];
        if let Some(h) = &mut ec.held {
            h.left -= DT;
            if h.left <= 1e-4 {
                ec.held = None;
                ec.wander = None;
            }
        }
        if let Some(s) = &mut ec.slow {
            s.left -= DT;
            if s.left <= 1e-4 {
                ec.slow = None;
            }
        }
        if let Some((_, left)) = &mut ec.distracted {
            *left -= DT;
            if *left <= 1e-4 || alerted {
                ec.distracted = None;
            }
        }
        if ec.held.is_some_and(|h| h.kind.out()) {
            if let Some(c) = ec.cast.take() {
                if let Some(t) = c.telegraph {
                    self.telegraphs.retain(|x| x.id != t);
                }
                self.events.push(Event::CastStop {
                    by: UnitRef::Enemy(id),
                    ability: c.ability as u16,
                    why: Stop::Interrupted,
                });
            }
        }
    }

    /// Bleeds and burns on enemy `ei` tick (never a crit, as in WoW), and run out.
    fn tick_burns(&mut self, ei: usize) {
        if self.enemies[ei].hp <= 0.0 {
            self.combat.enemies[ei].burns.clear();
            return;
        }
        let mut due = Vec::new();
        for b in &mut self.combat.enemies[ei].burns {
            b.next -= DT;
            if b.next <= 1e-4 {
                b.next += b.every;
                b.left -= b.every;
                due.push((b.by, b.amount, b.silent));
            }
        }
        self.combat.enemies[ei].burns.retain(|b| b.left > 1e-4);
        for (by, amount, silent) in due {
            self.strike(by, ei, amount, false, silent, false);
        }
    }

    /// A sheep walks to a spot within [`SHEEP_ROAM`] of where it was turned, then another,
    /// never through a wall.
    fn wander(&mut self, ei: usize, content: &Content) {
        if !self.is_sheep(ei) {
            return;
        }
        let Some(map) = self.map().cloned() else {
            return;
        };
        let speed = content.rules.combat.sheep_speed;
        let e = &mut self.enemies[ei];
        let ec = &mut self.combat.enemies[ei];
        let (home, mut goal) = ec.wander.unwrap_or((e.pos, e.pos));
        if goal.distance(e.pos) <= SHEEP_THERE {
            let angle = self.combat.dice.unit() * std::f32::consts::TAU;
            let far = self.combat.dice.unit() * SHEEP_ROAM;
            goal = home + V2::from_angle(angle, far);
        }
        let (here, to) = (V3::at(e.pos, e.z), V3::at(goal, e.z));
        if !map.walk_line(here, to) {
            ec.wander = Some((home, e.pos));
            e.speed = 0.0;
            return;
        }
        let mut next = 0;
        crate::enemies::follow(e, &[to], &mut next, speed, &map);
        ec.wander = Some((home, goal));
    }

    /// What enemy `ei`'s senses may do this step.
    fn engage(&self, ei: usize, content: &Content) -> Engage {
        let (e, ec) = (&self.enemies[ei], &self.combat.enemies[ei]);
        let hold = if e.hp <= 0.0 {
            Hold::Out
        } else if let Some(h) = ec.held {
            if h.kind.out() {
                Hold::Out
            } else {
                Hold::Still
            }
        } else if ec.cast.is_some() {
            Hold::Still
        } else if let Some((spot, _)) = ec.distracted {
            Hold::Facing(spot)
        } else {
            Hold::Free
        };
        Engage {
            hold,
            reach: self
                .keep_at(ei, content)
                .unwrap_or(content.rules.senses.chase_reach),
            speed: 1.0 - ec.slow.map_or(0.0, |s| s.by),
        }
    }

    /// How far a caster stands off the hero it is after: its longest spell's range, a little
    /// inside, while it has a clear line to them; `None` for one that is no caster, or blind to
    /// its hero (it then closes in until it sees them).
    fn keep_at(&self, ei: usize, content: &Content) -> Option<f32> {
        let e = &self.enemies[ei];
        let range = content.enemies[e.kind]
            .kit
            .iter()
            .map(|&(a, _)| &content.abilities[a])
            .filter(|a| a.aim == Aim::Enemy && a.telegraph.is_none() && a.range > NEEDS_SIGHT)
            .map(|a| a.range)
            .fold(0.0f32, f32::max);
        if range <= 0.0 {
            return None;
        }
        let (player, _) = self.senses.target_of(ei)?;
        let h = self.hero(player)?;
        let sees = self.clear_line(
            V3::at(e.pos, e.z + EYE_HEIGHT),
            V3::at(h.pos, h.z + CHEST_HEIGHT),
        );
        sees.then_some((range - KEEP_INSIDE).max(content.rules.senses.chase_reach))
    }

    /// What every client sees of enemy `ei`'s fight.
    fn enemy_fight(&self, ei: usize) -> EnemyFight {
        let ec = &self.combat.enemies[ei];
        let mut auras: Vec<u32> = Vec::new();
        let spells = ec
            .held
            .map(|h| h.spell)
            .into_iter()
            .chain(ec.slow.map(|s| s.spell))
            .chain(ec.burns.iter().map(|b| b.spell));
        for s in spells {
            if !auras.contains(&s) {
                auras.push(s);
            }
        }
        EnemyFight {
            control: ec.held.map(|h| h.kind),
            slowed: ec.slow.is_some(),
            casting: ec.cast.map(|c| Casting {
                ability: c.ability as u16,
                left: c.left.max(0.0),
                total: c.total,
                channel: false,
                target: Some(UnitRef::Hero(c.player)),
            }),
            auras,
        }
    }
}
```

In `crates/nightfall-sim/src/senses/mod.rs`, change `use awareness::{Mind, Shout};` to `use awareness::{Mind, Shout, State};` and add to `impl Senses`:

```rust
    /// The hero enemy `i` is after, and where it last saw them, when it is alerted.
    pub(crate) fn target_of(&self, i: usize) -> Option<(PlayerId, V3)> {
        match self.minds.get(i)?.state {
            State::Alerted {
                player, last_seen, ..
            } => Some((player, last_seen)),
            _ => None,
        }
    }
```

In `crates/nightfall-sim/src/senses/react.rs`:

- Add `use crate::combat::{Engage, Hold};`.
- The module doc's last sentence becomes: "Calm again, its brain finds its way back to its order. What holds it in the fight (`crate::combat`) comes as an [`Engage`] each step: out of the fight it neither looks, thinks nor moves; held still, it stands turned to its hero; distracted, it stands turned to the spot while calm or wary; and a chase stops the engage's reach short, at its speed."
- `step_enemies` gains a last parameter `engage: &[Engage]`.
- In its look-and-think loop, the first line of the loop body becomes:

```rust
        if engage.get(i).is_some_and(|g| g.hold == Hold::Out) {
            continue;
        }
```

- In the shouts loop, the ally condition gains `&& engage.get(j).is_none_or(|g| g.hold != Hold::Out)`.
- Replace the act loop (`for (i, e) in enemies.iter_mut().enumerate() { … }`) with:

```rust
    for (i, e) in enemies.iter_mut().enumerate() {
        let g = engage
            .get(i)
            .copied()
            .unwrap_or(Engage::free(r.chase_reach));
        let mind = &mut s.minds[i];
        let held = mind.state != State::Calm || g.hold != Hold::Free;
        brains[i].hold(held);
        match g.hold {
            // The fight moves it (a sheep) or nothing does.
            Hold::Out => continue,
            Hold::Still => {
                e.speed = 0.0;
                if let State::Alerted { last_seen, .. } = mind.state {
                    stand(e, Some(last_seen.ground()));
                }
                continue;
            }
            Hold::Facing(spot) if matches!(mind.state, State::Calm | State::Wary { .. }) => {
                stand(e, Some(spot));
                continue;
            }
            _ => {}
        }
        if !held {
            s.ways[i] = Way::default();
            e.asleep = mind.sleeper && brains[i].at_post(e.pos);
            continue;
        }
        e.asleep = false;
        react(e, mind, &mut s.ways[i], &map, nav, content, &mut searches, g);
    }
```

- `react` gains a last parameter `g: Engage`; its two `go(e, way, place, def.walk_speed, ARRIVE, …)` and `go(e, way, last_seen, def.run_speed, r.chase_reach, …)` calls become `go(e, way, place, def.walk_speed * g.speed, ARRIVE, …)` and `go(e, way, last_seen, def.run_speed * g.speed, g.reach, …)`.

In `crates/nightfall-sim/src/world.rs`'s `step`, replace the `if let Some(nav) = &mut self.nav { … }` block with:

```rust
        if self.nav.is_some() {
            self.hold_enemies();
        }
        if let Some(nav) = &mut self.nav {
            let engage: Vec<Engage> = self.combat.enemies.iter().map(|c| c.engage).collect();
            senses::step_enemies(
                &mut self.senses,
                &mut self.enemies,
                &mut self.brains,
                &self.heroes,
                nav,
                &self.content,
                &engage,
            );
            enemies::step(
                &mut self.enemies,
                &mut self.brains,
                nav,
                &self.content,
                &mut self.warnings,
                &mut self.plan_cursor,
            );
        }
```

and add `Engage` to its `crate::combat::{…}` import.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test, the senses' and world knowledge's enemy tests unchanged, and the 8 new ones in `combat::tests`).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: enemies held, slowed, distracted and bled; sheep wander; casters keep their distance

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Heroes falling: downed, crawling, revived, dead

**Files:**
- Create: `crates/nightfall-sim/src/combat/falling.rs`
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (module), `crates/nightfall-sim/src/combat/heroes.rs` (`step_fighters`, `set_pace`), `crates/nightfall-sim/src/combat/tests.rs`, `crates/nightfall-sim/src/senses/react.rs` (`sense`)

**Interfaces:**
- Consumes: `Life`, `Event`, `Stop`, `UnitRef` (Task 3); `World::{stop_cast, stop_eating}` (Task 5); `CombatRules::{downed_for, revive_for, revive_reach, revive_health, crawl_speed, combat_linger}` (Task 1); `World::{break_stealth, make_noise}`, `Loudness::Normal`, `react::sense` (senses).
- Produces:
  - `World::hurt_hero(&mut self, player: PlayerId, amount: f32)` (public: a fall, a trap or a test hook hurts a hero through it); crate-only `World::{wound(&mut self, PlayerId, f32, Option<UnitRef>), step_fall(&mut self, hi: usize)}`.
  - A downed hero crawls at `crawl_speed`; a dead one is rooted with no speed; the senses neither see nor hear a hero who is not up.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/combat/tests.rs`:

```rust
#[test]
fn a_hero_at_zero_is_downed_and_crawls() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.heroes[0].stealthed = true;
    w.hurt_hero(0, 5000.0);
    let h = w.hero(0).unwrap();
    assert!(matches!(h.fight.life, Life::Downed { .. }));
    assert_eq!(h.hp, 0.0);
    assert!(!h.stealthed);
    assert!(w.drain_events().contains(&Event::Downed { player: 0 }));
    w.step();
    assert!((w.heroes[0].speed - 7.0 * 0.3).abs() < 1e-4);
}

#[test]
fn a_teammate_standing_by_revives_in_five_seconds_with_a_noise() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 12.0, 30.0);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 2);
    assert_eq!(w.hero(1).unwrap().noise, 20.0);
    steps(&mut w, 149);
    let h = w.hero(0).unwrap();
    assert!(h.fight.up());
    assert!((h.hp - 1000.0 * 0.3).abs() < 1e-3);
    assert!(w.drain_events().contains(&Event::Revived { player: 0, by: 1 }));
}

#[test]
fn walking_away_starts_the_revive_over() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 10.0, 30.0);
    join(&mut w, 1, "mage", 12.0, 30.0);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 120);
    stand(&mut w, 1, V2::new(30.0, 30.0), V2::new(1.0, 0.0), Gait::Run);
    w.step();
    assert!(matches!(
        w.hero(0).unwrap().fight.life,
        Life::Downed { revive, .. } if revive == 0.0
    ));
    stand(&mut w, 1, V2::new(12.0, 30.0), V2::new(1.0, 0.0), Gait::Run);
    steps(&mut w, 120);
    assert!(!w.hero(0).unwrap().fight.up());
    steps(&mut w, 40);
    assert!(w.hero(0).unwrap().fight.up());
}

#[test]
fn unrevived_for_45_seconds_is_dead_and_cannot_press() {
    let mut w = world(&post("brute", 50.0, 50.0, 0.0));
    join(&mut w, 0, "rogue", 10.0, 30.0);
    w.hurt_hero(0, 5000.0);
    press(&mut w, 0, 1, 2, foe(&w, 0), None);
    w.step();
    assert!(w.drain_events().contains(&Event::Refused {
        player: 0,
        why: Refusal::Down
    }));
    steps(&mut w, 45 * 30);
    assert_eq!(w.hero(0).unwrap().fight.life, Life::Dead);
    assert!(w.drain_events().contains(&Event::Died { player: 0 }));
    assert!(w.hero(0).unwrap().fight.rooted);
    assert_eq!(w.hero(0).unwrap().speed, 0.0);
    w.hurt_hero(0, 10.0);
    assert!(w.drain_events().is_empty());
}

#[test]
fn an_enemy_leaves_a_downed_hero_alone() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    steps(&mut w, 5);
    w.hurt_hero(0, 5000.0);
    steps(&mut w, 6 * 30);
    let alerted = Mark {
        enemy: id,
        alert: Alert::Alerted,
    };
    assert!(!w.marks_for(0).contains(&alerted));
    steps(&mut w, 25 * 30);
    assert!(w.marks_for(0).is_empty());
}
```

In `crates/nightfall-sim/src/combat/mod.rs`, below `mod hold;` add `mod falling;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim combat::tests`
Expected: FAIL to compile (`no method named hurt_hero`, `file not found for module falling`).

- [ ] **Step 3: Implement falling**

`crates/nightfall-sim/src/combat/falling.rs`:

```rust
//! Heroes falling (spec §8.1): at 0 health a hero is downed for `downed_for` seconds and crawls; a
//! teammate up and standing by them for `revive_for` seconds gets them up (a 20-yard noise as the
//! revive begins), with `revive_health` of their health; after that they are dead, the body where
//! it fell (sub-project 5 leaves their gear on it). Enemies leave the downed and the dead be:
//! the senses neither see nor hear them, so an enemy after them loses them and searches, which
//! gives a friend a chance.

use super::{Event, Life, Stop, UnitRef};
use crate::{Loudness, PlayerId, World, DT, V3};

impl World {
    /// `player`'s hero takes `amount` damage from nowhere in particular (a fall, a trap, a test
    /// hook); an enemy's blow goes through [`World::wound`] with its maker.
    pub fn hurt_hero(&mut self, player: PlayerId, amount: f32) {
        self.wound(player, amount, None);
    }

    /// `player`'s hero takes `amount` damage `by` something: it breaks their stealth (spec §6.3),
    /// ends their meal, keeps them in combat, and at 0 health downs them. Nothing happens to a
    /// hero who is not up.
    pub(crate) fn wound(&mut self, player: PlayerId, amount: f32, by: Option<UnitRef>) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        if !self.heroes[hi].fight.up() || !(amount > 0.0) {
            return;
        }
        let linger = self.content.rules.combat.combat_linger;
        let h = &mut self.heroes[hi];
        h.hp = (h.hp - amount).max(0.0);
        let down = h.hp <= 0.0;
        self.events.push(Event::Damage {
            to: UnitRef::Hero(player),
            by,
            amount,
            crit: false,
            ambush: false,
        });
        self.break_stealth(player);
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].combat_for = linger;
        self.stop_eating(hi);
        if down {
            self.down(hi);
        }
    }

    /// Hero `hi` goes down: their cast stops, they leave stealth and stop swinging.
    fn down(&mut self, hi: usize) {
        let downed_for = self.content.rules.combat.downed_for;
        self.stop_cast(hi, Stop::Interrupted);
        let h = &mut self.heroes[hi];
        let player = h.player;
        h.hp = 0.0;
        h.stealthed = false;
        h.fight.life = Life::Downed {
            left: downed_for,
            revive: 0.0,
        };
        h.fight.attacking = None;
        h.fight.sprint = 0.0;
        self.events.push(Event::Downed { player });
    }

    /// A downed hero's clock: a teammate up within `revive_reach` revives them, a 20-yard noise
    /// as the revive begins, and walking away starts it over; at `downed_for` they die.
    pub(crate) fn step_fall(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let Life::Downed { left, revive } = self.heroes[hi].fight.life else {
            return;
        };
        let (player, pos, z) = (self.heroes[hi].player, self.heroes[hi].pos, self.heroes[hi].z);
        let reviver = self
            .heroes
            .iter()
            .find(|o| o.player != player && o.fight.up() && o.pos.distance(pos) <= r.revive_reach)
            .map(|o| o.player);
        let left = left - DT;
        let revive = match reviver {
            Some(by) => {
                if revive <= 0.0 {
                    self.make_noise(by, V3::at(pos, z), Loudness::Normal);
                }
                revive + DT
            }
            None => 0.0,
        };
        match reviver {
            Some(by) if revive + 1e-4 >= r.revive_for => {
                let h = &mut self.heroes[hi];
                h.fight.life = Life::Up;
                h.hp = h.max_hp * r.revive_health;
                self.events.push(Event::Revived { player, by });
            }
            _ if left <= 1e-4 => {
                self.heroes[hi].fight.life = Life::Dead;
                self.events.push(Event::Died { player });
            }
            _ => self.heroes[hi].fight.life = Life::Downed { left, revive },
        }
    }
}
```

In `crates/nightfall-sim/src/combat/heroes.rs`:

- In `step_fighters`, `self.step_fall(hi);` comes first in the loop body.
- `set_pace` becomes:

```rust
    /// Hero `hi`'s pace this step: the senses' speed (stealth's) times Sprint, times
    /// `cast_move_speed` while casting a slowed ability, times `crawl_speed` downed; feet held for
    /// a rooted ability, and for the dead.
    fn set_pace(&mut self, hi: usize) {
        let content = self.content.clone();
        let r = &content.rules.combat;
        let sprint = 1.0 + self.sprint_by(hi);
        let hero = &mut self.heroes[hi];
        let moving = hero
            .fight
            .casting
            .map(|c| content.abilities[usize::from(c.ability)].moving);
        let cast = if moving == Some(Moving::Slowed) {
            r.cast_move_speed
        } else {
            1.0
        };
        let life = match hero.fight.life {
            Life::Up => 1.0,
            Life::Downed { .. } => r.crawl_speed,
            Life::Dead => 0.0,
        };
        hero.speed *= sprint * cast * life;
        hero.fight.rooted = moving == Some(Moving::Rooted) || hero.fight.life == Life::Dead;
    }
```

  and `Life` joins its `super::{…}` import.

In `crates/nightfall-sim/src/senses/react.rs`'s `sense`, add as its first lines:

```rust
    // The downed and the dead are left be (spec §8.1): no sight of them, no sound.
    if !h.fight.up() {
        return Sensed {
            player: h.player,
            ..Sensed::default()
        };
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test and the 5 new ones in `combat::tests`).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: heroes fall: downed 45 s and crawling, revived by a friend in 5 s, else dead

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Enemies strike back: swings, casts, telegraphed blows, and the dodge

**Files:**
- Create: `crates/nightfall-sim/src/combat/strikes.rs`
- Modify: `crates/nightfall-sim/src/combat/mod.rs` (module), `crates/nightfall-sim/src/combat/heroes.rs` (`take_presses`, `tick_hero`), `crates/nightfall-sim/src/combat/tests.rs`, `crates/nightfall-sim/src/world.rs` (`step`)

**Interfaces:**
- Consumes: `EnemyCombat`, `EnemyCast`, `Telegraph`, `Hold`, `Event`, `Stop`, `UnitRef`, `Combat::next_telegraph` (Task 3); `Charges::{spend, tick}`, `in_range` (Task 2); `AbilityDef::windup`, `Shape::{contains, reach}`, `TelegraphDef`, `EnemyDef::{swing, reach, kit}`, `DodgeRules` (Task 1); `World::{wound, clear_line, stop_cast, stop_eating}`, `heroes::NEEDS_SIGHT` (Tasks 5, 7); `Senses::target_of` (Task 6); `enemies::heading`, `World::make_noise` (earlier plans).
- Produces:
  - Crate-only `World::strike_back(&mut self)`: every enemy after an up hero swings in reach, uses a ready move that reaches (a cast from afar, or a telegraph on the ground), casts run down and land; telegraphs that are due land on every up hero inside them unless their dodge's moment covers it.
  - Crate-only `World::dodge(&mut self, PlayerId)`; `take_presses` counts `PlayerInput::dodge_presses` (round past 255, the first a baseline); charges come back in `tick_hero`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/nightfall-sim/src/combat/tests.rs`:

```rust
#[test]
fn a_brute_in_reach_swings_every_two_seconds_and_breaks_stealth() {
    let mut w = world(&post("brute", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 28.0, 30.0);
    w.heroes[0].stealthed = true;
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, 125);
    assert_eq!(
        damage_to(&w.drain_events(), UnitRef::Hero(0)),
        vec![40.0, 40.0, 40.0]
    );
    assert!(!w.hero(0).unwrap().stealthed);
}

#[test]
fn a_caster_casts_from_range_and_its_bolt_lands() {
    let mut w = world(&post("caster", 35.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    steps(&mut w, 150);
    let events = w.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::CastStart { by: UnitRef::Enemy(i), .. } if *i == id)));
    assert!(events.contains(&Event::CastGo {
        by: UnitRef::Enemy(id),
        ability: ability(&w, "smite") as u16,
        hits: vec![UnitRef::Hero(0)],
        at: None
    }));
    assert_eq!(w.hero(0).unwrap().hp, 750.0);
}

#[test]
fn an_enemys_cast_stops_when_its_hero_falls() {
    let mut w = world(&post("caster", 35.0, 30.0, 180.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    let id = w.enemies[0].id;
    w.alert(id, 0);
    for _ in 0..150 {
        if w.enemies[0].fight.casting.is_some() {
            break;
        }
        w.step();
    }
    assert!(w.enemies[0].fight.casting.is_some());
    w.hurt_hero(0, 5000.0);
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Enemy(id),
        ability: ability(&w, "smite") as u16,
        why: Stop::TargetLost
    }));
}

#[test]
fn a_telegraph_shows_then_lands_on_whoever_stands_in_it() {
    let mut w = world(&post("stomper", 30.0, 30.0, 180.0));
    join(&mut w, 0, "rogue", 25.0, 30.0);
    join(&mut w, 1, "mage", 10.0, 30.0);
    w.alert(w.enemies[0].id, 0);
    for _ in 0..200 {
        if !w.telegraphs.is_empty() {
            break;
        }
        w.step();
    }
    let t = w.telegraphs[0];
    assert_eq!(t.shape, Shape::Circle { radius: 8.0 });
    assert_eq!(t.at.ground(), w.enemies[0].pos);
    assert!((t.total - 1.5).abs() < 1e-4);
    assert!(w.combat.enemies[0].cast.is_some());
    steps(&mut w, 50);
    let events = w.drain_events();
    assert_eq!(damage_to(&events, UnitRef::Hero(0)), vec![150.0]);
    assert!(damage_to(&events, UnitRef::Hero(1)).is_empty());
    assert!(w.telegraphs.is_empty());
}

/// A stomp put on the ground under player 0 to land in `steps_to_land` steps, the hero pressing
/// dodge in the first of them.
fn stomped_after_a_dodge(steps_to_land: usize) -> Vec<Event> {
    let mut w = world(&post("elite", 40.0, 40.0, 0.0));
    join(&mut w, 0, "rogue", 20.0, 30.0);
    w.telegraphs.push(Telegraph {
        id: 99,
        enemy: w.enemies[0].id,
        ability: ability(&w, "stomp") as u16,
        shape: Shape::Circle { radius: 8.0 },
        at: V3::new(20.0, 30.0, 0.0),
        facing: V2::new(1.0, 0.0),
        left: steps_to_land as f32 * DT,
        total: 1.5,
    });
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(20.0, 30.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            dodge_presses: 1,
            ..Default::default()
        },
    );
    steps(&mut w, steps_to_land + 1);
    w.drain_events()
}

#[test]
fn a_dodge_makes_a_telegraph_miss_only_inside_its_window() {
    // Landing in the press's step and the 8 after it, 0.3 s: a miss.
    let inside = stomped_after_a_dodge(9);
    assert!(inside.contains(&Event::Evaded {
        player: 0,
        enemy: 1
    }));
    assert!(damage_to(&inside, UnitRef::Hero(0)).is_empty());
    // One step later: a hit.
    let late = stomped_after_a_dodge(10);
    assert_eq!(damage_to(&late, UnitRef::Hero(0)), vec![150.0]);
}

#[test]
fn dodges_come_back_one_at_a_time_and_are_heard() {
    let mut w = world("");
    join(&mut w, 0, "rogue", 20.0, 30.0);
    let dodge = |w: &mut World, presses: u8| {
        w.set_input(
            0,
            PlayerInput {
                pose: Some(HeroPose {
                    pos: V2::new(20.0, 30.0),
                    facing: V2::new(1.0, 0.0),
                    z: 0.0,
                }),
                dodge_presses: presses,
                ..Default::default()
            },
        );
        w.step();
    };
    for n in 1..=3 {
        dodge(&mut w, n);
    }
    let dodged = w
        .drain_events()
        .iter()
        .filter(|e| matches!(e, Event::Dodged { player: 0 }))
        .count();
    assert_eq!(dodged, 2);
    assert_eq!(w.hero(0).unwrap().fight.dodge.have, 0);
    assert_eq!(w.hero(0).unwrap().noise, 20.0);
    steps(&mut w, 8 * 30);
    assert_eq!(w.hero(0).unwrap().fight.dodge.have, 1);
    // A count that went round past 255 is one press.
    let mut w = world("");
    w.add_hero(0, "rogue", "Hero", 1, 0).unwrap();
    dodge(&mut w, 255);
    dodge(&mut w, 0);
    let dodged = w
        .drain_events()
        .iter()
        .filter(|e| matches!(e, Event::Dodged { player: 0 }))
        .count();
    assert_eq!(dodged, 1);
}

#[test]
fn a_dodge_stops_a_cast() {
    let mut w = world(&post("brute", 30.0, 30.0, 0.0));
    join(&mut w, 0, "mage", 10.0, 30.0);
    press(&mut w, 0, 1, 1, foe(&w, 0), None);
    w.step();
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(10.0, 30.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            dodge_presses: 1,
            ..Default::default()
        },
    );
    w.step();
    assert!(w.drain_events().contains(&Event::CastStop {
        by: UnitRef::Hero(0),
        ability: 0,
        why: Stop::Cancelled
    }));
}
```

In `crates/nightfall-sim/src/combat/mod.rs`, below `mod falling;` add `mod strikes;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim combat::tests`
Expected: FAIL to compile (`file not found for module strikes`); once it compiles, the new tests fail (no swing lands, no telegraph shows).

- [ ] **Step 3: Implement enemies striking back and the dodge**

`crates/nightfall-sim/src/combat/strikes.rs`:

```rust
//! Enemies striking back (spec §6.1, §7.1) and the dodge. An enemy after an up hero uses a move
//! of its kind when one is ready and reaches (a caster's bolt from afar; an elite's big move,
//! shown on the ground as a telegraph before it lands), else swings plainly when in reach. A
//! telegraph lands on every up hero inside it when due, except one whose dodge's moment
//! (`dodge.immune`, from the press) covers it. A dodge spends a charge (each comes back in turn),
//! makes a 20-yard noise and stops a cast or a meal; the hop itself is the client's.

use super::heroes::NEEDS_SIGHT;
use super::{in_range, EnemyCast, Event, Hold, Stop, Telegraph, UnitRef};
use crate::enemies::heading;
use crate::{
    Aim, Effect, Loudness, PlayerId, Shape, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V3,
};

impl World {
    /// Every enemy after a hero fights for a step; every telegraph that is due lands; and the
    /// dodges' moments run down.
    pub(crate) fn strike_back(&mut self) {
        for ei in 0..self.enemies.len().min(self.combat.enemies.len()) {
            self.fight_back(ei);
        }
        self.land_telegraphs();
        for hc in &mut self.combat.heroes {
            hc.immune = (hc.immune - DT).max(0.0);
        }
    }

    /// Enemy `ei`'s turn: its clocks run, its cast goes on or lands, else it begins a ready move
    /// that reaches its hero, else it swings when in reach.
    fn fight_back(&mut self, ei: usize) {
        let content = self.content.clone();
        {
            let ec = &mut self.combat.enemies[ei];
            ec.swing_in = (ec.swing_in - DT).max(0.0);
            for t in &mut ec.moves_in {
                *t = (*t - DT).max(0.0);
            }
        }
        if self.enemies[ei].hp <= 0.0 || self.combat.enemies[ei].engage.hold == Hold::Out {
            return;
        }
        let id = self.enemies[ei].id;
        if let Some(mut c) = self.combat.enemies[ei].cast {
            // A telegraph lands by itself ([`World::land_telegraphs`]).
            if c.telegraph.is_some() {
                return;
            }
            c.left -= DT;
            let up = self.hero(c.player).is_some_and(|h| h.fight.up());
            if !up {
                self.combat.enemies[ei].cast = None;
                self.events.push(Event::CastStop {
                    by: UnitRef::Enemy(id),
                    ability: c.ability as u16,
                    why: Stop::TargetLost,
                });
            } else if c.left <= 1e-4 {
                self.combat.enemies[ei].cast = None;
                self.spell_lands(ei, c.ability, c.player);
            } else {
                self.combat.enemies[ei].cast = Some(c);
            }
            return;
        }
        let Some((player, _)) = self.senses.target_of(ei) else {
            return;
        };
        let Some(hi) = self.hero_slot(player).filter(|&hi| self.heroes[hi].fight.up()) else {
            return;
        };
        let def = &content.enemies[self.enemies[ei].kind];
        for (m, &(a, every)) in def.kit.iter().enumerate() {
            let ready = self.combat.enemies[ei].moves_in.get(m).is_some_and(|t| *t <= 0.0);
            if ready && self.move_reaches(ei, hi, a) {
                self.combat.enemies[ei].moves_in[m] = every;
                self.begin_move(ei, hi, a);
                return;
            }
        }
        let Some(blow) = def.swing else {
            return;
        };
        let (e, h) = (&self.enemies[ei], &self.heroes[hi]);
        if e.pos.distance(h.pos) > def.reach || self.combat.enemies[ei].swing_in > 0.0 {
            return;
        }
        let to = h.pos;
        self.enemies[ei].facing = heading(to - self.enemies[ei].pos);
        self.combat.enemies[ei].swing_in = blow.every;
        let amount = self.combat.dice.between(blow.min, blow.max);
        self.events.push(Event::Swing {
            by: UnitRef::Enemy(id),
            to: UnitRef::Hero(player),
        });
        self.wound(player, amount, Some(UnitRef::Enemy(id)));
    }

    /// Whether enemy `ei`'s move `a` reaches hero `hi` from where it stands: a circle around it
    /// within its size, else the ability's range, with a clear line for a far one.
    fn move_reaches(&self, ei: usize, hi: usize, a: usize) -> bool {
        let ab = &self.content.abilities[a];
        let (e, h) = (&self.enemies[ei], &self.heroes[hi]);
        let d = e.pos.distance(h.pos);
        match ab.telegraph {
            Some(t) if t.on_self => d <= t.shape.reach(),
            _ if ab.aim == Aim::Me => d <= ab.radius,
            _ => {
                d <= ab.range
                    && (ab.range <= NEEDS_SIGHT
                        || self.clear_line(
                            V3::at(e.pos, e.z + EYE_HEIGHT),
                            V3::at(h.pos, h.z + CHEST_HEIGHT),
                        ))
            }
        }
    }

    /// Enemy `ei` begins move `a` at hero `hi`: turned to them, a telegraph put down where it
    /// will land (around itself, from itself for a cone, else under the hero as they stand now),
    /// and a cast or windup started; an instant lands at once.
    fn begin_move(&mut self, ei: usize, hi: usize, a: usize) {
        let content = self.content.clone();
        let ab = &content.abilities[a];
        let (id, from, from_z) = (self.enemies[ei].id, self.enemies[ei].pos, self.enemies[ei].z);
        let (player, to, to_z) = (self.heroes[hi].player, self.heroes[hi].pos, self.heroes[hi].z);
        let facing = heading(to - from);
        self.enemies[ei].facing = facing;
        let windup = ab.windup();
        let mut telegraph = None;
        if let Some(t) = ab.telegraph {
            let at = if t.on_self || matches!(t.shape, Shape::Cone { .. }) {
                V3::at(from, from_z)
            } else {
                V3::at(to, to_z)
            };
            let tid = self.combat.next_telegraph;
            self.combat.next_telegraph += 1;
            self.telegraphs.push(Telegraph {
                id: tid,
                enemy: id,
                ability: a as u16,
                shape: t.shape,
                at,
                facing,
                left: windup,
                total: windup,
            });
            telegraph = Some(tid);
        }
        if windup <= 0.0 {
            self.spell_lands(ei, a, player);
            return;
        }
        self.combat.enemies[ei].cast = Some(EnemyCast {
            ability: a,
            player,
            left: windup,
            total: windup,
            telegraph,
        });
        self.events.push(Event::CastStart {
            by: UnitRef::Enemy(id),
            ability: a as u16,
            target: Some(UnitRef::Hero(player)),
            secs: windup,
        });
    }

    /// An enemy's ability does its damage: its `Damage` effects, and `Weapon` as its kind's
    /// swing. Other effects are heroes' alone for now.
    fn enemy_damage(&mut self, ei: usize, a: usize) -> f32 {
        let content = self.content.clone();
        let swing = content.enemies[self.enemies[ei].kind].swing;
        let mut total = 0.0;
        for effect in &content.abilities[a].effects {
            match *effect {
                Effect::Damage { min, max, .. } => total += self.combat.dice.between(min, max),
                Effect::Weapon { percent, bonus } => {
                    if let Some(s) = swing {
                        total += self.combat.dice.between(s.min, s.max) * percent / 100.0 + bonus;
                    }
                }
                _ => {}
            }
        }
        total
    }

    /// Enemy `ei`'s cast of `a` at `player` goes off: it lands if the hero is still up, in range
    /// and in sight.
    fn spell_lands(&mut self, ei: usize, a: usize, player: PlayerId) {
        let id = self.enemies[ei].id;
        let range = self.content.abilities[a].range;
        let reach = self.content.rules.combat.clone();
        let hit = self.hero_slot(player).filter(|&hi| {
            let (e, h) = (&self.enemies[ei], &self.heroes[hi]);
            h.fight.up()
                && in_range(e.pos.distance(h.pos), range, &reach)
                && (range <= NEEDS_SIGHT
                    || self.clear_line(
                        V3::at(e.pos, e.z + EYE_HEIGHT),
                        V3::at(h.pos, h.z + CHEST_HEIGHT),
                    ))
        });
        let mut hits = Vec::new();
        if hit.is_some() {
            let amount = self.enemy_damage(ei, a);
            self.wound(player, amount, Some(UnitRef::Enemy(id)));
            hits.push(UnitRef::Hero(player));
        }
        self.events.push(Event::CastGo {
            by: UnitRef::Enemy(id),
            ability: a as u16,
            hits,
            at: None,
        });
    }

    /// Telegraphs run down, and those due land.
    fn land_telegraphs(&mut self) {
        for t in &mut self.telegraphs {
            t.left -= DT;
        }
        let due: Vec<Telegraph> = self
            .telegraphs
            .iter()
            .filter(|t| t.left <= 1e-4)
            .copied()
            .collect();
        self.telegraphs.retain(|t| t.left > 1e-4);
        for t in due {
            self.telegraph_lands(t);
        }
    }

    /// Telegraph `t` lands on every up hero inside it, except those whose dodge's moment covers
    /// it, who see it miss.
    fn telegraph_lands(&mut self, t: Telegraph) {
        let Some(ei) = self.enemies.iter().position(|e| e.id == t.enemy) else {
            return;
        };
        if let Some(c) = &self.combat.enemies[ei].cast {
            if c.telegraph == Some(t.id) {
                self.combat.enemies[ei].cast = None;
            }
        }
        let inside: Vec<PlayerId> = self
            .heroes
            .iter()
            .filter(|h| h.fight.up() && t.shape.contains(t.at.ground(), t.facing, h.pos))
            .map(|h| h.player)
            .collect();
        let mut hits = Vec::new();
        for p in inside {
            let ci = self.combat.hero_index(p);
            if self.combat.heroes[ci].immune > 1e-4 {
                self.events.push(Event::Evaded {
                    player: p,
                    enemy: t.enemy,
                });
                continue;
            }
            let amount = self.enemy_damage(ei, usize::from(t.ability));
            self.wound(p, amount, Some(UnitRef::Enemy(t.enemy)));
            hits.push(UnitRef::Hero(p));
        }
        self.events.push(Event::CastGo {
            by: UnitRef::Enemy(t.enemy),
            ability: t.ability,
            hits,
            at: Some(t.at),
        });
    }

    /// `player` dodges (spec §6.1): a charge spent if there is one; telegraphed blows miss them
    /// for `dodge.immune` seconds; a cast or a meal stops; a 20-yard noise. Their client has
    /// already hopped them away.
    pub(crate) fn dodge(&mut self, player: PlayerId) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let r = self.content.rules.combat.dodge;
        if !self.heroes[hi].fight.up() || !self.heroes[hi].fight.dodge.spend(&r) {
            return;
        }
        let ci = self.combat.hero_index(player);
        self.combat.heroes[ci].immune = r.immune;
        self.stop_cast(hi, Stop::Cancelled);
        self.stop_eating(hi);
        let at = V3::at(self.heroes[hi].pos, self.heroes[hi].z);
        self.make_noise(player, at, Loudness::Normal);
        self.events.push(Event::Dodged { player });
    }
}
```

In `crates/nightfall-sim/src/combat/heroes.rs`:

- `take_presses` counts the dodge presses: after `hc.started = true;` add

```rust
        // Dodges come as a count, round past 255, as Stealth's presses do.
        let dodged = if first {
            0
        } else {
            input.dodge_presses.wrapping_sub(hc.dodges)
        };
        hc.dodges = input.dodge_presses;
```

  and at its end, after the press:

```rust
        let charges = self.content.rules.combat.dodge.charges;
        for _ in 0..dodged.min(charges) {
            self.dodge(player);
        }
```

  (the `if first { return; }` line moves below these, so a first input's count is still recorded; it now reads `if !first { if let Some(p) = … { self.press(hi, p); } }` followed by the dodge loop).
- In `tick_hero`, after `f.sprint = …;` add `f.dodge.tick(DT, &r.dodge);`.

The whole of `take_presses` then reads:

```rust
    pub(crate) fn take_presses(&mut self, player: PlayerId, input: &PlayerInput) {
        let Some(hi) = self.hero_slot(player) else {
            return;
        };
        let ci = self.combat.hero_index(player);
        let hc = &mut self.combat.heroes[ci];
        hc.moving = input.gait != Gait::Still;
        let first = !hc.started;
        hc.started = true;
        // Dodges come as a count, round past 255, as Stealth's presses do.
        let dodged = if first {
            0
        } else {
            input.dodge_presses.wrapping_sub(hc.dodges)
        };
        hc.dodges = input.dodge_presses;
        let last = hc.seq;
        if let Some(p) = input.cast {
            hc.seq = Some(p.seq);
        }
        if !first {
            if let Some(p) = input.cast.filter(|p| last != Some(p.seq)) {
                self.press(hi, p);
            }
        }
        let charges = self.content.rules.combat.dodge.charges;
        for _ in 0..dodged.min(charges) {
            self.dodge(player);
        }
    }
```

In `crates/nightfall-sim/src/world.rs`'s `step`, `if self.nav.is_some() { self.hold_enemies(); }` becomes:

```rust
        if self.nav.is_some() {
            self.hold_enemies();
            // Before the senses listen, so the noise of these blows is heard this step.
            self.strike_back();
        }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (every earlier test and the 7 new ones in `combat::tests`). The senses' tests are unchanged: their enemy kinds have no `swing`, so their chases end in no blows.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/nightfall-sim
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "nightfall-sim: enemies swing, cast from range and telegraph big moves; dodges make them miss

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The client's presses: the bar, WoW's `UseAction`, ground aim, the dodge key

**Files:**
- Create: `crates/benilla-app/src/nightfall/bar.rs`, `crates/benilla-app/src/nightfall/dodge.rs`
- Modify: `crates/benilla-app/src/nightfall/run.rs` (`LocalInput`), `crates/benilla-app/src/nightfall/units.rs` (`unit_of`), `crates/benilla-app/src/nightfall/view.rs` (`HERO_SERIAL_BASE`), `crates/benilla-app/src/nightfall/stance.rs` (`stealth_key`), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `CastPress`, `UnitRef`, `HeroFight::{dodge, rooted, up}`, `ClassDef::kit`, `AbilityDef::{aim, needs, has}`, `Shape::contains`, `World::telegraphs` (Tasks 1, 3); `run::{Sim, LocalInput}`, `view::Views`, `units::{unit_guid, UnitKind}`, `stance::stealth_key` (earlier plans); engine: `UiScript::{take_action_uses, register_addon_bindings, run, session, lua}`, `benilla_ui::bindings_xml::Binding`, `crate::ui_script::{UiInput, PendingEntryUiLoad}`, `crate::script_calls::apply_script_calls`, `crate::ui_action::PlayerActions`, `crate::target::Selection`, `crate::net::{KnockBackMessage, SelfGuid}`, `crate::player::Player::{active, move_flags}`, `crate::creature_anim::move_flags`, `benilla_protocol::JumpInfo`, `WorldPoint::terrain_height_under`, `WorldCamera`.
- Produces:
  - `run::LocalInput` gains `pub cast: Option<CastPress>`, `pub dodge_presses: u8` and `press(&mut self, slot: u8, Option<UnitRef>, Option<V3>)`.
  - `units::unit_of(guid: u64) -> Option<UnitRef>`; `view::HERO_SERIAL_BASE` becomes `pub(super)`.
  - `bar::{slot_ability(&[(u8, usize)], u32) -> Option<usize>, attacks(&[(u8, usize)], &[AbilityDef]) -> Vec<u8>, ground_hit(Vec3, Vec3, f32, impl Fn(Vec3) -> Option<f32>) -> Option<Vec3>, ahead(&Hero, f32) -> V3}`; systems `bar::{fill, presses, autofight}`.
  - `dodge::{COUNTER, binding() -> Binding, dodge_way(V2, u32) -> V2, hop(V2, f32) -> JumpInfo}`; systems `dodge::{setup, key}`.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/bar.rs`:

```rust
//! The action bar (spec §6.1): the hero's abilities on WoW's own bar as a server's
//! `SMSG_INITIAL_SPELLS` and `SMSG_ACTION_BUTTONS` would put them there (the main bar, and the bar
//! a stealthed rogue sees), and its presses as the host needs them. A press is WoW's own
//! `UseAction`, from the keys 1 to = or a click, taken out of the script's calls before the
//! engine's own cast path sees it ([`presses`]): it becomes a [`CastPress`] at the current target,
//! or at the ground under the mouse for an ability aimed at the ground; Stealth's button counts a
//! Stealth press instead. `NIGHTFALL_AUTOFIGHT=1` targets the nearest enemy, presses the hero's
//! attacks in turn and dodges telegraphs, for test runs.
//!
//! [`CastPress`]: nightfall_sim::CastPress

use std::sync::OnceLock;

use benilla_protocol::messages::{ActionButton, ACTION_KIND_SPELL};
use benilla_ui::script::UiScript;
use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use nightfall_sim::{AbilityDef, Aim, Effect, Hero, UnitRef, V3};

use super::run::{LocalInput, Sim};
use super::units;
use crate::target::Selection;
use crate::ui_action::PlayerActions;

/// Yards between the cursor ray's samples as it looks for the ground.
const AIM_STEP: f32 = 0.25;
/// How far the cursor ray looks for the ground, yards.
const AIM_REACH: f32 = 120.0;
/// Seconds between the autofight's presses.
const AUTO_EVERY: f32 = 1.6;
/// Seconds between the autofight's dodges.
const AUTO_DODGE_EVERY: f32 = 0.5;

/// The ability at WoW action `action` on a class's bar.
pub(super) fn slot_ability(kit: &[(u8, usize)], action: u32) -> Option<usize> {
    kit.iter()
        .find(|&&(s, _)| u32::from(s) == action)
        .map(|&(_, a)| a)
}

/// The main bar's actions that strike an enemy and need no stealth, in bar order: what the
/// autofight presses.
pub(super) fn attacks(kit: &[(u8, usize)], abilities: &[AbilityDef]) -> Vec<u8> {
    kit.iter()
        .filter(|&&(s, a)| {
            s <= 12 && abilities[a].aim == Aim::Enemy && !abilities[a].needs.stealth
        })
        .map(|&(s, _)| s)
        .collect()
}

/// The first point along the ray from `origin` the way `dir` goes, within `max` yards, at or
/// below the ground `height` gives (Bevy's coordinates, y up): where a cursor ray meets the
/// terrain.
pub(super) fn ground_hit(
    origin: Vec3,
    dir: Vec3,
    max: f32,
    height: impl Fn(Vec3) -> Option<f32>,
) -> Option<Vec3> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let mut t = 0.0;
    while t <= max {
        let p = origin + dir * t;
        if let Some(h) = height(p) {
            if p.y <= h {
                return Some(Vec3::new(p.x, h, p.z));
            }
        }
        t += AIM_STEP;
    }
    None
}

/// Where an ability aimed at the ground goes when the cursor finds none (a background test
/// window): four fifths of its range straight ahead.
pub(super) fn ahead(hero: &Hero, range: f32) -> V3 {
    V3::at(hero.pos + hero.facing * (range * 0.8), hero.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{Content, ContentSources};

    fn content() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 1000.0, speed: 7.0, power_max: 100.0,
                bar: [(1, "stealth"), (2, "strike"), (11, "stone"), (74, "ambush")])]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: r#"[
                (key: "stealth", name: "Stealth", spell: 1785, aim: Me, effects: [Stealth]),
                (key: "strike", name: "Strike", spell: 1760, aim: Enemy, range: 5.0,
                 effects: [Weapon(bonus: 22.0)]),
                (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0, effects: []),
                (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0,
                 needs: (stealth: true), effects: [Weapon(percent: 250.0)]),
            ]"#,
        })
        .unwrap()
    }

    #[test]
    fn an_action_names_its_ability_and_the_autofight_takes_plain_attacks() {
        let c = content();
        let kit = &c.classes[0].kit;
        assert_eq!(slot_ability(kit, 2), Some(1));
        assert_eq!(slot_ability(kit, 74), Some(3));
        assert_eq!(slot_ability(kit, 5), None);
        assert_eq!(attacks(kit, &c.abilities), vec![2]);
    }

    #[test]
    fn the_cursor_ray_meets_the_ground() {
        let flat = |_: Vec3| Some(10.0);
        let hit = ground_hit(Vec3::new(0.0, 20.0, 0.0), Vec3::new(1.0, -1.0, 0.0), 50.0, flat)
            .unwrap();
        assert!((hit.x - 10.0).abs() < 0.3, "{hit:?}");
        assert_eq!(hit.y, 10.0);
        assert!(ground_hit(Vec3::new(0.0, 20.0, 0.0), Vec3::Y, 50.0, flat).is_none());
        assert!(ground_hit(Vec3::new(0.0, 20.0, 0.0), Vec3::ZERO, 50.0, flat).is_none());
    }
}
```

`crates/benilla-app/src/nightfall/dodge.rs`:

```rust
//! The dodge (spec §6.1): a 6-yard hop the way the hero moves, on a key of its own. The key is a
//! WoW key binding, `NIGHTFALL_DODGE`, listed under "Nightfall" in the game's own Key Bindings
//! window and saved with the player's other bindings; it starts on Caps Lock and the mouse's back
//! button, which 1.12's `DefaultBindings.wtf` leaves free. The client hops at once when its mirror
//! shows a charge (the host's word would come a round trip later), as a knockback the controller
//! flies with its own collision, and counts the press into the input; the host spends the charge
//! and makes telegraphed blows miss.

use benilla_protocol::JumpInfo;
use benilla_ui::bindings_xml::Binding;
use benilla_ui::script::UiScript;
use bevy::prelude::*;
use nightfall_sim::V2;

use super::content;
use super::run::{LocalInput, Sim};
use super::view::Views;
use crate::creature_anim::move_flags::{BACKWARD, FORWARD, STRAFE_LEFT, STRAFE_RIGHT};
use crate::net::{KnockBackMessage, SelfGuid};
use crate::ui_script::PendingEntryUiLoad;

/// The binding's command.
const BINDING: &str = "NIGHTFALL_DODGE";
/// The Lua global the binding counts its presses in.
pub(super) const COUNTER: &str = "NIGHTFALL_DODGES";
/// Run once per UI session: the binding's names in the Key Bindings window, its counter, and its
/// default keys unless the player has bound it already.
const SETUP: &str = r#"
BINDING_HEADER_NIGHTFALL = "Nightfall"
BINDING_NAME_NIGHTFALL_DODGE = "Dodge"
NIGHTFALL_DODGES = NIGHTFALL_DODGES or 0
if not GetBindingKey("NIGHTFALL_DODGE") then
    SetBinding("CAPSLOCK", "NIGHTFALL_DODGE")
    SetBinding("BUTTON5", "NIGHTFALL_DODGE")
end
"#;
/// Yards a second up the hop starts at; with WoW's gravity it lasts about half a second.
const HOP_UP: f32 = 5.0;
/// WoW's gravity, yards a second squared.
const GRAVITY: f32 = 19.291_105;

/// The binding as the Key Bindings window lists it: a press counts into [`COUNTER`].
pub(super) fn binding() -> Binding {
    Binding {
        name: BINDING.into(),
        header: Some("NIGHTFALL".into()),
        run_on_up: false,
        hidden: false,
        platform: None,
        body: format!("{COUNTER} = ({COUNTER} or 0) + 1"),
    }
}

/// The way a dodge goes: where the movement keys held point, turned with `facing` (WoW's ground
/// plane, x north, y west); straight back with none held.
pub(super) fn dodge_way(facing: V2, flags: u32) -> V2 {
    let f = facing.normalize_or_zero();
    let left = V2::new(-f.y, f.x);
    let mut way = V2::ZERO;
    if flags & FORWARD != 0 {
        way += f;
    }
    if flags & BACKWARD != 0 {
        way += f * -1.0;
    }
    if flags & STRAFE_LEFT != 0 {
        way += left;
    }
    if flags & STRAFE_RIGHT != 0 {
        way += left * -1.0;
    }
    let way = way.normalize_or_zero();
    if way == V2::ZERO {
        f * -1.0
    } else {
        way
    }
}

/// The launch of a hop of `distance` yards the way `way` goes, in the knockback's terms.
pub(super) fn hop(way: V2, distance: f32) -> JumpInfo {
    let air = 2.0 * HOP_UP / GRAVITY;
    JumpInfo {
        zspeed: -HOP_UP,
        cos_angle: way.x,
        sin_angle: way.y,
        xy_speed: distance / air,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dodge_goes_the_way_the_keys_point_and_back_with_none() {
        let north = V2::new(1.0, 0.0);
        assert_eq!(dodge_way(north, FORWARD), north);
        assert_eq!(dodge_way(north, 0), V2::new(-1.0, 0.0));
        // Left of north is west, +y.
        assert_eq!(dodge_way(north, STRAFE_LEFT), V2::new(0.0, 1.0));
        let d = dodge_way(north, FORWARD | STRAFE_RIGHT);
        assert!((d.x - 0.7071).abs() < 1e-3 && (d.y + 0.7071).abs() < 1e-3);
        // Forward and back cancel: back it is.
        assert_eq!(dodge_way(north, FORWARD | BACKWARD), V2::new(-1.0, 0.0));
    }

    #[test]
    fn the_hop_covers_its_distance_in_the_air() {
        let j = hop(V2::new(0.0, 1.0), 6.0);
        let air = 2.0 * -j.zspeed / GRAVITY;
        assert!((j.xy_speed * air - 6.0).abs() < 1e-4);
        assert_eq!((j.cos_angle, j.sin_angle), (0.0, 1.0));
    }

    #[test]
    fn the_binding_counts_its_presses() {
        let b = binding();
        assert_eq!(b.name, "NIGHTFALL_DODGE");
        assert_eq!(b.body, "NIGHTFALL_DODGES = (NIGHTFALL_DODGES or 0) + 1");
    }
}
```

In `crates/benilla-app/src/nightfall/units.rs`, add `use nightfall_sim::UnitRef;` and below `unit_guid`:

```rust
/// The hero or enemy a guid names, as [`unit_guid`] made it: a creature's serial is its enemy's
/// id, a player's is its hero's number past [`super::view::HERO_SERIAL_BASE`].
pub(super) fn unit_of(guid: u64) -> Option<UnitRef> {
    if guid & 0xFFFF_0000_0000_0000 == HIGH_CREATURE {
        return Some(UnitRef::Enemy((guid & 0xFFFF_FFFF) as u32));
    }
    let serial = guid.checked_sub(super::view::HERO_SERIAL_BASE)?;
    (serial < 256).then_some(UnitRef::Hero(serial as u8))
}
```

and to its tests (create `#[cfg(test)] mod tests { use super::*; … }` if it has none):

```rust
    #[test]
    fn a_guid_names_the_hero_or_enemy_it_was_made_for() {
        let enemy = unit_guid(UnitKind::Creature { entry: 3 }, 41);
        assert_eq!(unit_of(enemy), Some(UnitRef::Enemy(41)));
        let hero = unit_guid(UnitKind::Player, super::super::view::HERO_SERIAL_BASE + 2);
        assert_eq!(unit_of(hero), Some(UnitRef::Hero(2)));
        assert_eq!(unit_of(3), None);
        assert_eq!(unit_of(0xF110_0000_0000_0007), None);
    }
```

In `crates/benilla-app/src/nightfall/view.rs`, `const HERO_SERIAL_BASE` becomes `pub(super) const HERO_SERIAL_BASE`.

In `crates/benilla-app/src/nightfall/run.rs`:

- Add `CastPress`, `UnitRef` and `V3` to its `nightfall_sim` import.
- `LocalInput` becomes:

```rust
/// The local hero's input for the next step.
#[derive(Resource, Default)]
pub(super) struct LocalInput {
    pose: Option<HeroPose>,
    /// How the hero moves, read off the controller ([`super::stance::gait_of`]).
    pub gait: Gait,
    /// Stealth presses this session, counted round past 255 ([`PlayerInput::stealth_presses`]).
    pub stealth_presses: u8,
    /// The latest ability press, repeated in every input until the next ([`PlayerInput::cast`]).
    pub cast: Option<CastPress>,
    /// Dodge presses this session, counted round past 255.
    pub dodge_presses: u8,
}

impl LocalInput {
    /// The input for the next step, with `pose`.
    fn with_pose(&self, pose: Option<HeroPose>) -> PlayerInput {
        PlayerInput {
            pose,
            gait: self.gait,
            stealth_presses: self.stealth_presses,
            cast: self.cast,
            dodge_presses: self.dodge_presses,
        }
    }

    /// A new press of WoW action `slot`, at `target` or the spot `at`: it gets the next number,
    /// so the host tells it from the last.
    pub fn press(&mut self, slot: u8, target: Option<UnitRef>, at: Option<V3>) {
        let seq = self.cast.map_or(1, |c| c.seq.wrapping_add(1));
        self.cast = Some(CastPress {
            seq,
            slot,
            target,
            at,
        });
    }
}
```

- Add to its tests:

```rust
    #[test]
    fn each_press_gets_the_next_number() {
        let mut input = LocalInput::default();
        input.press(2, None, None);
        input.press(2, None, None);
        assert_eq!(input.cast.map(|c| (c.seq, c.slot)), Some((2, 2)));
        input.cast = Some(CastPress {
            seq: 255,
            ..CastPress::default()
        });
        input.press(3, None, None);
        assert_eq!(input.cast.unwrap().seq, 0);
        assert_eq!(input.with_pose(None).cast, input.cast);
    }
```

In `crates/benilla-app/src/nightfall/mod.rs`, add `mod bar;` and `mod dodge;` beside the other modules.

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (the new tests in `bar`, `dodge`, `units` and `run`). Warnings that the systems below are missing are compile errors only once Step 2 registers them.

- [ ] **Step 2: Write the systems**

Append to `bar.rs`, above its tests:

```rust
fn autofight_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_AUTOFIGHT").is_some())
}

/// The hero's abilities on WoW's bar, each at its action, and in the spellbook; written again only
/// when the hero's class changes (a new run as another class).
pub(super) fn fill(sim: Res<Sim>, mut shown: Local<Option<usize>>, mut actions: ResMut<PlayerActions>) {
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    if *shown == Some(hero.class) {
        return;
    }
    let content = sim.world.content();
    actions.buttons.clear();
    actions.spells.clear();
    for &(slot, ability) in &content.classes[hero.class].kit {
        let spell = content.abilities[ability].spell;
        let index = slot - 1;
        actions.buttons.insert(
            index,
            ActionButton {
                slot: index,
                action: spell,
                kind: ACTION_KIND_SPELL,
            },
        );
        actions.spells.insert(spell);
    }
    actions.dirty = true;
    *shown = Some(hero.class);
}

/// Where the mouse cursor points on the ground, in WoW's coordinates.
fn cursor_ground(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    world_point: &WorldPoint,
) -> Option<V3> {
    let cursor = windows.single().ok()?.cursor_position()?;
    let (camera, tf) = cameras.single().ok()?;
    let ray = camera.viewport_to_world(tf, cursor).ok()?;
    let p = ground_hit(ray.origin, *ray.direction, AIM_REACH, |p| {
        world_point.terrain_height_under(p)
    })?;
    Some(V3::new(-p.z, -p.x, p.y))
}

/// This frame's `UseAction` presses, taken before the engine's cast path sees them: each ability
/// on the hero's bar becomes the next [`LocalInput::press`], at the current target (WoW's own
/// selection, Tab included) or, aimed at the ground, at the spot under the mouse; Stealth's
/// button counts a Stealth press. A press of an empty button does nothing.
pub(super) fn presses(
    script: Option<NonSendMut<UiScript>>,
    sim: Res<Sim>,
    selection: Res<Selection>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    world_point: WorldPoint,
    mut input: ResMut<LocalInput>,
) {
    let Some(mut script) = script else {
        return;
    };
    let uses = script.take_action_uses();
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let content = sim.world.content();
    let kit = &content.classes[hero.class].kit;
    for u in uses {
        let Some(ability) = slot_ability(kit, u.action) else {
            continue;
        };
        let a = &content.abilities[ability];
        if a.has(|e| matches!(e, Effect::Stealth)) {
            input.stealth_presses = input.stealth_presses.wrapping_add(1);
            continue;
        }
        let at = (a.aim == Aim::Ground).then(|| {
            cursor_ground(&windows, &cameras, &world_point)
                .unwrap_or_else(|| ahead(hero, a.range))
        });
        let target = selection.guid.and_then(units::unit_of);
        input.press(u.action as u8, target, at);
    }
}

/// `NIGHTFALL_AUTOFIGHT=1`: with no living enemy targeted, WoW's own Tab picks the nearest; else
/// every [`AUTO_EVERY`] seconds the next of the hero's plain attacks is pressed through
/// `UseAction`, as a key would; standing in a telegraph, the dodge binding is pressed.
pub(super) fn autofight(
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    selection: Res<Selection>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
    mut last_dodge: Local<f32>,
    mut turn: Local<usize>,
) {
    if !autofight_on() {
        return;
    }
    let Some(script) = script else {
        return;
    };
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let now = real.elapsed_secs();
    let threatened = sim
        .world
        .telegraphs
        .iter()
        .any(|t| t.shape.contains(t.at.ground(), t.facing, hero.pos));
    if threatened && hero.fight.up() && now - *last_dodge >= AUTO_DODGE_EVERY {
        *last_dodge = now;
        let c = super::dodge::COUNTER;
        let _ = script.run(&format!("{c} = ({c} or 0) + 1"));
    }
    if now - *last < AUTO_EVERY || !hero.fight.up() {
        return;
    }
    *last = now;
    let alive = matches!(
        selection.guid.and_then(units::unit_of),
        Some(UnitRef::Enemy(id)) if sim.world.enemy(id).is_some_and(|e| e.hp > 0.0)
    );
    if !alive {
        let _ = script.run("TargetNearestEnemy()");
        return;
    }
    let content = sim.world.content();
    let moves = attacks(&content.classes[hero.class].kit, &content.abilities);
    if moves.is_empty() {
        return;
    }
    let action = moves[*turn % moves.len()];
    *turn += 1;
    let _ = script.run(&format!("UseAction({action})"));
}
```

Append to `dodge.rs`, above its tests:

```rust
/// Once per UI session (a `/reload` is a new one), after its load: the binding registered under
/// "Nightfall" and [`SETUP`] run.
pub(super) fn setup(
    script: Option<NonSendMut<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut done: Local<Option<u64>>,
) {
    let Some(mut script) = script else {
        return;
    };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done == Some(session) {
        return;
    }
    script.register_addon_bindings("Nightfall", &[binding()]);
    if script.run(SETUP).is_ok() {
        *done = Some(session);
    }
}

/// A dodge press (the binding's count went up): when our mirror shows the hero up, free-footed and
/// with a charge, the controller hops them [`DodgeRules::distance`] yards the way they move, and
/// the press goes into the input. The first count read is a baseline.
///
/// [`DodgeRules::distance`]: nightfall_sim::DodgeRules
pub(super) fn key(
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    player: Res<crate::player::Player>,
    self_guid: Res<SelfGuid>,
    views: Res<Views>,
    bodies: Query<&Transform>,
    mut knocks: MessageWriter<KnockBackMessage>,
    mut input: ResMut<LocalInput>,
    mut seen: Local<Option<u32>>,
    mut counter: Local<u32>,
) {
    let Some(script) = script else {
        return;
    };
    let now: u32 = script.lua().globals().get(COUNTER).unwrap_or(0);
    let Some(last) = seen.replace(now) else {
        return;
    };
    if now == last {
        return;
    }
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let f = &hero.fight;
    if !(f.up() && f.dodge.have > 0 && !f.rooted && player.active) {
        return;
    }
    let (Some(guid), Some(tf)) = (
        self_guid.0,
        views.heroes.get(&sim.me).and_then(|&e| bodies.get(e).ok()),
    ) else {
        return;
    };
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    let way = dodge_way(V2::new(yaw.cos(), yaw.sin()), player.move_flags());
    *counter = counter.wrapping_add(1);
    knocks.write(KnockBackMessage {
        guid,
        counter: *counter,
        launch: hop(way, content().rules.combat.dodge.distance),
    });
    input.dodge_presses = input.dodge_presses.wrapping_add(1);
}
```

In `crates/benilla-app/src/nightfall/stance.rs`, Stealth leaves the raw key for the bar:
- The module doc's sentence "Stealth sits on `1` until sub-project 4 puts it on the action bar." becomes "Stealth is a button on the action bar ([`super::bar::presses`] counts its presses)."
- Delete `STEALTH_KEY`, and replace `stealth_key` with:

```rust
/// The autoplay `sneak` presses Stealth every [`AUTO_PRESS_EVERY`] seconds until the hero is in
/// stealth; a player presses Stealth's button on the action bar.
pub(super) fn stealth_key(
    sim: Res<Sim>,
    real: Res<Time<Real>>,
    mut input: ResMut<LocalInput>,
    mut last_auto: Local<Option<f32>>,
) {
    if !(autoplay() && auto_gait() == AutoGait::Sneak) {
        return;
    }
    let now = real.elapsed_secs();
    let out_of_stealth = sim.world.hero(sim.me).is_some_and(|h| !h.stealthed);
    if out_of_stealth && last_auto.is_none_or(|t| now - t >= AUTO_PRESS_EVERY) {
        input.stealth_presses = input.stealth_presses.wrapping_add(1);
        *last_auto = Some(now);
    }
}
```

  and delete the imports the compiler then calls unused (`UiScript`).

In `NightfallPlugin::build`, add:

```rust
            .add_systems(
                Update,
                (bar::presses, dodge::key)
                    // After the UI's key and click handlers queue `UseAction`, before the engine
                    // applies the script's calls, and before the input goes out.
                    .after(crate::ui_script::UiInput)
                    .before(crate::script_calls::apply_script_calls)
                    .before(run::feed_pose)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (bar::fill, bar::autofight, dodge::setup)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: See the bar and a fight begin**

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_CLASS=mage NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 WOW_LIVE_SHOT=target/fight-bar.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=3 WOW_LIVE_SHOT_EVERY=4 ./target/release/azeroth-together.exe > target/fight-bar.log 2>&1 &
```

Expected, once `target/fight-bar-002.png` exists (then `kill` the PID): the action bar holds Frostbolt, Fireball, Fire Blast, Frost Nova, Arcane Missiles, Polymorph, Blink and Conjure Water on 1 to 8, Throw a Stone on `-` and Eat and Drink on `=`, with WoW's own icons; an enemy is targeted (the target frame shows it), and enemies lose health (the target frame's bar drops) though nothing is drawn of the casts yet (Task 10). The log has no `ui_action: cast … refused` lines (the engine's cast path never saw the presses).

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: the action bar's presses go to the host; ground aim; a dodge key in WoW's bindings

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The fight shown: casts, cast bar, impacts, swings, combat text, refusals, Blink

**Files:**
- Create: `crates/benilla-app/src/nightfall/fx.rs`, `crates/benilla-app/src/nightfall/fight.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Event`, `Refusal`, `Stop`, `UnitRef`, `AbilityDef::{spell, tick_spell, channel}` (Tasks 1, 3); `run::Sim::events`, `run::take_events` (Task 3); `view::Views`, `enemies::EnemyViews`, `units::wow_to_bevy` (earlier plans); engine: `crate::creature_anim::{CastEvent, CastEventKind, PlaySeq, SpellGoTargets, SwingMessage}`, `crate::combat_text::CombatTextSpawn`, `crate::ui_cast::{CastBarFeed, CastBarEdge}`, `crate::ui_action::{UiErrorKeys, UiError}`, `crate::net::{TeleportMessage, SelfGuid}`.
- Produces:
  - `fx::Fx` (a `SystemParam`) with `start(Entity, u32)`, `fail(Entity, u32)`, `go(Entity, u32, &[Entity], Option<Vec3>)`, `swing(Entity, Entity)`, `number(Entity, f32, bool, u32)`, `text(Entity, String, u32)`; `fx::{COLOR_SPELL, COLOR_HIT, COLOR_HURT, COLOR_EVADE}`.
  - `fight::{refusal_key(Refusal, u8) -> &'static str, stop_edge(Stop) -> CastBarEdge, ms(f32) -> u32}`; system `fight::show_events`.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/fx.rs`:

```rust
//! The messages that make the host's fight look like a server's: `SMSG_SPELL_START`'s precast
//! glow, `SMSG_SPELL_GO`'s cast kit with a missile or an impact at each target, a failed cast, a
//! melee swing with its flinch and sound, and floating combat text. Copied from `together::units`'
//! `Fx`, which this game does not depend on, with the start and the failure added.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::combat_text::CombatTextSpawn;
use crate::creature_anim::{CastEvent, CastEventKind, PlaySeq, SpellGoTargets, SwingMessage};

/// More impacts than this per cast only cost frames.
const MAX_VISUAL_HITS: usize = 12;

/// A spell's damage, 0xAARRGGBB: yellow.
pub(super) const COLOR_SPELL: u32 = 0xFFFF_DE00;
/// A blow's damage: white.
pub(super) const COLOR_HIT: u32 = 0xFFFF_FFFF;
/// Damage to our own hero: red.
pub(super) const COLOR_HURT: u32 = 0xFFFF_3030;
/// A telegraphed blow that missed: pale blue.
pub(super) const COLOR_EVADE: u32 = 0xFF9F_D0FF;

#[derive(SystemParam)]
pub(super) struct Fx<'w> {
    seq: ResMut<'w, PlaySeq>,
    casts: MessageWriter<'w, CastEvent>,
    gos: MessageWriter<'w, SpellGoTargets>,
    swings: MessageWriter<'w, SwingMessage>,
    text: MessageWriter<'w, CombatTextSpawn>,
}

impl Fx<'_> {
    /// `SMSG_SPELL_START`: the precast kit on the caster, held until the cast goes or fails.
    pub fn start(&mut self, caster: Entity, spell: u32) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Start,
            seq,
        });
    }

    /// A cast that died without going off.
    pub fn fail(&mut self, caster: Entity, spell: u32) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Fail,
            seq,
        });
    }

    /// `SMSG_SPELL_GO`: the cast kit on the caster, and a missile or an impact at each hit.
    pub fn go(&mut self, caster: Entity, spell: u32, hits: &[Entity], dest: Option<Vec3>) {
        let seq = self.seq.next();
        self.casts.write(CastEvent {
            entity: caster,
            spell_id: spell,
            kind: CastEventKind::Go,
            seq,
        });
        self.gos.write(SpellGoTargets {
            caster,
            spell_id: spell,
            hits: hits.iter().copied().take(MAX_VISUAL_HITS).collect(),
            misses: Vec::new(),
            dest,
            ammo_display_id: None,
            seq,
        });
    }

    /// A melee swing: the attack animation, the victim's flinch, blood and the hit sound. Its
    /// number comes with the blow's own damage event.
    pub fn swing(&mut self, attacker: Entity, victim: Entity) {
        let seq = self.seq.next();
        self.swings.write(SwingMessage {
            attacker,
            victim: Some(victim),
            hit_info: 0x2,
            victim_state: 1,
            damage: 0,
            displayed: false,
            seq,
        });
    }

    /// A floating number over `anchor`, 0xAARRGGBB; a crit bigger.
    pub fn number(&mut self, anchor: Entity, amount: f32, crit: bool, color: u32) {
        let shown = amount.round().max(1.0) as i64;
        self.text.write(CombatTextSpawn {
            anchor,
            text: shown.to_string(),
            category: if crit { 2 } else { 0 },
            color: Some(color),
        });
    }

    /// Floating words over `anchor`, 0xAARRGGBB.
    pub fn text(&mut self, anchor: Entity, text: String, color: u32) {
        self.text.write(CombatTextSpawn {
            anchor,
            text,
            category: 0,
            color: Some(color),
        });
    }
}
```

`crates/benilla-app/src/nightfall/fight.rs`:

```rust
//! The fight, shown (spec §6). [`show_events`] plays what happened each step through the engine's
//! own seams, as a server's packets would: a cast's precast glow and our cast bar, a channel's
//! bar, the cast's visual with its missile and impacts, a stopped cast, melee swings, floating
//! combat text, WoW's own red error text for a press the host turned down, and a Blink's teleport
//! for our hero. It decides nothing: every event is the host's.

use bevy::prelude::*;
use nightfall_sim::{Event, Refusal, Stop, UnitRef};

use super::enemies::EnemyViews;
use super::fx::{Fx, COLOR_EVADE, COLOR_HIT, COLOR_HURT, COLOR_SPELL};
use super::run::Sim;
use super::units;
use super::view::Views;
use crate::net::{SelfGuid, TeleportMessage};
use crate::ui_action::{UiError, UiErrorKeys};
use crate::ui_cast::{CastBarEdge, CastBarFeed};

/// `ClassDef::power` of a class whose power is mana.
const MANA: u8 = 0;

/// WoW's own GlobalStrings key for a refusal, the one its client shows for the same mistake
/// (`NoPower` by the class's power).
pub(super) fn refusal_key(why: Refusal, power: u8) -> &'static str {
    match why {
        Refusal::Down => "SPELL_FAILED_CASTER_DEAD",
        Refusal::Busy => "SPELL_FAILED_SPELL_IN_PROGRESS",
        Refusal::NotReady => "SPELL_FAILED_NOT_READY",
        Refusal::NoPower if power == MANA => "ERR_OUT_OF_MANA",
        Refusal::NoPower => "ERR_OUT_OF_ENERGY",
        Refusal::Moving => "SPELL_FAILED_MOVING",
        Refusal::NeedStealth => "SPELL_FAILED_ONLY_STEALTHED",
        Refusal::InCombat => "SPELL_FAILED_AFFECTING_COMBAT",
        Refusal::NeedCombo => "SPELL_FAILED_NO_COMBO_POINTS",
        Refusal::NoTarget => "ERR_GENERIC_NO_TARGET",
        Refusal::BadTarget => "SPELL_FAILED_BAD_TARGETS",
        Refusal::Aware => "SPELL_FAILED_TARGET_AFFECTING_COMBAT",
        Refusal::OutOfRange => "SPELL_FAILED_OUT_OF_RANGE",
        Refusal::Facing => "SPELL_FAILED_UNIT_NOT_INFRONT",
        Refusal::Behind => "SPELL_FAILED_NOT_BEHIND",
        Refusal::NotInSight => "SPELL_FAILED_LINE_OF_SIGHT",
    }
}

/// Our cast bar's edge for a cast of ours that stopped: a channel's end empties it, a lost
/// target reads "Failed", anything else "Interrupted".
pub(super) fn stop_edge(why: Stop) -> CastBarEdge {
    match why {
        Stop::Finished => CastBarEdge::ChannelUpdate { remaining_ms: 0 },
        Stop::TargetLost => CastBarEdge::Failed,
        Stop::Moved | Stop::Cancelled | Stop::Interrupted => CastBarEdge::Interrupted,
    }
}

/// Seconds as the cast bar's milliseconds.
pub(super) fn ms(secs: f32) -> u32 {
    (secs * 1000.0).round().max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_reads_as_wow_says_it() {
        assert_eq!(refusal_key(Refusal::NoPower, 0), "ERR_OUT_OF_MANA");
        assert_eq!(refusal_key(Refusal::NoPower, 3), "ERR_OUT_OF_ENERGY");
        assert_eq!(refusal_key(Refusal::Behind, 3), "SPELL_FAILED_NOT_BEHIND");
        assert_eq!(refusal_key(Refusal::Aware, 3), "SPELL_FAILED_TARGET_AFFECTING_COMBAT");
    }

    #[test]
    fn a_stopped_cast_ends_its_bar_its_way() {
        assert!(matches!(
            stop_edge(Stop::Finished),
            CastBarEdge::ChannelUpdate { remaining_ms: 0 }
        ));
        assert!(matches!(stop_edge(Stop::TargetLost), CastBarEdge::Failed));
        assert!(matches!(stop_edge(Stop::Moved), CastBarEdge::Interrupted));
        assert_eq!(ms(2.5), 2500);
        assert_eq!(ms(-1.0), 0);
    }
}
```

In `crates/benilla-app/src/nightfall/mod.rs`, add `mod fight;` and `mod fx;` beside the other modules.

Run: `cargo test -p benilla-app --lib nightfall::fight -j 2`
Expected: PASS (2 tests).

- [ ] **Step 2: Write the system**

Append to `fight.rs`, above its tests:

```rust
/// This frame's events, shown: every hero's and drawn enemy's casts, swings and numbers; ours
/// also on our cast bar, in WoW's red error text, and (a Blink) in our controller's teleport.
/// An event about a unit not drawn (an enemy past the view distance) shows only what it can.
pub(super) fn show_events(
    sim: Res<Sim>,
    views: Res<Views>,
    enemy_views: Res<EnemyViews>,
    self_guid: Res<SelfGuid>,
    mut fx: Fx,
    mut bar: ResMut<CastBarFeed>,
    mut errors: ResMut<UiErrorKeys>,
    mut teleports: MessageWriter<TeleportMessage>,
    mut teleport_counter: Local<u32>,
) {
    let content = sim.world.content();
    let body = |u: UnitRef| match u {
        UnitRef::Hero(p) => views.heroes.get(&p).copied(),
        UnitRef::Enemy(id) => enemy_views.drawn.get(&id).copied(),
    };
    let me = UnitRef::Hero(sim.me);
    let power = sim
        .world
        .hero(sim.me)
        .map_or(MANA, |h| content.classes[h.class].power);
    for event in &sim.events {
        match event {
            Event::CastStart {
                by, ability, secs, ..
            } => {
                let spell = content.abilities[usize::from(*ability)].spell;
                if let Some(e) = body(*by) {
                    fx.start(e, spell);
                }
                if *by == me {
                    bar.0.push(CastBarEdge::Start {
                        spell_id: spell,
                        cast_time_ms: ms(*secs),
                    });
                }
            }
            Event::ChannelStart {
                by, ability, secs, ..
            } => {
                let spell = content.abilities[usize::from(*ability)].spell;
                if let Some(e) = body(*by) {
                    fx.start(e, spell);
                }
                if *by == me {
                    bar.0.push(CastBarEdge::ChannelStart {
                        spell_id: spell,
                        duration_ms: ms(*secs),
                    });
                }
            }
            Event::CastGo {
                by,
                ability,
                hits,
                at,
            } => {
                let a = &content.abilities[usize::from(*ability)];
                let channel = a.channel > 0.0;
                let spell = if channel {
                    a.tick_spell.unwrap_or(a.spell)
                } else {
                    a.spell
                };
                let struck: Vec<Entity> = hits.iter().filter_map(|u| body(*u)).collect();
                let dest = at.map(|p| units::wow_to_bevy([p.x, p.y, p.z]));
                if let Some(e) = body(*by) {
                    fx.go(e, spell, &struck, dest);
                }
                if *by == me && !channel {
                    bar.0.push(CastBarEdge::Stop);
                }
            }
            Event::CastStop { by, ability, why } => {
                let spell = content.abilities[usize::from(*ability)].spell;
                if let (Some(e), false) = (body(*by), *why == Stop::Finished) {
                    fx.fail(e, spell);
                }
                if *by == me {
                    bar.0.push(stop_edge(*why));
                }
            }
            Event::Damage {
                to, amount, crit, by, ..
            } => {
                let Some(e) = body(*to) else { continue };
                let color = if *to == me {
                    COLOR_HURT
                } else if matches!(by, Some(UnitRef::Hero(_))) {
                    COLOR_SPELL
                } else {
                    COLOR_HIT
                };
                fx.number(e, *amount, *crit, color);
            }
            Event::Swing { by, to } => {
                if let (Some(a), Some(v)) = (body(*by), body(*to)) {
                    fx.swing(a, v);
                }
            }
            Event::Evaded { player, .. } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    fx.text(e, "Evade".into(), COLOR_EVADE);
                }
            }
            Event::Refused { player, why } if *player == sim.me => {
                errors.0.push(UiError::key(refusal_key(*why, power)));
            }
            Event::Blinked { player, to } if *player == sim.me => {
                let (Some(guid), Some(hero)) = (self_guid.0, sim.world.hero(sim.me)) else {
                    continue;
                };
                *teleport_counter = teleport_counter.wrapping_add(1);
                teleports.write(TeleportMessage {
                    guid,
                    counter: *teleport_counter,
                    position: [to.x, to.y, to.z],
                    orientation: hero.facing.y.atan2(hero.facing.x),
                });
            }
            Event::Downed { player } | Event::Died { player } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    let word = if matches!(event, Event::Died { .. }) {
                        "Dead"
                    } else {
                        "Down!"
                    };
                    fx.text(e, word.into(), COLOR_HURT);
                }
            }
            Event::Revived { player, .. } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    fx.text(e, "Revived".into(), COLOR_EVADE);
                }
            }
            _ => {}
        }
    }
}
```

In `NightfallPlugin::build`, add:

```rust
            .add_systems(
                Update,
                fight::show_events
                    // After this frame's events are taken and every body they name is drawn.
                    .after(run::take_events)
                    .after(enemies::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: See the casts, the bar and the numbers**

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_CLASS=mage NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 WOW_LIVE_SHOT=target/fight-cast.png WOW_LIVE_SHOT_AT=36 WOW_LIVE_SHOT_COUNT=8 WOW_LIVE_SHOT_EVERY=1 ./target/release/azeroth-together.exe > target/fight-cast.log 2>&1 &
```

Expected, once `target/fight-cast-007.png` exists (then `kill` the PID): across the shots, the mage's hands glow while casting and WoW's cast bar fills under the character ("Frostbolt", then "Fireball"); a frostbolt flies to the target and its impact plays; yellow numbers rise over the target and red ones over the mage when struck; an enemy swinging plays its attack animation. If a press is refused (Fire Blast out of range), WoW's red "Out of range." shows at the top of the screen. Stop the run.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: casts, cast bar, impacts, swings, combat text and refusals shown as a server's would be

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The fight in the units' fields: power, combo points, cooldown sweeps, auras, roots, the sheep

**Files:**
- Create: `crates/benilla-app/src/nightfall/auras.rs`
- Modify: `crates/benilla-app/src/nightfall/fight.rs` (`fields`), `crates/benilla-app/src/nightfall/units.rs` (`set_vitals`), `crates/benilla-app/src/nightfall/view.rs` (`write_vitals`), `crates/benilla-app/src/nightfall/enemies.rs` (`sync`'s rank), `crates/benilla-app/src/nightfall/conceal.rs` (its `heroes` goes), `crates/benilla-app/src/nightfall/run.rs` (`back_to_menu`), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `HeroFight::{power, max_power, combo, combo_on, gcd, cooldowns, attacking, rooted, sprint, eating, life}`, `EnemyFight::{control, auras}`, `ClassDef::{power, kit}`, `AbilityDef::{spell, gcd, has}`, `EnemyDef::{display, elite}` (Tasks 1, 3); `conceal::bytes_1` (senses Task 9); `units::{unit_guid, UnitKind, set_vitals}`, `view::{Views, write_vitals}`, `enemies::{EnemyViews, sync}` (earlier plans); `fight::ms` (Task 10); engine: `crate::spell::Cooldowns::{clear_spell, apply_wire_cooldown, wipe}`, `crate::creature_anim::Engaged`, `crate::net::{ObjectStore, Guid, FieldChanged, SelfGuid, MoveModeMessage}`, `benilla_protocol::{MoveMode, field::{FIELD_UNIT_AURA, FIELD_UNIT_AURAFLAGS, FIELD_UNIT_BYTES_1}, messages::{ObjectFields, ObjectType}}`.
- Produces:
  - `fight::{power_pairs(u8, f32, f32) -> [(u16, u32); 2], combo_bytes(u8, u8) -> u32, waits(&[(u8, usize)], &[AbilityDef], &HeroFight) -> Vec<(u32, f32)>}`; system `fight::fields`.
  - `auras::{aura_fields(&[u32], &[u32]) -> Vec<(u16, u32)>, stand_of(&HeroFight) -> u8, HeroLook}`; systems `auras::{heroes, enemies}` (`auras::heroes` replaces `conceal::heroes`).
  - `units::set_vitals` no longer writes power; `view::write_vitals` shows a downed hero at 1 health.

- [ ] **Step 1: Write the pure parts with their tests**

Append to `crates/benilla-app/src/nightfall/fight.rs`, above `show_events`:

```rust
/// `UNIT_FIELD_POWER1` (the first of five power slots: mana, rage, focus, energy, happiness) and
/// `UNIT_FIELD_MAXPOWER1`.
const FIELD_POWER1: u16 = 23;
const FIELD_MAXPOWER1: u16 = 29;
/// `PLAYER_FIELD_BYTES`, whose byte 1 is the combo points, and `PLAYER_FIELD_COMBO_TARGET`, the
/// guid they sit on (two words).
const FIELD_PLAYER_BYTES: u16 = 1222;
const FIELD_COMBO_TARGET: u16 = 714;

/// A hero's power of type `ty` as its fields hold it.
pub(super) fn power_pairs(ty: u8, now: f32, max: f32) -> [(u16, u32); 2] {
    [
        (FIELD_POWER1 + u16::from(ty), now.max(0.0).round() as u32),
        (FIELD_MAXPOWER1 + u16::from(ty), max.max(0.0).round() as u32),
    ]
}

/// `PLAYER_FIELD_BYTES` with `combo` points in byte 1 and the action bar `toggles` in byte 2,
/// as they were; byte 0's flags (a ghost's release timer, tracking stealth) a hero never has.
pub(super) fn combo_bytes(toggles: u8, combo: u8) -> u32 {
    u32::from(toggles) << 16 | u32::from(combo) << 8
}

/// Each spell on the bar and the seconds it still waits: its own cooldown, or the global one
/// for an ability that starts it; a spell on two slots (Stealth on both bars) once.
pub(super) fn waits(
    kit: &[(u8, usize)],
    abilities: &[nightfall_sim::AbilityDef],
    f: &nightfall_sim::HeroFight,
) -> Vec<(u32, f32)> {
    let mut out: Vec<(u32, f32)> = Vec::new();
    for &(_, a) in kit {
        let own = f
            .cooldowns
            .iter()
            .find(|c| usize::from(c.ability) == a)
            .map_or(0.0, |c| c.left);
        let gcd = if abilities[a].gcd > 0.0 { f.gcd } else { 0.0 };
        let spell = abilities[a].spell;
        let wait = own.max(gcd);
        match out.iter_mut().find(|(s, _)| *s == spell) {
            Some(w) => w.1 = w.1.max(wait),
            None => out.push((spell, wait)),
        }
    }
    out
}
```

and to its tests:

```rust
    #[test]
    fn power_and_combo_points_go_in_their_fields() {
        assert_eq!(power_pairs(3, 72.4, 100.0), [(26, 72), (32, 100)]);
        assert_eq!(power_pairs(0, 1300.0, 1300.0), [(23, 1300), (29, 1300)]);
        assert_eq!(combo_bytes(3, 4), 0x0003_0400);
    }

    #[test]
    fn a_button_waits_out_its_cooldown_or_the_global_one() {
        use nightfall_sim::{Cooldown, HeroFight};
        let c = super::super::bar_test_content();
        let kit = &c.classes[0].kit;
        let f = HeroFight {
            gcd: 1.0,
            cooldowns: vec![Cooldown {
                ability: 2,
                left: 15.0,
            }],
            ..HeroFight::default()
        };
        // Stealth (no global cooldown), Strike (the global one), Stone (its own 15 s).
        assert_eq!(waits(kit, &c.abilities, &f), vec![(1785, 0.0), (1760, 1.0), (4164, 15.0)]);
    }
```

That test needs a small content of its own; add to `crates/benilla-app/src/nightfall/mod.rs`:

```rust
/// A rogue whose bar holds Stealth (on both bars), a strike that starts the global cooldown, and
/// a stone: content for the client's tests.
#[cfg(test)]
pub(super) fn bar_test_content() -> nightfall_sim::Content {
    nightfall_sim::Content::from_ron(nightfall_sim::ContentSources {
        classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
            base_hp: 1000.0, speed: 7.0, power_max: 100.0,
            bar: [(1, "stealth"), (2, "strike"), (11, "stone"), (73, "stealth")])]"#,
        map: r#"(key: "t", name: "T", map: 0,
            bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
            entries: [(5.0, 5.0, 0.0)])"#,
        rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
            fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
            hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
        enemies: "[]",
        spawns: "[]",
        abilities: r#"[
            (key: "stealth", name: "Stealth", spell: 1785, aim: Me, cooldown: 10.0,
             effects: [Stealth]),
            (key: "strike", name: "Strike", spell: 1760, aim: Enemy, range: 5.0, gcd: 1.0,
             effects: [Weapon(bonus: 22.0)]),
            (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0,
             cooldown: 20.0, effects: []),
        ]"#,
    })
    .unwrap()
}
```

`crates/benilla-app/src/nightfall/auras.rs`:

```rust
//! What is on each unit, as its aura fields would say (spec §6): a hero's Stealth, Sprint and meal
//! in their buff slots (the buff frame, and Stealth's fade), an enemy's holds, slows and bleeds in
//! its debuff slots (the target frame's debuffs, and each aura's own look: Frost Nova's ice, a
//! stun's stars, Polymorph's sparkle); a hero's stand state (kneeling downed, sitting at a meal),
//! stealth's creep bit, and the Stealth form that turns a rogue's bar to its stealth page (actions
//! 73 to 84, as WoW's own does when `ui_action`'s feed reads the form); and a polymorphed enemy's
//! sheep. Each unit is written only when what is on it changed, ringing the aura watch as a
//! server's values update would.

use std::collections::HashMap;

use benilla_protocol::field::{FIELD_UNIT_AURA, FIELD_UNIT_AURAFLAGS, FIELD_UNIT_BYTES_1};
use benilla_protocol::messages::{ObjectFields, ObjectType};
use bevy::prelude::*;
use nightfall_sim::{ControlKind, Effect, HeroFight, Life, PlayerId};

use super::conceal;
use super::enemies::EnemyViews;
use super::run::Sim;
use super::view::Views;
use crate::net::{FieldChanged, Guid, ObjectStore};

/// `UNIT_FIELD_DISPLAYID`: the model drawn, which the engine swaps in place when it moves.
const FIELD_UNIT_DISPLAYID: u16 = 131;
/// The sheep Polymorph turns an enemy into.
const SHEEP_DISPLAY: u32 = 856;
/// Stealth's aura when a class's bar has no Stealth of its own: Rank 3's, whose kit fades a body.
const STEALTH_AURA: u32 = 1786;
/// `SpellShapeshiftForm.dbc`'s Stealth: the form whose bonus bar is a rogue's stealth page.
const STEALTH_FORM: u8 = 30;
/// `UNIT_FIELD_BYTES_1` byte 0's stand states.
const STAND: u8 = 0;
const SIT: u8 = 1;
const KNEEL: u8 = 8;
/// Aura slots: 32 buffs, then 16 debuffs.
const BUFF_SLOTS: usize = 32;
const DEBUFF_SLOTS: usize = 16;
/// An aura's `AURAFLAGS` nibble: a cancelable buff's, and a debuff's (an effect bit alone).
const BUFF_FLAGS: u32 = 0x3;
const DEBUFF_FLAGS: u32 = 0x2;

/// Every aura field for `buffs` (from slot 0) and `debuffs` (from slot 32): each slot's spell, 0
/// where empty, and the six flag words.
pub(super) fn aura_fields(buffs: &[u32], debuffs: &[u32]) -> Vec<(u16, u32)> {
    let mut ids = [0u32; BUFF_SLOTS + DEBUFF_SLOTS];
    let mut flags = [0u32; (BUFF_SLOTS + DEBUFF_SLOTS) / 8];
    let slots = buffs
        .iter()
        .take(BUFF_SLOTS)
        .enumerate()
        .map(|(i, &s)| (i, s, BUFF_FLAGS))
        .chain(
            debuffs
                .iter()
                .take(DEBUFF_SLOTS)
                .enumerate()
                .map(|(j, &s)| (BUFF_SLOTS + j, s, DEBUFF_FLAGS)),
        );
    for (i, spell, f) in slots {
        ids[i] = spell;
        flags[i / 8] |= f << ((i % 8) * 4);
    }
    ids.iter()
        .enumerate()
        .map(|(i, &s)| (FIELD_UNIT_AURA + i as u16, s))
        .chain(
            flags
                .iter()
                .enumerate()
                .map(|(w, &f)| (FIELD_UNIT_AURAFLAGS + w as u16, f)),
        )
        .collect()
}

/// How a hero stands: kneeling while downed, sitting at a meal, else up.
pub(super) fn stand_of(f: &HeroFight) -> u8 {
    match f.life {
        Life::Downed { .. } => KNEEL,
        _ if f.eating > 0.0 => SIT,
        _ => STAND,
    }
}

/// What a hero's body shows of the fight.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct HeroLook {
    body: Entity,
    buffs: Vec<u32>,
    stand: u8,
    creep: bool,
    form: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffs_fill_from_the_first_slot_and_debuffs_from_the_thirty_third() {
        let f = aura_fields(&[1786, 2983], &[865]);
        assert_eq!(f.len(), 48 + 6);
        assert_eq!(f[0], (FIELD_UNIT_AURA, 1786));
        assert_eq!(f[1], (FIELD_UNIT_AURA + 1, 2983));
        assert_eq!(f[2], (FIELD_UNIT_AURA + 2, 0));
        assert_eq!(f[32], (FIELD_UNIT_AURA + 32, 865));
        // Slots 0 and 1 in the first flag word, slot 32 in the fifth.
        assert_eq!(f[48], (FIELD_UNIT_AURAFLAGS, 0x33));
        assert_eq!(f[52], (FIELD_UNIT_AURAFLAGS + 4, 0x2));
        assert!(aura_fields(&[], &[]).iter().all(|&(_, v)| v == 0));
    }

    #[test]
    fn the_downed_kneel_and_the_eating_sit() {
        let mut f = HeroFight::default();
        assert_eq!(stand_of(&f), STAND);
        f.eating = 3.0;
        assert_eq!(stand_of(&f), SIT);
        f.life = Life::Downed {
            left: 30.0,
            revive: 0.0,
        };
        assert_eq!(stand_of(&f), KNEEL);
    }
}
```

In `crates/benilla-app/src/nightfall/mod.rs`, add `mod auras;`.

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (the new tests in `fight` and `auras`).

- [ ] **Step 2: Write the systems**

Append to `fight.rs`, after `show_events`:

```rust
/// What `fields` last wrote, so it writes only changes.
#[derive(Default)]
pub(super) struct Written {
    power: HashMap<PlayerId, (Entity, [(u16, u32); 2])>,
    combo: Option<(u8, Option<u32>)>,
    /// When each bar spell's sweep ends, as written into the engine's cooldowns.
    ready_at: HashMap<u32, Instant>,
    attacking: Option<u32>,
    rooted: bool,
    counter: u32,
}

/// Every hero's power into their fields (the player, party and target frames); and for our hero:
/// combo points on their target (the combo frame), the bar's cooldown sweeps (written into the
/// engine's own cooldowns when they change by more than a blink), the ready stance while
/// auto-attacking (`Engaged`), and WoW's root while a rooted cast or death holds their feet.
pub(super) fn fields(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    self_guid: Res<SelfGuid>,
    mut stores: Query<&mut ObjectStore>,
    mut cooldowns: ResMut<Cooldowns>,
    mut modes: MessageWriter<MoveModeMessage>,
    mut written: Local<Written>,
) {
    let content = sim.world.content();
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let ty = content.classes[hero.class].power;
        let pairs = power_pairs(ty, hero.fight.power, hero.fight.max_power);
        if written.power.get(&hero.player) == Some(&(e, pairs)) {
            continue;
        }
        if let Ok(mut store) = stores.get_mut(e) {
            store.0.merge(ObjectFields::from_pairs(&pairs));
            written.power.insert(hero.player, (e, pairs));
        }
    }
    let (Some(me), Some(&body)) = (sim.world.hero(sim.me), views.heroes.get(&sim.me)) else {
        return;
    };
    let f = &me.fight;
    if written.combo != Some((f.combo, f.combo_on)) {
        if let Ok(mut store) = stores.get_mut(body) {
            let toggles = store.0.player_action_bar_toggles().unwrap_or(0);
            let on = f.combo_on.map_or(0, |id| {
                units::unit_guid(units::UnitKind::Creature { entry: 0 }, u64::from(id))
            });
            store.0.merge(ObjectFields::from_pairs(&[
                (FIELD_PLAYER_BYTES, combo_bytes(toggles, f.combo)),
                (FIELD_COMBO_TARGET, on as u32),
                (FIELD_COMBO_TARGET + 1, (on >> 32) as u32),
            ]));
            written.combo = Some((f.combo, f.combo_on));
        }
    }
    let now = Instant::now();
    for (spell, wait) in waits(&content.classes[me.class].kit, &content.abilities, f) {
        let end = (wait > 0.05).then(|| now + Duration::from_secs_f32(wait));
        let old = written.ready_at.get(&spell).copied();
        let moved = match (old, end) {
            (None, None) => false,
            (Some(a), Some(b)) => (if a > b { a - b } else { b - a }) > Duration::from_millis(150),
            _ => true,
        };
        if !moved {
            continue;
        }
        cooldowns.clear_spell(spell);
        match end {
            Some(end) => {
                cooldowns.apply_wire_cooldown(spell, ms(wait), None, now);
                written.ready_at.insert(spell, end);
            }
            None => {
                written.ready_at.remove(&spell);
            }
        }
    }
    if written.attacking != f.attacking {
        match f.attacking {
            Some(id) => {
                let guid = units::unit_guid(units::UnitKind::Creature { entry: 0 }, u64::from(id));
                commands.entity(body).insert(Engaged(guid));
            }
            None => {
                commands.entity(body).remove::<Engaged>();
            }
        }
        written.attacking = f.attacking;
    }
    if written.rooted != f.rooted {
        if let Some(guid) = self_guid.0 {
            written.counter = written.counter.wrapping_add(1);
            modes.write(MoveModeMessage {
                guid,
                counter: written.counter,
                mode: MoveMode::Root,
                apply: f.rooted,
            });
            written.rooted = f.rooted;
        }
    }
}
```

and add to `fight.rs`'s imports:

```rust
use std::collections::HashMap;
use std::time::Duration;

use benilla_protocol::messages::ObjectFields;
use benilla_protocol::MoveMode;
use bevy::platform::time::Instant;
use nightfall_sim::PlayerId;

use crate::creature_anim::Engaged;
use crate::net::{MoveModeMessage, ObjectStore};
use crate::spell::Cooldowns;
```

Append to `auras.rs`, above its tests:

```rust
/// The spell of the first ability on `class`'s bar that `pick` picks.
fn spell_on_bar(content: &nightfall_sim::Content, class: usize, pick: fn(&Effect) -> bool) -> Option<u32> {
    content.classes[class]
        .kit
        .iter()
        .map(|&(_, a)| &content.abilities[a])
        .find(|a| a.has(pick))
        .map(|a| a.spell)
}

/// Every hero's body shows their stealth, Sprint and meal as buffs, stands as they stand, creeps
/// in stealth and wears the Stealth form (a rogue's bar turns to its stealth page). Replaces
/// sub-project 3's `conceal::heroes`, which wrote Stealth's aura alone.
pub(super) fn heroes(
    sim: Res<Sim>,
    views: Res<Views>,
    mut bodies: Query<(&mut ObjectStore, &Guid)>,
    mut changed: MessageWriter<FieldChanged>,
    mut shown: Local<HashMap<PlayerId, HeroLook>>,
) {
    let content = sim.world.content();
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let stealth = spell_on_bar(content, hero.class, |e| matches!(e, Effect::Stealth))
            .unwrap_or(STEALTH_AURA);
        let sprint = spell_on_bar(content, hero.class, |e| matches!(e, Effect::Sprint { .. }));
        let meal = spell_on_bar(content, hero.class, |e| matches!(e, Effect::Eat { .. }));
        let buffs: Vec<u32> = [
            hero.stealthed.then_some(stealth),
            sprint.filter(|_| hero.fight.sprint > 0.0),
            meal.filter(|_| hero.fight.eating > 0.0),
        ]
        .into_iter()
        .flatten()
        .collect();
        let look = HeroLook {
            body: e,
            buffs,
            stand: stand_of(&hero.fight),
            creep: hero.stealthed,
            form: if hero.stealthed { STEALTH_FORM } else { 0 },
        };
        if shown.get(&hero.player) == Some(&look) {
            continue;
        }
        let Ok((mut store, guid)) = bodies.get_mut(e) else {
            continue;
        };
        let untrackable = store.0.unit_is_untrackable();
        let bytes = conceal::bytes_1(&store.0, Some(look.stand), look.creep, untrackable);
        let bytes = (bytes & !0x00FF_0000) | (u32::from(look.form) << 16);
        let mut pairs = vec![(FIELD_UNIT_BYTES_1, bytes)];
        pairs.extend(aura_fields(&look.buffs, &[]));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        changed.write(FieldChanged {
            entity: e,
            guid: guid.0,
            kind: ObjectType::Player,
            index: FIELD_UNIT_AURA,
            old: 0,
            new: 1,
        });
        shown.insert(hero.player, look);
    }
}

/// Every drawn enemy shows what is on it as debuffs, and a sheep while polymorphed.
pub(super) fn enemies(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    mut bodies: Query<(&mut ObjectStore, &Guid)>,
    mut changed: MessageWriter<FieldChanged>,
    mut shown: Local<HashMap<u32, (Entity, Vec<u32>, u32)>>,
) {
    let content = sim.world.content();
    for enemy in &sim.world.enemies {
        let Some(&e) = views.drawn.get(&enemy.id) else {
            continue;
        };
        let display = if enemy.fight.control == Some(ControlKind::Polymorph) {
            SHEEP_DISPLAY
        } else {
            content.enemies[enemy.kind].display
        };
        let now = (e, enemy.fight.auras.clone(), display);
        if shown.get(&enemy.id) == Some(&now) {
            continue;
        }
        let Ok((mut store, guid)) = bodies.get_mut(e) else {
            continue;
        };
        let mut pairs = aura_fields(&[], &enemy.fight.auras);
        pairs.push((FIELD_UNIT_DISPLAYID, display));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        changed.write(FieldChanged {
            entity: e,
            guid: guid.0,
            kind: ObjectType::Unit,
            index: FIELD_UNIT_AURA,
            old: 0,
            new: 1,
        });
        shown.insert(enemy.id, now);
    }
}
```

In `crates/benilla-app/src/nightfall/conceal.rs`: delete `heroes`, `stealth_aura`, `STEALTH_AURA`, `AURA_LIVE` and the test `stealth_puts_its_aura_in_the_first_slot_and_takes_it_out`; its module doc's last sentence becomes "A hero's stealth look is `auras::heroes`'." Keep `bytes_1` and its tests.

In `crates/benilla-app/src/nightfall/units.rs`'s `set_vitals`, delete the two power pairs (`(FIELD_POWER, 100)`, `(FIELD_MAX_POWER, 100)`) and the comment above them, and the constants if now unused: power is the fight's ([`super::fight::fields`]).

In `crates/benilla-app/src/nightfall/view.rs`'s `write_vitals`:
- The controller's speed is never 0 (a rooted hero is held by WoW's root, not by speed): `run_speeds(hero.speed)` becomes `run_speeds(hero.speed.max(0.1))`.
- A downed hero shows 1 health, so WoW plays no death until they die: the `set_vitals(…, hero.hp, …)` call takes `shown_hp` instead, with, before it:

```rust
            let shown_hp = match hero.fight.life {
                nightfall_sim::Life::Downed { .. } => hero.hp.max(1.0),
                _ => hero.hp,
            };
```

In `crates/benilla-app/src/nightfall/enemies.rs`'s `sync`, the `CreatureRecord { … }` literal's `rank: 0,` becomes `rank: u32::from(def.elite),` (WoW's rank 1: the elite dragon on the target frame).

In `crates/benilla-app/src/nightfall/run.rs`'s `back_to_menu`, add the parameters `mut cooldowns: ResMut<crate::spell::Cooldowns>` and `mut modes: MessageWriter<crate::net::MoveModeMessage>`, and before `next.set(Phase::Menu);`:

```rust
    // Nothing of the fight follows the player out: no sweeps on the bar, no root on their feet.
    cooldowns.wipe();
    modes.write(crate::net::MoveModeMessage {
        guid: 0,
        counter: 0,
        mode: benilla_protocol::MoveMode::Root,
        apply: false,
    });
```

In `NightfallPlugin::build`, the senses' chain `(conceal::enemies, conceal::heroes, stance::trace)` becomes `(conceal::enemies, auras::heroes, auras::enemies, stance::trace)`, and add:

```rust
            .add_systems(
                Update,
                fight::fields
                    .after(view::write_vitals)
                    .after(enemies::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: See power, combo points, sweeps, debuffs, the stealth bar and the sheep**

The Rogue, sneaking up and fighting:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOPLAY_GAIT=sneak WOW_LIVE_SHOT=target/fight-rogue.png WOW_LIVE_SHOT_AT=30 WOW_LIVE_SHOT_COUNT=2 WOW_LIVE_SHOT_EVERY=4 ./target/release/azeroth-together.exe > target/fight-rogue.log 2>&1 &
```

Expected, once `target/fight-rogue-001.png` exists (then `kill` the PID): the rogue stealthed shows the Stealth buff and the stealth page of the bar (Ambush, Garrote, Cheap Shot and Sap on 2 to 5). If the bar stays on the main page, the Stealth form did not turn it: note it in the hand-off, and the fallback is a content change (move the openers to the main bar's 8 to 10, and drop the 73-to-84 lines).

Then the fight (no sneaking), and the Mage's sheep:

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 WOW_LIVE_SHOT=target/fight-fields.png WOW_LIVE_SHOT_AT=36 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=2 ./target/release/azeroth-together.exe > target/fight-fields.log 2>&1 &
```

Expected: the rogue's energy bar falls with each strike and climbs again; combo point gems light on the target frame; the bar's buttons sweep for the global cooldown after each press and Gouge's sweep runs on; the target frame shows Gouge's debuff icon while it holds, and an elite target (Fenros, Naraxis) has the elite dragon. With `NIGHTFALL_CLASS=mage` and the same command (shots to `target/fight-mage.png`), the mana bar falls with each spell, the Frostbolt slow shows as a debuff, and a polymorphed enemy turns into a sheep (if it does not, note it: the debuff still shows and it still wanders). Stop each run.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: power, combo points, cooldown sweeps, auras, the stealth bar, roots and sheep in the units' fields

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Telegraphs on the ground, and the fallen player's line

**Files:**
- Create: `crates/benilla-app/src/nightfall/telegraphs.rs`, `crates/benilla-app/src/nightfall/fallen.rs`
- Modify: `crates/benilla-app/src/nightfall/run.rs` (`step_sim`'s `NIGHTFALL_FRAIL`, `back_to_menu`), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `Telegraph { id, shape, at, facing, left, total }`, `Shape`, `Life`, `Hero`, `CombatRules::{revive_for, revive_reach}`, `World::{telegraphs, hurt_hero}` (Tasks 1, 3, 7); `run::Sim`, `units::wow_to_bevy`, `NightfallEntity`, `crate::char_select::wow_font` (earlier plans); `WorldPoint::terrain_height_under`; `UiScript::run`.
- Produces:
  - `telegraphs::{ground_grid(Shape, V2, V2) -> (Vec<V2>, Vec<u32>), glow(f32, f32) -> f32}`, `telegraphs::TelegraphViews` (a `Resource`); system `telegraphs::draw`.
  - `fallen::{status(&Hero, &[Hero], &CombatRules) -> Option<String>, crossed(at: f32, elapsed: f32) -> bool, frail_player() -> Option<PlayerId>}`, `fallen::StatusLine` (a `Component`); systems `fallen::{status_line, death_popup}`.

- [ ] **Step 1: Write the pure parts with their tests**

`crates/benilla-app/src/nightfall/telegraphs.rs`:

```rust
//! Telegraphs (spec §6.1): an elite's big move shown on the ground where it will land, 1 to 1.5
//! seconds before it does: a red circle or cone draped over the terrain, glowing brighter as the
//! blow draws near, gone when it lands. Our own unlit mesh, not one of WoW's decals (which have a
//! circle only); concealed enemies' telegraphs show too, since the ground glows whoever casts.

use std::collections::{HashMap, HashSet};

use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use nightfall_sim::{Shape, V2};

use super::run::Sim;
use super::{units, NightfallEntity};

/// Rings from the centre out, and points around (or across) each.
const RINGS: usize = 4;
const SEGMENTS: usize = 32;
/// Yards over the ground the shape floats, so the terrain never hides it.
const LIFT: f32 = 0.12;

/// The ground a telegraph covers, as rings of points out from where it is put (the first point
/// its centre) and the triangles between them, indices into the points.
pub(super) fn ground_grid(shape: Shape, at: V2, facing: V2) -> (Vec<V2>, Vec<u32>) {
    let (reach, span, wrap) = match shape {
        Shape::Circle { radius } => (radius, std::f32::consts::TAU, true),
        Shape::Cone { length, angle } => (length, angle.to_radians(), false),
    };
    let base = facing.y.atan2(facing.x) - if wrap { 0.0 } else { span / 2.0 };
    let per_ring = if wrap { SEGMENTS } else { SEGMENTS + 1 };
    let mut points = vec![at];
    for k in 1..=RINGS {
        let r = reach * k as f32 / RINGS as f32;
        for j in 0..per_ring {
            let a = base + span * j as f32 / SEGMENTS as f32;
            points.push(at + V2::from_angle(a, r));
        }
    }
    let ring = |k: usize, j: usize| (1 + (k - 1) * per_ring + j % per_ring) as u32;
    let mut tris = Vec::new();
    for j in 0..SEGMENTS {
        tris.extend([0, ring(1, j), ring(1, j + 1)]);
        for k in 1..RINGS {
            let (a, b, c, d) = (ring(k, j), ring(k, j + 1), ring(k + 1, j), ring(k + 1, j + 1));
            tris.extend([a, c, b, b, c, d]);
        }
    }
    (points, tris)
}

/// The shape's alpha as its blow draws near: faint as it shows, strong as it lands.
pub(super) fn glow(left: f32, total: f32) -> f32 {
    0.15 + 0.5 * (1.0 - (left / total.max(1e-3)).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circle_is_rings_of_points_around_its_centre() {
        let at = V2::new(10.0, 20.0);
        let (points, tris) = ground_grid(Shape::Circle { radius: 8.0 }, at, V2::new(1.0, 0.0));
        assert_eq!(points.len(), 1 + RINGS * SEGMENTS);
        assert_eq!(tris.len(), 3 * SEGMENTS * (1 + 2 * (RINGS - 1)));
        assert!(tris.iter().all(|&i| (i as usize) < points.len()));
        let outer = points.last().unwrap().distance(at);
        assert!((outer - 8.0).abs() < 1e-3);
    }

    #[test]
    fn a_cone_spreads_either_side_of_its_facing() {
        let at = V2::ZERO;
        let (points, tris) = ground_grid(
            Shape::Cone {
                length: 8.0,
                angle: 120.0,
            },
            at,
            V2::new(1.0, 0.0),
        );
        assert_eq!(points.len(), 1 + RINGS * (SEGMENTS + 1));
        assert!(tris.iter().all(|&i| (i as usize) < points.len()));
        // Every point lies within 60° of north.
        assert!(points[1..].iter().all(|p| p.normalize_or_zero().x >= 0.5 - 1e-4));
    }

    #[test]
    fn the_glow_grows_as_the_blow_nears() {
        assert!((glow(1.5, 1.5) - 0.15).abs() < 1e-6);
        assert!((glow(0.0, 1.5) - 0.65).abs() < 1e-6);
        assert!(glow(0.5, 1.5) > glow(1.0, 1.5));
    }
}
```

`crates/benilla-app/src/nightfall/fallen.rs`:

```rust
//! A fallen player's own words (spec §8.1): a line low on the screen while their hero is down
//! ("You are down: 32 s."), being revived, dead, or reviving a friend; WoW's death popup kept
//! away, since there is no spirit to release; and, for test runs, `NIGHTFALL_FRAIL=<player>`,
//! which drops that hero 15 seconds into the run on the host, to see a fall and a revive.

use std::sync::OnceLock;

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use nightfall_sim::{CombatRules, Hero, Life, PlayerId, DT};

use super::run::Sim;
use super::NightfallEntity;

/// Seconds into a run `NIGHTFALL_FRAIL`'s hero falls.
pub(super) const FRAIL_AT: f32 = 15.0;
/// Seconds between closing WoW's death popup again while dead.
const POPUP_EVERY: f32 = 0.5;
/// Closes WoW's death popup if it is up.
const HIDE_POPUP: &str =
    r#"if StaticPopup_Visible and StaticPopup_Visible("DEATH") then StaticPopup_Hide("DEATH") end"#;

/// What the line says for our hero, if anything: down (and how long they have), being revived,
/// dead, or reviving a downed friend within reach.
pub(super) fn status(me: &Hero, heroes: &[Hero], r: &CombatRules) -> Option<String> {
    match me.fight.life {
        Life::Downed { revive, .. } if revive > 0.0 => Some(format!(
            "Being revived... {:.0}%",
            (revive / r.revive_for * 100.0).min(100.0)
        )),
        Life::Downed { left, .. } => Some(format!(
            "You are down: {:.0} s. A friend who stands by you can get you up.",
            left.ceil()
        )),
        Life::Dead => Some("You are dead. Your body lies where you fell.".into()),
        Life::Up => heroes
            .iter()
            .find(|h| {
                h.player != me.player
                    && matches!(h.fight.life, Life::Downed { .. })
                    && h.pos.distance(me.pos) <= r.revive_reach
            })
            .map(|h| format!("Reviving {}...", h.name)),
    }
}

/// Whether the step that brought the run to `elapsed` seconds crossed `at`.
pub(super) fn crossed(at: f32, elapsed: f32) -> bool {
    elapsed >= at && elapsed - DT < at
}

/// `NIGHTFALL_FRAIL`'s player, if set.
pub(super) fn frail_player() -> Option<PlayerId> {
    static PLAYER: OnceLock<Option<PlayerId>> = OnceLock::new();
    *PLAYER.get_or_init(|| std::env::var("NIGHTFALL_FRAIL").ok()?.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{HeroFight, Stance, V2};

    fn hero(player: PlayerId, x: f32, life: Life) -> Hero {
        Hero {
            player,
            class: 0,
            name: format!("Hero{player}"),
            race: 1,
            sex: 0,
            pos: V2::new(x, 0.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 0.0,
            max_hp: 1000.0,
            speed: 7.0,
            stance: Stance::Still,
            stealthed: false,
            visibility: 35.0,
            noise: 0.0,
            fight: HeroFight {
                life,
                ..HeroFight::default()
            },
        }
    }

    #[test]
    fn the_line_says_how_a_hero_stands() {
        let r = CombatRules::default();
        let down = hero(0, 0.0, Life::Downed { left: 31.2, revive: 0.0 });
        assert_eq!(
            status(&down, &[], &r).unwrap(),
            "You are down: 32 s. A friend who stands by you can get you up."
        );
        let reviving = hero(0, 0.0, Life::Downed { left: 20.0, revive: 2.5 });
        assert_eq!(status(&reviving, &[], &r).unwrap(), "Being revived... 50%");
        assert!(status(&hero(0, 0.0, Life::Dead), &[], &r).unwrap().starts_with("You are dead"));
        let me = hero(1, 2.0, Life::Up);
        let friend = hero(0, 0.0, Life::Downed { left: 20.0, revive: 0.5 });
        assert_eq!(
            status(&me, &[friend.clone(), me.clone()], &r).unwrap(),
            "Reviving Hero0..."
        );
        let far = hero(1, 10.0, Life::Up);
        assert_eq!(status(&far, &[friend, far.clone()], &r), None);
    }

    #[test]
    fn the_frail_hook_falls_once() {
        assert!(!crossed(FRAIL_AT, FRAIL_AT - DT));
        assert!(crossed(FRAIL_AT, FRAIL_AT + DT * 0.5));
        assert!(!crossed(FRAIL_AT, FRAIL_AT + DT * 1.5));
    }
}
```

(`Hero`'s literal must name every field the earlier plans gave it; if the compiler names one this list misses, add it with its default.)

In `crates/benilla-app/src/nightfall/mod.rs`, add `mod fallen;` and `mod telegraphs;`.

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS (3 in `telegraphs`, 2 in `fallen`).

- [ ] **Step 2: Write the systems**

Append to `telegraphs.rs`, above its tests:

```rust
/// Which entity draws which telegraph, and its material (whose alpha follows the windup).
#[derive(Resource, Default)]
pub(super) struct TelegraphViews {
    drawn: HashMap<u32, (Entity, Handle<StandardMaterial>)>,
}

fn red(alpha: f32) -> Color {
    Color::srgba(0.95, 0.12, 0.08, alpha)
}

/// Every telegraph in the world gets a shape on the ground, made once (its points on the
/// client's own terrain) and glowing as it nears; one that landed or went goes.
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut views: ResMut<TelegraphViews>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for t in &sim.world.telegraphs {
        let alpha = glow(t.left, t.total);
        if let Some((_, mat)) = views.drawn.get(&t.id) {
            if let Some(m) = materials.get_mut(mat) {
                m.base_color = red(alpha);
            }
            continue;
        }
        let (points, tris) = ground_grid(t.shape, t.at.ground(), t.facing);
        let positions: Vec<[f32; 3]> = points
            .iter()
            .map(|p| {
                let mut b = units::wow_to_bevy([p.x, p.y, t.at.z]);
                b.y = world_point.terrain_height_under(b).unwrap_or(b.y) + LIFT;
                b.to_array()
            })
            .collect();
        let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_indices(Indices::U32(tris));
        let mat = materials.add(StandardMaterial {
            base_color: red(alpha),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let e = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(mat.clone()),
                Transform::IDENTITY,
                Visibility::default(),
                NightfallEntity,
            ))
            .id();
        views.drawn.insert(t.id, (e, mat));
    }
    let live: HashSet<u32> = sim.world.telegraphs.iter().map(|t| t.id).collect();
    views.drawn.retain(|id, (e, _)| {
        let keep = live.contains(id);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
}
```

Append to `fallen.rs`, above its tests:

```rust
/// The line's node.
#[derive(Component)]
pub(super) struct StatusLine;

/// The line follows our hero: made with the run (it goes with the run's other entities), shown
/// while there is something to say.
pub(super) fn status_line(
    mut commands: Commands,
    sim: Res<Sim>,
    assets: Res<AssetServer>,
    mut roots: Query<(&mut Visibility, &Children), With<StatusLine>>,
    mut texts: Query<&mut Text>,
) {
    let Ok((mut visibility, children)) = roots.single_mut() else {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    bottom: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                GlobalZIndex(300),
                Visibility::Hidden,
                StatusLine,
                NightfallEntity,
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(""),
                    TextFont {
                        font: crate::char_select::wow_font(&assets),
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.0)),
                    TextShadow::default(),
                ));
            });
        return;
    };
    let said = sim
        .world
        .hero(sim.me)
        .and_then(|me| status(me, &sim.world.heroes, &sim.world.content().rules.combat));
    let want = if said.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != want {
        *visibility = want;
    }
    if let Some(said) = said {
        for &child in children {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != said {
                    text.0 = said.clone();
                }
            }
        }
    }
}

/// While our hero is dead, WoW's "Release Spirit" popup is closed whenever it shows: there is no
/// spirit to release, and its button would ask a server that is not there.
pub(super) fn death_popup(
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
) {
    let dead = sim
        .world
        .hero(sim.me)
        .is_some_and(|h| h.fight.life == Life::Dead);
    let now = real.elapsed_secs();
    if !dead || now - *last < POPUP_EVERY {
        return;
    }
    *last = now;
    if let Some(script) = script {
        let _ = script.run(HIDE_POPUP);
    }
}
```

In `crates/benilla-app/src/nightfall/run.rs`'s `step_sim`, in the host loop after `sim.world.step();` add:

```rust
        if let Some(p) = super::fallen::frail_player() {
            if super::fallen::crossed(super::fallen::FRAIL_AT, sim.world.elapsed) {
                sim.world.hurt_hero(p, f32::MAX);
            }
        }
```

and the module doc's test hooks gain "`NIGHTFALL_FRAIL=<player>` drops that hero 15 s into the run (the host's)". In `back_to_menu`, add the parameter `mut telegraph_views: ResMut<super::telegraphs::TelegraphViews>,` and after the enemy views' reset `*telegraph_views = super::telegraphs::TelegraphViews::default();` (the shapes are `NightfallEntity`s, which it already despawns).

In `NightfallPlugin::build`, add `.init_resource::<telegraphs::TelegraphViews>()` beside the other views, and:

```rust
            .add_systems(
                Update,
                (telegraphs::draw, fallen::status_line, fallen::death_popup)
                    .after(enemies::place)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 3: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds.

- [ ] **Step 4: See a telegraph, a dodge through it, a fall and the line**

Fenros brought next to the entry in a scratch copy of the content (never in `content/`), the autofight dodging what it can:

```bash
rm -rf target/nf-fight && cp -r content/nightfall target/nf-fight
sed -i 's/Sentry(at: (-10100.0, -840.0), facing: 45.0)/Sentry(at: (-10058.0, -780.0), facing: 45.0)/' target/nf-fight/spawns.ron
grep -c "(-10058.0, -780.0)" target/nf-fight/spawns.ron
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_CONTENT=target/nf-fight NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 WOW_SHOT_REQUIRE="Fenros" WOW_LIVE_SHOT=target/fight-telegraph.png WOW_LIVE_SHOT_AT=30 WOW_LIVE_SHOT_COUNT=12 WOW_LIVE_SHOT_EVERY=1 ./target/release/azeroth-together.exe > target/fight-telegraph.log 2>&1 &
```

Expected: the `grep` prints `1`. Once `target/fight-telegraph-011.png` exists (then `kill` the PID): in some shot a red cone lies on the ground before Fenros, brightening; in a later one it is gone; the rogue hops away (a short jump arc) as it shows, and "Evade" rises over them when the dodge's moment covered the blow, else a big red number. The target frame shows Fenros with the elite dragon.

A fall, without enemies' help (the autoplay starts the run, solo, 12 s in; the hero falls 15 s into the run and dies 45 s later):

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_FRAIL=0 WOW_LIVE_SHOT=target/fight-down.png WOW_LIVE_SHOT_AT=36 WOW_LIVE_SHOT_COUNT=2 WOW_LIVE_SHOT_EVERY=45 ./target/release/azeroth-together.exe > target/fight-down.log 2>&1 &
```

Expected: `target/fight-down-000.png` shows the hero kneeling with 1 health on the player frame and the gold line "You are down: … s. A friend who stands by you can get you up."; `target/fight-down-001.png` (45 s later) shows the hero dead, "You are dead. Your body lies where you fell." and no "Release Spirit" popup. If the hero is not yet dead in the second shot (a slow load), take one more with `WOW_LIVE_SHOT_EVERY=55`. Stop the run.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add crates/benilla-app/src/nightfall
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: telegraphs drawn on the ground; the downed and dead player's line; NIGHTFALL_FRAIL

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 13: Docs, the full check, and a fight in co-op

**Files:**
- Modify: `CLAUDE.md`, `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line, §13's dodge key)

- [ ] **Step 1: Document the fight in `CLAUDE.md`**

Under "Where things live", after the Nightfall senses entry sub-project 3 added, add:

```markdown
- Nightfall's fight (sub-project 4): abilities are content (`content/nightfall/abilities.ron`,
  `nightfall-sim/src/abilities.rs`): each with its Spell.dbc rank at level 30 (`spell`: icon,
  tooltip, cast-bar name, visual), cost, cast or channel, cooldown, GCD, range, facing, what its
  caster may do meanwhile (`moving`: Free, Slowed, Rooted), its `sound`, needs and effects; heroes'
  bars (`classes.ron`'s `bar`, WoW action ids, 73-84 the rogue's stealth page) and enemies' `moves`
  name them by key. `nightfall-sim/src/combat/`: `rules.rs` (dice, arcs, ranges, the casting
  allowance, dodge charges), `effects.rs` (damage with the ambush and crits, holds, slows, bleeds,
  combos, Vanish, Blink, Distract, meals), `heroes.rs` (presses taken once, WoW's refusals, casts
  and channels held to their feet, auto-attack, regeneration), `hold.rs` (holds and burns ticking,
  sheep, what each enemy's senses may do), `falling.rs` (downed 45 s, revived in 5 s, dead),
  `strikes.rs` (enemy swings, casts and telegraphs; the dodge). Numbers in `rules.ron`'s
  `combat:`. Every hero's and enemy's fight rides in the snapshot (`HeroFight`, `EnemyFight`),
  telegraphs beside them, `Event`s in `State` (protocol 4). Client: `nightfall/bar.rs` (the bar
  filled; WoW's `UseAction` taken before the engine's cast path and sent as a `CastPress` at the
  target or the ground under the cursor), `dodge.rs` (the `NIGHTFALL_DODGE` binding, Caps Lock and
  mouse 5 by default, in WoW's Key Bindings window; a knockback hop), `fight.rs` (events through
  the engine's cast bar, spell visuals, swings, combat text and red errors; power, combo points,
  cooldown sweeps, the ready stance and WoW's root in the fields), `auras.rs` (buffs, debuffs,
  stand states, the stealth form, sheep), `telegraphs.rs` (red shapes on the terrain), `fallen.rs`
  (the downed player's line).
```

Under "Build, test, run", add:

```markdown
- Nightfall's fight: `NIGHTFALL_AUTOFIGHT=1` targets the nearest enemy (WoW's own Tab), presses
  the hero's attacks in turn and dodges telegraphs; `NIGHTFALL_FRAIL=<player>` drops that hero 15 s
  into the run (the host's), to see a fall, the line and a revive. The dodge is WoW's key binding
  `NIGHTFALL_DODGE` (Key Bindings → Nightfall).
```

- [ ] **Step 2: Mark the spec**

In the spec's status line, after the sentence sub-project 3 added, add: `Sub-project 4 built (plan docs/superpowers/plans/2026-10-17-nightfall-combat.md).` In §13, the "Dodge key" bullet becomes: `- **Dodge key:** decided in sub-project 4: a WoW key binding (NIGHTFALL_DODGE), Caps Lock and the mouse's back button by default, both free in 1.12's default bindings; rebound in WoW's own Key Bindings window.`

- [ ] **Step 3: Run every test this work touches**

Run: `cargo test -p nightfall-sim -p nightfall-bake -p together-net -p together-sim`
Expected: PASS (the Darkshire bake test skips without `WOW_DATA`).
Run: `cargo test -p benilla-app --lib nightfall -j 2`
Expected: PASS.
Run: `cargo test -p benilla-app --lib target -j 2`
Expected: PASS (the engine's targeting, which the bar's presses order around, is untouched).
Run: `cargo build --release -p azeroth-together -j 2`
Expected: builds with no new warnings in `nightfall-sim` or `nightfall/`.

- [ ] **Step 4: A fight in co-op on one PC**

Start the host (a Rogue) in the background, and a few seconds later the client (a Mage):

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 NIGHTFALL_FRAIL=0 NIGHTFALL_NAME=Host WOW_LIVE_SHOT=target/host-fight.png WOW_LIVE_SHOT_AT=45 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/host-fight.log 2>&1 &
```

```bash
WOW_DATA=D:/Games/azeroth-together/WoW/Data BENILLA_HOME=target/nf-home WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_AUTOFIGHT=1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend WOW_LIVE_SHOT=target/client-fight.png WOW_LIVE_SHOT_AT=40 WOW_LIVE_SHOT_COUNT=6 WOW_LIVE_SHOT_EVERY=5 ./target/release/azeroth-together.exe > target/client-fight.log 2>&1 &
```

Expected, once both last shots exist (then `kill` both PIDs):
- The client's shots show its own mage casting with WoW's cast bar and mana falling, and the host's rogue fighting (swings and numbers over the enemies it strikes, as the host's events arrive).
- The host's hero falls 15 s into the run (`NIGHTFALL_FRAIL=0`); the autoplay circles bring the mage past the body at times: when it stays within 3 yards long enough the host's log shows nothing amiss and the host's shots show "Being revived... " then the rogue up again; if the circles never line up, the host dies after 45 s with "You are dead" — either is right, and the mage's line reads "Reviving Host..." while it is near.
- Neither log has a panic, a `Refused` loop or a disconnect.

- [ ] **Step 5: Commit**

```bash
git -c safe.directory=D:/Games/azeroth-together-stealth add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md
git -c safe.directory=D:/Games/azeroth-together-stealth commit -m "Nightfall: document the fight; sub-project 4 built

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

## Self-review against the spec

| Spec | Where |
|---|---|
| §6.1 WoW tab-target: action bars, cast bars, GCD, ranges, facing | Tasks 5 (checks, GCD, ranges, facing), 9 (the bar, `UseAction`, WoW's Tab as the target), 10 (cast bar), 11 (sweeps) |
| §6.1 fixed level 30 | Task 1 (level-30 ranks and hero numbers; `hero_level` already 30) |
| §6.1 movement while casting free/slowed/rooted, `cast_move_speed` 50% | Tasks 1 (`moving`), 2 (`allowance`), 5 (speed, root, breaks), 11 (WoW's root) |
| §6.1 Dodge: 6 yd, 2 charges, 8 s each, telegraphed hits miss 0.3 s, 20 yd noise | Tasks 2 (`Charges`), 8 (`dodge`, the window), 9 (the hop and the key) |
| §6.1 Throw a stone: 20 yd noise at a spot within 25 yd, every 20 s | Task 1 (`throw_stone`), 5 (ground aim's checks, sound at the spot), 9 (cursor aim) |
| §6.1 Eat and drink over 15 s sitting still; nothing regenerates in combat | Tasks 4 (meal), 5 (meal stops on a step, regen only out of combat), 11 (sitting) |
| §6.1 Telegraphs on elites/rares, 1 to 1.5 s, circle or cone; normal enemies swing plainly | Tasks 1 (Fenros, Naraxis), 8, 12 |
| §6.2 Mage kit with its noise and movement | Task 1 (`abilities.ron`), 4 (Polymorph sheep, Blink, Frost Nova root, conjured drinks) |
| §6.3 Rogue kit: Stealth 70%, openers, combo points, Gouge from the front, Sprint 8 s, Distract 40 yd, Vanish 5 min | Tasks 1, 4 (Sap, combos, Vanish, Distract), 5 (Stealth's cooldown and combat rule) |
| §5.4 ambush ×1.5 first hit; openers need an unaware target | Tasks 4 (`World::ambush` before the alert), 5 (`Needs::unaware` via `opener_allowed`) |
| §7.1 casters fight at range; elites use telegraphs | Tasks 6 (`keep_at`), 8 |
| §8.1 downed 45 s, crawl, revive 5 s with a 20 yd noise, dead after | Tasks 7, 11 (kneel, 1 health), 12 (the line) |
| §11 `cast_move_speed`, `downed_for` in `rules.ron` | Task 1 (`combat:`) |
| §13 dodge key | Task 9, Task 13 (spec), rulings |
| §12 tests | sim unit tests in every task; client pure tests in 9 to 12; manual shots in 9 to 13 |

Out of this plan, by the spec's own split: loot, bags, food and water as items, the end screen (sub-project 5); Stitches, ravens and the night clock (6); rivals (7), whose kits will name these abilities by key.
