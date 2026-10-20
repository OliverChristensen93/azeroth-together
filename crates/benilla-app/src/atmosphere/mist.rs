//! Ground mist: soft sheets of mist lying a yard or two over the ground around the player, each
//! drifting with the wind and fading in and out over half a minute, so the mist moves without
//! anything popping. A zone asks for its own (`SkyRequest::mist`) and rain thickens it. The
//! sheets are flat, so the camera looking down sees them as a veil the grass pokes through, and
//! from the side they thin to nothing where the distance fog takes over. The texture is 1.12's
//! own snow mist.

use bevy::prelude::*;

use benilla_world::lighting::WowLighting;
use benilla_world::view::WorldCamera;
use benilla_world::weather::{WeatherGust, WeatherKind, WeatherState};
use benilla_world::world_point::WorldPoint;
use benilla_world::world_unit::ViewerUnit;

use super::{AtmosphereSettings, SkyRequest};

/// Sheets at full mist.
const MAX_SHEETS: usize = 28;
/// How far from the player sheets lie, and where they are dropped, yards.
const RADIUS: f32 = 45.0;
const DROP: f32 = 65.0;
/// A sheet's peak opacity at full mist.
const OPACITY: f32 = 0.8;
/// How much rain thickens the mist at full rain.
const RAIN_MIST: f32 = 0.45;

#[derive(Resource)]
struct MistAssets {
    mesh: Handle<Mesh>,
    texture: Handle<Image>,
}

/// One sheet of mist.
#[derive(Component)]
struct Sheet {
    born: f32,
    life: f32,
    material: Handle<StandardMaterial>,
}

/// The mist's random stream and clock.
#[derive(Resource)]
struct MistState {
    rng: u64,
}

impl MistState {
    fn roll(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.rng >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// How thick the mist is: the zone's own, plus rain's.
fn amount(req: &SkyRequest, rain: f32) -> f32 {
    (req.mist + RAIN_MIST * rain).clamp(0.0, 1.0)
}

/// A sheet's opacity `t` seconds into a `life`: in over a quarter, out over a quarter.
fn fade(t: f32, life: f32) -> f32 {
    let edge = life * 0.25;
    (t / edge).min((life - t) / edge).clamp(0.0, 1.0)
}

/// The mist's colour: the fog's, lifted toward a pale grey so it reads as mist, not murk.
fn tint(l: &WowLighting) -> Vec3 {
    let fog = Vec3::from_array(l.fog_color);
    fog.lerp(Vec3::new(0.8, 0.82, 0.86), 0.5)
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.insert_resource(MistAssets {
        mesh: meshes.add(Plane3d::default().mesh().size(1.0, 1.0)),
        texture: asset_server.load("mpq://textures/weather/snowmist01.blp"),
    });
    commands.insert_resource(MistState { rng: 0x5EED_0F_0015 });
}

fn drive(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<AtmosphereSettings>,
    req: Res<SkyRequest>,
    weather: Option<Res<WeatherState>>,
    gust: Option<Res<WeatherGust>>,
    lighting: Res<WowLighting>,
    assets: Option<Res<MistAssets>>,
    mut state: ResMut<MistState>,
    focus: Query<&GlobalTransform, (With<ViewerUnit>, Without<Sheet>)>,
    cam: Query<&GlobalTransform, (With<WorldCamera>, Without<Sheet>)>,
    world_point: WorldPoint,
    mut sheets: Query<(Entity, &Sheet, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next_dump: Local<f32>,
) {
    let Some(assets) = assets else { return };
    let rain = weather
        .as_ref()
        .filter(|w| w.effect_kind == WeatherKind::Rain)
        .map_or(0.0, |w| w.effect_density);
    let thick = if settings.mist && req.active { amount(&req, rain) } else { 0.0 };
    let center = focus
        .iter()
        .next()
        .or(cam.iter().next())
        .map(GlobalTransform::translation);
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    let wind = gust.map_or(Vec3::ZERO, |g| g.wind) * 0.35 + Vec3::new(0.3, 0.0, 0.2);
    let color = tint(&lighting);

    let mut alive = 0;
    for (e, sheet, mut tf) in &mut sheets {
        let t = now - sheet.born;
        let far = center.is_none_or(|c| (tf.translation - c).xz().length() > DROP);
        if t > sheet.life || far || thick <= 0.0 {
            materials.remove(&sheet.material);
            commands.entity(e).despawn();
            continue;
        }
        alive += 1;
        tf.translation += wind * dt;
        if let Some(m) = materials.get_mut(&sheet.material) {
            let a = (OPACITY * settings.mist_strength * thick * fade(t, sheet.life)).min(1.0);
            m.base_color = Color::srgba(color.x, color.y, color.z, a);
        }
    }

    // One new sheet a frame at most, so a mist comes on as a drift, not a pop.
    let want = (thick * MAX_SHEETS as f32).round() as usize;
    if super::dump_due(now, &mut next_dump) {
        let ground = center.and_then(|c| world_point.terrain_height_under(c + Vec3::Y * 200.0));
        info!(
            "atmos mist: {} alive, amount {:.2}, focus {:?}, ground {:?}",
            alive, thick, center, ground
        );
    }
    let Some(center) = center else { return };
    if alive >= want {
        return;
    }
    let angle = state.roll() * std::f32::consts::TAU;
    let r = RADIUS * state.roll().sqrt();
    let at = center + Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
    let Some(ground) = world_point.terrain_height_under(at + Vec3::Y * 200.0) else {
        return;
    };
    let size = 14.0 + 12.0 * state.roll();
    let lift = 0.8 + 1.7 * state.roll();
    let life = 16.0 + 12.0 * state.roll();
    let yaw = state.roll() * std::f32::consts::TAU;
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(color.x, color.y, color.z, 0.0),
        base_color_texture: Some(assets.texture.clone()),
        unlit: true,
        fog_enabled: false,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        depth_bias: 4.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(Vec3::new(at.x, ground + lift, at.z))
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::new(size, 1.0, size)),
        Sheet {
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
    fn rain_thickens_the_mist_and_it_never_passes_full() {
        let req = SkyRequest { mist: 0.3, active: true, ..Default::default() };
        assert_eq!(amount(&req, 0.0), 0.3);
        assert!(amount(&req, 1.0) > 0.7);
        let thick = SkyRequest { mist: 0.9, ..req };
        assert_eq!(amount(&thick, 1.0), 1.0);
    }

    #[test]
    fn a_sheet_fades_in_and_out() {
        assert_eq!(fade(0.0, 20.0), 0.0);
        assert_eq!(fade(10.0, 20.0), 1.0);
        assert_eq!(fade(20.0, 20.0), 0.0);
        assert!(fade(2.5, 20.0) > 0.4 && fade(2.5, 20.0) < 0.6);
    }
}