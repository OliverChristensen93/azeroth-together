//! Nightfall's night: the light stands at midnight, the fog closes in at 40 yards, the world is
//! drawn only a little past it, and every hero carries a small warm light so a player sees their
//! feet. All of it comes from `content/nightfall/rules.ron` and is undone when the player goes
//! back to the pick screen, so Survivors is never left in the dark.

use benilla_world::lighting::{FogOverride, WorldPointLight};
use benilla_world::view::ViewDistance;
use bevy::prelude::*;
use nightfall_sim::Atmosphere;

use super::view::Views;
use super::content;

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
    fog: Option<Res<FogOverride>>,
    view: Option<ResMut<ViewDistance>>,
) {
    if fog.is_some() {
        return;
    }
    let Some(mut view) = view else { return };
    let (own, fog) = apply(&content().rules.atmosphere, &mut view);
    // The inn keeps the engine's own view distance saved while it draws the hub's: that is the
    // one to give back, not the hub's.
    if saved.is_none() {
        commands.insert_resource(own);
    }
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

/// The world stands on the map of the phase.
pub(super) fn follow_map(
    current: Option<ResMut<benilla_world::world_map::CurrentMap>>,
    phase: Res<State<super::Phase>>,
) {
    let Some(mut current) = current else { return };
    // The inn's map between raids, the raid's during one (both are map 0 for Goldshire and
    // Duskwood, but the hub may move to another map).
    let want = if *phase.get() == super::Phase::Inn {
        super::inn().place.map
    } else {
        content().map.map
    };
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

/// The view distance the inn draws, and the view distance to give back on leaving Nightfall: the
/// one saved by an earlier phase if there is one (a raid set its own over the engine's), else the
/// view's own now. Leaves the view at the hub's draw distance.
pub(super) fn inn_view(
    saved: Option<SavedView>,
    view: &mut ViewDistance,
    hub_distance: f32,
) -> SavedView {
    let engines = saved.unwrap_or(SavedView {
        farclip: view.farclip,
    });
    view.farclip = hub_distance;
    engines
}

/// In the inn: the hub's evening light and draw distance and no fog (the night's is taken away if
/// a raid set it), a contrast to the night outside (spec §8.4). The engine's own view distance is
/// kept as [`SavedView`] all the while, for [`leave`] to give back.
pub(super) fn inn_light(
    mut commands: Commands,
    saved: Option<Res<SavedView>>,
    view: Option<ResMut<ViewDistance>>,
    fog: Option<Res<FogOverride>>,
    debug: Option<ResMut<benilla_world::dev_state::DebugState>>,
) {
    let place = &super::inn().place;
    if let Some(mut view) = view {
        if saved.is_none() || view.farclip != place.view_distance {
            let keep = inn_view(saved.as_deref().copied(), &mut view, place.view_distance);
            if saved.is_none() {
                commands.insert_resource(keep);
            }
        }
    }
    if fog.is_some() {
        commands.remove_resource::<FogOverride>();
    }
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != place.minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = place.minute;
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
    fn the_inn_draws_the_hubs_distance_and_keeps_the_engines_to_give_back() {
        let mut view = ViewDistance {
            farclip: 350.0,
            nearclip: 0.2,
        };
        let kept = inn_view(None, &mut view, 200.0);
        assert_eq!((view.farclip, kept.farclip), (200.0, 350.0));
        // After a raid (which saved the engine's and set its own), the same is kept.
        let mut view = ViewDistance {
            farclip: 120.0,
            nearclip: 0.2,
        };
        let kept = inn_view(Some(kept), &mut view, 200.0);
        assert_eq!((view.farclip, kept.farclip), (200.0, 350.0));
        restore(&kept, &mut view);
        assert_eq!(view.farclip, 350.0);
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
