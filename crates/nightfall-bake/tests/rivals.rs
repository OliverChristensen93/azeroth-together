//! The rivals' places and routes stand on the real bake (corrections, ruling 1): every entry, every
//! point of interest and every waypoint is ground a unit can stand on and not water, every leg of
//! every route is found by one search within the rivals' budget, and each shipped point of
//! interest holds a container. Skipped without a WoW install (set `WOW_DATA`).
//!
//! `cargo test --release -p nightfall-bake --test rivals measure_routes -- --ignored --nocapture`
//! prints every route as `rivals.ron` writes it, walked along the real ground: the scratch the
//! shipped waypoints were measured with. Where the shortest way wades a river the route is walked
//! through stops given in `NIGHTFALL_ROUTE_HINTS_<EXIT KEY>` (`x,y;x,y`): the bridge's were the
//! north road's points `-10236,-802;-10116,-682;-10017,-583;-9949,-503` and the Westfall road's the
//! west road's `-10720,330;-10715,462;-10715,582;-10715,660;-10670,740`.

use std::path::PathBuf;
use std::sync::Arc;

use nightfall_bake::{load_or_bake, BakeRequest};
use nightfall_sim::{
    Content, ContentSources, HeightMap, Navigator, PathPlan, RoutePlace, ERRAND_BUDGET,
    RIVAL_BUDGET, V2, V3,
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

/// Where a unit placed at `p` stands, as `spawn_all` finds it.
fn stand(map: &HeightMap, p: V2) -> Option<V3> {
    let z = map.ground_at(p)?;
    map.stand_near(V3::at(p, z))
}

/// Why `p` is no place for a waypoint, if it is not.
fn bad_spot(map: &HeightMap, p: V2) -> Option<String> {
    let Some(at) = stand(map, p) else {
        return Some("no ground to stand on".into());
    };
    if map.is_water(at.ground()) {
        return Some("in water".into());
    }
    None
}

/// The place a route end names, as a position, and what to call it.
fn place(content: &Content, p: RoutePlace) -> (V2, String) {
    match p {
        RoutePlace::Entry(i) => (content.rivals.entries[i], format!("Entry({i})")),
        RoutePlace::Poi(i) => {
            let poi = &content.rivals.pois[i];
            (V2::new(poi.at.0, poi.at.1), format!("Poi(\"{}\")", poi.key))
        }
        RoutePlace::Exit(i) => (
            content.map.exits[i].at,
            format!("Exit(\"{}\")", content.map.exits[i].key),
        ),
    }
}

/// Every pair of places the rivals travel between: each entry to each point of interest, each
/// point of interest to each exit.
fn pairs(content: &Content) -> Vec<(RoutePlace, RoutePlace)> {
    let r = &content.rivals;
    let mut out = Vec::new();
    for e in 0..r.entries.len() {
        for p in 0..r.pois.len() {
            out.push((RoutePlace::Entry(e), RoutePlace::Poi(p)));
        }
    }
    for p in 0..r.pois.len() {
        for x in 0..content.map.exits.len() {
            out.push((RoutePlace::Poi(p), RoutePlace::Exit(x)));
        }
    }
    out
}

#[test]
fn the_rivals_places_and_every_route_leg_stand_and_are_found_within_budget() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let leg_max = content.rules.rivals.route_leg;
    let mut problems = Vec::new();
    for (i, e) in content.rivals.entries.iter().enumerate() {
        if let Some(why) = bad_spot(&map, *e) {
            problems.push(format!("entry {i} {e:?}: {why}"));
        }
    }
    for p in &content.rivals.pois {
        let at = V2::new(p.at.0, p.at.1);
        if let Some(why) = bad_spot(&map, at) {
            problems.push(format!("point {} {at:?}: {why}", p.key));
        }
        // Each point holds a container a party can search, on ground a unit can reach it from.
        let held: Vec<V2> = content
            .loot
            .containers
            .iter()
            .map(|k| k.at)
            .filter(|c| c.distance(at) <= p.radius)
            .collect();
        if held.is_empty() {
            problems.push(format!(
                "point {}: no container within {} yd",
                p.key, p.radius
            ));
        }
        for c in held {
            if let Some(why) = bad_spot(&map, c) {
                problems.push(format!("point {}: a container at {c:?}: {why}", p.key));
            }
        }
    }
    let mut nav = Navigator::new(map.clone());
    let (mut legs, mut wading, mut dearest) = (0, 0, (0u64, String::new()));
    for (from, to) in pairs(&content) {
        let (a, an) = place(&content, from);
        let (b, bn) = place(&content, to);
        let Some(route) = content.rivals.route(from, to) else {
            problems.push(format!("no route {an} -> {bn}"));
            continue;
        };
        for (i, p) in route.iter().enumerate() {
            // An exit's own spot may be a ford or the bridge's foot; its ring is what counts.
            let exit_end = matches!(to, RoutePlace::Exit(_)) && i + 1 == route.len();
            let bad = if exit_end {
                stand(&map, *p)
                    .is_none()
                    .then(|| "no ground to stand on".to_string())
            } else {
                bad_spot(&map, *p)
            };
            if let Some(why) = bad {
                problems.push(format!("route {an} -> {bn} point {i} {p:?}: {why}"));
            }
        }
        assert_eq!(route.first(), Some(&a), "{an} -> {bn} starts at its place");
        assert_eq!(route.last(), Some(&b), "{an} -> {bn} ends at its place");
        for (i, w) in route.windows(2).enumerate() {
            legs += 1;
            let len = w[0].distance(w[1]);
            if len > leg_max {
                problems.push(format!("route {an} -> {bn} leg {i} is {len:.0} yd"));
            }
            let (Some(f), Some(t)) = (stand(&map, w[0]), stand(&map, w[1])) else {
                continue;
            };
            // Each leg as a party asks for it, cold, in both directions.
            for (f, t) in [(f, t), (t, f)] {
                nav.begin_step(RIVAL_BUDGET);
                let before = nav.floors_looked_at();
                match nav.plan_path(f, t, false, RIVAL_BUDGET) {
                    PathPlan::Found(path) => {
                        let spent = nav.floors_looked_at() - before;
                        if spent > dearest.0 {
                            dearest = (spent, format!("{an} -> {bn} leg {i}"));
                        }
                        // A leg may wade a stretch it cannot avoid (the Westfall road's ford, the
                        // bridge's foot); how many do is printed.
                        if path.iter().any(|q| map.is_water(q.ground())) {
                            wading += 1;
                        }
                    }
                    other => problems.push(format!(
                        "route {an} -> {bn} leg {i} ({len:.0} yd): {other:?} within {RIVAL_BUDGET}"
                    )),
                }
            }
        }
    }
    println!(
        "{legs} legs, both ways ({wading} of those searches wade); the dearest search took {} floors ({}); budget {RIVAL_BUDGET}, errand {ERRAND_BUDGET}",
        dearest.0, dearest.1
    );
    assert!(
        problems.is_empty(),
        "the rivals' places and routes: {problems:#?}"
    );
}

