//! A rival party's goals (spec §7.4): visit points of interest for loot, fight what comes on
//! the way, then extract or hunt the heroes as its aggression says. Like Single Player Tarkov's
//! questing bots (§1.1), a party has an errand of its own and drops it when the night gets in
//! the way: a hero it is alerted to is fought by a bold party and slipped away from by a
//! cautious one (unless the hero is too near, or struck first); monsters are fought; a wary
//! member stops the party, a checking one leads it to look; a party too hurt breaks off. Pure:
//! the bots build a [`PartyView`] each step and walk where the goal says.

use crate::{PoiDef, RivalRules, V2, V3};

/// WoW's walk speed as a share of its run speed (the stock 2.5 against 7), for a calm party.
pub(crate) const WALK_RATIO: f32 = 2.5 / 7.0;
/// Yards from a path's point at which it counts as reached.
const ARRIVE: f32 = 0.4;
/// Below this roll a party goes to its second-best point, so parties of one map vary.
const SECOND_BEST: f32 = 0.25;
/// Yards added to every point's distance when weighing it: a near poor point should not always
/// beat a far rich one.
const POI_NEAR: f32 = 50.0;

/// What a party is about.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Goal {
    /// Nothing to do yet (no exit open): it stands.
    #[default]
    Wait,
    /// To point of interest `poi` (an index into the rivals' points), and its loot.
    Loot { poi: usize },
    /// A member is wary: the party stops and faces `face`.
    Hold { face: V2 },
    /// A member is checking: the party goes to look at `at`.
    Check { at: V3 },
    /// In a fight: each member at its own target.
    Fight,
    /// Breaking off from `from`, for `left` seconds more.
    Flee { from: V2, left: f32 },
    /// After the heroes, toward `at`, for `left` seconds more.
    Hunt { at: V3, left: f32 },
    /// Out by exit `exit` (an index into the map's exits).
    Extract { exit: usize },
}

/// A hero the party is alerted to: where it was last seen, and how far from the leader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FoeSeen {
    pub(crate) at: V3,
    pub(crate) distance: f32,
}

/// What a party knows this step, for [`decide`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PartyView {
    pub(crate) aggression: f32,
    /// The weakest member's share of health.
    pub(crate) health: f32,
    /// A member's bag is full, or its load past `full_load`.
    pub(crate) full: bool,
    /// The night has reached `leave_by`.
    pub(crate) late: bool,
    /// Points of interest still to visit.
    pub(crate) pois_left: u8,
    /// The point being looted now, while it holds loot.
    pub(crate) looting: Option<usize>,
    /// The next point to go to.
    pub(crate) next_poi: Option<usize>,
    pub(crate) foe: Option<FoeSeen>,
    /// A hero has struck one of them this raid.
    pub(crate) hurt_by_hero: bool,
    /// An enemy is after one of them, or guards the point being looted.
    pub(crate) monsters: bool,
    /// A member is wary of a place (`false`) or checking it (`true`).
    pub(crate) wary: Option<(V3, bool)>,
    /// Where a hero was last heard or noticed, for a hunt.
    pub(crate) lead: Option<V3>,
    /// The party has hunted already this raid.
    pub(crate) hunted: bool,
    /// The nearest exit open now.
    pub(crate) exit: Option<usize>,
}

