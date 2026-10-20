//! The request's weather into the engine, as a server's `SMSG_WEATHER` would bring it: one
//! `WeatherMessage` each time the weather or its strength changes, smooth, so the engine's own
//! ramps (about ten seconds) carry the change.

use bevy::prelude::*;

use benilla_world::weather::{WeatherKind, WeatherMessage};

use super::SkyRequest;

/// The weather last sent, as (type on the wire, grade in twentieths).
#[derive(Resource, Default, PartialEq, Clone, Copy, Debug)]
struct Sent(Option<(u32, u8)>);

/// The wire type of a kind (vmangos `WeatherType`).
fn wire(kind: WeatherKind) -> u32 {
    match kind {
        WeatherKind::Fine => 0,
        WeatherKind::Rain => 1,
        WeatherKind::Snow => 2,
        WeatherKind::Sand => 3,
    }
}

/// The engine shows nothing under a grade of 0.25, so a light shower starts above it.
fn grade(intensity: f32) -> f32 {
    0.3 + 0.7 * intensity.clamp(0.0, 1.0)
}

/// The SoundEntries loop kit for a kind and strength (vmangos `Weather::GetSound`): light,
/// medium and heavy thirds.
fn sound_kit(kind: WeatherKind, intensity: f32) -> u32 {
    let step = match intensity {
        i if i < 0.34 => 0,
        i if i < 0.67 => 1,
        _ => 2,
    };
    match kind {
        WeatherKind::Fine => 0,
        WeatherKind::Rain => 8533 + step,
        WeatherKind::Snow => 8536 + step,
        WeatherKind::Sand => 8556 + step,
    }
}

/// The message for a request, or `None` when it is what was last sent.
fn message_for(req: &SkyRequest, sent: &mut Sent) -> Option<WeatherMessage> {
    let kind = if req.active { req.weather } else { WeatherKind::Fine };
    let key = match kind {
        WeatherKind::Fine => (0, 0),
        k => (wire(k), (req.intensity.clamp(0.0, 1.0) * 20.0).round() as u8),
    };
    // Nothing was ever sent and none is wanted: leave the engine's weather (a server's) alone.
    if sent.0.is_none() && key.0 == 0 {
        return None;
    }
    if sent.0 == Some(key) {
        return None;
    }
    sent.0 = Some(key);
    let intensity = f32::from(key.1) / 20.0;
    Some(WeatherMessage {
        weather_type: key.0,
        grade: if key.0 == 0 { 0.0 } else { grade(intensity) },
        sound_id: sound_kit(kind, intensity),
        instant: false,
    })
}

fn drive(req: Res<SkyRequest>, mut sent: ResMut<Sent>, mut out: MessageWriter<WeatherMessage>) {
    if let Some(m) = message_for(&req, &mut sent) {
        info!(
            "atmosphere: weather {} grade {:.2} (sound kit {})",
            m.weather_type, m.grade, m.sound_id
        );
        out.write(m);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Sent>().add_systems(Update, drive);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rain(i: f32) -> SkyRequest {
        SkyRequest {
            weather: WeatherKind::Rain,
            intensity: i,
            active: true,
            ..Default::default()
        }
    }

    #[test]
    fn a_change_sends_once_and_a_repeat_sends_nothing() {
        let mut sent = Sent::default();
        let m = message_for(&rain(0.5), &mut sent).unwrap();
        assert_eq!((m.weather_type, m.sound_id, m.instant), (1, 8534, false));
        assert!(m.grade > 0.25);
        assert!(message_for(&rain(0.5), &mut sent).is_none());
        assert!(message_for(&rain(0.51), &mut sent).is_none(), "under a twentieth is no change");
        assert_eq!(message_for(&rain(0.9), &mut sent).unwrap().sound_id, 8535);
    }

    #[test]
    fn fine_is_sent_only_after_weather_and_when_the_game_stops() {
        let mut sent = Sent::default();
        let fine = SkyRequest { active: true, ..Default::default() };
        assert!(message_for(&fine, &mut sent).is_none());
        message_for(&rain(1.0), &mut sent).unwrap();
        let stopped = SkyRequest::default();
        let m = message_for(&stopped, &mut sent).unwrap();
        assert_eq!((m.weather_type, m.sound_id, m.grade), (0, 0, 0.0));
        assert!(message_for(&stopped, &mut sent).is_none());
    }
}
