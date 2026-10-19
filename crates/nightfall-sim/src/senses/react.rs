//! Enemies acting on what they make of the heroes (spec §5.3). Each step every enemy looks (its
//! notice range, by its facing and the hero's visibility, in line of sight and never past the
//! fog) and listens (the noises made since the last step), thinks ([`Mind::think`]), and shouts
//! the step it becomes alerted: every ally within `shout` yards that is not alerted already is
//! alerted to the same hero, and those allies do not shout on. An enemy that is not calm holds its
//! [`Brain`]: wary, it stops and faces the place; checking, it walks there and stands; alerted, it
//! runs at its hero to `chase_reach` and faces them (the swing is sub-project 4's); searching, it
//! walks to where its hero was last seen and looks around. A place no path reaches is looked at
//! from where it stands. Calm again, its brain finds its way back to its order. What holds it in
//! the fight (`crate::combat`) comes as an [`Engage`] each step: out of the fight it neither
//! looks, thinks nor moves; held still, it stands turned to its hero; distracted, it stands turned
//! to the spot while calm or wary; and a chase stops the engage's reach short, at its speed.
//!
//! Every path an enemy's senses ask for goes through [`Navigator::plan_path`], so it shares the
//! step's floor budget with the orders' searches ([`crate::enemies`]) and a failed search is not
//! made again at once; on top of that an enemy that found no way waits [`RETRY`] seconds before it
//! asks again.

use std::sync::Arc;

use super::awareness::{Mind, Sensed, State};
use super::noise::{hears, Noise};
use super::visibility::{facing_factor, fill_rate, notice_range};
use super::Senses;
use crate::combat::{Engage, Hold};
use crate::enemies::{self, heading, Brain, Enemy, ARRIVE, WANDER_BUDGET};
use crate::heightmap::{HeightMap, Navigator, PathPlan};
use crate::{Content, Hero, PlayerId, SenseRules, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

/// Seconds between turns while an enemy looks around.
const LOOK_EVERY: f32 = 2.5;
/// A chase searches its path again once its hero is this far from the path's end, yards.
const REPATH: f32 = 2.0;
/// A chase whose path has run out is at its hero within this many yards past `chase_reach`.
const CHASE_SETTLE: f32 = 1.0;
/// Seconds before asking for a path again to a goal none was found to.
const RETRY: f32 = 1.0;

/// The way an enemy goes while its senses hold it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Way {
    route: Option<Route>,
    /// Seconds left of the wait before asking again for a path to `failed`, the goal none was
    /// found to. The wait is for that goal alone, a place near it (see [`REPATH`]); another is
    /// asked for at once.
    retry: f32,
    failed: Option<V3>,
    /// How many paths this enemy has asked the navigator for (tests count them).
    #[cfg(test)]
    asked: u32,
}

impl Way {
    /// A step passes: the wait for a retry runs down whatever the enemy is doing.
    fn tick(&mut self) {
        self.retry = (self.retry - DT).max(0.0);
    }
}

#[derive(Clone, Debug)]
struct Route {
    goal: V3,
    path: Arc<[V3]>,
    next: usize,
    /// The path ends where the enemy stood when it was found: no nearer is to be had.
    nearest: bool,
}

/// How a step toward a place went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Progress {
    There,
    Going,
    /// No way there: it stands.
    Stuck,
}

