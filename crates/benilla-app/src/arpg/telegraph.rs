//! Enemy telegraphs on the ground: a red edge where the blow will land and a fill that grows
//! until it does. Plus a small ring under the cursor, where the next ability goes.

use std::collections::HashMap;

use arpg_sim::{Telegraph, V2};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::camera::AimPoint;
use super::{RunEntity, Sim};

/// Above the ground, against z-fighting.
const LIFT: f32 = 0.08;
const EDGE: f32 = 0.15;

/// The drawn telegraph of each enemy winding up: its edge and its fill, and what shape they are.
#[derive(Resource, Default)]
pub(super) struct Shown {
    by_enemy: HashMap<u32, (Entity, Entity, Kind)>,
    cursor: Option<Entity>,
    looks: Option<Looks>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Arc,
    Circle,
    Line,
}

struct Looks {
    fill: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
    cursor: Handle<StandardMaterial>,
}

impl Looks {
    fn new(materials: &mut Assets<StandardMaterial>) -> Self {
        let mut flat = |color: Color| {
            materials.add(StandardMaterial {
                base_color: color,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                depth_bias: 50.0,
                ..default()
            })
        };
        Self {
            fill: flat(Color::srgba(1.0, 0.15, 0.1, 0.35)),
            edge: flat(Color::srgba(1.0, 0.3, 0.2, 0.9)),
            cursor: flat(Color::srgba(0.6, 0.85, 1.0, 0.7)),
        }
    }
}

fn kind_of(t: &Telegraph) -> Kind {
    match t {
        Telegraph::Arc { .. } => Kind::Arc,
        Telegraph::Circle { .. } => Kind::Circle,
        Telegraph::Line { .. } => Kind::Line,
    }
}

/// Lying flat, facing up.
fn flat() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

pub(super) fn show(
    mut commands: Commands,
    sim: Res<Sim>,
    aim: Res<AimPoint>,
    mut shown: ResMut<Shown>,
    terrain: WorldPoint,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tfs: Query<&mut Transform>,
) {
    let world = &sim.world;
    let shown = &mut *shown;
    let looks = shown
        .looks
        .get_or_insert_with(|| Looks::new(&mut materials));
    let fy = world.content().zones[world.zone()].start.2;
    let on_ground = |p: V2| {
        let mut at = super::view::ground(p, fy);
        if let Some(h) = terrain.terrain_height_under(at + Vec3::Y * 50.0) {
            at.y = h;
        }
        at + Vec3::Y * LIFT
    };

    let mut live = Vec::new();
    for enemy in world.enemies() {
        let Some(t) = world.telegraph(enemy) else {
            continue;
        };
        live.push(enemy.id);
        let kind = kind_of(&t);
        if shown.by_enemy.get(&enemy.id).is_some_and(|s| s.2 != kind) {
            if let Some((a, b, _)) = shown.by_enemy.remove(&enemy.id) {
                commands.entity(a).despawn();
                commands.entity(b).despawn();
            }
        }
        let parts = *shown.by_enemy.entry(enemy.id).or_insert_with(|| {
            let (edge, fill): (Mesh, Mesh) = match t {
                Telegraph::Arc { radius, angle, .. } => (
                    CircularSector::new(radius, angle / 2.0).into(),
                    CircularSector::new(radius, angle / 2.0).into(),
                ),
                Telegraph::Circle { radius, .. } => (
                    Annulus::new((radius - EDGE).max(0.05), radius).into(),
                    Circle::new(radius).into(),
                ),
                Telegraph::Line { from, to, .. } => {
                    let len = from.distance(to).max(0.1);
                    (Rectangle::new(len, EDGE).into(), Rectangle::new(len, 1.0).into())
                }
            };
            // An arc's edge is the whole wedge, faint; the fill is the stronger part growing.
            let edge_look = if kind == Kind::Arc {
                looks.fill.clone()
            } else {
                looks.edge.clone()
            };
            let fill_look = if kind == Kind::Arc {
                looks.edge.clone()
            } else {
                looks.fill.clone()
            };
            let edge = commands
                .spawn((
                    Mesh3d(meshes.add(edge)),
                    MeshMaterial3d(edge_look),
                    Transform::default(),
                    RunEntity,
                ))
                .id();
            let fill = commands
                .spawn((
                    Mesh3d(meshes.add(fill)),
                    MeshMaterial3d(fill_look),
                    Transform::default(),
                    RunEntity,
                ))
                .id();
            (edge, fill, kind)
        });
        let (edge, fill, _) = parts;
        let (edge_tf, fill_tf) = match t {
            Telegraph::Arc {
                from,
                dir,
                progress,
                ..
            } => {
                let at = on_ground(from);
                let turn = Quat::from_rotation_y(super::units::yaw_toward(super::view::ground(
                    dir, 0.0,
                ))) * flat();
                (
                    Transform::from_translation(at).with_rotation(turn),
                    Transform::from_translation(at + Vec3::Y * 0.01)
                        .with_rotation(turn)
                        .with_scale(Vec3::splat(progress.max(0.05))),
                )
            }
            Telegraph::Circle { at, progress, .. } => {
                let at = on_ground(at);
                (
                    Transform::from_translation(at).with_rotation(flat()),
                    Transform::from_translation(at - Vec3::Y * 0.01)
                        .with_rotation(flat())
                        .with_scale(Vec3::splat(progress.max(0.05))),
                )
            }
            Telegraph::Line { from, to, progress } => {
                let a = on_ground(from);
                let b = on_ground(to);
                let d = b - a;
                let yaw = (-d.z).atan2(d.x);
                let turn = Quat::from_rotation_y(yaw) * flat();
                let width = 1.2;
                (
                    Transform::from_translation((a + b) * 0.5).with_rotation(turn),
                    Transform::from_translation((a + b) * 0.5)
                        .with_rotation(turn)
                        .with_scale(Vec3::new(1.0, width * progress.max(0.05), 1.0)),
                )
            }
        };
        if let Ok(mut tf) = tfs.get_mut(edge) {
            *tf = edge_tf;
        }
        if let Ok(mut tf) = tfs.get_mut(fill) {
            *tf = fill_tf;
        }
    }
    shown.by_enemy.retain(|id, (a, b, _)| {
        let keep = live.contains(id);
        if !keep {
            commands.entity(*a).despawn();
            commands.entity(*b).despawn();
        }
        keep
    });

    // The cursor ring.
    let cursor = *shown.cursor.get_or_insert_with(|| {
        commands
            .spawn((
                Mesh3d(meshes.add(Annulus::new(0.45, 0.6))),
                MeshMaterial3d(looks.cursor.clone()),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id()
    });
    if let Ok(mut tf) = tfs.get_mut(cursor) {
        if let Some(p) = aim.0 {
            *tf = Transform::from_translation(on_ground(super::view::to_ground(p)))
                .with_rotation(flat());
        }
    }
    commands.entity(cursor).insert(if aim.0.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}
