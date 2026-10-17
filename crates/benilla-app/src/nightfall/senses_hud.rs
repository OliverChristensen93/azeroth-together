//! The senses on screen (spec §5.5, §10): an eye under the player frame that opens and brightens
//! with the hero's visibility (its number beneath, for tuning), up to three rings around it for
//! the loudest noise they made in the last second, and a heartbeat while an enemy alerted to them
//! is near. The vignette is the night grade's ([`super::grade::follow_visibility`]).

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use nightfall_sim::Alert;

use super::run::Sim;
use super::NightfallEntity;

/// The eye's box, UI pixels: the rings fit inside it.
const BOX: f32 = 84.0;
/// The eye at its widest, and its pupil, UI pixels.
const EYE_W: f32 = 34.0;
const PUPIL: f32 = 11.0;
/// The rings' diameters, UI pixels, innermost first.
const RINGS: [f32; 3] = [48.0, 64.0, 80.0];
/// Where the eye's centre sits in WoW's 768-high interface units: under the player frame's
/// portrait (the frame is anchored at the top left).
const EYE_AT: Vec2 = Vec2::new(60.0, 112.0);
/// The heartbeat's thump: the trimmed data has no heartbeat, so a small body falling on dirt
/// stands in for one.
const THUMP: &str = r"Sound\Effects\DeathImpacts\mDeathImpactSmallDirtA.wav";
/// The second beat of each heartbeat follows the first by this much, seconds.
const DUB_AFTER: f32 = 0.16;

#[derive(Component)]
pub(super) struct EyeRoot;
#[derive(Component)]
pub(super) struct EyeWhite;
#[derive(Component)]
pub(super) struct EyePupil;
/// One ring, innermost 0.
#[derive(Component)]
pub(super) struct EyeRing(usize);
#[derive(Component)]
pub(super) struct EyeNumber;

/// The eye's centre on a window `height` logical pixels high, before the UI scale.
pub(super) fn eye_center(height: f32) -> Vec2 {
    EYE_AT * (height / 768.0)
}

/// How open the eye is (a share of its width) and its colour at `visibility` 0..100: nearly shut
/// and slate-dark when hidden, wide and lamplit when seen.
pub(super) fn eye_look(visibility: f32) -> (f32, [f32; 3]) {
    let t = (visibility / 100.0).clamp(0.0, 1.0);
    let dim = [0.32, 0.38, 0.48];
    let lit = [1.0, 0.86, 0.45];
    let mix = |i: usize| dim[i] + (lit[i] - dim[i]) * t;
    (0.12 + 0.88 * t, [mix(0), mix(1), mix(2)])
}

/// How many rings show for a noise of `radius` yards: none for silence, one for a footstep's
/// whisper, two for a run, three for anything louder.
pub(super) fn ring_count(radius: f32) -> usize {
    if radius <= 0.0 {
        0
    } else if radius < 10.0 {
        1
    } else if radius < 20.0 {
        2
    } else {
        3
    }
}

/// Seconds between heartbeats with an enemy alerted to us `distance` yards off: from 0.9 at
/// `within` down to 0.45 on top of us; none past `within`.
pub(super) fn beat_interval(distance: f32, within: f32) -> Option<f32> {
    (distance <= within).then(|| 0.45 + 0.45 * (distance / within.max(1e-3)).clamp(0.0, 1.0))
}

/// The eye and its rings, once per run (they go with the run's other entities).
pub(super) fn spawn_eye(
    mut commands: Commands,
    assets: Res<AssetServer>,
    eyes: Query<(), With<EyeRoot>>,
) {
    if !eyes.is_empty() {
        return;
    }
    let font = crate::char_select::wow_font(&assets);
    commands
        .spawn((
            EyeRoot,
            NightfallEntity,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(BOX),
                height: Val::Px(BOX),
                ..default()
            },
            GlobalZIndex(350),
            // Hidden until the hero exists and the eye is placed; [`drive_eye`] shows it.
            Visibility::Hidden,
        ))
        .with_children(|root| {
            for (i, d) in RINGS.into_iter().enumerate() {
                root.spawn((
                    EyeRing(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px((BOX - d) / 2.0),
                        top: Val::Px((BOX - d) / 2.0),
                        width: Val::Px(d),
                        height: Val::Px(d),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.85, 0.85, 0.8, 0.45)),
                    Visibility::Hidden,
                ));
            }
            root.spawn((
                EyeWhite,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((BOX - EYE_W) / 2.0),
                    width: Val::Px(EYE_W),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.85)),
                BorderColor::all(Color::WHITE),
            ))
            .with_children(|eye| {
                eye.spawn((
                    EyePupil,
                    Node {
                        width: Val::Px(PUPIL),
                        height: Val::Px(PUPIL),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                ));
            });
            root.spawn((
                EyeNumber,
                Text::new(""),
                TextFont {
                    font,
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(BOX / 2.0 - 7.0),
                    top: Val::Px(BOX - 12.0),
                    ..default()
                },
            ));
        });
}