/// One step of every enemy's senses: look, listen, think, shout, act.
pub(crate) fn step_enemies(
    s: &mut Senses,
    enemies: &mut [Enemy],
    brains: &mut [Brain],
    heroes: &[Hero],
    nav: &mut Navigator,
    content: &Content,
    engage: &[Engage],
) {
    if s.minds.len() != enemies.len() || brains.len() != enemies.len() {
        return;
    }
    let r = &content.rules.senses;
    let fog = content.rules.atmosphere.fog_end;
    let map = nav.map().clone();
    let noises = std::mem::take(&mut s.noises);
    // Whoever made a noise and has since left (got out, dropped off) is still heard once: a noise
    // marks a place, and the beat that gets a hero out is made as they go.
    let mut gone: Vec<PlayerId> = Vec::new();
    for n in &noises {
        if !heroes.iter().any(|h| h.player == n.by) && !gone.contains(&n.by) {
            gone.push(n.by);
        }
    }
    let mut shouts = std::mem::take(&mut s.shouts);
    let mut sensed: Vec<Sensed> = Vec::with_capacity(heroes.len());
    for (i, e) in enemies.iter().enumerate() {
        if engage.get(i).is_some_and(|g| g.hold == Hold::Out) {
            continue;
        }
        let mind = &mut s.minds[i];
        let eye = V3::at(e.pos, e.z + EYE_HEIGHT);
        // Every hero, seen or not: an alerted enemy must be told it lost sight of its hero.
        sensed.clear();
        sensed.extend(
            heroes
                .iter()
                .map(|h| sense(e, mind, eye, h, &noises, &map, fog, r)),
        );
        for &player in &gone {
            if let Some(heard) = overheard(e, eye, player, &noises, &map, r) {
                sensed.push(Sensed {
                    player,
                    heard: Some(heard),
                    ..Sensed::default()
                });
            }
        }
        if let Some(shout) = mind.think(&sensed, DT, &r.awareness) {
            shouts.push((i, shout));
        }
    }
    // A shout is heard by the allies near its maker that are not after anyone yet. They are
    // alerted without a shout of their own, so it does not carry on from ally to ally. A shout
    // for a hero who is no longer up (a blow of theirs queued it, and they were downed before it
    // was given out) is dropped: the downed are left be, and no ally is sent to the body.
    for (i, shout) in shouts {
        if !shout.at.is_finite()
            || !heroes
                .iter()
                .any(|h| h.player == shout.player && h.fight.up())
        {
            continue;
        }
        let from = enemies[i].pos;
        for (j, ally) in enemies.iter().enumerate() {
            if j != i
                && ally.pos.distance(from) <= r.awareness.shout
                && !s.minds[j].is_alerted()
                && engage.get(j).is_none_or(|g| g.hold != Hold::Out)
            {
                s.minds[j].alert(shout.player, shout.at, &r.awareness);
            }
        }
    }
    // One count of path requests for the senses of the whole step. They go to the enemies from a
    // different one each step, so a crowd that has used them up does not always leave the same
    // few waiting. (The senses spend the step's floors before the orders do.)
    let mut asks = content.rules.world.paths_per_step;
    let n = enemies.len();
    let first = s.cursor % n.max(1);
    s.cursor = s.cursor.wrapping_add(1);
    for k in 0..n {
        let i = (first + k) % n;
        let e = &mut enemies[i];
        let g = engage
            .get(i)
            .copied()
            .unwrap_or(Engage::free(r.chase_reach));
        let mind = &mut s.minds[i];
        let held = mind.state != State::Calm || g.hold != Hold::Free;
        brains[i].hold(held);
        // The wait for a retry runs down whatever holds the enemy, so one freed asks again on time.
        s.ways[i].tick();
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
                e.asleep = false;
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
        react(e, mind, &mut s.ways[i], &map, nav, content, &mut asks, g);
    }
}

/// The loudest noise of `player`, who has left, that enemy `e` hears this step: the place it came
/// from. Nothing is seen of someone who is not there.
fn overheard(
    e: &Enemy,
    eye: V3,
    player: PlayerId,
    noises: &[Noise],
    map: &HeightMap,
    r: &SenseRules,
) -> Option<V3> {
    if !e.pos.is_finite() || !eye.is_finite() || map.grid().column_of(e.pos).is_none() {
        return None;
    }
    noises
        .iter()
        .filter(|n| {
            n.by == player
                && n.at.is_finite()
                && map.grid().column_of(n.at.ground()).is_some()
                && hears(eye, n, Some(map), r.walls)
        })
        .max_by(|a, b| a.radius.total_cmp(&b.radius))
        .map(|n| n.at)
}

