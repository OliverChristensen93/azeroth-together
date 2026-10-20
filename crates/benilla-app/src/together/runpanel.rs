//! The run's panel under the minimap, in a frame like WoW's own: the clock, the gold and what the
//! next chest costs, the boss's approach as a bar, the affixes and power-ups in play as icons with
//! their tooltips ([`super::tooltip`]), and friends who have fallen. The damage meter sits under
//! it. Over the middle of the screen, a short warning comes and fades: a boss approaching or
//! arriving, an affix joining the run.

use bevy::prelude::*;
use together_sim::{chest_price, Affix, PowerUp, BOSS_EVERY};

use super::hud::HudTop;
use super::levelup::TextureIcon;
use super::tooltip::TipThing;
use super::{content, Phase, Sim};

/// The panel's width (the damage meter's), and how far down it starts, in the 768-unit canvas.
pub(super) const PANEL_W: f32 = 214.0;
pub(super) const PANEL_TOP: f32 = 158.0;
/// Where the damage meter starts, under the panel.
pub(super) const METER_TOP: f32 = 278.0;
const ICON: f32 = 18.0;
/// Seconds of warning before the boss comes, and how long a warning stays.
const BOSS_WARNING: f32 = 10.0;
const WARNING_SECS: f32 = 5.0;

const PANEL_BG: Color = Color::srgba(0.02, 0.025, 0.06, 0.88);
const FRAME: Color = Color::srgb(0.72, 0.6, 0.32);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const TEXT: Color = Color::srgb(0.92, 0.89, 0.8);
const DIM: Color = Color::srgb(0.66, 0.63, 0.58);
const RED: Color = Color::srgb(1.0, 0.35, 0.3);
const WARNING: Color = Color::srgb(1.0, 0.55, 0.2);

#[derive(Component)]
pub(super) struct RunClock;

#[derive(Component)]
pub(super) struct RunGold;

#[derive(Component)]
pub(super) struct RunChest;

#[derive(Component)]
pub(super) struct BossRow;

#[derive(Component)]
pub(super) struct BossName;

#[derive(Component)]
pub(super) struct BossFill;

#[derive(Component)]
pub(super) struct BossTime;

/// The row of affix and power-up icons.
#[derive(Component)]
pub(super) struct IconRow;

/// A power-up icon's seconds.
#[derive(Component)]
pub(super) struct BuffSecs(PowerUp);

#[derive(Component)]
pub(super) struct FallenText;

#[derive(Component)]
pub(super) struct WarningText;

#[derive(Resource)]
pub(super) struct RunFont(Handle<Font>);

/// What the panel last built its icon row from, and what it has warned about.
#[derive(Default)]
pub(super) struct Seen {
    icons: Option<(Vec<Affix>, Vec<PowerUp>)>,
    affixes: usize,
    boss_warned: f32,
    boss_here: bool,
    surrounded: bool,
    warning_until: f32,
}

fn text(font: &Handle<Font>, size: f32, color: Color) -> impl Bundle {
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

fn icon(name: &'static str, size: f32) -> impl Bundle {
    (
        ImageNode {
            color: Color::srgba(0.2, 0.2, 0.2, 1.0),
            ..default()
        },
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        TextureIcon(name),
    )
}

/// A power-up's icon.
fn buff_icon(p: PowerUp) -> &'static str {
    match p {
        PowerUp::Swiftness => "Ability_Rogue_Sprint",
        PowerUp::Berserk => "Racial_Troll_Berserk",
        PowerUp::Bloodlust => "Spell_Nature_BloodLust",
        PowerUp::DivineShield => "Spell_Holy_DivineIntervention",
        PowerUp::Fortune => "INV_Misc_Coin_02",
        PowerUp::Detonation => "Spell_Nature_WispSplode",
        PowerUp::Magnet => "Spell_Nature_Polymorph",
    }
}

