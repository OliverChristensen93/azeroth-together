//! A damage meter in the style of WoW's damage-meter addons: "Damage Done" with the run's damage
//! per second, and a row per ability, most damage first, as icon, bar, name and
//! `total (per second, share)`. Under the minimap, during a run.
//!
//! Laid out on the 768-unit canvas (`UiScale`, set by [`super::menu::apply_ui_scale`]).

use bevy::prelude::*;

use super::menu::SpellIcon;
use super::{content, Phase, Sim, LOCAL_PLAYER};

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

#[derive(Component)]
pub(super) struct MeterRoot;

#[derive(Component)]
pub(super) struct MeterRows;

#[derive(Component)]
pub(super) struct MeterTotal;

#[derive(Resource)]
pub(super) struct MeterFont(Handle<Font>);

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

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            MeterRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                top: Val::Px(172.0),
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
                    ..default()
                },
                BackgroundColor(Color::srgba(0.16, 0.12, 0.07, 0.95)),
            ))
            .with_children(|t| {
                t.spawn(label(
                    &font,
                    "Damage Done",
                    10.0,
                    Color::srgb(1.0, 0.82, 0.0),
                ));
                t.spawn((
                    label(&font, "", 9.0, Color::srgb(0.85, 0.85, 0.85)),
                    MeterTotal,
                ));
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

/// Show the meter in a run, redrawn every [`EVERY`] seconds from the hero's damage by ability.
#[allow(clippy::too_many_arguments)]
pub(super) fn refresh(
    mut commands: Commands,
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    font: Option<Res<MeterFont>>,
    mut root: Query<&mut Visibility, With<MeterRoot>>,
    rows: Query<Entity, With<MeterRows>>,
    mut total_text: Query<&mut Text, With<MeterTotal>>,
    mut next: Local<f32>,
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
    if !playing || time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + EVERY;
    let Some(hero) = sim.world.hero(LOCAL_PLAYER) else {
        return;
    };
    let content = content();
    let secs = f64::from(sim.world.elapsed.max(1.0));
    let total = hero.damage_done;
    if let Ok(mut t) = total_text.single_mut() {
        let s = format!("{:.0} dps", total / secs);
        if t.0 != s {
            t.0 = s;
        }
    }
    let mut by: Vec<(usize, f64)> = hero.damage_by.clone();
    by.sort_by(|a, b| b.1.total_cmp(&a.1));
    let top = by.first().map_or(1.0, |r| r.1.max(1.0));
    let Ok(rows) = rows.single() else {
        return;
    };
    commands.entity(rows).despawn_related::<Children>();
    commands.entity(rows).with_children(|r| {
        if by.is_empty() {
            r.spawn(label(
                &font.0,
                "No damage yet",
                9.0,
                Color::srgb(0.6, 0.6, 0.6),
            ));
        }
        for (i, &(ability, dealt)) in by.iter().enumerate() {
            let def = &content.abilities[ability];
            let share = if total > 0.0 { dealt / total } else { 0.0 };
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
                        width: Val::Percent((dealt / top * 100.0) as f32),
                        ..default()
                    },
                    BackgroundColor(BARS[i % BARS.len()].with_alpha(0.85)),
                ));
                row.spawn((
                    SpellIcon(def.visual),
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
                row.spawn((
                    label(
                        &font.0,
                        format!("{}. {}", i + 1, def.name),
                        9.0,
                        Color::WHITE,
                    ),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                row.spawn((
                    label(
                        &font.0,
                        format!("{:.0} ({:.0}, {:.0}%)", dealt, dealt / secs, share * 100.0),
                        9.0,
                        Color::WHITE,
                    ),
                    Node {
                        margin: UiRect::right(Val::Px(3.0)),
                        ..default()
                    },
                ));
            });
        }
    });
}
