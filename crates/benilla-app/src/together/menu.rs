//! The main menu, laid out like the 1.12 character create and select glue screens and built from
//! their own widgets and art ([`crate::glue`]): the race, gender and class tower on the left, the
//! zone list in the character-select frame on the right, the class's spells in a panel at the
//! bottom, and Play Solo (with Host and Join Co-op beside it) under the hero, who stands in the
//! picked zone while the camera circles them.
//!
//! The screen lays out on the reference's 768-unit canvas through `UiScale`, which is set only
//! while the menu is up ([`apply_ui_scale`]).

use std::collections::HashMap;

use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use bevy::ui_render::ui_material::MaterialNode;
use bevy::window::PrimaryWindow;
use together_sim::Content;

use super::{content, Phase};
use super::{dress, units};
use crate::char_select::race_name;
use crate::glue::art::{class_tc, race_tc, tc_rect, GlueArt, DIM, GOLD, NAME_EDGE};
use crate::glue::backdrop::{backdrop_border, tiled_bg_node};
use crate::glue::widgets::{
    abs, glue_button, icon_button, outlined_text, GlueBtnKind, GlueDisabled, GlueText, Hilight,
    HoverLabel, LockHighlight,
};
use crate::glue::GlueClicks;

/// The create screen's two race columns.
const ALLIANCE: [u8; 4] = [1, 3, 4, 7];
const HORDE: [u8; 4] = [2, 5, 6, 8];

/// `CharacterSelectCharacterFrame`'s border and fill tints.
const FRAME_BORDER: Color = Color::srgb(0.8, 0.8, 0.8);
const FRAME_FILL: Color = Color::srgba(0.09, 0.09, 0.09, 0.85);

/// The tower ends under the class grid and Randomize.
const TOWER_H: f32 = 590.0;

/// The zone rows: the character-select row, with room under them for the zone's description.
const ROW_TOP: f32 = 44.0;
const ROW_PITCH: f32 = 40.0;
const ROW_H: f32 = 34.0;
const ROW_W: f32 = 228.0;

/// The class panel: centred at the bottom, above the hero's name.
const PANEL_W: f32 = 470.0;
const PANEL_H: f32 = 150.0;
const ICON: f32 = 30.0;

/// The pick: class, look (one per race and sex the class's trainers come in) and zone.
#[derive(Resource)]
pub(super) struct Selection {
    pub class: usize,
    /// `(display, race, sex)`, filled once the creature data is in.
    pub looks: Vec<(u32, u8, u8)>,
    pub look: Option<usize>,
    pub zone: usize,
    /// The zone the world is standing in: the last one played, or the saved pick at boot.
    /// Picking another zone changes only `zone`, so no loading screen rises until Play.
    pub stage: usize,
    /// Bumped on every change, so the screen refreshes once.
    pub version: u32,
}

impl Selection {
    pub(super) fn new(content: &Content) -> Self {
        let class = content.class(&super::class_key()).unwrap_or(0);
        let zone = content.zone(&super::zone_key()).unwrap_or(0);
        Self {
            class,
            looks: Vec::new(),
            look: None,
            zone,
            stage: zone,
            version: 0,
        }
    }

    pub(super) fn hero_look(&self) -> Option<(u32, u8, u8)> {
        self.look.and_then(|i| self.looks.get(i)).copied()
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuAction {
    Race(u8),
    Sex(u8),
    Class(usize),
    Zone(usize),
    Randomize,
    PlaySolo,
    HostCoop,
    JoinCoop,
    Exit,
}

/// The screen's root; [`build_menu`] fills it once the glue art is in.
#[derive(Component)]
pub(super) struct MenuRoot;

#[derive(Component)]
pub(super) struct MenuBuilt;

/// A race button's face, re-cropped to the picked gender.
#[derive(Component)]
pub(super) struct RaceFace(u8);

#[derive(Component)]
pub(super) struct PickName;

#[derive(Component)]
pub(super) struct ClassTitle;

#[derive(Component)]
pub(super) struct ClassText;

#[derive(Component)]
pub(super) struct SpellHover;

#[derive(Component)]
pub(super) struct ZoneText;

/// The class panel's row of spell icons, rebuilt when the class changes.
#[derive(Component)]
pub(super) struct IconRow;

/// One icon in [`IconRow`]: an ability of the picked class, by index into the content.
#[derive(Component)]
pub(super) struct SpellSlot(pub(super) usize);

/// An icon slot filled from the spell's `SpellIcon.dbc` icon once the client data is up.
#[derive(Component)]
pub(super) struct SpellIcon(pub(super) u32);

#[derive(Component)]
pub(super) struct IconDone;

/// The menu's hero, standing in the zone.
#[derive(Component)]
pub(super) struct PreviewHero;

/// `Gnome Mage` and the like.
fn hero_title(content: &Content, sel: &Selection) -> String {
    match sel.hero_look() {
        Some((_, race, _)) => format!("{} {}", race_name(race), content.classes[sel.class].name),
        None => content.classes[sel.class].name.clone(),
    }
}

pub(super) fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The saved pick, `benilla-config/together-pick.txt`: `class race sex zone` by key, restored at
/// boot so the menu opens on the last hero and zone.
fn pick_path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("together-pick.txt"))
}

