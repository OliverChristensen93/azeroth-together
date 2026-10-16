//! Enemies acting on what they make of the heroes (spec §5.3). Each step every enemy looks (its
//! notice range, by its facing and the hero's visibility, in line of sight and never past the
//! fog) and listens (the noises made since the last step), thinks ([`Mind::think`]), and shouts
//! the step it becomes alerted: every ally within `shout` yards that is not alerted already is
//! alerted to the same hero, and those allies do not shout on. An enemy that is not calm holds its
//! [`Brain`]: wary, it stops and faces the place; checking, it walks there and stands; alerted, it
//! runs at its hero to `chase_reach` and faces them (the swing is sub-project 4's); searching, it
//! walks to where its hero was last seen and looks around. A place no path reaches is looked at
//! from where it stands. Calm again, its brain finds its way back to its order.
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
use crate::enemies::{self, heading, Brain, Enemy, ARRIVE, WANDER_BUDGET};
use crate::heightmap::{HeightMap, Navigator, PathPlan};
use crate::{Content, Hero, SenseRules, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3};

/// Seconds between turns while an enemy looks around.
const LOOK_EVERY: f32 = 2.5;
/// A chase searches its path again once its hero is this far from the path's end, yards.
const REPATH: f32 = 2.0;
/// Seconds before asking for a path again to a goal none was found to.
const RETRY: f32 = 1.0;

/// The way an enemy goes while its senses hold it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Way {
    route: Option<Route>,
    /// Seconds before asking for a path again to a goal none was found to.
    retry: f32,
}

#[derive(Clone, Debug)]
struct Route {
    goal: V3,
    path: Arc<[V3]>,
    next: usize,
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
) {
    if s.minds.len() != enemies.len() || brains.len() != enemies.len() {
        return;
    }
    let r = &content.rules.senses;
    let fog = content.rules.atmosphere.fog_end;
    let map = nav.map().clone();
    let noises = std::mem::take(&mut s.noises);
    let mut shouts = std::mem::take(&mut s.shouts);
    for (i, e) in enemies.iter().enumerate() {
        let mind = &mut s.minds[i];
        let eye = V3::at(e.pos, e.z + EYE_HEIGHT);
        // Every hero, seen or not: an alerted enemy must be told it lost sight of its hero.
        let sensed: Vec<Sensed> = heroes
            .iter()
            .map(|h| sense(e, mind, eye, h, &noises, &map, fog, r))
            .collect();
        if let Some(shout) = mind.think(&sensed, DT, &r.awareness) {
            shouts.push((i, shout));
        }
    }
    // A shout is heard by the allies near its maker that are not after anyone yet. They are
    // alerted without a shout of their own, so it does not carry on from ally to ally.
    for (i, shout) in shouts {
        if !shout.at.is_finite() || !heroes.iter().any(|h| h.player == shout.player) {
            continue;
        }
        let from = enemies[i].pos;
        for (j, ally) in enemies.iter().enumerate() {
            if j != i && ally.pos.distance(from) <= r.awareness.shout && !s.minds[j].is_alerted() {
                s.minds[j].alert(shout.player, shout.at, &r.awareness);
            }
        }
    }
    // One count of path requests for the senses of the whole step.
    let mut asks = content.rules.world.paths_per_step;
    for (i, e) in enemies.iter_mut().enumerate() {
        let mind = &mut s.minds[i];
        let held = mind.state != State::Calm;
        brains[i].hold(held);
        if !held {
            s.ways[i] = Way::default();
            e.asleep = mind.sleeper && brains[i].at_post(e.pos);
            continue;
        }
        e.asleep = false;
        react(e, mind, &mut s.ways[i], &map, nav, content, &mut asks);
    }
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
    let in_sight =
        (on_grid && d <= fog && map.line_of_sight(eye, V3::at(h.pos, h.z + CHEST_HEIGHT)))
            .then_some(feet);
    let noticed = in_sight.filter(|_| !mind.blind()).and_then(|at| {
        // An enemy whose facing is not a number looks at the hero from the side.
        let facing = if e.facing.is_finite() {
            facing_factor(e.facing, h.pos - e.pos, mind.cone, r)
        } else {
            r.side
        };
        let range = notice_range(h.visibility, facing, fog, r);
        (d <= range).then(|| (at, fill_rate(d, range, &r.awareness)))
    });
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
fn react(
    e: &mut Enemy,
    mind: &mut Mind,
    way: &mut Way,
    map: &Arc<HeightMap>,
    nav: &mut Navigator,
    content: &Content,
    asks: &mut u32,
) {
    let def = &content.enemies[e.kind];
    let r = &content.rules.senses;
    let state = mind.state;
    match state {
        State::Calm => {}
        State::Wary { place, .. } => stand(e, Some(place.ground())),
        State::Checking { place, .. } | State::Searching { place, .. } if !mind.there => {
            if go(e, way, place, def.walk_speed, ARRIVE, map, nav, asks) != Progress::Going {
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
                def.run_speed,
                r.chase_reach,
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
/// the way, else along a path, asked for again when the goal moves. At most `asks` paths are asked
/// for in a step by all the senses together; the [`Navigator`] may still put a request off to the
/// next step (its floors are spent) or refuse it (it failed a moment ago). A goal no path was
/// found to is not asked for again for [`RETRY`] seconds.
#[allow(clippy::too_many_arguments)]
fn go(
    e: &mut Enemy,
    way: &mut Way,
    goal: V3,
    speed: f32,
    reach: f32,
    map: &Arc<HeightMap>,
    nav: &mut Navigator,
    asks: &mut u32,
) -> Progress {
    way.retry = (way.retry - DT).max(0.0);
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
        e.speed = 0.0;
        way.route = None;
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
    if fresh {
        let rt = way.route.as_ref().unwrap();
        if rt.next >= rt.path.len() {
            // It has come as near as the path goes (the search ends on the floor nearest the
            // goal): asking again would only find the same end.
            e.speed = 0.0;
            return Progress::There;
        }
    } else {
        if way.retry > 0.0 {
            way.route = None;
            e.speed = 0.0;
            return Progress::Stuck;
        }
        if *asks > 0 {
            *asks -= 1;
            match nav.plan_path(here, goal, false, WANDER_BUDGET) {
                PathPlan::Found(path) => {
                    way.route = Some(Route {
                        goal,
                        path,
                        next: 0,
                    })
                }
                PathPlan::NoPath | PathPlan::KnownNoPath => {
                    way.route = None;
                    way.retry = RETRY;
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
            nav.begin_step(100_000);
            let mut asks = all;
            let p = go(
                &mut e, &mut way, goal, 7.0, ARRIVE, &map, &mut nav, &mut asks,
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
        };
        let goal = V3::new(30.5, 25.0, 0.0);
        let mut way = Way::default();
        let mut used = 0;
        let mut last = Progress::Going;
        for _ in 0..600 {
            nav.begin_step(100_000);
            let mut asks = 1000;
            last = go(
                &mut e, &mut way, goal, 7.0, ARRIVE, &map, &mut nav, &mut asks,
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
                &mut e, &mut way, goal, 7.0, ARRIVE, &map, &mut nav, &mut asks,
            );
            assert_eq!(p, Progress::Stuck);
            assert_eq!(asks, 10);
            assert_eq!(e.pos, V2::new(10.0, 10.0));
        }
    }
}