/// The party's goal this step, from the one it had and what it knows (see the module).
pub(crate) fn decide(now: Goal, v: &PartyView, r: &RivalRules) -> Goal {
    // A flight runs its course: a party that turns round at once is caught.
    if let Goal::Flee { left, .. } = now {
        if left > 0.0 {
            return now;
        }
    }
    if let Some(f) = v.foe {
        let from = f.at.ground();
        if v.health < r.flee_health {
            return Goal::Flee {
                from,
                left: r.flee_for,
            };
        }
        if v.aggression >= r.engage_at || v.hurt_by_hero || f.distance <= r.avoid_within {
            return Goal::Fight;
        }
        return Goal::Flee {
            from,
            left: r.flee_for,
        };
    }
    if v.monsters {
        return Goal::Fight;
    }
    if let Some((at, checking)) = v.wary {
        return if checking {
            Goal::Check { at }
        } else {
            Goal::Hold { face: at.ground() }
        };
    }
    let done = v.full || v.late || v.pois_left == 0;
    if !done {
        if let Some(poi) = v.looting.or(v.next_poi) {
            return Goal::Loot { poi };
        }
    }
    if v.aggression >= r.hunt_at && v.health >= r.rest_below && !v.full {
        match now {
            Goal::Hunt { at, left } if left > 0.0 => {
                return Goal::Hunt {
                    at: v.lead.unwrap_or(at),
                    left,
                };
            }
            _ if !v.hunted => {
                if let Some(at) = v.lead {
                    return Goal::Hunt {
                        at,
                        left: r.hunt_for,
                    };
                }
            }
            _ => {}
        }
    }
    v.exit.map_or(Goal::Wait, |exit| Goal::Extract { exit })
}

