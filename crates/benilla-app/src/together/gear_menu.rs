//! The menu's Equipment screen: what the picked class wears into its next run, and the stash it
//! is chosen from.
//!
//! It opens from the Equipment button in the menu's bottom-left corner (`TOGETHER_GEAR_SCREEN=1`
//! opens it for a test shot) as a panel over the zone list, leaving the hero in view, who wears
//! every piece as it is chosen. The worn pieces sit in two columns and a weapon row, as on WoW's
//! character sheet; under them, what they add up to once all are switched on, and the stash, a
//! page of icons at a time, sorted by slot and quality. A click on a stash piece wears it (what
//! it replaces goes back into the stash); a click on a worn piece takes it off. Every icon has
//! its tooltip ([`super::gear_tip`]). A button switches between gear per class and gear shared
//! by every class (decision 9h).

use bevy::prelude::*;
use together_sim::{describe_line, GearPiece, Slot, Stat};

use super::armory::GearStore;
use super::content;
use super::gear_look::{display, GearIcon};
use super::gear_tip::{GearTip, TipHint};
use super::menu::{framed, MenuBuilt, MenuRoot, Selection};
use crate::glue::art::{GlueArt, DIM, GOLD};
use crate::glue::widgets::{glue_button, outlined_text, GlueBtnKind, GlueText};
use crate::glue::GlueClicks;

/// The panel's width, and its stash grid's icons a row and rows a page.
const W: f32 = 520.0;
const PER_ROW: usize = 10;
const ROWS: usize = 4;
const STASH_ICON: f32 = 40.0;
const WORN_ICON: f32 = 34.0;

const GREEN: Color = Color::srgb(0.3, 1.0, 0.3);
const GREY: Color = Color::srgb(0.55, 0.55, 0.55);

/// Whether the panel is open, and the stash page shown.
#[derive(Resource)]
pub(super) struct GearScreen {
    pub open: bool,
    page: usize,
}

