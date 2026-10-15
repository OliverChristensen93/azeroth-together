//! The level-up screen: the run dims, "Level Up!" over the hand of cards, each card a spell to
//! learn or raise, in its rarity's colour, with exactly what it would do; under them the spells
//! the hero has, Skip and Reroll. 1, 2 and 3 (or a click) pick a card, R rerolls.
//!
//! Laid out on the 768-unit canvas (`UiScale`, set by [`super::menu::apply_ui_scale`]).

use bevy::prelude::*;
use together_sim::{ability_stats, AbilityKind, Card, CardKind, Hero, Rarity, MAX_ABILITIES};

use super::menu::SpellIcon;
use super::{content, LocalInput, Sim, LOCAL_PLAYER};

const CARD_W: f32 = 176.0;
const CARD_H: f32 = 262.0;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const GREEN: Color = Color::srgb(0.25, 1.0, 0.25);
const GREY: Color = Color::srgb(0.62, 0.62, 0.62);

#[derive(Component)]
pub(super) struct LevelUpRoot;

#[derive(Component)]
pub(super) struct LevelBadge;

/// The row the cards are built into, and the spell slots and buttons under it.
#[derive(Component)]
pub(super) struct CardRow;

#[derive(Component)]
pub(super) struct SpellRow;

#[derive(Component)]
pub(super) struct MoreText;

/// A card, by its index in the hand.
#[derive(Component)]
pub(super) struct CardButton(u8);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum HandButton {
    Skip,
    Reroll,
}

#[derive(Component)]
pub(super) struct RerollText;

#[derive(Resource)]
pub(super) struct Fonts {
    text: Handle<Font>,
    title: Handle<Font>,
}

fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// A common card's name is white on the dark card; its border a muted grey.
fn rarity_color(r: Rarity) -> Color {
    rgb(r.color())
}

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

/// [`label`] without the shadow, for dark text on a light badge, where a shadow reads as a blur.
fn flat_label(
    font: &Handle<Font>,
    text: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let fonts = Fonts {
        text: crate::char_select::wow_font(&asset_server),
        title: asset_server.load("mpq://Fonts/MORPHEUS.TTF"),
    };
    commands
        .spawn((
            LevelUpRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(300),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.85, 0.63, 0.1)),
                BorderColor::all(Color::srgb(1.0, 0.85, 0.4)),
            ))
            .with_children(|b| {
                b.spawn((flat_label(&fonts.text, "", 10.0, Color::BLACK), LevelBadge));
            });
            root.spawn((
                label(&fonts.title, "Level Up!", 46.0, GOLD),
                Node {
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
            ));
            root.spawn(label(
                &fonts.text,
                "Choose a spell. Its rarity decides how strong it is.",
                11.0,
                Color::srgb(0.85, 0.85, 0.85),
            ));
            root.spawn((
                CardRow,
                Node {
                    margin: UiRect::top(Val::Px(18.0)),
                    column_gap: Val::Px(14.0),
                    ..default()
                },
            ));
            root.spawn((Node {
                margin: UiRect::top(Val::Px(18.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },))
                .with_children(|bar| {
                    bar.spawn(label(&fonts.text, "SPELLS", 10.0, GREY));
                    bar.spawn((
                        SpellRow,
                        Node {
                            column_gap: Val::Px(4.0),
                            ..default()
                        },
                    ));
                    for (button, text) in [(HandButton::Skip, "Skip"), (HandButton::Reroll, "")] {
                        bar.spawn((
                            button,
                            Button,
                            Node {
                                margin: UiRect::left(Val::Px(8.0)),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.1, 0.05)),
                            BorderColor::all(Color::srgb(0.7, 0.55, 0.2)),
                        ))
                        .with_children(|b| {
                            let mut t = b.spawn(label(&fonts.text, text, 12.0, GOLD));
                            if button == HandButton::Reroll {
                                t.insert(RerollText);
                            }
                        });
                    }
                });
            root.spawn((
                MoreText,
                label(&fonts.text, "", 10.0, GREY),
                Node {
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
            ));
        });
    commands.insert_resource(fonts);
}