/// The point of interest a party at `at` goes to next, of those not in `skip`: the best by
/// worth over distance (plus [`POI_NEAR`]); a `roll` (0 to 1) under [`SECOND_BEST`] takes the
/// second best instead. Ties go to the lower index.
pub(crate) fn next_poi(pois: &[PoiDef], skip: &[usize], at: V2, roll: f32) -> Option<usize> {
    let mut scored: Vec<(f32, usize)> = pois
        .iter()
        .enumerate()
        .filter(|(i, _)| !skip.contains(i))
        .map(|(i, p)| {
            (
                p.worth / (at.distance(V2::new(p.at.0, p.at.1)) + POI_NEAR),
                i,
            )
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    match scored.len() {
        0 => None,
        1 => Some(scored[0].1),
        _ => Some(scored[usize::from(roll < SECOND_BEST)].1),
    }
}

/// Where member `rank` (1 and 2; the leader is 0) walks: `gap` yards behind its leader, one to
/// the left and one to the right.
pub(crate) fn formation_spot(leader: V2, facing: V2, rank: usize, gap: f32) -> V2 {
    let f = match facing.normalize_or_zero() {
        f if f == V2::ZERO => V2::new(1.0, 0.0),
        f => f,
    };
    let left = V2::new(-f.y, f.x);
    let side = if rank % 2 == 1 { 1.0 } else { -1.0 };
    let back = gap * rank.div_ceil(2) as f32;
    leader + f * -back + left * (side * gap * 0.6)
}

/// `step` yards along `path` from `from`: past each point reached (within [`ARRIVE`]), `next`
/// counting them; at its end it stays where it is. The height is the point walked toward.
pub(crate) fn walk_along(from: V3, path: &[V3], next: &mut usize, step: f32) -> V3 {
    let mut at = from.ground();
    let mut z = from.z;
    let mut left = step;
    while let Some(p) = path.get(*next) {
        let to = p.ground();
        let d = at.distance(to);
        z = p.z;
        if d <= left.max(ARRIVE) {
            at = to;
            left -= d;
            *next += 1;
            if left <= 0.0 {
                break;
            }
            continue;
        }
        at = at + (to - at) * (left / d);
        break;
    }
    V3::at(at, z)
}

/// The spot `distance` yards from `at`, straight away from `from` (any way off when on it).
pub(crate) fn away_from(at: V2, from: V2, distance: f32) -> V2 {
    let way = match (at - from).normalize_or_zero() {
        w if w == V2::ZERO => V2::new(1.0, 0.0),
        w => w,
    };
    at + way * distance
}
#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> RivalRules {
        RivalRules::default()
    }

    /// A healthy party with points left and nothing about.
    fn calm(aggression: f32) -> PartyView {
        PartyView {
            aggression,
            health: 1.0,
            pois_left: 2,
            next_poi: Some(1),
            exit: Some(0),
            ..PartyView::default()
        }
    }

    fn foe(distance: f32) -> Option<FoeSeen> {
        Some(FoeSeen {
            at: V3::new(distance, 0.0, 0.0),
            distance,
        })
    }

    #[test]
    fn a_cautious_party_slips_away_from_a_hero_and_a_bold_one_fights() {
        let shy = PartyView {
            foe: foe(30.0),
            ..calm(0.2)
        };
        assert!(matches!(decide(Goal::Wait, &shy, &r()), Goal::Flee { .. }));
        let bold = PartyView {
            foe: foe(30.0),
            ..calm(0.8)
        };
        assert_eq!(decide(Goal::Wait, &bold, &r()), Goal::Fight);
        let cornered = PartyView {
            foe: foe(5.0),
            ..calm(0.2)
        };
        assert_eq!(decide(Goal::Wait, &cornered, &r()), Goal::Fight);
        let struck = PartyView {
            foe: foe(30.0),
            hurt_by_hero: true,
            ..calm(0.2)
        };
        assert_eq!(decide(Goal::Wait, &struck, &r()), Goal::Fight);
    }

    #[test]
    fn a_hurt_party_flees_and_a_flight_runs_its_course() {
        let hurt = PartyView {
            foe: foe(10.0),
            health: 0.2,
            ..calm(0.9)
        };
        let Goal::Flee { left, .. } = decide(Goal::Fight, &hurt, &r()) else {
            panic!("a party this hurt breaks off");
        };
        assert_eq!(left, r().flee_for);
        let fleeing = Goal::Flee {
            from: V2::ZERO,
            left: 3.0,
        };
        assert_eq!(decide(fleeing, &calm(0.9), &r()), fleeing);
        let over = Goal::Flee {
            from: V2::ZERO,
            left: 0.0,
        };
        assert_eq!(decide(over, &calm(0.9), &r()), Goal::Loot { poi: 1 });
    }

    #[test]
    fn monsters_are_fought_and_a_wary_party_stops_or_checks() {
        let beset = PartyView {
            monsters: true,
            ..calm(0.1)
        };
        assert_eq!(decide(Goal::Loot { poi: 1 }, &beset, &r()), Goal::Fight);
        let at = V3::new(4.0, 5.0, 0.0);
        let wary = PartyView {
            wary: Some((at, false)),
            ..calm(0.1)
        };
        assert_eq!(
            decide(Goal::Loot { poi: 1 }, &wary, &r()),
            Goal::Hold { face: at.ground() }
        );
        let checking = PartyView {
            wary: Some((at, true)),
            ..calm(0.1)
        };
        assert_eq!(
            decide(Goal::Loot { poi: 1 }, &checking, &r()),
            Goal::Check { at }
        );
    }

    #[test]
    fn a_party_loots_until_it_is_full_late_or_done() {
        let looting = PartyView {
            looting: Some(0),
            ..calm(0.1)
        };
        assert_eq!(decide(Goal::Wait, &looting, &r()), Goal::Loot { poi: 0 });
        assert_eq!(decide(Goal::Wait, &calm(0.1), &r()), Goal::Loot { poi: 1 });
        for done in [
            PartyView {
                full: true,
                ..calm(0.1)
            },
            PartyView {
                late: true,
                ..calm(0.1)
            },
            PartyView {
                pois_left: 0,
                ..calm(0.1)
            },
            PartyView {
                next_poi: None,
                ..calm(0.1)
            },
        ] {
            assert_eq!(decide(Goal::Wait, &done, &r()), Goal::Extract { exit: 0 });
        }
    }

    #[test]
    fn a_bold_party_hunts_once_then_leaves() {
        let lead = Some(V3::new(50.0, 50.0, 0.0));
        let done = PartyView {
            pois_left: 0,
            lead,
            ..calm(0.9)
        };
        assert_eq!(
            decide(Goal::Wait, &done, &r()),
            Goal::Hunt {
                at: lead.unwrap(),
                left: r().hunt_for
            }
        );
        let newer = V3::new(60.0, 40.0, 0.0);
        let on = Goal::Hunt {
            at: lead.unwrap(),
            left: 10.0,
        };
        let heard = PartyView {
            lead: Some(newer),
            hunted: true,
            ..done
        };
        assert_eq!(
            decide(on, &heard, &r()),
            Goal::Hunt {
                at: newer,
                left: 10.0
            }
        );
        let spent = Goal::Hunt {
            at: newer,
            left: 0.0,
        };
        assert_eq!(decide(spent, &heard, &r()), Goal::Extract { exit: 0 });
        let no_lead = PartyView { lead: None, ..done };
        assert_eq!(
            decide(Goal::Wait, &no_lead, &r()),
            Goal::Extract { exit: 0 }
        );
        let weak = PartyView {
            health: 0.5,
            ..done
        };
        assert_eq!(decide(Goal::Wait, &weak, &r()), Goal::Extract { exit: 0 });
        let shy = PartyView {
            aggression: 0.3,
            ..done
        };
        assert_eq!(decide(Goal::Wait, &shy, &r()), Goal::Extract { exit: 0 });
    }

    #[test]
    fn with_no_exit_open_a_done_party_waits() {
        let stuck = PartyView {
            pois_left: 0,
            exit: None,
            ..calm(0.1)
        };
        assert_eq!(decide(Goal::Wait, &stuck, &r()), Goal::Wait);
    }

    fn poi(x: f32, worth: f32) -> PoiDef {
        PoiDef {
            key: format!("p{x}"),
            name: "P".into(),
            at: (x, 0.0),
            radius: 5.0,
            worth,
        }
    }

    #[test]
    fn the_next_point_is_the_best_by_worth_and_distance_not_yet_seen() {
        let pois = [poi(10.0, 1.0), poi(40.0, 3.0), poi(100.0, 1.0)];
        // 1/60 against 3/90 against 1/150: the rich one 40 yards off.
        assert_eq!(next_poi(&pois, &[], V2::ZERO, 0.9), Some(1));
        assert_eq!(next_poi(&pois, &[1], V2::ZERO, 0.9), Some(0));
        assert_eq!(
            next_poi(&pois, &[], V2::ZERO, 0.1),
            Some(0),
            "the dice take the second"
        );
        assert_eq!(next_poi(&pois, &[0, 1, 2], V2::ZERO, 0.9), None);
        assert_eq!(next_poi(&pois, &[0, 1], V2::ZERO, 0.1), Some(2));
    }

    #[test]
    fn followers_walk_behind_their_leader_left_and_right() {
        let leader = V2::new(10.0, 10.0);
        let east = V2::new(1.0, 0.0);
        let a = formation_spot(leader, east, 1, 2.5);
        let b = formation_spot(leader, east, 2, 2.5);
        assert!(a.x < leader.x && b.x < leader.x, "behind");
        assert!(a.y > leader.y && b.y < leader.y, "one to each side");
        assert!((a.distance(leader) - b.distance(leader)).abs() < 1e-4);
    }

    #[test]
    fn walking_a_path_turns_its_corners_and_stops_at_its_end() {
        let path = [V3::new(4.0, 0.0, 0.0), V3::new(4.0, 4.0, 1.0)];
        let mut next = 0;
        let a = walk_along(V3::new(0.0, 0.0, 0.0), &path, &mut next, 3.0);
        assert!((a.x - 3.0).abs() < 1e-4 && a.y.abs() < 1e-4);
        assert_eq!(next, 0);
        let b = walk_along(a, &path, &mut next, 3.0);
        assert!(
            (b.x - 4.0).abs() < 1e-4 && (b.y - 2.0).abs() < 1e-4,
            "round the corner"
        );
        assert_eq!((next, b.z), (1, 1.0));
        let c = walk_along(b, &path, &mut next, 10.0);
        assert_eq!((c.ground(), next), (V2::new(4.0, 4.0), 2));
        let d = walk_along(c, &path, &mut next, 10.0);
        assert_eq!(d, c, "at the end it stays");
    }

    #[test]
    fn away_is_straight_off_from_the_threat() {
        let p = away_from(V2::new(10.0, 0.0), V2::ZERO, 20.0);
        assert!((p.x - 30.0).abs() < 1e-4 && p.y.abs() < 1e-4);
        let q = away_from(V2::ZERO, V2::ZERO, 5.0);
        assert!((q.length() - 5.0).abs() < 1e-4, "on top of it, any way off");
    }
}
