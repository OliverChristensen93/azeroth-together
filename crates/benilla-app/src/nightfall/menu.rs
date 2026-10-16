//! Nightfall's main menu, built from the glue screens' own widgets and art ([`crate::glue`]): the
//! title, the class buttons and the class's line, Play Solo, Host Co-op, Join (an invite code or an
//! address from the clipboard), the lobby's status, and Back to the pick screen. During a run, a
//! Leave Run button at the top right.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};
use nightfall_sim::Content;

use super::session::{Net, Role};
use super::{content, Phase};
use crate::game_pick::GameMode;
use crate::glue::art::GlueArt;
use crate::glue::widgets::{glue_button, GlueBtnKind};
use crate::glue::GlueClicks;
use crate::textinput::{wayland_display, HostClipboard};

/// The longest code or address the join takes.
const MAX_ADDRESS: usize = 128;

/// The class picked.
#[derive(Resource)]
pub(super) struct Selection {
    pub class: usize,
}

impl Selection {
    pub(super) fn new(content: &Content) -> Self {
        let class = super::class_key()
            .and_then(|k| content.class(&k))
            .unwrap_or(0);
        Self { class }
    }
}

/// Play Solo or Start Run was clicked: the run starts next frame.
#[derive(Resource, Default)]
pub(super) struct StartRequest(pub bool);

#[derive(Component)]
pub(super) struct MenuRoot;

#[derive(Component)]
pub(super) struct RunBar;

#[derive(Component)]
pub(super) struct MenuBuilt;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuAction {
    Class(usize),
    PlaySolo,
    Host,
    Join,
    CopyCode,
    StartRun,
    Leave,
    Back,
}

/// The run bar's button: end the run (a host takes everyone back to the lobby).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LeaveRun;

#[derive(Component)]
pub(super) struct ClassLine;

#[derive(Component)]
pub(super) struct StatusText;

/// Which (menu, run bar) show: the menu at Nightfall's menu, the run bar during its runs.
pub(super) fn roots_visible(mode: GameMode, phase: Phase) -> (bool, bool) {
    let ours = mode == GameMode::Nightfall;
    (ours && phase == Phase::Menu, ours && phase == Phase::Playing)
}

/// The code or address in pasted text, whatever surrounds it.
fn clean_address(text: &str) -> Option<String> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || ".:-[]".contains(*c))
        .take(MAX_ADDRESS)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

pub(super) fn spawn_roots(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(410),
        Visibility::Hidden,
    ));
    commands.spawn((
        RunBar,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        GlobalZIndex(410),
        Visibility::Hidden,
    ));
}

pub(super) fn show_roots(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    mut menus: Query<&mut Visibility, (With<MenuRoot>, Without<RunBar>)>,
    mut bars: Query<&mut Visibility, (With<RunBar>, Without<MenuRoot>)>,
) {
    let (menu, bar) = roots_visible(*mode.get(), *phase.get());
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut menus {
        if *v != vis(menu) {
            *v = vis(menu);
        }
    }
    for mut v in &mut bars {
        if *v != vis(bar) {
            *v = vis(bar);
        }
    }
}

/// Build both screens once the glue art is in.
pub(super) fn build_menu(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, Without<MenuBuilt>)>,
    bars: Query<Entity, With<RunBar>>,
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
    let content = content();
    let text = |s: &str, size: f32, color: Color| {
        (
            Text::new(s),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
        )
    };
    commands.entity(root).insert(MenuBuilt).with_children(|ui| {
        ui.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(40.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|panel| {
            panel.spawn(text("Nightfall", 32.0, Color::srgb(0.85, 0.8, 0.6)));
            panel
                .spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    for (i, class) in content.classes.iter().enumerate() {
                        glue_button(row, &art, &font, MenuAction::Class(i), &class.name, 150.0, 38.0, GlueBtnKind::Small, 1.0);
                    }
                });
            panel.spawn((text("", 13.0, Color::WHITE), ClassLine));
            panel
                .spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    for (action, caption) in [
                        (MenuAction::PlaySolo, "Play Solo"),
                        (MenuAction::Host, "Host Co-op"),
                        (MenuAction::Join, "Join (paste code)"),
                        (MenuAction::CopyCode, "Copy Invite Code"),
                        (MenuAction::StartRun, "Start Run"),
                        (MenuAction::Leave, "Leave"),
                    ] {
                        glue_button(row, &art, &font, action, caption, 170.0, 38.0, GlueBtnKind::Small, 1.0);
                    }
                });
            panel.spawn((text("", 13.0, Color::srgb(0.9, 0.85, 0.7)), StatusText));
        });
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(c, &art, &font, MenuAction::Back, "Back", 120.0, 32.0, GlueBtnKind::Small, 1.0);
        });
    });
    if let Ok(bar) = bars.single() {
        commands.entity(bar).with_children(|c| {
            glue_button(c, &art, &font, LeaveRun, "Leave Run", 140.0, 32.0, GlueBtnKind::Small, 1.0);
        });
    }
}