/// An affix's icon.
fn affix_icon(a: Affix) -> &'static str {
    match a {
        Affix::Sanguine => "Spell_Shadow_LifeDrain02",
        Affix::Bolstering => "Ability_Warrior_BattleShout",
        Affix::Volcanic => "Spell_Fire_Volcano",
        Affix::Spiteful => "Spell_Shadow_RaiseDead",
    }
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let px = Val::Px;
    commands
        .spawn((
            HudTop,
            Node {
                position_type: PositionType::Absolute,
                right: px(6.0),
                top: px(PANEL_TOP),
                width: px(PANEL_W),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(px(8.0), px(6.0)),
                row_gap: px(4.0),
                border: UiRect::all(px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor::all(FRAME),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            // The clock (its tooltip has the kills and the danger), and the gold.
            p.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    text(&font, 18.0, GOLD),
                    RunClock,
                    TipThing::Run,
                    Interaction::default(),
                ));
                row.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: px(4.0),
                    ..default()
                })
                .with_children(|g| {
                    g.spawn(icon("INV_Misc_Coin_01", 14.0));
                    g.spawn((text(&font, 14.0, TEXT), RunGold));
                    g.spawn((
                        Text::new("?"),
                        TextFont {
                            font: font.clone(),
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(DIM),
                        Node {
                            margin: UiRect::left(px(6.0)),
                            ..default()
                        },
                        TipThing::Help,
                        Interaction::default(),
                    ));
                });
            });
            p.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: px(5.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(icon("INV_Box_01", 14.0));
                row.spawn((text(&font, 11.0, DIM), RunChest));
            });
            // The boss's approach.
            p.spawn((
                BossRow,
                Node {
                    align_items: AlignItems::Center,
                    column_gap: px(5.0),
                    ..default()
                },
            ))
            .with_children(|row| {
                row.spawn(icon("INV_Misc_Bone_HumanSkull_01", 16.0));
                row.spawn((text(&font, 11.0, TEXT), BossName));
                row.spawn((
                    Node {
                        flex_grow: 1.0,
                        height: px(7.0),
                        border: UiRect::all(px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.15, 0.05, 0.05, 0.9)),
                    BorderColor::all(Color::srgba(0.6, 0.5, 0.3, 0.8)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.7, 0.15, 0.1)),
                        BossFill,
                    ));
                });
                row.spawn((text(&font, 11.0, DIM), BossTime));
            });
            p.spawn((
                IconRow,
                Node {
                    column_gap: px(3.0),
                    row_gap: px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
            ));
            p.spawn((text(&font, 11.0, RED), FallenText));
        });
    // The warning over the middle of the screen: the only text without a frame, and it fades.
    commands
        .spawn((
            HudTop,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(24.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|w| {
            w.spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: 28.0,
                    ..default()
                },
                TextColor(WARNING),
                // A heavy shadow: it has no frame, and must read over grass and stone alike.
                TextShadow {
                    offset: Vec2::new(2.0, 2.0),
                    color: Color::srgba(0.0, 0.0, 0.0, 0.95),
                },
                TextLayout::new_with_justify(Justify::Center),
                WarningText,
            ));
        });
    commands.insert_resource(RunFont(font));
}

fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn set(t: &mut Text, s: String) {
    if t.0 != s {
        t.0 = s;
    }
}