/// What enemy `e` sees and hears of hero `h` this step.
#[allow(clippy::too_many_arguments)]
fn sense(
    e: &Enemy,
    mind: &Mind,
    eye: V3,
    h: &Hero,
    noises: &[Noise],
    map: &HeightMap,
    fog: f32,
    r: &SenseRules,
) -> Sensed {
    // The downed and the dead are left be (spec §8.1): no sight of them, no sound.
    if !h.fight.up() {
        return Sensed {
            player: h.player,
            ..Sensed::default()
        };
    }
    let feet = V3::at(h.pos, h.z);
    let grid = map.grid();
    // A ray is only cast between points on the map's grid, so none is ever longer than the fog
    // and a unit off the grid (or not at a number) is unseen rather than walked towards.
    let on_grid = e.pos.is_finite()
        && h.pos.is_finite()
        && eye.is_finite()
        && feet.is_finite()
        && grid.column_of(e.pos).is_some()
        && grid.column_of(h.pos).is_some();
    let d = e.pos.distance(h.pos);
    // An enemy whose facing is not a number looks at the hero from the side.
    let facing = if e.facing.is_finite() {
        facing_factor(e.facing, h.pos - e.pos, mind.cone, r)
    } else {
        r.side
    };
    let range = notice_range(h.visibility, facing, fog, r);
    // How far a ray is worth casting: an enemy after this hero keeps track of them through the
    // fog; any other can notice them no further than its notice range, and a sleeper not at all.
    let looks = if mind.alerted_to(h.player) {
        fog
    } else if mind.blind() {
        f32::NEG_INFINITY
    } else {
        range
    };
    let in_sight = (on_grid
        && d <= looks.min(fog)
        && map.line_of_sight(eye, V3::at(h.pos, h.z + CHEST_HEIGHT)))
    .then_some(feet);
    let noticed = in_sight
        .filter(|_| !mind.blind() && d <= range)
        .map(|at| (at, fill_rate(d, range, &r.awareness)));
    let heard = noises
        .iter()
        .filter(|n| {
            n.by == h.player
                && on_grid
                && n.at.is_finite()
                && grid.column_of(n.at.ground()).is_some()
                && hears(eye, n, Some(map), r.walls)
        })
        .max_by(|a, b| a.radius.total_cmp(&b.radius))
        .map(|n| n.at);
    Sensed {
        player: h.player,
        noticed,
        in_sight,
        heard,
    }
}

/// A held enemy acts on its state for a step.
#[allow(clippy::too_many_arguments)]
fn react(
    e: &mut Enemy,
    mind: &mut Mind,
    way: &mut Way,
    map: &Arc<HeightMap>,
    nav: &mut Navigator,
    content: &Content,
    asks: &mut u32,
    g: Engage,
) {
    let def = &content.enemies[e.kind];
    let r = &content.rules.senses;
    let state = mind.state;
    match state {
        State::Calm => {}
        State::Wary { place, .. } => stand(e, Some(place.ground())),
        State::Checking { place, .. } | State::Searching { place, .. } if !mind.there => {
            if go(
                e,
                way,
                place,
                def.walk_speed * g.speed,
                ARRIVE,
                REPATH,
                map,
                nav,
                asks,
            ) != Progress::Going
            {
                mind.there = true;
            }
        }
        State::Checking { .. } => stand(e, None),
        State::Searching { left, .. } => look_around(e, left, r.awareness.search_for),
        State::Alerted { last_seen, .. } => {
            if go(
                e,
                way,
                last_seen,
                def.run_speed * g.speed,
                g.reach,
                g.reach + CHASE_SETTLE,
                map,
                nav,
                asks,
            ) != Progress::Going
            {
                stand(e, Some(last_seen.ground()));
            }
        }
    }
}

/// It stands, turned toward `face` when given.
fn stand(e: &mut Enemy, face: Option<V2>) {
    e.speed = 0.0;
    if let Some(p) = face {
        if p.is_finite() && p.distance(e.pos) > 1e-3 {
            e.facing = heading(p - e.pos);
        }
    }
}

/// It stands and looks around: a new way every [`LOOK_EVERY`] seconds of its search, the same
/// ways every run.
fn look_around(e: &mut Enemy, left: f32, total: f32) {
    e.speed = 0.0;
    let turn = ((total - left) / LOOK_EVERY).floor();
    e.facing = V2::from_angle(e.id as f32 * 1.3 + turn * 2.2, 1.0);
}

