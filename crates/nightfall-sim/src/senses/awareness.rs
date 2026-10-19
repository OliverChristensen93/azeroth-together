//! What an enemy makes of the heroes (spec §5.3): a suspicion of each, 0 to 100, raised by seeing
//! and hearing them and drained by time, and the state that puts the enemy in. Unaware below
//! `suspicious`; wary ("?": it stops and looks) from there; checking (it goes to look) from
//! `check`, and still on its way while suspicion stays at `suspicious` or more; alerted ("!":
//! after its hero, shouting to its allies) at `alert`. An alerted enemy out of sight of its hero
//! `lose_after` seconds searches where it last saw them for `search_for` seconds once there, then
//! goes back unaware. A sleeper (spec §7.1) hears but does not see until alerted.
//!
//! Pure state: [`super::react`] tells it what the enemy saw and heard each step, and moves the
//! enemy as it says.

use serde::{Deserialize, Serialize};

use crate::{AwarenessRules, PlayerId, V3};

/// What an enemy shows a player over its head (spec §5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alert {
    /// "?": suspicious of you, or searching for you.
    Suspicious,
    /// "!": alerted to you.
    Alerted,
}

/// One enemy's mark for one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    pub enemy: u32,
    pub alert: Alert,
}

/// What an enemy made of one hero this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sensed {
    pub player: PlayerId,
    /// Noticed within its notice range: where the hero stands, and how fast suspicion fills a
    /// second.
    pub noticed: Option<(V3, f32)>,
    /// In plain sight (within the fog, in line of sight), noticed or not: how an alerted enemy
    /// keeps track of its hero.
    pub in_sight: Option<V3>,
    /// The floor a noise of theirs came from, the loudest heard this step.
    pub heard: Option<V3>,
}

/// Its suspicion of one hero, and the place to check for them: where they were last seen or heard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Suspicion {
    pub player: PlayerId,
    pub level: f32,
    pub place: V3,
}

/// An enemy's state of mind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum State {
    /// About its business.
    Calm,
    /// It stops and faces `place`.
    Wary { player: PlayerId, place: V3 },
    /// It goes to look at `place`.
    Checking { player: PlayerId, place: V3 },
    /// After `player`, last seen at `last_seen`, out of sight these `unseen` seconds.
    Alerted {
        player: PlayerId,
        last_seen: V3,
        unseen: f32,
    },
    /// It lost `player`: it goes to `place` and looks around, `left` seconds once there.
    Searching {
        player: PlayerId,
        place: V3,
        left: f32,
    },
}

/// A shout: allies in reach go after `player`, last seen at `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shout {
    pub player: PlayerId,
    pub at: V3,
}

/// One enemy's mind.
#[derive(Clone, Debug)]
pub(crate) struct Mind {
    /// Its front cone, degrees.
    pub cone: f32,
    /// Hears but does not see until alerted.
    pub sleeper: bool,
    /// Its suspicion of each hero met.
    pub of: Vec<Suspicion>,
    pub state: State,
    /// It has reached the place it is checking or searching (or found no way there).
    pub there: bool,
}

impl Mind {
    pub(crate) fn new(cone: f32, sleeper: bool) -> Self {
        Self {
            cone,
            sleeper,
            of: Vec::new(),
            state: State::Calm,
            there: false,
        }
    }

    /// Blind now: a sleeper that has not been alerted since it last lay down.
    pub(crate) fn blind(&self) -> bool {
        self.sleeper && !matches!(self.state, State::Alerted { .. } | State::Searching { .. })
    }

    /// Its suspicion of `player`'s hero.
    pub(crate) fn level(&self, player: PlayerId) -> f32 {
        self.of
            .iter()
            .find(|s| s.player == player)
            .map_or(0.0, |s| s.level)
    }

    pub(crate) fn is_alerted(&self) -> bool {
        matches!(self.state, State::Alerted { .. })
    }

    pub(crate) fn alerted_to(&self, player: PlayerId) -> bool {
        matches!(self.state, State::Alerted { player: p, .. } if p == player)
    }

    /// What it shows `player` (spec §5.5): "!" alerted to them, "?" searching for them or at least
    /// `suspicious` of them.
    pub(crate) fn mark(&self, player: PlayerId, r: &AwarenessRules) -> Option<Alert> {
        match self.state {
            State::Alerted { player: p, .. } if p == player => Some(Alert::Alerted),
            State::Searching { player: p, .. } if p == player => Some(Alert::Suspicious),
            _ if self.level(player) >= r.suspicious => Some(Alert::Suspicious),
            _ => None,
        }
    }

    fn entry(&mut self, player: PlayerId) -> &mut Suspicion {
        let i = match self.of.iter().position(|s| s.player == player) {
            Some(i) => i,
            None => {
                self.of.push(Suspicion {
                    player,
                    level: 0.0,
                    place: V3::default(),
                });
                self.of.len() - 1
            }
        };
        &mut self.of[i]
    }

