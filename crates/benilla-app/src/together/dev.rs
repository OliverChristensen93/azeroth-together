//! The menu's dev panel, for testing: how fast experience comes, how tough the hero and the horde
//! are, how big the horde gets, and the rank new spells start at (so augments come within
//! minutes). Saved in `benilla-config/together-dev-run.txt`, they apply to the runs this player
//! starts or hosts; a co-op guest's own settings do nothing, since the host's run is the one
//! played. The panel's frame and buttons follow the extraction branch's dev panel.

use bevy::prelude::*;
use together_sim::{Stat, World, MAX_RANK};

use super::menu::framed;
use crate::glue::art::{GlueArt, DIM, GOLD};
use crate::glue::widgets::{glue_button, outlined_text, GlueBtnKind, GlueText};
use crate::glue::GlueClicks;

/// The steps a multiplier moves through.
const STEPS: [f32; 10] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0, 20.0];

/// One line of the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Experience,
    HeroHealth,
    EnemyHealth,
    EnemyDamage,
    Horde,
    StartRank,
}

impl Row {
    const ALL: [Row; 6] = [
        Row::Experience,
        Row::HeroHealth,
        Row::EnemyHealth,
        Row::EnemyDamage,
        Row::Horde,
        Row::StartRank,
    ];

    fn name(self) -> &'static str {
        match self {
            Row::Experience => "Experience gain",
            Row::HeroHealth => "Hero health",
            Row::EnemyHealth => "Enemy health",
            Row::EnemyDamage => "Enemy damage",
            Row::Horde => "Horde size",
            Row::StartRank => "Spells start at rank",
        }
    }

    /// The key it is saved under.
    fn key(self) -> &'static str {
        match self {
            Row::Experience => "xp",
            Row::HeroHealth => "hero_health",
            Row::EnemyHealth => "enemy_health",
            Row::EnemyDamage => "enemy_damage",
            Row::Horde => "horde",
            Row::StartRank => "start_rank",
        }
    }
}

/// The test settings: multipliers, 1 for a normal run, and a starting rank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DevTuning {
    pub xp: f32,
    pub hero_health: f32,
    pub enemy_health: f32,
    pub enemy_damage: f32,
    pub horde: f32,
    pub start_rank: u8,
}

impl Default for DevTuning {
    fn default() -> Self {
        Self {
            xp: 1.0,
            hero_health: 1.0,
            enemy_health: 1.0,
            enemy_damage: 1.0,
            horde: 1.0,
            start_rank: 1,
        }
    }
}

impl DevTuning {
    fn get(&self, row: Row) -> f32 {
        match row {
            Row::Experience => self.xp,
            Row::HeroHealth => self.hero_health,
            Row::EnemyHealth => self.enemy_health,
            Row::EnemyDamage => self.enemy_damage,
            Row::Horde => self.horde,
            Row::StartRank => f32::from(self.start_rank),
        }
    }

    fn set(&mut self, row: Row, v: f32) {
        match row {
            Row::Experience => self.xp = v,
            Row::HeroHealth => self.hero_health = v,
            Row::EnemyHealth => self.enemy_health = v,
            Row::EnemyDamage => self.enemy_damage = v,
            Row::Horde => self.horde = v,
            Row::StartRank => self.start_rank = (v.round() as u8).clamp(1, MAX_RANK),
        }
    }

    /// `row` moved `by` steps: a rank by one, a multiplier along [`STEPS`].
    fn step(&mut self, row: Row, by: i32) {
        let v = self.get(row);
        let next = if row == Row::StartRank {
            v + by as f32
        } else {
            let at = STEPS
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs()))
                .map_or(3, |(i, _)| i) as i32;
            STEPS[(at + by).clamp(0, STEPS.len() as i32 - 1) as usize]
        };
        self.set(row, next);
    }

    /// The settings on a new run: faster experience, tougher or weaker hero and horde, and its
    /// heroes' spells raised to the starting rank (a test loadout's higher ranks are kept).
    pub fn apply(&self, world: &mut World) {
        world.xp_rate *= self.xp;
        world.learn_rank = self.start_rank;
        world.difficulty.enemy_health += self.enemy_health - 1.0;
        world.difficulty.enemy_damage += self.enemy_damage - 1.0;
        world.difficulty.horde += self.horde - 1.0;
        for hero in &mut world.heroes {
            hero.stats.add(Stat::Health, self.hero_health - 1.0);
            hero.max_hp *= self.hero_health;
            hero.hp = hero.max_hp;
            for o in &mut hero.abilities {
                o.level = o.level.max(self.start_rank);
            }
        }
    }
}

/// The dev panel's state: whether it is open, and the settings.
#[derive(Resource)]
pub(super) struct DevSettings {
    pub open: bool,
    pub tuning: DevTuning,
}

impl Default for DevSettings {
    fn default() -> Self {
        Self {
            // `TOGETHER_DEV_PANEL` opens it, for a test shot of the menu.
            open: std::env::var_os("TOGETHER_DEV_PANEL").is_some(),
            tuning: load().unwrap_or_default(),
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DevAction {
    /// Opens or closes the panel.
    Toggle,
    /// One setting a step down or up, by its index in [`Row::ALL`].
    Down(usize),
    Up(usize),
    /// Every setting back to normal.
    Reset,
}

/// The panel's root, shown while open.
#[derive(Component)]
pub(super) struct DevPanel;

/// One setting's line: its name and value.
#[derive(Component)]
pub(super) struct DevRow(usize);

fn path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("together-dev-run.txt"))
}

/// The saved settings, one `key=value` a line; a key left out keeps its normal value.
fn load() -> Option<DevTuning> {
    let text = crate::local_state::read_to_string(&path()?).ok()?;
    let mut t = DevTuning::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (Some(row), Ok(v)) = (
            Row::ALL.into_iter().find(|r| r.key() == key.trim()),
            value.trim().parse::<f32>(),
        ) else {
            continue;
        };
        t.set(row, v);
    }
    Some(t)
}

fn save(t: &DevTuning) {
    if let Some(path) = path() {
        let text: String = Row::ALL
            .into_iter()
            .map(|r| format!("{}={}\n", r.key(), t.get(r)))
            .collect();
        let _ = crate::local_state::write_atomic(&path, &text);
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
    const W: f32 = 360.0;
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
                for row in 0..Row::ALL.len() {
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
                        text: "Rank 5, 10 and 15 bring augments.",
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
            let by = if matches!(action, DevAction::Up(_)) {
                1
            } else {
                -1
            };
            dev.tuning.step(Row::ALL[row], by);
            save(&dev.tuning);
        }
        DevAction::Reset => {
            dev.tuning = DevTuning::default();
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
        let r = Row::ALL[row.0];
        let v = dev.tuning.get(r);
        let s = if r == Row::StartRank {
            format!("{}  {v}", r.name())
        } else {
            format!("{}  x{v}", r.name())
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipliers_step_along_the_scale_and_ranks_by_one() {
        let mut t = DevTuning::default();
        t.step(Row::Experience, 1);
        assert_eq!(t.xp, 1.5);
        t.step(Row::Experience, 100);
        assert_eq!(t.xp, 20.0);
        t.step(Row::StartRank, 4);
        assert_eq!(t.start_rank, 5);
        t.step(Row::StartRank, -10);
        assert_eq!(t.start_rank, 1);
    }
}