/// A card's body: what the spell does, and what this card changes.
fn card_lines(hero: &Hero, card: &Card) -> (String, Vec<String>) {
    let content = content();
    let def = &content.abilities[card.ability];
    let mut lines = Vec::new();
    let reach = |count: u32| match def.kind {
        AbilityKind::Nova => format!("{:.0} yd around you", def.radius),
        _ if count > 1 => format!("{count} targets within {:.0} yd", def.range),
        _ => format!("{:.0} yd range", def.range),
    };
    match card.kind {
        CardKind::Learn => {
            let s = ability_stats(def, 1, card.rarity.learn_bonus());
            lines.push(format!(
                "{:.0} damage every {:.1} sec",
                s.damage.round(),
                s.cooldown
            ));
            lines.push(reach(s.count));
        }
        CardKind::Upgrade => {
            let owned = hero.abilities.iter().find(|o| o.ability == card.ability);
            let (rank, bonus) = owned.map_or((1, 0.0), |o| (o.level, o.bonus));
            let now = ability_stats(def, rank, bonus);
            let next = ability_stats(def, rank + 1, bonus + card.rarity.upgrade_bonus());
            lines.push(format!(
                "Damage {:.0} » {:.0}",
                now.damage.round(),
                next.damage.round()
            ));
            lines.push(format!(
                "Cooldown {:.1} » {:.1} sec",
                now.cooldown, next.cooldown
            ));
            if next.count != now.count {
                lines.push(format!("Targets {} » {}", now.count, next.count));
            }
        }
    }
    (def.description.clone(), lines)
}

fn build_card(
    row: &mut ChildSpawnerCommands,
    fonts: &Fonts,
    hero: &Hero,
    index: usize,
    card: &Card,
) {
    let content = content();
    let def = &content.abilities[card.ability];
    let color = rarity_color(card.rarity);
    let border = if card.rarity == Rarity::Common {
        Color::srgb(0.45, 0.45, 0.45)
    } else {
        color
    };
    let rank = hero
        .abilities
        .iter()
        .find(|o| o.ability == card.ability)
        .map_or(1, |o| o.level);
    let (badge, sub) = match card.kind {
        CardKind::Learn => ("NEW".to_string(), "New spell".to_string()),
        CardKind::Upgrade => (
            format!("RANK {}", rank + 1),
            format!("Rank {rank} » {}", rank + 1),
        ),
    };
    let (description, lines) = card_lines(hero, card);
    let bonus = card.rarity.bonus(card.kind);
    let footer = match card.kind {
        CardKind::Learn => format!(
            "Takes a spell slot {}/{MAX_ABILITIES}",
            hero.abilities.len() + 1
        ),
        CardKind::Upgrade => "Improves a spell you know".into(),
    };

    row.spawn((
        CardButton(index as u8),
        Button,
        Node {
            width: Val::Px(CARD_W),
            height: Val::Px(CARD_H),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(30.0), Val::Px(10.0)),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.07, 0.06, 0.05, 0.95)),
        BorderColor::all(border),
    ))
    .with_children(|c| {
        // The key that picks it, top left; NEW or the rank it raises to, top right.
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(7.0),
                top: Val::Px(7.0),
                width: Val::Px(17.0),
                height: Val::Px(17.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.1, 0.08)),
            BorderColor::all(Color::srgb(0.6, 0.5, 0.3)),
        ))
        .with_children(|n| {
            n.spawn(label(
                &fonts.text,
                (index + 1).to_string(),
                9.0,
                Color::WHITE,
            ));
        });
        let (badge_bg, badge_fg) = match card.kind {
            CardKind::Learn if card.rarity != Rarity::Common => (color, Color::BLACK),
            CardKind::Learn => (Color::WHITE, Color::BLACK),
            CardKind::Upgrade => (Color::srgb(0.12, 0.1, 0.08), Color::WHITE),
        };
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(7.0),
                top: Val::Px(7.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(badge_bg),
            BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
        ))
        .with_children(|b| {
            b.spawn(flat_label(&fonts.text, badge, 8.0, badge_fg));
        });

        c.spawn((
            SpellIcon(def.visual),
            ImageNode {
                color: Color::srgba(0.2, 0.2, 0.2, 1.0),
                ..default()
            },
            Node {
                width: Val::Px(54.0),
                height: Val::Px(54.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.7, 0.7, 0.7)),
        ));
        c.spawn((
            label(
                &fonts.text,
                card.rarity.name().to_uppercase(),
                8.0,
                if card.rarity == Rarity::Common {
                    GREY
                } else {
                    color
                },
            ),
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
        ));
        c.spawn(label(&fonts.text, def.name.clone(), 16.0, color));
        c.spawn(label(&fonts.text, sub, 9.0, GREY));
        c.spawn((
            Node {
                width: Val::Percent(80.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
        ));
        c.spawn((
            label(&fonts.text, description, 9.5, Color::WHITE),
            TextLayout::new_with_justify(Justify::Center),
        ));
        for line in lines {
            c.spawn((
                label(&fonts.text, line, 9.5, Color::srgb(1.0, 0.9, 0.6)),
                TextLayout::new_with_justify(Justify::Center),
                Node {
                    margin: UiRect::top(Val::Px(2.0)),
                    ..default()
                },
            ));
        }
        if bonus > 0.0 {
            c.spawn((
                label(
                    &fonts.text,
                    format!("+{:.0}% damage", bonus * 100.0),
                    10.0,
                    GREEN,
                ),
                Node {
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
            ));
        }
        c.spawn((
            label(&fonts.text, footer, 8.0, GREY),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(8.0),
                ..default()
            },
        ));
    });
}

