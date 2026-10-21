//! Where things are: the minimap's marks (the boss, camps' rares and elites, a banner's
//! destination, caches, and for a dwarf's Treasure Finding every chest, shrine and pot), the
//! arena's edge on the minimap and as a glowing fence on the ground, each rare camp's leash as a
//! ring on the ground, and each shrine's circle on the ground, filling as it is stood in.

use std::collections::HashMap;

use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use together_sim::{InteractState, InteractableKind, ESCORT_GOAL, V2};

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
    let blip = |p: V2, shape, size_px, color, rim, name: String| ExtraBlip {
        pos: [p.x, p.y],
        shape,
        size_px,
        color,
        rim,
        name,
    };
    let white = [1.0, 1.0, 1.0, 1.0];
    let treasure = world
        .hero(sim.me)
        .and_then(|h| content.race(h.race))
        .is_some_and(|r| r.treasure());
    for it in &world.interactables {
        let name = it.kind.name().to_string();
        match (&it.state, it.kind) {
            (InteractState::Escort { .. }, _) => {
                // Marked at its destination, below.
            }
            (InteractState::Ready, InteractableKind::Cache) => {
                blips.push(blip(it.pos, ExtraShape::GoldDot, 11.0, white, true, name));
            }
            (InteractState::Ready, InteractableKind::WarBanner) => {
                let green = [0.7, 1.0, 0.7, 1.0];
                blips.push(blip(it.pos, ExtraShape::Flag, 11.0, green, false, name));
            }
            (InteractState::Ready, _) if treasure => {
                let size = if it.kind == InteractableKind::Pot {
                    6.0
                } else {
                    9.0
                };
                blips.push(blip(it.pos, ExtraShape::GoldDot, size, white, false, name));
            }
            _ => {}
        }
        if let InteractState::Escort { to, .. } = it.state {
            let name = "War Banner: carry it here".to_string();
            blips.push(blip(to, ExtraShape::Flag, 16.0, white, true, name));
        }
    }
    for e in &world.enemies {
        let kind = &content.enemies[e.def].name;
        if e.boss {
            let name = format!("{kind} (boss)");
            blips.push(blip(e.pos, ExtraShape::Skull, 16.0, white, true, name));
        } else if world.guards.contains(&e.id) {
            let name = format!("{kind} (rare)");
            let silver = [0.8, 0.8, 1.0, 1.0];
            blips.push(blip(e.pos, ExtraShape::RedDot, 12.0, silver, false, name));
        } else if e.elite {
            let name = format!("{kind} (elite)");
            blips.push(blip(e.pos, ExtraShape::RedDot, 9.0, white, false, name));
        }
    }
    extras.blips = blips;
    let mut rings = vec![ExtraRing {
        center: [center.x, center.y],
        radius: world.zone().leash,
        width_px: 2.0,
        color: [1.0, 0.35, 0.15, 0.85],
    }];
    // Each rare camp's leash, while its rare stands.
    for camp in &world.camps {
        let guarded = world.enemies.iter().any(|e| {
            world.guards.contains(&e.id)
                && e.pos.distance(camp.pos) <= together_sim::CAMP_LEASH * 1.5
        });
        if guarded {
            rings.push(ExtraRing {
                center: [camp.pos.x, camp.pos.y],
                radius: together_sim::CAMP_LEASH,
                width_px: 1.0,
                color: [0.75, 0.75, 1.0, 0.6],
            });
        }
    }
    extras.rings = rings;
}

/// Nothing on the minimap once the run is over.
pub(super) fn clear_minimap(mut extras: ResMut<MinimapExtras>) {
    *extras = MinimapExtras::default();
}

/// The arena's edge on the ground: a low fence of light that follows the terrain, fading upward.
/// Built from what terrain has loaded and rebuilt every few seconds while some is missing (the
/// far side of a big arena streams in only as a hero nears it), then twice more as finer terrain
/// comes.
#[derive(Component)]
pub(super) struct Fence;

#[derive(Default)]
pub(super) struct FenceState {
    /// Builds since the fence was last whole, and when to try again.
    whole_builds: u32,
    next_try: f32,
    built: bool,
}

