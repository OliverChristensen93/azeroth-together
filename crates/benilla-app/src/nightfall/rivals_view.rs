//! Rivals on screen (spec §7.4). They are heroes of another side in the snapshot, so the hero
//! code draws them already, as WoW players; this module makes them hostile ones (the monster
//! faction template, so WoW's plates, rings and target frame show them red, as a hostile player
//! is shown), conceals those our hero does not see (no Tab, plate or minimap dot, as for enemies;
//! their body is still drawn, faded when in stealth), floats their "?" and "!" over them, names
//! the parties out on the map table, and holds the rivals' test hooks.

use std::collections::{HashMap, HashSet};

use benilla_protocol::field::FIELD_UNIT_BYTES_1;
use benilla_protocol::messages::ObjectFields;
use benilla_world::view::WorldCamera;
use bevy::prelude::*;
use nightfall_sim::{Content, PlayerId, RaidRoll, RivalOut, Side, V2};

use super::conceal::{bytes_1, follow_target, settle_grace};
use super::marks::{mark_height, show_marks, MarkTexts};
use super::run::Sim;
use super::units::{FACTION_MONSTER, FACTION_PLAYER};
use super::view::Views;
use crate::net::ObjectStore;
use crate::target::{Concealed, Selection};

/// `UNIT_FIELD_FACTIONTEMPLATE`.
const FIELD_FACTION: u16 = 35;
/// Seconds into the raid a party a test hook names comes, when `NIGHTFALL_RIVALS_AT` is unset.
const HOOK_AT: f32 = 5.0;

/// The faction template a hero of `side` wears: the players' own for the party (what every hero
/// is spawned with), the monsters' for a rival, which WoW's reaction rules make hostile to every
/// player.
pub(super) fn faction_of(side: Side) -> u32 {
    match side {
        Side::Party => FACTION_PLAYER,
        Side::Rival(_) => FACTION_MONSTER,
    }
}

/// The map table's line about rivals (spec §8.5's intel): the parties the planned raid brings.
pub(super) fn intel_lines(roll: &RaidRoll, content: &Content) -> Vec<String> {
    let names: Vec<&str> = roll
        .rivals
        .iter()
        .filter_map(|o| content.rivals.parties.get(usize::from(o.party)))
        .map(|p| p.name.as_str())
        .collect();
    vec![if names.is_empty() {
        "Rivals: none heard of".into()
    } else {
        format!("Rivals: {}", names.join(", "))
    }]
}

/// The planned raid's rivals become the parties named in `keys` (comma-separated), each coming
/// `at` seconds in at the middle of its aggression, by the rivals' entry nearest the map's first
/// entry that is out of the party's sight there (`arrive_unseen`), else the farthest. The
/// warnings are for keys the content has not.
pub(super) fn force(roll: &mut RaidRoll, content: &Content, keys: &str, at: f32) -> Vec<String> {
    let mut warnings = Vec::new();
    let entries = &content.rivals.entries;
    if entries.is_empty() {
        return vec!["NIGHTFALL_RIVALS: rivals.ron has no entries".into()];
    }
    let Some(first) = content.map.entries.first() else {
        return vec!["NIGHTFALL_RIVALS: the map has no entries".into()];
    };
    let start = V2::new(first[0], first[1]);
    let unseen = content.rules.night.arrive_unseen;
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| {
        start
            .distance(entries[a])
            .total_cmp(&start.distance(entries[b]))
    });
    let entry = order
        .iter()
        .copied()
        .find(|&i| start.distance(entries[i]) >= unseen)
        .unwrap_or(*order.last().unwrap_or(&0));
    roll.rivals.clear();
    for (n, key) in keys
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .enumerate()
    {
        let Some(party) = content.party(key) else {
            warnings.push(format!(
                "NIGHTFALL_RIVALS: no party \"{key}\" in rivals.ron"
            ));
            continue;
        };
        let (lo, hi) = content.rivals.parties[party].aggression;
        roll.rivals.push(RivalOut {
            party: party as u8,
            entry: entry as u8,
            at,
            aggression: (lo + hi) * 0.5,
            seed: roll.seed ^ (n as u64 + 1),
        });
    }
    warnings
}

/// `NIGHTFALL_RIVALS` and `NIGHTFALL_RIVALS_AT` on the planned raid, warning of what they name
/// wrong.
pub(super) fn force_from_env(roll: &mut RaidRoll, content: &Content) {
    let Ok(keys) = std::env::var("NIGHTFALL_RIVALS") else {
        return;
    };
    let at = std::env::var("NIGHTFALL_RIVALS_AT")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|t: &f32| t.is_finite() && *t >= 0.0)
        .unwrap_or(HOOK_AT);
    for w in force(roll, content, &keys, at) {
        warn!("{w}");
    }
}

