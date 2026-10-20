//! The rivals' bots on the real Duskwood bake, a whole night (the corrections' rulings 1 and 2,
//! spec §7.4): every party leaves by an exit, leaves unseen, or dies within the night, with the
//! bridge open and with it shut, and no step spends more path floors than the rivals' pool allows
//! on top of what the enemies' share is. The worst step of a whole world is timed too. Both need
//! a WoW install (set `WOW_DATA`) and are slow; run them in release:
//!
//! `WOW_DATA=... cargo test --release -p nightfall-bake --test rival_night -- --ignored --nocapture`
//!
//! `NIGHTFALL_NIGHT_SEEDS=1,2,3` picks the raids; `NIGHTFALL_TRACE_RIVALS=1` prints each party at the
//! end, `NIGHTFALL_TRACE_EVERY=1` every minute, `NIGHTFALL_TRACE_GONE=1` where and when each rival
//! left the world.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nightfall_bake::{load_or_bake, BakeRequest};
use nightfall_sim::{
    Content, ContentSources, Event, HeightMap, PlayerId, PlayerInput, RaidRoll, World, RIVAL_POOL,
    STEP_FLOORS, TICK_HZ,
};

/// Seconds of night simulated: past the hunt (1200) and Stitches (1560) of the shipped stages.
const NIGHT: f32 = 1_800.0;
/// Milliseconds the worst step of a whole world may take in release (the 30 Hz tick is 33.3).
const TARGET_MS: f64 = 20.0;

fn shipped() -> Content {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/nightfall");
    let read = |name: &str| {
        std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let (classes, map, rules) = (read("classes.ron"), read("map.ron"), read("rules.ron"));
    let (enemies, spawns, abilities) = (
        read("enemies.ron"),
        read("spawns.ron"),
        read("abilities.ron"),
    );
    let (items, loot, monsters) = (read("items.ron"), read("loot.ron"), read("monsters.ron"));
    let rivals = read("rivals.ron");
    Content::from_ron(ContentSources {
        classes: &classes,
        map: &map,
        rules: &rules,
        enemies: &enemies,
        spawns: &spawns,
        abilities: &abilities,
        items: &items,
        loot: &loot,
        monsters: &monsters,
        rivals: &rivals,
    })
    .expect("the shipped content parses")
}

fn baked(content: &Content, data: &std::path::Path) -> HeightMap {
    let cache = std::env::var_os("NIGHTFALL_BAKE_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/nf-home/nightfall")
        });
    let map = &content.map;
    let req = BakeRequest {
        data_dir: data,
        map_id: map.map,
        key: &map.key,
        bounds: &map.bounds,
        props: &[],
    };
    load_or_bake(&req, Some(&cache), &|_| {})
        .expect("the bake")
        .map
}

/// What a night of the rivals came to.
struct Night {
    members: Vec<PlayerId>,
    /// Members still up in the world at the end.
    left: Vec<String>,
    /// Members that died.
    dead: usize,
    /// Members seen out by an exit (a `RivalOut`); the rest of those gone slipped out unseen.
    out_by_exit: usize,
    /// Seconds in at which the raid was over (the parked hero died, and nobody extracts after).
    over_at: Option<usize>,
    worst: Duration,
    worst_floors: u64,
    /// Steps run.
    steps: usize,
}

/// A night on the real map: the shipped parties come in at the rivals' entries, the exits named in
/// `shut` closed from the start, and the hero stands at the player's entry.
fn night(
    content: &Arc<Content>,
    map: &Arc<HeightMap>,
    seed: u64,
    shut: &[&str],
    secs: f32,
) -> Night {
    let mut roll = RaidRoll::standard(content, seed);
    for key in shut {
        let i = content
            .map
            .exits
            .iter()
            .position(|e| e.key == *key)
            .unwrap_or_else(|| panic!("no exit {key}"));
        roll.exits[i].open = false;
    }
    let mut w = World::new(content.clone());
    w.begin_raid(roll).expect("the roll is accepted");
    w.attach_map(map.clone());
    w.add_hero(0, "rogue", "Hero", 1, 0).expect("a hero joins");
    w.set_input(0, PlayerInput::default());
    w.step();
    let mut members = Vec::new();
    for (number, def) in (0..content.rivals.parties.len()).enumerate() {
        let entry = content.rivals.entries[number % content.rivals.entries.len()];
        let (lo, hi) = content.rivals.parties[def].aggression;
        let aggression = lo + (hi - lo) * 0.5;
        members.extend(w.add_party(def, number as u8, aggression, entry, seed + number as u64));
    }
    w.step();
    let (mut worst, mut worst_floors) = (Duration::ZERO, 0u64);
    let steps = (secs * TICK_HZ as f32) as usize;
    let mut out_by_exit = 0;
    let mut over_at = None;
    let mut last: Vec<Option<(f32, f32)>> = vec![None; members.len()];
    for i in 0..steps {
        let before = w.floors_looked_at();
        let t = Instant::now();
        w.step();
        worst = worst.max(t.elapsed());
        worst_floors = worst_floors.max(w.floors_looked_at() - before);
        if std::env::var_os("NIGHTFALL_TRACE_EVERY").is_some() && i % (60 * TICK_HZ as usize) == 0 {
            for line in w.rival_debug() {
                println!("{:>5}s {line}", i / TICK_HZ as usize);
            }
        }
        if std::env::var_os("NIGHTFALL_TRACE_GONE").is_some() {
            for (k, &m) in members.iter().enumerate() {
                match w.hero(m) {
                    Some(h) => last[k] = Some((h.pos.x, h.pos.y)),
                    None => {
                        if let Some((x, y)) = last[k].take() {
                            println!(
                                "{:>5}s rival {m} gone at ({x:.0}, {y:.0})",
                                i / TICK_HZ as usize
                            );
                        }
                    }
                }
            }
        }
        if over_at.is_none() && w.is_over() {
            over_at = Some(i / TICK_HZ as usize);
        }
        out_by_exit += w
            .drain_events()
            .iter()
            .filter(|e| matches!(e, Event::RivalOut { .. }))
            .count();
    }
    let left = members
        .iter()
        .filter_map(|&m| w.hero(m))
        .filter(|h| h.fight.up())
        .map(|h| format!("{} at {:?}", h.name, h.pos))
        .collect();
    let dead = members
        .iter()
        .filter(|&&m| w.hero(m).is_some_and(|h| !h.fight.up()))
        .count();
    if std::env::var_os("NIGHTFALL_TRACE_RIVALS").is_some() {
        for line in w.rival_debug() {
            println!("{line}");
        }
    }
    if let Some(at) = over_at {
        println!("  (the raid was over at {at} s: the hero died)");
    }
    Night {
        members,
        left,
        dead,
        out_by_exit,
        over_at,
        worst,
        worst_floors,
        steps,
    }
}

