//! The small parts of Nightfall's own panels (spec §8.4: our own glue panels first): an item's
//! icon in a slot with its count, a text button, a label, WoW's quality colours, and a line that
//! says what an item is.

use bevy::prelude::*;
use nightfall_sim::ItemDef;

/// A slot's side, in the UI's units.
pub(super) const SLOT: f32 = 34.0;
pub(super) const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
pub(super) const GREY: Color = Color::srgb(0.7, 0.7, 0.7);
pub(super) const RED: Color = Color::srgb(1.0, 0.35, 0.3);
pub(super) const PANEL: Color = Color::srgba(0.03, 0.03, 0.05, 0.92);
pub(super) const EDGE: Color = Color::srgb(0.55, 0.47, 0.3);

/// An icon to load into this image: `Interface\Icons\<name>.blp`.
#[derive(Component)]
pub(super) struct ItemIcon(pub String);

/// An [`ItemIcon`] already loaded (or tried).
#[derive(Component)]
pub(super) struct IconDone;

/// Load every new [`ItemIcon`] once the asset reader is up.
pub(super) fn fill_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &ItemIcon), Without<IconDone>>,
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
        commands.entity(e).insert(IconDone);
    }
}

/// WoW's colour for an item of `quality`.
pub(super) fn quality_color(quality: u8) -> Color {
    match quality {
        0 => Color::srgb(0.62, 0.62, 0.62),
        1 => Color::WHITE,
        2 => Color::srgb(0.12, 1.0, 0.0),
        3 => Color::srgb(0.0, 0.44, 0.87),
        _ => Color::srgb(0.64, 0.21, 0.93),
    }
}

/// A line of text in WoW's font.
pub(super) fn text(
    font: &Handle<Font>,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow::default(),
    )
}

/// A slot carrying `tag`: an empty square, or `item`'s icon edged in its quality's colour, its
/// count in the corner above 1.
pub(super) fn slot<T: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    tag: T,
    item: Option<(&ItemDef, u16)>,
) {
    let edge = item.map_or(Color::srgb(0.3, 0.3, 0.3), |(d, _)| {
        quality_color(d.quality)
    });
    let mut cell = parent.spawn((
        Button,
        tag,
        Interaction::default(),
        Node {
            width: Val::Px(SLOT),
            height: Val::Px(SLOT),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(edge),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
    ));
    if let Some((def, count)) = item {
        cell.with_children(|s| {
            s.spawn((
                ImageNode {
                    color: Color::srgba(0.2, 0.2, 0.2, 1.0),
                    ..default()
                },
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ItemIcon(def.icon.clone()),
            ));
            if count > 1 {
                s.spawn((
                    text(font, count.to_string(), 10.0, Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(2.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                ));
            }
        });
    }
}

/// A plain text button carrying `tag`.
pub(super) fn text_button<T: Component>(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    tag: T,
    caption: &str,
) {
    parent
        .spawn((
            Button,
            tag,
            Interaction::default(),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(EDGE),
            BackgroundColor(Color::srgba(0.12, 0.08, 0.03, 0.9)),
        ))
        .with_children(|b| {
            b.spawn(text(font, caption, 12.0, GOLD));
        });
}

/// What `count` of `def` is, in one line: name, kind, weight, value and what it adds.
pub(super) fn describe(def: &ItemDef, count: u16) -> String {
    let mut parts = vec![if count > 1 {
        format!("{} ×{count}", def.name)
    } else {
        def.name.clone()
    }];
    parts.push(format!("{:?}", def.kind));
    parts.push(format!("weight {:.1}", def.weight * f32::from(count)));
    if def.value > 0 {
        parts.push(format!("{} gold", def.value * u32::from(count)));
    }
    if def.bonus.health > 0.0 {
        parts.push(format!("+{:.0} health", def.bonus.health));
    }
    if def.bonus.damage > 0.0 {
        parts.push(format!("+{:.0}% damage", def.bonus.damage * 100.0));
    }
    if def.bag_slots > 0 {
        parts.push(format!("{} slots", def.bag_slots));
    }
    if !def.text.is_empty() {
        parts.push(def.text.clone());
    }
    parts.join("  ·  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{Bonus, ItemKind};

    #[test]
    fn an_item_says_what_it_is() {
        let blade = ItemDef {
            key: "blade".into(),
            name: "Night Watch Blade".into(),
            kind: ItemKind::Weapon,
            icon: "INV_Sword_05".into(),
            quality: 2,
            weight: 3.0,
            stack: 1,
            value: 40,
            slot: Some(nightfall_sim::EquipSlot::MainHand),
            look: None,
            bonus: Bonus {
                health: 0.0,
                damage: 0.1,
            },
            bag_slots: 0,
            restore: 1.0,
            on_use: None,
            light: None,
            text: String::new(),
        };
        assert_eq!(
            describe(&blade, 1),
            "Night Watch Blade  ·  Weapon  ·  weight 3.0  ·  40 gold  ·  +10% damage"
        );
        assert!(describe(&blade, 2).starts_with("Night Watch Blade ×2  ·  Weapon  ·  weight 6.0"));
    }
}