/// Every hero's body wears its side's faction: a rival's turns hostile the frame it is drawn.
pub(super) fn hostile(sim: Res<Sim>, views: Res<Views>, mut stores: Query<&mut ObjectStore>) {
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let Ok(mut store) = stores.get_mut(e) else {
            continue;
        };
        let want = faction_of(hero.side);
        if store.0.unit_faction_template() != Some(want) {
            store
                .0
                .merge(ObjectFields::from_pairs(&[(FIELD_FACTION, want)]));
        }
    }
}

/// This frame's [`nightfall_sim::RivalView`] for our hero, worked out once for every system that
/// reads it (on the host it casts a ray per rival per watcher).
#[derive(Resource, Default)]
pub(super) struct RivalSight(pub(super) nightfall_sim::RivalView);

/// Works [`RivalSight`] out, before the systems that read it.
pub(super) fn look(sim: Res<Sim>, mut sight: ResMut<RivalSight>) {
    let view = sim.world.rival_view(sim.me);
    if sight.0 != view {
        sight.0 = view;
    }
}

/// The selected rival's grace, as the enemies' [`TargetGrace`](super::conceal::TargetGrace) is.
#[derive(Resource, Default)]
pub(super) struct RivalGrace(super::conceal::TargetGrace);

/// Every rival our hero does not see (this player's [`nightfall_sim::RivalView`]) is concealed
/// and untrackable; one they see is neither. Its body is still drawn: walls hide it on screen.
/// A selected rival that stays out of sight for the enemies' grace is deselected.
#[allow(clippy::too_many_arguments)]
pub(super) fn conceal(
    mut commands: Commands,
    sim: Res<Sim>,
    sight: Res<RivalSight>,
    views: Res<Views>,
    time: Res<Time<Real>>,
    mut grace: ResMut<RivalGrace>,
    mut selection: ResMut<Selection>,
    mut bodies: Query<(&mut ObjectStore, Has<Concealed>)>,
) {
    let now = time.elapsed_secs();
    for hero in sim.world.heroes.iter().filter(|h| h.side.is_rival()) {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let Ok((mut store, concealed)) = bodies.get_mut(e) else {
            continue;
        };
        let hide = !sight.0.seen.contains(&hero.player);
        if hide != concealed {
            if hide {
                commands.entity(e).insert(Concealed);
            } else {
                commands.entity(e).remove::<Concealed>();
            }
        }
        follow_target(&mut grace.0, &mut selection, e, hide, now);
        let stealthed = store.0.unit_is_stealthed();
        let now = bytes_1(&store.0, None, stealthed, store.0.unit_is_untrackable());
        let want = bytes_1(&store.0, None, stealthed, hide);
        if now != want {
            store
                .0
                .merge(ObjectFields::from_pairs(&[(FIELD_UNIT_BYTES_1, want)]));
        }
    }
    settle_grace(&mut grace.0, &selection);
}

/// Which text node shows which rival's mark.
#[derive(Resource, Default)]
pub(super) struct RivalMarkViews {
    nodes: HashMap<PlayerId, Entity>,
}

/// Every rival's mark for us over its head, where the camera sees it, as the enemies' marks are
/// drawn (one shared routine); hidden while the rival is concealed, and the node of a rival that
/// no longer marks us, or whose body is gone, is despawned.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn marks(
    mut commands: Commands,
    sight: Res<RivalSight>,
    views: Res<Views>,
    assets: Res<AssetServer>,
    ui_scale: Res<UiScale>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    bodies: Query<(&Transform, Has<Concealed>)>,
    mut nodes: ResMut<RivalMarkViews>,
    mut texts: MarkTexts,
) {
    let cam = cams.single().ok();
    nodes.nodes.retain(|_, e| texts.contains(*e));
    let marked: HashSet<PlayerId> = sight
        .0
        .marks
        .iter()
        .filter(|m| views.heroes.contains_key(&m.rival))
        .map(|m| m.rival)
        .collect();
    let mut placed = Vec::new();
    for m in &sight.0.marks {
        let Some(&body) = views.heroes.get(&m.rival) else {
            continue;
        };
        let (Ok((tf, concealed)), Some((camera, cam_tf))) = (bodies.get(body), cam) else {
            continue;
        };
        if concealed {
            continue;
        }
        let head = tf.translation + Vec3::Y * mark_height(1.0);
        let Ok(at) = camera.world_to_viewport(&GlobalTransform::from(*cam_tf), head) else {
            continue;
        };
        placed.push((m.rival, m.alert, at));
    }
    show_marks(
        &mut commands,
        &assets,
        ui_scale.0,
        &mut nodes.nodes,
        &mut texts,
        &marked,
        &placed,
    );
}

