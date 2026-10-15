//! A plain text HUD: the local hero's health, the party's level and experience, the clock and the
//! kills; the level-up cards while they are up; and the end of the run.

use bevy::prelude::*;
use together_sim::Card;

use super::{Sim, LOCAL_PLAYER};

#[derive(Component)]
pub(super) struct StatusText;

#[derive(Component)]
pub(super) struct CardsText;

#[derive(Component)]
pub(super) struct CenterPanel;

/// The run's status line and hint, shown only in a run.
#[derive(Component)]
pub(super) struct HudTop;

/// The run's HUD shows in a run; at the menu the status line and any panel are hidden.
pub(super) fn show_hud(
    phase: Res<State<super::Phase>>,
    mut top: Query<&mut Visibility, (With<HudTop>, Without<CenterPanel>)>,
    mut panel: Query<&mut Visibility, (With<CenterPanel>, Without<HudTop>)>,
) {
    let playing = *phase.get() == super::Phase::Playing;
    let want = if playing {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut top {
        if *v != want {
            *v = want;
        }
    }
    if !playing {
        for mut v in &mut panel {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
    }
}

fn text(font: &Handle<Font>, size: f32) -> (Text, TextFont, TextColor, TextShadow) {
    (
        Text::new(""),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.82, 0.0)),
        TextShadow::default(),
    )
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    // Top centre, clear of WoW's player and target frames (left) and minimap (right).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Px(6.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            HudTop,
        ))
        .with_children(|p| {
            p.spawn((
                text(&font, 20.0),
                TextLayout::new_with_justify(Justify::Center),
                StatusText,
            ));
            p.spawn((
                Text::new("WoW controls   ·   1 2 3 pick a level-up card"),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                TextShadow::default(),
            ));
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                // High enough to leave the hero, in the middle of the screen, in view.
                top: Val::Percent(9.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            CenterPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::all(Val::Px(18.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ))
            .with_children(|b| {
                b.spawn((
                    text(&font, 28.0),
                    TextLayout::new_with_justify(Justify::Center),
                    CardsText,
                ));
            });
        });
}

fn clock(secs: f32) -> String {
    let s = secs as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

pub(super) fn refresh(
    sim: Res<Sim>,
    mut status: Query<&mut Text, (With<StatusText>, Without<CardsText>)>,
    mut cards: Query<&mut Text, (With<CardsText>, Without<StatusText>)>,
    mut panel: Query<&mut Visibility, With<CenterPanel>>,
) {
    let world = &sim.world;
    let content = world.content();
    let Some(hero) = world.hero(LOCAL_PLAYER) else {
        return;
    };
    if let Ok(mut t) = status.single_mut() {
        let owned: Vec<String> = hero
            .abilities
            .iter()
            .map(|o| format!("{} {}", content.abilities[o.ability].name, o.level))
            .collect();
        // Health, level and experience are in WoW's own player frame and XP bar.
        let s = format!(
            "{}   Kills {}\n{}",
            clock(world.elapsed),
            world.kills,
            owned.join("   "),
        );
        if t.0 != s {
            t.0 = s;
        }
    }

    let center = if world.game_over() {
        Some(format!(
            "You have fallen.\n\nSurvived {}  ·  Level {}  ·  {} kills  ·  {:.0} damage\n\nPress Enter to return to the menu",
            clock(world.elapsed),
            world.level,
            world.kills,
            hero.damage_done,
        ))
    } else if let Some(offer) = hero.offers.first() {
        let mut s = format!("Level {}! Choose one:\n\n", world.level);
        for (i, card) in offer.cards.iter().enumerate() {
            let line = match *card {
                Card::Learn(a) => format!("{}  {}  (new)", i + 1, content.abilities[a].name),
                Card::Upgrade(a) => {
                    let level = hero
                        .abilities
                        .iter()
                        .find(|o| o.ability == a)
                        .map_or(1, |o| o.level);
                    format!(
                        "{}  {}  (rank {})",
                        i + 1,
                        content.abilities[a].name,
                        level + 1
                    )
                }
            };
            s.push_str(&line);
            s.push('\n');
        }
        if hero.offers.len() > 1 {
            s.push_str(&format!("\n({} more after this)", hero.offers.len() - 1));
        }
        Some(s)
    } else if world.paused() {
        Some("Waiting for the others to choose…".into())
    } else {
        None
    };
    if let Ok(mut vis) = panel.single_mut() {
        let want = if center.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    if let (Some(s), Ok(mut t)) = (center, cards.single_mut()) {
        if t.0 != s {
            t.0 = s;
        }
    }
}
