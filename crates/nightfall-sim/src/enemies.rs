//! Nightfall's enemies as the host runs them (spec §7.1): placed from the content's spawns on the
//! height map's floors, then each about its business. A sentry holds its post, a patrol walks its
//! route round and round, a wanderer roams its area and rests between walks. They go where the
//! map's paths go, so around houses rather than through them. What they notice is the senses'
//! ([`crate::senses`]): while an enemy is not calm its [`Brain`] is held, and on release it finds
//! its way back to its order. How they fight comes with sub-project 4.

use std::collections::HashSet;
use std::f32::consts::TAU;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::combat::EnemyFight;
use crate::content::Behaviour;
use crate::heightmap::{HeightMap, Navigator, PathPlan, MAX_STEP};
use crate::{Content, DT, V2, V3};

/// Floors a patrol's or sentry's path search may look at: its legs can be long, and each is
/// searched once (the [`Navigator`] keeps them). A 400 yd leg through Duskwood's woods takes up
/// to about 26 000; a search that finds nothing costs about 8 ms on the real map.
pub const PATROL_BUDGET: usize = 30_000;
/// Floors a wanderer's search may look at: its walks are short, and a spot it cannot reach (in a
/// closed house) must be given up on cheaply.
pub const WANDER_BUDGET: usize = 4_000;
/// Floors all of a step's searches may look at together: one patrol's search and two
/// wanderers'. A search starts only while what is left covers its whole budget, so no step
/// spends more on paths than this (about 10 ms on the real map) however many enemies are lost.
pub const STEP_FLOORS: usize = PATROL_BUDGET + 2 * WANDER_BUDGET;
/// Within this many yards of a point, an enemy is there.
pub(crate) const ARRIVE: f32 = 0.6;
/// How far apart a group of sentries stands around its post.
const GROUP_SPACING: f32 = 1.5;
/// Seconds before a sentry tries again to reach a post it found no path to.
const RETRY: f32 = 5.0;
/// Failed walks in a row before a wanderer's area is reported.
const WANDER_GIVE_UP: u32 = 10;

/// An enemy as every client sees it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enemy {
    pub id: u32,
    /// Index into [`Content::enemies`].
    pub kind: usize,
    pub pos: V2,
    /// The floor it stands on.
    pub z: f32,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// How fast it moved this step, yards a second (0 standing).
    pub speed: f32,
    /// A sleeper lying at its post (spec §7.1), which the client draws lying down.
    pub asleep: bool,
    /// Its side of the fight (spec §6).
    pub fight: EnemyFight,
}