fn save_pick(content: &Content, sel: &Selection) {
    let Some((_, race, sex)) = sel.hero_look() else {
        return;
    };
    if let Some(path) = pick_path() {
        let line = format!(
            "{} {race} {sex} {}",
            content.classes[sel.class].key, content.zones[sel.zone].key
        );
        let _ = crate::local_state::write_atomic(&path, &line);
    }
}

/// Every class's looks, `(display, race, sex)`: each race the class allows (`CharBaseInfo`, as the
/// create screen asks it), in both sexes, as that race's player body.
fn all_looks(
    content: &Content,
    char_create: &crate::entities::CharCreate,
) -> Vec<Vec<(u32, u8, u8)>> {
    content
        .classes
        .iter()
        .map(|c| {
            let mut out: Vec<(u32, u8, u8)> = Vec::new();
            for race in 1..=8u8 {
                if !char_create.0.allows(race, c.class_id) {
                    continue;
                }
                for sex in 0..=1u8 {
                    if let Some(display) = char_create.0.body_display(race, sex) {
                        out.push((display, race, sex));
                    }
                }
            }
            out
        })
        .collect()
}

fn find(looks: &[(u32, u8, u8)], race: Option<u8>, sex: Option<u8>) -> Option<usize> {
    looks
        .iter()
        .position(|&(_, r, s)| race.is_none_or(|x| x == r) && sex.is_none_or(|x| x == s))
}

fn set(sel: &mut Selection, all: &[Vec<(u32, u8, u8)>], class: usize, look: usize) {
    sel.class = class;
    sel.looks = all[class].clone();
    sel.look = Some(look);
    sel.version += 1;
}

/// Once the creature data can resolve looks: the saved pick, else the first look of the class.
pub(super) fn restore_pick(
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut selection: ResMut<Selection>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(char_create) = char_create.as_deref() else {
        return;
    };
    *done = true;
    let content = content();
    let all = all_looks(&content, char_create);
    // A test run's TOGETHER_CLASS beats the saved pick.
    let saved = pick_path()
        .filter(|_| std::env::var_os("TOGETHER_CLASS").is_none())
        .and_then(|p| crate::local_state::read_to_string(&p).ok())
        .and_then(|text| {
            let v: Vec<&str> = text.split_whitespace().collect();
            let [class, race, sex, zone] = v[..] else {
                return None;
            };
            Some((
                content.class(class)?,
                race.parse::<u8>().ok()?,
                sex.parse::<u8>().ok()?,
                content.zone(zone)?,
            ))
        });
    if let Some((class, race, sex, zone)) = saved {
        selection.zone = zone;
        selection.stage = zone;
        if let Some(l) = find(&all[class], Some(race), Some(sex)) {
            set(&mut selection, &all, class, l);
            return;
        }
    }
    let class = selection.class;
    if let Some(l) = find(&all[class], None, None) {
        set(&mut selection, &all, class, l);
    }
}

/// The menu's root, spawned at startup and filled once the glue art is in.
pub(super) fn spawn_root(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(410),
        bevy::ui::FocusPolicy::Pass,
    ));
}