/// A step toward `goal` at `speed`, stopping `reach` short of it: straight when nothing stands in
/// the way, else along a path, asked for again when the goal moves. A path that has run out
/// counts as arrived within `settle` of the goal. At most `asks` paths are asked
/// for in a step by all the senses together; the [`Navigator`] may still put a request off to the
/// next step (its floors are spent) or refuse it (it failed a moment ago). A goal no path was
/// found to is not asked for again for [`RETRY`] seconds ([`Way::tick`] counts them down).
#[allow(clippy::too_many_arguments)]
fn go(
    e: &mut Enemy,
    way: &mut Way,
    goal: V3,
    speed: f32,
    reach: f32,
    settle: f32,
    map: &Arc<HeightMap>,
    nav: &mut Navigator,
    asks: &mut u32,
) -> Progress {
    let grid = map.grid();
    // A goal off the map, or not at a number, is a goal nothing reaches (and a straight walk
    // towards it would not end).
    if !goal.is_finite()
        || grid.column_of(goal.ground()).is_none()
        || grid.column_of(e.pos).is_none()
    {
        way.route = None;
        e.speed = 0.0;
        return Progress::Stuck;
    }
    let to = goal.ground();
    let d = e.pos.distance(to);
    if d <= reach {
        // It stops, keeping its route: a caster stopping short of a hero who shifts a little goes
        // on again at once, and asking for a path every other step would be the cost.
        e.speed = 0.0;
        return Progress::There;
    }
    let here = V3::at(e.pos, e.z);
    if map.walk_line(here, goal) {
        way.route = None;
        // Aim a little inside the reach, so it ends within it rather than on the goal.
        let aim = to - (to - e.pos) * (reach * 0.9 / d);
        let mut next = 0;
        enemies::follow(e, &[V3::at(aim, goal.z)], &mut next, speed, map);
        return Progress::Going;
    }
    let fresh = way
        .route
        .as_ref()
        .is_some_and(|rt| rt.goal.ground().distance(to) <= REPATH);
    // A route that has run out while the goal is still near its end: the enemy is there if it is
    // within `settle` of the goal, or if the path could come no nearer (the search ends on the
    // floor nearest the goal, up to 2 yd off); else the goal is asked for again.
    let ended = fresh
        && way
            .route
            .as_ref()
            .is_some_and(|rt| rt.next >= rt.path.len());
    if ended && (d <= settle || way.route.as_ref().is_some_and(|rt| rt.nearest)) {
        e.speed = 0.0;
        return Progress::There;
    }
    if !fresh || ended {
        let blocked = way.retry > 0.0
            && way
                .failed
                .is_some_and(|f| f.ground().distance(to) <= REPATH);
        if blocked {
            way.route = None;
            e.speed = 0.0;
            return Progress::Stuck;
        }
        if *asks > 0 {
            *asks -= 1;
            #[cfg(test)]
            {
                way.asked += 1;
            }
            match nav.plan_path(here, goal, false, WANDER_BUDGET) {
                PathPlan::Found(path) => {
                    // A path that ends where the enemy stands cannot be bettered by asking again.
                    let nearest = path
                        .last()
                        .is_some_and(|end| end.ground().distance(e.pos) <= ARRIVE);
                    way.route = Some(Route {
                        goal,
                        path,
                        next: 0,
                        nearest,
                    })
                }
                PathPlan::NoPath | PathPlan::KnownNoPath => {
                    way.route = None;
                    way.retry = RETRY;
                    way.failed = Some(goal);
                    e.speed = 0.0;
                    return Progress::Stuck;
                }
                // The step's floors are spent: the old route, if any, goes on until next step.
                PathPlan::Later => {}
            }
        }
    }
    let Some(rt) = way.route.as_mut() else {
        e.speed = 0.0;
        return Progress::Going;
    };
    let path = rt.path.clone();
    if enemies::follow(e, &path, &mut rt.next, speed, map) && rt.goal.ground().distance(to) > REPATH
    {
        // The goal moved on as it arrived: next step asks again.
        way.route = None;
    }
    Progress::Going
}

#[cfg(test)]
impl Senses {
    /// How many paths enemy `i`'s senses have asked for while held.
    pub(crate) fn paths_asked(&self, i: usize) -> u32 {
        self.ways.get(i).map_or(0, |w| w.asked)
    }

