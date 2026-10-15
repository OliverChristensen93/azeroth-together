//! A spell's tooltip in WoW's look: what it does now with every bonus applied, its augments, its
//! next milestone and its crit. It shows by the cursor over the action bar's spells (WoW's own
//! tooltip for them names the real 1.12 spell, so [`super::ui`] turns it off) and over the
//! level-up screen's row of spells, drawn above the level-up screen.

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use together_sim::{
    milestone_rank, AbilityKind, Augment, Cone, Milestone, Owned, Stat, Tag, Targeting, World,
    MAX_RANK,
};

use super::{Phase, Sim};

/// The tooltip's width, in the 768-unit canvas.
const WIDTH: f32 = 270.0;

/// Which action button the mouse is over: its paged id, 0 for none.
const HOVER_LUA: &str = r#"
local f = GetMouseFocus()
if f and f.GetName then
    local n = f:GetName()
    if n and string.find(n, "^ActionButton%d+$") then
        return ActionButton_GetPagedID(f)
    end
end
return 0
"#;

const WHITE: Color = Color::WHITE;
const GREY: Color = Color::srgb(0.55, 0.55, 0.55);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const GREEN: Color = Color::srgb(0.12, 1.0, 0.12);
const BLUE: Color = Color::srgb(0.45, 0.7, 1.0);

#[derive(Component)]
pub(super) struct TipRoot;

#[derive(Component)]
pub(super) struct TipBody;

/// A spell on the level-up screen's row: its index among the hero's spells.
#[derive(Component)]
pub(super) struct SpellSlot(pub usize);

#[derive(Resource)]
pub(super) struct TipFont(Handle<Font>);

/// One line: text on the left, perhaps more on the right.
#[derive(Clone, PartialEq)]
pub(super) struct Line {
    left: String,
    right: String,
    color: Color,
    right_color: Color,
    size: f32,
}

impl Line {
    fn one(text: impl Into<String>, color: Color) -> Line {
        Line {
            left: text.into(),
            right: String::new(),
            color,
            right_color: color,
            size: 11.0,
        }
    }

    fn two(
        left: impl Into<String>,
        right: impl Into<String>,
        color: Color,
        right_color: Color,
    ) -> Line {
        Line {
            left: left.into(),
            right: right.into(),
            color,
            right_color,
            size: 11.0,
        }
    }
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    commands
        .spawn((
            TipRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(WIDTH),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.07, 0.94)),
            BorderColor::all(Color::srgb(0.6, 0.6, 0.6)),
            GlobalZIndex(600),
            Visibility::Hidden,
        ))
        .with_children(|t| {
            t.spawn((
                TipBody,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ));
        });
    commands.insert_resource(TipFont(font));
}

/// The school a spell deals, for its sentence: Frostfire, Spellfire and Spellfrost when it has
/// two.
fn school(tags: together_sim::Tags) -> &'static str {
    let (fire, frost, arcane) = (
        tags.has(Tag::Fire),
        tags.has(Tag::Frost),
        tags.has(Tag::Arcane),
    );
    match (fire, frost, arcane) {
        (true, true, _) => "Frostfire",
        (true, false, true) => "Spellfire",
        (false, true, true) => "Spellfrost",
        _ => Tag::SCHOOLS
            .into_iter()
            .find(|&t| tags.has(t))
            .map_or("Physical", |t| t.name()),
    }
}

