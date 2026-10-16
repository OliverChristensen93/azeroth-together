//! An equipment piece's tooltip, in WoW's item look: its name in its quality's colour, its item
//! level, where it is worn and what it is, its stat lines, a unique's "Equip:" effect and its
//! flavour text, and a line on what a click does. It shows by the cursor over any gear icon, at
//! the menu's stash and in a run's gear bar, above everything else.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use together_sim::{describe_line, GearPiece, Slot, WeaponKind};

use super::{Phase, Sim};

/// The tooltip's width, in the 768-unit canvas.
const WIDTH: f32 = 280.0;

const WHITE: Color = Color::WHITE;
const GREY: Color = Color::srgb(0.6, 0.6, 0.6);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const GREEN: Color = Color::srgb(0.12, 1.0, 0.12);
const RED: Color = Color::srgb(1.0, 0.25, 0.25);

/// What a click on the icon does, for the tooltip's last line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TipHint {
    /// A stash piece at the menu: it is worn on a click, in place of `replaces` if any.
    Wear(Option<GearPiece>),
    /// A worn piece at the menu: a click takes it off.
    TakeOff,
    /// A piece the local hero brought into the run: on, or dormant with a price.
    Run,
}

/// A gear icon's tooltip.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(super) struct GearTip {
    pub piece: GearPiece,
    pub hint: TipHint,
}

#[derive(Component)]
pub(super) struct GearTipRoot;

#[derive(Component)]
pub(super) struct GearTipBody;

#[derive(Resource)]
pub(super) struct GearTipFont(Handle<Font>);

/// One line: text on the left, perhaps more on the right.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct Line {
    left: String,
    right: String,
    color: Color,
    size: f32,
}

impl Line {
    fn one(text: impl Into<String>, color: Color) -> Line {
        Line {
            left: text.into(),
            right: String::new(),
            color,
            size: 11.0,
        }
    }
}

fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Where it is worn and what it is: "Two-Hand" and "Staff", "Head" and "Cloth".
fn kind_line(p: &GearPiece) -> (String, String) {
    match p.weapon() {
        Some(WeaponKind::Shield) => ("Off Hand".into(), "Shield".into()),
        Some(WeaponKind::Orb) => ("Held In Off-hand".into(), String::new()),
        Some(w) => {
            let hand = if w.two_hand() {
                "Two-Hand"
            } else if p.slot == Slot::OffHand {
                "Off Hand"
            } else {
                "One-Hand"
            };
            let what = match w {
                WeaponKind::Staff => "Staff",
                WeaponKind::Greatsword => "Sword",
                WeaponKind::Sword => "Sword",
                WeaponKind::Mace => "Mace",
                WeaponKind::Axe => "Axe",
                WeaponKind::Dagger => "Dagger",
                WeaponKind::Shield | WeaponKind::Orb => "",
            };
            (hand.into(), what.into())
        }
        None if p.slot == Slot::Back => ("Back".into(), String::new()),
        None => (p.slot.name().into(), p.armor.name().into()),
    }
}

/// The tooltip's lines for `tip`; `run` is the local hero's state when in a run: whether the
/// piece is on, what it costs and the hero's gold.
fn lines(tip: &GearTip, run: Option<(bool, Option<u32>, u32)>) -> Vec<Line> {
    let p = &tip.piece;
    let mut out = vec![
        Line {
            size: 14.0,
            ..Line::one(p.name(), rgb(p.rarity.color()))
        },
        Line::one(format!("Item Level {}", p.item_level), GOLD),
    ];
    let (left, right) = kind_line(p);
    out.push(Line {
        right,
        ..Line::one(left, WHITE)
    });
    for (stat, v) in p.stats() {
        out.push(Line::one(describe_line(stat, v), WHITE));
    }
    if let Some(u) = p.unique {
        out.push(Line::one(format!("Equip: {}", u.effect()), GREEN));
        out.push(Line::one(format!("\"{}\"", u.flavor()), GOLD));
    }
    match tip.hint {
        TipHint::Wear(replaces) => {
            out.push(Line::one("Click to wear it.", GREEN));
            if let Some(r) = replaces {
                out.push(Line::one(format!("Replaces {}.", r.name()), GREY));
            }
        }
        TipHint::TakeOff => {
            out.push(Line::one("Worn. Click to take it off.", GREY));
            out.push(Line::one(
                "It starts every run dormant: switch it on with gold.",
                GREY,
            ));
        }
        TipHint::Run => match run {
            Some((true, _, _)) => out.push(Line::one("Switched on.", GREEN)),
            Some((false, Some(price), gold)) => {
                let color = if gold >= price { GREEN } else { RED };
                out.push(Line::one(
                    format!("Dormant. Click to switch it on: {price} gold (you have {gold})."),
                    color,
                ));
            }
            _ => {}
        },
    }
    out
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            GearTipRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(WIDTH),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.07, 0.94)),
            BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
            GlobalZIndex(650),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|t| {
            t.spawn((
                GearTipBody,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ));
        });
    commands.insert_resource(GearTipFont(font));
}

