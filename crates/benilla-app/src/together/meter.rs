//! A damage meter in the style of WoW's damage-meter addons: "Damage Done" with the run's damage
//! per second, and a row per ability, most damage first, as icon, bar, name and
//! `total (per second, share)`. In co-op, a row per player comes first, in class colours, then our
//! own abilities. Under the minimap, during a run. Clicking the title switches to Boss Damage, then
//! to Healing & Absorbed; Reset counts from now.
//!
//! Laid out on the 768-unit canvas (`UiScale`, set by [`super::menu::apply_ui_scale`]).

use bevy::prelude::*;

use super::menu::{rgb, SpellIcon};
use super::{content, Phase, Sim};

const WIDTH: f32 = 214.0;
const ROW_H: f32 = 15.0;
/// Seconds between redraws: often enough to read live, rarely enough to cost nothing.
const EVERY: f32 = 0.5;

/// One bar colour per row, in order.
const BARS: [Color; 6] = [
    Color::srgb(0.70, 0.22, 0.18),
    Color::srgb(0.22, 0.40, 0.78),
    Color::srgb(0.78, 0.60, 0.14),
    Color::srgb(0.80, 0.38, 0.10),
    Color::srgb(0.25, 0.62, 0.28),
    Color::srgb(0.55, 0.28, 0.75),
];

fn label(font: &Handle<Font>, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow::default(),
    )
}

#[derive(Component)]
pub(super) struct MeterRoot;

#[derive(Component)]
pub(super) struct MeterRows;

#[derive(Component)]
pub(super) struct MeterTotal;

/// The header's title, which switches the view when clicked, and its Reset button.
#[derive(Component)]
pub(super) struct MeterTitle;

#[derive(Component)]
pub(super) struct MeterTitleText;

#[derive(Component)]
pub(super) struct MeterReset;

#[derive(Resource)]
pub(super) struct MeterFont(Handle<Font>);

/// What the meter counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Damage,
    Boss,
    Healing,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Mode::Damage => "Damage Done",
            Mode::Boss => "Boss Damage",
            Mode::Healing => "Healing & Absorbed",
        }
    }

    fn next(self) -> Mode {
        match self {
            Mode::Damage => Mode::Boss,
            Mode::Boss => Mode::Healing,
            Mode::Healing => Mode::Damage,
        }
    }

    fn per_second(self) -> &'static str {
        match self {
            Mode::Healing => "hps",
            _ => "dps",
        }
    }
}

/// A hero's numbers as the meter reads them: its total, and by ability.
#[derive(Clone, Debug, Default)]
struct Tally {
    total: f64,
    by: Vec<(usize, f64)>,
}

/// The meter's view, and what everything stood at when Reset was last pressed: the numbers shown
/// are since then.
#[derive(Default)]
pub(super) struct MeterState {
    mode: Mode,
    next: f32,
    /// The run time of the reset, and each player's damage, boss damage and healing then.
    since: f32,
    base: Vec<(together_sim::PlayerId, [Tally; 3])>,
}

/// The healing view's two rows, in place of abilities.
const HEALED: usize = usize::MAX;
const ABSORBED: usize = usize::MAX - 1;

/// Hero `h`'s numbers for each view: damage, boss damage, and healing with what its shields
/// absorbed.
fn tallies(h: &together_sim::Hero) -> [Tally; 3] {
    [
        Tally {
            total: h.damage_done,
            by: h.damage_by.clone(),
        },
        Tally {
            total: h.boss_damage,
            by: h.boss_by.clone(),
        },
        Tally {
            total: h.healed + h.absorbed,
            by: vec![(HEALED, h.healed), (ABSORBED, h.absorbed)],
        },
    ]
}

