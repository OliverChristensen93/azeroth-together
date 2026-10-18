//! The card screen: the run dims, a title over the hand of cards, each card in its rarity's
//! colour with exactly what it would do; under them the spells the hero has, Skip and Reroll.
//! 1, 2 and 3 (or a click) pick a card, R rerolls, T freezes the hand (it comes back at the next
//! level-up, less the card taken), G then a card banishes its spell for the run. A level-up
//! deals spells and tomes; a shrine,
//! an altar or a chest deals its own hand under its own name.
//!
//! Laid out on the 768-unit canvas (`UiScale`, set by [`super::menu::apply_ui_scale`]).

use bevy::prelude::*;
use together_sim::{
    ability_stats, describe, fallback_gold, shield_size, AbilityKind, Augment, Boost, Card,
    CardKind, CastSpec, Hero, InteractableKind, Item, Offer, Rarity, Source, Stat, MAX_ABILITIES,
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
pub(super) struct CardButton(pub(super) u8);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum HandButton {
    Skip,
    Reroll,
    Freeze,
    Banish,
}

/// The next card clicked (or its key pressed) banishes its spell instead of taking it.
#[derive(Resource, Default)]
pub(super) struct BanishMode(pub bool);

#[derive(Component)]
pub(super) struct FreezeText;

#[derive(Component)]
pub(super) struct BanishText;

#[derive(Component)]
pub(super) struct RerollText;

/// The Skip button's caption: Hold on an augment screen.
#[derive(Component)]
pub(super) struct SkipText;

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
            // The dimming is under it, with the rays ([`super::flair`]).
            BackgroundColor(Color::NONE),
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
                    for (button, text) in [
                        (HandButton::Skip, "Skip"),
                        (HandButton::Reroll, ""),
                        (HandButton::Freeze, ""),
                        (HandButton::Banish, ""),
                    ] {
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
                            match button {
                                HandButton::Reroll => t.insert(RerollText),
                                HandButton::Skip => t.insert(SkipText),
                                HandButton::Freeze => t.insert(FreezeText),
                                HandButton::Banish => t.insert(BanishText),
                            };
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

/// What a card shows in its icon slot: a spell's own icon, or an icon texture by path (a book
/// for a tome, the item's own for an item), so a tome or an item never looks like a spell.
#[derive(Clone, Copy)]
enum Icon {
    Spell(u32),
    Texture(&'static str),
}

/// An icon slot filled from a texture under `Interface\Icons`, by name.
#[derive(Component)]
pub(super) struct TextureIcon(pub &'static str);

/// The book a tome of `stat` wears.
pub(super) fn tome_icon(stat: Stat) -> &'static str {
    match stat {
        Stat::Damage => "INV_Misc_Book_07",
        Stat::Cooldown => "INV_Misc_Book_08",
        Stat::Area => "INV_Misc_Book_06",
        Stat::Speed => "INV_Misc_Book_04",
        Stat::Health => "INV_Misc_Book_09",
        Stat::Regen => "INV_Misc_Book_03",
        Stat::Armor => "INV_Misc_Book_10",
        Stat::Pickup => "INV_Misc_Book_05",
        Stat::Luck => "INV_Misc_Book_11",
        Stat::XpGain => "INV_Misc_Book_12",
        Stat::GoldGain => "INV_Misc_Book_02",
        Stat::Crit => "INV_Misc_Book_01",
        Stat::CritDamage => "Ability_Racial_BloodRage",
        Stat::Projectiles => "INV_Misc_Book_13",
    }
}

/// The icon an item wears: a gem, a fang, a key, a charm, a ring, a bag.
pub(super) fn item_icon(item: Item) -> &'static str {
    match item {
        Item::StaticShard => "INV_Misc_Gem_Sapphire_02",
        Item::EmberCore => "INV_Misc_Gem_Ruby_02",
        Item::VampiricFang => "INV_Misc_MonsterFang_01",
        Item::SkeletonKey => "INV_Misc_Key_03",
        Item::RabbitsFoot => "INV_Jewelry_Talisman_04",
        Item::BerserkersRing => "INV_Jewelry_Ring_15",
        Item::FrostWard => "INV_Misc_Gem_Diamond_02",
        Item::GoblinPurse => "INV_Misc_Bag_10",
    }
}

/// What kind of card it is: named on the card under its icon, and tinting its background.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Spell,
    Tome,
    Item,
    Pact,
    /// A general augment for a spell.
    Augment,
    /// One of a spell's capstones.
    Capstone,
    /// Gold, a heal or a reroll, when nothing else is left.
    Fallback,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Spell => "SPELL",
            Kind::Tome => "TOME",
            Kind::Item => "ITEM",
            Kind::Pact => "PACT",
            Kind::Augment => "AUGMENT",
            Kind::Capstone => "CAPSTONE",
            Kind::Fallback => "BOON",
        }
    }

    /// The card's background: brown for spells, blue for tomes, green for items, purple for
    /// pacts, grey for the rest.
    fn tint(self) -> Color {
        match self {
            Kind::Spell => Color::srgba(0.07, 0.06, 0.05, 0.95),
            Kind::Tome => Color::srgba(0.04, 0.07, 0.15, 0.95),
            Kind::Item => Color::srgba(0.04, 0.11, 0.06, 0.95),
            Kind::Pact => Color::srgba(0.11, 0.04, 0.12, 0.95),
            Kind::Augment => Color::srgba(0.03, 0.10, 0.12, 0.95),
            Kind::Capstone => Color::srgba(0.13, 0.08, 0.02, 0.95),
            Kind::Fallback => Color::srgba(0.09, 0.09, 0.09, 0.95),
        }
    }
}

