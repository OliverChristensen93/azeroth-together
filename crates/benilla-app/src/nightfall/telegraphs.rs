//! Telegraphs (spec §6.1): an elite's big move shown on the ground where it will land, 1 to 1.5
//! seconds before it does: a red circle or cone draped over the terrain, glowing brighter as the
//! blow draws near, gone when it lands. Our own unlit mesh, not one of WoW's decals (which have a
//! circle only); concealed enemies' telegraphs show too, since the ground glows whoever casts.

use std::collections::{HashMap, HashSet};

use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use nightfall_sim::{Shape, V2};

use super::run::Sim;
use super::{enemies, units, NightfallEntity};

/// Rings from the centre out, and points around (or across) each.
const RINGS: usize = 4;
const SEGMENTS: usize = 32;
/// Yards over the ground the shape floats, so the terrain never hides it.
const LIFT: f32 = 0.12;
/// A glow that moved less than this is not written to the material again.
const GLOW_STEP: f32 = 0.004;

/// The ground a telegraph covers, as rings of points out from where it is put (the first point
/// its centre) and the triangles between them, indices into the points.
pub(super) fn ground_grid(shape: Shape, at: V2, facing: V2) -> (Vec<V2>, Vec<u32>) {
    let (reach, span, wrap) = match shape {
        Shape::Circle { radius } => (radius, std::f32::consts::TAU, true),
        Shape::Cone { length, angle } => (length, angle.to_radians(), false),
    };
    let base = facing.y.atan2(facing.x) - if wrap { 0.0 } else { span / 2.0 };
    let per_ring = if wrap { SEGMENTS } else { SEGMENTS + 1 };
    let mut points = vec![at];
    for k in 1..=RINGS {
        let r = reach * k as f32 / RINGS as f32;
        for j in 0..per_ring {
            let a = base + span * j as f32 / SEGMENTS as f32;
            points.push(at + V2::from_angle(a, r));
        }
    }
    let ring = |k: usize, j: usize| (1 + (k - 1) * per_ring + j % per_ring) as u32;
    let mut tris = Vec::new();
    for j in 0..SEGMENTS {
        tris.extend([0, ring(1, j), ring(1, j + 1)]);
        for k in 1..RINGS {
            let (a, b, c, d) = (
                ring(k, j),
                ring(k, j + 1),
                ring(k + 1, j),
                ring(k + 1, j + 1),
            );
            tris.extend([a, c, b, b, c, d]);
        }
    }
    (points, tris)
}

/// The shape's alpha as its blow draws near: faint as it shows, strong as it lands.
pub(super) fn glow(left: f32, total: f32) -> f32 {
    0.15 + 0.5 * (1.0 - (left / total.max(1e-3)).clamp(0.0, 1.0))
}

/// A telegraph's shape on the ground.
#[derive(Component)]
pub(super) struct TelegraphShape;

/// One telegraph's drawing: its entity, its material (whose alpha follows the windup) and the
/// alpha last written to it.
struct Drawn {
    entity: Entity,
    material: Handle<StandardMaterial>,
    alpha: f32,
}

/// Which entity draws which telegraph.
#[derive(Resource, Default)]
pub(super) struct TelegraphViews {
    drawn: HashMap<u32, Drawn>,
}

fn red(alpha: f32) -> Color {
    Color::srgba(0.95, 0.12, 0.08, alpha)
}

/// The mesh of a telegraph's shape: `points` on the ground of the client's own terrain (`height`
/// gives each its height, the host's floor where the terrain has no tile), floating [`LIFT`] over.
fn ground_mesh(
    points: &[V2],
    z: f32,
    height: impl Fn(Vec3) -> Option<f32>,
    tris: Vec<u32>,
) -> Mesh {
    let positions: Vec<[f32; 3]> = points
        .iter()
        .map(|p| {
            let mut b = units::wow_to_bevy([p.x, p.y, z]);
            b.y = enemies::draw_height(z, height(b)) + LIFT;
            b.to_array()
        })
        .collect();
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(tris))
}