/// The panel kept up to date, its icon row rebuilt when the affixes or power-ups change, and the
/// warnings raised and faded.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    time: Res<Time<Real>>,
    font: Option<Res<RunFont>>,
    mut texts: ParamSet<(
        Query<&mut Text, With<RunClock>>,
        Query<&mut Text, With<RunGold>>,
        Query<&mut Text, With<RunChest>>,
        Query<&mut Text, With<BossName>>,
        Query<&mut Text, With<BossTime>>,
        Query<&mut Text, With<FallenText>>,
        Query<(&mut Text, &mut TextColor), With<WarningText>>,
        Query<(&mut Text, &BuffSecs)>,
    )>,
    mut boss_row: Query<&mut Node, (With<BossRow>, Without<BossFill>, Without<FallenText>)>,
    mut boss_fill: Query<&mut Node, (With<BossFill>, Without<BossRow>, Without<FallenText>)>,
    mut fallen_line: Query<&mut Node, (With<FallenText>, Without<BossRow>, Without<BossFill>)>,
    icons: Query<Entity, With<IconRow>>,
    net: Res<super::session::Net>,
    mut seen: Local<Seen>,
) {
    let Some(sim) = sim.filter(|_| *phase.get() == Phase::Playing) else {
        *seen = Seen::default();
        return;
    };
    let world = &sim.world;
    let Some(hero) = world.hero(sim.me) else {
        return;
    };
    let content = content();
    let now = time.elapsed_secs();
    if let Ok(mut t) = texts.p0().single_mut() {
        set(&mut t, clock(world.elapsed));
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        set(&mut t, hero.gold.to_string());
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        // In co-op, the ping to the host beside it.
        let ping = net
            .ping_ms()
            .map_or(String::new(), |ms| format!("   ·   {ms} ms"));
        set(
            &mut t,
            format!("Next chest: {} gold{ping}", chest_price(hero.chests)),
        );
    }

    // The boss: its approach as a bar until it comes; then its own bar takes over.
    let boss_alive = world.enemies.iter().any(|e| e.boss);
    let boss = world
        .zone()
        .boss_ix
        .map(|b| content.enemies[b].name.clone());
    let left = world.next_boss - world.elapsed;
    let approaching = boss.is_some() && !boss_alive && left.is_finite() && left > 0.0;
    if let Ok(mut row) = boss_row.single_mut() {
        let want = if approaching {
            Display::Flex
        } else {
            Display::None
        };
        if row.display != want {
            row.display = want;
        }
    }
    let mut warning: Option<String> = None;
    if let Some(name) = boss.as_deref() {
        if approaching {
            // One timer: to the swarm halfway there, then to the boss.
            let swarm_left = world.next_surround - world.elapsed;
            let (label, left) = if swarm_left.is_finite() && swarm_left > 0.0 {
                ("Swarm in".to_string(), swarm_left)
            } else {
                (format!("{name} in"), left)
            };
            if let Ok(mut t) = texts.p3().single_mut() {
                set(&mut t, label);
            }
            if let Ok(mut t) = texts.p4().single_mut() {
                set(&mut t, clock(left));
            }
            if let Ok(mut fill) = boss_fill.single_mut() {
                let half = BOSS_EVERY * 0.5;
                fill.width = Val::Percent((1.0 - left / half).clamp(0.0, 1.0) * 100.0);
            }
            if left <= BOSS_WARNING && seen.boss_warned != world.next_boss {
                seen.boss_warned = world.next_boss;
                warning = Some(format!("{name} approaches"));
            }
        }
        if boss_alive && !seen.boss_here {
            warning = Some(format!("{name} is here"));
        }
    }
    seen.boss_here = boss_alive;
    if world.affixes.len() > seen.affixes {
        if let Some(a) = world.affixes.last() {
            warning = Some(format!("{}\n{}", a.name(), a.hint()));
        }
    }
    seen.affixes = world.affixes.len();
    let surrounded = world.surround > 0.0;
    if surrounded && !seen.surrounded {
        warning = Some("Surrounded!\nThey come from every side".into());
    }
    seen.surrounded = surrounded;
    {
        let mut q = texts.p6();
        if let Ok((mut t, mut color)) = q.single_mut() {
            if let Some(w) = warning {
                set(&mut t, w);
                seen.warning_until = now + WARNING_SECS;
            }
            // Held while the cards are up (it would sit under them), then fades over its last
            // second.
            if !hero.offers.is_empty() {
                seen.warning_until += time.delta_secs();
            }
            let hidden = !hero.offers.is_empty();
            let alpha = if hidden {
                0.0
            } else {
                (seen.warning_until - now).clamp(0.0, 1.0)
            };
            if alpha <= 0.0 && !hidden && !t.0.is_empty() {
                t.0.clear();
            }
            color.0 = WARNING.with_alpha(alpha);
        }
    }

    let fallen: Vec<String> = if hero.alive && !world.game_over() {
        world
            .heroes
            .iter()
            .filter(|h| !h.alive)
            .map(|h| format!("{} has fallen: stand beside them", h.name))
            .collect()
    } else {
        Vec::new()
    };
    if let Ok(mut t) = texts.p5().single_mut() {
        set(&mut t, fallen.join("\n"));
    }
    // An empty line takes no room.
    if let Ok(mut line) = fallen_line.single_mut() {
        let want = if fallen.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if line.display != want {
            line.display = want;
        }
    }

    // Affixes and power-ups as icons; power-ups count down in their corner.
    let buffs: Vec<PowerUp> = world.buffs.iter().map(|&(p, _)| p).collect();
    let key = (world.affixes.clone(), buffs.clone());
    if seen.icons.as_ref() != Some(&key) {
        if let (Ok(row), Some(font)) = (icons.single(), font.as_ref()) {
            commands.entity(row).despawn_related::<Children>();
            commands.entity(row).with_children(|r| {
                for &a in &world.affixes {
                    r.spawn((
                        icon(affix_icon(a), ICON),
                        TipThing::Affix(a),
                        Interaction::default(),
                        BorderColor::all(Color::srgb(0.8, 0.25, 0.2)),
                    ));
                }
                for &p in &buffs {
                    r.spawn((
                        icon(buff_icon(p), ICON),
                        TipThing::PowerUp(p),
                        Interaction::default(),
                    ))
                    .with_children(|i| {
                        i.spawn((
                            text(&font.0, 9.0, Color::WHITE),
                            BuffSecs(p),
                            Node {
                                position_type: PositionType::Absolute,
                                right: Val::Px(0.0),
                                bottom: Val::Px(-1.0),
                                ..default()
                            },
                        ));
                    });
                }
            });
            seen.icons = Some(key);
        }
    }
    for (mut t, secs) in &mut texts.p7() {
        if let Some(&(_, left)) = world.buffs.iter().find(|(p, _)| *p == secs.0) {
            set(&mut t, format!("{:.0}", left.ceil()));
        }
    }
}
