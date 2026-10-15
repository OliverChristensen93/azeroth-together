//! The card screen: the run dims, a title over the hand of cards, each card in its rarity's
//! colour with exactly what it would do; under them the spells the hero has, Skip and Reroll.
//! 1, 2 and 3 (or a click) pick a card, R rerolls. A level-up deals spells and tomes; a shrine,
//! an altar or a chest deals its own hand under its own name.
//!
//! Laid out on the 768-unit canvas (`UiScale`, set by [`super::menu::apply_ui_scale`]).

use bevy::prelude::*;
use together_sim::{
    ability_stats, AbilityKind, Card, CardKind, Hero, InteractableKind, Rarity, Source, Stat,
    MAX_ABILITIES,
};

use super::menu::SpellIcon;
use super::{content, LocalInput, Sim};

const CARD_W: f32 = 176.0;
const CARD_H: f32 = 262.0;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const GREEN: Color = Color::srgb(0.25, 1.0, 0.25);
const RED: Color = Color::srgb(1.0, 0.3, 0.25);
const GREY: Color = Color::srgb(0.62, 0.62, 0.62);

#[derive(Component)]
pub(super) struct LevelUpRoot;

#[derive(Component)]
pub(super) struct LevelBadge;

/// The title over the hand, and the line under it.
#[derive(Component)]
pub(super) struct TitleText;