/// Build the screen into [`MenuRoot`] once the archives are readable, so the glue art is there.
pub(super) fn build_menu(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, Without<MenuBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    asset_server: Res<AssetServer>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;
    let content = content();
    commands.entity(root).insert(MenuBuilt).with_children(|ui| {
        tower(ui, &art, f, &content);
        if let Some(logo) = &art.logo {
            ui.spawn((
                ImageNode::new(logo.clone()),
                abs(1.0, 3.0, 7.0, 256.0, 128.0),
            ));
        }
        zone_frame(ui, &art, f, &content);
        class_panel(ui, &art, f);
        lobby_panel(ui, &art, f);
        super::dev::dev_panel(ui, &art, f);
        super::dev::dev_button(ui, &art, f);

        // The hero's name line, GlueFontNormalHuge at BOTTOM (0,100).
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(100.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|c| {
            outlined_text(
                c,
                Node::default(),
                (),
                PickName,
                GlueText {
                    text: "",
                    size: 22.0,
                    color: GOLD,
                    wrap: false,
                },
                f,
                1.0,
            );
        });

        // Play Solo where Enter World stands, Host and Join Co-op either side of it.
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(30.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(16.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                f,
                MenuAction::HostCoop,
                "Host Co-op",
                150.0,
                38.0,
                GlueBtnKind::Small,
                1.0,
            );
            glue_button(
                c,
                &art,
                f,
                MenuAction::PlaySolo,
                "Play Solo",
                200.0,
                60.0,
                GlueBtnKind::Normal,
                1.0,
            );
            glue_button(
                c,
                &art,
                f,
                MenuAction::JoinCoop,
                "Join Co-op",
                150.0,
                38.0,
                GlueBtnKind::Small,
                1.0,
            );
        });
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|c| {
            outlined_text(
                c,
                Node::default(),
                (),
                (),
                GlueText {
                    text: "Host Co-op shares a run with up to three friends; Join Co-op joins one",
                    size: 11.0,
                    color: DIM,
                    wrap: false,
                },
                f,
                1.0,
            );
        });
        // `CharacterSelectExitButton`'s corner.
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                f,
                MenuAction::Exit,
                "Exit",
                120.0,
                32.0,
                GlueBtnKind::Small,
                1.0,
            );
        });
    });
}

/// A character-select style frame: the tooltip fill and the name border.
pub(super) fn framed(
    ui: &mut ChildSpawnerCommands,
    art: &GlueArt,
    node: Node,
    inner: impl FnOnce(&mut ChildSpawnerCommands),
) {
    ui.spawn(node).with_children(|frame| {
        if let (Some(bg), Some(border)) = (&art.tooltip_bg, &art.name_border) {
            frame.spawn((
                tiled_bg_node(bg.clone(), NAME_EDGE, 1.0, FRAME_FILL),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(5.0),
                    right: Val::Px(5.0),
                    top: Val::Px(4.0),
                    bottom: Val::Px(5.0),
                    ..default()
                },
            ));
            backdrop_border(frame, border, NAME_EDGE, FRAME_BORDER);
        } else {
            frame.spawn((BackgroundColor(FRAME_FILL), crate::glue::widgets::overlay()));
        }
        inner(frame);
    });
}

