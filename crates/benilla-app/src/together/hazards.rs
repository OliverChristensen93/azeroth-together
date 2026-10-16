//! Elites' and bosses' mechanics on screen: each telegraph a purple rune on the ground (a row of
//! them along a charge's line) until it goes off, their spell visuals, the local hero thrown or
//! pulled where a mechanic sent it, and the boss bar across the top: name, health and one
//! sentence about what the boss does.

use bevy::prelude::*;
use together_sim::{Event, Shape, V2};

use super::{content, units, view::Views, Phase, Sim};

/// `G_RuneGroundPurple01.mdx`, and its radius at scale 1 in yards.
const RUNE: u32 = 465;
const RUNE_RADIUS: f32 = 2.8;
/// Object serials for telegraph runes: this plus the telegraph's id times 32, plus its rune.
const TELEGRAPH_SERIAL_BASE: u64 = 0x6000_0000;
/// Yards between the runes along a charge's line.
const LINE_STEP: f32 = 2.0;
/// The boss bar's width, in the 768-unit canvas.
const BAR_W: f32 = 360.0;

/// The step's mechanic events, passed on by [`super::view::apply_events`].
#[derive(Resource, Default)]
pub(super) struct Pending(pub Vec<Event>);

/// The runes of each telegraph on screen, by its id.
#[derive(Resource, Default)]
pub(super) struct Telegraphs(std::collections::HashMap<u32, Vec<Entity>>);

#[derive(Component)]
pub(super) struct BossBar;

#[derive(Component)]
pub(super) struct BossName;

#[derive(Component)]
pub(super) struct BossFill;

#[derive(Component)]
pub(super) struct BossHint;

/// The ground under `p`, at the terrain's height where there is terrain.
fn on_ground(world_point: &benilla_world::world_point::WorldPoint, p: V2, fallback_y: f32) -> Vec3 {
    let mut at = units::ground_to_bevy(p, fallback_y);
    if let Some(h) = world_point.terrain_height_under(at) {
        at.y = h;
    }
    at
}

/// Telegraphs drawn and cleared, enemies' casts played, and the local hero moved where a
/// mechanic threw it.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    mut pending: ResMut<Pending>,
    mut shown: ResMut<Telegraphs>,
    mut player: ResMut<crate::player::Player>,
    mut bodies: Query<&mut Transform>,
    world_point: benilla_world::world_point::WorldPoint,
    mut fx: units::Fx,
) {
    let fallback_y = sim.world.zone().center[2];
    for event in pending.0.drain(..) {
        match event {
            Event::Telegraph { id, pos, shape, .. } => {
                let (spots, scale): (Vec<V2>, f32) = match shape {
                    Shape::Circle { radius } => (vec![pos], radius / RUNE_RADIUS),
                    Shape::Line { to, width } => {
                        let len = pos.distance(to);
                        let n = ((len / LINE_STEP) as usize).clamp(1, 30);
                        let spots = (0..=n)
                            .map(|k| pos + (to - pos) * (k as f32 / n as f32))
                            .collect();
                        (spots, width * 0.5 / RUNE_RADIUS)
                    }
                };
                let runes = spots
                    .into_iter()
                    .enumerate()
                    .map(|(k, at)| {
                        units::spawn_object(
                            &mut commands,
                            TELEGRAPH_SERIAL_BASE + u64::from(id) * 32 + k as u64,
                            RUNE,
                            scale,
                            on_ground(&world_point, at, fallback_y) + Vec3::Y * 0.06,
                        )
                    })
                    .collect();
                shown.0.insert(id, runes);
            }
            Event::TelegraphEnd { id } => {
                for e in shown.0.remove(&id).into_iter().flatten() {
                    commands.entity(e).despawn();
                }
            }
            Event::EnemyCast {
                enemy,
                visual,
                targets,
                dest,
            } => {
                let Some(&from) = views.enemies.get(&enemy) else {
                    continue;
                };
                let hits: Vec<Entity> = targets
                    .iter()
                    .filter_map(|&t| match t {
                        together_sim::Target::Enemy(id) => views.enemies.get(&id).copied(),
                        together_sim::Target::Hero(p) => views.heroes.get(&p).copied(),
                    })
                    .collect();
                let dest = dest.map(|p| on_ground(&world_point, p, fallback_y));
                fx.cast(from, visual, &hits, dest);
            }
            Event::Shoved { hero, to } => {
                // Our own hero's place is ours to keep: a throw or pull moves it here, as a
                // knock-back from the server would.
                if hero != sim.me {
                    continue;
                }
                let Some(&body) = views.heroes.get(&hero) else {
                    continue;
                };
                if let Ok(mut tf) = bodies.get_mut(body) {
                    let at = on_ground(&world_point, to, tf.translation.y);
                    tf.translation = at;
                    player.pos = at;
                }
            }
            _ => {}
        }
    }
}