#[derive(Component)]
pub(super) struct HintText;

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
                TitleText,
            ));
            root.spawn((
                label(&fonts.text, "", 11.0, Color::srgb(0.85, 0.85, 0.85)),
                HintText,
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

/// The spell whose icon a tome of `stat` wears: one of the shipped abilities, whose icons the
/// game data is sure to have.
fn stat_icon(stat: Stat) -> u32 {
    match stat {
        // Mortal Strike, Blade Flurry, Arcane Explosion, Hurricane.
        Stat::Damage => 12294,
        Stat::Cooldown => 13877,
        Stat::Area => 1449,
        Stat::Speed => 16914,
        // Holy Wrath, Holy Nova, Thunder Clap, Arcane Missiles.
        Stat::Health => 2812,
        Stat::Regen => 15237,
        Stat::Armor => 6343,
        Stat::Pickup => 7268,
        // Starfire, Mind Blast, Judgement, Aimed Shot, Multi-Shot.
        Stat::Luck => 2912,
        Stat::XpGain => 8092,
        Stat::GoldGain => 20271,
        Stat::Crit => 19434,
        Stat::Projectiles => 2643,
    }
}

/// What a card shows.
struct Face {
    /// The spell whose icon it wears.
    icon: u32,
    name: String,
    /// The line under the name.
    sub: String,
    /// The corner badge: NEW, the rank it raises to, TOME or PACT.
    badge: String,
    /// Whether the badge stands out in the card's colour, rather than dark.
    bright: bool,
    description: String,
    /// What it changes, each line in its colour.
    lines: Vec<(String, Color)>,
    footer: String,
}

/// The face of a spell card: what the spell does, and what this card changes.
fn spell_face(hero: &Hero, ability: usize, learn: bool, rarity: Rarity) -> Face {
    let content = content();
    let def = &content.abilities[ability];
    let reach = |count: u32| match def.kind {
        AbilityKind::Nova => format!("{:.0} yd around you", def.radius),
        _ if count > 1 => format!("{count} targets within {:.0} yd", def.range),
        _ => format!("{:.0} yd range", def.range),
    };
    let text = |s: String| (s, Color::srgb(1.0, 0.9, 0.6));
    let mut lines = Vec::new();
    let (badge, sub, bonus, footer) = if learn {
        let s = ability_stats(def, 1, rarity.learn_bonus()).with(&hero.stats);
        lines.push(text(format!(
            "{:.0} damage every {:.1} sec",
            s.damage.round(),
            s.cooldown
        )));
        lines.push(text(reach(s.count)));
        (
            "NEW".to_string(),
            "New spell".to_string(),
            rarity.learn_bonus(),
            format!(
                "Takes a spell slot {}/{MAX_ABILITIES}",
                hero.abilities.len() + 1
            ),
        )
    } else {
        let owned = hero.abilities.iter().find(|o| o.ability == ability);
        let (rank, bonus) = owned.map_or((1, 0.0), |o| (o.level, o.bonus));
        let now = ability_stats(def, rank, bonus).with(&hero.stats);
        let next = ability_stats(def, rank + 1, bonus + rarity.upgrade_bonus()).with(&hero.stats);
        lines.push(text(format!(
            "Damage {:.0} » {:.0}",
            now.damage.round(),
            next.damage.round()
        )));
        lines.push(text(format!(
            "Cooldown {:.1} » {:.1} sec",
            now.cooldown, next.cooldown
        )));
        if next.count != now.count {
            lines.push(text(format!("Targets {} » {}", now.count, next.count)));
        }
        (
            format!("RANK {}", rank + 1),
            format!("Rank {rank} » {}", rank + 1),
            rarity.upgrade_bonus(),
            "Improves a spell you know".to_string(),
        )
    };
    if bonus > 0.0 {
        lines.push((format!("+{:.0}% damage", bonus * 100.0), GREEN));
    }
    Face {
        icon: def.visual,
        name: def.name.clone(),
        sub,
        badge,
        bright: learn,
        description: def.description.clone(),
        lines,
        footer,
    }
}

/// A stat as it stands on the hero, for "now » after" lines.
fn stat_now(hero: &Hero, stat: Stat) -> String {
    stat.describe(hero.stats.get(stat))
}

/// The face of any card.
fn face(hero: &Hero, card: &Card) -> Face {
    match card.kind {
        CardKind::Learn(a) => spell_face(hero, a, true, card.rarity),
        CardKind::Upgrade(a) => spell_face(hero, a, false, card.rarity),
        CardKind::Tome(stat) => Face {
            icon: stat_icon(stat),
            name: stat.tome().into(),
            sub: stat.name().into(),
            badge: "TOME".into(),
            bright: true,
            description: "Raises a stat for the rest of the run.".into(),
            lines: vec![
                (stat.describe(stat.tome_amount(card.rarity)), GREEN),
                (format!("You have {}", stat_now(hero, stat)), GREY),
            ],
            footer: "Stacks with every other tome".into(),
        },
        CardKind::Pact(pact) => Face {
            icon: stat_icon(pact.boon),
            name: format!("Pact of {}", pact.boon.name()),
            sub: "A bargain with the altar".into(),
            badge: "PACT".into(),
            bright: true,
            description: "A great boon, at a price.".into(),
            lines: vec![
                (pact.boon.describe(pact.boon_amount(card.rarity)), GREEN),
                (pact.bane.describe(), RED),
            ],
            footer: if pact.bane.shared() {
                "The bane falls on the whole party".into()
            } else {
                "The bane is yours alone".into()
            },
        },
    }
}

fn build_card(
    row: &mut ChildSpawnerCommands,
    fonts: &Fonts,
    hero: &Hero,
    index: usize,
    card: &Card,
) {
    let color = rarity_color(card.rarity);
    let border = if card.rarity == Rarity::Common {
        Color::srgb(0.45, 0.45, 0.45)
    } else {
        color
    };
    let face = face(hero, card);

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
        // The key that picks it, top left; what kind of card it is, top right.
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
        let (badge_bg, badge_fg) = match (face.bright, card.rarity) {
            (true, Rarity::Common) => (Color::WHITE, Color::BLACK),
            (true, _) => (color, Color::BLACK),
            (false, _) => (Color::srgb(0.12, 0.1, 0.08), Color::WHITE),
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
            b.spawn(flat_label(&fonts.text, face.badge.clone(), 8.0, badge_fg));
        });

        c.spawn((
            SpellIcon(face.icon),
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
        c.spawn((
            label(&fonts.text, face.name, 15.0, color),
            TextLayout::new_with_justify(Justify::Center),
        ));
        c.spawn(label(&fonts.text, face.sub, 9.0, GREY));
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
            label(&fonts.text, face.description, 9.5, Color::WHITE),
            TextLayout::new_with_justify(Justify::Center),
        ));
        for (line, tint) in face.lines {
            c.spawn((
                label(&fonts.text, line, 9.5, tint),
                TextLayout::new_with_justify(Justify::Center),
                Node {
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                },
            ));
        }
        c.spawn((
            label(&fonts.text, face.footer, 8.0, GREY),
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
    source: Option<Source>,
    cards: Vec<Card>,
    spells: Vec<(u32, u8)>,
    rerolls: u32,
    more: usize,
}

/// The screen's title and the line under it, for a hand from `source`.
fn titles(source: Source) -> (&'static str, &'static str) {
    let Source::Interactable(kind) = source else {
        return (
            "Level Up!",
            "Choose a spell or a tome. Its rarity decides how strong it is.",
        );
    };
    let hint = match kind {
        InteractableKind::TitanShrine | InteractableKind::GoldenShrine => {
            "The titans offer their knowledge. Choose a tome."
        }
        InteractableKind::CursedAltar => "Every boon has its bane. Choose wisely, or skip.",
        _ => "Choose your treasure.",
    };
    (kind.name(), hint)
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
        Query<&mut Text, With<TitleText>>,
        Query<&mut Text, With<HintText>>,
    )>,
    mut shown: Local<Option<Shown>>,
) {
    // A fallen hero chooses once raised.
    let hero = sim
        .as_ref()
        .and_then(|s| s.world.hero(s.me))
        .filter(|h| h.alive);
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
        source: Some(offer.source),
        cards: offer.cards.clone(),
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
    let (title, hint) = titles(offer.source);
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format!("LEVEL {}", now.level);
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        t.0 = title.into();
    }
    if let Ok(mut t) = texts.p4().single_mut() {
        t.0 = hint.into();
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("Reroll {}   (R)", now.rerolls);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        t.0 = if now.more > 0 {
            format!("{} more hands to choose after this one", now.more)
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