    /// Alerted to `player`, last seen at `at`: by an ally's shout, or a hit (sub-project 4).
    pub(crate) fn alert(&mut self, player: PlayerId, at: V3, r: &AwarenessRules) {
        let e = self.entry(player);
        e.level = r.alert;
        e.place = at;
        self.state = State::Alerted {
            player,
            last_seen: at,
            unseen: 0.0,
        };
        self.there = false;
    }

    /// A scent of `player`'s hero at `place` (a tracker, spec §7.3): its suspicion of them held
    /// halfway from `check` to `alert`, so this step's drain leaves it checking, and the place to
    /// check moved to the scent, so it walks the trail. One already alerted keeps its chase.
    pub(crate) fn sniff(&mut self, player: PlayerId, place: V3, r: &AwarenessRules) {
        if self.is_alerted() {
            return;
        }
        let e = self.entry(player);
        e.level = e.level.max((r.check + r.alert) * 0.5);
        let moved = e.place.ground().distance(place.ground()) > 0.5;
        e.place = place;
        if let State::Checking { player: p, place: at } = &mut self.state {
            if *p == player {
                *at = place;
            }
        }
        if moved {
            self.there = false;
        }
    }

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

    /// Its thoughts end (it died): calm, and suspicious of nobody. It keeps its cone and whether
    /// it sleeps.
    pub(crate) fn calm(&mut self) {
        self.of.clear();
        self.state = State::Calm;
        self.there = false;
    }

    /// A hero left the run: nothing of them stays, and a state about them ends.
    pub(crate) fn forget(&mut self, player: PlayerId) {
        self.of.retain(|s| s.player != player);
        let about = match self.state {
            State::Calm => None,
            State::Wary { player: p, .. }
            | State::Checking { player: p, .. }
            | State::Alerted { player: p, .. }
            | State::Searching { player: p, .. } => Some(p),
        };
        if about == Some(player) {
            self.state = State::Calm;
            self.there = false;
        }
    }

