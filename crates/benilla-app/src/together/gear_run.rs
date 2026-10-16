//! The gear bar in a run: the pieces the local hero brought, as a row of icons under its tomes
//! and items. A dormant piece is dimmed with its price in the corner; clicking it switches it
//! on, and Y switches on the best piece the purse pays for. A line beside the row says what Y
//! would buy. Gold is the same purse that pays for chests, so every coin is "a chest, or a piece
//! I know" (decision 9f).

use bevy::prelude::*;
use together_sim::{GearPiece, Hero, Slot};

use super::gear_look::{display, GearIcon};
use super::gear_tip::{GearTip, TipHint};
use super::inventory::InventoryRoot;
use super::{autoplay, LocalInput, Phase, Sim};

/// An icon's size, in the 768-unit canvas.
const ICON: f32 = 30.0;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const GREY: Color = Color::srgb(0.7, 0.7, 0.7);
const GREEN: Color = Color::srgb(0.3, 1.0, 0.3);

/// The bar's row, a child of the inventory's column.
#[derive(Component)]
pub(super) struct GearBar;

/// The icons' row inside it, rebuilt when the gear changes.
#[derive(Component)]
pub(super) struct GearBarIcons;

/// The line saying what Y buys.
#[derive(Component)]
pub(super) struct GearBarHint;

/// One icon: the slot its piece is in.
#[derive(Component, Clone, Copy)]
pub(super) struct GearBarSlot(Slot);

#[derive(Resource)]
pub(super) struct GearBarFont(Handle<Font>);

fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The bar, put into the inventory's column under its strip of tomes and items.
pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    roots: Query<Entity, With<InventoryRoot>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Ok(root) = roots.single() else {
        return;
    };
    *done = true;
    let font = crate::char_select::wow_font(&asset_server);
    let bar = commands
        .spawn((
            GearBar,
            Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|row| {
            row.spawn((
                GearBarIcons,
                Node {
                    column_gap: Val::Px(3.0),
                    ..default()
                },
            ));
            row.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(GREEN),
                TextShadow::default(),
                GearBarHint,
            ));
        })
        .id();
    commands.entity(root).insert_children(1, &[bar]);
    commands.insert_resource(GearBarFont(font));
}

/// What Y would switch on, and for how much.
fn next_buy(hero: &Hero) -> Option<(Slot, GearPiece, u32)> {
    let slot = hero.gear.next_affordable(hero.gold)?;
    let piece = hero.gear.in_slot(slot)?.piece;
    Some((slot, piece, hero.gear.price(slot)?))
}

/// The bar kept to the hero's gear, a click or Y switching a piece on, and autoplay buying as
/// soon as it can.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    keys: Res<ButtonInput<KeyCode>>,
    font: Option<Res<GearBarFont>>,
    mut input: ResMut<LocalInput>,
    clicks: Query<(&Interaction, &GearBarSlot), Changed<Interaction>>,
    mut bar: Query<&mut Visibility, With<GearBar>>,
    icons: Query<Entity, With<GearBarIcons>>,
    mut hint: Query<&mut Text, With<GearBarHint>>,
    mut shown: Local<Option<Vec<(GearPiece, bool, bool)>>>,
) {
    let hero = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .and_then(|sim| sim.world.hero(sim.me));
    let has_gear = hero.is_some_and(|h| !h.gear.worn.is_empty());
    let want = if has_gear {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut bar {
        if *v != want {
            *v = want;
        }
    }
    let Some(hero) = hero.filter(|_| has_gear) else {
        *shown = None;
        return;
    };

    // A click on a dormant piece, Y for the best one the purse pays for; autoplay buys whatever
    // it can. The world checks the price again.
    let clicked = clicks
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, s)| s.0)
        .filter(|&s| hero.gear.price(s).is_some());
    let buy = clicked.or_else(|| {
        (keys.just_pressed(KeyCode::KeyY) || autoplay())
            .then(|| next_buy(hero).map(|(s, _, _)| s))
            .flatten()
    });
    if let Some(slot) = buy {
        if input.gear_on.is_none() {
            input.gear_on = Some(slot.index() as u8);
        }
    }

    let text = match next_buy(hero) {
        Some((_, piece, price)) => format!("Y: switch on {} ({price} gold)", piece.name()),
        None => {
            let n = hero.gear.switched_on();
            let cheapest = Slot::ALL.iter().filter_map(|&s| hero.gear.price(s)).min();
            match cheapest {
                Some(p) => format!("Gear {n}/{}: next for {p} gold", hero.gear.worn.len()),
                None => format!("Gear {n}/{}: all switched on", hero.gear.worn.len()),
            }
        }
    };
    if let Ok(mut t) = hint.single_mut() {
        if t.0 != text {
            t.0 = text;
        }
    }

    // The icons, rebuilt when a piece goes on or the purse comes to pay for one.
    let key: Vec<(GearPiece, bool, bool)> = hero
        .gear
        .worn
        .iter()
        .map(|w| {
            let affordable = hero
                .gear
                .price(w.piece.slot)
                .is_some_and(|p| hero.gold >= p);
            (w.piece, w.on, affordable)
        })
        .collect();
    if shown.as_ref() == Some(&key) {
        return;
    }
    let (Ok(row), Some(font)) = (icons.single(), font) else {
        return;
    };
    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|row| {
        for w in &hero.gear.worn {
            let p = w.piece;
            let border = rgb(p.rarity.color());
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
                BorderColor::all(border),
                GearIcon(display(&p)),
                GearTip {
                    piece: p,
                    hint: TipHint::Run,
                },
                GearBarSlot(p.slot),
                Button,
            ))
            .with_children(|i| {
                if w.on {
                    return;
                }
                // Dormant: dimmed, its price in the corner.
                i.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    bevy::ui::FocusPolicy::Pass,
                ));
                let price = hero.gear.price(p.slot).unwrap_or(0);
                i.spawn((
                    Text::new(price.to_string()),
                    TextFont {
                        font: font.0.clone(),
                        font_size: 9.0,
                        ..default()
                    },
                    TextColor(if hero.gold >= price { GOLD } else { GREY }),
                    TextShadow::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(1.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    bevy::ui::FocusPolicy::Pass,
                ));
            });
        }
    });
    *shown = Some(key);
}