/// Show the tooltip by the cursor while a gear icon is under it, rebuilt when its text changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    tips: Query<(&GearTip, &Interaction, &InheritedVisibility)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    font: Option<Res<GearTipFont>>,
    mut root: Query<(&mut Node, &mut Visibility), With<GearTipRoot>>,
    body: Query<Entity, With<GearTipBody>>,
    mut shown: Local<Vec<Line>>,
) {
    let Ok((mut node, mut vis)) = root.single_mut() else {
        return;
    };
    let over = |i: &Interaction| matches!(i, Interaction::Hovered | Interaction::Pressed);
    let hovered = tips
        .iter()
        .find(|(_, i, v)| over(i) && v.get())
        .map(|(t, _, _)| *t);
    let Some(tip) = hovered else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        shown.clear();
        return;
    };
    let run = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .and_then(|sim| sim.world.hero(sim.me))
        .and_then(|hero| {
            let w = hero.gear.in_slot(tip.piece.slot)?;
            Some((w.on, hero.gear.price(tip.piece.slot), hero.gold))
        });
    let tip_lines = lines(&tip, run);
    let (Ok(window), Some(font)) = (windows.single(), font) else {
        return;
    };
    let s = scale.0.max(0.01);
    let (w, h) = (window.width() / s, window.height() / s);
    let cursor = window
        .cursor_position()
        .map_or(Vec2::new(w * 0.5, h * 0.5), |c| c / s);
    // Below and to the right of the cursor, flipped where it would leave the screen.
    let left = if cursor.x + 18.0 + WIDTH > w {
        cursor.x - 18.0 - WIDTH
    } else {
        cursor.x + 18.0
    };
    node.left = Val::Px(left.max(0.0));
    let tall = 40.0 + 15.0 * tip_lines.len() as f32;
    node.top = Val::Px((cursor.y + 16.0).min(h - tall).max(0.0));
    node.bottom = Val::Auto;
    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }
    if *shown == tip_lines {
        return;
    }
    let Ok(body) = body.single() else {
        return;
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        for line in &tip_lines {
            b.spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new(line.left.clone()),
                    TextFont {
                        font: font.0.clone(),
                        font_size: line.size,
                        ..default()
                    },
                    TextColor(line.color),
                    TextShadow::default(),
                    Node {
                        flex_shrink: 1.0,
                        ..default()
                    },
                ));
                if !line.right.is_empty() {
                    row.spawn((
                        Text::new(line.right.clone()),
                        TextFont {
                            font: font.0.clone(),
                            font_size: 11.0,
                            ..default()
                        },
                        TextColor(WHITE),
                        TextShadow::default(),
                    ));
                }
            });
        }
    });
    *shown = tip_lines;
}

#[cfg(test)]
mod tests {
    use super::*;
    use together_sim::{Armor, Rarity, Unique};

    #[test]
    fn a_unique_tooltip_names_its_effect_and_a_dormant_one_its_price() {
        let tip = GearTip {
            piece: GearPiece::unique(Unique::Atiesh, 40, 1),
            hint: TipHint::Run,
        };
        let l = lines(&tip, Some((false, Some(90), 20)));
        assert_eq!(l[0].left, "Atiesh, Greatstaff of the Guardian");
        assert_eq!(l[2].left, "Two-Hand");
        assert_eq!(l[2].right, "Staff");
        assert!(l
            .iter()
            .any(|x| x.left.starts_with("Equip: Arcane Explosion")));
        assert!(l.last().unwrap().left.contains("90 gold"));
        assert_eq!(l.last().unwrap().color, RED);
        let hood = GearPiece::new(Slot::Head, Rarity::Common, 10, Armor::Cloth, 1);
        let l = lines(
            &GearTip {
                piece: hood,
                hint: TipHint::Wear(None),
            },
            None,
        );
        assert_eq!((l[2].left.as_str(), l[2].right.as_str()), ("Head", "Cloth"));
    }
}
