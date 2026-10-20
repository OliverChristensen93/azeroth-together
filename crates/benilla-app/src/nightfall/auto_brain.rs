//! What `NIGHTFALL_AUTOPLAY` decides: when to sneak, what to take and when to head for a way out.
//!
//! The decisions are pure functions of [`Known`], what the hero itself can know: where it stands,
//! how full and heavy its bags are, how late the night is, which exits are open and which enemies
//! are near and whether they have noticed it. The system [`plan`] gathers that from the world once
//! a frame into [`AutoPlan`]; the walk, the looting and the Stealth presses
//! ([`super::exits::autoexit`], [`super::loot_view::autoloot`], [`super::stance::auto_stealth`])
//! read it. Nothing here changes the world: the hero acts through the same presses and acts a
//! player makes.

use bevy::prelude::*;
use nightfall_sim::{Content, Stack, V2};

use super::run::{autoplay, Sim};

/// How near (yards) an enemy that has not noticed the hero makes a rogue sneak.
pub(super) const SNEAK_NEAR: f32 = 35.0;

/// An enemy the hero can see, and whether it has noticed the hero at all (a mark "?" or "!").
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Foe {
    pub at: V2,
    /// Suspicious of the hero, or alerted to it.
    pub aware: bool,
}

/// What the hero knows when it decides.
#[derive(Clone, Debug)]
pub(super) struct Known {
    pub rogue: bool,
    pub at: V2,
    /// Backpack slots holding something, and all of them.
    pub pack_used: usize,
    pub pack_size: usize,
    /// The weight carried, and the weight at which the burden is whole (`weight_full`).
    pub load: f32,
    pub weight_cap: f32,
    /// The night is in its later half.
    pub late: bool,
    /// The middle of every open exit.
    pub exits: Vec<V2>,
    pub foes: Vec<Foe>,
}

/// What the hero does about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Plan {
    /// Be in stealth (press Stealth until it is).
    pub sneak: bool,
    /// Walk to this exit and leave: the nearest open one. `None` while there is more to do.
    pub leave: Option<V2>,
}

/// The hero's plan, written each frame by [`plan`].
#[derive(Resource, Default)]
pub(super) struct AutoPlan(pub Plan);

/// Whether the bags hold at least half of their slots.
pub(super) fn bags_half_full(used: usize, size: usize) -> bool {
    size > 0 && used * 2 >= size
}

/// Whether the night is in its later half: the stage `stage` of `stages`.
pub(super) fn night_is_late(stage: usize, stages: usize) -> bool {
    stages > 0 && stage * 2 >= stages
}

/// Whether a rogue should sneak: some enemy within [`SNEAK_NEAR`] has not noticed it.
pub(super) fn should_sneak(rogue: bool, at: V2, foes: &[Foe]) -> bool {
    rogue
        && foes
            .iter()
            .any(|f| !f.aware && f.at.distance(at) <= SNEAK_NEAR)
}

/// The open exit nearest `from`.
pub(super) fn nearest_exit(exits: &[V2], from: V2) -> Option<V2> {
    exits
        .iter()
        .copied()
        .min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)))
}

/// Whether the hero has had enough: bags half full, the burden whole, or the night late.
pub(super) fn has_had_enough(k: &Known) -> bool {
    bags_half_full(k.pack_used, k.pack_size) || k.load >= k.weight_cap || k.late
}

/// The decision, from what the hero knows.
pub(super) fn decide(k: &Known) -> Plan {
    Plan {
        sneak: should_sneak(k.rogue, k.at, &k.foes),
        leave: if has_had_enough(k) {
            nearest_exit(&k.exits, k.at)
        } else {
            None
        },
    }
}

/// The slots of an open container to take, in order: each stack that still fits under `cap` once
/// the ones taken before it are counted, while a backpack slot is free (`free_slots`). A stack
/// that is too heavy is left; a lighter one after it may still go.
pub(super) fn takes(
    stacks: &[Option<Stack>],
    load: f32,
    cap: f32,
    free_slots: usize,
    weight_of: impl Fn(&Stack) -> f32,
) -> Vec<u16> {
    let (mut load, mut free) = (load, free_slots);
    let mut out = Vec::new();
    for (i, stack) in stacks.iter().enumerate() {
        let Some(s) = stack else { continue };
        let w = weight_of(s);
        if free > 0 && load + w <= cap {
            load += w;
            free -= 1;
            out.push(i as u16);
        }
    }
    out
}

/// The weight of a whole stack.
pub(super) fn stack_weight(s: &Stack, content: &Content) -> f32 {
    content
        .items
        .get(usize::from(s.item))
        .filter(|d| d.weight.is_finite())
        .map_or(0.0, |d| d.weight * f32::from(s.count))
}

/// `NIGHTFALL_AUTOPLAY`: work out [`AutoPlan`] from what the hero knows.
pub(super) fn plan(sim: Res<Sim>, mut out: ResMut<AutoPlan>) {
    if !autoplay() {
        return;
    }
    gather(&sim, &mut out);
}