/// A small random number generator (xorshift64*), seeded per enemy, so a run plays the same every
/// time with the same inputs.
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..1`.
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// What an enemy is set to do.
pub(crate) enum Order {
    Sentry {
        post: V3,
        facing: V2,
    },
    /// `None` marks a point with no floor, or one no path reaches: it is skipped.
    Patrol {
        route: Vec<Option<V3>>,
        target: usize,
        pause: f32,
    },
    /// `height` is the content's: with one, its walks stay near that height (an upper storey);
    /// without, on the ground under each spot.
    Wander {
        center: V3,
        radius: f32,
        height: Option<f32>,
    },
}

/// An enemy's mind, the host's alone: never sent.
pub(crate) struct Brain {
    /// Index into [`Content::spawns`], for warnings.
    spawn: usize,
    order: Order,
    /// The path being walked, and the next point of it.
    path: Option<Arc<[V3]>>,
    next: usize,
    /// Seconds to stand before moving on.
    wait: f32,
    /// Failed searches in a row.
    failures: u32,
    /// A warning about it has been given.
    warned: bool,
    /// Held by its senses: its order waits while it turns, checks, chases or searches.
    held: bool,
    rng: Rng,
}

impl Brain {
    /// Its spawn line in the content.
    pub(crate) fn spawn(&self) -> usize {
        self.spawn
    }

    /// Held by its senses, or released. Taking hold drops the path it was on, so on release it
    /// searches its way back: to its post, or to the patrol point it was going to.
    pub(crate) fn hold(&mut self, on: bool) {
        if on && !self.held {
            self.path = None;
            self.next = 0;
            self.wait = 0.0;
        }
        self.held = on;
    }

    /// Whether `pos` is at its post (a sentry's or a sleeper's).
    pub(crate) fn at_post(&self, pos: V2) -> bool {
        matches!(&self.order, Order::Sentry { post, .. } if post.ground().distance(pos) <= ARRIVE)
    }
}

/// How a warning names a spawn.
fn who(content: &Content, spawn: usize) -> String {
    let s = &content.spawns[spawn];
    format!("spawns.ron: \"{}\" ({})", s.place, s.kind)
}

/// `d` as a facing; north when it is (nearly) nothing.
pub(crate) fn heading(d: V2) -> V2 {
    let n = d.normalize_or_zero();
    if n == V2::ZERO {
        V2::new(1.0, 0.0)
    } else {
        n
    }
}

/// The next point of the route after `from` that is not skipped; `from` itself when none is.
fn next_target(route: &[Option<V3>], from: usize) -> usize {
    (1..=route.len())
        .map(|d| (from + d) % route.len())
        .find(|&i| route[i].is_some())
        .unwrap_or(from)
}

/// Where member `k` of spawn `si` stands at `spot`: `stand`, a floor a unit can stand on; else
/// `floor`, with a warning, since an enemy there cannot walk away (on a ledge, a tree trunk's
/// top or a narrow wall).
fn stand_or_warn(
    content: &Content,
    si: usize,
    k: usize,
    spot: V2,
    stand: Option<V3>,
    floor: Option<V3>,
    warnings: &mut Vec<String>,
) -> Option<V3> {
    if stand.is_some() {
        return stand;
    }
    let f = floor?;
    warnings.push(format!(
        "{}: {} ({:.1}, {:.1}) has no floor to walk on within 2 yd; it stands at height {:.1} \
         and cannot walk away",
        who(content, si),
        if k == 0 {
            "its spot".to_owned()
        } else {
            format!("member {}'s spot", k + 1)
        },
        spot.x,
        spot.y,
        f.z
    ));
    Some(f)
}

/// Every enemy the content's spawns place, on the map's floors, with a warning for each that
/// could not be placed. Ids count from 1 in the content's order.
pub(crate) fn spawn_all(
    content: &Content,
    map: &HeightMap,
) -> (Vec<Enemy>, Vec<Brain>, Vec<String>) {
    let mut enemies = Vec::new();
    let mut brains = Vec::new();
    let mut warnings = Vec::new();
    let mut id = 1u32;
    for (si, s) in content.spawns.iter().enumerate() {
        let Some(kind) = content.enemy(&s.kind) else {
            continue;
        };
        let def = &content.enemies[kind];
        // Where a point stands: on a floor a unit can stand on near the height the content gives,
        // else near the terrain, a yard or two aside if need be (`stand`: never a tree trunk's or
        // a rock's top over the ground it was meant for); `floor` is the nearest floor at all.
        let hint = |p: V2| s.z.or_else(|| map.ground_at(p)).unwrap_or(0.0);
        let stand = |p: V2| map.stand_near(V3::at(p, hint(p)));
        let floor = |p: V2| map.floor_near(p, hint(p)).map(|z| V3::at(p, z));
        for k in 0..s.count as usize {
            let mut rng =
                Rng::new(0x9E37_79B9_7F4A_7C15 ^ u64::from(id).wrapping_mul(0xBF58_476D_1CE4_E5B9));
            let placed = match &s.behaviour {
                Behaviour::Sentry { at, facing } | Behaviour::Sleeper { at, facing } => {
                    let post = V2::new(at.0, at.1);
                    let spot = if k == 0 {
                        post
                    } else {
                        post + V2::from_angle(k as f32 * TAU / s.count as f32, GROUP_SPACING)
                    };
                    let face = V2::from_angle(facing.to_radians(), 1.0);
                    let at = stand_or_warn(
                        content,
                        si,
                        k,
                        spot,
                        stand(spot),
                        floor(spot),
                        &mut warnings,
                    );
                    at.map(|p| {
                        (
                            p,
                            Order::Sentry {
                                post: p,
                                facing: face,
                            },
                            face,
                        )
                    })
                }
                Behaviour::Patrol { route, pause } => {
                    let points: Vec<Option<V3>> =
                        route.iter().map(|&(x, y)| stand(V2::new(x, y))).collect();
                    if k == 0 {
                        for (i, p) in points.iter().enumerate() {
                            if p.is_none() {
                                warnings.push(format!(
                                    "{}: patrol point {} ({:.1}, {:.1}) has no floor to walk on \
                                     within 2 yd; it is skipped",
                                    who(content, si),
                                    i + 1,
                                    route[i].0,
                                    route[i].1
                                ));
                            }
                        }
                    }
                    let first = k % points.len();
                    points[first].map(|p| {
                        let target = next_target(&points, first);
                        let facing = points[target]
                            .map_or(V2::new(1.0, 0.0), |t| heading(t.ground() - p.ground()));
                        let order = Order::Patrol {
                            route: points.clone(),
                            target,
                            pause: *pause,
                        };
                        (p, order, facing)
                    })
                }
                Behaviour::Wander { center, radius } => {
                    let middle = V2::new(center.0, center.1);
                    let c = stand(middle).or_else(|| floor(middle));
                    c.and_then(|c| {
                        let spot = c.ground()
                            + V2::from_angle(rng.next_f32() * TAU, radius * rng.next_f32().sqrt());
                        let facing = V2::from_angle(rng.next_f32() * TAU, 1.0);
                        let at = stand(spot).or_else(|| stand(middle));
                        let at = stand_or_warn(content, si, k, middle, at, Some(c), &mut warnings)?;
                        let order = Order::Wander {
                            center: c,
                            radius: *radius,
                            height: s.z,
                        };
                        Some((at, order, facing))
                    })
                }
            };
            let Some((at, order, facing)) = placed else {
                // Named as the one member, not the group: the rest may stand.
                let member = format!("member {} of {}", k + 1, s.count);
                warnings.push(match &s.behaviour {
                    Behaviour::Patrol { route, .. } => format!(
                        "{}: {member} would start on patrol point {}, which has no floor; that \
                         member is left out",
                        who(content, si),
                        k % route.len() + 1
                    ),
                    _ => format!(
                        "{}: {member} has no floor to stand on; that member is left out",
                        who(content, si)
                    ),
                });
                continue;
            };
            enemies.push(Enemy {
                id,
                kind,
                pos: at.ground(),
                z: at.z,
                facing,
                hp: def.hp,
                max_hp: def.hp,
                speed: 0.0,
                asleep: matches!(s.behaviour, Behaviour::Sleeper { .. }),
                fight: EnemyFight::default(),
            });
            brains.push(Brain {
                spawn: si,
                order,
                path: None,
                next: 0,
                wait: 0.0,
                failures: 0,
                warned: false,
                held: false,
                rng,
            });
            id += 1;
        }
    }
    (enemies, brains, warnings)
}

/// What the enemies share in choosing their ways: the host's alone, never sent.
#[derive(Default)]
pub(crate) struct Planner {
    /// Which enemy chooses its way first next step, so the path budget goes round.
    cursor: usize,
    /// The (spawn, patrol point) pairs a search found no path to. The [`Navigator`] remembers a
    /// failure only for a moment and only from near where it was searched, so without this every
    /// member of a patrol group (each walking from elsewhere on the route) would pay the full
    /// search for a point the first already found sealed, and warn about it again.
    failed_points: HashSet<(usize, usize)>,
    /// Searches that found nothing, so far.
    failed_searches: u64,
    /// Paths asked of the [`Navigator`] so far, kept ones included.
    asked: u64,
}

impl Planner {
    /// How many path searches have found nothing so far.
    pub(crate) fn failed_searches(&self) -> u64 {
        self.failed_searches
    }
}

/// One step for every enemy: those with nowhere to go choose a way (at most `paths_per_step`
/// searches looking at no more than [`STEP_FLOORS`] floors in all, starting one enemy further
/// along each step so every enemy gets its turn), then all walk.
#[cfg(test)]
pub(crate) fn step(
    enemies: &mut [Enemy],
    brains: &mut [Brain],
    nav: &mut Navigator,
    content: &Content,
    warnings: &mut Vec<String>,
    planner: &mut Planner,
) {
    nav.begin_step(STEP_FLOORS);
    step_planned(enemies, brains, nav, content, warnings, planner);
}

/// One step for every enemy, as the world runs it: the floors it may search are what the senses
/// have left of the step's [`STEP_FLOORS`], since they search before the orders do (a chase before
/// a patrol's leg), so a crowd on the heels of a hero cannot make the step dearer, only the
/// orders later.
pub(crate) fn step_planned(
    enemies: &mut [Enemy],
    brains: &mut [Brain],
    nav: &mut Navigator,
    content: &Content,
    warnings: &mut Vec<String>,
    planner: &mut Planner,
) {
    let n = enemies.len();
    if n == 0 {
        return;
    }
    let mut budget = content.rules.world.paths_per_step;
    let first = planner.cursor % n;
    planner.cursor = planner.cursor.wrapping_add(1);
    for i in 0..n {
        let k = (first + i) % n;
        plan(
            &enemies[k],
            &mut brains[k],
            nav,
            content,
            warnings,
            planner,
            &mut budget,
        );
    }
    let map = nav.map();
    for (e, b) in enemies.iter_mut().zip(brains.iter_mut()) {
        walk(e, b, map, content);
    }
}

/// An idle enemy chooses where to go next and searches its path, if the step's budget allows.
fn plan(
    e: &Enemy,
    b: &mut Brain,
    nav: &mut Navigator,
    content: &Content,
    warnings: &mut Vec<String>,
    planner: &mut Planner,
    budget: &mut u32,
) {
    if b.held || b.wait > 0.0 || b.path.is_some() || *budget == 0 {
        return;
    }
    let here = V3::at(e.pos, e.z);
    let (goal, cache, limit) = match &mut b.order {
        Order::Sentry { post, .. } => {
            if here.ground().distance(post.ground()) <= ARRIVE {
                return;
            }
            (*post, true, PATROL_BUDGET)
        }
        Order::Patrol {
            route,
            target,
            pause,
        } => {
            let Some(goal) = route[*target] else {
                *target = next_target(route, *target);
                return;
            };
            if planner.failed_points.contains(&(b.spawn, *target)) {
                // A groupmate found no path to it: skip it without a search or a warning.
                route[*target] = None;
                *target = next_target(route, *target);
                return;
            }
            if here.ground().distance(goal.ground()) <= ARRIVE {
                *target = next_target(route, *target);
                b.wait = *pause;
                return;
            }
            (goal, true, PATROL_BUDGET)
        }
        Order::Wander {
            center,
            radius,
            height,
        } => {
            let spot = center.ground()
                + V2::from_angle(b.rng.next_f32() * TAU, *radius * b.rng.next_f32().sqrt());
            let hint = height
                .or_else(|| nav.map().ground_at(spot))
                .unwrap_or(center.z);
            // A spot with no floor to walk on (a trunk's top, a hole) is no walk: rest and pick
            // another.
            let Some(goal) = nav.stand_near(V3::at(spot, hint)) else {
                b.wait = 1.0;
                return;
            };
            (goal, false, WANDER_BUDGET)
        }
    };
    planner.asked += 1;
    let searched = match nav.plan_path(here, goal, cache, limit) {
        PathPlan::Found(path) => {
            *budget -= 1;
            b.path = Some(path);
            b.next = 0;
            b.failures = 0;
            return;
        }
        // The step's floors are spent; next step, with the same order, it asks again.
        PathPlan::Later => return,
        PathPlan::NoPath => {
            *budget -= 1;
            planner.failed_searches += 1;
            true
        }
        PathPlan::KnownNoPath => false,
    };
    match &mut b.order {
        Order::Patrol { .. } if !searched => {
            // Someone near here found no path near there a moment ago: wait and search for
            // ourselves before skipping the point for good.
            b.wait = 1.0;
        }
        Order::Sentry { .. } => {
            b.wait = RETRY;
            if !b.warned {
                b.warned = true;
                warnings.push(format!("{}: no path to its post", who(content, b.spawn)));
            }
        }
        Order::Patrol { route, target, .. } => {
            planner.failed_points.insert((b.spawn, *target));
            warnings.push(format!(
                "{}: no path to patrol point {} ({:.1}, {:.1}); it is skipped",
                who(content, b.spawn),
                *target + 1,
                goal.x,
                goal.y
            ));
            route[*target] = None;
            *target = next_target(route, *target);
        }
        Order::Wander { .. } => {
            b.failures += 1;
            b.wait = 1.0;
            if b.failures >= WANDER_GIVE_UP && !b.warned {
                b.warned = true;
                warnings.push(format!(
                    "{}: finds no ground to walk to in its area",
                    who(content, b.spawn)
                ));
            }
        }
    }
}

/// An enemy walks its path for a step at its walking speed, on the floor under it; one its senses
/// hold is moved by them instead.
fn walk(e: &mut Enemy, b: &mut Brain, map: &HeightMap, content: &Content) {
    if b.held {
        return;
    }
    if b.wait > 0.0 {
        b.wait = (b.wait - DT).max(0.0);
        e.speed = 0.0;
        return;
    }
    let Some(path) = b.path.clone() else {
        e.speed = 0.0;
        if let Order::Sentry { facing, .. } = &b.order {
            e.facing = *facing;
        }
        return;
    };
    let speed = content.enemies[e.kind].walk_speed;
    if follow(e, &path, &mut b.next, speed, map) {
        b.path = None;
        arrive(e, b, content);
    }
}

/// `e` goes a step along `path` from point `next` at `speed`, on the floor under it, turned the
/// way it moves; whether it has reached the end.
pub(crate) fn follow(
    e: &mut Enemy,
    path: &[V3],
    next: &mut usize,
    speed: f32,
    map: &HeightMap,
) -> bool {
    let from = e.pos;
    let mut left = speed * DT;
    while left > 1e-6 && *next < path.len() {
        let to = path[*next];
        let d = e.pos.distance(to.ground());
        if d <= left {
            e.pos = to.ground();
            e.z = to.z;
            left -= d;
            *next += 1;
        } else {
            e.pos += (to.ground() - e.pos) * (left / d);
            left = 0.0;
        }
    }
    e.z = map.floor_within(e.pos, e.z, MAX_STEP).unwrap_or(e.z);
    let moved = e.pos - from;
    if moved.length() > 1e-4 {
        e.facing = moved.normalize_or_zero();
        e.speed = speed;
    } else {
        e.speed = 0.0;
    }
    *next >= path.len()
}

/// At the end of a path: a sentry turns to its watch, a patrol rests and takes its next point, a
/// wanderer rests a while.
///
/// A sentry also takes where its path ended as its post from now on. The search ends a path on
/// the walkable floor nearest the goal, which for a post against a wall or on a ledge is up to
/// 2 yd from it; held to the post as given, the sentry would never be there and would ask for
/// its path again every step.
fn arrive(e: &mut Enemy, b: &mut Brain, content: &Content) {
    match &mut b.order {
        Order::Sentry { post, facing } => {
            e.facing = *facing;
            *post = V3::at(e.pos, e.z);
        }
        Order::Patrol {
            route,
            target,
            pause,
        } => {
            *target = next_target(route, *target);
            b.wait = *pause;
        }
        Order::Wander { .. } => {
            let (lo, hi) = content.rules.world.wander_pause;
            b.wait = lo + (hi - lo) * b.rng.next_f32();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::heightmap::testmap::{flat, house};
    use crate::{Content, ContentSources, HeightMap, World, V2};

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
        entries: [(5.0, 5.0, 0.0)])"#;
    const ENEMIES: &str = r#"[
        (key: "skeleton", name: "Skeleton", display: 200, level: 28, hp: 900.0, walk_speed: 2.5, run_speed: 7.0),
    ]"#;

    fn world(spawns: &str, paths_per_step: u32) -> World {
        let rules = format!(
            r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
                fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
                world: (share_sight_within: 10.0, paths_per_step: {paths_per_step}, wander_pause: (1.0, 2.0)))"#
        );
        let content = Content::from_ron(ContentSources {
            classes: CLASSES,
            map: MAP,
            rules: &rules,
            enemies: ENEMIES,
            spawns,
            abilities: "[]",
        })
        .unwrap();
        World::new(Arc::new(content))
    }

    /// Flat ground 50 yd across with a closed house (no door, no roof) over [20, 30]².
    fn town() -> Arc<HeightMap> {
        let mut b = flat(50.0);
        house(
            &mut b,
            V2::new(20.0, 20.0),
            V2::new(30.0, 30.0),
            None,
            false,
        );
        Arc::new(b.finish())
    }

    fn in_house(p: V2) -> bool {
        p.x > 20.0 && p.x < 30.0 && p.y > 20.0 && p.y < 30.0
    }

    const GATE: &str =
        r#"[(place: "Gate", kind: "skeleton", behaviour: Sentry(at: (10.0, 10.0), facing: 90.0))]"#;

    #[test]
    fn a_world_without_a_map_has_no_enemies() {
        let mut w = world(GATE, 4);
        w.step();
        assert!(w.enemies.is_empty());
        assert!(w.map().is_none());
    }

    #[test]
    fn a_sentry_holds_its_post_facing_its_way() {
        let mut w = world(GATE, 4);
        w.attach_map(town());
        for _ in 0..30 {
            w.step();
        }
        let e = &w.enemies[0];
        assert_eq!(e.id, 1);
        assert_eq!(e.pos, V2::new(10.0, 10.0));
        assert_eq!(e.z, 0.0);
        assert!(e.facing.y > 0.99, "{:?}", e.facing);
        assert_eq!(e.speed, 0.0);
        assert_eq!(e.hp, 900.0);
        assert!(w.warnings().is_empty());
    }

    #[test]
    fn a_patrol_walks_around_a_house_not_through_it() {
        let mut w = world(
            r#"[(place: "Road", kind: "skeleton", behaviour: Patrol(route: [(25.0, 10.0), (25.0, 40.0)], pause: 0.0))]"#,
            4,
        );
        w.attach_map(town());
        let mut reached = false;
        for _ in 0..900 {
            w.step();
            let e = &w.enemies[0];
            assert!(!in_house(e.pos), "{:?}", e.pos);
            reached |= e.pos.distance(V2::new(25.0, 40.0)) <= 0.6;
        }
        assert!(reached);
    }

    #[test]
    fn wanderers_stay_in_their_area() {
        let mut w = world(
            r#"[(place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (10.0, 40.0), radius: 6.0))]"#,
            4,
        );
        w.attach_map(town());
        assert_eq!(w.enemies.len(), 3);
        let start: Vec<V2> = w.enemies.iter().map(|e| e.pos).collect();
        for _ in 0..1800 {
            w.step();
            for e in &w.enemies {
                assert!(e.pos.distance(V2::new(10.0, 40.0)) <= 6.75, "{:?}", e.pos);
            }
        }
        assert!(w.enemies.iter().zip(&start).any(|(e, s)| e.pos != *s));
    }

    #[test]
    fn an_unreachable_patrol_point_is_skipped_and_named() {
        let mut w = world(
            r#"[(place: "Road", kind: "skeleton", behaviour: Patrol(route: [(10.0, 10.0), (25.0, 25.0), (40.0, 10.0)], pause: 0.0))]"#,
            4,
        );
        w.attach_map(town());
        let mut reached = false;
        for _ in 0..1200 {
            w.step();
            reached |= w.enemies[0].pos.distance(V2::new(40.0, 10.0)) <= 0.6;
        }
        assert!(reached);
        let named = w
            .warnings()
            .iter()
            .filter(|m| m.contains("\"Road\"") && m.contains("patrol point 2"))
            .count();
        assert_eq!(named, 1, "{:?}", w.warnings());
    }

    #[test]
    fn a_spawn_with_no_floor_is_left_out_and_named() {
        let mut b = flat(50.0);
        for col in b.columns_in(V2::new(8.0, 8.0), V2::new(12.0, 12.0)) {
            b.set_ground(col, f32::NAN);
        }
        let mut w = world(GATE, 4);
        w.attach_map(Arc::new(b.finish()));
        assert!(w.enemies.is_empty());
        assert!(w.warnings()[0].contains("\"Gate\""));
    }

    #[test]
    fn no_more_paths_a_step_than_the_rules_allow() {
        let mut w = world(
            r#"[(place: "Pack", kind: "skeleton", count: 10, behaviour: Wander(center: (10.0, 10.0), radius: 5.0))]"#,
            2,
        );
        w.attach_map(town());
        w.step();
        assert_eq!(w.enemies.iter().filter(|e| e.speed > 0.0).count(), 2);
    }

    #[test]
    fn the_same_run_twice_is_the_same() {
        let run = || {
            let mut w = world(
                r#"[
                    (place: "Road", kind: "skeleton", behaviour: Patrol(route: [(25.0, 10.0), (25.0, 40.0)])),
                    (place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (10.0, 40.0), radius: 6.0)),
                ]"#,
                4,
            );
            w.attach_map(town());
            for _ in 0..300 {
                w.step();
            }
            w.snapshot()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_mirror_takes_the_hosts_enemies() {
        let mut host = world(GATE, 4);
        host.attach_map(town());
        host.step();
        let mut mirror = world("[]", 4);
        mirror.apply_snapshot(host.snapshot());
        assert_eq!(mirror.enemies, host.enemies);
        assert!(mirror.enemy(1).is_some());
        mirror.step();
        assert_eq!(mirror.enemies, host.enemies);
    }

    #[test]
    fn a_sentry_cut_off_from_its_post_waits_to_try_again_and_names_it_once() {
        use super::{Brain, Enemy, Navigator, Order, Planner, Rng, RETRY};
        use crate::{DT, V3};

        let w = world(GATE, 4);
        let content = w.content().clone();
        let mut nav = Navigator::new(town());
        // Outside the closed house, with a post inside it.
        let post = V3::new(25.0, 25.0, 0.0);
        let mut enemies = vec![Enemy {
            id: 1,
            kind: 0,
            pos: V2::new(10.0, 10.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 900.0,
            max_hp: 900.0,
            speed: 0.0,
            asleep: false,
            fight: Default::default(),
        }];
        let mut brains = vec![Brain {
            spawn: 0,
            order: Order::Sentry {
                post,
                facing: V2::new(1.0, 0.0),
            },
            path: None,
            next: 0,
            wait: 0.0,
            failures: 0,
            warned: false,
            held: false,
            rng: Rng::new(1),
        }];
        let (mut warnings, mut planner) = (Vec::new(), Planner::default());
        super::step(
            &mut enemies,
            &mut brains,
            &mut nav,
            &content,
            &mut warnings,
            &mut planner,
        );
        // The search failed once: the sentry now rests instead of searching again next step.
        assert!(brains[0].wait > RETRY - 2.0 * DT, "{}", brains[0].wait);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("\"Gate\""));
        assert_eq!(planner.failed_searches(), 1);
        // Over 12 s it retries twice more, and the warning is not repeated.
        for _ in 0..360 {
            super::step(
                &mut enemies,
                &mut brains,
                &mut nav,
                &content,
                &mut warnings,
                &mut planner,
            );
        }
        // The retries do happen (each 5 s), and no more often than that.
        assert_eq!(planner.failed_searches(), 3);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(enemies[0].pos, V2::new(10.0, 10.0));
    }

    #[test]
    fn a_patrol_group_finds_a_sealed_point_sealed_once() {
        // Three patrollers start on the first three points; the fourth is inside the closed house.
        let mut w = world(
            r#"[(place: "Road", kind: "skeleton", count: 3, behaviour: Patrol(
                route: [(10.0, 10.0), (40.0, 10.0), (40.0, 40.0), (25.0, 25.0)], pause: 0.0))]"#,
            4,
        );
        w.attach_map(town());
        assert_eq!(w.enemies.len(), 3);
        for _ in 0..2400 {
            w.step();
        }
        let named = w
            .warnings()
            .iter()
            .filter(|m| m.contains("\"Road\"") && m.contains("patrol point 4"))
            .count();
        assert_eq!(named, 1, "{:?}", w.warnings());
        assert_eq!(w.failed_searches(), 1);
        // Everyone kept walking the rest of the route meanwhile.
        assert!(w.enemies.iter().all(|e| !in_house(e.pos)));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "takes no snapshots")]
    fn a_world_with_a_map_refuses_a_snapshot() {
        let mut host = world(GATE, 4);
        host.attach_map(town());
        let snapshot = host.snapshot();
        host.apply_snapshot(snapshot);
    }

    /// Sentries standing at `stand`, each posted at its `post`, on flat ground 400 yd across
    /// with a closed house over [200, 210]².
    fn lost_sentries(
        stand_and_post: &[(V2, crate::V3)],
    ) -> (Vec<super::Enemy>, Vec<super::Brain>, super::Navigator) {
        use super::{Brain, Enemy, Navigator, Order, Rng};

        let mut b = flat(400.0);
        house(
            &mut b,
            V2::new(200.0, 200.0),
            V2::new(210.0, 210.0),
            None,
            false,
        );
        let mut enemies = Vec::new();
        let mut brains = Vec::new();
        for (i, &(at, post)) in stand_and_post.iter().enumerate() {
            enemies.push(Enemy {
                id: i as u32 + 1,
                kind: 0,
                pos: at,
                z: 0.0,
                facing: V2::new(1.0, 0.0),
                hp: 900.0,
                max_hp: 900.0,
                speed: 0.0,
                asleep: false,
                fight: Default::default(),
            });
            brains.push(Brain {
                spawn: 0,
                order: Order::Sentry {
                    post,
                    facing: V2::new(1.0, 0.0),
                },
                path: None,
                next: 0,
                wait: 0.0,
                failures: 0,
                warned: false,
                held: false,
                rng: Rng::new(i as u64),
            });
        }
        (enemies, brains, Navigator::new(Arc::new(b.finish())))
    }

    #[test]
    fn a_step_never_spends_more_than_its_floors_on_paths() {
        use super::{Planner, PATROL_BUDGET, STEP_FLOORS};
        use crate::V3;

        // Twelve sentries far apart, all posted inside the closed house: twelve searches that
        // each run out of budget, which no step may take on together.
        let ends: Vec<(V2, V3)> = (0..12)
            .map(|k| {
                let at = V2::new(20.0 + 30.0 * k as f32, 60.0);
                (at, V3::new(204.0 + 0.1 * k as f32, 205.0, 0.0))
            })
            .collect();
        let (mut enemies, mut brains, mut nav) = lost_sentries(&ends);
        let content = world(GATE, 4).content().clone();
        let (mut warnings, mut planner) = (Vec::new(), Planner::default());
        for _ in 0..12 {
            let before = nav.floors_looked_at();
            super::step(
                &mut enemies,
                &mut brains,
                &mut nav,
                &content,
                &mut warnings,
                &mut planner,
            );
            let spent = nav.floors_looked_at() - before;
            assert!(spent <= STEP_FLOORS as u64, "{spent} floors in one step");
            assert!(
                spent >= PATROL_BUDGET as u64,
                "the budget went unused: {spent}"
            );
        }
        // Each got its turn, one a step, and each failed once.
        assert_eq!(planner.failed_searches(), 12);
        assert_eq!(warnings.len(), 12, "{warnings:?}");
    }

    #[test]
    fn a_sentry_group_cut_off_from_its_post_searches_once() {
        use super::{Planner, PATROL_BUDGET};
        use crate::V3;

        // Three sentries a yard apart outside the closed house, posted a yard apart inside it.
        let ends = [
            (V2::new(150.25, 150.25), V3::new(204.25, 204.25, 0.0)),
            (V2::new(150.75, 151.25), V3::new(205.25, 204.25, 0.0)),
            (V2::new(151.25, 150.75), V3::new(204.75, 205.25, 0.0)),
        ];
        let (mut enemies, mut brains, mut nav) = lost_sentries(&ends);
        let content = world(GATE, 4).content().clone();
        let (mut warnings, mut planner) = (Vec::new(), Planner::default());
        for _ in 0..30 {
            super::step(
                &mut enemies,
                &mut brains,
                &mut nav,
                &content,
                &mut warnings,
                &mut planner,
            );
        }
        assert_eq!(planner.failed_searches(), 1);
        assert_eq!(nav.floors_looked_at(), PATROL_BUDGET as u64);
        // All three know, and wait to try again.
        assert!(brains.iter().all(|b| b.wait > 0.0));
        assert_eq!(warnings.len(), 3, "{warnings:?}");
    }

    /// Flat ground 50 yd across with tree trunks a yard thick and 8 tall: one at the gate's
    /// post (10, 10), one on a patrol point (30, 10), and a grove of them every 3 yd over the
    /// nest around (10, 40).
    fn woods() -> Arc<HeightMap> {
        let mut b = flat(50.0);
        let mut trunk = |x: f32, y: f32| {
            b.add_box(
                V2::new(x - 0.5, y - 0.5),
                V2::new(x + 0.5, y + 0.5),
                0.0,
                8.0,
            );
        };
        trunk(10.2, 10.2);
        trunk(30.0, 10.0);
        for i in 0..5 {
            for j in 0..5 {
                trunk(4.0 + 3.0 * i as f32, 34.0 + 3.0 * j as f32);
            }
        }
        Arc::new(b.finish())
    }

    #[test]
    fn enemies_stand_beside_a_trunk_never_on_it() {
        let mut w = world(
            r#"[
                (place: "Gate", kind: "skeleton", behaviour: Sentry(at: (10.2, 10.2), facing: 90.0)),
                (place: "Road", kind: "skeleton", behaviour: Patrol(route: [(30.0, 10.0), (30.0, 30.0)], pause: 0.0)),
                (place: "Nest", kind: "skeleton", count: 3, behaviour: Wander(center: (10.0, 40.0), radius: 6.0)),
            ]"#,
            4,
        );
        let map = woods();
        w.attach_map(map.clone());
        assert!(w.warnings().is_empty(), "{:?}", w.warnings());
        let gate = &w.enemies[0];
        assert_eq!(gate.z, 0.0);
        assert!(
            gate.pos.distance(V2::new(10.2, 10.2)) <= 2.0,
            "{:?}",
            gate.pos
        );
        let mut reached = false;
        for _ in 0..1800 {
            w.step();
            for e in &w.enemies {
                assert!(
                    e.z.abs() < 0.01,
                    "enemy {} at {:?} stands at {}",
                    e.id,
                    e.pos,
                    e.z
                );
                assert!(map.stand_near(crate::V3::at(e.pos, e.z)).is_some());
            }
            reached |= w.enemies[1].pos.distance(V2::new(30.0, 10.0)) <= 2.0;
        }
        assert!(reached);
        assert_eq!(w.failed_searches(), 0);
    }

    #[test]
    fn a_spot_with_no_ground_to_walk_on_nearby_is_named() {
        // A sentry posted in the middle of a rock 6 yd across and 3 tall: no ground within 2 yd.
        let rock = || {
            let mut b = flat(50.0);
            b.add_box(V2::new(7.0, 7.0), V2::new(13.0, 13.0), 0.0, 3.0);
            Arc::new(b.finish())
        };
        let mut w = world(GATE, 4);
        w.attach_map(rock());
        assert_eq!(w.enemies[0].z, 3.0);
        assert_eq!(w.warnings().len(), 1, "{:?}", w.warnings());
        assert!(w.warnings()[0].contains("\"Gate\""));
        assert!(w.warnings()[0].contains("cannot walk away"));
        // Posted on the rock's top on purpose, it stands there unremarked.
        let mut w = world(
            r#"[(place: "Gate", kind: "skeleton", z: Some(3.0), behaviour: Sentry(at: (10.0, 10.0), facing: 90.0))]"#,
            4,
        );
        w.attach_map(rock());
        assert_eq!(w.enemies[0].z, 3.0);
        assert!(w.warnings().is_empty(), "{:?}", w.warnings());
    }

    #[test]
    fn a_sentry_posted_against_a_wall_settles_there_and_stops_asking() {
        use super::Planner;
        use crate::V3;

        // The post is on the wall's face, where no unit can stand; the search ends its path on
        // the nearest floor a unit can stand on, a yard out. Walked away and back, the sentry
        // settles there.
        let post = V3::new(20.1, 10.0, 0.0);
        let (mut enemies, mut brains, _) = lost_sentries(&[(V2::new(10.0, 10.0), post)]);
        let mut b = flat(50.0);
        b.add_box(V2::new(20.0, 0.0), V2::new(20.5, 50.0), 0.0, 3.0);
        let mut nav = super::Navigator::new(Arc::new(b.finish()));
        assert!(nav
            .map()
            .stand_near(post)
            .is_some_and(|p| p.ground() != post.ground()));
        let content = world(GATE, 4).content().clone();
        let (mut warnings, mut planner) = (Vec::new(), Planner::default());
        let mut run = |steps: usize, enemies: &mut Vec<super::Enemy>, planner: &mut Planner| {
            for _ in 0..steps {
                super::step(
                    enemies,
                    &mut brains,
                    &mut nav,
                    &content,
                    &mut warnings,
                    planner,
                );
            }
        };
        run(300, &mut enemies, &mut planner);
        let settled = enemies[0].pos;
        assert!(settled.distance(post.ground()) < 2.0, "{settled:?}");
        let asked = planner.asked;
        run(300, &mut enemies, &mut planner);
        assert_eq!(planner.asked, asked, "it kept asking for its path");
        assert_eq!(enemies[0].pos, settled);
        // Pushed away (as a chase will), it walks back to the same spot and settles again.
        enemies[0].pos = V2::new(5.0, 30.0);
        run(600, &mut enemies, &mut planner);
        assert_eq!(enemies[0].pos, settled);
        let asked = planner.asked;
        run(300, &mut enemies, &mut planner);
        assert_eq!(planner.asked, asked);
        assert_eq!(planner.failed_searches(), 0);
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_patrol_member_on_a_point_with_no_floor_is_named_alone() {
        // The first of two points is over a hole: the member starting there is left out, the one
        // starting on the second walks.
        let mut b = flat(50.0);
        for col in b.columns_in(V2::new(6.0, 6.0), V2::new(14.0, 14.0)) {
            b.set_ground(col, f32::NAN);
        }
        let mut w = world(
            r#"[(place: "Road", kind: "skeleton", count: 2, behaviour: Patrol(route: [(10.0, 10.0), (40.0, 10.0)], pause: 0.0))]"#,
            4,
        );
        w.attach_map(Arc::new(b.finish()));
        assert_eq!(w.enemies.len(), 1);
        let warnings = w.warnings();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("patrol point 1") && warnings[0].contains("skipped"));
        assert!(
            warnings[1].contains("member 1 of 2")
                && warnings[1].contains("that member is left out"),
            "{warnings:?}"
        );
    }
}
