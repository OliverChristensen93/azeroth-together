//! The showcase stands on the real bake (spec §12): every spot `NIGHTFALL_SHOWCASE=1` puts an
//! enemy, the rival party and the ravens on is ground a unit can stand on, not water, outside the
//! entry's arrive-unseen ring, and reachable on foot from the entry; the host then plays a minute
//! of the night with all of it out. Run by hand with a WoW install (set `WOW_DATA`):
//! `cargo test --release -p nightfall-bake --test showcase -- --ignored --nocapture`.

use std::path::PathBuf;
use std::sync::Arc;

use nightfall_bake::{load_or_bake, BakeRequest};
use nightfall_sim::{
    showcase_plan, Content, ContentSources, HeightMap, Navigator, RaidRoll, World, ERRAND_BUDGET,
    TICK_HZ, V2, V3,
};

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

/// Where a unit placed at `p` stands, if on ground and not in water.
fn stand(map: &HeightMap, p: V2) -> Option<V3> {
    let z = map.ground_at(p)?;
    let at = map.stand_near(V3::at(p, z))?;
    (!map.is_water(at.ground())).then_some(at)
}

#[test]
#[ignore = "needs WOW_DATA (reads the 1.12 data and bakes Duskwood); run with --release"]
fn every_showcase_spot_stands_is_dry_and_is_reachable_from_the_entry() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let e = content.map.entries[0];
    let entry = V2::new(e[0], e[1]);
    let ring = content.rules.night.arrive_unseen;
    let mut nav = Navigator::new(map.clone());
    let plan = showcase_plan(&content, &mut nav, entry);
    let mut problems = Vec::new();
    problems.extend(plan.missing.iter().map(|k| format!("{k}: no spawn line")));
    problems.extend(plan.unplaced.iter().map(|k| format!("{k}: no spot")));
    assert_eq!(
        plan.enemies.len(),
        content.enemies.len(),
        "one of each enemy kind"
    );
    let from = stand(&map, entry).expect("the entry stands");
    let mut farthest = 0.0f32;
    for (spawn, key, at) in &plan.enemies {
        let far = at.ground().distance(entry);
        farthest = farthest.max(far);
        println!("{key:<26} ({:.1}, {:.1}) {far:.0} yd from the entry", at.x, at.y);
        if content.spawns[*spawn].kind != *key {
            problems.push(format!("{key}: takes its senses from another kind's line"));
        }
        if stand(&map, at.ground()).is_none() {
            problems.push(format!("{key} at {at:?}: no dry ground to stand on"));
        }
        if far <= ring {
            problems.push(format!("{key}: {far:.0} yd from the entry, inside the ring"));
        }
        // A fresh search, so the plan's own answer is not trusted.
        let mut fresh = Navigator::new(map.clone());
        if fresh.find_path(from, *at, false, ERRAND_BUDGET).is_none() {
            problems.push(format!("{key} at {at:?}: no path from the entry"));
        }
    }
    // A minute's walk at the hero's speed (7 yd a second) with room to spare for the turns.
    assert!(farthest < 7.0 * 40.0, "the farthest is {farthest:.0} yd out");
    let (poi, at) = plan.poi.expect("the rivals have a point of interest");
    println!(
        "rival party at {} ({:.1}, {:.1})",
        content.rivals.pois[poi].name, at.x, at.y
    );
    match stand(&map, at) {
        None => problems.push(format!("point {at:?}: no dry ground to stand on")),
        Some(p) => {
            let mut fresh = Navigator::new(map.clone());
            if fresh.find_path(from, p, false, ERRAND_BUDGET * 4).is_none() {
                problems.push(format!("point {at:?}: no path from the entry"));
            }
        }
    }
    for &k in &plan.flocks {
        let f = content.map.flocks[k];
        println!("ravens settle at ({:.1}, {:.1})", f.x, f.y);
        if stand(&map, f).is_none() {
            problems.push(format!("flock {k} at {f:?}: no dry ground"));
        }
    }
    assert!(!plan.flocks.is_empty(), "a flock of ravens");
    assert!(problems.is_empty(), "the showcase: {problems:#?}");
}

#[test]
#[ignore = "needs WOW_DATA (reads the 1.12 data and bakes Duskwood); run with --release"]
fn a_minute_of_night_with_the_showcase_out_runs_clean() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let content = Arc::new(content);
    let mut world = World::new(content.clone());
    world.begin_raid(RaidRoll::standard(&content, 1)).unwrap();
    world.attach_map(map);
    world.add_hero(0, &content.classes[0].key, "Us", 1, 0).unwrap();
    let before = world.enemies.len();
    let told = world.run_showcase();
    assert!(told.is_empty(), "{told:?}");
    assert_eq!(world.enemies.len(), before + content.enemies.len());
    let warned = world.warnings().len();
    for _ in 0..(60 * TICK_HZ as usize) {
        world.step();
    }
    assert_eq!(world.warnings().len(), warned, "{:?}", world.warnings());
    assert_eq!(world.rival_debug().len(), 1, "the party stands");
}
