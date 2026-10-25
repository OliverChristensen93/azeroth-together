//! The HUD: health and mana over a bar of the class's slots (icon, key, cooldown), a cast bar
//! while casting, and a word when the hero falls. Plain Bevy UI: FrameXML is off in this game.

use std::collections::HashMap;

use arpg_sim::Action;
use bevy::prelude::*;

use super::{Sim, ME};

const SLOT: f32 = 52.0;
const GAP: f32 = 6.0;
const BAR_W: f32 = 8.0 * SLOT + 7.0 * GAP;

#[derive(Component)]
pub(super) struct HealthFill;
#[derive(Component)]
pub(super) struct ManaFill;
#[derive(Component)]
pub(super) struct HealthText;
#[derive(Component)]
pub(super) struct CastBar;
#[derive(Component)]
pub(super) struct CastFill;
#[derive(Component)]
pub(super) struct Banner;

/// A slot's icon, to be filled with its spell's icon once the spell data is in.
#[derive(Component)]
pub(super) struct SlotIcon {
    slot: usize,
    done: bool,
}

/// The dark cover over a slot, as tall as the share of its cooldown left.
#[derive(Component)]
pub(super) struct SlotCooldown(usize);

/// The seconds left on a slot's cooldown.
#[derive(Component)]
pub(super) struct SlotTime(usize);

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let text = |size: f32| TextFont {
        font: font.clone(),
        font_size: size,
        ..default()
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(400),
        ))
        .with_children(|root| {
            // Bottom centre: the cast bar, health, mana, then the slots.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                bottom: Val::Px(14.0),
                margin: UiRect::left(Val::Px(-BAR_W / 2.0)),
                width: Val::Px(BAR_W),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    CastBar,
                    Node {
                        width: Val::Px(BAR_W * 0.5),
                        height: Val::Px(10.0),
                        align_self: AlignSelf::Center,
                        margin: UiRect::bottom(Val::Px(18.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    Visibility::Hidden,
                ))
                .with_children(|b| {
                    b.spawn((
                        CastFill,
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.75, 0.1)),
                    ));
                });
                let bar = |col: &mut ChildSpawnerCommands, color: Color, fill: bool| {
                    col.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(14.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    ))
                    .with_children(|b| {
                        let mut f = b.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(color),
                        ));
                        if fill {
                            f.insert(HealthFill);
                        } else {
                            f.insert(ManaFill);
                        }
                    });
                };
                bar(col, Color::srgb(0.1, 0.75, 0.15), true);
                bar(col, Color::srgb(0.15, 0.35, 0.95), false);
                col.spawn((
                    HealthText,
                    Text::new(""),
                    text(13.0),
                    TextColor(Color::WHITE),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(30.0),
                        left: Val::Px(6.0),
                        ..default()
                    },
                ));
                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(GAP),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|row| {
                    for slot in 0..8 {
                        row.spawn((
                            Node {
                                width: Val::Px(SLOT),
                                height: Val::Px(SLOT),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.8, 0.7, 0.4, 0.8)),
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        ))
                        .with_children(|s| {
                            s.spawn((
                                SlotIcon { slot, done: false },
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                ImageNode {
                                    color: Color::NONE,
                                    ..default()
                                },
                            ));
                            s.spawn((
                                SlotCooldown(slot),
                                Node {
                                    position_type: PositionType::Absolute,
                                    bottom: Val::Px(0.0),
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(0.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                            ));
                            s.spawn((
                                SlotTime(slot),
                                Text::new(""),
                                text(16.0),
                                TextColor(Color::WHITE),
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(15.0),
                                    left: Val::Px(14.0),
                                    ..default()
                                },
                            ));
                            s.spawn((
                                Text::new(super::input::label(slot)),
                                text(11.0),
                                TextColor(Color::srgb(1.0, 0.85, 0.3)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(1.0),
                                    right: Val::Px(3.0),
                                    ..default()
                                },
                            ));
                        });
                    }
                });
            });
            root.spawn((
                Banner,
                Text::new(""),
                text(30.0),
                TextColor(Color::srgb(1.0, 0.82, 0.0)),
                TextLayout::new_with_justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(38.0),
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
}

#[allow(clippy::type_complexity)]
pub(super) fn refresh(
    sim: Res<Sim>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<HealthFill>>,
        Query<&mut Node, With<ManaFill>>,
        Query<&mut Node, With<CastFill>>,
        Query<(&mut Node, &SlotCooldown)>,
    )>,
    mut cast_bar: Query<&mut Visibility, With<CastBar>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<HealthText>>,
        Query<(&mut Text, &SlotTime)>,
        Query<&mut Text, With<Banner>>,
    )>,
    mut icons: Query<(&mut ImageNode, &mut SlotIcon)>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u32, Handle<Image>>>,
) {
    let world = &sim.world;
    let Some(hero) = world.hero(ME) else {
        return;
    };
    let pct = |a: f32, b: f32| Val::Percent((a / b.max(1.0) * 100.0).clamp(0.0, 100.0));
    for mut n in &mut nodes.p0() {
        n.width = pct(hero.hp, hero.max_hp);
    }
    for mut n in &mut nodes.p1() {
        n.width = pct(hero.mana, hero.max_mana);
    }
    let slots = world.content().slots(hero.class);
    for (mut n, cd) in &mut nodes.p3() {
        let total = slots
            .get(cd.0)
            .map_or(1.0, |&a| world.content().abilities[a].cooldown.max(0.01));
        let left = hero.cooldowns.get(cd.0).copied().unwrap_or(0.0);
        n.height = Val::Percent((left / total * 100.0).clamp(0.0, 100.0));
    }
    let casting = match hero.action {
        Action::Casting { left, total, .. } => Some(1.0 - left / total.max(1e-3)),
        Action::Channeling { left, total, .. } => Some(left / total.max(1e-3)),
        Action::Idle => None,
    };
    if let Some(p) = casting {
        for mut n in &mut nodes.p2() {
            n.width = Val::Percent(p.clamp(0.0, 1.0) * 100.0);
        }
    }
    for mut v in &mut cast_bar {
        let want = if casting.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for mut t in &mut texts.p0() {
        let s = format!(
            "{:.0} / {:.0}    {:.0} / {:.0}",
            hero.hp.max(0.0),
            hero.max_hp,
            hero.mana,
            hero.max_mana
        );
        if t.0 != s {
            t.0 = s;
        }
    }
    for (mut t, st) in &mut texts.p1() {
        let left = hero.cooldowns.get(st.0).copied().unwrap_or(0.0);
        let s = if left >= 1.0 {
            format!("{left:.0}")
        } else if left > 0.0 {
            format!("{left:.1}")
        } else {
            String::new()
        };
        if t.0 != s {
            t.0 = s;
        }
    }
    for mut t in &mut texts.p2() {
        let s = if world.is_over() {
            format!(
                "You have fallen after {}:{:02}.\nPress Enter to begin again.",
                world.time() as u32 / 60,
                world.time() as u32 % 60
            )
        } else {
            String::new()
        };
        if t.0 != s {
            t.0 = s;
        }
    }

    // The icons, once the spell data and the archives can give them.
    let (Some(spells), Some(assets)) = (spells.as_deref(), assets.as_deref_mut()) else {
        return;
    };
    for (mut img, mut icon) in &mut icons {
        if icon.done {
            continue;
        }
        let Some(&ab) = slots.get(icon.slot) else {
            icon.done = true;
            continue;
        };
        let spell = world.content().abilities[ab].visual;
        let handle = match cache.get(&spell) {
            Some(h) => Some(h.clone()),
            None => {
                let path = spells
                    .catalog
                    .get(spell)
                    .and_then(|d| d.icon.clone())
                    .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".into());
                let h = assets.sprite_texture(&format!("{path}.blp"), &mut images);
                if let Some(h) = &h {
                    cache.insert(spell, h.clone());
                }
                h
            }
        };
        if let Some(h) = handle {
            img.image = h;
            img.color = Color::WHITE;
            icon.done = true;
        }
    }
}