/// `CharacterCreateConfigurationFrame`, 206×600 at TOPLEFT (28,-74): the race columns under the
/// faction banners, the gender pair, the class grid and Randomize.
fn tower(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>, content: &Content) {
    let px = Val::Px;
    ui.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(28.0),
        top: px(74.0),
        width: px(206.0),
        height: px(TOWER_H),
        ..default()
    })
    .with_children(|tower| {
        if let Some(bg) = &art.tower_bg {
            tower.spawn((
                ImageNode::new(bg.clone()),
                abs(1.0, -3.0, 0.0, 218.0, TOWER_H + 6.0),
            ));
        }
        // The create tower's top piece, then its foot piece cut to end where the class grid does.
        if let Some((border, size)) = &art.tower_border {
            for (top, height, tc) in [
                (0.0, 236.0, [0.0, 0.875, 0.0, 0.9375]),
                (
                    236.0,
                    TOWER_H - 236.0 + 6.0,
                    [
                        0.0,
                        0.875,
                        1.0 - (TOWER_H - 236.0 + 6.0) / 256.0 * 0.8203125 / (210.0 / 256.0),
                        1.0,
                    ],
                ),
            ] {
                tower.spawn((
                    ImageNode {
                        image: border.clone(),
                        rect: Some(tc_rect(*size, tc)),
                        ..default()
                    },
                    abs(1.0, -9.0, top, 224.0, height),
                ));
            }
        }
        if let Some(banners) = &art.banners {
            tower.spawn((
                ImageNode::new(banners.clone()),
                abs(1.0, -27.0, 60.0, 256.0, 259.0),
            ));
        }
        for (label, center) in [("Alliance", 51.0), ("Horde", 151.0)] {
            outlined_text(
                tower,
                Node {
                    justify_content: JustifyContent::Center,
                    ..abs(1.0, center - 60.0, 40.0, 120.0, 17.0)
                },
                (),
                (),
                GlueText {
                    text: label,
                    size: 15.0,
                    color: GOLD,
                    wrap: false,
                },
                f,
                1.0,
            );
        }

        for (faction, left) in [(ALLIANCE, 33.0), (HORDE, 127.0)] {
            tower
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(68.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5.0),
                    ..default()
                })
                .with_children(|col| {
                    for race in faction {
                        let face = art.races.as_ref().and_then(|(sheet, size)| {
                            race_tc(race, 0).map(|tc| (sheet.clone(), tc_rect(*size, tc)))
                        });
                        icon_button(
                            col,
                            f,
                            MenuAction::Race(race),
                            Some(RaceFace(race)),
                            face,
                            None::<()>,
                            race_name(race),
                            art,
                            1.0,
                        );
                    }
                });
        }

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(53.0),
                top: px(303.0),
                flex_direction: FlexDirection::Row,
                column_gap: px(5.0),
                ..default()
            })
            .with_children(|g| {
                for (sex, label) in [(0u8, "Male"), (1u8, "Female")] {
                    let face = art.gender.as_ref().map(|(sheet, size)| {
                        let half = if sex == 0 {
                            [0.0, 0.5, 0.0, 1.0]
                        } else {
                            [0.5, 1.0, 0.0, 1.0]
                        };
                        (sheet.clone(), tc_rect(*size, half))
                    });
                    icon_button(
                        g,
                        f,
                        MenuAction::Sex(sex),
                        None::<()>,
                        face,
                        None::<()>,
                        label,
                        art,
                        1.0,
                    );
                }
            });

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(103.0 - (3.0 * 48.0 + 2.0 * 4.0) / 2.0),
                width: px(3.0 * 48.0 + 2.0 * 4.0),
                top: px(368.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: px(4.0),
                row_gap: px(4.0),
                ..default()
            })
            .with_children(|grid| {
                for (i, class) in content.classes.iter().enumerate() {
                    let face = art.classes.as_ref().and_then(|(sheet, size)| {
                        class_tc(class.class_id).map(|tc| (sheet.clone(), tc_rect(*size, tc)))
                    });
                    icon_button(
                        grid,
                        f,
                        MenuAction::Class(i),
                        None::<()>,
                        face,
                        None::<()>,
                        &class.name,
                        art,
                        1.0,
                    );
                }
            });

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(30.0),
                top: px(TOWER_H - 46.0),
                ..default()
            })
            .with_children(|r| {
                glue_button(
                    r,
                    art,
                    f,
                    MenuAction::Randomize,
                    "Randomize",
                    146.0,
                    30.0,
                    GlueBtnKind::Small,
                    1.0,
                );
            });
    });
}

/// The zone list in `CharacterSelectCharacterFrame` (260×642 at TOPRIGHT (-5,-15)): one row per
/// zone, lowest level first, and the picked zone's description under them.
fn zone_frame(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>, content: &Content) {
    let px = Val::Px;
    let mut order: Vec<(usize, u32, u32)> = content
        .zones
        .iter()
        .enumerate()
        .map(|(i, z)| {
            let levels = z.enemy_ix.iter().map(|&e| content.enemies[e].level);
            let lo = levels.clone().min().unwrap_or(1);
            let hi = levels.max().unwrap_or(lo);
            (i, lo, hi)
        })
        .collect();
    order.sort_by_key(|&(i, lo, _)| (lo, content.zones[i].name.clone()));
    let rows = order.len() as f32;

    framed(
        ui,
        art,
        Node {
            position_type: PositionType::Absolute,
            right: px(5.0),
            top: px(15.0),
            width: px(260.0),
            height: px(642.0),
            ..default()
        },
        |frame| {
            outlined_text(
                frame,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(12.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                (),
                (),
                GlueText {
                    text: "Choose a Zone",
                    size: 18.0,
                    color: DIM,
                    wrap: false,
                },
                f,
                1.0,
            );
            for (row, &(i, lo, hi)) in order.iter().enumerate() {
                let zone = &content.zones[i];
                frame
                    .spawn((
                        MenuAction::Zone(i),
                        Button,
                        LockHighlight::default(),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(24.0),
                            top: px(ROW_TOP + row as f32 * ROW_PITCH),
                            width: px(ROW_W),
                            height: px(ROW_H),
                            ..default()
                        },
                    ))
                    .with_children(|b| {
                        if let Some(hi) = &art.select_highlight {
                            b.spawn((
                                Hilight,
                                Visibility::Hidden,
                                MaterialNode(hi.clone()),
                                abs(1.0, -20.0, -6.0, ROW_W + 20.0, ROW_H + 12.0),
                            ));
                        }
                        outlined_text(
                            b,
                            abs(1.0, 0.0, 2.0, ROW_W, 16.0),
                            (),
                            (),
                            GlueText {
                                text: &zone.name,
                                size: 15.0,
                                color: GOLD,
                                wrap: false,
                            },
                            f,
                            1.0,
                        );
                        let levels = if lo == hi {
                            format!("Level {lo}")
                        } else {
                            format!("Level {lo}-{hi}")
                        };
                        outlined_text(
                            b,
                            abs(1.0, 0.0, 19.0, ROW_W, 14.0),
                            (),
                            (),
                            GlueText {
                                text: &format!("{levels}  {}", zone.continent),
                                size: 12.0,
                                color: Color::WHITE,
                                wrap: false,
                            },
                            f,
                            1.0,
                        );
                    });
            }
            outlined_text(
                frame,
                abs(1.0, 24.0, ROW_TOP + rows * ROW_PITCH + 16.0, ROW_W, 200.0),
                (),
                ZoneText,
                GlueText {
                    text: "",
                    size: 13.0,
                    color: Color::WHITE,
                    wrap: true,
                },
                f,
                1.0,
            );
        },
    );
}

