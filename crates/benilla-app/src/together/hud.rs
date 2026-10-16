//! A plain text HUD: the clock, the kills, the gold and the hero's spells (and, joined, the ping
//! to the host), the party's power-ups and the boss; fallen friends and how to raise them; a
//! carried banner's way to go, and what a shrine or chest nearby does and how
//! full its circle is; and the end of the run.

use bevy::prelude::*;

use together_sim::{
    chest_price, InteractState, Interactable, InteractableKind, REVIVE_RANGE, REVIVE_SECS,
};

use super::Sim;

#[derive(Component)]
pub(super) struct StatusText;

#[derive(Component)]
pub(super) struct CardsText;

#[derive(Component)]
pub(super) struct CenterPanel;

/// The run's status line and hint, shown only in a run.
#[derive(Component)]
pub(super) struct HudTop;

/// The prompt by a shrine or chest: its text, and the bar its circle fills.
#[derive(Component)]
pub(super) struct PromptPanel;

#[derive(Component)]
pub(super) struct PromptText;

#[derive(Component)]
pub(super) struct PromptFill;

/// How near an interactable's circle the prompt comes up, in yards past its edge.
const PROMPT_REACH: f32 = 8.0;

/// The run's HUD shows in a run; at the menu the status line and any panel are hidden.
#[allow(clippy::type_complexity)]
pub(super) fn show_hud(
    phase: Res<State<super::Phase>>,
    mut top: Query<&mut Visibility, (With<HudTop>, Without<CenterPanel>, Without<PromptPanel>)>,
    mut panel: Query<&mut Visibility, (Or<(With<CenterPanel>, With<PromptPanel>)>, Without<HudTop>)>,
) {
    let playing = *phase.get() == super::Phase::Playing;
    let want = if playing {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut top {
        if *v != want {
            *v = want;
        }
    }
    if !playing {
        for mut v in &mut panel {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
    }
}

fn text(font: &Handle<Font>, size: f32) -> (Text, TextFont, TextColor, TextShadow) {
    (
        Text::new(""),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.82, 0.0)),
        TextShadow::default(),
    )
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    // Top centre, clear of WoW's player and target frames (left) and minimap (right).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Px(3.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            HudTop,
        ))
        .with_children(|p| {
            p.spawn((
                text(&font, 12.0),
                TextLayout::new_with_justify(Justify::Center),
                StatusText,
            ));
            p.spawn((
                Text::new(
                    "WoW controls   ·   cards: 1 2 3 or click, R rerolls   ·   \
                     stand in a shrine's circle to use it",
                ),
                TextFont {
                    font: font.clone(),
                    font_size: 9.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                TextShadow::default(),
            ));
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                // High enough to leave the hero, in the middle of the screen, in view.
                top: Val::Percent(9.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            CenterPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            ))
            .with_children(|b| {
                b.spawn((
                    text(&font, 16.0),
                    TextLayout::new_with_justify(Justify::Center),
                    CardsText,
                ));
            });
        });
    // Low in the middle, over the action bar and clear of the hero.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                bottom: Val::Percent(22.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            PromptPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(5.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ))
            .with_children(|b| {
                b.spawn((
                    text(&font, 12.0),
                    TextLayout::new_with_justify(Justify::Center),
                    PromptText,
                ));
                b.spawn((
                    Node {
                        width: Val::Px(180.0),
                        height: Val::Px(7.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
                    BorderColor::all(Color::srgb(0.6, 0.5, 0.3)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.82, 0.0)),
                        PromptFill,
                    ));
                });
            });
        });
}

/// The interactable nearest the hero at `at` that is worth a prompt: one ready to use, or a
/// challenge still being fought, within [`PROMPT_REACH`] of its circle. Pots speak for themselves.
fn nearby(world: &together_sim::World, at: together_sim::V2) -> Option<&Interactable> {
    world
        .interactables
        .iter()
        .filter(|i| i.kind != InteractableKind::Pot)
        .filter(|i| i.ready() || matches!(i.state, InteractState::Challenge(_)))
        .filter(|i| i.pos.distance(at) <= i.kind.radius() + PROMPT_REACH)
        .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
}

