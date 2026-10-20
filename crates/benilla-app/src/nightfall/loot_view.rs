//! The raid's containers drawn and searched (spec §8.2): a chest, crate or grave as its WoW model,
//! open once searched; things dropped on the ground; our own remains from the last raid. A
//! right-click on one, on a dead enemy or on a fallen friend searches it. `NIGHTFALL_AUTOPLAY`
//! loots what it walks past, and goes a short way out of its circle to a container near it,
//! standing still while it searches.
//!
//! What is drawn, clicked and looted is only what the loot view names ([`nightfall_sim::LootView`]:
//! the containers within the host's reach of our hero). A body is its enemy's own unit and no
//! GameObject is made for it, so a body out in the fog shows nothing the enemy's own concealment
//! does not.

use std::collections::{HashMap, HashSet};

use benilla_world::interact::WorldRightClick;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{
    Act, ContainerMark, Content, Holder, Inventory, ItemKind, Life, Place, PlayerId, V2,
};

use super::auto_brain::{stack_weight, takes};
use super::enemies::EnemyViews;
use super::run::{autoplay, Autoplay, LocalInput, Sim};
use super::view::Views;
use super::{content, units};
use crate::net::{GuidIndex, ObjectStore};

/// Containers' GameObject serials start here, above every other object Nightfall draws.
pub(super) const CONTAINER_SERIAL_BASE: u64 = 0x20_0000;

/// How far out of its way (yards) `NIGHTFALL_AUTOPLAY` goes to a container: a detour off its
/// circle, never a trek across the map.
const AUTO_DETOUR: f32 = 14.0;
/// Seconds `NIGHTFALL_AUTOPLAY` heads for one container before giving it up.
const AUTO_HEAD_FOR: f32 = 20.0;
/// How often `NIGHTFALL_AUTOPLAY` asks to open one that will not open, before giving it up.
const AUTO_TRIES: u8 = 8;
/// How many acts `NIGHTFALL_AUTOPLAY` spends wearing what it found and lighting a lantern.
const AUTO_KIT_ACTS: u8 = 12;

/// Which entity draws which container, and whether it was drawn open.
#[derive(Resource, Default)]
pub(super) struct ContainerViews {
    /// By container id (a [`ContainerMark::id`]): its object, and whether it is drawn open.
    pub drawn: HashMap<u32, (Entity, bool)>,
}

/// How a container looks: its display and scale; `None` for a body, which is its unit.
pub(super) fn look_of(mark: &ContainerMark, me: PlayerId, content: &Content) -> Option<(u32, f32)> {
    match mark.holder {
        Holder::Placed(k) => content
            .loot
            .kinds
            .get(usize::from(k))
            .map(|k| (k.display, k.scale)),
        Holder::Pile => Some((content.loot.pile_display, 0.6)),
        Holder::Remains(p) if p == me => Some((content.loot.remains_display, 0.8)),
        _ => None,
    }
}

/// Every container near us drawn, open once searched; those gone from the view go. One placed
/// without a height waits for the terrain under it to stream.
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut views: ResMut<ContainerViews>,
    mut index: ResMut<GuidIndex>,
    mut stores: Query<&mut ObjectStore>,
) {
    let loot = sim.world.loot_view(Some(sim.me));
    sync_drawn(
        &mut commands,
        &loot.marks,
        sim.me,
        &content(),
        &mut views,
        &mut index,
        &mut stores,
        |at| world_point.terrain_height_under(at),
    );
}