/// A card's background colour, which it lightens to under the mouse.
#[derive(Component)]
pub(super) struct CardTint(Color);

/// Fill [`TextureIcon`] slots once the asset reader is up.
pub(super) fn fill_texture_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &TextureIcon), Without<TextureDone>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(assets) = assets.as_deref_mut() else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let path = format!(r"Interface\Icons\{}.blp", icon.0);
        if let Some(h) = assets.sprite_texture(&path, &mut images) {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(TextureDone);
    }
}

/// A [`TextureIcon`] slot already filled.
#[derive(Component)]
pub(super) struct TextureDone;
/// What a card shows.
struct Face {
    icon: Icon,
    kind: Kind,
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

/// The face of a spell card: a new spell's numbers, or just what an upgrade's `boosts` give.
fn spell_face(
    hero: &Hero,
    ability: usize,
    upgrade: Option<[Option<Boost>; 3]>,
    rarity: Rarity,
) -> Face {
    let learn = upgrade.is_none();
    let content = content();
    let def = &content.abilities[ability];
    let text = |s: String| (s, Color::srgb(1.0, 0.9, 0.6));
    let mut lines = Vec::new();
    let (badge, sub, bonus, footer) = if learn {
        // The description has every other number; a shield's gives its cooldown too.
        if def.kind != AbilityKind::Shield {
            lines.push(text(format!(
                "Every {:.1} sec",
                ability_stats(def, rarity.learn_bonus())
                    .with(&hero.stats)
                    .cooldown
            )));
        }
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
        let rank = owned.map_or(1, |o| o.level);
        for boost in upgrade.into_iter().flatten().flatten() {
            lines.push((boost.describe(boost.amount(rarity)), GREEN));
        }
        let milestone = match rank + 1 {
            5 | 15 => Some("an augment"),
            10 if !Augment::capstones_of(&def.key).is_empty() => Some("its capstone"),
            10 => Some("an augment"),
            _ => None,
        };
        if let Some(what) = milestone {
            lines.push((format!("Rank {} brings {what}", rank + 1), GOLD));
        }
        (
            format!("RANK {}", rank + 1),
            format!("Rank {rank} » {}", rank + 1),
            0.0,
            "Hover the icon for the spell".to_string(),
        )
    };
    if bonus > 0.0 {
        lines.push((format!("+{:.0}% damage", bonus * 100.0), GREEN));
    }
    Face {
        icon: Icon::Spell(def.visual),
        kind: Kind::Spell,
        name: def.name.clone(),
        sub,
        badge,
        bright: learn,
        // An upgrade shows only what it gives; the spell itself is on its icon's tooltip.
        description: if learn {
            learn_text(hero, ability, rarity)
        } else {
            String::new()
        },
        lines,
        footer,
    }
}

/// A new spell's description with its numbers as it would be learned now: the card's bonus and
/// the hero's stats on it.
fn learn_text(hero: &Hero, ability: usize, rarity: Rarity) -> String {
    let def = &content().abilities[ability];
    let st = ability_stats(def, rarity.learn_bonus()).with(&hero.stats);
    let mut s = CastSpec::base(ability, def);
    s.damage = st.damage;
    s.cooldown = st.cooldown;
    s.count = st.count;
    s.range = st.range;
    s.radius = st.radius;
    s.shield = shield_size(def.shield, rarity.learn_bonus());
    describe(def, &s, hero.max_hp)
}

/// A stat as it stands on the hero, for "now » after" lines.
fn stat_now(hero: &Hero, stat: Stat) -> String {
    stat.describe(hero.stats.get(stat))
}

/// The face of any card, at the party's `level`.
fn face(hero: &Hero, level: u32, card: &Card) -> Face {
    match card.kind {
        CardKind::Learn(a) => spell_face(hero, a, None, card.rarity),
        CardKind::Upgrade(a, boosts) => spell_face(hero, a, Some(boosts), card.rarity),
        CardKind::Tome(stat) => {
            let rules = &content().rules;
            let rank = hero.tome_rank(stat);
            let (sub, footer) = if rank == 0 {
                (
                    format!("New tome  ·  rank 1 of {}", rules.tome_max_rank),
                    format!(
                        "Takes a tome slot {}/{}",
                        hero.tomes.len() + 1,
                        rules.tome_slots
                    ),
                )
            } else {
                (
                    format!("Rank {rank} » {} of {}", rank + 1, rules.tome_max_rank),
                    "Raises a tome you hold".to_string(),
                )
            };
            Face {
                icon: Icon::Texture(tome_icon(stat)),
                kind: Kind::Tome,
                name: stat.tome().into(),
                sub,
                badge: format!("{}/{}", rank + 1, rules.tome_max_rank),
                bright: true,
                description: format!("Raises your {}.", stat.name().to_lowercase()),
                lines: vec![
                    (stat.describe(stat.tome_amount(card.rarity)), GREEN),
                    (format!("You have {}", stat_now(hero, stat)), GREY),
                ],
                footer,
            }
        }
        CardKind::Item(item) => {
            let held = hero.stacks(item);
            let max = item.max_copies(content().rules.item_max_copies);
            let mut lines = vec![(item.describe(held.saturating_add(1)), GREEN)];
            if held > 0 {
                lines.push((format!("You hold {held}: {}", item.describe(held)), GREY));
            }
            Face {
                icon: Icon::Texture(item_icon(item)),
                kind: Kind::Item,
                name: item.name().into(),
                sub: format!("Item  ·  copy {} of {max}", held + 1),
                badge: format!("{}/{max}", held + 1),
                bright: true,
                description: "Kept for the rest of the run.".into(),
                lines,
                footer: "Copies stack".into(),
            }
        }
        CardKind::Gold => Face {
            icon: Icon::Texture("INV_Misc_Coin_02"),
            kind: Kind::Fallback,
            name: "Purse of Gold".into(),
            sub: "Nothing left to learn".into(),
            badge: "GOLD".into(),
            bright: false,
            description: "Take gold instead.".into(),
            lines: vec![(format!("+{} gold", fallback_gold(level)), GOLD)],
            footer: "Before your gold gain".into(),
        },
        CardKind::Heal => Face {
            icon: Icon::Texture("INV_Potion_51"),
            kind: Kind::Fallback,
            name: "Healing Draught".into(),
            sub: "Nothing left to learn".into(),
            badge: "HEAL".into(),
            bright: false,
            description: "Mend your wounds instead.".into(),
            lines: vec![("Health back to full".into(), GREEN)],
            footer: String::new(),
        },
        CardKind::Reroll => Face {
            icon: Icon::Texture("INV_Scroll_03"),
            kind: Kind::Fallback,
            name: "Gambler's Coin".into(),
            sub: "Nothing left to learn".into(),
            badge: "REROLL".into(),
            bright: false,
            description: "Try your luck later instead.".into(),
            lines: vec![("+1 reroll".into(), GREEN)],
            footer: String::new(),
        },
        CardKind::Pact(pact) => Face {
            icon: Icon::Texture("INV_Misc_Bone_HumanSkull_01"),
            kind: Kind::Pact,
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
        CardKind::Augment(ability, augment) => augment_face(hero, ability, augment),
    }
}

/// The face of an augment card: what it does to the spell it is for.
fn augment_face(hero: &Hero, ability: usize, augment: Augment) -> Face {
    let content = content();
    let def = &content.abilities[ability];
    let rank = hero
        .abilities
        .iter()
        .find(|o| o.ability == ability)
        .map_or(1, |o| o.level);
    let capstone = augment.capstone_of().is_some();
    let tags: Vec<&str> = def.tags.iter().map(|t| t.name()).collect();
    Face {
        icon: Icon::Spell(def.visual),
        kind: if capstone {
            Kind::Capstone
        } else {
            Kind::Augment
        },
        name: augment.name().to_string(),
        sub: format!("{}  ·  rank {rank}", def.name),
        badge: if capstone { "CAPSTONE" } else { "AUGMENT" }.to_string(),
        bright: capstone,
        description: augment.description().to_string(),
        lines: vec![(tags.join("  ·  "), GREY)],
        footer: if capstone {
            "One of this spell's three capstones".to_string()
        } else {
            "Hold to wait for three others".to_string()
        },
    }
}

fn build_card(
    row: &mut ChildSpawnerCommands,
    fonts: &Fonts,
    hero: &Hero,
    level: u32,
    index: usize,
    card: &Card,
) {
    let color = rarity_color(card.rarity);
    let border = if card.rarity == Rarity::Common {
        Color::srgb(0.45, 0.45, 0.45)
    } else {
        color
    };
    let face = face(hero, level, card);

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
        BackgroundColor(face.kind.tint()),
        CardTint(face.kind.tint()),
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

        let mut icon = c.spawn((
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
        match face.icon {
            Icon::Spell(visual) => icon.insert(SpellIcon(visual)),
            Icon::Texture(name) => icon.insert(TextureIcon(name)),
        };
        // A spell card's icon shows the spell's tooltip, as it stands now, under the mouse.
        if let Some(ability) = card.ability() {
            icon.insert((super::tooltip::TipAbility(ability), Interaction::default()));
        }
        c.spawn((
            label(
                &fonts.text,
                format!("{} {}", card.rarity.name().to_uppercase(), face.kind.name()),
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
        let bare = face.description.is_empty();
        if !bare {
            c.spawn((
                label(&fonts.text, face.description, 9.5, Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
            ));
        }
        for (line, tint) in face.lines {
            c.spawn((
                label(&fonts.text, line, if bare { 12.0 } else { 9.5 }, tint),
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
    frozen: bool,
    banishes: u8,
    banishing: bool,
}

/// The screen's title and the line under it, for `offer`.
fn titles(offer: &Offer) -> (String, String) {
    let kind = match offer.source {
        Source::LevelUp => {
            return (
                "Level Up!".into(),
                "Choose a spell or a tome. Its rarity decides how strong it is.".into(),
            )
        }
        Source::Augment(ability) => {
            let name = &content().abilities[ability].name;
            let capstone = matches!(
                offer.cards.first().map(|c| c.kind),
                Some(CardKind::Augment(_, a)) if a.capstone_of().is_some()
            );
            return if capstone {
                (
                    format!("{name}: Capstone"),
                    format!("Choose {name}'s capstone. Its three capstones are always these."),
                )
            } else {
                (
                    format!("{name}: Augment"),
                    format!(
                        "Choose an augment, or Hold: {name} waits at its rank and offers three new ones after your next level-up."
                    ),
                )
            };
        }
        Source::Interactable(kind) => kind,
    };
    let hint = match kind {
        InteractableKind::TitanShrine | InteractableKind::GoldenShrine => {
            "The titans offer their knowledge. Choose a tome."
        }
        InteractableKind::CursedAltar => "Every boon has its bane. Choose wisely, or skip.",

        _ => "Choose your treasure.",
    };
    (kind.name().to_string(), hint.to_string())
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
        Query<&mut Text, With<SkipText>>,
        Query<&mut Text, With<FreezeText>>,
        Query<&mut Text, With<BanishText>>,
    )>,
    banishing: Res<BanishMode>,
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
        frozen: hero.frozen,
        banishes: hero.banishes,
        banishing: banishing.0,
    };
    if shown.as_ref() == Some(&now) {
        return;
    }
    let (title, hint) = titles(offer);
    let augmenting = matches!(offer.source, Source::Augment(_));
    let capstone = matches!(
        offer.cards.first().map(|c| c.kind),
        Some(CardKind::Augment(_, a)) if a.capstone_of().is_some()
    );
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
        t.0 = if capstone {
            "No reroll".to_string()
        } else {
            format!("Reroll {}   (R)", now.rerolls)
        };
    }
    if let Ok(mut t) = texts.p5().single_mut() {
        t.0 = if augmenting { "Hold" } else { "Skip" }.to_string();
    }
    let levelling = offer.source == Source::LevelUp;
    if let Ok(mut t) = texts.p6().single_mut() {
        t.0 = match (levelling, now.frozen) {
            (false, _) => "No freeze".to_string(),
            (true, false) => "Freeze   (T)".to_string(),
            (true, true) => "Frozen: unfreeze   (T)".to_string(),
        };
    }
    if let Ok(mut t) = texts.p7().single_mut() {
        t.0 = match (now.banishes, now.banishing) {
            (0, _) => "No banish".to_string(),
            (_, true) => "Banish: choose a new spell".to_string(),
            (n, false) => format!("Banish {n}   (G)"),
        };
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
                build_card(r, &fonts, hero, now.level, i, card);
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
                        super::tooltip::SpellSlot(i),
                        Interaction::default(),
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
    mut banishing: ResMut<BanishMode>,
    mut cards: Query<
        (&CardButton, &CardTint, &Interaction, &mut BackgroundColor),
        (Changed<Interaction>, Without<HandButton>),
    >,
    mut buttons: Query<(&HandButton, &Interaction, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (card, tint, interaction, mut bg) in &mut cards {
        match interaction {
            Interaction::Pressed => {
                if std::mem::take(&mut banishing.0) {
                    input.banish = Some(card.0);
                } else {
                    input.pick = Some(card.0);
                }
            }
            Interaction::Hovered => bg.0 = tint.0.lighter(0.06),
            Interaction::None => bg.0 = tint.0,
        }
    }
    for (button, interaction, mut bg) in &mut buttons {
        match interaction {
            Interaction::Pressed => match button {
                HandButton::Skip => input.skip = true,
                HandButton::Reroll => input.reroll = true,
                HandButton::Freeze => input.freeze = true,
                HandButton::Banish => banishing.0 = !banishing.0,
            },
            Interaction::Hovered => bg.0 = Color::srgb(0.3, 0.17, 0.08),
            Interaction::None => bg.0 = Color::srgb(0.18, 0.1, 0.05),
        }
    }
}
