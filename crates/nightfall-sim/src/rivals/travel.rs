//! How a party gets anywhere (the corrections' rulings 1 and 2): by the routes of `rivals.ron`,
//! leg by leg. Each leg runs from one waypoint to the next (at most `route_leg` yards, measured
//! on the real bake) and is one search through [`Navigator::plan_path`] on the rivals' own pool
//! of the step's floors ([`RIVAL_POOL`]), so rivals can never make a step spend more than
//! [`crate::STEP_FLOORS`] allows however many are lost. A party that makes no headway for
//! `stuck_for` seconds, or has no open way out, is [`Progress`]'s to give up on: it leaves unseen.

use std::sync::Arc;

use crate::heightmap::{Navigator, PathPlan};
use crate::{ExitDef, RivalContent, RivalRules, RoutePlace, V2, V3};

use super::goals::walk_along;

/// Floors one leg's search may look at. A leg is at most `route_leg` (80) yards; measured on the
/// real Duskwood bake, the dearest of the shipped legs (both ways) takes 812 floors, so this has
/// five times that to go round a house or a ridge, and a leg that finds nothing is given up on
/// cheaply. `nightfall-bake`'s `rivals` test checks every shipped leg is found within it.
pub const RIVAL_BUDGET: usize = 4_000;
/// Floors all of a step's rival searches may look at together: three legs' worth of
/// [`RIVAL_BUDGET`] (`paths_per_step` keeps the parties to two). Kept apart from the enemies'
/// pool, which [`crate::STEP_FLOORS`] adds it to.
pub const RIVAL_POOL: usize = 12_000;
/// Yards a party must close on the end of its route to count as headway.
const HEADWAY: f32 = 1.0;

/// What a step of walking came to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Walk {
    /// The party's walker is now here.
    On(V3),
    /// The step's rival floors are spent: it stands and asks again next step.
    Waiting,
    /// A leg has no way (nothing to stand on at its end, or no path within the budget): the
    /// party can go no further this way and [`Progress`] will give it up.
    Blocked,
    /// The route's last waypoint is reached.
    Arrived,
}

/// A party leader's walk along one route.
#[derive(Clone, Debug)]
pub(crate) struct Travel {
    points: Vec<V2>,
    /// The waypoint walked toward (the route's first is where it starts).
    to: usize,
    path: Option<Arc<[V3]>>,
    next: usize,
}

impl Travel {
    /// A walk along `points` (ends included, as [`RivalContent::route`] gives them), starting at
    /// the first.
    pub(crate) fn new(points: Vec<V2>) -> Self {
        Self {
            points,
            to: 1,
            path: None,
            next: 0,
        }
    }

    /// Yards of route left from `at`: the way to the next waypoint and the legs after it. It only
    /// falls as the party walks on, so a winding route is never mistaken for being stuck.
    pub(crate) fn left(&self, at: V2) -> f32 {
        let Some(&next) = self.points.get(self.to) else {
            return 0.0;
        };
        at.distance(next)
            + self.points[self.to..]
                .windows(2)
                .map(|w| w[0].distance(w[1]))
                .sum::<f32>()
    }

    /// The path of the leg being walked, once searched.
    #[cfg(test)]
    pub(crate) fn path(&self) -> Option<Arc<[V3]>> {
        self.path.clone()
    }

    /// Whether the next [`Travel::step`] starts a search (the leg's path is not known yet).
    pub(crate) fn needs_search(&self) -> bool {
        self.path.is_none() && self.to < self.points.len()
    }

    /// Forgets the leg's path (after a flight or a fight moved the party off it): the next step
    /// searches again from where it stands.
    pub(crate) fn replan(&mut self) {
        self.path = None;
        self.next = 0;
    }

