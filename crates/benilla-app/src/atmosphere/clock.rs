//! The request's time of day into the engine's clock: written as a live `WorldTime` after the
//! session clock publishes its own (noon with no server), so the sky, sun and `Light.dbc` light
//! follow it, gliding by the fractional minute.

use bevy::prelude::*;

use benilla_world::dev_state::DebugState;
use benilla_world::lighting::{LightingResolveSet, WorldTime};
use benilla_world::schedule::WorldStage;

use super::SkyRequest;

/// The clock for a minute of the day.
fn world_time(minute: f32) -> WorldTime {
    let m = minute.rem_euclid(1440.0);
    WorldTime {
        minute: (m as u32).min(1439),
        minute_f: m,
        day: f64::from(m) / 1440.0,
        live: true,
    }
}

fn drive(
    req: Res<SkyRequest>,
    time: Option<ResMut<WorldTime>>,
    debug: Option<ResMut<DebugState>>,
) {
    let Some(minute) = req.minute.filter(|_| req.active) else {
        return;
    };
    if let Some(mut time) = time {
        *time = world_time(minute);
    }
    // The lighting reads the clock only while it follows it (a game may have pinned an hour).
    if let Some(mut debug) = debug {
        if !debug.lighting.follow_server_time {
            debug.lighting.follow_server_time = true;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        drive.after(WorldStage::Net).before(LightingResolveSet),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minute_becomes_a_live_clock() {
        let t = world_time(1050.5);
        assert_eq!((t.minute, t.minute_f, t.live), (1050, 1050.5, true));
        assert_eq!(world_time(1441.0).minute, 1);
    }
}
