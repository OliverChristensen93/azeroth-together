//! The talent web between runs ([`together_sim::TalentWeb`]): the Talents screen at the menu,
//! where a player spends points on the web for the class they picked, and the renown a run earns
//! toward more points.
//!
//! The screen draws every node and link of the web on a canvas that pans (drag) and zooms (the
//! wheel). Hovering a node shows what it gives and what taking it costs; a click takes it, and
//! the shortest way to it, or gives a taken node back when nothing else hangs on it. Respec is
//! free, and each class keeps three loadouts. Points are counted per class or shared by every
//! class: a setting on the screen.
//!
//! Everything a player keeps is `benilla-config/together-talents.txt`. In co-op the allocation
//! travels in the lobby pick ([`together_net::HeroPick::talents`]), and the host gives every
//! hero its talents when it builds the run ([`super::Sim::hosted`]).
//!
//! Test switches: `TOGETHER_TALENTS=1` opens the screen at start; `TOGETHER_TALENT_POINTS=<n>`
//! adds n points for this session (never past the web's top).

use std::sync::{Arc, OnceLock};

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition, UiTransform};
use together_sim::{renown_for_run, NodeKind, Tag, TalentProfile, TalentWeb, LOADOUT_SLOTS};

use super::menu::{framed, MenuBuilt, MenuRoot, Selection};
use super::{content, Phase, Sim};
use crate::glue::art::{GlueArt, DIM, GOLD};
use crate::glue::widgets::{glue_button, outlined_text, GlueBtnKind, GlueText};
use crate::glue::GlueClicks;

static WEB: OnceLock<Arc<TalentWeb>> = OnceLock::new();

/// Keep the web [`super::prepare`] read.
pub(super) fn set_web(web: TalentWeb) {
    let _ = WEB.set(Arc::new(web));
}

/// The talent web, as [`super::prepare`] read it.
pub(super) fn web() -> Arc<TalentWeb> {
    WEB.get().expect("together::prepare ran").clone()
}

/// The canvas's edges inside the screen, in UI units: left and right, top, bottom.
const CANVAS_SIDE: f32 = 12.0;
const CANVAS_TOP: f32 = 62.0;
const CANVAS_BOTTOM: f32 = 92.0;
/// How far the view zooms in and out, and where it starts.
const ZOOM_MIN: f32 = 0.25;
const ZOOM_MAX: f32 = 2.0;
const ZOOM_START: f32 = 0.6;
/// A class name's width under its start.
const LABEL_W: f32 = 120.0;

/// The talents a player keeps, and the screen's view of the web.
#[derive(Resource)]
pub(super) struct Talents {
    pub profile: TalentProfile,
    pub open: bool,
    /// The web point at the canvas's middle, and UI units per web unit.
    center: Vec2,
    zoom: f32,
    /// The class the view was last centred on, by index into the content's classes.
    centred_for: Option<usize>,
    /// Bumped on every change to what is taken, so the screen recolours.
    version: u32,
    /// The node under the cursor, and the nodes a click on it would take.
    hovered: Option<usize>,
    preview: Vec<usize>,
    /// Where a drag of the canvas last had the cursor, in canvas UI units.
    drag: Option<Vec2>,
    /// `TOGETHER_TALENT_POINTS`: points for this session only.
    session_bonus: u32,
}

impl Default for Talents {
    fn default() -> Self {
        Self {
            profile: load().unwrap_or_default(),
            open: std::env::var_os("TOGETHER_TALENTS").is_some(),
            center: Vec2::ZERO,
            zoom: ZOOM_START,
            centred_for: None,
            version: 0,
            hovered: None,
            preview: Vec::new(),
            drag: None,
            session_bonus: std::env::var("TOGETHER_TALENT_POINTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        }
    }
}

impl Talents {
    /// The points `class` has to spend.
    pub fn points(&self, class: &str) -> u32 {
        let web = web();
        (self.profile.points(class, &web.points) + self.session_bonus).min(web.points.max)
    }

    /// What `class` has taken in its loadout in use, as far as its points reach.
    pub fn taken(&self, class: &str) -> Vec<usize> {
        let web = web();
        let saved = web.indices(self.profile.active(class));
        web.sanitize(class, &saved, self.points(class))
    }

    /// The allocation as the lobby pick carries it.
    pub fn pick(&self, class: &str) -> Vec<u16> {
        self.taken(class).into_iter().map(|n| n as u16).collect()
    }

    /// Make `taken` the loadout `class` uses, and save.
    fn set_taken(&mut self, class: &str, taken: &[usize]) {
        let ids = web().ids(taken);
        let c = self.profile.class_mut(class);
        let slot = c.active.min(LOADOUT_SLOTS - 1);
        c.loadouts[slot] = ids;
        self.changed();
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
        save(&self.profile);
    }
}

fn path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("together-talents.txt"))
}