    /// Walks `yards` along the route from `at`, searching a leg when it begins.
    pub(crate) fn step(&mut self, nav: &mut Navigator, at: V3, yards: f32) -> Walk {
        let Some(&target) = self.points.get(self.to) else {
            return Walk::Arrived;
        };
        if self.path.is_none() {
            let Some(goal) = nav
                .map()
                .ground_at(target)
                .and_then(|z| nav.stand_near(V3::at(target, z)))
            else {
                return Walk::Blocked;
            };
            match nav.plan_path(at, goal, false, RIVAL_BUDGET) {
                PathPlan::Found(path) => {
                    self.path = Some(path);
                    self.next = 0;
                }
                PathPlan::Later => return Walk::Waiting,
                PathPlan::NoPath | PathPlan::KnownNoPath => return Walk::Blocked,
            }
        }
        let path = self.path.clone().expect("a path was just found");
        let now = walk_along(at, &path, &mut self.next, yards);
        if self.next >= path.len() {
            self.path = None;
            self.next = 0;
            self.to += 1;
            if self.to >= self.points.len() {
                return Walk::Arrived;
            }
        }
        Walk::On(now)
    }
}

/// How long a party has gone without headway.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Progress {
    best: Option<f32>,
    idle: f32,
    /// The next [`Progress::tick`] starts from its own `left` without ending the idleness: the
    /// goal changed, not the headway.
    rebased: bool,
}

impl Progress {
    /// Notes `left` (yards of route left) after `dt` seconds: headway of a yard starts the count
    /// again. Called while the party is going somewhere; a fight or a rest is not idleness, and
    /// the bot [`Progress::reset`]s for them.
    pub(crate) fn tick(&mut self, left: f32, dt: f32) {
        if std::mem::take(&mut self.rebased) {
            self.best = Some(left);
            self.idle += dt;
            return;
        }
        match self.best {
            Some(b) if left > b - HEADWAY => self.idle += dt,
            _ => {
                self.best = Some(left);
                self.idle = 0.0;
            }
        }
    }

    /// The goal changed (a point given up for the next): what is left is measured from the new
    /// one, but the time already idle stays, so a party that cycles through goals it cannot reach
    /// is still given up on.
    pub(crate) fn rebase(&mut self) {
        self.rebased = true;
    }

    /// Seconds with no headway.
    #[cfg(test)]
    pub(crate) fn idle(&self) -> f32 {
        self.idle
    }

    /// A count that has run `idle` seconds and was nearest `best` yards from its goal.
    #[cfg(test)]
    pub(crate) fn aged(idle: f32, best: f32) -> Self {
        Self {
            best: Some(best),
            idle,
            rebased: false,
        }
    }

    /// Starts counting again (a new goal, a fight, a rest).
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Whether the party has made no headway for `stuck_for` seconds.
    pub(crate) fn stuck(&self, r: &RivalRules) -> bool {
        self.idle >= r.stuck_for
    }
}