/// The co-op panel, top centre between the tower and the zone list: what [`super::lobby`] writes,
/// and its buttons (only those that apply show).
fn lobby_panel(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>) {
    use super::lobby::{LobbyAction, LobbyPanel, LobbyText};
    let px = Val::Px;
    const W: f32 = 480.0;
    ui.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(16.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        LobbyPanel,
        Visibility::Hidden,
    ))
    .with_children(|centre| {
        framed(
            centre,
            art,
            Node {
                width: px(W),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(px(18.0), px(18.0), px(14.0), px(12.0)),
                row_gap: px(8.0),
                ..default()
            },
            |panel| {
                outlined_text(
                    panel,
                    Node {
                        width: px(W - 36.0),
                        ..default()
                    },
                    (),
                    LobbyText,
                    GlueText {
                        text: "",
                        size: 13.0,
                        color: Color::WHITE,
                        wrap: true,
                    },
                    f,
                    1.0,
                );
                panel
                    .spawn(Node {
                        column_gap: px(10.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        for (action, caption) in [
                            (LobbyAction::CopyCode, "Copy Invite Code"),
                            (LobbyAction::Paste, "Paste"),
                            (LobbyAction::StartRun, "Start Run"),
                            (LobbyAction::Connect, "Connect"),
                            (LobbyAction::Stop, "Leave"),
                            (LobbyAction::Cancel, "Cancel"),
                        ] {
                            glue_button(
                                row,
                                art,
                                f,
                                action,
                                caption,
                                140.0,
                                32.0,
                                GlueBtnKind::Small,
                                1.0,
                            );
                        }
                    });
            },
        );
    });
}

/// The class panel at the bottom centre: the class in its colour, what it is, and its spells.
fn class_panel(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>) {
    let px = Val::Px;
    ui.spawn(Node {
        position_type: PositionType::Absolute,
        bottom: px(132.0),
        width: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        ..default()
    })
    .with_children(|centre| {
        framed(
            centre,
            art,
            Node {
                width: px(PANEL_W),
                height: px(PANEL_H),
                ..default()
            },
            |panel| {
                outlined_text(
                    panel,
                    abs(1.0, 18.0, 12.0, PANEL_W - 36.0, 20.0),
                    (),
                    ClassTitle,
                    GlueText {
                        text: "",
                        size: 18.0,
                        color: GOLD,
                        wrap: false,
                    },
                    f,
                    1.0,
                );
                outlined_text(
                    panel,
                    abs(1.0, 18.0, 36.0, PANEL_W - 36.0, 48.0),
                    (),
                    ClassText,
                    GlueText {
                        text: "",
                        size: 12.0,
                        color: Color::WHITE,
                        wrap: true,
                    },
                    f,
                    1.0,
                );
                panel.spawn((
                    IconRow,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(18.0),
                        top: px(84.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(4.0),
                        ..default()
                    },
                ));
                outlined_text(
                    panel,
                    abs(1.0, 18.0, 120.0, PANEL_W - 36.0, 16.0),
                    (),
                    SpellHover,
                    GlueText {
                        text: "",
                        size: 12.0,
                        color: GOLD,
                        wrap: false,
                    },
                    f,
                    1.0,
                );
            },
        );
    });
}