fn load() -> Option<TalentProfile> {
    let text = crate::local_state::read_to_string(&path()?).ok()?;
    Some(TalentProfile::from_text(&text))
}

fn save(profile: &TalentProfile) {
    if let Some(path) = path() {
        let _ = crate::local_state::write_atomic(&path, &profile.to_text());
    }
}

/// What the run just played earned, given once.
#[derive(Resource, Default)]
pub(super) struct RunAward {
    done: bool,
    /// The run so far: our class key and name, the party's level, seconds and kills.
    last: Option<(String, String, u32, f32, u32)>,
    text: String,
}

/// The renown line shown at the end of a run.
#[derive(Component)]
pub(super) struct AwardPanel;

#[derive(Component)]
pub(super) struct AwardText;

/// The screen's buttons.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TalentAction {
    Open,
    Close,
    Loadout(usize),
    Respec,
    Mode,
    Center,
    TestPoints,
}

#[derive(Component)]
pub(super) struct TalentsBuilt;

#[derive(Component)]
pub(super) struct TalentScreen;

#[derive(Component)]
pub(super) struct TalentCanvas;

/// A node on the canvas, by index into the web.
#[derive(Component)]
pub(super) struct TalentDot(usize);

/// A link between two nodes.
#[derive(Component)]
pub(super) struct TalentEdge(usize, usize);

/// A start node's class name.
#[derive(Component)]
pub(super) struct TalentLabel(usize);

#[derive(Component)]
pub(super) struct TalentTitle;

#[derive(Component)]
pub(super) struct TalentPoints;

#[derive(Component)]
pub(super) struct TalentTip;

#[derive(Component)]
pub(super) struct TalentTipText;

pub(super) struct TalentsPlugin;

impl Plugin for TalentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Talents>()
            .init_resource::<RunAward>()
            .add_systems(Startup, setup_award)
            .add_systems(
                Update,
                (
                    build_screen,
                    input,
                    refresh.before(crate::glue::GlueVisuals),
                )
                    .chain()
                    .run_if(in_state(Phase::Menu)),
            )
            .add_systems(Update, track_run.run_if(in_state(Phase::Playing)))
            .add_systems(Update, show_award.after(track_run))
            .add_systems(OnEnter(Phase::Playing), |mut a: ResMut<RunAward>| {
                *a = RunAward::default();
            })
            .add_systems(OnExit(Phase::Playing), award_on_leave)
            .add_systems(OnExit(Phase::Menu), |mut t: ResMut<Talents>| {
                t.open = false;
            });
    }
}

/// A school's colour on the web.
fn school_color(tag: Option<Tag>) -> Color {
    match tag {
        Some(Tag::Fire) => Color::srgb(0.95, 0.45, 0.12),
        Some(Tag::Frost) => Color::srgb(0.45, 0.75, 1.0),
        Some(Tag::Arcane) => Color::srgb(0.75, 0.5, 1.0),
        Some(Tag::Nature) => Color::srgb(0.4, 0.85, 0.3),
        Some(Tag::Shadow) => Color::srgb(0.6, 0.35, 0.8),
        Some(Tag::Holy) => Color::srgb(1.0, 0.88, 0.45),
        Some(Tag::Physical) => Color::srgb(0.8, 0.62, 0.45),
        _ => Color::srgb(0.72, 0.72, 0.68),
    }
}

/// WoW's class colours, for the starts.
fn class_color(key: &str) -> Color {
    let hex = match key {
        "warrior" => 0xC79C6E,
        "paladin" => 0xF58CBA,
        "hunter" => 0xABD473,
        "rogue" => 0xFFF569,
        "priest" => 0xFFFFFF,
        "shaman" => 0x0070DE,
        "mage" => 0x69CCF0,
        "warlock" => 0x9482C9,
        "druid" => 0xFF7D0A,
        _ => 0xCCCCCC,
    };
    super::menu::rgb(hex)
}

