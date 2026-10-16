//! A spell's tooltip in WoW's look: what it does now with every bonus applied, what its upgrades
//! add up to, its augments, its next milestone and its crit. It shows by the cursor over the
//! action bar's spells (WoW's own tooltip for them names the real 1.12 spell, so [`super::ui`]
//! turns it off), over the level-up screen's row of spells and over a spell card's icon, drawn
//! above the level-up screen. Holding Shift adds each augment's description.

use benilla_ui::script::UiScript;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use together_sim::{
    milestone_rank, AbilityKind, Augment, Cone, Item, Milestone, Owned, Stat, Tag, Targeting,
    World, MAX_RANK,
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

/// A spell card's icon: the ability it learns or upgrades.
#[derive(Component)]
pub(super) struct TipAbility(pub usize);

/// A tome or an item in the inventory strip ([`super::inventory`]).
#[derive(Component, Clone, Copy)]
pub(super) enum TipThing {
    Tome(Stat),
    Item(Item),
}

/// What the mouse is over.
enum Hover {
    /// A tome or an item the hero carries.
    Thing(TipThing),
    /// One of the hero's spells, by its index.
    Slot(usize),
    /// A card's spell, which the hero may not know yet.
    Ability(usize),
}

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

/// The tooltip of hero `h`'s spell `owned` in `world`; `extended` (Shift held) with each
/// augment's description.
fn lines(world: &World, h: usize, owned: &Owned, extended: bool) -> Vec<Line> {
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
    out.push(Line::two(
        "Automatic",
        format!("{:.1} sec cooldown", s.cooldown),
        WHITE,
        WHITE,
    ));
    out.push(Line::one(
        together_sim::describe(def, &s, hero.max_hp),
        GOLD,
    ));

    // Every number it has, one a line.
    let stat = |name: &str, value: String| Line::two(name, value, GREY, WHITE);
    let shield = def.kind == AbilityKind::Shield;
    // A ward or a buff works on the hero: no area of its own to show.
    let on_you = matches!(def.kind, AbilityKind::Ward | AbilityKind::Buff);
    let school = school(s.tags);
    let dmg = s.damage.round();
    let tick = 1.0 / s.tick_rate.max(0.1);
    out.push(match def.kind {
        AbilityKind::Shield => stat(
            "Shield",
            format!(
                "{:.0} ({:.0}% of health)",
                hero.max_hp * s.shield,
                s.shield * 100.0
            ),
        ),
        AbilityKind::Ground => stat("Damage", format!("{dmg} {school} every {tick:.1} sec")),
        AbilityKind::Buff if s.damage <= 0.0 => stat("Lasts", format!("{:.0} sec", s.fx.buff.secs)),
        _ => stat("Damage", format!("{dmg} {school}")),
    });
    if s.charges > 1 {
        out.push(stat("Charges", s.charges.to_string()));
    }
    let picks = matches!(
        s.targeting,
        Targeting::Nearest | Targeting::ElitesFirst | Targeting::Densest | Targeting::Weakest
    );
    if picks && !shield {
        out.push(stat("Range", format!("{:.0} yd", s.range)));
    }
    if matches!(def.kind, AbilityKind::Bolt | AbilityKind::Strike) {
        let who = match s.targeting {
            Targeting::ElitesFirst => "bosses and elites first",
            Targeting::Weakest => "the most wounded",
            _ => "the nearest",
        };
        out.push(stat("Targets", format!("{}, {who}", s.count)));
        if s.volley > 1 {
            out.push(stat("Missiles", format!("{} a target", s.volley)));
        }
        if s.pierce > 0 {
            out.push(stat("Pierces", format!("{} enemies", s.pierce)));
        }
        if s.radius > 0.0 {
            out.push(stat("Splash", format!("{:.0} yd, half damage", s.radius)));
        }
    } else if !shield && !on_you {
        let (name, value) = match (s.targeting, s.cone) {
            (Targeting::Ahead, Cone::Ahead) => ("Cone", format!("{:.0} yd ahead", s.radius)),
            (Targeting::Ahead, Cone::Behind) => ("Cone", format!("{:.0} yd behind", s.radius)),
            (Targeting::Ahead, Cone::Four) => ("Cones", format!("{:.0} yd, four ways", s.radius)),
            _ if s.ring > 0.0 => ("Ring", format!("{:.0} to {:.0} yd", s.ring, s.radius)),
            (Targeting::Around | Targeting::Caster, _) => {
                ("Radius", format!("{:.0} yd around you", s.radius))
            }
            _ => ("Radius", format!("{:.0} yd", s.radius)),
        };
        out.push(stat(name, value));
    }
    if def.kind == AbilityKind::Ground {
        out.push(stat("Duration", format!("{:.1} sec", s.ground)));
    }
    if def.kind == AbilityKind::Nova && s.ground > 0.0 {
        out.push(stat("Burning ground", format!("{:.1} sec", s.ground)));
    }
    if shield {
        out.push(stat("Duration", format!("up to {:.0} sec", def.duration)));
    }
    if s.burn > 0.0 {
        let secs = s.fx.dot_time * s.dot_secs / s.dot_speed.max(0.1);
        let total = s.burn * s.dot_mult * s.dot_secs * s.tick_rate * 100.0;
        out.push(stat(
            "Burning",
            format!("{total:.0}% of the hit over {secs:.1} sec"),
        ));
        let every = tick / s.dot_speed.max(0.1);
        out.push(stat("Burn ticks", format!("every {every:.1} sec")));
    }
    if s.chill > 0.0 {
        out.push(stat(
            "Chill",
            format!("{:.0}% slower for {:.0} sec", s.slow * 100.0, s.chill),
        ));
    }
    if s.freeze > 0.0 {
        out.push(stat("Freeze", format!("{:.1} sec", s.freeze)));
    }
    if s.freeze_chance > 0.0 {
        out.push(stat(
            "Freeze chance",
            format!("{:.0}% a hit", s.freeze_chance * 100.0),
        ));
    }
    if s.stun > 0.0 {
        out.push(stat("Stun", format!("{:.1} sec", s.stun)));
    }
    if s.knockback > 0.0 {
        out.push(stat("Knockback", "yes".to_string()));
    }
    // The other classes' effects.
    let fx = &s.fx;
    if fx.ward.is_some() {
        out.push(stat("Charges", fx.bank.to_string()));
    }
    if fx.heal > 0.0 {
        out.push(stat(
            "Heal",
            format!(
                "{:.0} ({:.0}% of health)",
                hero.max_hp * fx.heal,
                fx.heal * 100.0
            ),
        ));
    }
    if fx.leech > 0.0 {
        out.push(stat("Lifesteal", format!("{:.0}%", fx.leech * 100.0)));
    }
    if fx.fear > 0.0 {
        out.push(stat("Fear", format!("{:.1} sec", fx.fear)));
    }
    if fx.root > 0.0 {
        out.push(stat("Root", format!("{:.1} sec", fx.root)));
    }
    if fx.weaken > 0.0 {
        out.push(stat("Weakens", format!("{:.0} sec", fx.weaken)));
    }
    if fx.mark > 0.0 {
        out.push(stat(
            "Marks",
            format!("+{:.0}% damage taken", fx.mark * 100.0),
        ));
    }
    if fx.jumps > 0 {
        out.push(stat("Jumps", fx.jumps.to_string()));
    }
    if fx.stacks > 1 {
        out.push(stat("Stacks", format!("up to {}", fx.stacks)));
    }
    if !shield {
        out.push(stat(
            "Critical strike",
            format!(
                "{:.0}% for {:.0}% damage",
                s.crit.min(1.0) * 100.0,
                s.crit_mult * 100.0
            ),
        ));
    }

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
    let upgrades: Vec<String> = owned
        .mods
        .totals()
        .into_iter()
        .map(|(b, v)| b.describe(v))
        .collect();
    if !upgrades.is_empty() {
        out.push(Line::one(
            format!("Upgrades: {}", upgrades.join(", ")),
            GREEN,
        ));
    }
    if !owned.augments.is_empty() {
        if extended {
            for a in &owned.augments {
                out.push(Line::one(a.name(), BLUE));
                out.push(Line::one(a.description(), WHITE));
            }
        } else {
            let names: Vec<&str> = owned.augments.iter().map(|a| a.name()).collect();
            out.push(Line::one(format!("Augments: {}", names.join(", ")), BLUE));
        }
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
    let tags: Vec<&str> = s.tags.iter().map(|t| t.name()).collect();
    out.push(Line::one(tags.join("  ·  "), GREY));
    if !owned.augments.is_empty() && !extended {
        out.push(Line::one("Hold Shift for its augments' details", GREY));
    }
    out
}

/// The tooltip of a tome or item hero `h` carries: what it does, and where it stands.
fn thing_lines(world: &World, h: usize, thing: TipThing) -> Vec<Line> {
    let hero = &world.heroes[h];
    let rules = &world.content().rules;
    match thing {
        TipThing::Tome(stat) => vec![
            Line {
                size: 14.0,
                ..Line::two(
                    stat.tome(),
                    format!("Rank {}/{}", hero.tome_rank(stat), rules.tome_max_rank),
                    WHITE,
                    GREY,
                )
            },
            Line::one(format!("Raises your {}.", stat.name().to_lowercase()), GOLD),
            Line::one(
                format!("In all: {}", stat.describe(hero.stats.get(stat))),
                GREEN,
            ),
        ],
        TipThing::Item(item) => {
            let n = hero
                .items
                .iter()
                .find(|(i, _)| *i == item)
                .map_or(0, |&(_, n)| n);
            vec![
                Line {
                    size: 14.0,
                    ..Line::two(
                        item.name(),
                        format!("{n}/{}", item.max_copies(rules.item_max_copies)),
                        WHITE,
                        GREY,
                    )
                },
                Line::one(format!("{}.", item.describe(n)), GOLD),
            ]
        }
    }
}

/// The spell under the mouse, by its index among the hero's spells: on the level-up screen's
/// row, or on WoW's action bar (whose buttons hold the spells in order from the first).
/// `TOGETHER_TIP_SLOT=<n>` holds button `n` for a test shot.
fn hovered(
    slots: &Query<(&SpellSlot, &Interaction)>,
    cards: &Query<(&TipAbility, &Interaction)>,
    things: &Query<(&TipThing, &Interaction)>,
    script: Option<&UiScript>,
) -> Option<Hover> {
    let over = |i: &Interaction| matches!(i, Interaction::Hovered | Interaction::Pressed);
    if let Some((slot, _)) = slots.iter().find(|(_, i)| over(i)) {
        return Some(Hover::Slot(slot.0));
    }
    if let Some((card, _)) = cards.iter().find(|(_, i)| over(i)) {
        return Some(Hover::Ability(card.0));
    }
    if let Some((thing, _)) = things.iter().find(|(_, i)| over(i)) {
        return Some(Hover::Thing(*thing));
    }
    if let Some(n) = std::env::var("TOGETHER_TIP_SLOT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        return n.checked_sub(1).map(Hover::Slot);
    }
    let id = script?.eval::<i64>(HOVER_LUA).ok()?;
    (id >= 1).then(|| Hover::Slot(id as usize - 1))
}

/// Show the tooltip by the cursor while a spell is under it, rebuilt when its text changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn show_tip(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    script: Option<NonSend<UiScript>>,
    slots: Query<(&SpellSlot, &Interaction)>,
    cards: Query<(&TipAbility, &Interaction)>,
    things: Query<(&TipThing, &Interaction)>,
    keys: Res<ButtonInput<KeyCode>>,
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
            let hover = hovered(&slots, &cards, &things, script.as_deref())?;
            let h = sim.world.heroes.iter().position(|x| x.player == sim.me)?;
            let known = &sim.world.heroes[h].abilities;
            // `TOGETHER_TIP_SHIFT` holds Shift for a test shot.
            let extended = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
                || std::env::var_os("TOGETHER_TIP_SHIFT").is_some();
            let owned = match hover {
                Hover::Slot(i) => known.get(i)?.clone(),
                Hover::Ability(a) => known
                    .iter()
                    .find(|o| o.ability == a)
                    .cloned()
                    .unwrap_or_else(|| Owned::new(a, 0.0)),
                Hover::Thing(thing) => return Some(thing_lines(&sim.world, h, thing)),
            };
            Some(lines(&sim.world, h, &owned, extended))
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