/// What a click on the screen does.
#[allow(clippy::too_many_arguments)]
pub(super) fn menu_clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &MenuAction)>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut selection: ResMut<Selection>,
    mut start: ResMut<super::StartRequest>,
    mut net: ResMut<super::session::Net>,
    mut dialog: ResMut<super::lobby::JoinDialog>,
    mut exit: MessageWriter<AppExit>,
    mut seed: Local<u64>,
) {
    let Some(char_create) = char_create.as_deref() else {
        return;
    };
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    let content = content();
    let all = all_looks(&content, char_create);
    let cur = selection.hero_look().map(|(_, r, s)| (r, s));
    let (race, sex) = (cur.map(|c| c.0), cur.map_or(0, |c| c.1));
    match action {
        MenuAction::Zone(i) => {
            if selection.zone != i {
                selection.zone = i;
                selection.version += 1;
            }
        }
        MenuAction::PlaySolo => {
            if selection.hero_look().is_some() {
                start.0 = true;
            }
        }
        MenuAction::HostCoop => {
            if net.is_solo() {
                dialog.open = false;
                net.host();
            }
        }
        MenuAction::JoinCoop => {
            if net.is_solo() {
                net.status.clear();
                dialog.open = true;
            }
        }
        MenuAction::Exit => {
            exit.write(AppExit::Success);
        }
        MenuAction::Randomize => {
            let picks: Vec<(usize, usize)> = all
                .iter()
                .enumerate()
                .flat_map(|(c, looks)| (0..looks.len()).map(move |l| (c, l)))
                .collect();
            if !picks.is_empty() {
                *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(
                    web_time::SystemTime::now()
                        .duration_since(web_time::UNIX_EPOCH)
                        .map_or(1, |d| d.as_nanos() as u64),
                );
                let (c, l) = picks[(*seed >> 33) as usize % picks.len()];
                set(&mut selection, &all, c, l);
            }
        }
        // The class keeps the race and gender where it can.
        MenuAction::Class(c) => {
            let looks = &all[c];
            let l = find(looks, race, Some(sex))
                .or_else(|| find(looks, race, None))
                .or_else(|| find(looks, None, Some(sex)))
                .or_else(|| find(looks, None, None));
            if let Some(l) = l {
                set(&mut selection, &all, c, l);
            }
        }
        // The race keeps the class where it can, else takes the first class it has.
        MenuAction::Race(r) => {
            let classes = std::iter::once(selection.class).chain(0..all.len());
            for c in classes {
                let l = find(&all[c], Some(r), Some(sex)).or_else(|| find(&all[c], Some(r), None));
                if let Some(l) = l {
                    set(&mut selection, &all, c, l);
                    break;
                }
            }
        }
        MenuAction::Sex(s) => {
            if let Some(l) = find(&all[selection.class], race, Some(s)) {
                let class = selection.class;
                set(&mut selection, &all, class, l);
            }
        }
    }
    save_pick(&content, &selection);
}