impl Default for GearScreen {
    fn default() -> Self {
        Self {
            open: std::env::var_os("TOGETHER_GEAR_SCREEN").is_some(),
            page: 0,
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum GearAction {
    /// The menu's Equipment button, and the panel's Close.
    Toggle,
    /// Per class, or shared by every class.
    Shared,
    PrevPage,
    NextPage,
    /// Wear stash piece `n` (its index in the locker's stash).
    Wear(usize),
    /// Take off the piece in a slot.
    TakeOff(Slot),
}

#[derive(Component)]
pub(super) struct GearBuilt;

#[derive(Component)]
pub(super) struct GearPanel;

/// The parts rebuilt when the stash or the pick changes.
#[derive(Component)]
pub(super) struct WornList;

#[derive(Component)]
pub(super) struct StashGrid;

#[derive(Component)]
pub(super) struct GearTitle;

#[derive(Component)]
pub(super) struct ModeText;

#[derive(Component)]
pub(super) struct SummaryText;

#[derive(Component)]
pub(super) struct PageText;

/// The "n new" note beside the Equipment button.
#[derive(Component)]
pub(super) struct NewNote;

#[derive(Resource)]
pub(super) struct GearFont(Handle<Font>);

fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

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

/// The button and the panel, added to the menu once it is built.
pub(super) fn build(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, With<MenuBuilt>, Without<GearBuilt>)>,
    art: Res<GlueArt>,
    asset_server: Res<AssetServer>,
) {
    let Ok(root) = roots.single() else {
        return;
    };
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;
    let px = Val::Px;
    commands.entity(root).insert(GearBuilt).with_children(|ui| {
        // The Equipment button, bottom left under the race and class tower, above Talents.
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            left: px(56.0),
            bottom: px(76.0),
            align_items: AlignItems::Center,
            column_gap: px(8.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                f,
                GearAction::Toggle,
                "Equipment",
                170.0,
                38.0,
                GlueBtnKind::Small,
                1.0,
            );
            c.spawn((label(f, 12.0, GREEN), NewNote));
        });

        // The panel, over the zone list.
        ui.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(5.0),
                top: px(15.0),
                ..default()
            },
            GearPanel,
            // Solid under the frame's own fill, so the zone list does not show through.
            BackgroundColor(Color::srgba(0.02, 0.02, 0.02, 0.97)),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Block,
            Interaction::default(),
        ))
        .with_children(|corner| {
            framed(
                corner,
                &art,
                Node {
                    width: px(W),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::new(px(16.0), px(16.0), px(12.0), px(14.0)),
                    row_gap: px(7.0),
                    ..default()
                },
                |panel| {
                    panel.spawn((label(f, 16.0, GOLD), GearTitle));
                    panel
                        .spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(10.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                label(f, 11.0, Color::WHITE),
                                ModeText,
                                Node {
                                    width: px(W - 32.0 - 180.0),
                                    ..default()
                                },
                            ));
                            glue_button(
                                row,
                                &art,
                                f,
                                GearAction::Shared,
                                "Per class / Shared",
                                170.0,
                                28.0,
                                GlueBtnKind::Small,
                                1.0,
                            );
                        });
                    panel.spawn((
                        WornList,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(4.0),
                            ..default()
                        },
                    ));
                    panel.spawn((
                        label(f, 11.0, GREEN),
                        SummaryText,
                        Node {
                            width: px(W - 32.0),
                            ..default()
                        },
                    ));
                    panel
                        .spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                label(f, 12.0, GOLD),
                                PageText,
                                Node {
                                    width: px(W - 32.0 - 2.0 * 58.0 - 16.0),
                                    ..default()
                                },
                            ));
                            for (action, caption) in
                                [(GearAction::PrevPage, "<"), (GearAction::NextPage, ">")]
                            {
                                glue_button(
                                    row,
                                    &art,
                                    f,
                                    action,
                                    caption,
                                    58.0,
                                    26.0,
                                    GlueBtnKind::Small,
                                    1.0,
                                );
                            }
                        });
                    panel.spawn((
                        StashGrid,
                        Node {
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: px(4.0),
                            row_gap: px(4.0),
                            width: px(PER_ROW as f32 * (STASH_ICON + 4.0)),
                            min_height: px(ROWS as f32 * (STASH_ICON + 4.0)),
                            align_content: AlignContent::FlexStart,
                            ..default()
                        },
                    ));
                    outlined_text(
                        panel,
                        Node {
                            width: px(W - 32.0),
                            ..default()
                        },
                        (),
                        (),
                        GlueText {
                            text: "Click a stash piece to wear it, a worn piece to take it off. \
                                   Worn pieces start every run dormant: spend gold in the run \
                                   to switch them on (Y, or click its icon).",
                            size: 11.0,
                            color: DIM,
                            wrap: true,
                        },
                        f,
                        1.0,
                    );
                    panel
                        .spawn(Node {
                            justify_content: JustifyContent::Center,
                            ..default()
                        })
                        .with_children(|row| {
                            glue_button(
                                row,
                                &art,
                                f,
                                GearAction::Toggle,
                                "Close",
                                120.0,
                                30.0,
                                GlueBtnKind::Small,
                                1.0,
                            );
                        });
                },
            );
        });
    });
    commands.insert_resource(GearFont(font));
}

/// The stash's pieces in the order shown: by slot, then the rarest and highest first, each with
/// its index in the locker.
fn sorted_stash(stash: &[GearPiece]) -> Vec<(usize, GearPiece)> {
    let mut v: Vec<(usize, GearPiece)> = stash.iter().copied().enumerate().collect();
    v.sort_by(|(_, a), (_, b)| {
        a.slot
            .cmp(&b.slot)
            .then(b.rarity.cmp(&a.rarity))
            .then(b.item_level.cmp(&a.item_level))
    });
    v
}