/// What the prompt says, and how full its bar is: a banner being carried, wherever the hero is;
/// else the shrine or chest the hero stands near (its name, what it does, its price, and the bar
/// its circle fills), or a challenge's elites still standing.
fn prompt_text(world: &together_sim::World, me: together_sim::PlayerId) -> Option<(String, f32)> {
    let hero = world
        .hero(me)
        .filter(|h| h.alive && h.offers.is_empty() && !world.game_over())?;
    let carry = world.interactables.iter().find_map(|i| match i.state {
        InteractState::Escort { carrier, to, left } => Some((carrier, to, left)),
        _ => None,
    });
    if let Some((carrier, to, left)) = carry {
        let who = world.hero(carrier)?;
        let far = who.pos.distance(to).max(0.0);
        let s = if carrier == me {
            format!(
                "War Banner\nCarry it to the green circle  ·  {far:.0} yd  ·  {} left",
                clock(left)
            )
        } else {
            format!(
                "War Banner\n{} carries it  ·  {far:.0} yd  ·  {} left",
                who.name,
                clock(left)
            )
        };
        return Some((s, 0.0));
    }
    let it = nearby(world, hero.pos)?;
    let mut s = format!("{}\n{}", it.kind.name(), it.kind.description());
    if let InteractState::Challenge(left) = &it.state {
        s = format!("{}\n{} elites still stand", it.kind.name(), left.len());
    } else if it.kind == InteractableKind::Chest {
        let price = chest_price(hero.chests);
        s.push_str(&format!("\nCosts {price} gold  ·  you have {}", hero.gold));
    }
    if it.ready() && it.charge == 0.0 {
        s.push_str("\nStand in the circle");
    }
    Some((s, it.charge))
}

