//! "?" and "!" over the enemies suspicious of or alerted to our hero (spec §5.5): a glyph of WoW's
//! own font floating over the head, from this player's marks ([`Sim::marks`]). Hidden while the
//! enemy is [`Concealed`]: what our hero does not see, the interface does not show. A text node
//! per marked enemy rather than combat text, which rises, fades and stays where it was spawned.

use std::collections::{HashMap, HashSet};

use benilla_world::view::WorldCamera;
use bevy::prelude::*;
use nightfall_sim::Alert;

use super::enemies::EnemyViews;
use super::run::Sim;
use super::NightfallEntity;
use crate::target::Concealed;

/// The glyph's size, UI pixels.
const SIZE: f32 = 30.0;
/// Half the glyph's width and its height, UI pixels, to set it centred over the head.
const HALF_W: f32 = 8.0;
const HIGH: f32 = 34.0;

/// A mark's text node.
#[derive(Component)]
pub(super) struct MarkText;

/// Which text node shows which enemy's mark.
#[derive(Resource, Default)]
pub(super) struct MarkViews {
    nodes: HashMap<u32, Entity>,
}

/// The glyph and colour of a mark: a gold "?", a red "!".
pub(super) fn mark_face(alert: Alert) -> (&'static str, Color) {
    match alert {
        Alert::Suspicious => ("?", Color::srgb(1.0, 0.82, 0.0)),
        Alert::Alerted => ("!", Color::srgb(1.0, 0.15, 0.1)),
    }
}

/// How high over an enemy's feet its mark floats, yards: over the head of a model at `scale`.
pub(super) fn mark_height(scale: f32) -> f32 {
    2.7 * scale
}