/// The screen's per-frame state: highlights, hover labels, which races and classes the pick
/// allows, the gender on the race faces, the name line, the class panel and the zone text.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh_menu_ui(
    mut commands: Commands,
    selection: Res<Selection>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    art: Res<GlueArt>,
    mut buttons: Query<(
        &MenuAction,
        &Interaction,
        &Children,
        Option<&mut LockHighlight>,
        Option<&mut GlueDisabled>,
        &mut Node,
        &mut Visibility,
    )>,
    mut labels: Query<&mut Visibility, (With<HoverLabel>, Without<MenuAction>)>,
    mut faces: Query<(&RaceFace, &mut ImageNode)>,
    mut texts: ParamSet<(
        Query<(&mut Text, &mut TextColor), With<PickName>>,
        Query<(&mut Text, &mut TextColor), With<ClassTitle>>,
        Query<&mut Text, With<ClassText>>,
        Query<&mut Text, With<ZoneText>>,
        Query<&mut Text, With<SpellHover>>,
    )>,
    rows: Query<Entity, With<IconRow>>,
    mut cache: Local<(Option<u32>, bool, Vec<Vec<(u32, u8, u8)>>)>,
    mut shown_class: Local<Option<usize>>,
    net: Res<super::session::Net>,
    dialog: Res<super::lobby::JoinDialog>,
) {
    let content = content();
    // Hosting, joined or typing an address: the solo and co-op buttons wait.
    let co_op_busy = !net.is_solo() || dialog.open;
    let have = char_create.is_some();
    if cache.0 != Some(selection.version) || cache.1 != have {
        *cache = (
            Some(selection.version),
            have,
            char_create
                .as_deref()
                .map(|c| all_looks(&content, c))
                .unwrap_or_default(),
        );
    }
    let all = &cache.2;
    let cur = selection.hero_look().map(|(_, r, s)| (r, s));

    for (action, interaction, children, lock, disabled, mut node, mut vis) in &mut buttons {
        let selected = match *action {
            MenuAction::Race(r) => cur.is_some_and(|c| c.0 == r),
            MenuAction::Sex(s) => cur.is_some_and(|c| c.1 == s),
            MenuAction::Class(i) => selection.class == i,
            MenuAction::Zone(i) => selection.zone == i,
            _ => false,
        };
        if let Some(mut lock) = lock {
            if lock.0 != selected {
                lock.0 = selected;
            }
        }
        if let Some(mut disabled) = disabled {
            let off = match *action {
                MenuAction::PlaySolo => cur.is_none() || co_op_busy,
                MenuAction::HostCoop | MenuAction::JoinCoop => co_op_busy,
                _ => false,
            };
            if disabled.0 != off {
                disabled.0 = off;
            }
        }
        match *action {
            // As on the create screen, a class the race cannot be leaves the grid.
            MenuAction::Class(i) => {
                let ok = cur.is_none_or(|(r, _)| {
                    all.get(i).is_some_and(|l| find(l, Some(r), None).is_some())
                });
                let display = if ok { Display::Flex } else { Display::None };
                if node.display != display {
                    node.display = display;
                }
            }
            MenuAction::Race(r) => {
                let ok = all.is_empty() || all.iter().any(|l| find(l, Some(r), None).is_some());
                let want = if ok {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want {
                    *vis = want;
                }
            }
            _ => {}
        }
        let show = selected || *interaction != Interaction::None || art.races.is_none();
        for child in children {
            if let Ok(mut label) = labels.get_mut(*child) {
                let want = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *label != want {
                    *label = want;
                }
            }
        }
    }

    if let (Some((_, size)), Some((_, sex))) = (&art.races, cur) {
        for (face, mut img) in &mut faces {
            if let Some(tc) = race_tc(face.0, sex) {
                let rect = Some(tc_rect(*size, tc));
                if img.rect != rect {
                    img.rect = rect;
                }
            }
        }
    }

    let class = &content.classes[selection.class];
    let color = rgb(class.color);
    let title = hero_title(&content, &selection);
    let paint = |t: &mut Text, c: &mut TextColor, want: &str| {
        if t.0 != want {
            t.0 = want.to_owned();
        }
        if c.0 != color {
            c.0 = color;
        }
    };
    if let Ok((mut t, mut c)) = texts.p0().single_mut() {
        paint(&mut t, &mut c, &title);
    }
    if let Ok((mut t, mut c)) = texts.p1().single_mut() {
        paint(&mut t, &mut c, &class.name);
    }
    let start = &content.abilities[class.start_ix].name;
    let class_line = format!(
        "{}\nHealth {:.0}   Regen {:.1}/s   Speed {:.1}   Starts with {start}",
        class.description, class.base_hp, class.base_regen, class.speed
    );
    // A race's passives show in its button's tooltip ([`super::tooltip`]).
    if let Ok(mut t) = texts.p2().single_mut() {
        if t.0 != class_line {
            t.0 = class_line;
        }
    }
    let zone = &content.zones[selection.zone];
    let zone_line = format!(
        "{}\n\n{} kinds of enemy.",
        zone.description,
        zone.enemy_ix.len()
    );
    if let Ok(mut t) = texts.p3().single_mut() {
        if t.0 != zone_line {
            t.0 = zone_line;
        }
    }
    // A pointed-at spell shows its full tooltip by the cursor ([`super::tooltip`]).
    let hover = String::new();
    if let Ok(mut t) = texts.p4().single_mut() {
        if t.0 != hover {
            t.0 = hover;
        }
    }

    // The class's spells, rebuilt when the class changes.
    if *shown_class != Some(selection.class) {
        if let Ok(row) = rows.single() {
            *shown_class = Some(selection.class);
            commands.entity(row).despawn_related::<Children>();
            commands.entity(row).with_children(|r| {
                for &a in &class.pool_ix {
                    r.spawn((
                        SpellSlot(a),
                        SpellIcon(content.abilities[a].visual),
                        Button,
                        ImageNode {
                            color: Color::srgba(0.2, 0.2, 0.2, 1.0),
                            ..default()
                        },
                        Node {
                            width: Val::Px(ICON),
                            height: Val::Px(ICON),
                            ..default()
                        },
                    ));
                }
            });
        }
    }
}

/// Fill icon slots from `SpellIcon.dbc` once the client data and the asset reader are up.
pub(super) fn fill_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &SpellIcon), Without<IconDone>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u32, Handle<Image>>>,
) {
    let (Some(spells), Some(assets)) = (spells.as_deref(), assets.as_deref_mut()) else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let handle = match cache.get(&icon.0) {
            Some(h) => Some(h.clone()),
            None => {
                let path = spells
                    .catalog
                    .get(icon.0)
                    .and_then(|d| d.icon.clone())
                    .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".into());
                let h = assets.sprite_texture(&format!("{path}.blp"), &mut images);
                if let Some(h) = &h {
                    cache.insert(icon.0, h.clone());
                }
                h
            }
        };
        if let Some(h) = handle {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(IconDone);
    }
}

