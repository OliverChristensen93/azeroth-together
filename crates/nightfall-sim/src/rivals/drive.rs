//! The rivals' bots (spec §7.4): each step every party picks its goal ([`decide`]) from what its
//! members know, and every rival's input for the next step is set as a player's client sets its
//! own: a pose walked along a path at its pace, a gait, Stealth presses, an ability press picked
//! by tag ([`choose`]), a dodge, and the acts that search and empty a container. The host takes
//! those inputs the way it takes a player's, so nothing a rival does skips a rule: its presses
//! are checked, its searches heard, its pace its load's.
//!
//! A party travels by the routes of `rivals.ron` (the corrections' ruling 1): its leader walks a
//! route leg by leg ([`Travel`]), its followers keep behind it, and every search is one of the
//! rivals' own pool of the step's floors ([`RIVAL_POOL`]), inside the step's `begin_step`. A
//! party that gets nowhere for `stuck_for` seconds leaves unseen.

use super::goals::{
    away_from, decide, formation_spot, next_poi, walk_along, FoeSeen, Goal, PartyView, WALK_RATIO,
};
use super::kit::{choose, ready, Moment};
use super::travel::{exit_by_route, Walk};
use super::{Journey, Travel, Way, RIVAL_BUDGET, RIVAL_POOL};
use crate::combat::stands_right;
use crate::heightmap::PathPlan;
use crate::senses::State;
use crate::{
    Act, ActPress, Aim, CastPress, Facing, Gait, HeroPose, Holder, Moving, PlayerId, PlayerInput,
    RoutePlace, Tag, UnitRef, World, CHEST_HEIGHT, DT, EYE_HEIGHT, V2, V3,
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
/// Yards a member may fall behind its leader before the leader waits for it.
const REGROUP: f32 = 14.0;
/// Yards from its leader within which a follower takes its place in the formation.
const FORMATION: f32 = 6.0;

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
    /// It wanted to move and could not: no way is known, or the step had no floors left.
    blocked: bool,
}

/// What one step toward a spot came to.
enum Step {
    /// It stands here next.
    To(V3),
    /// It is there already.
    There,
    /// No way is known (yet).
    Blocked,
}