    /// One step of thinking (spec §5.3): what it saw and heard of each hero raises its suspicion
    /// of them, neither drains it, and the state follows. The hero it is alerted to is held at
    /// `alert` while it is after them. Returns a shout the step it becomes alerted.
    pub(crate) fn think(
        &mut self,
        sensed: &[Sensed],
        dt: f32,
        r: &AwarenessRules,
    ) -> Option<Shout> {
        let blind = self.blind();
        for s in sensed {
            let chasing = self.alerted_to(s.player);
            if let State::Alerted {
                player,
                last_seen,
                unseen,
            } = &mut self.state
            {
                if *player == s.player {
                    *unseen += dt;
                    if let Some(at) = s.in_sight.or(s.noticed.map(|n| n.0)) {
                        *last_seen = at;
                        *unseen = 0.0;
                    }
                }
            }
            let e = self.entry(s.player);
            let mut fed = false;
            if let (false, Some((at, fill))) = (blind, s.noticed) {
                e.level += fill * dt;
                e.place = at;
                fed = true;
            }
            if let Some(at) = s.heard {
                e.level += r.noise;
                e.place = at;
                fed = true;
            }
            if chasing {
                e.level = r.alert;
                fed = true;
            }
            if !fed {
                e.level -= r.drain * dt;
            }
            // A level that is not a number (a fill or a noise gone wrong) is none at all: a clamp
            // would keep it.
            e.level = if e.level.is_finite() {
                e.level.clamp(0.0, r.alert)
            } else {
                0.0
            };
        }
        // Someone who is not there to be sensed (a hero who got out, and was heard once as they
        // went) is neither seen nor heard again: the suspicion of them drains like any other.
        for s in self
            .of
            .iter_mut()
            .filter(|s| !sensed.iter().any(|x| x.player == s.player))
        {
            s.level = (s.level - r.drain * dt).max(0.0);
        }
        if let State::Alerted {
            player,
            last_seen,
            unseen,
        } = self.state
        {
            if unseen >= r.lose_after {
                self.state = State::Searching {
                    player,
                    place: last_seen,
                    left: r.search_for,
                };
                self.there = false;
                self.entry(player).level = r.check;
            }
            return None;
        }
        // The hero it is most suspicious of; on a tie the lower player, so a run plays the same.
        let top = self
            .of
            .iter()
            .copied()
            .max_by(|a, b| a.level.total_cmp(&b.level).then(b.player.cmp(&a.player)));
        if let Some(t) = top.filter(|t| t.level >= r.alert) {
            self.state = State::Alerted {
                player: t.player,
                last_seen: t.place,
                unseen: 0.0,
            };
            self.there = false;
            return Some(Shout {
                player: t.player,
                at: t.place,
            });
        }
        if let State::Searching { left, .. } = &mut self.state {
            if self.there {
                *left -= dt;
            }
            if *left <= 0.0 {
                self.state = State::Calm;
                self.there = false;
                for s in &mut self.of {
                    s.level = 0.0;
                }
            }
            return None;
        }
        let checking = matches!(self.state, State::Checking { .. });
        let next = match top {
            Some(t) if t.level >= r.check || (checking && t.level >= r.suspicious) => {
                State::Checking {
                    player: t.player,
                    place: t.place,
                }
            }
            Some(t) if t.level >= r.suspicious => State::Wary {
                player: t.player,
                place: t.place,
            },
            _ => State::Calm,
        };
        // A check going on to the same place keeps its arrival; anything else starts afresh.
        let same = matches!(
            (self.state, next),
            (State::Checking { place: a, .. }, State::Checking { place: b, .. })
                if a.ground().distance(b.ground()) <= 1.0
        );
        if !same {
            self.there = false;
        }
        self.state = next;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DT;

    const AT: V3 = V3::new(10.0, 10.0, 0.0);
    const ELSEWHERE: V3 = V3::new(20.0, 5.0, 0.0);

    fn sees(player: PlayerId, at: V3, fill: f32) -> Sensed {
        Sensed {
            player,
            noticed: Some((at, fill)),
            in_sight: Some(at),
            heard: None,
        }
    }

    fn hears(player: PlayerId, at: V3) -> Sensed {
        Sensed {
            player,
            heard: Some(at),
            ..Sensed::default()
        }
    }

    fn nothing(player: PlayerId) -> Sensed {
        Sensed {
            player,
            ..Sensed::default()
        }
    }

    /// `secs` of thinking with the same senses each step; the shouts it gave.
    fn think_for(m: &mut Mind, sensed: &[Sensed], secs: f32, r: &AwarenessRules) -> Vec<Shout> {
        (0..(secs / DT).round() as usize)
            .filter_map(|_| m.think(sensed, DT, r))
            .collect()
    }

    #[test]
    fn seeing_fills_at_its_rate_and_a_noise_adds_its_share() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(0, AT, 30.0)], 1.0, &r);
        assert!((m.level(0) - 30.0).abs() < 0.01, "{}", m.level(0));
        assert_eq!(m.state, State::Calm);
        m.think(&[hears(0, ELSEWHERE)], DT, &r);
        assert!((m.level(0) - 65.0).abs() < 0.01, "{}", m.level(0));
        assert_eq!(
            m.state,
            State::Checking {
                player: 0,
                place: ELSEWHERE
            }
        );
    }

    #[test]
    fn without_sight_or_noise_suspicion_drains() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(0, AT, 50.0)], 1.0, &r);
        think_for(&mut m, &[nothing(0)], 0.5, &r);
        assert!((m.level(0) - 45.0).abs() < 0.01);
        assert!(matches!(m.state, State::Wary { player: 0, .. }));
        think_for(&mut m, &[nothing(0)], 1.0, &r);
        assert!((m.level(0) - 35.0).abs() < 0.01);
        assert_eq!(m.state, State::Calm);
    }

    #[test]
    fn the_states_follow_the_thresholds_and_it_shouts_once() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(&mut m, &[sees(0, AT, 60.0)], 0.7, &r);
        assert_eq!(
            m.state,
            State::Wary {
                player: 0,
                place: AT
            }
        );
        think_for(&mut m, &[sees(0, AT, 60.0)], 0.4, &r);
        assert_eq!(
            m.state,
            State::Checking {
                player: 0,
                place: AT
            }
        );
        let shouts = think_for(&mut m, &[sees(0, AT, 60.0)], 1.0, &r);
        assert_eq!(shouts, vec![Shout { player: 0, at: AT }]);
        assert!(m.alerted_to(0));
    }

    #[test]
    fn a_check_keeps_going_until_suspicion_falls_below_suspicious() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.think(&[hears(0, AT)], DT, &r);
        m.think(&[hears(0, AT)], DT, &r);
        assert!(matches!(m.state, State::Checking { .. }));
        // 70 drains below `check` within a second, but the check goes on down to `suspicious`.
        think_for(&mut m, &[nothing(0)], 2.5, &r);
        assert!(matches!(m.state, State::Checking { .. }));
        think_for(&mut m, &[nothing(0)], 1.0, &r);
        assert_eq!(m.state, State::Calm);
    }

    #[test]
    fn an_alerted_enemy_holds_while_it_sees_and_searches_after_losing_sight() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        think_for(&mut m, &[sees(0, ELSEWHERE, 10.0)], 1.0, &r);
        think_for(&mut m, &[nothing(0)], 4.9, &r);
        assert!(m.alerted_to(0));
        assert_eq!(m.level(0), 100.0);
        think_for(&mut m, &[nothing(0)], 0.2, &r);
        assert_eq!(
            m.state,
            State::Searching {
                player: 0,
                place: ELSEWHERE,
                left: 15.0
            }
        );
        assert_eq!(m.mark(0, &r), Some(Alert::Suspicious));
    }

    #[test]
    fn a_search_counts_only_once_there_and_ends_unaware() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        think_for(&mut m, &[nothing(0)], 5.1, &r);
        think_for(&mut m, &[nothing(0)], 20.0, &r);
        assert!(matches!(m.state, State::Searching { .. }));
        m.there = true;
        think_for(&mut m, &[nothing(0)], 14.9, &r);
        assert!(matches!(m.state, State::Searching { .. }));
        think_for(&mut m, &[nothing(0)], 0.2, &r);
        assert_eq!(m.state, State::Calm);
        assert_eq!(m.level(0), 0.0);
        assert_eq!(m.mark(0, &r), None);
    }

    #[test]
    fn seeing_its_hero_during_a_search_alerts_it_again() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        think_for(&mut m, &[nothing(0)], 5.1, &r);
        let shouts = think_for(&mut m, &[sees(0, ELSEWHERE, 60.0)], 1.0, &r);
        assert_eq!(shouts.len(), 1);
        assert!(m.alerted_to(0));
    }

    #[test]
    fn a_sleeper_hears_but_does_not_see_until_alerted() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, true);
        think_for(&mut m, &[sees(0, AT, 60.0)], 3.0, &r);
        assert_eq!(m.level(0), 0.0);
        assert!(m.blind());
        assert_eq!(m.think(&[hears(0, AT)], DT, &r), None);
        assert_eq!(m.think(&[hears(0, AT)], DT, &r), None);
        assert_eq!(
            m.think(&[hears(0, AT)], DT, &r),
            Some(Shout { player: 0, at: AT })
        );
        assert!(!m.blind());
        m.think(&[sees(0, ELSEWHERE, 60.0)], DT, &r);
        assert!(matches!(
            m.state,
            State::Alerted { last_seen, .. } if last_seen == ELSEWHERE
        ));
    }

    #[test]
    fn marks_say_what_it_thinks_of_each_hero() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        assert_eq!(m.mark(0, &r), Some(Alert::Alerted));
        assert_eq!(m.mark(1, &r), None);
        think_for(
            &mut m,
            &[sees(0, AT, 10.0), sees(1, ELSEWHERE, 45.0)],
            1.0,
            &r,
        );
        assert_eq!(m.mark(1, &r), Some(Alert::Suspicious));
        assert!(m.alerted_to(0));
    }

    #[test]
    fn ties_go_to_the_lower_player() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        think_for(
            &mut m,
            &[sees(1, ELSEWHERE, 45.0), sees(0, AT, 45.0)],
            1.0,
            &r,
        );
        assert_eq!(
            m.state,
            State::Wary {
                player: 0,
                place: AT
            }
        );
    }

    #[test]
    fn a_forgotten_hero_leaves_no_trace() {
        let r = AwarenessRules::default();
        let mut m = Mind::new(120.0, false);
        m.alert(0, AT, &r);
        m.forget(0);
        assert_eq!(m.state, State::Calm);
        assert_eq!(m.level(0), 0.0);
        assert_eq!(m.mark(0, &r), None);
    }

    #[test]
    fn suspicion_stays_within_zero_and_alert_whatever_is_fed_it() {
        let r = AwarenessRules::default();
        let in_range = |m: &Mind| (0.0..=r.alert).contains(&m.level(0));
        // A fill and a noise far past the top.
        let mut m = Mind::new(120.0, false);
        m.think(&[sees(0, AT, 1e9)], DT, &r);
        assert!(in_range(&m) && m.level(0) == r.alert, "{}", m.level(0));
        let mut m = Mind::new(120.0, false);
        for _ in 0..5 {
            m.think(&[hears(0, AT)], DT, &r);
        }
        assert!(in_range(&m), "{}", m.level(0));
        // A drain that overshoots the bottom.
        let mut m = Mind::new(120.0, false);
        let tough = AwarenessRules {
            drain: 1e9,
            ..AwarenessRules::default()
        };
        m.think(&[sees(0, AT, 10.0)], DT, &tough);
        m.think(&[nothing(0)], DT, &tough);
        assert_eq!(m.level(0), 0.0);
        // Numbers that are not: no level that is not a number is kept.
        for fill in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut m = Mind::new(120.0, false);
            m.think(&[sees(0, AT, fill)], DT, &r);
            assert!(in_range(&m), "fill {fill} left {}", m.level(0));
            m.think(&[nothing(0)], DT, &r);
            assert!(in_range(&m), "fill {fill} left {}", m.level(0));
        }
    }
}