fn run(name: &str, shut: &[&str], secs: f32, strict: bool) {
    let data = benilla_formats::wow_data_or_skip!();
    let content = Arc::new(shipped());
    let map = Arc::new(baked(&content, &data));
    let seeds: Vec<u64> = std::env::var("NIGHTFALL_NIGHT_SEEDS")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![1, 2]);
    let mut problems = Vec::new();
    let (mut by_exit, mut unseen) = (0, 0);
    for seed in seeds {
        let n = night(&content, &map, seed, shut, secs);
        let ms = n.worst.as_secs_f64() * 1000.0;
        println!(
            "{name}, seed {seed}: {} rivals, {} died, {} still about after {} steps; worst step {ms:.1} ms, \
             at most {} floors in a step (step budget {STEP_FLOORS}, rivals' pool {RIVAL_POOL})",
            n.members.len(),
            n.dead,
            n.left.len(),
            n.steps,
            n.worst_floors
        );
        let gone = n.members.len() - n.left.len() - n.dead;
        println!(
            "  of {} rivals: {} out by an exit, {} slipped out unseen, {} died",
            n.members.len(),
            n.out_by_exit,
            gone.saturating_sub(n.out_by_exit),
            n.dead
        );
        by_exit += n.out_by_exit;
        unseen += gone.saturating_sub(n.out_by_exit);
        // Once the raid is over nobody extracts: a party still on its way then is no fault, unless the
        // exits are near enough that every party should be out first (`strict`).
        if !n.left.is_empty() && (strict || n.over_at.is_none()) {
            problems.push(format!("seed {seed}: still about: {:?}", n.left));
        }
        assert!(
            n.worst_floors as usize <= STEP_FLOORS,
            "seed {seed}: a step looked at {} floors, over the step's {STEP_FLOORS}",
            n.worst_floors
        );
        if !cfg!(debug_assertions) && ms > TARGET_MS {
            problems.push(format!(
                "seed {seed}: the worst step took {ms:.1} ms, over {TARGET_MS} ms"
            ));
        }
    }
    // The bots work: most of those who live walk out by an exit, the backstop is for the few that
    // cannot.
    assert!(
        by_exit > 0 && unseen <= by_exit,
        "{name}: {by_exit} out by an exit, {unseen} slipped out unseen"
    );
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
#[ignore = "a whole night on the real bake; needs WOW_DATA; run with --release --nocapture"]
fn every_party_leaves_or_dies_within_the_night_with_every_exit_open() {
    run("every exit open", &[], NIGHT, true);
}

#[test]
#[ignore = "a whole night on the real bake; needs WOW_DATA; run with --release --nocapture"]
fn every_party_leaves_or_dies_within_the_night_with_the_bridge_shut() {
    run("bridge shut", &["bridge"], NIGHT, true);
}

#[test]
#[ignore = "a whole night on the real bake; needs WOW_DATA; run with --release --nocapture"]
fn every_party_leaves_or_dies_within_the_night_with_only_the_far_exit_open() {
    // The Westfall road, 2 400 yards off: the bridge, the Deadwind road and the cart are shut. The
    // parked hero dies about 950 s in and the raid ends with them, so the walkers among the parties
    // are still on the road by then: only that none is stuck (they slip out unseen) is asked.
    run(
        "only the Westfall road open",
        &["bridge", "deadwind_road", "darkshire_cart"],
        NIGHT,
        false,
    );
}