/// [`plan`]'s work: a hero that is not up (down, dead, gone, or its pack gone) has no plan, so
/// nothing walks it to an exit or presses Stealth for it. (Held by a stun or a root it still
/// plans: the client has no accessor for a hold, and the walk and presses are refused by the
/// host anyway.)
fn gather(sim: &Sim, out: &mut AutoPlan) {
    let content = super::content();
    let hero = sim.world.hero(sim.me).filter(|h| h.fight.up());
    let (Some(hero), Some(inv)) = (hero, sim.world.pack(sim.me)) else {
        if out.0 != Plan::default() {
            out.0 = Plan::default();
        }
        return;
    };
    let loot = sim.world.loot_view(Some(sim.me));
    let stages = content.rules.night.stages.len();
    let known = Known {
        rogue: content
            .classes
            .get(hero.class)
            .is_some_and(|c| c.stealth),
        at: hero.pos,
        pack_used: inv.pack.iter().filter(|s| s.is_some()).count(),
        pack_size: inv.pack.len(),
        load: hero.carry.load,
        weight_cap: content.rules.run.weight_full,
        late: night_is_late(usize::from(sim.world.night_view().stage), stages),
        exits: loot
            .exits
            .iter()
            .filter(|e| e.open)
            .filter_map(|e| content.map.exits.get(usize::from(e.exit)))
            .map(|d| d.at)
            .collect(),
        foes: sim
            .world
            .enemies
            .iter()
            // Only those the hero sees: a player does not know of the ones behind walls.
            .filter(|e| sim.seen.contains(&e.id))
            .map(|e| Foe {
                at: e.pos,
                aware: sim.marks.iter().any(|m| m.enemy == e.id),
            })
            .collect(),
    };
    let decided = decide(&known);
    if out.0 != decided {
        out.0 = decided;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Known {
        Known {
            rogue: true,
            at: V2::new(0.0, 0.0),
            pack_used: 1,
            pack_size: 16,
            load: 5.0,
            weight_cap: 60.0,
            late: false,
            exits: vec![V2::new(100.0, 0.0), V2::new(30.0, 40.0)],
            foes: vec![],
        }
    }

    fn foe(x: f32, aware: bool) -> Foe {
        Foe {
            at: V2::new(x, 0.0),
            aware,
        }
    }

    fn stack(item: u16) -> Stack {
        Stack::new(usize::from(item), 1)
    }

    #[test]
    fn a_hero_with_room_and_time_keeps_looting() {
        assert_eq!(decide(&known()).leave, None);
    }

    #[test]
    fn half_full_bags_send_it_to_the_nearest_open_exit() {
        let mut k = known();
        k.pack_used = 8;
        assert_eq!(decide(&k).leave, Some(V2::new(30.0, 40.0)));
        k.pack_used = 7;
        assert_eq!(decide(&k).leave, None);
    }

    #[test]
    fn a_late_night_or_a_whole_burden_sends_it_out_too() {
        let mut k = known();
        k.late = true;
        assert_eq!(decide(&k).leave, Some(V2::new(30.0, 40.0)));
        k.late = false;
        k.load = 60.0;
        assert_eq!(decide(&k).leave, Some(V2::new(30.0, 40.0)));
    }

    #[test]
    fn with_no_open_exit_there_is_nowhere_to_go() {
        let mut k = known();
        k.late = true;
        k.exits.clear();
        assert_eq!(decide(&k).leave, None);
    }

    #[test]
    fn the_night_is_late_from_its_middle_stage() {
        assert!(!night_is_late(1, 5));
        assert!(!night_is_late(2, 5));
        assert!(night_is_late(3, 5));
        assert!(!night_is_late(0, 0));
    }

    #[test]
    fn only_a_rogue_sneaks_and_only_near_an_unaware_enemy() {
        let mut k = known();
        k.foes = vec![foe(20.0, false)];
        assert!(decide(&k).sneak);
        k.rogue = false;
        assert!(!decide(&k).sneak);
        k.rogue = true;
        k.foes = vec![foe(20.0, true)];
        assert!(
            !decide(&k).sneak,
            "one that has noticed it is past sneaking"
        );
        k.foes = vec![foe(SNEAK_NEAR + 1.0, false)];
        assert!(!decide(&k).sneak, "too far to matter");
        k.foes = vec![];
        assert!(!decide(&k).sneak);
    }

    #[test]
    fn it_takes_what_fits_under_its_weight_and_leaves_the_rest() {
        let stacks = [Some(stack(0)), None, Some(stack(1)), Some(stack(2))];
        let weight = |s: &Stack| match s.item {
            0 => 10.0,
            1 => 50.0,
            _ => 5.0,
        };
        // 5 + 10 + 50 would be 65 over 60: the heavy one stays, the light one after it goes.
        assert_eq!(takes(&stacks, 5.0, 60.0, 8, weight), vec![0, 3]);
        // A full pack takes nothing, a pack with one slot free takes the first that fits.
        assert!(takes(&stacks, 5.0, 60.0, 0, weight).is_empty());
        assert_eq!(takes(&stacks, 5.0, 60.0, 1, weight), vec![0]);
    }

    /// A hero in a late night with an exit open has had enough, and walks out; the same hero
    /// downed has no plan at all.
    #[test]
    fn a_hero_that_is_down_has_no_plan() {
        use nightfall_sim::{RaidRoll, World};
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let content = super::super::content();
        let mut world = World::new(content.clone());
        world.begin_raid(RaidRoll::standard(&content, 1)).unwrap();
        world.add_hero(0, "rogue", "Us", 1, 0).unwrap();
        world.give_loadout(0, &nightfall_sim::Loadout::default());
        world.elapsed = 100_000.0;
        world.step();
        let mut sim = Sim::for_test(world);
        let mut out = AutoPlan::default();
        gather(&sim, &mut out);
        assert!(out.0.leave.is_some(), "late and an exit open: {:?}", out.0);
        sim.world.hurt_hero(0, 1.0e9);
        sim.world.step();
        assert!(!sim.world.hero(0).unwrap().fight.up());
        gather(&sim, &mut out);
        assert_eq!(out.0, Plan::default());
    }
}