/// The tooltip of hero `h`'s spell `owned` in `world`.
fn lines(world: &World, h: usize, owned: &Owned) -> Vec<Line> {
    let content = world.content();
    let def = &content.abilities[owned.ability];
    let hero = &world.heroes[h];
    let s = world.spell_spec(h, owned);
    let mut out = vec![Line {
        size: 14.0,
        ..Line::two(
            def.name.clone(),
            format!("Rank {}", owned.level),
            WHITE,
            GREY,
        )
    }];
    let reach = match (def.kind, s.targeting) {
        (AbilityKind::Shield, _) | (_, Targeting::Caster) => "On you".to_string(),
        (_, Targeting::Around) => format!("{:.0} yd around you", s.radius),
        (_, Targeting::Ahead) => match s.cone {
            Cone::Ahead => format!("{:.0} yd cone ahead of you", s.radius),
            Cone::Behind => format!("{:.0} yd cone behind you", s.radius),
            Cone::Four => format!("{:.0} yd cones all around you", s.radius),
        },
        _ => format!("{:.0} yd range", s.range),
    };
    out.push(Line::one(reach, WHITE));
    out.push(Line::two(
        "Automatic",
        format!("{:.1} sec cooldown", s.cooldown),
        WHITE,
        WHITE,
    ));

    let dmg = s.damage.round();
    let school = school(s.tags);
    let mut what = match def.kind {
        AbilityKind::Bolt | AbilityKind::Strike => {
            let who = match (s.targeting, s.count) {
                (Targeting::ElitesFirst, 1) => "the nearest enemy".to_string(),
                (_, 1) => "the nearest enemy".to_string(),
                (_, n) => format!("the {n} nearest enemies"),
            };
            let times = if s.volley > 1 {
                format!(" {} times", s.volley)
            } else {
                String::new()
            };
            let mut t = format!(
                "Strikes {who} within {:.0} yd{times} for {dmg} {school} damage.",
                s.range
            );
            if s.targeting == Targeting::ElitesFirst {
                t.push_str(" Bosses and elites are targeted first.");
            }
            if s.radius > 0.0 {
                t.push_str(&format!(
                    " It bursts for half damage within {:.0} yd.",
                    s.radius
                ));
            }
            t
        }
        AbilityKind::Nova => format!("Hits every enemy in its area for {dmg} {school} damage."),
        AbilityKind::Ground => format!(
            "Deals {dmg} {school} damage a second to everything within {:.0} yd for {:.0} sec, on the thickest crowd in range.",
            s.radius, s.ground
        ),
        AbilityKind::Shield => format!(
            "Shields you for {:.0} damage ({:.0}% of your health) for up to {:.0} sec.",
            hero.max_hp * s.shield,
            s.shield * 100.0,
            def.duration
        ),
    };
    if s.chill > 0.0 {
        what.push_str(" Chills (40% slower for 3 sec).");
    }
    if s.freeze > 0.0 {
        what.push_str(&format!(" Freezes for {:.1} sec.", s.freeze));
    }
    if s.stun > 0.0 {
        what.push_str(&format!(" Stuns for {:.1} sec.", s.stun));
    }
    if s.knockback > 0.0 {
        what.push_str(" Knocks enemies back.");
    }
    if s.burn > 0.0 {
        what.push_str(&format!(
            " Leaves them Burning for {:.0}% more over 4 sec.",
            s.burn * s.dot_mult * 100.0
        ));
    }
    if def.kind == AbilityKind::Nova && s.ground > 0.0 {
        what.push_str(&format!(
            " Leaves the ground burning for {:.0} sec.",
            s.ground
        ));
    }
    out.push(Line::one(what, GOLD));

    let damage = (1.0 + owned.bonus) * (1.0 + hero.stats.get(Stat::Damage)) - 1.0;
    let cooldown = hero.stats.get(Stat::Cooldown);
    let mut bonuses = Vec::new();
    if damage > 0.005 {
        bonuses.push(format!("+{:.0}% damage", damage * 100.0));
    }
    if cooldown > 0.005 {
        bonuses.push(format!("-{:.0}% cooldown", cooldown * 100.0));
    }
    if !bonuses.is_empty() {
        out.push(Line::one(
            format!("Your bonuses: {}", bonuses.join(", ")),
            GREEN,
        ));
    }
    if !owned.augments.is_empty() {
        let names: Vec<&str> = owned.augments.iter().map(|a| a.name()).collect();
        out.push(Line::one(format!("Augments: {}", names.join(", ")), BLUE));
    }
    let capstones = !Augment::capstones_of(&def.key).is_empty();
    match owned.awaiting(capstones) {
        Some(Milestone::Capstone) => out.push(Line::one(
            "Its capstone waits: choose it on its screen",
            GREEN,
        )),
        Some(Milestone::General) => out.push(Line::one(
            "An augment waits: choose it on its screen",
            GREEN,
        )),
        None if owned.augments.len() < 3 && owned.level < MAX_RANK => {
            let n = owned.augments.len();
            let what = if n == 1 && capstones {
                "its capstone"
            } else {
                "an augment"
            };
            out.push(Line::one(
                format!("Rank {} brings {what}", milestone_rank(n)),
                GREY,
            ));
        }
        None => {}
    }
    if def.kind != AbilityKind::Shield {
        out.push(Line::one(
            format!(
                "{:.0}% chance to crit for {:.0}% damage",
                s.crit.min(1.0) * 100.0,
                s.crit_mult * 100.0
            ),
            WHITE,
        ));
    }
    let tags: Vec<&str> = s.tags.iter().map(|t| t.name()).collect();
    out.push(Line::one(tags.join("  ·  "), GREY));
    out
}