/// `now` less `base`, ability by ability.
fn since(now: &Tally, base: Option<&Tally>) -> Tally {
    let Some(base) = base else {
        return now.clone();
    };
    let by = now
        .by
        .iter()
        .map(|&(a, v)| {
            let was = base.by.iter().find(|(b, _)| *b == a).map_or(0.0, |x| x.1);
            (a, (v - was).max(0.0))
        })
        .collect();
    Tally {
        total: (now.total - base.total).max(0.0),
        by,
    }
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            MeterRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                top: Val::Px(super::runpanel::METER_TOP),
                width: Val::Px(WIDTH),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 0.8)),
            Visibility::Hidden,
        ))
        .with_children(|m| {
            m.spawn((
                Node {
                    height: Val::Px(16.0),
                    padding: UiRect::horizontal(Val::Px(5.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.16, 0.12, 0.07, 0.95)),
            ))
            .with_children(|t| {
                // Click the title for the next view.
                t.spawn((MeterTitle, Button, Node::default()))
                    .with_children(|b| {
                        b.spawn((
                            label(&font, "Damage Done >", 10.0, Color::srgb(1.0, 0.82, 0.0)),
                            MeterTitleText,
                        ));
                    });
                t.spawn((
                    label(&font, "", 9.0, Color::srgb(0.85, 0.85, 0.85)),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                    TextLayout::new_with_justify(Justify::Right),
                    MeterTotal,
                ));
                t.spawn((
                    MeterReset,
                    Button,
                    Node {
                        padding: UiRect::horizontal(Val::Px(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.6, 0.5, 0.3, 0.9)),
                ))
                .with_children(|b| {
                    b.spawn(label(&font, "Reset", 8.0, Color::srgb(0.9, 0.9, 0.9)));
                });
            });
            m.spawn((
                MeterRows,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(2.0)),
                    row_gap: Val::Px(1.0),
                    ..default()
                },
            ));
        });
    commands.insert_resource(MeterFont(font));
}