/// The eye follows the hero: under the player frame, open and bright by visibility, ringed by
/// noise, its number beneath.
#[allow(clippy::type_complexity)]
pub(super) fn drive_eye(
    sim: Res<Sim>,
    ui_scale: Res<UiScale>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: ParamSet<(
        Query<(&mut Node, &mut Visibility), With<EyeRoot>>,
        Query<(&mut Node, &mut BorderColor), With<EyeWhite>>,
        Query<&mut BackgroundColor, With<EyePupil>>,
        Query<(&EyeRing, &mut Visibility)>,
        Query<&mut Text, With<EyeNumber>>,
    )>,
) {
    let (Some(hero), Ok(window)) = (sim.world.hero(sim.me), windows.single()) else {
        return;
    };
    let s = ui_scale.0;
    let c = eye_center(window.height());
    if let Ok((mut root, mut vis)) = nodes.p0().single_mut() {
        // The eye shows once it has a place: a joined client has no hero before its first snapshot.
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        root.left = Val::Px(c.x / s - BOX / 2.0);
        root.top = Val::Px(c.y / s - BOX / 2.0);
    }
    let (open, rgb) = eye_look(hero.visibility);
    let color = Color::srgb(rgb[0], rgb[1], rgb[2]);
    if let Ok((mut white, mut border)) = nodes.p1().single_mut() {
        let h = (EYE_W * open).max(3.0);
        white.height = Val::Px(h);
        white.top = Val::Px((BOX - h) / 2.0);
        *border = BorderColor::all(color);
    }
    if let Ok(mut pupil) = nodes.p2().single_mut() {
        *pupil = BackgroundColor(color.with_alpha(0.3 + 0.7 * open));
    }
    let rings = ring_count(hero.noise);
    let mut ring_q = nodes.p3();
    for (ring, mut vis) in &mut ring_q {
        let want = if ring.0 < rings {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    if let Ok(mut text) = nodes.p4().single_mut() {
        let number = format!("{:.0}", hero.visibility);
        if text.0 != number {
            text.0 = number;
        }
    }
}

/// When the next heartbeat is due, and its second beat.
#[derive(Default)]
pub(super) struct Beat {
    next: f32,
    dub: Option<f32>,
}

/// The heartbeat (spec §10): a lub-dub through WoW's own `PlaySoundFile`, faster the nearer the
/// nearest enemy alerted to our hero, while one is within `heartbeat_within` yards. A missing
/// sound file only makes it silent.
pub(super) fn heartbeat(
    sim: Res<Sim>,
    script: Option<NonSend<UiScript>>,
    real: Res<Time<Real>>,
    mut beat: Local<Beat>,
) {
    let within = sim.world.content().rules.senses.heartbeat_within;
    let now = real.elapsed_secs();
    let nearest = sim.world.hero(sim.me).and_then(|h| {
        sim.marks
            .iter()
            .filter(|m| m.alert == Alert::Alerted)
            .filter_map(|m| sim.world.enemy(m.enemy))
            .map(|e| e.pos.distance(h.pos))
            .min_by(f32::total_cmp)
    });
    let Some(interval) = nearest.and_then(|d| beat_interval(d, within)) else {
        // The first beat comes at once when a hunter closes in.
        beat.next = now;
        beat.dub = None;
        return;
    };
    let Some(script) = script else {
        return;
    };
    let thump = || {
        let _ = script.run(&format!("PlaySoundFile([[{THUMP}]])"));
    };
    if beat.dub.is_some_and(|t| now >= t) {
        beat.dub = None;
        thump();
    }
    if now >= beat.next {
        thump();
        beat.dub = Some(now + DUB_AFTER);
        beat.next = now + interval;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eye_sits_under_the_player_frame_at_any_height() {
        assert_eq!(eye_center(768.0), Vec2::new(60.0, 112.0));
        assert_eq!(eye_center(720.0), Vec2::new(56.25, 105.0));
    }

    #[test]
    fn the_eye_opens_and_brightens_with_visibility() {
        let (shut, dark) = eye_look(0.0);
        let (wide, lit) = eye_look(100.0);
        assert!((shut - 0.12).abs() < 1e-6);
        assert!((wide - 1.0).abs() < 1e-6);
        assert!(lit[0] > dark[0]);
        assert_eq!(eye_look(250.0), eye_look(100.0));
    }

    #[test]
    fn louder_noise_more_rings() {
        assert_eq!(ring_count(0.0), 0);
        assert_eq!(ring_count(4.0), 1);
        assert_eq!(ring_count(12.0), 2);
        assert_eq!(ring_count(40.0), 3);
    }

    #[test]
    fn the_heart_beats_faster_the_nearer_the_hunter() {
        assert_eq!(beat_interval(25.0, 20.0), None);
        assert!((beat_interval(20.0, 20.0).unwrap() - 0.9).abs() < 1e-6);
        assert_eq!(beat_interval(0.0, 20.0), Some(0.45));
        assert!(beat_interval(5.0, 20.0) < beat_interval(15.0, 20.0));
    }
}