/// The menu's hero is the picked look, standing in the stage zone: respawned when the pick
/// changes.
pub(super) fn preview_hero(
    mut commands: Commands,
    selection: Res<Selection>,
    heroes: Query<Entity, With<PreviewHero>>,
    world: WorldPoint,
    char_create: Option<Res<crate::entities::CharCreate>>,
    mut items: ResMut<crate::items::Items>,
    mut shown: Local<Option<(usize, u32, usize)>>,
) {
    let Some(char_create) = char_create.as_deref() else {
        return;
    };
    let content = content();
    let want = selection.hero_look();
    let key = want.map(|l| (selection.class, l.0, selection.stage));
    if key == *shown && (key.is_none() || !heroes.is_empty()) {
        return;
    }
    for e in &heroes {
        commands.entity(e).despawn();
    }
    *shown = key;
    let Some((_, race, sex)) = want else {
        return;
    };
    let class = &content.classes[selection.class];
    let mut pos = units::wow_to_bevy(content.zones[selection.stage].center);
    if let Some(h) = world.terrain_height_under(pos) {
        pos.y = h;
    }
    // The run's own body: the race's player model in the class's starting outfit. Its serial is
    // clear of the run's (heroes 0x10 + player, enemies the simulation's ids).
    if let Some(e) = dress::spawn_player_body(
        &mut commands,
        &mut items,
        char_create,
        class,
        (race, sex),
        0x00E0_0000,
        1,
        pos,
        0.0,
    ) {
        commands.entity(e).insert(PreviewHero);
    }
}

/// A slow orbit around the menu's hero; until it has a body, around the stage zone's centre. The
/// terrain streams around it.
pub(super) fn menu_camera(
    time: Res<Time<Real>>,
    selection: Res<Selection>,
    world: WorldPoint,
    heroes: Query<&Transform, (With<PreviewHero>, Without<WorldCamera>)>,
    mut player: ResMut<crate::player::Player>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
    mut orbit: Local<f32>,
) {
    let content = content();
    let focus = heroes.single().map(|t| t.translation).unwrap_or_else(|_| {
        let mut p = units::wow_to_bevy(content.zones[selection.stage].center);
        if let Some(h) = world.terrain_height_under(p) {
            p.y = h;
        }
        p
    });
    if !player.active {
        player.pos = focus;
    }
    *orbit += time.delta_secs() * 0.2;
    let d = 7.0;
    let eye = focus + Vec3::new(orbit.sin() * d, 1.9, orbit.cos() * d);
    let placed = Transform::from_translation(eye).looking_at(focus + Vec3::Y * 0.2, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

/// The reference lays its glue screens out on a screen 768 units tall at any resolution, and so
/// does all of ours: the menu, the HUD and the level-up screen. (WoW's own frames are drawn by
/// the UI engine, not Bevy's UI, and keep their own scale.)
pub(super) fn apply_ui_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
) {
    let want = windows.single().map(|w| w.height() / 768.0).unwrap_or(1.0);
    if want > 0.0 && (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}

/// The menu's root shows only at the menu.
pub(super) fn show_menu(
    phase: Res<State<Phase>>,
    mut roots: Query<&mut Visibility, With<MenuRoot>>,
) {
    let want = if *phase.get() == Phase::Menu {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// Leaving the menu: the preview hero goes (the run's hero takes its place).
pub(super) fn clear_preview(mut commands: Commands, heroes: Query<Entity, With<PreviewHero>>) {
    for e in &heroes {
        commands.entity(e).despawn();
    }
}