/// `q`'s one text set to `s`, if it differs.
fn set_text<F: bevy::ecs::query::QueryFilter>(q: &mut Query<&mut Text, F>, s: String) {
    if let Ok(mut t) = q.single_mut() {
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// What a click on the panel or its button does.
pub(super) fn clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &GearAction)>,
    selection: Res<Selection>,
    mut screen: ResMut<GearScreen>,
    mut store: ResMut<GearStore>,
) {
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    let class = content().classes[selection.class].key.clone();
    match action {
        GearAction::Toggle => {
            screen.open = !screen.open;
            if !screen.open {
                // Closing the screen has shown the new pieces.
                if store.armory.locker(&class).unseen > 0 {
                    store.armory.locker_mut(&class).unseen = 0;
                    store.changed();
                }
            }
        }
        GearAction::Shared => {
            store.armory.shared = !store.armory.shared;
            screen.page = 0;
            store.changed();
        }
        GearAction::PrevPage => screen.page = screen.page.saturating_sub(1),
        GearAction::NextPage => {
            let n = store.armory.locker(&class).stash.len();
            let pages = n.div_ceil(PER_ROW * ROWS).max(1);
            screen.page = (screen.page + 1).min(pages - 1);
        }
        GearAction::Wear(i) => {
            if store.armory.locker_mut(&class).equip(i) {
                store.changed();
            }
        }
        GearAction::TakeOff(slot) => {
            if store.armory.locker_mut(&class).unequip(slot) {
                store.changed();
            }
        }
    }
}