/// A node's size on the canvas at zoom 1, in UI units.
fn base_size(kind: NodeKind) -> f32 {
    match kind {
        NodeKind::Start => 34.0,
        NodeKind::Small => 15.0,
        NodeKind::Notable => 25.0,
        NodeKind::Keystone => 34.0,
    }
}

fn dim(c: Color, by: f32) -> Color {
    let s = c.to_srgba();
    Color::srgba(s.red * by, s.green * by, s.blue * by, s.alpha)
}

/// The Talents button under the race and class tower, and the screen itself (hidden), built
/// into the menu once it stands.
#[allow(clippy::type_complexity)]
pub(super) fn build_screen(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, With<MenuBuilt>, Without<TalentsBuilt>)>,
    art: Res<GlueArt>,
    asset_server: Res<AssetServer>,
) {
    let Ok(root) = roots.single() else { return };
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;
    let web = web();
    let px = Val::Px;
    commands.entity(root).insert(TalentsBuilt).with_children(|ui| {
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            left: px(56.0),
            bottom: px(40.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                f,
                TalentAction::Open,
                "Talents",
                150.0,
                38.0,
                GlueBtnKind::Small,
                1.0,
            );
        });

        ui.spawn((
            TalentScreen,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.01, 0.02, 0.985)),
            FocusPolicy::Block,
            GlobalZIndex(420),
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            // The title and the points line.
            screen
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    top: px(8.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4.0),
                    ..default()
                })
                .with_children(|head| {
                    outlined_text(
                        head,
                        Node::default(),
                        (),
                        TalentTitle,
                        GlueText {
                            text: "Talents",
                            size: 22.0,
                            color: GOLD,
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                    outlined_text(
                        head,
                        Node::default(),
                        (),
                        TalentPoints,
                        GlueText {
                            text: "",
                            size: 13.0,
                            color: Color::WHITE,
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                });

            // The web.
            screen
                .spawn((
                    TalentCanvas,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(CANVAS_SIDE),
                        right: px(CANVAS_SIDE),
                        top: px(CANVAS_TOP),
                        bottom: px(CANVAS_BOTTOM),
                        overflow: Overflow::clip(),
                        border: UiRect::all(px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 1.0)),
                    BorderColor::all(Color::srgb(0.3, 0.27, 0.2)),
                    FocusPolicy::Block,
                    Interaction::default(),
                    RelativeCursorPosition::default(),
                ))
                .with_children(|canvas| {
                    for (i, n) in web.nodes.iter().enumerate() {
                        for &j in &n.adjacent {
                            if j > i {
                                canvas.spawn((
                                    TalentEdge(i, j),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.25, 0.25, 0.25)),
                                    UiTransform::default(),
                                ));
                            }
                        }
                    }
                    for (i, n) in web.nodes.iter().enumerate() {
                        canvas.spawn((
                            TalentDot(i),
                            Node {
                                position_type: PositionType::Absolute,
                                border: UiRect::all(px(2.0)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::BLACK),
                            BorderColor::all(Color::BLACK),
                            Interaction::default(),
                        ));
                        if n.kind == NodeKind::Start {
                            canvas.spawn((
                                TalentLabel(i),
                                Node {
                                    position_type: PositionType::Absolute,
                                    width: px(LABEL_W),
                                    ..default()
                                },
                                Text::new(n.name.clone()),
                                TextLayout::new_with_justify(Justify::Center),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(class_color(n.class.as_deref().unwrap_or(""))),
                                TextShadow::default(),
                            ));
                        }
                    }
                });

            // The buttons.
            screen
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    bottom: px(38.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    column_gap: px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    let mut buttons: Vec<(TalentAction, &str)> = vec![
                        (TalentAction::Loadout(0), "Loadout 1"),
                        (TalentAction::Loadout(1), "Loadout 2"),
                        (TalentAction::Loadout(2), "Loadout 3"),
                    ];
                    buttons.extend([
                        (TalentAction::Respec, "Respec"),
                        (TalentAction::Mode, "Per Class / Shared"),
                        (TalentAction::Center, "Center"),
                        (TalentAction::TestPoints, "Test: +5 Points"),
                        (TalentAction::Close, "Close"),
                    ]);
                    for (action, caption) in buttons {
                        let w = match action {
                            TalentAction::Mode | TalentAction::TestPoints => 150.0,
                            _ => 104.0,
                        };
                        glue_button(row, &art, f, action, caption, w, 30.0, GlueBtnKind::Small, 1.0);
                    }
                });
            screen
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    bottom: px(14.0),
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
                            text: "Click a node to take it and the way to it; click a taken node to give it back. Drag to move, wheel to zoom, Escape to close.",
                            size: 11.0,
                            color: DIM,
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                });

            // The tooltip, which follows the cursor.
            screen
                .spawn((
                    TalentTip,
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    GlobalZIndex(430),
                    Visibility::Hidden,
                ))
                .with_children(|tip| {
                    framed(
                        tip,
                        &art,
                        Node {
                            width: px(320.0),
                            padding: UiRect::new(px(12.0), px(12.0), px(10.0), px(10.0)),
                            ..default()
                        },
                        |panel| {
                            outlined_text(
                                panel,
                                Node {
                                    width: px(296.0),
                                    ..default()
                                },
                                (),
                                TalentTipText,
                                GlueText {
                                    text: "",
                                    size: 12.0,
                                    color: Color::WHITE,
                                    wrap: true,
                                },
                                f,
                                1.0,
                            );
                        },
                    );
                });
        });
    });
}