/// Show the meter in a run, redrawn every [`EVERY`] seconds (or at once on a click) from the
/// heroes' numbers since the last reset.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    font: Option<Res<MeterFont>>,
    mut root: Query<&mut Visibility, With<MeterRoot>>,
    rows: Query<Entity, With<MeterRows>>,
    mut total_text: Query<&mut Text, (With<MeterTotal>, Without<MeterTitleText>)>,
    mut title_text: Query<&mut Text, (With<MeterTitleText>, Without<MeterTotal>)>,
    title: Query<&Interaction, (Changed<Interaction>, With<MeterTitle>)>,
    reset: Query<&Interaction, (Changed<Interaction>, With<MeterReset>)>,
    mut state: Local<MeterState>,
) {
    let playing = *phase.get() == Phase::Playing && sim.is_some();
    let want = if playing {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut root {
        if *v != want {
            *v = want;
        }
    }
    let (Some(sim), Some(font)) = (sim, font) else {
        return;
    };
    if !playing {
        return;
    }
    let world = &sim.world;
    // A new run starts the meter afresh.
    if world.elapsed < state.since {
        state.since = 0.0;
        state.base.clear();
    }
    let mut clicked = false;
    if title.iter().any(|i| *i == Interaction::Pressed) {
        state.mode = state.mode.next();
        clicked = true;
    }
    if reset.iter().any(|i| *i == Interaction::Pressed) {
        state.since = world.elapsed;
        state.base = world
            .heroes
            .iter()
            .map(|h| (h.player, tallies(h)))
            .collect();
        clicked = true;
    }
    if let Ok(mut t) = title_text.single_mut() {
        let s = format!("{} >", state.mode.title());
        if t.0 != s {
            t.0 = s;
        }
    }
    if !clicked && time.elapsed_secs() < state.next {
        return;
    }
    state.next = time.elapsed_secs() + EVERY;
    let Some(hero) = world.hero(sim.me) else {
        return;
    };
    let view = state.mode as usize;
    let tally = |h: &together_sim::Hero| {
        let base = state
            .base
            .iter()
            .find(|(p, _)| *p == h.player)
            .map(|(_, t)| &t[view]);
        since(&tallies(h)[view], base)
    };
    let content = content();
    let secs = f64::from((world.elapsed - state.since).max(1.0));
    let party = world.heroes.len() > 1;
    let mine = tally(hero);
    let total = if party {
        world.heroes.iter().map(|h| tally(h).total).sum()
    } else {
        mine.total
    };
    if let Ok(mut t) = total_text.single_mut() {
        let s = format!("{:.0} {}", total / secs, state.mode.per_second());
        if t.0 != s {
            t.0 = s;
        }
    }
    let Ok(rows) = rows.single() else {
        return;
    };
    let mode = state.mode;
    commands.entity(rows).despawn_related::<Children>();
    commands.entity(rows).with_children(|r| {
        if party {
            let mut players: Vec<(&together_sim::Hero, f64)> =
                world.heroes.iter().map(|h| (h, tally(h).total)).collect();
            players.sort_by(|a, b| b.1.total_cmp(&a.1));
            let top = players.first().map_or(1.0, |p| p.1.max(1.0));
            for (i, (h, v)) in players.iter().enumerate() {
                let class = &content.classes[h.class];
                bar_row(
                    r,
                    &font.0,
                    None,
                    format!("{}. {}", i + 1, h.name),
                    figures(*v, secs, total),
                    v / top,
                    dim(rgb(class.color)),
                );
            }
            r.spawn((
                label(&font.0, "Yours", 9.0, Color::srgb(1.0, 0.82, 0.0)),
                Node {
                    margin: UiRect::new(Val::Px(3.0), Val::ZERO, Val::Px(3.0), Val::Px(1.0)),
                    ..default()
                },
            ));
        }
        let mut by: Vec<(usize, f64)> = mine.by.iter().copied().filter(|r| r.1 > 0.0).collect();
        by.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top = by.first().map_or(1.0, |r| r.1.max(1.0));
        if by.is_empty() {
            let none = match mode {
                Mode::Damage => "No damage yet",
                Mode::Boss => "No boss damage yet",
                Mode::Healing => "No healing yet",
            };
            r.spawn(label(&font.0, none, 9.0, Color::srgb(0.6, 0.6, 0.6)));
        }
        for (i, &(ability, dealt)) in by.iter().enumerate() {
            let (visual, name) = match ability {
                HEALED => (None, "Healing".to_string()),
                ABSORBED => (None, "Absorbed".to_string()),
                a => {
                    let def = &content.abilities[a];
                    (Some(def.visual), def.name.clone())
                }
            };
            bar_row(
                r,
                &font.0,
                visual,
                format!("{}. {}", i + 1, name),
                figures(dealt, secs, mine.total),
                dealt / top,
                BARS[i % BARS.len()],
            );
        }
    });
}

/// `total (per second, share%)`.
fn figures(dealt: f64, secs: f64, of: f64) -> String {
    let share = if of > 0.0 { dealt / of } else { 0.0 };
    format!("{:.0} ({:.0}, {:.0}%)", dealt, dealt / secs, share * 100.0)
}

/// One meter row: a bar `fill` of the way across behind an optional spell icon, the name and the
/// figures.
fn bar_row(
    r: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    icon: Option<u32>,
    name: String,
    figures: String,
    fill: f64,
    color: Color,
) {
    r.spawn(Node {
        height: Val::Px(ROW_H),
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        ..default()
    })
    .with_children(|row| {
        // The bar, behind everything else in the row.
        row.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent((fill * 100.0) as f32),
                ..default()
            },
            BackgroundColor(color.with_alpha(0.85)),
        ));
        match icon {
            Some(visual) => {
                row.spawn((
                    SpellIcon(visual),
                    ImageNode {
                        color: Color::srgba(0.2, 0.2, 0.2, 1.0),
                        ..default()
                    },
                    Node {
                        width: Val::Px(ROW_H - 2.0),
                        height: Val::Px(ROW_H - 2.0),
                        margin: UiRect::left(Val::Px(1.0)),
                        ..default()
                    },
                ));
            }
            None => {
                row.spawn(Node {
                    width: Val::Px(0.0),
                    ..default()
                });
            }
        }
        row.spawn((
            label(font, name, 9.0, Color::WHITE),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ));
        row.spawn((
            label(font, figures, 9.0, Color::WHITE),
            Node {
                margin: UiRect::right(Val::Px(3.0)),
                ..default()
            },
        ));
    });
}

/// A class colour deep enough for white text on it: the rogue's yellow and the priest's white
/// would swallow a name.
fn dim(color: Color) -> Color {
    let c = color.to_srgba();
    Color::srgb(c.red * 0.5, c.green * 0.5, c.blue * 0.5)
}