/// Points round a circle at `center` (`radius` yards, `segments` steps), each on the terrain under
/// it, or `None` where none has loaded yet.
fn ring_feet(
    world_point: &WorldPoint,
    center: V2,
    radius: f32,
    segments: usize,
    fallback: f32,
) -> Vec<Option<Vec3>> {
    (0..=segments)
        .map(|k| {
            let a = k as f32 / segments as f32 * std::f32::consts::TAU;
            let at = units::ground_to_bevy(center + V2::from_angle(a, radius), fallback);
            world_point
                .terrain_height_under(at)
                .map(|h| Vec3::new(at.x, h, at.z))
        })
        .collect()
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
    if fences.is_empty() && state.built {
        *state = FenceState::default();
    }
    let now = time.elapsed_secs();
    if state.whole_builds >= 3 || now < state.next_try {
        return;
    }
    let world = &sim.world;
    let feet = ring_feet(
        &world_point,
        world.center(),
        world.zone().leash,
        FENCE_SEGMENTS,
        world.zone().center[2],
    );
    let whole = feet.iter().all(Option::is_some);
    state.next_try = now + if whole { 8.0 } else { 3.0 };
    if whole {
        state.whole_builds += 1;
    }
    if feet.iter().filter(|f| f.is_some()).count() < 2 {
        return;
    }
    for e in &fences {
        commands.entity(e).despawn();
    }
    let material = band_material(&mut materials);
    commands.spawn((
        Mesh3d(meshes.add(band_mesh(&feet, FENCE_HEIGHT, FENCE_COLOR))),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Fence,
        TogetherEntity,
    ));
    state.built = true;
}

/// The material a band of light wears: its colour in its vertices, unlit, blended, seen from
/// both sides.
fn band_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        depth_bias: 20.0,
        ..default()
    })
}

/// A band standing on `feet`, `height` tall, `color` at the foot fading to nothing at the top;
/// a gap where a foot is not known.
fn band_mesh(feet: &[Option<Vec3>], height: f32, color: [f32; 4]) -> Mesh {
    let mut positions = Vec::with_capacity(feet.len() * 2);
    let mut colors = Vec::with_capacity(feet.len() * 2);
    let mut indices = Vec::with_capacity(feet.len() * 6);
    let top = [color[0], color[1], color[2], 0.0];
    for f in feet {
        let f = f.unwrap_or(Vec3::ZERO);
        positions.push([f.x, f.y - 0.2, f.z]);
        positions.push([f.x, f.y + height, f.z]);
        colors.push(color);
        colors.push(top);
    }
    for k in 0..feet.len().saturating_sub(1) {
        if feet[k].is_none() || feet[k + 1].is_none() {
            continue;
        }
        let i = (k * 2) as u32;
        indices.extend_from_slice(&[i, i + 1, i + 2, i + 1, i + 3, i + 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// A camp's leash on the ground: a low band of light round the rare's camp, as far as it will
/// follow a hero before it gives up and runs home. Shown while a hero is near the camp or its
/// rare is in a fight.
#[derive(Component)]
pub(super) struct LeashRing;

/// Segments in a leash ring, its height and colour at the foot, and how near a hero must come to
/// a camp's edge for its ring to show.
const LEASH_SEGMENTS: usize = 120;
const LEASH_HEIGHT: f32 = 0.7;
const LEASH_COLOR: [f32; 4] = [0.75, 0.75, 1.0, 0.45];
const LEASH_SHOW_WITHIN: f32 = 20.0;

pub(super) struct LeashShown {
    entity: Entity,
    whole: bool,
    next_try: f32,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn leash_rings(
    mut commands: Commands,
    sim: Res<Sim>,
    world_point: WorldPoint,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rings: Query<(), With<LeashRing>>,
    time: Res<Time<Real>>,
    mut shown: Local<HashMap<usize, LeashShown>>,
) {
    // A new run: the last one's rings went with it.
    if rings.is_empty() {
        shown.clear();
    }
    let world = &sim.world;
    let now = time.elapsed_secs();
    let me = world.hero(sim.me).map(|h| h.pos);
    let leash = together_sim::CAMP_LEASH;
    for (c, camp) in world.camps.iter().enumerate() {
        // The camp's rare, if it stands: in a fight when hurt.
        let rare = world
            .enemies
            .iter()
            .find(|e| world.guards.contains(&e.id) && e.pos.distance(camp.pos) <= leash * 1.5);
        let fighting = rare.is_some_and(|e| e.hp < e.max_hp);
        let near = me.is_some_and(|p| p.distance(camp.pos) <= leash + LEASH_SHOW_WITHIN);
        let want = rare.is_some() && (fighting || near);
        if !want {
            if let Some(s) = shown.remove(&c) {
                commands.entity(s.entity).despawn();
            }
            continue;
        }
        if shown.get(&c).is_some_and(|s| s.whole || now < s.next_try) {
            continue;
        }
        let feet = ring_feet(
            &world_point,
            camp.pos,
            leash,
            LEASH_SEGMENTS,
            world.zone().center[2],
        );
        let whole = feet.iter().all(Option::is_some);
        if let Some(s) = shown.remove(&c) {
            commands.entity(s.entity).despawn();
        }
        let material = band_material(&mut materials);
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(band_mesh(&feet, LEASH_HEIGHT, LEASH_COLOR))),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                LeashRing,
                TogetherEntity,
            ))
            .id();
        shown.insert(
            c,
            LeashShown {
                entity,
                whole,
                next_try: now + 2.0,
            },
        );
    }
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