/// Every telegraph's runes gone, at the end of a run.
pub(super) fn clear(mut commands: Commands, mut shown: ResMut<Telegraphs>) {
    for e in shown.0.drain().flat_map(|(_, runes)| runes) {
        commands.entity(e).despawn();
    }
}

pub(super) fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let text = |size: f32, color: Color| {
        (
            Text::new(""),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
            TextShadow::default(),
            TextLayout::new_with_justify(Justify::Center),
        )
    };
    // Under the run's status line, top centre.
    commands
        .spawn((
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Px(44.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|b| {
            b.spawn((text(14.0, Color::srgb(1.0, 0.82, 0.0)), BossName));
            b.spawn((
                Node {
                    width: Val::Px(BAR_W),
                    height: Val::Px(12.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.0, 0.0, 0.85)),
                BorderColor::all(Color::srgb(0.7, 0.6, 0.35)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.75, 0.08, 0.05)),
                    BossFill,
                ));
            });
            b.spawn((
                text(10.0, Color::WHITE),
                Node {
                    max_width: Val::Px(BAR_W + 120.0),
                    ..default()
                },
                BossHint,
            ));
        });
}

/// The boss bar shown while a boss lives: its name, health and what it does.
#[allow(clippy::type_complexity)]
pub(super) fn boss_bar(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    mut bar: Query<&mut Visibility, With<BossBar>>,
    mut fill: Query<&mut Node, With<BossFill>>,
    mut name: Query<&mut Text, (With<BossName>, Without<BossHint>)>,
    mut hint: Query<&mut Text, (With<BossHint>, Without<BossName>)>,
) {
    let boss = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .and_then(|sim| sim.world.enemies.iter().find(|e| e.boss).cloned());
    let want = if boss.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut bar {
        if *v != want {
            *v = want;
        }
    }
    let Some(boss) = boss else {
        return;
    };
    let def = &content().enemies[boss.def];
    let share = (boss.hp / boss.max_hp.max(1.0)).clamp(0.0, 1.0);
    if let Ok(mut node) = fill.single_mut() {
        node.width = Val::Percent(share * 100.0);
    }
    let title = format!("{}  {:.0}%", def.name, share * 100.0);
    if let Ok(mut t) = name.single_mut() {
        if t.0 != title {
            t.0 = title;
        }
    }
    let sentence = if def.hint.is_empty() {
        def.mechanics
            .iter()
            .map(|m| m.hint())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        def.hint.clone()
    };
    if let Ok(mut t) = hint.single_mut() {
        if t.0 != sentence {
            t.0 = sentence;
        }
    }
}

/// The spells whose WoW state kits show an enemy's statuses, as its aura slots carry them:
/// Frost Nova's ice (Frozen), Frostbolt's chill (Chilled), Immolate's flames (Burning) and
/// Hammer of Justice's daze (Stunned).
const STATUS_AURAS: [(u8, u32); 4] = [
    (together_sim::status_bits::FROZEN, 122),
    (together_sim::status_bits::CHILLED, 116),
    (together_sim::status_bits::BURNING, 348),
    (together_sim::status_bits::STUNNED, 853),
];
/// `UNIT_FIELD_AURA` and `UNIT_FIELD_AURAFLAGS`, and the first of the harmful auras' slots.
const FIELD_AURA: u16 = 47;
const FIELD_AURAFLAGS: u16 = 95;
const FIRST_DEBUFF_SLOT: u16 = 32;
/// An occupied slot's flags nibble.
const AURA_ON: u32 = 0x0E;

/// Each enemy's statuses as auras on its unit, so the engine plays their WoW visuals: the ice
/// block of a Freeze, a Chill's tint, a burn's flames, a stun's daze.
pub(super) fn status_auras(
    sim: Res<Sim>,
    views: Res<Views>,
    mut stores: Query<&mut crate::net::ObjectStore>,
    mut shown: Local<std::collections::HashMap<u32, u8>>,
) {
    shown.retain(|id, _| views.enemies.contains_key(id));
    for enemy in &sim.world.enemies {
        let bits = enemy.status.bits();
        if shown.get(&enemy.id).copied().unwrap_or(0) == bits {
            continue;
        }
        let Some(mut store) = views
            .enemies
            .get(&enemy.id)
            .and_then(|&e| stores.get_mut(e).ok())
        else {
            continue;
        };
        let mut pairs = Vec::new();
        let mut flags = 0;
        for (k, &(bit, spell)) in STATUS_AURAS.iter().enumerate() {
            let on = bits & bit != 0;
            pairs.push((
                FIELD_AURA + FIRST_DEBUFF_SLOT + k as u16,
                if on { spell } else { 0 },
            ));
            if on {
                flags |= AURA_ON << (4 * k);
            }
        }
        // Slots 32 to 39 share one flags field, a nibble each.
        pairs.push((FIELD_AURAFLAGS + FIRST_DEBUFF_SLOT / 8, flags));
        store
            .0
            .merge(benilla_protocol::messages::ObjectFields::from_pairs(&pairs));
        shown.insert(enemy.id, bits);
    }
}