/// The canvas's size and the cursor on it, in UI units; the cursor `None` when off it.
fn canvas_view(node: &ComputedNode, rel: &RelativeCursorPosition) -> (Vec2, Option<Vec2>) {
    let size = node.size() * node.inverse_scale_factor();
    let cursor = rel
        .normalized
        .filter(|_| rel.cursor_over())
        .map(|n| (n + Vec2::splat(0.5)) * size);
    (size, cursor)
}

/// Clicks, the wheel and drags on the screen.
#[allow(clippy::too_many_arguments)]
pub(super) fn input(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &TalentAction)>,
    dots: Query<(Entity, &TalentDot, &Interaction)>,
    canvas: Query<(&ComputedNode, &RelativeCursorPosition), With<TalentCanvas>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    selection: Res<Selection>,
    mut talents: ResMut<Talents>,
) {
    let content = content();
    let class = content.classes[selection.class].key.clone();
    let web = web();
    if let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    {
        match action {
            TalentAction::Open => talents.open = true,
            TalentAction::Close => talents.open = false,
            TalentAction::Loadout(slot) => {
                talents.profile.class_mut(&class).active = slot.min(LOADOUT_SLOTS - 1);
                talents.changed();
            }
            TalentAction::Respec => talents.set_taken(&class, &[]),
            TalentAction::Mode => {
                talents.profile.shared = !talents.profile.shared;
                talents.changed();
            }
            TalentAction::Center => talents.centred_for = None,
            TalentAction::TestPoints => {
                let max = web.points.max;
                let p = &mut talents.profile;
                p.bonus = if p.bonus + 5 > max { 0 } else { p.bonus + 5 };
                talents.changed();
            }
        }
    }
    if !talents.open {
        talents.drag = None;
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        talents.open = false;
        return;
    }
    // Centre on the class's own tree when the class changes, or on Center.
    if talents.centred_for != Some(selection.class) {
        if let Some(s) = web.start_of(&class) {
            let (x, y) = web.nodes[s].pos;
            talents.center = Vec2::new(x, y) * 1.25;
        }
        talents.zoom = ZOOM_START;
        talents.centred_for = Some(selection.class);
    }

    let hovered = dots
        .iter()
        .find(|(_, _, i)| **i != Interaction::None)
        .map(|(_, d, _)| d.0);
    if hovered != talents.hovered {
        talents.hovered = hovered;
        let taken = talents.taken(&class);
        talents.preview = hovered
            .and_then(|n| web.path_to(&class, &taken, n))
            .unwrap_or_default();
    }

    if let Some(n) = dots
        .iter()
        .find(|(e, _, _)| clicks.hit(*e))
        .map(|(_, d, _)| d.0)
    {
        let mut taken = talents.taken(&class);
        if taken.contains(&n) {
            if web.can_refund(&class, &taken, n) {
                taken.retain(|&x| x != n);
                talents.set_taken(&class, &taken);
            }
        } else if let Some(path) = web.path_to(&class, &taken, n) {
            let free =
                talents.points(&class) as usize - taken.len().min(talents.points(&class) as usize);
            if path.len() <= free {
                taken.extend(path);
                talents.set_taken(&class, &taken);
            }
        }
        let taken = talents.taken(&class);
        talents.preview = talents
            .hovered
            .and_then(|h| web.path_to(&class, &taken, h))
            .unwrap_or_default();
    }

    let Ok((node, rel)) = canvas.single() else {
        return;
    };
    let (size, cursor) = canvas_view(node, rel);
    // The wheel zooms about the cursor.
    if let Some(at) = cursor {
        let lines = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        if lines != 0.0 {
            let off = at - size / 2.0;
            let under = talents.center + off / talents.zoom;
            let zoom = (talents.zoom * 1.15f32.powf(lines)).clamp(ZOOM_MIN, ZOOM_MAX);
            talents.zoom = zoom;
            talents.center = under - off / zoom;
        }
    }
    // A press on the bare canvas drags the web.
    if mouse.just_pressed(MouseButton::Left) && hovered.is_none() {
        talents.drag = cursor;
    }
    if !mouse.pressed(MouseButton::Left) {
        talents.drag = None;
    }
    if let (Some(from), Some(to)) = (
        talents.drag,
        rel.normalized.map(|n| (n + Vec2::splat(0.5)) * size),
    ) {
        if from != to {
            let zoom = talents.zoom;
            talents.center -= (to - from) / zoom;
            talents.drag = Some(to);
        }
    }
}

