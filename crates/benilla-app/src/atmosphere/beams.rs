//! Sunbeams in the air around the player: long, faint, slanted strips of light, each lying along
//! the sunlight from the ground up toward the sun and turned about its own length to face the
//! camera, so they read from the game's camera looking down, where screen-space shafts need the
//! sun in view. Each fades in and out over several seconds somewhere new, like light through
//! moving leaves. Additive and unlit; strongest with a low sun, gone at night, in rain and
//! under an overcast sky.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use benilla_world::lighting::WowLighting;
use benilla_world::view::WorldCamera;
use benilla_world::weather::{storm_blend, WeatherState};
use benilla_world::world_point::WorldPoint;
use benilla_world::world_unit::ViewerUnit;

use super::{AtmosphereSettings, SkyRequest};

/// Beams at once.
const BEAMS: usize = 10;
/// How far from the player beams stand, yards.
const RADIUS: f32 = 32.0;
/// A beam's peak opacity (additive).
const OPACITY: f32 = 0.3;

#[derive(Resource)]
struct BeamMesh(Handle<Mesh>);

/// One beam: its foot, size and life.
#[derive(Component)]
struct Beam {
    foot: Vec3,
    width: f32,
    length: f32,
    born: f32,
    life: f32,
    material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct BeamState {
    rng: u64,
    /// The last time a beam was placed, so they come one at a time.
    last: f32,
}

impl BeamState {
    fn roll(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.rng >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// The unit beam: a strip along +Y from 0 to 1, one wide across X, facing +Z; three columns
/// across (dark edge, bright middle, dark edge), fading toward both ends, faintest at the top.
fn beam_mesh() -> Mesh {
    let rows = [(0.0, 0.0), (0.12, 1.0), (0.55, 0.7), (1.0, 0.0)];
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for (y, a) in rows {
        for (x, edge) in [(-0.5, 0.0), (0.0, 1.0), (0.5, 0.0)] {
            positions.push([x, y, 0.0]);
            colors.push([1.0, 1.0, 1.0, a * edge]);
        }
    }
    let mut indices = Vec::new();
    for r in 0..3u32 {
        for c in 0..2u32 {
            let i = r * 3 + c;
            indices.extend([i, i + 1, i + 4, i, i + 4, i + 3]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

/// A beam's opacity `t` seconds into a `life`: in over a third, out over a third.
fn fade(t: f32, life: f32) -> f32 {
    let edge = life / 3.0;
    (t / edge).min((life - t) / edge).clamp(0.0, 1.0)
}

/// How strong the beams are from the sun's height above the horizon (`up`, the sine): none
/// below 3 degrees, most with a low sun, a little less at noon.
fn sun_height_gain(up: f32) -> f32 {
    let rise = ((up - 0.05) / 0.12).clamp(0.0, 1.0);
    rise * (1.0 - 0.45 * ((up - 0.3) / 0.6).clamp(0.0, 1.0))
}

/// The rotation that lays the unit beam along `axis` and turns its face toward `to_cam`.
fn facing(axis: Vec3, to_cam: Vec3) -> Quat {
    let y = axis.normalize_or_zero();
    let z = (to_cam - y * to_cam.dot(y)).try_normalize().unwrap_or(Vec3::Z);
    let x = y.cross(z);
    Quat::from_mat3(&Mat3::from_cols(x, y, z))
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(BeamMesh(meshes.add(beam_mesh())));
    commands.insert_resource(BeamState { rng: 0xBEA_A5, last: 0.0 });
}

fn drive(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<AtmosphereSettings>,
    req: Res<SkyRequest>,
    lighting: Res<WowLighting>,
    weather: Option<Res<WeatherState>>,
    mesh: Option<Res<BeamMesh>>,
    mut state: ResMut<BeamState>,
    focus: Query<&GlobalTransform, (With<ViewerUnit>, Without<Beam>)>,
    cam: Query<&GlobalTransform, (With<WorldCamera>, Without<Beam>)>,
    world_point: WorldPoint,
    mut beams: Query<(Entity, &Beam, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next_dump: Local<f32>,
) {
    let Some(mesh) = mesh else { return };
    let sun = lighting.celestial_dir().normalize_or_zero();
    let (wet, storm) = weather.map_or((0.0, 0.0), |w| (w.effect_density, storm_blend(w.sky_density)));
    let gain = if settings.beams && req.active {
        lighting.sun_day() * sun_height_gain(sun.y) * (1.0 - wet) * (1.0 - storm)
    } else {
        0.0
    };
    let eye = cam.iter().next().map(GlobalTransform::translation);
    let center = focus.iter().next().map(GlobalTransform::translation).or(eye);
    let now = time.elapsed_secs();
    // The sunlight's hue, at full brightness.
    let d = lighting.diffuse;
    let top = d[0].max(d[1]).max(d[2]).max(1e-3);
    let hue = Vec3::new(d[0], d[1] * 0.97, d[2] * 0.88) / top;

    let mut alive = 0;
    for (e, beam, mut tf) in &mut beams {
        let t = now - beam.born;
        let far = center.is_none_or(|c| (beam.foot - c).xz().length() > RADIUS * 1.6);
        if t > beam.life || far || gain <= 0.0 {
            materials.remove(&beam.material);
            commands.entity(e).despawn();
            continue;
        }
        alive += 1;
        if let Some(eye) = eye {
            let mid = beam.foot + sun * beam.length * 0.5;
            *tf = Transform::from_translation(beam.foot)
                .with_rotation(facing(sun, eye - mid))
                .with_scale(Vec3::new(beam.width, beam.length, 1.0));
        }
        if let Some(m) = materials.get_mut(&beam.material) {
            let a = (OPACITY * settings.beam_strength * gain * fade(t, beam.life)).min(1.0);
            m.base_color = Color::srgba(hue.x, hue.y, hue.z, a);
        }
    }

    if super::dump_due(now, &mut next_dump) {
        let ground = center.and_then(|c| world_point.terrain_height_under(c + Vec3::Y * 200.0));
        info!(
            "atmos beams: {} alive, amount {:.2}, focus {:?}, ground {:?}",
            alive, gain, center, ground
        );
    }
    let Some(center) = center else { return };
    if gain <= 0.0 || alive >= BEAMS || now - state.last < 0.7 {
        return;
    }
    let angle = state.roll() * std::f32::consts::TAU;
    let r = RADIUS * state.roll().sqrt();
    let at = center + Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
    let Some(ground) = world_point.terrain_height_under(at + Vec3::Y * 200.0) else {
        return;
    };
    state.last = now;
    let width = 2.0 + 4.0 * state.roll();
    let length = 30.0 + 25.0 * state.roll();
    let life = 6.0 + 6.0 * state.roll();
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(hue.x, hue.y, hue.z, 0.0),
        unlit: true,
        fog_enabled: false,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        ..default()
    });
    let foot = Vec3::new(at.x, ground - 0.5, at.z);
    commands.spawn((
        Mesh3d(mesh.0.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(foot).with_scale(Vec3::ZERO),
        Beam {
            foot,
            width,
            length,
            born: now,
            life,
            material,
        },
    ));
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(Update, drive);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beam_lies_along_the_sun_and_faces_the_camera() {
        let sun = Vec3::new(-0.66, 0.34, -0.66).normalize();
        let to_cam = Vec3::new(1.0, 0.5, 0.2);
        let q = facing(sun, to_cam);
        assert!((q * Vec3::Y - sun).length() < 1e-4);
        let face = q * Vec3::Z;
        assert!(face.dot(sun).abs() < 1e-4);
        assert!(face.dot(to_cam) > 0.0);
    }

    #[test]
    fn beams_need_a_risen_sun_and_like_it_low() {
        assert_eq!(sun_height_gain(0.0), 0.0);
        assert!(sun_height_gain(0.3) > sun_height_gain(0.95));
        assert!(sun_height_gain(0.95) > 0.4);
        assert_eq!(fade(0.0, 9.0), 0.0);
        assert_eq!(fade(4.5, 9.0), 1.0);
    }

    #[test]
    fn the_unit_beam_spans_zero_to_one() {
        let m = beam_mesh();
        assert_eq!(m.count_vertices(), 12);
    }
}