/// [`draw`] for the `marks` the loot view names, with `terrain` giving the ground under a point:
/// each container spawned once and kept; its lid opened and shut as it is searched; and one no
/// longer named (out of reach, or never a model) despawned, with its guid filed away.
#[expect(
    clippy::too_many_arguments,
    reason = "a system's own parameters, passed on so the rest can be tested without an engine"
)]
fn sync_drawn(
    commands: &mut Commands,
    marks: &[ContainerMark],
    me: PlayerId,
    content: &Content,
    views: &mut ContainerViews,
    index: &mut GuidIndex,
    stores: &mut Query<&mut ObjectStore>,
    terrain: impl Fn(Vec3) -> Option<f32>,
) {
    let mut seen = HashSet::new();
    for m in marks {
        let Some((display, scale)) = look_of(m, me, content) else {
            continue;
        };
        seen.insert(m.id);
        if let Some((e, open)) = views.drawn.get_mut(&m.id) {
            if *open != m.searched {
                if let Ok(mut store) = stores.get_mut(*e) {
                    units::set_go_open(&mut store, m.searched);
                }
                *open = m.searched;
            }
            continue;
        }
        let mut at = units::ground_to_bevy(m.at, m.z.unwrap_or(0.0));
        if m.z.is_none() {
            let Some(h) = terrain(at) else {
                continue;
            };
            at.y = h;
        }
        let serial = CONTAINER_SERIAL_BASE + u64::from(m.id);
        let e = units::spawn_object(commands, serial, display, scale, at, units::GO_TYPE_CHEST);
        index.0.insert(units::object_guid(serial), e);
        // Drawn shut, then opened by the next frame's compare if it was searched already.
        views.drawn.insert(m.id, (e, false));
    }
    views.drawn.retain(|id, (e, _)| {
        let keep = seen.contains(id);
        if !keep {
            index
                .0
                .remove(&units::object_guid(CONTAINER_SERIAL_BASE + u64::from(*id)));
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// A right-click on a container, a dead enemy or a fallen hero: search it.
pub(super) fn right_click(
    mut clicks: MessageReader<WorldRightClick>,
    press: Res<crate::target::PressPick>,
    drawn: Res<ContainerViews>,
    enemies: Res<EnemyViews>,
    views: Res<Views>,
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
) {
    if clicks.read().last().is_none() {
        return;
    }
    let Some(target) = press.object.target.or(press.hovered.any()) else {
        return;
    };
    let loot = sim.world.loot_view(Some(sim.me));
    let mark_of = |holder: Holder| loot.marks.iter().find(|m| m.holder == holder).map(|m| m.id);
    let id = drawn
        .drawn
        .iter()
        .find(|(_, (e, _))| *e == target)
        .map(|(id, _)| *id)
        .or_else(|| {
            let enemy = enemies.drawn.iter().find(|(_, e)| **e == target)?.0;
            mark_of(Holder::Body(*enemy))
        })
        .or_else(|| {
            let player = views.heroes.iter().find(|(_, e)| **e == target)?.0;
            mark_of(Holder::Hero(*player))
        });
    if let Some(id) = id {
        input.queue(Act::Open(id));
    }
}

/// What `NIGHTFALL_AUTOPLAY` does about the containers around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Target {
    /// One within reach: search it, standing still.
    Here(u32),
    /// One within the detour: walk to it.
    Toward(u32, V2),
    /// No container within reach or the detour: the circle (or another system's goal) goes on.
    Nothing,
}

/// The container `NIGHTFALL_AUTOPLAY` goes for from `from`: the nearest within `reach`, else the
/// nearest within `detour`; those `skip` says are done, locked, empty or ours alone to leave.
pub(super) fn target_of(
    marks: &[ContainerMark],
    from: V2,
    reach: f32,
    detour: f32,
    skip: impl Fn(&ContainerMark) -> bool,
) -> Target {
    let nearest = |within: f32| {
        marks
            .iter()
            .filter(|m| !skip(m))
            .filter(|m| m.at.distance(from) <= within)
            .min_by(|a, b| a.at.distance(from).total_cmp(&b.at.distance(from)))
    };
    if let Some(m) = nearest(reach) {
        Target::Here(m.id)
    } else if let Some(m) = nearest(detour) {
        Target::Toward(m.id, m.at)
    } else {
        Target::Nothing
    }
}

/// `NIGHTFALL_AUTOPLAY`'s looting so far in this run.
#[derive(Resource, Default)]
pub(super) struct AutoLoot {
    /// Containers searched and emptied, or given up on.
    done: HashSet<u32>,
    /// How many times each was asked to open.
    tries: HashMap<u32, u8>,
    /// Real seconds of the last decision.
    last: f32,
    /// The container being walked to, and when that began.
    heading: Option<(u32, f32)>,
    /// Acts spent wearing and lighting what it found.
    kit_acts: u8,
    /// The goal this system last gave [`Autoplay::goal`], so it clears only its own.
    detour: Option<V2>,
}

/// `NIGHTFALL_AUTOPLAY`: the nearest container within reach searched (standing still for it) and
/// emptied into the backpack, each once; one a short way off is walked to first. It gives up on
/// one that will not open or cannot be reached.
pub(super) fn autoloot(
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
    mut auto: ResMut<Autoplay>,
    mut state: ResMut<AutoLoot>,
    plan: Res<super::auto_brain::AutoPlan>,
    real: Res<Time<Real>>,
) {
    if !autoplay() {
        return;
    }
    let now = real.elapsed_secs();
    // A hero who is down or gone searches nothing, and walking is theirs no longer.
    let hero = sim
        .world
        .hero(sim.me)
        .filter(|h| matches!(h.fight.life, Life::Up));
    let Some(hero) = hero else {
        set_orders(&mut auto, false, None, &mut state.detour);
        return;
    };
    let content = content();
    let loot = sim.world.loot_view(Some(sim.me));
    let state = &mut *state;
    let skip = |m: &ContainerMark| {
        m.empty
            || m.locked
            || state.done.contains(&m.id)
            || matches!(m.holder, Holder::Remains(_) | Holder::Hero(_))
    };
    // Once it has had enough it searches no more, only finishes what is open.
    let target = if plan.0.leave.is_some() {
        Target::Nothing
    } else {
        target_of(
            &loot.marks,
            hero.pos,
            content.rules.run.reach,
            AUTO_DETOUR,
            skip,
        )
    };
    let hold = loot.open.is_some() || matches!(target, Target::Here(_));
    let mut goal = None;
    if let Target::Toward(id, at) = target {
        let since = match state.heading {
            Some((h, t)) if h == id => t,
            _ => {
                state.heading = Some((id, now));
                now
            }
        };
        if now - since > AUTO_HEAD_FOR {
            info!("nightfall: autoplay gives up reaching container {id}");
            state.done.insert(id);
            state.heading = None;
        } else {
            goal = Some(at);
        }
    } else {
        state.heading = None;
    }
    set_orders(&mut auto, hold, goal, &mut state.detour);
    if input.act.is_some() || !input.acts.is_empty() || now - state.last < 0.5 {
        return;
    }
    state.last = now;
    match (&loot.open, target) {
        (Some(open), _) if open.search.is_some() => {}
        (Some(open), _) => {
            // Only what it can carry: stack by stack, as a player drags them, never past its
            // weight or the free slots of its backpack.
            let free = sim
                .world
                .pack(sim.me)
                .map_or(0, |inv| inv.pack.iter().filter(|s| s.is_none()).count());
            let slots = takes(
                &open.stacks,
                hero.carry.load,
                content.rules.run.weight_full,
                free,
                |s| stack_weight(s, &content),
            );
            if !slots.is_empty() {
                info!(
                    "nightfall: autoplay takes {} of {} stacks from container {}",
                    slots.len(),
                    open.stacks.iter().flatten().count(),
                    open.id
                );
            }
            for slot in slots {
                input.queue(Act::Take(slot));
            }
            state.done.insert(open.id);
            input.queue(Act::Close);
        }
        (None, Target::Here(id)) => {
            let tries = state.tries.entry(id).or_default();
            *tries += 1;
            if *tries > AUTO_TRIES {
                info!("nightfall: autoplay gives up opening container {id}");
                state.done.insert(id);
            } else {
                input.queue(Act::Open(id));
            }
        }
        (None, _) => {
            // Nothing to search: put on what it found and light a lantern, a few acts at most.
            let inv = sim.world.pack(sim.me);
            let lit = hero.carry.lantern.is_some();
            if let (Some(inv), true) = (inv, state.kit_acts < AUTO_KIT_ACTS) {
                if let Some(act) = kit_up(inv, lit, &content) {
                    state.kit_acts += 1;
                    info!("nightfall: autoplay does {act:?}");
                    input.queue(act);
                }
            }
        }
    }
}

/// What `NIGHTFALL_AUTOPLAY` does with what it carries: the first thing in the backpack to wear in a
/// bare slot, or a lantern to light when none is lit. One act at a time; `None` when none is left.
pub(super) fn kit_up(inv: &Inventory, lit: bool, content: &Content) -> Option<Act> {
    for (i, stack) in inv.pack.iter().enumerate() {
        let Some(def) = stack.and_then(|s| content.items.get(usize::from(s.item))) else {
            continue;
        };
        let at = Place::Pack(i as u16);
        if let Some(slot) = def.slot {
            if inv.equip[slot.index()].is_none() {
                return Some(Act::Move {
                    from: at,
                    to: Place::Equip(slot),
                });
            }
        }
        if def.kind == ItemKind::Lantern && !lit {
            return Some(Act::Use(at));
        }
    }
    None
}

/// The orders `NIGHTFALL_AUTOPLAY`'s walk takes from the loot, written only when they change:
/// whether to stand still, and the goal of a detour to a container. A detour takes precedence over
/// any goal already set (the loot is picked up first); with none, the goal is left alone for
/// another system's (the exits') to own, and only a goal this one set (`ours`) is taken back.
fn set_orders(auto: &mut Autoplay, hold: bool, detour: Option<V2>, ours: &mut Option<V2>) {
    if auto.hold != hold {
        auto.hold = hold;
    }
    match detour {
        Some(at) => {
            if auto.goal != Some(at) {
                auto.goal = Some(at);
            }
            *ours = Some(at);
        }
        None => {
            if ours.take().is_some_and(|mine| auto.goal == Some(mine)) {
                auto.goal = None;
            }
        }
    }
}

/// A run ended: nothing of its containers is drawn any more (its entities went with the run), and
/// autoplay's looting starts afresh.
pub(super) fn forget(
    mut views: ResMut<ContainerViews>,
    mut state: ResMut<AutoLoot>,
    mut auto: ResMut<Autoplay>,
) {
    *views = ContainerViews::default();
    *state = AutoLoot::default();
    *auto = Autoplay::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(id: u32, holder: Holder, at: V2) -> ContainerMark {
        ContainerMark {
            id,
            holder,
            at,
            z: None,
            searched: false,
            empty: false,
            locked: false,
        }
    }

    /// [`sync_drawn`] once over `marks`, with `ground` under every point (none: still streaming).
    fn sync_once(world: &mut World, marks: Vec<ContainerMark>, ground: Option<f32>) {
        use bevy::ecs::system::RunSystemOnce;

        let content = super::super::shipped_content();
        world
            .run_system_once(
                move |mut commands: Commands,
                      mut views: ResMut<ContainerViews>,
                      mut index: ResMut<GuidIndex>,
                      mut stores: Query<&mut ObjectStore>| {
                    sync_drawn(
                        &mut commands,
                        &marks,
                        0,
                        &content,
                        &mut views,
                        &mut index,
                        &mut stores,
                        |_| ground,
                    );
                },
            )
            .unwrap();
    }

    fn objects(world: &mut World) -> usize {
        world
            .query_filtered::<Entity, With<super::super::NightfallEntity>>()
            .iter(world)
            .count()
    }

    #[test]
    fn a_container_is_drawn_once_opens_when_searched_and_goes_when_it_leaves_the_view() {
        let mut world = World::new();
        world.init_resource::<ContainerViews>();
        world.init_resource::<GuidIndex>();
        let guid = |id: u32| units::object_guid(CONTAINER_SERIAL_BASE + u64::from(id));
        let near = V2::new(1.0, 2.0);
        let crate_a = mark(1, Holder::Placed(0), near);
        let crate_b = mark(2, Holder::Placed(1), near);
        let body = mark(3, Holder::Body(9), near);

        // The ground has not streamed: a container placed without a height waits for it.
        sync_once(&mut world, vec![crate_a, crate_b], None);
        assert_eq!(objects(&mut world), 0);
        // It has: both are drawn (the body is its enemy's unit, no object of its own).
        sync_once(&mut world, vec![crate_a, crate_b, body], Some(3.0));
        assert_eq!(objects(&mut world), 2);
        let (e1, _) = world.resource::<ContainerViews>().drawn[&1];
        assert!(world.resource::<GuidIndex>().0.contains_key(&guid(1)));
        assert!(world.resource::<GuidIndex>().0.contains_key(&guid(2)));
        assert_eq!(
            world.get::<Transform>(e1).unwrap().translation,
            units::ground_to_bevy(near, 3.0)
        );
        assert_eq!(
            world.get::<ObjectStore>(e1).unwrap().0.gameobject_state(),
            Some(1)
        );

        // Seen again: kept, never spawned twice. Searched: its lid opens, once.
        sync_once(&mut world, vec![crate_a, crate_b], Some(3.0));
        assert_eq!(objects(&mut world), 2);
        let searched = ContainerMark {
            searched: true,
            ..crate_a
        };
        sync_once(&mut world, vec![searched, crate_b], Some(3.0));
        assert_eq!(world.resource::<ContainerViews>().drawn[&1], (e1, true));
        assert_eq!(
            world.get::<ObjectStore>(e1).unwrap().0.gameobject_state(),
            Some(0)
        );

        // One leaves the view: its object and its guid go, and nothing else.
        sync_once(&mut world, vec![searched], Some(3.0));
        assert_eq!(objects(&mut world), 1);
        assert!(!world.resource::<GuidIndex>().0.contains_key(&guid(2)));
        assert!(world.resource::<GuidIndex>().0.contains_key(&guid(1)));
        assert_eq!(world.resource::<ContainerViews>().drawn.len(), 1);
        // And all of them, when the view empties.
        sync_once(&mut world, Vec::new(), Some(3.0));
        assert_eq!(objects(&mut world), 0);
        assert!(world.resource::<GuidIndex>().0.is_empty());
    }

    #[test]
    fn a_run_ending_forgets_its_containers_and_autoplays_orders() {
        use bevy::ecs::system::RunSystemOnce;

        let mut world = World::new();
        let mut views = ContainerViews::default();
        views.drawn.insert(1, (Entity::PLACEHOLDER, true));
        world.insert_resource(views);
        world.insert_resource(AutoLoot {
            done: HashSet::from([4]),
            kit_acts: 3,
            ..AutoLoot::default()
        });
        world.insert_resource(Autoplay {
            hold: true,
            goal: Some(V2::new(1.0, 1.0)),
        });
        world.run_system_once(forget).unwrap();
        assert!(world.resource::<ContainerViews>().drawn.is_empty());
        let state = world.resource::<AutoLoot>();
        assert!(state.done.is_empty() && state.kit_acts == 0);
        let auto = world.resource::<Autoplay>();
        assert!(!auto.hold && auto.goal.is_none());
    }

    #[test]
    fn crates_and_piles_are_models_and_bodies_are_not() {
        let c = super::super::shipped_content();
        let at = V2::new(0.0, 0.0);
        let (display, scale) =
            look_of(&mark(1, Holder::Placed(0), at), 0, &c).expect("a placed crate has a model");
        assert_eq!(display, c.loot.kinds[0].display);
        assert_eq!(scale, c.loot.kinds[0].scale);
        assert_eq!(
            look_of(&mark(2, Holder::Pile, at), 0, &c),
            Some((c.loot.pile_display, 0.6))
        );
        // A body is its unit (an enemy's, a hero's); only our own remains are a model.
        assert_eq!(look_of(&mark(3, Holder::Body(7), at), 0, &c), None);
        assert_eq!(look_of(&mark(4, Holder::Hero(1), at), 0, &c), None);
        assert!(look_of(&mark(5, Holder::Remains(0), at), 0, &c).is_some());
        assert_eq!(look_of(&mark(6, Holder::Remains(1), at), 0, &c), None);
        // A kind the content lacks draws nothing.
        assert_eq!(look_of(&mark(7, Holder::Placed(200), at), 0, &c), None);
    }

    #[test]
    fn autoplay_searches_what_is_in_reach_and_goes_a_short_way_for_the_rest() {
        let from = V2::ZERO;
        let marks = [
            mark(1, Holder::Placed(0), V2::new(10.0, 0.0)),
            mark(2, Holder::Placed(0), V2::new(2.0, 0.0)),
            mark(3, Holder::Placed(0), V2::new(40.0, 0.0)),
        ];
        let none = |_: &ContainerMark| false;
        // The one in reach first, though another lies within the detour.
        assert_eq!(target_of(&marks, from, 3.0, 14.0, none), Target::Here(2));
        // Once that is done, the next is walked to; the far one is nothing.
        let done = |m: &ContainerMark| m.id == 2;
        assert_eq!(
            target_of(&marks, from, 3.0, 14.0, done),
            Target::Toward(1, V2::new(10.0, 0.0))
        );
        let done = |m: &ContainerMark| m.id != 3;
        assert_eq!(target_of(&marks, from, 3.0, 14.0, done), Target::Nothing);
    }

    #[test]
    fn autoplay_wears_what_fits_a_bare_slot_and_lights_a_lantern_once() {
        use nightfall_sim::Stack;

        let c = super::super::shipped_content();
        let tunic = c.item("watchmans_tunic").unwrap();
        let lantern = c.item("hooded_lantern").unwrap();
        let mut inv = Inventory::new(&c);
        inv.add(Stack::new(tunic, 1), &c);
        inv.add(Stack::new(lantern, 1), &c);
        let chest = nightfall_sim::EquipSlot::Chest;
        assert_eq!(
            kit_up(&inv, false, &c),
            Some(Act::Move {
                from: Place::Pack(0),
                to: Place::Equip(chest)
            })
        );
        inv.move_to(Place::Pack(0), Place::Equip(chest), &c)
            .unwrap();
        assert_eq!(kit_up(&inv, false, &c), Some(Act::Use(Place::Pack(1))));
        assert_eq!(kit_up(&inv, true, &c), None, "lit already");
        // A second tunic stays in the pack: the slot is taken.
        inv.add(Stack::new(tunic, 1), &c);
        assert_eq!(kit_up(&inv, true, &c), None);
    }

    #[test]
    fn the_loots_detour_takes_precedence_and_leaves_other_goals_alone() {
        let (crate_at, exit_at) = (V2::new(1.0, 1.0), V2::new(50.0, 0.0));
        let mut auto = Autoplay::default();
        let mut ours = None;
        // A detour to a crate sets the goal, and stands still when asked to.
        set_orders(&mut auto, false, Some(crate_at), &mut ours);
        assert_eq!(auto.goal, Some(crate_at));
        set_orders(&mut auto, true, Some(crate_at), &mut ours);
        assert!(auto.hold);
        // It is taken back with the detour.
        set_orders(&mut auto, false, None, &mut ours);
        assert!(!auto.hold && auto.goal.is_none() && ours.is_none());
        // With no detour another system's goal is not touched...
        auto.goal = Some(exit_at);
        set_orders(&mut auto, false, None, &mut ours);
        assert_eq!(auto.goal, Some(exit_at));
        // ...but a detour overrides it, and ends by clearing only a goal that is still its own.
        set_orders(&mut auto, false, Some(crate_at), &mut ours);
        assert_eq!(auto.goal, Some(crate_at));
        auto.goal = Some(exit_at);
        set_orders(&mut auto, false, None, &mut ours);
        assert_eq!(auto.goal, Some(exit_at), "the exits' goal replaced ours");
    }
}