/// The screen as it stands: shown or hidden, every node and link placed for the view and
/// coloured for what is taken, the title, the points and the tooltip.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    talents: Res<Talents>,
    selection: Res<Selection>,
    mut screen: Query<&mut Visibility, (With<TalentScreen>, Without<TalentTip>)>,
    canvas: Query<(&ComputedNode, &RelativeCursorPosition), With<TalentCanvas>>,
    mut dots: Query<
        (
            &TalentDot,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<TalentEdge>,
    >,
    mut edges: Query<
        (
            &TalentEdge,
            &mut Node,
            &mut BackgroundColor,
            &mut UiTransform,
        ),
        Without<TalentDot>,
    >,
    mut labels: Query<
        (&TalentLabel, &mut Node),
        (Without<TalentDot>, Without<TalentEdge>, Without<TalentTip>),
    >,
    mut texts: Query<
        (
            &mut Text,
            Has<TalentTitle>,
            Has<TalentPoints>,
            Has<TalentTipText>,
        ),
        Or<(With<TalentTitle>, With<TalentPoints>, With<TalentTipText>)>,
    >,
    mut tip: Query<
        (&mut Node, &mut Visibility),
        (
            With<TalentTip>,
            Without<TalentDot>,
            Without<TalentEdge>,
            Without<TalentLabel>,
        ),
    >,
    mut last: Local<Option<(Vec2, f32, Vec2, u32, Option<usize>, usize, Option<usize>)>>,
) {
    let want = if talents.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut screen {
        if *v != want {
            *v = want;
        }
    }
    if !talents.open {
        *last = None;
        return;
    }
    let Ok((cnode, rel)) = canvas.single() else {
        return;
    };
    let (size, cursor) = canvas_view(cnode, rel);
    let content = content();
    let class_def = &content.classes[selection.class];
    let class = class_def.key.as_str();
    let web = web();
    let taken = talents.taken(class);
    let points = talents.points(class);

    let state = (
        talents.center,
        talents.zoom,
        size,
        talents.version,
        talents.hovered,
        selection.class,
        talents.preview.last().copied(),
    );
    if *last != Some(state) {
        *last = Some(state);
        let zoom = talents.zoom;
        let to_canvas = |p: (f32, f32)| (Vec2::new(p.0, p.1) - talents.center) * zoom + size / 2.0;
        let start = web.start_of(class);
        let held = |i: usize| taken.contains(&i) || Some(i) == start;
        let free = points as usize > taken.len();
        for (dot, mut node, mut bg, mut border) in &mut dots {
            let n = &web.nodes[dot.0];
            let s = (base_size(n.kind) * zoom).max(base_size(n.kind) * 0.45);
            let at = to_canvas(n.pos);
            node.left = Val::Px(at.x - s / 2.0);
            node.top = Val::Px(at.y - s / 2.0);
            node.width = Val::Px(s);
            node.height = Val::Px(s);
            let color = match n.kind {
                NodeKind::Start => class_color(n.class.as_deref().unwrap_or("")),
                _ => school_color(n.school()),
            };
            let next = web.can_take(class, &taken, dot.0);
            let (fill, edge) = if held(dot.0) {
                (color, GOLD)
            } else if talents.preview.contains(&dot.0) {
                (dim(color, 0.6), Color::srgb(0.4, 1.0, 0.4))
            } else if next && free {
                (dim(color, 0.45), Color::srgb(0.85, 0.85, 0.8))
            } else if n.kind == NodeKind::Start {
                (dim(color, 0.3), Color::srgb(0.35, 0.35, 0.35))
            } else {
                (dim(color, 0.22), Color::srgb(0.3, 0.3, 0.3))
            };
            let edge = if talents.hovered == Some(dot.0) {
                Color::WHITE
            } else {
                edge
            };
            if bg.0 != fill {
                bg.0 = fill;
            }
            *border = BorderColor::all(edge);
        }
        for (e, mut node, mut bg, mut tf) in &mut edges {
            let a = to_canvas(web.nodes[e.0].pos);
            let b = to_canvas(web.nodes[e.1].pos);
            let d = b - a;
            let len = d.length();
            let both = held(e.0) && held(e.1);
            let path = (talents.preview.contains(&e.0) || held(e.0))
                && (talents.preview.contains(&e.1) || held(e.1))
                && !both;
            let thick = if both { 4.0 } else { 2.0 };
            let mid = (a + b) / 2.0;
            node.left = Val::Px(mid.x - len / 2.0);
            node.top = Val::Px(mid.y - thick / 2.0);
            node.width = Val::Px(len);
            node.height = Val::Px(thick);
            tf.rotation = Rot2::radians(d.y.atan2(d.x));
            bg.0 = if both {
                Color::srgb(0.85, 0.68, 0.2)
            } else if path {
                Color::srgb(0.35, 0.8, 0.35)
            } else if held(e.0) || held(e.1) {
                Color::srgb(0.45, 0.42, 0.35)
            } else {
                Color::srgb(0.2, 0.2, 0.22)
            };
        }
        for (label, mut node) in &mut labels {
            let n = &web.nodes[label.0];
            let at = to_canvas(n.pos);
            let s = (base_size(n.kind) * zoom).max(base_size(n.kind) * 0.45);
            node.left = Val::Px(at.x - LABEL_W / 2.0);
            node.top = Val::Px(at.y + s / 2.0 + 2.0);
        }
    }

    // The tooltip.
    let tip_text = talents
        .hovered
        .map(|n| tooltip(&web, class, &taken, points, n));
    if let Ok((mut node, mut vis)) = tip.single_mut() {
        let show = tip_text.is_some() && cursor.is_some();
        let v = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != v {
            *vis = v;
        }
        if let Some(c) = cursor {
            let screen_w = size.x + 2.0 * CANVAS_SIDE;
            let mut x = CANVAS_SIDE + c.x + 18.0;
            if x + 330.0 > screen_w {
                x = CANVAS_SIDE + c.x - 338.0;
            }
            let y = (CANVAS_TOP + c.y + 18.0).min(CANVAS_TOP + size.y - 120.0);
            node.left = Val::Px(x);
            node.top = Val::Px(y);
        }
    }

    let (level, into, need) = web.points.level(talents.profile.counted_renown(class));
    let renown = if level >= web.points.max {
        "top talent level".to_string()
    } else {
        format!("talent level {level}, {into} of {need} renown to the next")
    };
    let mode = if talents.profile.shared {
        "points shared by all classes"
    } else {
        "points for this class alone"
    };
    let test = match talents.profile.bonus + talents.session_bonus {
        0 => String::new(),
        n => format!("   ·   {n} test points"),
    };
    let points_line = format!(
        "{} of {points} points spent   ·   Loadout {}   ·   {renown}   ·   {mode}{test}",
        taken.len(),
        talents.profile.active_slot(class) + 1
    );
    let title = format!("Talent Web: {}", class_def.name);
    for (mut text, is_title, is_points, is_tip) in &mut texts {
        let want = if is_title {
            title.clone()
        } else if is_points {
            points_line.clone()
        } else if is_tip {
            tip_text.clone().unwrap_or_default()
        } else {
            continue;
        };
        if text.0 != want {
            text.0 = want;
        }
    }
}