pub(super) fn menu_clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &MenuAction)>,
    mut selection: ResMut<Selection>,
    mut start: ResMut<StartRequest>,
    mut net: ResMut<Net>,
    mut next_mode: ResMut<NextState<GameMode>>,
    mut clipboard: NonSendMut<HostClipboard>,
    window: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    let Some(action) = actions.iter().find(|(e, _)| clicks.hit(*e)).map(|(_, a)| *a) else {
        return;
    };
    let display = || wayland_display(window.iter().next());
    match action {
        MenuAction::Class(i) => selection.class = i,
        MenuAction::PlaySolo => {
            if net.is_solo() {
                start.0 = true;
            }
        }
        MenuAction::Host => {
            if net.is_solo() {
                net.host();
            }
        }
        MenuAction::Join => {
            if net.is_solo() {
                match clipboard.read(display()).as_deref().and_then(clean_address) {
                    Some(addr) => net.join(&addr),
                    None => {
                        net.status =
                            "Copy a friend's invite code (or address) first, then Join.".into();
                    }
                }
            }
        }
        MenuAction::CopyCode => {
            if let Some(code) = net.invite_code() {
                clipboard.write(display(), &code);
                net.status = "Invite code copied: paste it to your friends".into();
            }
        }
        MenuAction::StartRun => start.0 = true,
        MenuAction::Leave => net.leave(""),
        MenuAction::Back => {
            net.leave("");
            next_mode.set(GameMode::Pick);
        }
    }
}

/// Which actions show for the session's role: alone, hosting or joined.
fn shown(role: &Role) -> &'static [MenuAction] {
    match role {
        Role::Solo => &[MenuAction::PlaySolo, MenuAction::Host, MenuAction::Join],
    }
}

/// The class line, the status lines and which buttons show.
pub(super) fn refresh_menu(
    selection: Res<Selection>,
    net: Res<Net>,
    mut lines: Query<&mut Text, (With<ClassLine>, Without<StatusText>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<ClassLine>)>,
    mut buttons: Query<(&MenuAction, &mut Node)>,
) {
    let content = content();
    let class = &content.classes[selection.class];
    let line = format!("{}: {}", class.name, class.description);
    for mut t in &mut lines {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
    let body = net.lobby_text();
    for mut t in &mut status {
        if t.0 != body {
            t.0 = body.clone();
        }
    }
    let visible = shown(&net.role);
    for (action, mut node) in &mut buttons {
        let on = matches!(action, MenuAction::Class(_) | MenuAction::Back) || visible.contains(action);
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nightfall_screens_show_only_in_nightfall() {
        assert_eq!(roots_visible(GameMode::Nightfall, Phase::Menu), (true, false));
        assert_eq!(roots_visible(GameMode::Nightfall, Phase::Playing), (false, true));
        assert_eq!(roots_visible(GameMode::Pick, Phase::Menu), (false, false));
        assert_eq!(roots_visible(GameMode::Survivors, Phase::Playing), (false, false));
    }

    #[test]
    fn a_pasted_code_is_cleaned_of_what_surrounds_it() {
        assert_eq!(clean_address("  abc123xyz\n"), Some("abc123xyz".into()));
        assert_eq!(clean_address("192.168.1.5:7777"), Some("192.168.1.5:7777".into()));
        assert_eq!(clean_address(" \n "), None);
    }
}
