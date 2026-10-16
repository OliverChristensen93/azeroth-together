//! Nightfall's night: the light stands at midnight, the fog closes in at 40 yards, the world is
//! drawn only a little past it, and every hero carries a small warm light so a player sees their
//! feet. All of it comes from `content/nightfall/rules.ron` and is undone when the player goes
//! back to the pick screen, so Survivors is never left in the dark.

use benilla_world::lighting::{FogOverride, WorldPointLight};
use benilla_world::view::{ViewDistance, WorldCamera};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::Atmosphere;

use super::view::Views;
use super::{content, units};

/// The view distance before Nightfall changed it, to give back on leaving.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(super) struct SavedView {
    farclip: f32,
}

/// A hero that already carries its light.
#[derive(Component)]
pub(super) struct HeroLit;

/// Nightfall's view distance and fog, from `a`; what to give back on leaving.
pub(super) fn apply(a: &Atmosphere, view: &mut ViewDistance) -> (SavedView, FogOverride) {
    let saved = SavedView {
        farclip: view.farclip,
    };
    view.farclip = a.view_distance;
    let fog = FogOverride {
        start: a.fog_start,
        end: a.fog_end,
        color: [a.fog_color.0, a.fog_color.1, a.fog_color.2],
    };
    (saved, fog)
}

/// The view distance as it was before Nightfall.
pub(super) fn restore(saved: &SavedView, view: &mut ViewDistance) {
    view.farclip = saved.farclip;
}

/// In Nightfall: the fog and the short view, set once (until [`leave`] gives them back). A frame
/// system rather than `OnEnter`, because a launch straight into Nightfall enters before the engine
/// has made its `ViewDistance`.
pub(super) fn enter(
    mut commands: Commands,
    saved: Option<Res<SavedView>>,
    view: Option<ResMut<ViewDistance>>,
) {
    if saved.is_some() {
        return;
    }
    let Some(mut view) = view else { return };
    let (saved, fog) = apply(&content().rules.atmosphere, &mut view);
    commands.insert_resource(saved);
    commands.insert_resource(fog);
}

/// Leaving Nightfall: the engine's own fog and view distance again.
pub(super) fn leave(
    mut commands: Commands,
    saved: Option<Res<SavedView>>,
    view: Option<ResMut<ViewDistance>>,
) {
    if let (Some(saved), Some(mut view)) = (saved, view) {
        restore(&saved, &mut view);
    }
    commands.remove_resource::<SavedView>();
    commands.remove_resource::<FogOverride>();
}

/// The light stands at the night's minute, whatever the clock says.
pub(super) fn night(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    let minute = content().rules.atmosphere.night_minute;
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = minute;
    }
}

/// The world stands on Nightfall's map.
pub(super) fn follow_map(current: Option<ResMut<benilla_world::world_map::CurrentMap>>) {
    let Some(mut current) = current else { return };
    let want = content().map.map;
    if current.0 != want {
        current.0 = want;
    }
}

/// Each hero carries a small warm light, 1.6 yards up (about the chest).
pub(super) fn hero_lights(
    mut commands: Commands,
    views: Res<Views>,
    lit: Query<(), With<HeroLit>>,
) {
    let l = &content().rules.atmosphere.hero_light;
    for &e in views.heroes.values() {
        if lit.get(e).is_ok() {
            continue;
        }
        commands.entity(e).insert(HeroLit).with_children(|c| {
            c.spawn((
                WorldPointLight {
                    color: [l.color.0, l.color.1, l.color.2],
                    // `4π × intensity`, the point-light convention the packer undoes.
                    intensity: 4.0 * std::f32::consts::PI * l.intensity,
                    range: l.range,
                },
                Transform::from_xyz(0.0, 1.6, 0.0),
                Visibility::default(),
            ));
        });
    }
}

/// At the menu: a slow, wide orbit over the first entry, so the dark woods stand behind it and
/// the terrain streams there.
pub(super) fn menu_camera(
    time: Res<Time<Real>>,
    world: WorldPoint,
    mut player: ResMut<crate::player::Player>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
    mut orbit: Local<f32>,
) {
    let mut focus = units::wow_to_bevy(content().map.entries[0]);
    if let Some(h) = world.terrain_height_under(focus) {
        focus.y = h;
    }
    if !player.active {
        player.pos = focus;
    }
    *orbit += time.delta_secs() * 0.05;
    let d = 14.0;
    let eye = focus + Vec3::new(orbit.sin() * d, 4.0, orbit.cos() * d);
    let placed = Transform::from_translation(eye).looking_at(focus + Vec3::Y * 1.5, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atmosphere() -> Atmosphere {
        Atmosphere {
            night_minute: 0,
            fog_start: 10.0,
            fog_end: 40.0,
            fog_color: (0.04, 0.07, 0.08),
            view_distance: 120.0,
            hero_light: nightfall_sim::HeroLight {
                color: (1.0, 0.8, 0.6),
                intensity: 0.6,
                range: 9.0,
            },
        }
    }

    #[test]
    fn entering_sets_the_fog_and_view_and_leaving_gives_the_view_back() {
        let mut view = ViewDistance {
            farclip: 350.0,
            nearclip: 0.2,
        };
        let (saved, fog) = apply(&atmosphere(), &mut view);
        assert_eq!(view.farclip, 120.0);
        assert_eq!(
            fog,
            FogOverride {
                start: 10.0,
                end: 40.0,
                color: [0.04, 0.07, 0.08]
            }
        );
        restore(&saved, &mut view);
        assert_eq!(view.farclip, 350.0);
        assert_eq!(view.nearclip, 0.2);
    }
}