/// The exit a party at `from` takes: of those `open` the one whose route is shortest. `None`
/// when no open exit has a route, which is the backstop's other case: the party leaves unseen
/// rather than waits for a way that is shut. Without routes in the content (the tests' small
/// maps) the nearest open exit as the crow flies, from `at`.
pub(crate) fn exit_by_route(
    rivals: &RivalContent,
    exits: &[ExitDef],
    from: RoutePlace,
    at: V2,
    open: impl Fn(usize) -> bool,
) -> Option<usize> {
    let cost = |x: usize| -> Option<f32> {
        if rivals.routes.is_empty() {
            return Some(at.distance(exits[x].at));
        }
        let route = rivals.route(from, RoutePlace::Exit(x))?;
        Some(route.windows(2).map(|w| w[0].distance(w[1])).sum())
    };
    (0..exits.len())
        .filter(|&x| open(x))
        .filter_map(|x| cost(x).map(|c| (c, x)))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, x)| x)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{content, content_with, with_routes};
    use super::*;
    use crate::heightmap::testmap::flat;

    fn nav(size: f32) -> Navigator {
        Navigator::new(Arc::new(flat(size).finish()))
    }

    /// Walks `t` from `at` in strides of `yards` until it comes to something other than a step,
    /// at most `steps` times; returns that and the places walked to.
    fn walk(t: &mut Travel, n: &mut Navigator, mut at: V3, yards: f32, steps: usize) -> (Walk, V3) {
        for _ in 0..steps {
            n.begin_step(RIVAL_POOL);
            match t.step(n, at, yards) {
                Walk::On(p) => at = p,
                other => return (other, at),
            }
        }
        (Walk::On(at), at)
    }

    #[test]
    fn a_party_walks_a_route_leg_by_leg_to_its_end() {
        let mut n = nav(100.0);
        let mut t = Travel::new(vec![
            V2::new(10.0, 10.0),
            V2::new(50.0, 10.0),
            V2::new(50.0, 60.0),
        ]);
        let start = V3::new(10.0, 10.0, 0.0);
        let (end, at) = walk(&mut t, &mut n, start, 1.0, 400);
        assert_eq!(end, Walk::Arrived);
        assert!(
            at.ground().distance(V2::new(50.0, 60.0)) < 1.0,
            "ends at {at:?}"
        );
        // A walk that has arrived stays arrived.
        n.begin_step(RIVAL_POOL);
        assert_eq!(t.step(&mut n, at, 1.0), Walk::Arrived);
    }

    #[test]
    fn what_is_left_of_a_route_only_falls_as_the_party_walks_it() {
        let mut n = nav(100.0);
        let mut t = Travel::new(vec![
            V2::new(10.0, 10.0),
            V2::new(50.0, 10.0),
            V2::new(50.0, 60.0),
        ]);
        let mut at = V3::new(10.0, 10.0, 0.0);
        let mut last = t.left(at.ground());
        assert!((last - 90.0).abs() < 1e-3);
        for _ in 0..300 {
            n.begin_step(RIVAL_POOL);
            let Walk::On(p) = t.step(&mut n, at, 1.0) else {
                break;
            };
            at = p;
            let now = t.left(at.ground());
            assert!(now <= last + 1e-3, "{now} after {last}");
            last = now;
        }
        assert!(last < 1.0);
    }

    #[test]
    fn a_leg_waits_for_a_step_with_the_floors_to_search_it() {
        let mut n = nav(100.0);
        let mut t = Travel::new(vec![V2::new(10.0, 10.0), V2::new(50.0, 10.0)]);
        let at = V3::new(10.0, 10.0, 0.0);
        n.begin_step(RIVAL_BUDGET - 1);
        assert_eq!(t.step(&mut n, at, 1.0), Walk::Waiting);
        assert_eq!(n.floors_looked_at(), 0, "nothing was searched");
        n.begin_step(RIVAL_POOL);
        assert!(matches!(t.step(&mut n, at, 1.0), Walk::On(_)));
        // The search spent from the rivals' pool, never above it.
        assert!(n.floors_looked_at() <= RIVAL_POOL as u64);
    }

    #[test]
    fn a_leg_with_no_way_or_no_ground_is_blocked() {
        let mut b = flat(100.0);
        // A wall across the whole map: nothing walks over to the far side.
        b.add_box(V2::new(40.0, 0.0), V2::new(60.0, 100.0), 0.0, 10.0);
        let mut n = Navigator::new(Arc::new(b.finish()));
        let at = V3::new(10.0, 10.0, 0.0);
        let mut across = Travel::new(vec![V2::new(10.0, 10.0), V2::new(90.0, 10.0)]);
        let (end, _) = walk(&mut across, &mut n, at, 1.0, 5);
        assert_eq!(end, Walk::Blocked);
        let mut off = Travel::new(vec![V2::new(10.0, 10.0), V2::new(500.0, 10.0)]);
        let (end, _) = walk(&mut off, &mut n, at, 1.0, 5);
        assert_eq!(end, Walk::Blocked);
    }

    #[test]
    fn a_party_that_is_pushed_off_its_leg_searches_again_from_where_it_stands() {
        let mut n = nav(100.0);
        let mut t = Travel::new(vec![V2::new(10.0, 10.0), V2::new(50.0, 10.0)]);
        n.begin_step(RIVAL_POOL);
        let Walk::On(first) = t.step(&mut n, V3::new(10.0, 10.0, 0.0), 2.0) else {
            panic!("a first step");
        };
        t.replan();
        let shoved = V3::new(first.x, 40.0, 0.0);
        n.begin_step(RIVAL_POOL);
        let Walk::On(back) = t.step(&mut n, shoved, 2.0) else {
            panic!("a step from the new place");
        };
        assert!(
            back.y < 40.0,
            "walks back toward the leg's end from {shoved:?}: {back:?}"
        );
    }

    #[test]
    fn a_party_with_no_headway_is_stuck_after_the_stuck_for_seconds() {
        let r = RivalRules::default();
        let mut p = Progress::default();
        p.tick(100.0, 1.0);
        for _ in 0..(r.stuck_for as usize - 1) {
            p.tick(100.0, 1.0);
        }
        assert!(!p.stuck(&r), "one second short");
        p.tick(100.0, 1.0);
        assert!(p.stuck(&r));
        // Half a yard is no headway; a yard is, and starts the count again.
        p.reset();
        p.tick(100.0, 1.0);
        for _ in 0..(r.stuck_for as usize) {
            p.tick(99.5, 1.0);
        }
        assert!(p.stuck(&r));
        p.tick(98.0, 1.0);
        assert!(!p.stuck(&r));
        // A reset (a fight, a new goal) clears it.
        for _ in 0..(r.stuck_for as usize + 5) {
            p.tick(98.0, 1.0);
        }
        assert!(p.stuck(&r));
        p.reset();
        assert!(!p.stuck(&r));
    }

    #[test]
    fn a_new_goal_measures_from_itself_but_keeps_the_idle_time() {
        let r = RivalRules::default();
        let mut p = Progress::default();
        p.tick(100.0, 1.0);
        for _ in 0..40 {
            p.tick(100.0, 1.0);
        }
        // The goal changed to a nearer one: that alone is no headway.
        p.rebase();
        p.tick(10.0, 1.0);
        for _ in 0..(r.stuck_for as usize - 41) {
            p.tick(10.0, 1.0);
        }
        assert!(p.stuck(&r), "the idle time carried over");
        // But getting nearer the new goal is.
        p.tick(8.0, 1.0);
        assert!(!p.stuck(&r));
    }

    #[test]
    fn a_party_takes_the_exit_with_the_shortest_route_that_is_open() {
        let c = content_with("", &with_routes(), "");
        let yard = RoutePlace::Poi(0);
        let len = |x: usize| -> f32 {
            let r = c.rivals.route(yard, RoutePlace::Exit(x)).unwrap();
            r.windows(2).map(|w| w[0].distance(w[1])).sum()
        };
        let cart = c.map.exits.iter().position(|e| e.key == "cart").unwrap();
        let all: Vec<usize> = (0..c.map.exits.len()).collect();
        let best = *all
            .iter()
            .min_by(|&&a, &&b| len(a).total_cmp(&len(b)).then(a.cmp(&b)))
            .unwrap();
        let pick = |open: &dyn Fn(usize) -> bool| {
            exit_by_route(&c.rivals, &c.map.exits, yard, V2::ZERO, open)
        };
        assert_eq!(pick(&|_| true), Some(best));
        // The best shut: the next best.
        let next = *all
            .iter()
            .filter(|&&x| x != best)
            .min_by(|&&a, &&b| len(a).total_cmp(&len(b)).then(a.cmp(&b)))
            .unwrap();
        assert_eq!(pick(&|x| x != best), Some(next));
        // Every exit shut: no way out, which is the backstop's case.
        assert_eq!(pick(&|_| false), None);
        // The cart has a route from the yard only listed the other way round: it still counts.
        assert_eq!(pick(&|x| x == cart), Some(cart));
        // A place with no route to an exit (an entry) has none, however many are open.
        let none = exit_by_route(
            &c.rivals,
            &c.map.exits,
            RoutePlace::Entry(0),
            V2::ZERO,
            |_| true,
        );
        assert_eq!(none, None);
    }

    #[test]
    fn without_routes_in_the_content_the_nearest_open_exit_is_taken() {
        let c = content("", "");
        assert!(c.rivals.routes.is_empty());
        let near = c.map.exits.iter().position(|e| e.key == "north").unwrap();
        let at = c.map.exits[near].at + V2::new(1.0, 1.0);
        let pick = exit_by_route(&c.rivals, &c.map.exits, RoutePlace::Poi(0), at, |_| true);
        assert_eq!(pick, Some(near));
        let other = exit_by_route(&c.rivals, &c.map.exits, RoutePlace::Poi(0), at, |x| {
            x != near
        });
        assert!(other.is_some() && other != Some(near));
    }
}