/// What hovering node `n` says, as glue markup: its name, kind, what it gives and what a click
/// does.
fn tooltip(web: &TalentWeb, class: &str, taken: &[usize], points: u32, n: usize) -> String {
    let node = &web.nodes[n];
    let kind = match node.kind {
        NodeKind::Start => "Class start",
        NodeKind::Small => "Talent",
        NodeKind::Notable => "Notable",
        NodeKind::Keystone => "Keystone (one at a time)",
    };
    let mut out = format!("|cffffc800{}|r\n|cff909090{kind}|r", node.name);
    for line in node.lines() {
        out += &format!("\n{line}");
    }
    let free = points.saturating_sub(taken.len() as u32);
    let status = if node.kind == NodeKind::Start {
        if web.start_of(class) == Some(n) {
            "|cff909090Your class starts here.|r".to_string()
        } else {
            "|cff909090Another class's start: no path runs through it.|r".to_string()
        }
    } else if taken.contains(&n) {
        if web.can_refund(class, taken, n) {
            "|cff40ff40Taken.|r Click to give it back.".to_string()
        } else {
            "|cff40ff40Taken.|r Other talents hang on it.".to_string()
        }
    } else {
        match web.path_to(class, taken, n) {
            Some(path) if path.len() as u32 <= free => {
                let s = if path.len() == 1 { "" } else { "s" };
                format!("|cff40ff40Click to take it: {} point{s}.|r", path.len())
            }
            Some(path) => format!(
                "|cffff4040Needs {} points; you have {free} left.|r",
                path.len()
            ),
            None if node.kind == NodeKind::Keystone && web.holds_keystone(taken) => {
                "|cffff4040You hold a keystone already: one at a time.|r".to_string()
            }
            None => "|cffff4040Out of reach.|r".to_string(),
        }
    };
    out += &format!("\n\n{status}");
    out
}

