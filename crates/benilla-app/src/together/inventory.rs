//! What the hero carries and what it adds up to: a strip of its tomes and items under WoW's
//! player frame, each with its tooltip under the mouse ([`super::tooltip`]), and the stat sheet,
//! opened by the strip's Stats button or Tab. Both stay above the level-up screen, so a tome or
//! item card can be weighed against what the hero has.

use bevy::prelude::*;
use together_sim::{Hero, Item, Stat, BASE_CRIT};

use super::levelup::{item_icon, tome_icon, TextureIcon};
use super::tooltip::TipThing;
use super::{content, Phase, Sim};

/// An icon's size in the strip, in the 768-unit canvas.
const ICON: f32 = 30.0;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const GREY: Color = Color::srgb(0.7, 0.7, 0.7);

#[derive(Component)]
pub(super) struct InventoryRoot;

/// The row the tome and item icons are built in.
#[derive(Component)]
pub(super) struct Strip;

/// The Stats button, which opens and closes the sheet.
#[derive(Component)]
pub(super) struct SheetButton;

#[derive(Component)]
pub(super) struct Sheet;

/// The sheet's two columns: the stats' names and their values.
#[derive(Component)]
pub(super) struct SheetNames;

#[derive(Component)]
pub(super) struct SheetValues;

/// Whether the stat sheet is open. `TOGETHER_STATS` opens it, for a test shot.
#[derive(Resource)]
pub(super) struct SheetOpen(bool);

impl Default for SheetOpen {
    fn default() -> Self {
        Self(std::env::var_os("TOGETHER_STATS").is_some())
    }
}

#[derive(Resource)]
pub(super) struct InventoryFont(Handle<Font>);

fn label(font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(""),
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
    // Under WoW's player frame, top left.
    commands
        .spawn((
            InventoryRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(74.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            GlobalZIndex(350),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(Node {
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    SheetButton,
                    Button,
                    Node {
                        height: Val::Px(ICON),
                        padding: UiRect::horizontal(Val::Px(7.0)),
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.04, 0.02, 0.85)),
                    BorderColor::all(Color::srgb(0.6, 0.5, 0.3)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("Stats"),
                        TextFont {
                            font: font.clone(),
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(GOLD),
                        TextShadow::default(),
                    ));
                });
                row.spawn((
                    Strip,
                    Node {
                        column_gap: Val::Px(3.0),
                        row_gap: Val::Px(3.0),
                        flex_wrap: FlexWrap::Wrap,
                        max_width: Val::Px(9.0 * (ICON + 3.0)),
                        ..default()
                    },
                ));
            });
            root.spawn((
                Sheet,
                Node {
                    align_self: AlignSelf::FlexStart,
                    padding: UiRect::all(Val::Px(8.0)),
                    column_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.02, 0.07, 0.92)),
                BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
                Visibility::Hidden,
            ))
            .with_children(|s| {
                s.spawn((label(&font, 11.0, GREY), SheetNames));
                s.spawn((
                    label(&font, 11.0, Color::WHITE),
                    TextLayout::new_with_justify(Justify::Right),
                    SheetValues,
                ));
            });
        });
    commands.insert_resource(InventoryFont(font));
}

/// A signed percentage: "+12%", "-5%".
fn pct(v: f32) -> String {
    let p = (v * 100.0).round();
    if p < 0.0 {
        format!("{p:.0}%")
    } else {
        format!("+{p:.0}%")
    }
}

