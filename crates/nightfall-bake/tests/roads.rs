//! The night's places stand on the real bake (spec §7.3): every road point and every signature
//! monster's post is ground a unit can stand on and not water, each leg of a patrol finds a path
//! within the patrol's budget, and no leg is longer than the budget can search. Skipped without a
//! WoW install (set `WOW_DATA`).
//!
//! `NIGHTFALL_ROADS_RESAMPLE=1 cargo test -p nightfall-bake --test roads resample -- --ignored
//! --nocapture` prints each road as points walked along the real ground, 40 yards apart: the
//! scratch the content's roads were measured with.

use std::path::PathBuf;
use std::sync::Arc;

use nightfall_bake::{load_or_bake, BakeRequest};
use nightfall_sim::{
    Behaviour, Content, ContentSources, HeightMap, Navigator, World, PATROL_BUDGET, V2, V3,
};

/// The longest leg of a road the content may ask a patrol for, yards: the map's comment says
/// about 60, and a leg a little over (a path corner, a stretch of water the points skip) still
/// finds its path inside the patrol's budget, which the test below also checks.
const LEG: f32 = 80.0;

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

/// Where a unit placed at `p` stands, as `spawn_all` finds it: on a floor near the terrain.
fn stand(map: &HeightMap, p: V2) -> Option<V3> {
    let z = map.ground_at(p)?;
    map.stand_near(V3::at(p, z))
}

/// Why `p` is no place for a road point or a post, if it is not.
fn bad_spot(map: &HeightMap, p: V2) -> Option<String> {
    let Some(at) = stand(map, p) else {
        return Some("no ground to stand on".into());
    };
    if map.is_water(at.ground()) {
        return Some("in water".into());
    }
    None
}

#[test]
fn the_nights_roads_and_posts_stand_and_every_leg_has_a_path() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let mut nav = Navigator::new(map.clone());
    let mut problems = Vec::new();
    for road in &content.map.roads {
        for (i, p) in road.points.iter().enumerate() {
            if let Some(why) = bad_spot(&map, *p) {
                problems.push(format!("road {} point {i} {p:?}: {why}", road.key));
            }
        }
        for (i, w) in road.points.windows(2).enumerate() {
            let len = w[0].distance(w[1]);
            if len > LEG {
                problems.push(format!("road {} leg {i} is {len:.0} yd", road.key));
            }
        }
    }
    // Every spawn the night adds: its posts and every leg of its route.
    for s in content
        .spawns
        .iter()
        .filter(|s| s.monster.is_some() || s.stage_at.is_some())
    {
        let points: Vec<V2> = match &s.behaviour {
            Behaviour::Sentry { at, .. } | Behaviour::Sleeper { at, .. } => {
                vec![V2::new(at.0, at.1)]
            }
            Behaviour::Wander { center, .. } => vec![V2::new(center.0, center.1)],
            Behaviour::Patrol { route, .. } => route.iter().map(|&(x, y)| V2::new(x, y)).collect(),
            Behaviour::Road { .. } => unreachable!("the content made it a patrol"),
        };
        for (i, p) in points.iter().enumerate() {
            if let Some(why) = bad_spot(&map, *p) {
                problems.push(format!("{} point {i} {p:?}: {why}", s.place));
            }
        }
        if let Behaviour::Patrol { .. } = s.behaviour {
            // The patrol walks its last point back to its first.
            let n = points.len();
            for i in 0..n {
                let (a, b) = (points[i], points[(i + 1) % n]);
                let (Some(from), Some(to)) = (stand(&map, a), stand(&map, b)) else {
                    continue;
                };
                nav.begin_step(PATROL_BUDGET);
                if nav.find_path(from, to, false, PATROL_BUDGET).is_none() {
                    problems.push(format!(
                        "{}: no path from point {i} to the next ({:.0} yd)",
                        s.place,
                        a.distance(b)
                    ));
                }
            }
        }
    }
    for f in &content.map.flocks {
        if let Some(why) = bad_spot(&map, *f) {
            problems.push(format!("a flock at {f:?}: {why}"));
        }
    }
    for z in &content.map.zones {
        let pts = z.area.points();
        let middle = pts.iter().fold(V2::ZERO, |a, p| a + *p) * (1.0 / pts.len() as f32);
        if let Some(why) = bad_spot(&map, middle) {
            problems.push(format!("zone {} centre {middle:?}: {why}", z.key));
        }
    }
    // And the host places every enemy of the content on this map without a word of complaint.
    let mut world = World::new(Arc::new(content));
    world.attach_map(map);
    problems.extend(world.warnings().iter().cloned());
    // And two minutes of night on their own: wanderers find ground in their areas, patrols find
    // their points (a content mistake is a warning the host logs once).
    for _ in 0..(120 * nightfall_sim::TICK_HZ as usize) {
        world.step();
    }
    problems.extend(
        world
            .warnings()
            .iter()
            .skip(problems.len().min(world.warnings().len()))
            .cloned(),
    );
    assert!(problems.is_empty(), "the night's places: {problems:#?}");
}