/// `NIGHTFALL_TRACE_RIVALS=1`: every 2 seconds, each party's goal and members (on the host) and
/// what this player is told of the rivals.
pub(super) fn trace(
    sim: Res<Sim>,
    sight: Res<RivalSight>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
) {
    if std::env::var_os("NIGHTFALL_TRACE_RIVALS").is_none() || real.elapsed_secs() - *last < 2.0 {
        return;
    }
    *last = real.elapsed_secs();
    for line in sim.world.rival_debug() {
        info!("nightfall: rivals: {line}");
    }
    let v = &sight.0;
    info!("nightfall: rivals seen {:?}, marks {:?}", v.seen, v.marks);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Content {
        Content::from_ron(nightfall_sim::ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
            rivals: include_str!("../../../../content/nightfall/rivals.ron"),
        })
        .unwrap()
    }

    #[test]
    fn rivals_wear_the_monsters_faction_and_the_party_the_players() {
        assert_eq!(faction_of(Side::Party), FACTION_PLAYER);
        assert_eq!(faction_of(Side::Rival(2)), FACTION_MONSTER);
    }

    #[test]
    fn the_map_table_names_the_parties_out() {
        let c = shipped();
        let mut roll = RaidRoll::standard(&c, 1);
        assert_eq!(
            intel_lines(&roll, &c),
            vec!["Rivals: none heard of".to_string()]
        );
        let warnings = force(&mut roll, &c, "grave_robbers, watch_deserters", 5.0);
        assert!(warnings.is_empty());
        assert_eq!(
            intel_lines(&roll, &c),
            vec!["Rivals: Grave robbers, Night Watch deserters".to_string()]
        );
    }

    #[test]
    fn the_hook_brings_the_named_parties_out_of_the_partys_sight() {
        let c = shipped();
        let mut roll = RaidRoll::standard(&c, 7);
        let warnings = force(&mut roll, &c, "cutthroats,nobody", 12.0);
        assert_eq!(warnings.len(), 1);
        assert_eq!(roll.rivals.len(), 1);
        let o = roll.rivals[0];
        assert_eq!(usize::from(o.party), c.party("cutthroats").unwrap());
        assert_eq!(o.at, 12.0);
        let start = V2::new(c.map.entries[0][0], c.map.entries[0][1]);
        let e = c.rivals.entries[usize::from(o.entry)];
        let far = c
            .rivals
            .entries
            .iter()
            .all(|x| start.distance(*x) < c.rules.night.arrive_unseen);
        assert!(start.distance(e) >= c.rules.night.arrive_unseen || far);
        let (lo, hi) = c.rivals.parties[usize::from(o.party)].aggression;
        assert!((o.aggression - (lo + hi) * 0.5).abs() < 1e-5);
    }

    /// A world of our hero (0) and a rival (1) near them, each with a bare body, for [`hostile`]
    /// and [`conceal`] to run on.
    fn world_with_a_rival() -> (World, Entity, Entity) {
        use std::sync::Arc;

        let mut sim = Sim::for_test(nightfall_sim::World::new(Arc::new(shipped())));
        sim.world.add_hero(0, "rogue", "Me", 1, 0).unwrap();
        sim.world.add_hero(1, "rogue", "Rival", 1, 0).unwrap();
        sim.world.heroes[1].side = Side::Rival(0);
        let mut world = World::new();
        let mine = world.spawn(ObjectStore(ObjectFields::from_pairs(&[]))).id();
        let theirs = world.spawn(ObjectStore(ObjectFields::from_pairs(&[]))).id();
        let mut views = Views::default();
        views.heroes.insert(0, mine);
        views.heroes.insert(1, theirs);
        world.insert_resource(views);
        world.insert_resource(sim);
        world.init_resource::<RivalSight>();
        world.init_resource::<RivalGrace>();
        world.init_resource::<Selection>();
        let mut time = Time::<Real>::default();
        time.update_with_duration(std::time::Duration::ZERO);
        world.insert_resource(time);
        (world, mine, theirs)
    }

    #[test]
    fn a_rivals_body_turns_hostile_and_ours_stays_a_players() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, mine, theirs) = world_with_a_rival();
        world.run_system_once(hostile).unwrap();
        let faction = |w: &World, e| w.get::<ObjectStore>(e).unwrap().0.unit_faction_template();
        assert_eq!(faction(&world, mine), Some(FACTION_PLAYER));
        assert_eq!(faction(&world, theirs), Some(FACTION_MONSTER));
    }

    #[test]
    fn an_unseen_rival_is_concealed_and_untrackable_and_a_seen_one_is_not() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, mine, theirs) = world_with_a_rival();
        // Far out of sight (past the fog): hidden.
        world.resource_mut::<Sim>().world.heroes[1].pos = V2::new(9000.0, 9000.0);
        world.run_system_once(look).unwrap();
        world.run_system_once(conceal).unwrap();
        assert!(
            world.get::<Concealed>(theirs).is_some(),
            "unseen: concealed"
        );
        assert!(world
            .get::<ObjectStore>(theirs)
            .unwrap()
            .0
            .unit_is_untrackable());
        assert!(world.get::<Concealed>(mine).is_none(), "ours is left alone");

        // Beside our hero: seen.
        let near = world.resource::<Sim>().world.heroes[0].pos + V2::new(3.0, 0.0);
        world.resource_mut::<Sim>().world.heroes[1].pos = near;
        world.run_system_once(look).unwrap();
        world.run_system_once(conceal).unwrap();
        assert!(world.get::<Concealed>(theirs).is_none(), "seen: revealed");
        assert!(!world
            .get::<ObjectStore>(theirs)
            .unwrap()
            .0
            .unit_is_untrackable());
    }

    /// Lets `secs` of real time pass for the next run of [`conceal`].
    fn pass(world: &mut World, secs: f32) {
        world
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::from_secs_f32(secs));
    }

    #[test]
    fn a_selected_rival_that_stays_out_of_sight_is_deselected_after_the_grace() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, _mine, theirs) = world_with_a_rival();
        // Selected while seen, beside our hero.
        let near = world.resource::<Sim>().world.heroes[0].pos + V2::new(3.0, 0.0);
        world.resource_mut::<Sim>().world.heroes[1].pos = near;
        *world.resource_mut::<Selection>() = Selection {
            target: Some(theirs),
            guid: Some(9),
            last: None,
        };
        world.run_system_once(look).unwrap();
        world.run_system_once(conceal).unwrap();
        assert_eq!(world.resource::<Selection>().target, Some(theirs));

        // Out of sight a moment: still selected. Past the grace: let go.
        world.resource_mut::<Sim>().world.heroes[1].pos = V2::new(9000.0, 9000.0);
        world.run_system_once(look).unwrap();
        world.run_system_once(conceal).unwrap();
        pass(&mut world, 1.0);
        world.run_system_once(conceal).unwrap();
        assert_eq!(world.resource::<Selection>().target, Some(theirs));
        pass(&mut world, super::super::conceal::TARGET_GRACE);
        world.run_system_once(conceal).unwrap();
        assert_eq!(world.resource::<Selection>().target, None);
        assert_eq!(world.resource::<Selection>().guid, None);
    }

    #[test]
    fn a_rival_selected_from_memory_while_unseen_is_dropped_at_once() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut world, _mine, theirs) = world_with_a_rival();
        world.resource_mut::<Sim>().world.heroes[1].pos = V2::new(9000.0, 9000.0);
        world.resource_mut::<Selection>().target = Some(theirs);
        world.run_system_once(look).unwrap();
        world.run_system_once(conceal).unwrap();
        assert_eq!(world.resource::<Selection>().target, None);
    }

    #[test]
    fn a_rivals_mark_node_goes_when_the_mark_does() {
        use bevy::ecs::system::RunSystemOnce;

        let mut app = bevy::app::App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<UiScale>();
        app.init_resource::<Views>();
        app.init_resource::<RivalSight>();
        let node = app
            .world_mut()
            .spawn((
                super::super::marks::MarkText,
                Node::default(),
                Text::new("!"),
                TextColor(Color::WHITE),
                Visibility::Inherited,
            ))
            .id();
        let mut nodes = RivalMarkViews::default();
        nodes.nodes.insert(1, node);
        app.insert_resource(nodes);
        // No mark for rival 1 this frame.
        app.world_mut().run_system_once(marks).unwrap();
        assert!(app.world().get_entity(node).is_err(), "no mark: despawned");
        assert!(app.world().resource::<RivalMarkViews>().nodes.is_empty());
    }
}