impl World {
    /// The rivals' turn, last in a step (their senses came first, before the enemies took the
    /// step's noises): every party picks its goal, a party that is stuck slips away unseen, and
    /// every rival's input for the next step is set.
    pub(crate) fn step_rivals(&mut self) {
        if self.map().is_none() || self.rivals.bodies.is_empty() {
            return;
        }
        self.rivals.paths_left = self.content().rules.rivals.paths_per_step;
        self.rivals.floors_start = self.nav.as_ref().map_or(0, |n| n.floors_looked_at());
        for pi in 0..self.rivals.parties.len() {
            self.plan_party(pi);
        }
        self.slip_away_unseen();
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

    /// The backstop (the corrections' ruling 1): a party that made no headway for `stuck_for`
    /// seconds, and a rival that could not move for as long, vanish with their packs once no hero
    /// of the party sees them. One a hero sees stays and is tried again next step.
    fn slip_away_unseen(&mut self) {
        let stuck_for = self.content().rules.rivals.stuck_for;
        let mut going: Vec<PlayerId> = Vec::new();
        for p in &self.rivals.parties {
            if p.leaving {
                going.extend(&p.members);
            }
        }
        going.extend(
            self.rivals
                .bodies
                .iter()
                .filter(|b| b.pinned >= stuck_for)
                .map(|b| b.player),
        );
        for player in going {
            self.rival_leaves_unseen(player);
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
    /// in it, the next point is picked, [`decide`] says what it is about, its route is kept, and
    /// a goal it makes no headway toward for `give_up` seconds is given up (and a party that
    /// makes none toward anything for `stuck_for` is left to slip away).
    fn plan_party(&mut self, pi: usize) {
        let content = self.content().clone();
        let living = self.living(pi);
        let Some(&leader) = living.first() else {
            return;
        };
        let lpos = self.hero(leader).map_or(V2::ZERO, |h| h.pos);
        if let Goal::Flee { left, .. } | Goal::Hunt { left, .. } = &mut self.rivals.parties[pi].goal
        {
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
                p.place = Some(RoutePlace::Poi(poi));
                p.journey = None;
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
            p.progress.rebase();
        }
        p.goal = goal;
        self.keep_journey(pi, goal, lpos);
        let waiting = self.waits_for_party(pi, leader);
        self.headway(pi, lpos, waiting);
        let p = &mut self.rivals.parties[pi];
        p.leaving = p.progress.stuck(&content.rules.rivals);
    }

    /// The route party `pi`'s leader walks for `goal`: to a point not yet reached, or out by an
    /// exit it is not in the ring of. Any other goal sets the walk's leg aside (a fight or a
    /// flight moved the leader off it), to be searched again from where it stands.
    fn keep_journey(&mut self, pi: usize, goal: Goal, lpos: V2) {
        let content = self.content().clone();
        let looting = self.rivals.parties[pi].looting;
        let dest = match goal {
            Goal::Loot { poi } if looting != Some(poi) => Some(RoutePlace::Poi(poi)),
            Goal::Extract { exit } => Some(RoutePlace::Exit(exit)),
            _ => None,
        };
        let Some(dest) = dest else {
            if let Some(j) = &mut self.rivals.parties[pi].journey {
                j.travel.replan();
            }
            return;
        };
        // In the ring of the exit it walks to, a leader has arrived: no route is walked again.
        if let RoutePlace::Exit(x) = dest {
            let e = &content.map.exits[x];
            if lpos.distance(e.at) <= e.radius {
                self.rivals.parties[pi].journey = None;
                return;
            }
        }
        if self.rivals.parties[pi]
            .journey
            .as_ref()
            .is_some_and(|j| j.to == dest)
        {
            return;
        }
        let journey = if content.rivals.routes.is_empty() {
            None
        } else {
            self.route_points(pi, dest).map(|points| Journey {
                to: dest,
                travel: Travel::new(points),
            })
        };
        self.rivals.parties[pi].journey = journey;
    }

    /// The route from where party `pi` last was to `dest`, the waypoints in order. The content
    /// has a route from each entry to each point and from each point to each exit; the rest are
    /// made of those: a point to another by way of the party's entry, an entry to an exit by way
    /// of the point that makes it shortest. `None` when there is no such way, or no place yet.
    pub(super) fn route_points(&self, pi: usize, dest: RoutePlace) -> Option<Vec<V2>> {
        let rc = &self.content().rivals;
        let p = &self.rivals.parties[pi];
        let here = p.place?;
        if here == dest {
            return None;
        }
        if let Some(r) = rc.route(here, dest) {
            return Some(r);
        }
        let join = |mut a: Vec<V2>, b: Vec<V2>| {
            a.extend(b.into_iter().skip(1));
            a
        };
        match (here, dest) {
            (RoutePlace::Poi(_), RoutePlace::Poi(_)) => {
                // Back along the way in as far as the two ways in part, then out along the
                // other: the start both share is walked once, not twice.
                let entry = RoutePlace::Entry(p.entry.unwrap_or(0));
                let (mut back, out) = (rc.route(entry, here)?, rc.route(entry, dest)?);
                let shared = back
                    .iter()
                    .zip(&out)
                    .take_while(|(a, b)| a == b)
                    .count()
                    .max(1);
                back.reverse();
                back.truncate(back.len() - shared + 1);
                back.extend(out.into_iter().skip(shared));
                Some(back)
            }
            (RoutePlace::Entry(_), RoutePlace::Exit(_)) => (0..rc.pois.len())
                .filter_map(|q| {
                    let poi = RoutePlace::Poi(q);
                    Some(join(rc.route(here, poi)?, rc.route(poi, dest)?))
                })
                .min_by(|a, b| length(a).total_cmp(&length(b))),
            _ => None,
        }
    }

    /// Whether party `pi`'s leader, at `lpos`, gets nearer its goal's place (by what is left of
    /// its route, or as the crow flies without one); after `give_up` seconds without, the goal is
    /// given up: a point counts as visited, an exit as shut to them, a hunt ends, a check is
    /// dropped. The same reading feeds the backstop's clock.
    /// A leader `waiting` for its party makes no headway, but that is no reason to give the goal up.
    fn headway(&mut self, pi: usize, lpos: V2, waiting: bool) {
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
            // Waiting with nothing to do is idleness; a fight, a rest, a look or a loot is not.
            match p.goal {
                Goal::Wait => p.progress.tick(0.0, DT),
                _ => p.progress.reset(),
            }
            return;
        };
        let d = match &p.journey {
            Some(j) => j.travel.left(lpos),
            None => lpos.distance(place),
        };
        p.progress.tick(d, DT);
        if d < p.best - HEADWAY {
            p.best = d;
            p.stalled = 0.0;
        } else if !waiting {
            // A leader that waits for a member of its party is not stalled, only held.
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
                for b in self
                    .rivals
                    .bodies
                    .iter_mut()
                    .filter(|b| members.contains(&b.player))
                {
                    if matches!(
                        b.mind.state,
                        State::Checking { .. } | State::Wary { .. } | State::Searching { .. }
                    ) {
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
        for b in self
            .rivals
            .bodies
            .iter_mut()
            .filter(|b| members.contains(&b.player))
        {
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
            late: content
                .rivals
                .leave_by
                .is_some_and(|s| self.night.stage >= s),
            exit: self.pick_exit(pi, lpos),
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
        self.pack(player)
            .is_none_or(|i| i.first_empty(false).is_none())
            || load >= full_at
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

    /// The exit party `pi` takes: of those open now and not given up, the one whose route from
    /// where it last was is shortest; without routes in the content (a small map), the nearest
    /// to `at` as the crow flies. `None` when no open exit has a way.
    fn pick_exit(&self, pi: usize, at: V2) -> Option<usize> {
        let now = self.elapsed;
        let content = self.content();
        let p = &self.rivals.parties[pi];
        let open = |x: usize| !p.shut.contains(&x) && self.raid.roll.exit_open(x, now);
        // From a point (or without routes, on a small map) the content's own routes say.
        if content.rivals.routes.is_empty() || matches!(p.place, Some(RoutePlace::Poi(_))) {
            let here = p.place.unwrap_or(RoutePlace::Entry(0));
            return exit_by_route(&content.rivals, &content.map.exits, here, at, open);
        }
        (0..content.map.exits.len())
            .filter(|&x| open(x))
            .filter_map(|x| {
                let points = self.route_points(pi, RoutePlace::Exit(x))?;
                Some((length(&points), x))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, x)| x)
    }

    /// Whether container `id` is one `player` would loot: unlocked, not someone's remains,
    /// unsearched or holding something, and not one it is done with.
    fn lootable(&self, id: u32, player: PlayerId) -> bool {
        let Some(c) = self.raid.containers.iter().find(|c| c.id == id) else {
            return false;
        };
        let done = self
            .rivals
            .body(player)
            .is_some_and(|b| b.done.contains(&id));
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
                Goal::Check { at } => {
                    self.go_with_party(pi, player, hi, at.ground(), false, false, &mut plan)
                }
                Goal::Hunt { at, .. } => {
                    self.go_with_party(pi, player, hi, at.ground(), true, false, &mut plan)
                }
                Goal::Hold { face } => plan.face = Some(face - self.heroes[hi].pos),
                Goal::Flee { from, .. } => self.flee_plan(player, hi, from, &mut plan),
                Goal::Extract { exit } => self.exit_plan(pi, player, hi, exit, &mut plan),
                Goal::Wait => {}
            }
        }
        if busy || held {
            plan.to = None;
            plan.gait = Gait::Still;
            plan.blocked = false;
        }
        let kind = self.rivals.body(player).map_or(0, |b| b.kind);
        plan.stealth = self.content().rivals.kinds[kind].sneaks
            && !matches!(goal, Goal::Fight | Goal::Flee { .. });
        self.send(player, hi, plan);
    }

    /// The leader of party `pi` and `player`'s rank among the living (0 leads).
    fn rank_of(&self, pi: usize, player: PlayerId) -> (PlayerId, usize) {
        let living = self.living(pi);
        let rank = living.iter().position(|&m| m == player).unwrap_or(0);
        (living.first().copied().unwrap_or(player), rank)
    }

    /// Whether the party walks calmly: a hunted party, or one past the night's `leave_by`, runs.
    fn hurries(&self, pi: usize) -> bool {
        let late = self
            .content()
            .rivals
            .leave_by
            .is_some_and(|s| self.night.stage >= s);
        late || self.rivals.parties[pi].hunted
    }

    /// `player` goes toward `place` with its party: the leader there (along its route when it has
    /// one and `routed`), the rest behind it in formation, catching up at a run when far behind.
    fn go_with_party(
        &mut self,
        pi: usize,
        player: PlayerId,
        hi: usize,
        place: V2,
        run: bool,
        routed: bool,
        plan: &mut Plan,
    ) {
        let gap = self.content().rules.rivals.gap;
        let (leader, rank) = self.rank_of(pi, player);
        let run = run || self.hurries(pi);
        let goal = match self.hero(leader).filter(|_| leader != player) {
            Some(l) => self.follow_spot(hi, l.pos, l.facing, rank, gap),
            None => {
                if self.waits_for_party(pi, player) {
                    return;
                }
                if routed && self.route_step(pi, hi, run, plan) {
                    return;
                }
                place
            }
        };
        let far = self.heroes[hi].pos.distance(goal);
        if far <= THERE {
            return;
        }
        let run = run || (leader != player && far > 8.0);
        let goal_z = self.hero(leader).filter(|_| leader != player).map(|l| l.z);
        self.walk_to(player, hi, goal, goal_z, run, plan);
        // A follower close to its leader that finds no way to its spot is where it should be.
        if leader != player
            && plan.to.is_none()
            && self
                .hero(leader)
                .is_some_and(|l| l.pos.distance(self.heroes[hi].pos) <= FORMATION)
        {
            plan.blocked = false;
        }
    }

    /// Where a follower of rank `rank` walks to behind a leader at `lead` facing `facing`: its place
    /// in the formation once it is near, the leader itself while it is far. A place in the formation
    /// is a spot beside the way, which has no floor to reach it by as often as not on rough ground;
    /// the leader stands on one.
    fn follow_spot(&self, hi: usize, lead: V2, facing: V2, rank: usize, gap: f32) -> V2 {
        if self.heroes[hi].pos.distance(lead) > FORMATION {
            lead
        } else {
            formation_spot(lead, facing, rank, gap)
        }
    }

    /// Whether the leader of party `pi`, `player`, waits: a member of its party is further behind
    /// than [`REGROUP`] yards. A party keeps together on its way; a follower that cannot come
    /// (no way to it) gives itself up in time and the party walks on.
    fn waits_for_party(&self, pi: usize, player: PlayerId) -> bool {
        let Some(me) = self.hero(player) else {
            return false;
        };
        self.living(pi)
            .into_iter()
            .filter(|&m| m != player)
            .filter_map(|m| self.hero(m))
            .any(|h| h.pos.distance(me.pos) > REGROUP)
    }

    /// The leader's next step along its party's route, if it has one to walk. Whether the route
    /// took the turn (when it did, the plan holds where the leader stands next, or that it could
    /// not move).
    fn route_step(&mut self, pi: usize, hi: usize, run: bool, plan: &mut Plan) -> bool {
        if self.rivals.parties[pi].journey.is_none() {
            return false;
        }
        let h = &self.heroes[hi];
        let at = V3::at(h.pos, h.z);
        let pace = if run || h.stealthed {
            h.speed
        } else {
            h.speed * WALK_RATIO
        };
        let can = self.can_search();
        let rivals = &mut self.rivals;
        let (Some(nav), Some(j)) = (self.nav.as_mut(), rivals.parties[pi].journey.as_mut()) else {
            return false;
        };
        let searches = j.travel.needs_search();
        if searches && !can {
            plan.blocked = true;
            return true;
        }
        let walk = j.travel.step(nav, at, pace * DT);
        if searches && walk != Walk::Waiting {
            rivals.paths_left = rivals.paths_left.saturating_sub(1);
        }
        match walk {
            Walk::On(p) => {
                plan.to = Some(p);
                plan.gait = if run { Gait::Run } else { Gait::Walk };
            }
            Walk::Waiting | Walk::Blocked => plan.blocked = true,
            Walk::Arrived => {
                let to = j.to;
                if let RoutePlace::Poi(_) = to {
                    rivals.parties[pi].place = Some(to);
                }
                rivals.parties[pi].journey = None;
            }
        }
        true
    }

    /// Whether the rivals may begin another search this step: one of the `paths_per_step`, and
    /// the rivals' pool of floors still covers a whole leg's budget.
    fn can_search(&self) -> bool {
        let Some(nav) = self.nav.as_ref() else {
            return false;
        };
        let spent = nav
            .floors_looked_at()
            .saturating_sub(self.rivals.floors_start) as usize;
        self.rivals.paths_left > 0 && spent + RIVAL_BUDGET <= RIVAL_POOL
    }

    /// [`World::step_toward`] into the plan: where it stands next, or that it wanted to move and
    /// could not.
    fn walk_to(
        &mut self,
        player: PlayerId,
        hi: usize,
        goal: V2,
        goal_z: Option<f32>,
        run: bool,
        plan: &mut Plan,
    ) {
        match self.step_toward(player, hi, goal, goal_z, run) {
            Step::To(p) => plan.to = Some(p),
            Step::There => {}
            Step::Blocked => plan.blocked = true,
        }
        if plan.to.is_some() {
            plan.gait = if run { Gait::Run } else { Gait::Walk };
        }
    }

    /// The next spot on `player`'s way to `goal` at its pace (running, or in stealth, at its
    /// speed; walking at [`WALK_RATIO`] of it): straight when nothing stands between, else along
    /// a path, searched within the rivals' allowance this step.
    fn step_toward(
        &mut self,
        player: PlayerId,
        hi: usize,
        goal: V2,
        goal_z: Option<f32>,
        run: bool,
    ) -> Step {
        let Some(map) = self.map().cloned() else {
            return Step::Blocked;
        };
        let now = self.elapsed;
        let h = &self.heroes[hi];
        let from = V3::at(h.pos, h.z);
        let pace = if run || h.stealthed {
            h.speed
        } else {
            h.speed * WALK_RATIO
        };
        let step = pace * DT;
        if from.ground().distance(goal) <= THERE * 0.5 {
            return Step::There;
        }
        // The floor of the goal that the thing walked to stands on (its `goal_z`), else the one
        // nearest this rival: a column of two floors (a bank over a hollow) has one it can reach.
        let want = goal_z.unwrap_or(h.z);
        let to = V3::at(goal, map.floor_near(goal, want).unwrap_or(want));
        if map.walk_line(from, to) {
            if let Some(b) = self.rivals.body_mut(player) {
                b.way = Way::default();
            }
            let d = goal - from.ground();
            let n = from.ground() + d * (step.min(d.length()) / d.length().max(1e-6));
            return Step::To(V3::at(
                n,
                // The nearest floor under or over it: one that drops away is fallen to, so a rival
                // never hangs in the air above ground it can no longer stand on.
                map.floor_within(n, from.z, 1.0)
                    .or_else(|| map.floor_near(n, from.z))
                    .unwrap_or(from.z),
            ));
        }
        let (stale, waiting) = match self.rivals.body(player) {
            Some(b) => {
                let moved = b.way.toward.is_none_or(|t| t.distance(goal) > REPATH);
                (
                    b.way.path.is_none() || moved,
                    b.way.failed && !moved && now < b.way.retry_at,
                )
            }
            None => return Step::Blocked,
        };
        if waiting {
            return Step::Blocked;
        }
        if stale {
            if !self.can_search() {
                return Step::Blocked;
            }
            let Some(nav) = self.nav.as_mut() else {
                return Step::Blocked;
            };
            let found = nav.plan_path(from, to, false, RIVAL_BUDGET);
            let known = matches!(found, PathPlan::KnownNoPath);
            let path = match found {
                PathPlan::Found(p) => Some(p),
                PathPlan::NoPath | PathPlan::KnownNoPath => None,
                // The step's floors are spent: nothing was searched, ask again next step.
                PathPlan::Later => return Step::Blocked,
            };
            if !known {
                self.rivals.paths_left = self.rivals.paths_left.saturating_sub(1);
            }
            let Some(b) = self.rivals.body_mut(player) else {
                return Step::Blocked;
            };
            b.way = Way {
                failed: path.is_none(),
                retry_at: now + RETRY,
                path,
                next: 0,
                toward: Some(goal),
            };
        }
        let Some(b) = self.rivals.body_mut(player) else {
            return Step::Blocked;
        };
        match b.way.path.clone() {
            Some(path) => Step::To(walk_along(from, &path, &mut b.way.next, step)),
            None => Step::Blocked,
        }
    }

    /// Looting point `poi`: until the party is there, with it; there, each member to a container
    /// of its own, searched and emptied through the acts a player's client sends (open, take
    /// all, close).
    fn loot_plan(&mut self, pi: usize, player: PlayerId, hi: usize, poi: usize, plan: &mut Plan) {
        let content = self.content().clone();
        let centre = poi_at(&content, poi);
        if self.rivals.parties[pi].looting != Some(poi) {
            return self.go_with_party(pi, player, hi, centre, false, true, plan);
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
            return self.go_with_party(pi, player, hi, centre, false, false, plan);
        };
        let Some(c) = self.raid.containers.iter().find(|c| c.id == id) else {
            return;
        };
        let (at, searched, holds) = (c.at, c.searched, c.stacks.iter().any(Option::is_some));
        let here = self.heroes[hi].pos;
        plan.face = Some(at - here);
        if here.distance(at) > content.rules.rivals.loot_reach {
            self.walk_to(player, hi, at, None, false, plan);
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
            let lead = self
                .hero(self.rank_of(pi, player).0)
                .map_or(self.heroes[hi].pos, |h| h.pos);
            return self.go_with_party(pi, player, hi, lead, true, false, plan);
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
        let want = if stand_off > 0.0 {
            stand_off
        } else {
            MELEE_STAND
        };
        if d > want || !in_sight {
            self.walk_to(player, hi, tpos, Some(tz), true, plan);
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
            if let Some(k) = b.kit.iter().find(|k| {
                k.tags.contains(&Tag::Escape)
                    && ready(fight, k.ability, &content.abilities[k.ability])
            }) {
                plan.press = Some((k.slot, None, None));
            }
        }
        let spot = content
            .map
            .bounds
            .clamp(away_from(self.heroes[hi].pos, from, FLEE_RUN));
        self.walk_to(player, hi, spot, None, true, plan);
    }

    /// Out by exit `exit`: the leader along its route to the ring, then each member to a spot of
    /// its own inside it, and it stands.
    fn exit_plan(&mut self, pi: usize, player: PlayerId, hi: usize, exit: usize, plan: &mut Plan) {
        let content = self.content().clone();
        let e = &content.map.exits[exit];
        let (leader, rank) = self.rank_of(pi, player);
        let hunted = self.senses.hunted(player) || self.hurries(pi);
        if leader == player && self.heroes[hi].pos.distance(e.at) > e.radius {
            if self.waits_for_party(pi, player) {
                return;
            }
            if self.route_step(pi, hi, hunted, plan) {
                return;
            }
        }
        let spot = e.at + V2::from_angle(rank as f32 * std::f32::consts::TAU / 3.0, e.radius * 0.4);
        if self.heroes[hi].pos.distance(spot) <= THERE {
            return;
        }
        // A follower behind its leader keeps behind it until the ring is near.
        let to = match self
            .hero(leader)
            .filter(|l| leader != player && l.pos.distance(e.at) > e.radius)
        {
            Some(l) => self.follow_spot(hi, l.pos, l.facing, rank, content.rules.rivals.gap),
            None => spot,
        };
        let goal_z = self
            .hero(leader)
            .filter(|l| leader != player && l.pos.distance(e.at) > e.radius)
            .map(|l| l.z);
        self.walk_to(
            player,
            hi,
            to,
            goal_z,
            hunted || self.heroes[hi].pos.distance(to) > 8.0,
            plan,
        );
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
        let rolled = self
            .rivals
            .body(player)
            .map(|b| b.dodged.clone())
            .unwrap_or_default();
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
        let to = content
            .map
            .bounds
            .clamp(pos + way * content.rules.combat.dodge.distance);
        let map = self.map()?;
        let end = V3::at(to, map.floor_within(to, z, 1.0)?);
        map.walk_line(V3::at(pos, z), end).then_some(end)
    }

    /// Whether `at` lies in a telegraph that has not landed yet and would hurt `player`.
    fn in_danger(&self, player: PlayerId, at: V2) -> bool {
        self.telegraphs.iter().any(|t| {
            t.left > 0.0
                && t.by.is_none_or(|by| self.hostile(by, player))
                && t.shape.contains(t.at.ground(), t.facing, at)
        })
    }

    /// `player`'s input from `plan`, set as a client sets its own: the pose (where it walks, the
    /// way it faces), the gait it moves at, a new press at most every [`PRESS_EVERY`] (the last
    /// one repeated, as a client repeats it), Stealth pressed when it wants in and may, a dodge
    /// counted, and an act repeated until the host takes it. A rival that stepped out of a
    /// telegraph stays out until it has landed (the corrections' ruling 4): a step into one that
    /// has not is not taken.
    fn send(&mut self, player: PlayerId, hi: usize, mut plan: Plan) {
        let now = self.elapsed;
        let (pos, z, facing, stealthed, acted) = {
            let h = &self.heroes[hi];
            (h.pos, h.z, h.facing, h.stealthed, h.carry.acted)
        };
        if let Some(to) = plan.to {
            if !plan.dodge && !self.in_danger(player, pos) && self.in_danger(player, to.ground()) {
                plan.to = None;
            }
        }
        let may_stealth = plan.stealth && !stealthed && self.may_stealth(player);
        let to = plan.to.unwrap_or(V3::at(pos, z));
        // Always on a floor: a path's point can sit above the ground it crosses, and a rival that
        // hangs in the air has no floor to search a way from.
        let to = V3::at(
            to.ground(),
            self.map()
                .and_then(|m| m.floor_near(to.ground(), to.z))
                .unwrap_or(to.z),
        );
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
        b.pinned = if plan.blocked && plan.to.is_none() {
            b.pinned + DT
        } else {
            0.0
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
        };
        self.set_input(player, input);
    }
}

/// Point `poi`'s centre.
fn poi_at(content: &crate::Content, poi: usize) -> V2 {
    let p = &content.rivals.pois[poi];
    V2::new(p.at.0, p.at.1)
}

/// The yards a run of waypoints covers.
fn length(points: &[V2]) -> f32 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
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