/// Places a route is walked through on its way to an exit, where the shortest way wades a river
/// (the bridge's): the stops are given as `x,y` pairs in `NIGHTFALL_ROUTE_HINTS_<exit key>`.
fn hints_for(key: &str) -> Vec<V2> {
    let var = format!("NIGHTFALL_ROUTE_HINTS_{}", key.to_uppercase());
    std::env::var(var)
        .ok()
        .map(|s| {
            s.split(';')
                .filter_map(|p| p.split_once(','))
                .map(|(x, y)| V2::new(x.trim().parse().unwrap(), y.trim().parse().unwrap()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
#[ignore = "prints every rival route walked along the real ground; run by hand with --release"]
fn measure_routes() {
    let data = benilla_formats::wow_data_or_skip!();
    let content = shipped();
    let map = Arc::new(baked(&content, &data));
    let mut nav = Navigator::new(map.clone());
    const BUDGET: usize = 50_000_000;
    // Yards between the waypoints printed, along the path.
    const SPACING: f32 = 30.0;
    for (from, to) in pairs(&content) {
        let (a, an) = place(&content, from);
        let (b, bn) = place(&content, to);
        let mut stops = vec![a];
        if let RoutePlace::Exit(x) = to {
            stops.extend(hints_for(&content.map.exits[x].key));
        }
        stops.push(b);
        // The whole walk: every point of the paths between the stops.
        let mut walk: Vec<V2> = vec![a];
        let mut spent = 0;
        let mut found = true;
        for w in stops.windows(2) {
            let (Some(f), Some(t)) = (stand(&map, w[0]), stand(&map, w[1])) else {
                println!("// {an} -> {bn}: a stop {:?} has no ground", w[0]);
                found = false;
                break;
            };
            nav.begin_step(BUDGET);
            let before = nav.floors_looked_at();
            let Some(path) = nav.find_path(f, t, false, BUDGET) else {
                println!(
                    "// {an} -> {bn}: no path at all ({} yd)",
                    w[0].distance(w[1])
                );
                found = false;
                break;
            };
            spent += nav.floors_looked_at() - before;
            walk.extend(path.iter().map(|q| q.ground()));
        }
        if !found {
            continue;
        }
        // A waypoint each SPACING yards along the walk, the first dry point from there on;
        // none within 8 yards of the end (the end is the place itself).
        let (mut via, mut wet): (Vec<V2>, usize) = (Vec::new(), 0);
        let (mut last, mut owed, mut total) = (a, 0.0f32, 0.0f32);
        let mut pending = false;
        for &q in &walk[1..] {
            let d = last.distance(q);
            let mut at = 0.0;
            while d > 0.0 && owed + (d - at) >= SPACING {
                at += SPACING - owed;
                owed = 0.0;
                pending = true;
                let p = last + (q - last) * (at / d);
                if pending && !map.is_water(p) && p.distance(b) > 8.0 {
                    via.push(p);
                    pending = false;
                } else if map.is_water(p) {
                    wet += 1;
                    println!("//   wet sample at {p:?} on {an} -> {bn}");
                }
            }
            // A sample that fell in water is moved on to the next dry point of the walk.
            if pending && !map.is_water(q) && q.distance(b) > 8.0 {
                via.push(q);
                pending = false;
                owed = 0.0;
            } else {
                owed += d - at;
            }
            total += d;
            last = q;
        }
        let mut gap = 0.0f32;
        let mut prev = a;
        for p in via.iter().copied().chain([b]) {
            gap = gap.max(prev.distance(p));
            prev = p;
        }
        let pts: Vec<String> = via
            .iter()
            .map(|p| format!("({:.0}.0, {:.0}.0)", p.x, p.y))
            .collect();
        println!(
            "// {an} -> {bn}: {:.0} yd as the crow flies, {total:.0} walked, {spent} floors, {wet} wet samples, longest leg {gap:.0}\n(from: {an}, to: {bn}, via: [{}]),",
            a.distance(b),
            pts.join(", ")
        );
    }
}
