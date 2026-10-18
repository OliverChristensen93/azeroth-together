//! Cooldowns worth watching. The movement ability's button (F): its icon, its key and how long
//! until it is ready, above the left end of the action bar. And a cooldown manager under the
//! hero: the spells whose reach depends on where you stand (cones, and novas around you) with a
//! long enough cooldown to plan around ([`min_cooldown`], 4 s by default), so you can line them
//! up on the horde. Spells cast every second or two are left off: they would only flicker.

use bevy::prelude::*;
use together_sim::{AbilityKind, Owned, Tag, Targeting};

use super::menu::SpellIcon;
use super::{Phase, Sim};

const ICON: f32 = 34.0;
const SMALL: f32 = 28.0;
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);

/// The shortest base cooldown the manager shows: `TOGETHER_CD_MIN` seconds, else 4.
fn min_cooldown() -> f32 {
    static MIN: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *MIN.get_or_init(|| {
        std::env::var("TOGETHER_CD_MIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4.0)
    })
}

#[derive(Component)]
pub(super) struct DefenseRoot;

#[derive(Component)]
pub(super) struct DefenseIcon;

/// A cooldown's shade (its height the share left) and its seconds, on the defensive button or a
/// manager slot (`Some(ability index)`).
#[derive(Component)]
pub(super) struct Shade(Option<usize>);

#[derive(Component)]
pub(super) struct Secs(Option<usize>);

#[derive(Component)]
pub(super) struct ManagerRow;

#[derive(Resource)]
pub(super) struct CdFont(Handle<Font>);

fn secs_text(font: &Handle<Font>, size: f32) -> impl Bundle {
    (
        Text::new(""),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow::default(),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Percent(28.0),
            ..default()
        },
    )
}

fn shade() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            height: Val::Percent(0.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
    )
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands.insert_resource(CdFont(font.clone()));
    // The movement ability: above the action bar's left end.
    commands
        .spawn((
            DefenseRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-330.0)),
                bottom: Val::Px(96.0),
                width: Val::Px(ICON),
                height: Val::Px(ICON),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(Color::BLACK),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            b.spawn((
                ImageNode::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                DefenseIcon,
            ));
            b.spawn((shade(), Shade(None)));
            b.spawn((secs_text(&font, 13.0), Secs(None)));
            // The key, in the corner, as the action bar shows its hotkeys.
            b.spawn((
                Text::new("F"),
                TextFont {
                    font: font.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                TextShadow::default(),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(2.0),
                    top: Val::Px(0.0),
                    ..default()
                },
            ));
        });
    // The manager: under the hero, centred.
    commands.spawn((
        ManagerRow,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Percent(60.0),
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(4.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Whether hero's spell `def` is one the manager watches.
fn watched(def: &together_sim::AbilityDef) -> bool {
    let positional = def.tag_set.has(Tag::Cone)
        || matches!(def.targeting(), Targeting::Around | Targeting::Ahead)
        || (def.kind == AbilityKind::Nova && def.target.is_none());
    positional && def.cooldown > min_cooldown()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    font: Option<Res<CdFont>>,
    mut defense: Query<&mut Visibility, (With<DefenseRoot>, Without<ManagerRow>)>,
    mut defense_icon: Query<(Entity, Option<&SpellIcon>), With<DefenseIcon>>,
    mut row: Query<(Entity, &mut Visibility), (With<ManagerRow>, Without<DefenseRoot>)>,
    mut shades: Query<(&mut Node, &Shade)>,
    mut secs: Query<(&mut Text, &Secs)>,
    mut built: Local<Vec<usize>>,
) {
    let hero = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .and_then(|s| {
            s.world
                .heroes
                .iter()
                .position(|h| h.player == s.me)
                .map(|h| (s, h))
        });
    let show = |v: &mut Visibility, on: bool| {
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    };
    let Some((sim, h)) = hero else {
        for mut v in &mut defense {
            show(&mut v, false);
        }
        for (_, mut v) in &mut row {
            show(&mut v, false);
        }
        built.clear();
        return;
    };
    let world = &sim.world;
    let hero = &world.heroes[h];
    let content = world.content();

    // The movement ability.
    let defensive = content.classes[hero.class].defensive_ix;
    for mut v in &mut defense {
        show(&mut v, defensive.is_some() && hero.alive);
    }
    let mut left = [(None, 0.0f32, 1.0f32)].to_vec();
    if let Some(d) = defensive {
        let visual = content.abilities[d].visual;
        if let Ok((e, icon)) = defense_icon.single_mut() {
            if icon.map(|i| i.0) != Some(visual) {
                commands
                    .entity(e)
                    .remove::<super::menu::IconDone>()
                    .insert(SpellIcon(visual));
            }
        }
        let total = world.spell_spec(h, &Owned::new(d, 0.0)).cooldown.max(0.1);
        left[0] = (None, hero.spell.fx.defense_cd.max(0.0), total);
    }

    // The manager's spells, rebuilt when the set changes.
    let watch: Vec<usize> = hero
        .abilities
        .iter()
        .enumerate()
        .filter(|(_, o)| watched(&content.abilities[o.ability]))
        .map(|(i, _)| i)
        .collect();
    for (row, mut v) in &mut row {
        show(&mut v, !watch.is_empty() && hero.alive);
        if *built != watch {
            if let Some(font) = font.as_ref() {
                commands.entity(row).despawn_related::<Children>();
                commands.entity(row).with_children(|r| {
                    for &i in &watch {
                        let visual = content.abilities[hero.abilities[i].ability].visual;
                        r.spawn((
                            Node {
                                width: Val::Px(SMALL),
                                height: Val::Px(SMALL),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.5, 0.5, 0.5)),
                            BackgroundColor(Color::BLACK),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                ImageNode::default(),
                                SpellIcon(visual),
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                            ));
                            b.spawn((shade(), Shade(Some(i))));
                            b.spawn((secs_text(&font.0, 11.0), Secs(Some(i))));
                        });
                    }
                });
                *built = watch.clone();
            }
        }
    }
    for &i in &watch {
        let owned = &hero.abilities[i];
        let total = world.spell_spec(h, owned).cooldown.max(0.1);
        left.push((Some(i), owned.cd.max(0.0), total));
    }
    for (mut node, which) in &mut shades {
        if let Some(&(_, l, t)) = left.iter().find(|(k, ..)| *k == which.0) {
            let pct = Val::Percent((l / t).clamp(0.0, 1.0) * 100.0);
            if node.height != pct {
                node.height = pct;
            }
        }
    }
    for (mut text, which) in &mut secs {
        if let Some(&(_, l, _)) = left.iter().find(|(k, ..)| *k == which.0) {
            let s = if l > 0.05 {
                if l < 10.0 {
                    format!("{l:.1}")
                } else {
                    format!("{:.0}", l.ceil())
                }
            } else {
                String::new()
            };
            if text.0 != s {
                text.0 = s;
            }
        }
    }
}
