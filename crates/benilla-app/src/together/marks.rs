//! Where things are: the minimap's marks (the boss, champions and elites, a banner's
//! destination, caches, and for a dwarf's Treasure Finding every chest, shrine and pot), the
//! arena's edge on the minimap and as a glowing fence on the ground, and each shrine's circle on
//! the ground, filling as it is stood in.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use benilla_world::world_point::WorldPoint;
use together_sim::{InteractState, InteractableKind, V2, ESCORT_GOAL};

use crate::minimap::{ExtraBlip, ExtraRing, ExtraShape, MinimapExtras};

use super::{units, Sim, TogetherEntity};

/// Segments in the ground fence, and its height and colour at the foot (it fades upward).
const FENCE_SEGMENTS: usize = 360;
const FENCE_HEIGHT: f32 = 1.6;
const FENCE_COLOR: [f32; 4] = [1.0, 0.35, 0.15, 0.55];
/// How far above the ground a shrine's circle lies, and its edge's width.
const LIFT: f32 = 0.1;
const EDGE: f32 = 0.18;

/// The minimap's marks for this frame, from the world.
pub(super) fn fill_minimap(sim: Res<Sim>, mut extras: ResMut<MinimapExtras>) {
    let world = &sim.world;
    let content = world.content();
    let center = world.center();
    let mut blips = Vec::new();
    let blip = |p: V2, shape, size_px, color, rim| ExtraBlip {
        pos: [p.x, p.y],
        shape,
        size_px,
        color,
        rim,
    };
    let white = [1.0, 1.0, 1.0, 1.0];
    let treasure = world
        .hero(sim.me)
        .and_then(|h| content.race(h.race))
        .is_some_and(|r| r.treasure());
    for it in &world.interactables {
        match (&it.state, it.kind) {
            (InteractState::Escort { to, .. }, _) => {
                blips.push(blip(*to, ExtraShape::Flag, 16.0, white, true));
            }
            (InteractState::Ready, InteractableKind::Cache) => {
                blips.push(blip(it.pos, ExtraShape::GoldDot, 11.0, white, true));
            }
            (InteractState::Ready, InteractableKind::WarBanner) => {
                blips.push(blip(it.pos, ExtraShape::Flag, 11.0, [0.7, 1.0, 0.7, 1.0], false));
            }
            (InteractState::Ready, _) if treasure => {
                let size = if it.kind == InteractableKind::Pot { 6.0 } else { 9.0 };
                blips.push(blip(it.pos, ExtraShape::GoldDot, size, white, false));
            }
            _ => {}
        }
    }
    for e in &world.enemies {
        if e.boss {
            blips.push(blip(e.pos, ExtraShape::Skull, 16.0, white, true));
        } else if world.champions.contains(&e.id) {
            blips.push(blip(e.pos, ExtraShape::RedDot, 14.0, [1.0, 0.75, 0.2, 1.0], true));
        } else if e.elite {
            blips.push(blip(e.pos, ExtraShape::RedDot, 9.0, white, false));
        }
    }
    extras.blips = blips;
    extras.rings = vec![ExtraRing {
        center: [center.x, center.y],
        radius: world.zone().leash,
        width_px: 2.0,
        color: [1.0, 0.35, 0.15, 0.85],
    }];
}

/// Nothing on the minimap once the run is over.
pub(super) fn clear_minimap(mut extras: ResMut<MinimapExtras>) {
    *extras = MinimapExtras::default();
}

/// The arena's edge on the ground: a low fence of light that follows the terrain, fading upward.
/// Built once the terrain under all of it has loaded, and again a few times as finer terrain
/// streams in.
#[derive(Component)]
pub(super) struct Fence;

#[derive(Default)]
pub(super) struct FenceState {
    built: u32,
    next_try: f32,
}

pub(super) fn build_fence(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    fences: Query<Entity, With<Fence>>,
    time: Res<Time<Real>>,
    mut state: Local<FenceState>,
) {
    // A fresh run: nothing of the last one's fence is left.
    if fences.is_empty() && state.built > 0 {
        *state = FenceState::default();
    }
    let now = time.elapsed_secs();
    if state.built >= 4 || now < state.next_try {
        return;
    }
    state.next_try = now + if state.built == 0 { 1.0 } else { 8.0 };
    let world = &sim.world;
    let center = world.center();
    let leash = world.zone().leash;
    let fallback = world.zone().center[2];
    let mut feet = Vec::with_capacity(FENCE_SEGMENTS + 1);
    for k in 0..=FENCE_SEGMENTS {
        let a = k as f32 / FENCE_SEGMENTS as f32 * std::f32::consts::TAU;
        let p = center + V2::from_angle(a, leash);
        let at = units::ground_to_bevy(p, fallback);
        let Some(h) = world_point.terrain_height_under(at + Vec3::Y * 200.0) else {
            // Not all of it has loaded yet.
            return;
        };
        feet.push(Vec3::new(at.x, h, at.z));
    }
    for e in &fences {
        commands.entity(e).despawn();
    }
    let mesh = fence_mesh(&feet);
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        depth_bias: 20.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Fence,
        TogetherEntity,
    ));
    state.built += 1;
}

