//! A plain text HUD: the clock, the kills and the hero's spells (and, joined, the ping to the
//! host); fallen friends and how to raise them; and the end of the run.

use bevy::prelude::*;

use together_sim::{REVIVE_RANGE, REVIVE_SECS};

use super::Sim;

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
                top: Val::Px(3.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            HudTop,
        ))
        .with_children(|p| {
            p.spawn((
                text(&font, 12.0),
                TextLayout::new_with_justify(Justify::Center),
                StatusText,
            ));
            p.spawn((
                Text::new("WoW controls   ·   level-up: 1 2 3 or click a card, R rerolls"),
                TextFont {
                    font: font.clone(),
                    font_size: 9.0,
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
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ))
            .with_children(|b| {
                b.spawn((
                    text(&font, 16.0),
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
    net: Res<super::session::Net>,
) {
    let world = &sim.world;
    let content = world.content();
    let Some(hero) = world.hero(sim.me) else {
        return;
    };
    if let Ok(mut t) = status.single_mut() {
        let owned: Vec<String> = hero
            .abilities
            .iter()
            .map(|o| format!("{} {}", content.abilities[o.ability].name, o.level))
            .collect();
        // Health, level and experience are in WoW's own player frame and XP bar.
        let ping = net
            .ping_ms()
            .map_or(String::new(), |ms| format!("   Ping {ms} ms"));
        let mut s = format!(
            "{}   Kills {}{ping}\n{}",
            clock(world.elapsed),
            world.kills,
            owned.join("   "),
        );
        // Friends who have fallen, while the run goes on.
        if hero.alive && !world.game_over() {
            for h in world.heroes.iter().filter(|h| !h.alive) {
                s.push_str(&format!(
                    "\n{} has fallen: stand beside them to raise them",
                    h.name
                ));
            }
        }
        if t.0 != s {
            t.0 = s;
        }
    }

    let center = if world.game_over() {
        let fallen = if world.heroes.len() > 1 {
            "The party has fallen."
        } else {
            "You have fallen."
        };
        let next = if net.is_client() {
            "The host takes everyone back to the menu"
        } else {
            "Press Enter to return to the menu"
        };
        Some(format!(
            "{fallen}\n\nSurvived {}  ·  Level {}  ·  {} kills  ·  {:.0} damage\n\n{next}",
            clock(world.elapsed),
            world.level,
            world.kills,
            hero.damage_done,
        ))
    } else if !hero.alive {
        let raising = if hero.revive > 0.0 {
            format!("Being raised…  {:.0}%", hero.revive * 100.0)
        } else {
            format!("A friend who stands beside you for {REVIVE_SECS:.0} seconds raises you.")
        };
        Some(format!("You have fallen.\n\n{raising}"))
    } else if !hero.offers.is_empty() {
        // The level-up screen ([`super::levelup`]) is up.
        None
    } else if let Some(down) = world
        .heroes
        .iter()
        .find(|h| !h.alive && h.revive > 0.0 && h.pos.distance(hero.pos) <= REVIVE_RANGE)
    {
        Some(format!(
            "Raising {}…  {:.0}%",
            down.name,
            down.revive * 100.0
        ))
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