/// The spell under the mouse, by its index among the hero's spells: on the level-up screen's
/// row, or on WoW's action bar (whose buttons hold the spells in order from the first).
/// `TOGETHER_TIP_SLOT=<n>` holds button `n` for a test shot.
fn hovered(slots: &Query<(&SpellSlot, &Interaction)>, script: Option<&UiScript>) -> Option<usize> {
    if let Some((slot, _)) = slots
        .iter()
        .find(|(_, i)| matches!(i, Interaction::Hovered | Interaction::Pressed))
    {
        return Some(slot.0);
    }
    if let Some(n) = std::env::var("TOGETHER_TIP_SLOT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        return n.checked_sub(1);
    }
    let id = script?.eval::<i64>(HOVER_LUA).ok()?;
    (id >= 1).then(|| id as usize - 1)
}

/// Show the tooltip by the cursor while a spell is under it, rebuilt when its text changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn show_tip(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    script: Option<NonSend<UiScript>>,
    slots: Query<(&SpellSlot, &Interaction)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    font: Option<Res<TipFont>>,
    mut root: Query<(&mut Node, &mut Visibility), With<TipRoot>>,
    body: Query<Entity, With<TipBody>>,
    mut shown: Local<Vec<Line>>,
) {
    let Ok((mut node, mut vis)) = root.single_mut() else {
        return;
    };
    let tip = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .and_then(|sim| {
            let slot = hovered(&slots, script.as_deref())?;
            let h = sim.world.heroes.iter().position(|x| x.player == sim.me)?;
            let owned = sim.world.heroes[h].abilities.get(slot)?;
            Some(lines(&sim.world, h, owned))
        });
    let Some(tip) = tip else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        shown.clear();
        return;
    };
    let (Ok(window), Some(font)) = (windows.single(), font) else {
        return;
    };
    let s = scale.0.max(0.01);
    let (w, h) = (window.width() / s, window.height() / s);
    let cursor = window
        .cursor_position()
        .map_or(Vec2::new(w * 0.5, h * 0.8), |c| c / s);
    // Above and to the right of the cursor, flipped to the left near the right edge.
    let left = if cursor.x + 18.0 + WIDTH > w {
        cursor.x - 18.0 - WIDTH
    } else {
        cursor.x + 18.0
    };
    node.left = Val::Px(left.max(0.0));
    node.bottom = Val::Px((h - cursor.y + 14.0).min(h - 40.0));
    if *vis != Visibility::Inherited {
        *vis = Visibility::Inherited;
    }
    if *shown == tip {
        return;
    }
    let Ok(body) = body.single() else {
        return;
    };
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        for line in &tip {
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
                            font_size: line.size.min(12.0),
                            ..default()
                        },
                        TextColor(line.right_color),
                        TextShadow::default(),
                    ));
                }
            });
        }
    });
    *shown = tip;
}