/// The prompt: [`prompt_text`], shown while it has something to say.
pub(super) fn prompt(
    sim: Res<Sim>,
    mut panel: Query<&mut Visibility, With<PromptPanel>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut fill: Query<&mut Node, With<PromptFill>>,
) {
    let shown = prompt_text(&sim.world, sim.me);
    if let Ok(mut vis) = panel.single_mut() {
        let want = if shown.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    let Some((s, charge)) = shown else {
        return;
    };
    if let Ok(mut t) = text.single_mut() {
        if t.0 != s {
            t.0 = s;
        }
    }
    if let Ok(mut node) = fill.single_mut() {
        let width = Val::Percent(charge * 100.0);
        if node.width != width {
            node.width = width;
        }
    }
}
fn clock(secs: f32) -> String {
    let s = secs as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The way `to` points, as a compass word (WoW's `x` is north, `y` west; the minimap is
/// north-up).
fn compass(to: together_sim::V2) -> &'static str {
    const WAYS: [&str; 8] = ["N", "NW", "W", "SW", "S", "SE", "E", "NE"];
    let turn = to.y.atan2(to.x) / std::f32::consts::FRAC_PI_4;
    WAYS[(turn.round() as i32).rem_euclid(8) as usize]
}

/// Extraction runs: the heat and what it pays, Goldshire when the hero is in it, and how each
/// exit stands and where it lies. `None` in the arena.
fn extraction_status(world: &together_sim::World, hero: &together_sim::Hero) -> Option<String> {
    let together_sim::Mode::Extraction(x) = &world.mode else {
        return None;
    };
    let def = &world.content().extractions[x.def];
    let heat = world.heat_of(hero.player);
    let skulls = def.skull_index(heat);
    let skull = &def.heat.skulls[skulls];
    let top = def.heat.skulls.len() - 1;
    let next = def.heat.skulls.get(skulls + 1).map_or(String::new(), |n| {
        let part = ((heat - skull.at) / (n.at - skull.at) * 100.0).clamp(0.0, 99.0);
        format!("   next {part:.0}%")
    });
    let mut s = format!(
        "\nHeat {skulls}/{top}   Rewards x{:.2}{next}   Night floor {:.0}",
        skull.reward, x.floor
    );
    if def.in_safe(hero.pos) {
        s.push_str("\nGoldshire: safe. Nothing new comes; heat cools while everyone is home.");
    }
    if let Some(zone) = def.hot_zone_at(hero.pos) {
        s.push_str(&format!("\n{}: dense, and the heat climbs faster here.", zone.name));
    }
    for (i, (exit, state)) in def.exits.iter().zip(&x.exits).enumerate() {
        let to = def.exit_pos(i) - hero.pos;
        let way = format!("{:.0} yd {}", to.length(), compass(to));
        let lands = state.lands_in(exit, world.elapsed);
        let mut line = if lands <= 0.0 {
            let left = (state.departs - world.elapsed).max(0.0);
            format!("{}: flight boarding, leaves in {}   {way}", exit.name, clock(left))
        } else {
            format!(
                "{}: flight lands in {}, leaves at {}   {way}",
                exit.name,
                clock(lands),
                clock(state.departs)
            )
        };
        if state.warned && !state.came {
            line.push_str("   AMBUSH COMING");
        }
        s.push('\n');
        s.push_str(&line);
    }
    Some(s)
}

/// Extraction runs: what the center panel says once the hero has left (`over`: the whole run
/// has ended), with `next` telling how to go on. `None` in the arena.
fn extraction_end(
    world: &together_sim::World,
    hero: &together_sim::Hero,
    over: bool,
    next: &str,
) -> Option<String> {
    let together_sim::Mode::Extraction(x) = &world.mode else {
        return None;
    };
    let def = &world.content().extractions[x.def];
    if !over {
        let out: Vec<String> = world
            .heroes
            .iter()
            .filter(|h| h.alive)
            .map(|h| format!("{} is still out there", h.name))
            .collect();
        return Some(format!("You got out!\n\n{}", out.join("\n")));
    }
    let title = if hero.extracted {
        "You got out!"
    } else {
        "You did not make it out."
    };
    let party: Vec<String> = world
        .heroes
        .iter()
        .map(|h| {
            let how = if h.extracted { "got out" } else { "fell" };
            format!("{}: {how}", h.name)
        })
        .collect();
    Some(format!(
        "{title}\n\n{}\n\n{}  ·  Level {}  ·  {} kills  ·  {} gold  ·  peak heat {}/{}\n\n{next}",
        party.join("\n"),
        clock(world.elapsed),
        world.level,
        world.kills,
        hero.gold,
        def.skull_index(x.peak),
        def.heat.skulls.len() - 1,
    ))
}

pub(super) fn refresh(
    sim: Res<Sim>,
    mut status: Query<&mut Text, (With<StatusText>, Without<CardsText>)>,
    mut cards: Query<&mut Text, (With<CardsText>, Without<StatusText>)>,
    mut panel: Query<&mut Visibility, With<CenterPanel>>,
    net: Res<super::session::Net>,
) {
    let world = &sim.world;
    let content = world.content();
    let Some(hero) = world.hero(sim.me) else {
        return;
    };
    if let Ok(mut t) = status.single_mut() {
        let owned: Vec<String> = hero
            .abilities
            .iter()
            .map(|o| format!("{} {}", content.abilities[o.ability].name, o.level))
            .collect();
        // Health, level and experience are in WoW's own player frame and XP bar.
        let ping = net
            .ping_ms()
            .map_or(String::new(), |ms| format!("   Ping {ms} ms"));
        let danger = world.difficulty.overall();
        let danger = if danger > 0.0 {
            format!("   Danger +{:.0}%", danger * 100.0)
        } else {
            String::new()
        };
        let mut s = format!(
            "{}   Kills {}   Gold {}{danger}{ping}\n{}",
            clock(world.elapsed),
            world.kills,
            hero.gold,
            owned.join("   "),
        );
        let bosses: Vec<String> = world
            .enemies
            .iter()
            .filter(|e| e.boss)
            .map(|e| {
                let name = &content.enemies[e.def].name;
                format!("{name} {:.0}%", e.hp / e.max_hp.max(1.0) * 100.0)
            })
            .collect();
        if !bosses.is_empty() {
            s.push_str(&format!("\nBOSS  {}", bosses.join("   ")));
        } else if let Some(boss) = world
            .zone()
            .boss_ix
            .filter(|_| world.extraction_settings().is_none_or(|s| s.timed_boss))
        {
            s.push_str(&format!(
                "\n{} comes in {}",
                content.enemies[boss].name,
                clock((world.next_boss - world.elapsed).max(0.0))
            ));
        }
        if !world.buffs.is_empty() {
            let buffs: Vec<String> = world
                .buffs
                .iter()
                .map(|(p, left)| format!("{} {:.0}s", p.name(), left.ceil()))
                .collect();
            s.push('\n');
            s.push_str(&buffs.join("   "));
        }
        let rules = &content.rules;
        if !hero.tomes.is_empty() {
            let tomes: Vec<String> = hero
                .tomes
                .iter()
                .map(|&(stat, rank)| {
                    let short = stat.tome().trim_start_matches("Tome of ");
                    let short = short.trim_start_matches("the ");
                    format!("{short} {rank}/{}", rules.tome_max_rank)
                })
                .collect();
            s.push_str(&format!(
                "\nTomes {}/{}:  {}",
                hero.tomes.len(),
                rules.tome_slots,
                tomes.join("   ")
            ));
        }
        if !hero.items.is_empty() {
            let held: Vec<String> = hero
                .items
                .iter()
                .map(|&(item, n)| {
                    format!("{} {n}/{}", item.name(), item.max_copies(rules.item_max_copies))
                })
                .collect();
            s.push('\n');
            s.push_str(&held.join("   "));
        }
        if let Some(lines) = extraction_status(world, hero) {
            s.push_str(&lines);
        }
        // Friends who have fallen, while the run goes on.
        if hero.alive && !world.game_over() {
            for h in world.heroes.iter().filter(|h| !h.alive && !h.extracted) {
                s.push_str(&format!(
                    "\n{} has fallen: stand beside them to raise them",
                    h.name
                ));
            }
        }
        if t.0 != s {
            t.0 = s;
        }
    }

    let center = if world.game_over() {
        let next = if net.is_client() {
            "The host takes everyone back to the menu"
        } else {
            "Press Enter to return to the menu"
        };
        extraction_end(world, hero, true, next).or_else(|| {
            let fallen = if world.heroes.len() > 1 {
                "The party has fallen."
            } else {
                "You have fallen."
            };
            Some(format!(
                "{fallen}\n\nSurvived {}  ·  Level {}  ·  {} kills  ·  {:.0} damage\n\n{next}",
                clock(world.elapsed),
                world.level,
                world.kills,
                hero.damage_done,
            ))
        })
    } else if hero.extracted {
        extraction_end(world, hero, false, "")
    } else if !hero.alive {
        let raising = if hero.revive > 0.0 {
            format!("Being raised…  {:.0}%", hero.revive * 100.0)
        } else {
            format!("A friend who stands beside you for {REVIVE_SECS:.0} seconds raises you.")
        };
        Some(format!("You have fallen.\n\n{raising}"))
    } else if !hero.offers.is_empty() {
        // The level-up screen ([`super::levelup`]) is up.
        None
    } else if let Some(down) = world
        .heroes
        .iter()
        .find(|h| !h.alive && h.revive > 0.0 && h.pos.distance(hero.pos) <= REVIVE_RANGE)
    {
        Some(format!(
            "Raising {}…  {:.0}%",
            down.name,
            down.revive * 100.0
        ))
    } else if world.paused() {
        Some("Waiting for the others to choose…".into())
    } else {
        None
    };
    if let Ok(mut vis) = panel.single_mut() {
        let want = if center.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    if let (Some(s), Ok(mut t)) = (center, cards.single_mut()) {
        if t.0 != s {
            t.0 = s;
        }
    }
}