/// The stat sheet's lines: each stat's name and its total, from tomes, items and pacts alike.
fn sheet(hero: &Hero) -> Vec<(&'static str, String)> {
    let s = &hero.stats;
    let projectiles = s.get(Stat::Projectiles).round();
    vec![
        (
            "Health",
            format!("{:.0} / {:.0}", hero.hp.max(0.0), hero.max_hp),
        ),
        (
            "Regeneration",
            format!("{:.1} a second", s.get(Stat::Regen)),
        ),
        (
            "Armor",
            format!("{:.0}% less damage", s.get(Stat::Armor) * 100.0),
        ),
        ("Damage", pct(s.get(Stat::Damage))),
        ("Cooldown speed", pct(s.get(Stat::Cooldown))),
        ("Area", pct(s.get(Stat::Area))),
        (
            "Projectiles",
            if projectiles < 0.0 {
                format!("{projectiles:.0}")
            } else {
                format!("+{projectiles:.0}")
            },
        ),
        (
            "Critical strike",
            format!("{:.0}%", (BASE_CRIT + s.get(Stat::Crit)).min(1.0) * 100.0),
        ),
        (
            "Critical damage",
            format!("{:.0}%", (2.0 + s.get(Stat::CritDamage)) * 100.0),
        ),
        ("Move speed", pct(s.get(Stat::Speed))),
        ("Pickup range", pct(s.get(Stat::Pickup))),
        ("Luck", pct(s.get(Stat::Luck))),
        ("Experience", pct(s.get(Stat::XpGain))),
        ("Gold", pct(s.get(Stat::GoldGain))),
        ("Rerolls", hero.rerolls.to_string()),
    ]
}

/// One icon in the strip, its count in the corner and its tooltip under the mouse.
fn icon(
    row: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    name: &'static str,
    count: String,
    tip: TipThing,
) {
    row.spawn((
        ImageNode {
            color: Color::srgba(0.2, 0.2, 0.2, 1.0),
            ..default()
        },
        Node {
            width: Val::Px(ICON),
            height: Val::Px(ICON),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
        TextureIcon(name),
        tip,
        Interaction::default(),
    ))
    .with_children(|i| {
        i.spawn((
            Text::new(count),
            TextFont {
                font: font.clone(),
                font_size: 9.0,
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow::default(),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(1.0),
                bottom: Val::Px(0.0),
                ..default()
            },
        ));
    });
}

/// The strip rebuilt when the hero's tomes or items change, the sheet kept up to date, and
/// both shown only in a run.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    keys: Res<ButtonInput<KeyCode>>,
    font: Option<Res<InventoryFont>>,
    mut open: ResMut<SheetOpen>,
    button: Query<&Interaction, (Changed<Interaction>, With<SheetButton>)>,
    mut root: Query<&mut Visibility, (With<InventoryRoot>, Without<Sheet>)>,
    mut sheet_vis: Query<&mut Visibility, (With<Sheet>, Without<InventoryRoot>)>,
    strip: Query<Entity, With<Strip>>,
    mut names: Query<&mut Text, (With<SheetNames>, Without<SheetValues>)>,
    mut values: Query<&mut Text, (With<SheetValues>, Without<SheetNames>)>,
    mut shown: Local<Option<(Vec<(Stat, u8)>, Vec<(Item, u8)>)>>,
) {
    let hero = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .and_then(|sim| sim.world.hero(sim.me));
    let want = if hero.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut root {
        if *v != want {
            *v = want;
        }
    }
    let Some(hero) = hero else {
        *shown = None;
        return;
    };
    if keys.just_pressed(KeyCode::Tab) || button.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = !open.0;
    }
    let sheet_want = if open.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut sheet_vis {
        if *v != sheet_want {
            *v = sheet_want;
        }
    }
    if open.0 {
        let lines = sheet(hero);
        let n = lines.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("\n");
        let v = lines
            .iter()
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Ok(mut t) = names.single_mut() {
            if t.0 != n {
                t.0 = n;
            }
        }
        if let Ok(mut t) = values.single_mut() {
            if t.0 != v {
                t.0 = v;
            }
        }
    }

    let now = (hero.tomes.clone(), hero.items.clone());
    if shown.as_ref() == Some(&now) {
        return;
    }
    let (Ok(strip), Some(font)) = (strip.single(), font) else {
        return;
    };
    let rules = &content().rules;
    commands.entity(strip).despawn_related::<Children>();
    commands.entity(strip).with_children(|row| {
        for &(stat, rank) in &now.0 {
            icon(
                row,
                &font.0,
                tome_icon(stat),
                format!("{rank}/{}", rules.tome_max_rank),
                TipThing::Tome(stat),
            );
        }
        for &(item, n) in &now.1 {
            icon(
                row,
                &font.0,
                item_icon(item),
                format!("x{n}"),
                TipThing::Item(item),
            );
        }
    });
    *shown = Some(now);
}