/// What the screen shows, to rebuild only when it changes: the level, the hand, and the hero's
/// spells and rerolls.
#[derive(PartialEq, Clone, Default)]
pub(super) struct Shown {
    level: u32,
    cards: Vec<(usize, u8, u8)>,
    spells: Vec<(u32, u8)>,
    rerolls: u32,
    more: usize,
}

/// Show the screen while the hero has cards to choose, rebuilt when the hand changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn refresh(
    mut commands: Commands,
    sim: Option<Res<Sim>>,
    fonts: Option<Res<Fonts>>,
    mut root: Query<&mut Visibility, With<LevelUpRoot>>,
    rows: Query<Entity, With<CardRow>>,
    spell_rows: Query<Entity, With<SpellRow>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<LevelBadge>>,
        Query<&mut Text, With<RerollText>>,
        Query<&mut Text, With<MoreText>>,
    )>,
    mut shown: Local<Option<Shown>>,
) {
    let hero = sim.as_ref().and_then(|s| s.world.hero(LOCAL_PLAYER));
    let offer = hero.and_then(|h| h.offers.first().map(|o| (h, o)));
    let want = if offer.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut root {
        if *v != want {
            *v = want;
        }
    }
    let (Some((hero, offer)), Some(sim), Some(fonts)) = (offer, sim.as_ref(), fonts) else {
        *shown = None;
        return;
    };
    let content = content();
    let now = Shown {
        level: sim.world.level,
        cards: offer
            .cards
            .iter()
            .map(|c| (c.ability, c.kind as u8, c.rarity as u8))
            .collect(),
        spells: hero
            .abilities
            .iter()
            .map(|o| (content.abilities[o.ability].visual, o.level))
            .collect(),
        rerolls: hero.rerolls,
        more: hero.offers.len() - 1,
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format!("LEVEL {}", now.level);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("Reroll {}   (R)", now.rerolls);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        t.0 = if now.more > 0 {
            format!("{} more level-ups after this one", now.more)
        } else {
            String::new()
        };
    }
    if let Ok(row) = rows.single() {
        commands.entity(row).despawn_related::<Children>();
        commands.entity(row).with_children(|r| {
            for (i, card) in offer.cards.iter().enumerate() {
                build_card(r, &fonts, hero, i, card);
            }
        });
    }
    if let Ok(row) = spell_rows.single() {
        commands.entity(row).despawn_related::<Children>();
        commands.entity(row).with_children(|r| {
            for i in 0..MAX_ABILITIES {
                let mut slot = r.spawn((
                    Node {
                        width: Val::Px(26.0),
                        height: Val::Px(26.0),
                        border: UiRect::all(Val::Px(1.0)),
                        justify_content: JustifyContent::End,
                        align_items: AlignItems::End,
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.4, 0.35, 0.25)),
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ));
                if let Some(&(visual, rank)) = now.spells.get(i) {
                    slot.insert((
                        SpellIcon(visual),
                        ImageNode {
                            color: Color::srgba(0.2, 0.2, 0.2, 1.0),
                            ..default()
                        },
                    ));
                    slot.with_children(|s| {
                        s.spawn(label(&fonts.text, rank.to_string(), 8.0, Color::WHITE));
                    });
                }
            }
        });
    }
    *shown = Some(now);
}

/// A click on a card picks it; on Skip or Reroll, does that. Cards and buttons light up under the
/// mouse.
#[allow(clippy::type_complexity)]
pub(super) fn clicks(
    mut input: ResMut<LocalInput>,
    mut cards: Query<
        (&CardButton, &Interaction, &mut BackgroundColor),
        (Changed<Interaction>, Without<HandButton>),
    >,
    mut buttons: Query<(&HandButton, &Interaction, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (card, interaction, mut bg) in &mut cards {
        match interaction {
            Interaction::Pressed => input.pick = Some(card.0),
            Interaction::Hovered => bg.0 = Color::srgba(0.14, 0.12, 0.09, 0.97),
            Interaction::None => bg.0 = Color::srgba(0.07, 0.06, 0.05, 0.95),
        }
    }
    for (button, interaction, mut bg) in &mut buttons {
        match interaction {
            Interaction::Pressed => match button {
                HandButton::Skip => input.skip = true,
                HandButton::Reroll => input.reroll = true,
            },
            Interaction::Hovered => bg.0 = Color::srgb(0.3, 0.17, 0.08),
            Interaction::None => bg.0 = Color::srgb(0.18, 0.1, 0.05),
        }
    }
}