#[test]
#[ignore = "prints the roads as points walked along the real ground; run by hand"]
fn resample() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let mut nav = Navigator::new(map.clone());
    const BUDGET: usize = 50_000_000;
    // The waypoints each road is walked through: where it begins and ends on ground a unit can
    // reach, with the shipped points between them that do.
    let plans: [(&str, &[(f32, f32)]); 4] = [
        ("north_road", &[(-9930.0, -480.0), (-10540.0, -1130.0)]),
        (
            "raven_hill_road",
            &[(-10720.0, 330.0), (-10570.0, -979.0), (-10540.0, -1130.0)],
        ),
        ("west_road", &[(-10650.0, 760.0), (-10720.0, 330.0)]),
        ("east_road", &[(-10540.0, -1130.0), (-10570.0, -1830.0)]),
    ];
    for (key, waypoints) in plans {
        let keep: Vec<V3> = waypoints
            .iter()
            .filter_map(|&(x, y)| stand(&map, V2::new(x, y)))
            .collect();
        let mut out: Vec<V2> = Vec::new();
        for w in keep.windows(2) {
            nav.begin_step(BUDGET);
            let Some(path) = nav.find_path(w[0], w[1], false, BUDGET) else {
                println!("// {key}: no path between {:?} and {:?}", w[0], w[1]);
                continue;
            };
            let mut last: Option<V2> = out.last().copied();
            for p in path.iter() {
                let q = p.ground();
                if map.is_water(q) {
                    continue;
                }
                if last.is_none_or(|l| l.distance(q) >= 40.0) {
                    out.push(q);
                    last = Some(q);
                }
            }
            if let Some(e) = path.last() {
                let q = e.ground();
                if out.last().is_none_or(|l| l.distance(q) > 5.0) {
                    out.push(q);
                }
            }
        }
        let pts: Vec<String> = out
            .iter()
            .map(|p| format!("({:.0}.0, {:.0}.0)", p.x, p.y))
            .collect();
        println!("{key}: {} points\n[{}]", pts.len(), pts.join(", "));
    }
}

#[test]
#[ignore = "prints the best places for a wandering group near NIGHTFALL_SCAN=x,y; run by hand"]
fn scan_for_a_wander_centre() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let mut nav = Navigator::new(map.clone());
    let scan = std::env::var("NIGHTFALL_SCAN").expect("NIGHTFALL_SCAN=x,y");
    let (x, y) = scan.split_once(',').expect("x,y");
    let (x, y): (f32, f32) = (x.trim().parse().unwrap(), y.trim().parse().unwrap());
    let radius: f32 = std::env::var("NIGHTFALL_SCAN_RADIUS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(25.0);
    let mut best: Vec<(f32, V2)> = Vec::new();
    for dx in (-150..=150).step_by(10) {
        for dy in (-150..=150).step_by(10) {
            let c = V2::new(x + dx as f32, y + dy as f32);
            let Some(from) = stand(&map, c) else { continue };
            if bad_spot(&map, c).is_some() {
                continue;
            }
            // The share of eight spots round the ring a wanderer there could walk to.
            let mut ok = 0;
            for k in 0..8 {
                let spot = c + V2::from_angle(k as f32 * std::f32::consts::TAU / 8.0, radius);
                let Some(to) = stand(&map, spot) else {
                    continue;
                };
                nav.begin_step(4_000);
                if nav.find_path(from, to, false, 4_000).is_some() {
                    ok += 1;
                }
            }
            best.push((ok as f32 / 8.0, c));
        }
    }
    best.sort_by(|a, b| {
        b.0.total_cmp(&a.0).then(
            a.1.distance(V2::new(x, y))
                .total_cmp(&b.1.distance(V2::new(x, y))),
        )
    });
    for (share, c) in best.iter().take(12) {
        println!(
            "{share:.2} at ({:.0}, {:.0}), {:.0} yd from the old centre",
            c.x,
            c.y,
            c.distance(V2::new(x, y))
        );
    }
    let here = best.iter().find(|(_, c)| *c == V2::new(x, y));
    println!("the old centre: {here:?}");
}