    /// How suspicious enemy `i` is of `player`, 0 to 100.
    pub(crate) fn suspicion_of(&self, i: usize, player: crate::PlayerId) -> f32 {
        self.minds.get(i).map_or(0.0, |m| m.level(player))
    }

    /// Shouts raised and not yet given out, forgotten: a test's alert of one enemy then wakes
    /// none of its allies.
    pub(crate) fn drop_shouts(&mut self) {
        self.shouts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heightmap::testmap::{flat, house};

    /// An enemy outside a closed house, a goal inside it, and the navigator over the map.
    fn sealed() -> (Enemy, V3, Navigator, Arc<HeightMap>) {
        let mut b = flat(50.0);
        house(&mut b, V2::new(30.0, 30.0), V2::new(40.0, 40.0), None, true);
        let map = Arc::new(b.finish());
        let e = Enemy {
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
        };
        (
            e,
            V3::new(35.0, 35.0, 0.0),
            Navigator::new(map.clone()),
            map,
        )
    }

    /// How many paths an enemy that wants to get to a place nothing reaches asks for in `secs`
    /// seconds, with as many asks a step as it likes.
    fn asked_over(secs: f32) -> u32 {
        let (mut e, goal, mut nav, map) = sealed();
        let mut way = Way::default();
        let all = 1000;
        let mut used = 0;
        for _ in 0..(secs / DT) as usize {
            way.tick();
            nav.begin_step(100_000);
            let mut asks = all;
            let p = go(
                &mut e, &mut way, goal, 7.0, ARRIVE, ARRIVE, &map, &mut nav, &mut asks,
            );
            assert_eq!(p, Progress::Stuck);
            used += all - asks;
        }
        used
    }

    #[test]
    fn an_enemy_with_no_way_does_not_ask_for_one_every_step() {
        // Once at the start, and again each RETRY second (the navigator's own memory of a failure
        // spares the search itself, but not the asking): three seconds, at most four asks, not 90.
        let asked = asked_over(3.0);
        assert!((1..=4).contains(&asked), "{asked} paths asked for in 3 s");
    }

    #[test]
    fn a_path_that_ends_short_of_the_goal_is_not_asked_for_again_and_again() {
        // A goal inside a wall: the path ends on the floor nearest it, up to 2 yd away, and the
        // enemy stands there, the search done once.
        let mut b = flat(50.0);
        b.add_box(V2::new(30.0, 0.0), V2::new(32.0, 50.0), 0.0, 3.0);
        let map = Arc::new(b.finish());
        let mut nav = Navigator::new(map.clone());
        let mut e = Enemy {
            id: 1,
            kind: 0,
            pos: V2::new(10.0, 25.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 900.0,
            max_hp: 900.0,
            speed: 0.0,
            asleep: false,
            fight: Default::default(),
        };
        let goal = V3::new(30.5, 25.0, 0.0);
        let mut way = Way::default();
        let mut used = 0;
        let mut last = Progress::Going;
        for _ in 0..600 {
            way.tick();
            nav.begin_step(100_000);
            let mut asks = 1000;
            last = go(
                &mut e, &mut way, goal, 7.0, ARRIVE, ARRIVE, &map, &mut nav, &mut asks,
            );
            used += 1000 - asks;
        }
        assert_ne!(last, Progress::Going, "{:?}", e.pos);
        assert!(used <= 3, "{used} paths asked for");
        assert!(e.pos.x > 29.0 && e.pos.x < 30.0, "{:?}", e.pos);
    }

    #[test]
    fn a_goal_off_the_map_or_not_a_number_is_stuck() {
        let (mut e, _, mut nav, map) = sealed();
        for goal in [
            V3::new(f32::NAN, 5.0, 0.0),
            V3::new(1e30, 5.0, 0.0),
            V3::new(5.0, f32::INFINITY, 0.0),
        ] {
            let mut way = Way::default();
            let mut asks = 10;
            nav.begin_step(100_000);
            let p = go(
                &mut e, &mut way, goal, 7.0, ARRIVE, ARRIVE, &map, &mut nav, &mut asks,
            );
            assert_eq!(p, Progress::Stuck);
            assert_eq!(asks, 10);
            assert_eq!(e.pos, V2::new(10.0, 10.0));
        }
    }
    #[test]
    fn a_route_that_ran_out_short_of_a_moved_goal_is_asked_for_again() {
        // The enemy stands at the end of a route to (13, 25); the goal is now (13.5, 25), 3.5 yd
        // off behind a wall, and the route is not known to be the nearest there is.
        let mut b = flat(50.0);
        b.add_box(V2::new(11.0, 0.0), V2::new(11.5, 40.0), 0.0, 3.0);
        let map = Arc::new(b.finish());
        let mut nav = Navigator::new(map.clone());
        nav.begin_step(100_000);
        let (mut e, ..) = sealed();
        e.pos = V2::new(10.0, 25.0);
        let mut way = Way {
            route: Some(Route {
                goal: V3::new(13.0, 25.0, 0.0),
                path: Arc::from([V3::new(10.0, 25.0, 0.0)]),
                next: 1,
                nearest: false,
            }),
            ..Way::default()
        };
        let mut asks = 5;
        let goal = V3::new(13.5, 25.0, 0.0);
        let p = go(
            &mut e, &mut way, goal, 7.0, 2.0, 3.0, &map, &mut nav, &mut asks,
        );
        assert_eq!(asks, 4, "it asked again");
        assert_ne!(p, Progress::There);
        // Within the settle distance the same ended route is arrival, and nothing is asked.
        let mut way = Way {
            route: Some(Route {
                goal: V3::new(13.0, 25.0, 0.0),
                path: Arc::from([V3::new(10.0, 25.0, 0.0)]),
                next: 1,
                nearest: false,
            }),
            ..Way::default()
        };
        let mut asks = 5;
        let p = go(
            &mut e, &mut way, goal, 7.0, 2.0, 3.6, &map, &mut nav, &mut asks,
        );
        assert_eq!((p, asks), (Progress::There, 5));
    }

    #[test]
    fn an_enemy_that_found_no_path_asks_again_after_the_retry_wait() {
        use crate::senses::tests::{put, sentry, world};
        // A sentry outside a sealed house, after a hero inside it: no path, ever. Its wait runs
        // down each step (`Way::tick`), so it asks again every RETRY seconds; were the wait never
        // counted down, it would ask once and never again.
        let mut w = world(&sentry(10.0, 10.0, 0.0));
        let mut b = flat(50.0);
        house(&mut b, V2::new(30.0, 30.0), V2::new(40.0, 40.0), None, true);
        w.attach_map(Arc::new(b.finish()));
        put(&mut w, 0, 35.0, 35.0, crate::Gait::Still, 0);
        w.step();
        w.alert(1, 0);
        let (mut waits, mut last) = (0, 0.0);
        for _ in 0..90 {
            w.step();
            let now = w.senses.ways[0].retry;
            if now > last {
                waits += 1;
            }
            last = now;
        }
        assert!(waits >= 2, "it waited for a retry {waits} time(s) in 3 s");
    }

    #[test]
    fn a_held_enemys_retry_wait_runs_down() {
        use crate::combat::{Held, Hold};
        use crate::senses::tests::{put, sentry, world};
        use crate::{ControlKind, Gait};
        // A rooted enemy stands (`Hold::Still`) and looks for no path, but its wait for a retry
        // still counts down, so once freed it asks again on time.
        let mut w = world(&sentry(10.0, 10.0, 0.0));
        w.attach_map(Arc::new(flat(50.0).finish()));
        put(&mut w, 0, 35.0, 35.0, Gait::Still, 0);
        w.step();
        w.combat.enemies[0].held = Some(Held {
            kind: ControlKind::Root,
            left: 100.0,
            by: 0,
            spell: 0,
        });
        w.senses.ways[0].retry = 1.0;
        for _ in 0..15 {
            w.step();
        }
        assert_eq!(w.combat.enemies[0].engage.hold, Hold::Still);
        let left = w.senses.ways[0].retry;
        assert!(left < 0.55, "the wait did not run down: {left}");
        for _ in 0..30 {
            w.step();
        }
        assert_eq!(w.senses.ways[0].retry, 0.0);
    }
}