/// Every telegraph in the world gets a shape on the ground, made once (its points on the
/// client's own terrain) and glowing as it nears; one that landed or went (its caster died, was
/// stunned or sapped) is despawned the frame it leaves the world's list. Shapes are
/// [`NightfallEntity`]s, so a run's end takes them too.
pub(super) fn draw(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut views: ResMut<TelegraphViews>,
    shapes: Query<(), With<TelegraphShape>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A shape that went with the last run is made again if its telegraph is still there.
    views.drawn.retain(|_, d| shapes.contains(d.entity));
    for t in &sim.world.telegraphs {
        let alpha = glow(t.left, t.total);
        if let Some(d) = views.drawn.get_mut(&t.id) {
            // Only the glow moves: the mesh stays as it was made.
            if (d.alpha - alpha).abs() >= GLOW_STEP {
                if let Some(m) = materials.get_mut(&d.material) {
                    m.base_color = red(alpha);
                    d.alpha = alpha;
                }
            }
            continue;
        }
        let (points, tris) = ground_grid(t.shape, t.at.ground(), t.facing);
        let mesh = ground_mesh(
            &points,
            t.at.z,
            |b| world_point.terrain_height_under(b),
            tris,
        );
        let material = materials.add(StandardMaterial {
            base_color: red(alpha),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                Visibility::default(),
                TelegraphShape,
                NightfallEntity,
            ))
            .id();
        views.drawn.insert(
            t.id,
            Drawn {
                entity,
                material,
                alpha,
            },
        );
    }
    let live: HashSet<u32> = sim.world.telegraphs.iter().map(|t| t.id).collect();
    views.drawn.retain(|id, d| {
        let keep = live.contains(id);
        if !keep {
            commands.entity(d.entity).despawn();
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{Telegraph, V3};

    #[test]
    fn a_circle_is_rings_of_points_around_its_centre() {
        let at = V2::new(10.0, 20.0);
        let (points, tris) = ground_grid(Shape::Circle { radius: 8.0 }, at, V2::new(1.0, 0.0));
        assert_eq!(points.len(), 1 + RINGS * SEGMENTS);
        assert_eq!(tris.len(), 3 * SEGMENTS * (1 + 2 * (RINGS - 1)));
        assert!(tris.iter().all(|&i| (i as usize) < points.len()));
        let outer = points.last().unwrap().distance(at);
        assert!((outer - 8.0).abs() < 1e-3);
    }

    #[test]
    fn a_cone_spreads_either_side_of_its_facing() {
        let at = V2::ZERO;
        let (points, tris) = ground_grid(
            Shape::Cone {
                length: 8.0,
                angle: 120.0,
            },
            at,
            V2::new(1.0, 0.0),
        );
        assert_eq!(points.len(), 1 + RINGS * (SEGMENTS + 1));
        assert!(tris.iter().all(|&i| (i as usize) < points.len()));
        // Every point lies within 60° of the facing.
        assert!(points[1..]
            .iter()
            .all(|p| p.normalize_or_zero().x >= 0.5 - 1e-4));
    }

    #[test]
    fn the_glow_grows_as_the_blow_nears() {
        assert!((glow(1.5, 1.5) - 0.15).abs() < 1e-6);
        assert!((glow(0.0, 1.5) - 0.65).abs() < 1e-6);
        assert!(glow(0.5, 1.5) > glow(1.0, 1.5));
    }

    #[test]
    fn a_shape_over_a_missing_tile_rests_at_the_hosts_floor() {
        let (points, tris) = ground_grid(
            Shape::Circle { radius: 4.0 },
            V2::new(3.0, 4.0),
            V2::new(1.0, 0.0),
        );
        let mesh = ground_mesh(&points, 12.0, |_| None, tris);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .expect("positions");
        assert_eq!(positions.len(), points.len());
        assert!(positions
            .iter()
            .all(|p| (p[1] - (12.0 + LIFT)).abs() < 1e-4));
    }

    #[test]
    fn a_shape_follows_the_client_terrain_where_it_is_the_hosts_floor() {
        let (points, tris) = ground_grid(
            Shape::Circle { radius: 4.0 },
            V2::new(3.0, 4.0),
            V2::new(1.0, 0.0),
        );
        let mesh = ground_mesh(&points, 12.0, |_| Some(12.5), tris);
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .unwrap();
        assert!(positions
            .iter()
            .all(|p| (p[1] - (12.5 + LIFT)).abs() < 1e-4));
    }

    /// A world with a hosted run (no terrain, no camera), the assets `draw` makes shapes in, and
    /// a telegraph for each of `ids`.
    fn stage(ids: &[u32]) -> bevy::app::App {
        let mut app = bevy::app::App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Mesh>().init_asset::<StandardMaterial>();
        benilla_world::world_point::init_world_point_resources(app.world_mut());
        app.init_resource::<TelegraphViews>();
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        for &id in ids {
            sim.telegraphs.push(telegraph(id, 1.5));
        }
        app.insert_resource(Sim::for_test(sim));
        app
    }

    fn telegraph(id: u32, left: f32) -> Telegraph {
        Telegraph {
            id,
            enemy: 1,
            ability: 0,
            shape: Shape::Circle { radius: 5.0 },
            at: V3::new(10.0, 20.0, 0.0),
            facing: V2::new(1.0, 0.0),
            left,
            total: 1.5,
        }
    }

    fn run_draw(app: &mut bevy::app::App) {
        use bevy::ecs::system::RunSystemOnce;
        app.world_mut().run_system_once(draw).unwrap();
    }

    fn alpha_of(app: &bevy::app::App, entity: Entity) -> f32 {
        let handle = app
            .world()
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .unwrap();
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(&handle.0)
            .unwrap()
            .base_color
            .alpha()
    }

    #[test]
    fn a_telegraph_is_drawn_once_and_only_its_glow_changes() {
        let mut app = stage(&[4]);
        run_draw(&mut app);
        let entity = app.world().resource::<TelegraphViews>().drawn[&4].entity;
        assert!(app.world().get::<NightfallEntity>(entity).is_some());
        assert!(app.world().get::<Mesh3d>(entity).is_some());
        let mesh = app.world().get::<Mesh3d>(entity).unwrap().0.clone();
        assert!((alpha_of(&app, entity) - glow(1.5, 1.5)).abs() < 1e-4);

        // The blow draws near: the same mesh, the same entity, a brighter shape.
        app.world_mut().resource_mut::<Sim>().world.telegraphs[0].left = 0.3;
        run_draw(&mut app);
        assert_eq!(
            app.world().resource::<TelegraphViews>().drawn[&4].entity,
            entity
        );
        assert_eq!(app.world().get::<Mesh3d>(entity).unwrap().0, mesh);
        assert_eq!(
            app.world().resource::<Assets<Mesh>>().len(),
            1,
            "one mesh, ever"
        );
        assert!((alpha_of(&app, entity) - glow(0.3, 1.5)).abs() < 1e-4);

        // A telegraph that did not move leaves the material alone.
        app.world_mut().clear_trackers();
        run_draw(&mut app);
        assert!(!app
            .world()
            .is_resource_changed::<Assets<StandardMaterial>>());
    }

    #[test]
    fn a_telegraph_that_landed_or_was_taken_down_is_despawned_the_frame_it_leaves() {
        let mut app = stage(&[4, 5, 6]);
        run_draw(&mut app);
        let views = app.world().resource::<TelegraphViews>();
        let (e4, e5, e6) = (
            views.drawn[&4].entity,
            views.drawn[&5].entity,
            views.drawn[&6].entity,
        );
        // 4 landed; 6's caster died, was stunned or sapped.
        app.world_mut()
            .resource_mut::<Sim>()
            .world
            .telegraphs
            .retain(|t| t.id == 5);
        run_draw(&mut app);
        let world = app.world();
        assert!(world.get_entity(e4).is_err(), "landed: gone");
        assert!(world.get_entity(e6).is_err(), "taken down: gone");
        assert!(world.get_entity(e5).is_ok(), "still coming: kept");
        let views = world.resource::<TelegraphViews>();
        assert_eq!(views.drawn.keys().copied().collect::<Vec<_>>(), vec![5]);
    }

    #[test]
    fn a_shape_that_went_with_the_run_is_forgotten_and_a_new_one_drawn_if_still_due() {
        let mut app = stage(&[4]);
        run_draw(&mut app);
        let first = app.world().resource::<TelegraphViews>().drawn[&4].entity;
        app.world_mut().despawn(first);
        run_draw(&mut app);
        let second = app.world().resource::<TelegraphViews>().drawn[&4].entity;
        assert_ne!(first, second);
        assert!(app.world().get_entity(second).is_ok());

        // And with no telegraph left there is nothing to remember.
        app.world_mut().despawn(second);
        app.world_mut()
            .resource_mut::<Sim>()
            .world
            .telegraphs
            .clear();
        run_draw(&mut app);
        assert!(app.world().resource::<TelegraphViews>().drawn.is_empty());
    }
}