/// A strip standing on `feet`, [`FENCE_HEIGHT`] tall, its colour fading to nothing at the top.
fn fence_mesh(feet: &[Vec3]) -> Mesh {
    let mut positions = Vec::with_capacity(feet.len() * 2);
    let mut colors = Vec::with_capacity(feet.len() * 2);
    let mut indices = Vec::with_capacity(feet.len() * 6);
    let top = [FENCE_COLOR[0], FENCE_COLOR[1], FENCE_COLOR[2], 0.0];
    for (k, f) in feet.iter().enumerate() {
        positions.push([f.x, f.y - 0.2, f.z]);
        positions.push([f.x, f.y + FENCE_HEIGHT, f.z]);
        colors.push(FENCE_COLOR);
        colors.push(top);
        if k + 1 < feet.len() {
            let i = (k * 2) as u32;
            indices.extend_from_slice(&[i, i + 1, i + 2, i + 1, i + 3, i + 2]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// A shrine's circle on the ground: its edge, pulsing gently, and a fill that grows with its
/// charge. One per interactable that is used by standing in it.
#[derive(Component)]
pub(super) struct Circle;

pub(super) struct CircleParts {
    edge: Entity,
    fill: Entity,
    material: Handle<StandardMaterial>,
}

/// The circles' colour for each kind.
fn circle_color(kind: InteractableKind) -> Option<Color> {
    Some(match kind {
        InteractableKind::TitanShrine => Color::srgb(0.35, 0.65, 1.0),
        InteractableKind::GoldenShrine | InteractableKind::Chest | InteractableKind::Cache => {
            Color::srgb(1.0, 0.82, 0.25)
        }
        InteractableKind::CursedAltar => Color::srgb(0.75, 0.3, 1.0),
        InteractableKind::ChallengeIdol => Color::srgb(1.0, 0.35, 0.25),
        InteractableKind::WarBanner => Color::srgb(0.4, 1.0, 0.45),
        InteractableKind::Pot => return None,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn shrine_circles(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut parts: Query<&mut Transform, With<Circle>>,
    time: Res<Time<Real>>,
    mut shown: Local<HashMap<u32, CircleParts>>,
) {
    // A new run: the last one's circles went with it.
    if parts.is_empty() {
        shown.clear();
    }
    let world = &sim.world;
    let fallback = world.zone().center[2];
    let pulse = 0.55 + 0.35 * (time.elapsed_secs() * 3.0).sin();
    let mut live = Vec::new();
    for it in &world.interactables {
        // Ready ones show their own circle; a banner on its way shows its destination's.
        let (pos, radius, charge) = match &it.state {
            InteractState::Ready => (it.pos, it.kind.radius(), it.charge),
            InteractState::Escort { to, .. } => (*to, ESCORT_GOAL, 0.0),
            _ => continue,
        };
        let Some(color) = circle_color(it.kind) else {
            continue;
        };
        live.push(it.id);
        let mut at = units::ground_to_bevy(pos, fallback);
        if let Some(h) = world_point.terrain_height_under(at + Vec3::Y * 50.0) {
            at.y = h;
        }
        let at = at + Vec3::Y * LIFT;
        let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let entry = shown.entry(it.id).or_insert_with(|| {
            let edge_mat = materials.add(StandardMaterial {
                base_color: color.with_alpha(0.8),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                depth_bias: 50.0,
                ..default()
            });
            let fill_mat = materials.add(StandardMaterial {
                base_color: color.with_alpha(0.28),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                depth_bias: 49.0,
                ..default()
            });
            let edge = commands
                .spawn((
                    Mesh3d(meshes.add(Annulus::new((radius - EDGE).max(0.05), radius))),
                    MeshMaterial3d(edge_mat.clone()),
                    Transform::from_translation(at).with_rotation(flat),
                    Circle,
                    TogetherEntity,
                ))
                .id();
            let fill = commands
                .spawn((
                    Mesh3d(meshes.add(bevy::math::primitives::Circle::new(radius))),
                    MeshMaterial3d(fill_mat),
                    Transform::from_translation(at - Vec3::Y * 0.01)
                        .with_rotation(flat)
                        .with_scale(Vec3::splat(0.001)),
                    Circle,
                    TogetherEntity,
                ))
                .id();
            CircleParts {
                edge,
                fill,
                material: edge_mat,
            }
        });
        if let Ok(mut tf) = parts.get_mut(entry.edge) {
            tf.translation = at;
        }
        if let Ok(mut tf) = parts.get_mut(entry.fill) {
            tf.translation = at - Vec3::Y * 0.01;
            tf.scale = Vec3::splat(charge.clamp(0.001, 1.0));
        }
        if let Some(m) = materials.get_mut(&entry.material) {
            m.base_color = color.with_alpha(pulse);
        }
    }
    shown.retain(|id, p| {
        let keep = live.contains(id);
        if !keep {
            for e in [p.edge, p.fill] {
                if let Ok(mut ec) = commands.get_entity(e) {
                    ec.despawn();
                }
            }
        }
        keep
    });
}
