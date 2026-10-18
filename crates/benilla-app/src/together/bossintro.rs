//! A boss's entrance: when one comes, the run waits (as for cards) behind a panel that names it
//! and lists what it does, until Continue (or Enter, or Space). In co-op the run waits for every
//! player's Continue. Afterwards the boss bar keeps to its name and health; hovering it shows the
//! same list ([`mechanics_text`]).

use bevy::prelude::*;
use together_sim::{EnemyDef, Mechanic};

use super::{autoplay, LocalInput, Phase, Sim};

/// The panel's width, in the 768-unit canvas.
const WIDTH: f32 = 420.0;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);

/// The boss whose entrance is up (its enemy id), the bosses already introduced, and when the
/// panel opened (for autoplay).
#[derive(Resource, Default)]
pub(super) struct BossIntro {
    pub(super) showing: Option<u32>,
    seen: Vec<u32>,
    opened: f32,
}

impl BossIntro {
    /// Whether the panel is holding the run.
    pub(super) fn open(&self) -> bool {
        self.showing.is_some()
    }
}

#[derive(Component)]
pub(super) struct IntroRoot;

#[derive(Component)]
pub(super) struct IntroTitle;

#[derive(Component)]
pub(super) struct IntroBody;

#[derive(Component)]
pub(super) struct IntroContinue;

/// "DeathGrip" as "Death Grip".
pub(super) fn mechanic_name(m: Mechanic) -> String {
    let raw = format!("{m:?}");
    let mut out = String::new();
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// What a boss does, a line per mechanic; its own sentence when it lists none.
pub(super) fn mechanics_text(def: &EnemyDef) -> String {
    if def.mechanics.is_empty() {
        return def.hint.clone();
    }
    def.mechanics
        .iter()
        .map(|&m| format!("• {}: {}", mechanic_name(m), m.hint()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let morpheus: Handle<Font> = asset_server.load("mpq://Fonts/MORPHEUS.TTF");
    commands
        .spawn((
            IntroRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(320),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Px(WIDTH),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.02, 0.02, 0.95)),
                BorderColor::all(Color::srgb(0.75, 0.15, 0.1)),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(""),
                    TextFont {
                        font: morpheus.clone(),
                        font_size: 30.0,
                        ..default()
                    },
                    TextColor(GOLD),
                    TextShadow::default(),
                    TextLayout::new_with_justify(Justify::Center),
                    IntroTitle,
                ));
                p.spawn((
                    Text::new(""),
                    TextFont {
                        font: font.clone(),
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                    Node {
                        max_width: Val::Px(WIDTH - 32.0),
                        ..default()
                    },
                    IntroBody,
                ));
                p.spawn((
                    Button,
                    IntroContinue,
                    Node {
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.35, 0.05, 0.03, 0.95)),
                    BorderColor::all(Color::srgb(0.85, 0.7, 0.35)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("Continue"),
                        TextFont {
                            font: font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(GOLD),
                        TextShadow::default(),
                    ));
                });
            });
        });
}

/// A new boss opens the panel; Continue (Enter, Space) closes it. While open, our input asks the
/// run to wait.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    mut intro: ResMut<BossIntro>,
    mut input: ResMut<LocalInput>,
    mut root: Query<&mut Visibility, With<IntroRoot>>,
    mut title: Query<&mut Text, (With<IntroTitle>, Without<IntroBody>)>,
    mut body: Query<&mut Text, (With<IntroBody>, Without<IntroTitle>)>,
    button: Query<&Interaction, (Changed<Interaction>, With<IntroContinue>)>,
) {
    let playing = *phase.get() == Phase::Playing;
    let Some(sim) = sim.filter(|_| playing) else {
        *intro = BossIntro::default();
        set_visible(&mut root, false);
        return;
    };
    let world = &sim.world;
    let now = time.elapsed_secs();
    if intro.showing.is_none() {
        let alive_hero = world.hero(sim.me).is_some_and(|h| h.alive);
        let new = world
            .enemies
            .iter()
            .find(|e| e.boss && !intro.seen.contains(&e.id));
        if let (Some(boss), true) = (new, alive_hero) {
            let (id, def) = (boss.id, boss.def);
            intro.seen.push(id);
            intro.showing = Some(id);
            intro.opened = now;
            let def = &world.content().enemies[def];
            if let Ok(mut t) = title.single_mut() {
                t.0 = def.name.clone();
            }
            if let Ok(mut t) = body.single_mut() {
                t.0 = format!(
                    "A level {} boss has come for you.\n\n{}\n\nHover its bar at the top to read this \
                     again; click the bar to target it.",
                    def.level,
                    mechanics_text(def)
                );
            }
        }
    }
    if let Some(id) = intro.showing {
        let pressed = button.iter().any(|i| *i == Interaction::Pressed)
            || keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::Space)
            || (autoplay() && now - intro.opened > 3.0);
        // A boss that is gone (or a hero that fell) closes it too.
        let gone = world.enemy(id).is_none() || !world.hero(sim.me).is_some_and(|h| h.alive);
        if pressed || gone {
            intro.showing = None;
        }
    }
    input.boss_intro = intro.open();
    set_visible(&mut root, intro.open());
}

fn set_visible(root: &mut Query<&mut Visibility, With<IntroRoot>>, on: bool) {
    let want = if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in root.iter_mut() {
        if *v != want {
            *v = want;
        }
    }
}
