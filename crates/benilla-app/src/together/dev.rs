//! The menu's dev panel, for testing a run's pace: multipliers for when exits open, how long a
//! flight takes, how fast heat climbs, how big the extraction horde gets and how fast heroes heal.
//! They are saved in `benilla-config/together-dev.txt` and apply to the runs this player starts
//! or hosts ([`together_sim::Content::tuned`]); a co-op guest's own settings do nothing, since
//! the host's run is the one played.

use bevy::prelude::*;
use together_sim::Tuning;

use super::menu::framed;
use crate::glue::art::{GlueArt, DIM, GOLD};
use crate::glue::widgets::{glue_button, outlined_text, GlueBtnKind, GlueText};
use crate::glue::GlueClicks;

/// The steps a multiplier moves through.
const STEPS: [f32; 10] = [0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 5.0];

/// The settings in the panel's order.
const ROWS: [&str; 5] = [
    "Exits open (time)",
    "Flight time",
    "Heat gain",
    "Horde size",
    "Hero regen",
];

/// The dev panel's state: whether it is open, and the multipliers.
#[derive(Resource)]
pub(super) struct DevSettings {
    pub open: bool,
    pub tuning: Tuning,
}

impl Default for DevSettings {
    fn default() -> Self {
        Self {
            open: false,
            tuning: load().unwrap_or_default(),
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DevAction {
    /// Opens or closes the panel.
    Toggle,
    /// One setting a step down or up.
    Down(usize),
    Up(usize),
    /// Every setting back to 1.
    Reset,
}

/// The panel's root, shown while open.
#[derive(Component)]
pub(super) struct DevPanel;

/// One setting's line: its name and value.
#[derive(Component)]
pub(super) struct DevRow(usize);

fn value(t: &Tuning, row: usize) -> f32 {
    [t.exits, t.flight, t.heat, t.horde, t.regen][row]
}

fn value_mut(t: &mut Tuning, row: usize) -> &mut f32 {
    match row {
        0 => &mut t.exits,
        1 => &mut t.flight,
        2 => &mut t.heat,
        3 => &mut t.horde,
        _ => &mut t.regen,
    }
}

/// `v` moved `by` steps along [`STEPS`], from the step nearest it.
fn step(v: f32, by: i32) -> f32 {
    let at = STEPS
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs()))
        .map_or(4, |(i, _)| i) as i32;
    STEPS[(at + by).clamp(0, STEPS.len() as i32 - 1) as usize]
}

fn path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("together-dev.txt"))
}

fn load() -> Option<Tuning> {
    let text = crate::local_state::read_to_string(&path()?).ok()?;
    let v: Vec<f32> = text
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let [exits, flight, heat, horde, regen] = v[..] else {
        return None;
    };
    Some(Tuning {
        exits,
        flight,
        heat,
        horde,
        regen,
    })
}

fn save(t: &Tuning) {
    if let Some(path) = path() {
        let line = format!(
            "{} {} {} {} {}",
            t.exits, t.flight, t.heat, t.horde, t.regen
        );
        let _ = crate::local_state::write_atomic(&path, &line);
    }
}

/// The Dev button in the menu's corner, above Exit.
pub(super) fn dev_button(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>) {
    ui.spawn(Node {
        position_type: PositionType::Absolute,
        right: Val::Px(20.0),
        bottom: Val::Px(60.0),
        ..default()
    })
    .with_children(|c| {
        glue_button(
            c,
            art,
            f,
            DevAction::Toggle,
            "Dev",
            120.0,
            32.0,
            GlueBtnKind::Small,
            1.0,
        );
    });
}

/// The panel, hidden until the Dev button opens it: a line and a pair of step buttons per
/// setting, and Reset and Close.
pub(super) fn dev_panel(ui: &mut ChildSpawnerCommands, art: &GlueArt, f: &Handle<Font>) {
    let px = Val::Px;
    const W: f32 = 340.0;
    ui.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(20.0),
            bottom: px(100.0),
            ..default()
        },
        DevPanel,
        Visibility::Hidden,
    ))
    .with_children(|corner| {
        framed(
            corner,
            art,
            Node {
                width: px(W),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(px(16.0), px(16.0), px(12.0), px(12.0)),
                row_gap: px(6.0),
                ..default()
            },
            |panel| {
                outlined_text(
                    panel,
                    Node::default(),
                    (),
                    (),
                    GlueText {
                        text: "Dev settings (runs you start or host)",
                        size: 14.0,
                        color: GOLD,
                        wrap: false,
                    },
                    f,
                    1.0,
                );
                for row in 0..ROWS.len() {
                    panel
                        .spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: px(6.0),
                            ..default()
                        })
                        .with_children(|line| {
                            outlined_text(
                                line,
                                Node {
                                    width: px(W - 32.0 - 2.0 * 42.0 - 12.0),
                                    ..default()
                                },
                                (),
                                DevRow(row),
                                GlueText {
                                    text: "",
                                    size: 13.0,
                                    color: Color::WHITE,
                                    wrap: false,
                                },
                                f,
                                1.0,
                            );
                            for (action, caption) in
                                [(DevAction::Down(row), "-"), (DevAction::Up(row), "+")]
                            {
                                glue_button(
                                    line,
                                    art,
                                    f,
                                    action,
                                    caption,
                                    42.0,
                                    26.0,
                                    GlueBtnKind::Small,
                                    1.0,
                                );
                            }
                        });
                }
                outlined_text(
                    panel,
                    Node::default(),
                    (),
                    (),
                    GlueText {
                        text: "Exits and flights: times, so below 1 is sooner.",
                        size: 11.0,
                        color: DIM,
                        wrap: false,
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
                        for (action, caption) in
                            [(DevAction::Reset, "Reset"), (DevAction::Toggle, "Close")]
                        {
                            glue_button(
                                row,
                                art,
                                f,
                                action,
                                caption,
                                110.0,
                                30.0,
                                GlueBtnKind::Small,
                                1.0,
                            );
                        }
                    });
            },
        );
    });
}

/// What a click on the panel or its button does.
pub(super) fn clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &DevAction)>,
    mut dev: ResMut<DevSettings>,
) {
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    match action {
        DevAction::Toggle => dev.open = !dev.open,
        DevAction::Down(row) | DevAction::Up(row) => {
            let by = if matches!(action, DevAction::Up(_)) { 1 } else { -1 };
            let v = value_mut(&mut dev.tuning, row);
            *v = step(*v, by);
            save(&dev.tuning);
        }
        DevAction::Reset => {
            dev.tuning = Tuning::default();
            save(&dev.tuning);
        }
    }
}

/// The panel shown while open, each line its setting's value.
pub(super) fn refresh(
    dev: Res<DevSettings>,
    mut panel: Query<&mut Visibility, With<DevPanel>>,
    mut rows: Query<(&DevRow, &mut Text)>,
) {
    for mut v in &mut panel {
        let want = if dev.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    for (row, mut text) in &mut rows {
        let s = format!("{}  x{}", ROWS[row.0], value(&dev.tuning, row.0));
        if text.0 != s {
            text.0 = s;
        }
    }
}