/// The renown line at the end of a run, under the results.
pub(super) fn setup_award(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            AwardPanel,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Percent(24.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ))
            .with_children(|b| {
                b.spawn((
                    AwardText,
                    Text::new(""),
                    TextFont {
                        font: font.clone(),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.9, 1.0)),
                    TextShadow::default(),
                    TextLayout::new_with_justify(Justify::Center),
                ));
            });
        });
}

/// During a run: remember how far it got, and when it is over, give our hero's class its renown.
pub(super) fn track_run(
    sim: Option<Res<Sim>>,
    gave_up: Res<super::pause::GaveUp>,
    mut talents: ResMut<Talents>,
    mut award: ResMut<RunAward>,
) {
    let Some(sim) = sim else { return };
    let world = &sim.world;
    let Some(hero) = world.hero(sim.me) else {
        return;
    };
    let class = &world.content().classes[hero.class];
    award.last = Some((
        class.key.clone(),
        class.name.clone(),
        world.level,
        world.elapsed,
        world.kills,
    ));
    if (world.game_over() || gave_up.0) && !award.done {
        give(&mut talents, &mut award);
    }
}

/// Leaving a run before it was over (the host ended it): what it reached still counts.
pub(super) fn award_on_leave(mut talents: ResMut<Talents>, mut award: ResMut<RunAward>) {
    if !award.done {
        give(&mut talents, &mut award);
    }
}

fn give(talents: &mut Talents, award: &mut RunAward) {
    award.done = true;
    let Some((key, name, level, elapsed, kills)) = award.last.clone() else {
        return;
    };
    let earned = renown_for_run(level, elapsed, kills);
    let before = talents.points(&key);
    talents.profile.earn(&key, earned);
    talents.changed();
    let web = web();
    let after = talents.points(&key);
    let (lvl, into, need) = web.points.level(talents.profile.counted_renown(&key));
    let mut text = format!("+{earned} renown for the {name}   ·   talent level {lvl}");
    if lvl < web.points.max {
        text += &format!(" ({into} of {need} to the next)");
    }
    if after > before {
        let n = after - before;
        let s = if n == 1 { "" } else { "s" };
        text += &format!("\n{n} new talent point{s}: spend them on Talents at the menu");
    }
    info!("together: {}", text.replace('\n', "; "));
    award.text = text;
}

/// The renown line shows once the run is over.
pub(super) fn show_award(
    sim: Option<Res<Sim>>,
    gave_up: Res<super::pause::GaveUp>,
    award: Res<RunAward>,
    mut panel: Query<&mut Visibility, With<AwardPanel>>,
    mut text: Query<&mut Text, With<AwardText>>,
) {
    let over = sim.is_some_and(|s| s.world.game_over()) || gave_up.0;
    let show = award.done && over && !award.text.is_empty();
    for mut v in &mut panel {
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for mut t in &mut text {
        if t.0 != award.text {
            t.0 = award.text.clone();
        }
    }
}