/// A gear icon: its look, its rarity's border, its tooltip and what a click does.
fn icon(
    parent: &mut ChildSpawnerCommands,
    piece: Option<GearPiece>,
    size: f32,
    action: Option<GearAction>,
    hint: TipHint,
    new: Option<&Handle<Font>>,
) {
    let border = piece.map_or(Color::srgb(0.3, 0.3, 0.3), |p| rgb(p.rarity.color()));
    let mut e = parent.spawn((
        ImageNode {
            color: if piece.is_some() {
                Color::srgba(0.2, 0.2, 0.2, 1.0)
            } else {
                Color::srgba(0.05, 0.05, 0.05, 0.9)
            },
            ..default()
        },
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(border),
    ));
    if let Some(p) = piece {
        e.insert((GearIcon(display(&p)), GearTip { piece: p, hint }, Button));
    }
    if let Some(a) = action {
        e.insert(a);
    }
    if let Some(font) = new {
        e.with_children(|i| {
            i.spawn((
                Text::new("New"),
                TextFont {
                    font: font.clone(),
                    font_size: 9.0,
                    ..default()
                },
                TextColor(GREEN),
                TextShadow::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(1.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                bevy::ui::FocusPolicy::Pass,
            ));
        });
    }
}

/// The panel shown while open, and its worn list, stash page and texts kept to the armory and
/// the picked class.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    screen: Res<GearScreen>,
    store: Res<GearStore>,
    selection: Res<Selection>,
    font: Option<Res<GearFont>>,
    mut panel: Query<&mut Visibility, With<GearPanel>>,
    worn: Query<Entity, With<WornList>>,
    grid: Query<Entity, With<StashGrid>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<GearTitle>>,
        Query<&mut Text, With<ModeText>>,
        Query<&mut Text, With<SummaryText>>,
        Query<&mut Text, With<PageText>>,
        Query<&mut Text, With<NewNote>>,
    )>,
    mut shown: Local<Option<(u32, usize, usize, bool)>>,
) {
    let want = if screen.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut panel {
        if *v != want {
            *v = want;
        }
    }
    let Some(font) = font else {
        return;
    };
    let content = content();
    let class = &content.classes[selection.class];
    let locker = store.armory.locker(&class.key);
    let note = if locker.unseen > 0 {
        format!("{} new", locker.unseen)
    } else {
        String::new()
    };
    set_text(&mut texts.p4(), note);
    let key = (store.version, selection.class, screen.page, screen.open);
    if *shown == Some(key) || !screen.open {
        return;
    }
    *shown = Some(key);

    set_text(
        &mut texts.p0(),
        format!(
            "Equipment: {}",
            if store.armory.shared {
                "every class".to_string()
            } else {
                class.name.clone()
            }
        ),
    );
    let mode = if store.armory.shared {
        "Shared: every class wears this set and finds into this stash."
    } else {
        "Per class: each class wears its own set and keeps its own finds."
    };
    set_text(&mut texts.p1(), mode.to_string());

    // What the worn set adds up to, all switched on.
    let mut totals: Vec<(Stat, f32)> = Vec::new();
    for p in &locker.equipped {
        for (stat, v) in p.stats() {
            match totals.iter_mut().find(|(s, _)| *s == stat) {
                Some((_, t)) => *t += v,
                None => totals.push((stat, v)),
            }
        }
    }
    let summary = if totals.is_empty() {
        "Nothing worn: bosses and elites drop equipment in a run.".to_string()
    } else {
        let lines: Vec<String> = totals.iter().map(|&(s, v)| describe_line(s, v)).collect();
        let uniques: Vec<&str> = locker
            .equipped
            .iter()
            .filter_map(|p| p.unique.map(|u| u.name().split(',').next().unwrap_or("")))
            .collect();
        let mut s = format!("All switched on: {}", lines.join(", "));
        if !uniques.is_empty() {
            s += &format!(". Uniques: {}", uniques.join(", "));
        }
        s
    };
    set_text(&mut texts.p2(), summary);

    let sorted = sorted_stash(&locker.stash);
    let per_page = PER_ROW * ROWS;
    let pages = sorted.len().div_ceil(per_page).max(1);
    let page = screen.page.min(pages - 1);
    set_text(
        &mut texts.p3(),
        format!("Stash: {} pieces   Page {}/{pages}", sorted.len(), page + 1),
    );

    if let Ok(list) = worn.single() {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|list| {
            let columns: [&[Slot]; 2] = [
                &[
                    Slot::Head,
                    Slot::Shoulders,
                    Slot::Back,
                    Slot::Chest,
                    Slot::Wrists,
                ],
                &[Slot::Hands, Slot::Waist, Slot::Legs, Slot::Feet],
            ];
            let entry = |row: &mut ChildSpawnerCommands, slot: Slot| {
                let piece = locker.in_slot(slot).copied();
                // The off hand under a two-handed weapon shows it is taken.
                let blocked =
                    slot == Slot::OffHand && locker.equipped.iter().any(GearPiece::two_hand);
                row.spawn(Node {
                    width: Val::Px((W - 40.0) / 2.0),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|e| {
                    icon(
                        e,
                        piece,
                        WORN_ICON,
                        piece.map(|_| GearAction::TakeOff(slot)),
                        TipHint::TakeOff,
                        None,
                    );
                    let (text, color) = match piece {
                        Some(p) => (p.name(), rgb(p.rarity.color())),
                        None if blocked => ("(two-handed weapon)".to_string(), GREY),
                        None => (slot.name().to_string(), GREY),
                    };
                    e.spawn((
                        Text::new(text),
                        TextFont {
                            font: font.0.clone(),
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(color),
                        TextShadow::default(),
                        Node {
                            width: Val::Px((W - 40.0) / 2.0 - WORN_ICON - 8.0),
                            ..default()
                        },
                    ));
                });
            };
            list.spawn(Node {
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|cols| {
                for column in columns {
                    cols.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|c| {
                        for &slot in column {
                            entry(c, slot);
                        }
                    });
                }
            });
            list.spawn(Node {
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                entry(row, Slot::MainHand);
                entry(row, Slot::OffHand);
            });
        });
    }

    if let Ok(grid) = grid.single() {
        commands.entity(grid).despawn_related::<Children>();
        commands.entity(grid).with_children(|g| {
            for &(i, p) in sorted.iter().skip(page * per_page).take(per_page) {
                let replaces = locker.in_slot(p.slot).copied().or_else(|| {
                    // A two-hander replaces the off hand too; an off hand, a two-hander.
                    locker
                        .equipped
                        .iter()
                        .find(|q| q.two_hand() && p.slot == Slot::OffHand)
                        .copied()
                });
                icon(
                    g,
                    Some(p),
                    STASH_ICON,
                    Some(GearAction::Wear(i)),
                    TipHint::Wear(replaces),
                    locker.is_new(i).then_some(&font.0),
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use together_sim::{Armor, Rarity, Unique};

    #[test]
    fn the_stash_sorts_by_slot_then_rarest_first_keeping_indices() {
        let hood = GearPiece::new(Slot::Head, Rarity::Common, 10, Armor::Cloth, 1);
        let epic_hood = GearPiece::new(Slot::Head, Rarity::Epic, 10, Armor::Cloth, 2);
        let staff = GearPiece::unique(Unique::Atiesh, 30, 3);
        let sorted = sorted_stash(&[staff, hood, epic_hood]);
        let order: Vec<usize> = sorted.iter().map(|(i, _)| *i).collect();
        assert_eq!(order, vec![2, 1, 0]);
    }
}