/// Every mark over its enemy's head, where the camera sees it; a mark whose enemy is concealed or
/// off screen is hidden, and the node of an enemy that lost its mark (or left the world) is
/// despawned. Nodes are made as marks first appear and go with the run's other entities.
#[allow(clippy::type_complexity)]
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    enemy_views: Res<EnemyViews>,
    assets: Res<AssetServer>,
    ui_scale: Res<UiScale>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    bodies: Query<(&Transform, Has<Concealed>)>,
    mut views: ResMut<MarkViews>,
    mut texts: Query<(&mut Node, &mut Text, &mut TextColor, &mut Visibility), With<MarkText>>,
) {
    let s = ui_scale.0;
    let cam = cams.single().ok();
    // A node that went with the last run is made again when its enemy is next marked.
    views.nodes.retain(|_, e| texts.contains(*e));
    // A node whose enemy is no longer marked, or no longer in the world, goes at once rather than
    // lingering hidden.
    let marked: HashSet<u32> = sim
        .marks
        .iter()
        .filter(|m| sim.world.enemy(m.enemy).is_some())
        .map(|m| m.enemy)
        .collect();
    views.nodes.retain(|id, &mut node| {
        if marked.contains(id) {
            return true;
        }
        commands.entity(node).despawn();
        false
    });
    let mut shown = HashSet::new();
    for m in &sim.marks {
        let Some(enemy) = sim.world.enemy(m.enemy) else {
            continue;
        };
        let Some(&body) = enemy_views.drawn.get(&m.enemy) else {
            continue;
        };
        let Ok((tf, concealed)) = bodies.get(body) else {
            continue;
        };
        let Some((camera, cam_tf)) = cam else {
            continue;
        };
        if concealed {
            continue;
        }
        let scale = sim.world.content().enemies[enemy.kind].scale;
        let head = tf.translation + Vec3::Y * mark_height(scale);
        let Ok(at) = camera.world_to_viewport(&GlobalTransform::from(*cam_tf), head) else {
            continue;
        };
        let node = *views.nodes.entry(m.enemy).or_insert_with(|| {
            commands
                .spawn((
                    MarkText,
                    NightfallEntity,
                    Text::new(""),
                    TextFont {
                        font: crate::char_select::wow_font(&assets),
                        font_size: SIZE,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    GlobalZIndex(340),
                    Visibility::Hidden,
                ))
                .id()
        });
        // A node spawned this frame is placed from the next.
        let Ok((mut n, mut text, mut color, mut vis)) = texts.get_mut(node) else {
            continue;
        };
        shown.insert(m.enemy);
        let (glyph, tint) = mark_face(m.alert);
        n.left = Val::Px(at.x / s - HALF_W);
        n.top = Val::Px(at.y / s - HIGH);
        if text.0 != glyph {
            text.0 = glyph.into();
        }
        color.0 = tint;
        *vis = Visibility::Inherited;
    }
    for (id, &node) in &views.nodes {
        if !shown.contains(id) {
            if let Ok((_, _, _, mut vis)) = texts.get_mut(node) {
                *vis = Visibility::Hidden;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_for_suspicion_and_a_bang_for_an_alert() {
        assert_eq!(mark_face(Alert::Suspicious).0, "?");
        assert_eq!(mark_face(Alert::Alerted).0, "!");
        assert_ne!(mark_face(Alert::Suspicious).1, mark_face(Alert::Alerted).1);
    }

    #[test]
    fn the_mark_floats_over_a_bigger_model_higher() {
        assert_eq!(mark_height(1.0), 2.7);
        assert!(mark_height(1.3) > mark_height(1.0));
    }

    /// A world with a hosted run holding one enemy (id 5), no camera, and a mark node for each of
    /// the enemies `nodes` names, for [`draw`] to run on.
    fn world_with_nodes(nodes: &[u32]) -> (bevy::app::App, HashMap<u32, Entity>) {
        use std::sync::Arc;

        use nightfall_sim::{Content, ContentSources, Enemy, V2};

        let content = Content::from_ron(ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
        })
        .expect("the shipped content parses");
        let mut sim = Sim::for_test(nightfall_sim::World::new(Arc::new(content)));
        sim.world.enemies.push(Enemy {
            id: 5,
            kind: 0,
            pos: V2::new(0.0, 0.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 1.0,
            max_hp: 1.0,
            speed: 0.0,
            asleep: false,
            fight: Default::default(),
        });
        let mut app = bevy::app::App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_resource::<UiScale>();
        app.init_resource::<EnemyViews>();
        app.insert_resource(sim);
        let mut views = MarkViews::default();
        for &id in nodes {
            let node = app
                .world_mut()
                .spawn((
                    MarkText,
                    Node::default(),
                    Text::new("?"),
                    TextColor(Color::WHITE),
                    Visibility::Inherited,
                ))
                .id();
            views.nodes.insert(id, node);
        }
        let ids = views.nodes.clone();
        app.insert_resource(views);
        (app, ids)
    }

    #[test]
    fn a_node_whose_enemy_lost_its_mark_or_left_is_despawned() {
        use bevy::ecs::system::RunSystemOnce;

        // Enemy 5 is in the world and marked; 6 is marked but not in the world; 7 is unmarked.
        let (mut app, ids) = world_with_nodes(&[5, 6, 7]);
        app.world_mut().resource_mut::<Sim>().marks = vec![
            nightfall_sim::Mark {
                enemy: 5,
                alert: Alert::Alerted,
            },
            nightfall_sim::Mark {
                enemy: 6,
                alert: Alert::Suspicious,
            },
        ];
        app.world_mut().run_system_once(draw).unwrap();
        let world = app.world();
        assert!(
            world.get_entity(ids[&5]).is_ok(),
            "marked and in the world: kept"
        );
        assert!(
            world.get_entity(ids[&6]).is_err(),
            "marked but gone from the world: despawned"
        );
        assert!(
            world.get_entity(ids[&7]).is_err(),
            "no longer marked: despawned"
        );
        let views = world.resource::<MarkViews>();
        assert_eq!(views.nodes.keys().copied().collect::<Vec<_>>(), vec![5]);
        // The enemy has no drawn body, so its node is hidden rather than left showing where it was.
        assert_eq!(world.get::<Visibility>(ids[&5]), Some(&Visibility::Hidden));
    }

    #[test]
    fn a_node_that_went_with_the_run_is_forgotten() {
        use bevy::ecs::system::RunSystemOnce;

        let (mut app, ids) = world_with_nodes(&[5]);
        app.world_mut().despawn(ids[&5]);
        app.world_mut().run_system_once(draw).unwrap();
        assert!(app.world().resource::<MarkViews>().nodes.is_empty());
    }
}
