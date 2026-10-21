//! Elites' and bosses' mechanics on screen: each telegraph a flat shape on the ground until it
//! goes off (a purple circle that fills in as the blow winds up, a purple strip along a charge's
//! path, a green circle for a thing to run to), their spell visuals, the local hero thrown or
//! pulled where a mechanic sent it, and the boss bar across the top: name, health and one
//! sentence about what the boss does.
//!
//! The shapes are plain meshes rather than WoW's ground runes: a rune's model takes longer to
//! appear than a telegraph lasts.

use bevy::prelude::*;
use together_sim::{Event, Shape, V2};

use super::{content, units, view::Views, Phase, Sim};

/// The boss bar's width, in the 768-unit canvas.
const BAR_W: f32 = 260.0;
/// How far above the ground a telegraph lies, so it is not lost in the grass, and its edge's
/// width in yards.
const LIFT: f32 = 0.12;
const EDGE: f32 = 0.3;

/// The step's mechanic events, passed on by [`super::view::apply_events`].
#[derive(Resource, Default)]
pub(super) struct Pending(pub Vec<Event>);

/// The shapes of each telegraph on screen, by its id.
#[derive(Resource, Default)]
pub(super) struct Telegraphs(std::collections::HashMap<u32, Vec<Entity>>);

/// A danger's fill, growing from its centre to its edge as the blow winds up.
#[derive(Component)]
pub(super) struct Filling {
    born: f32,
    secs: f32,
}

/// The telegraphs' materials: a danger's purple and a lure's green, each a faint fill and a
/// stronger edge. Unlit and blended, drawn over the ground.
pub(super) struct Looks {
    danger_fill: Handle<StandardMaterial>,
    danger_edge: Handle<StandardMaterial>,
    lure_fill: Handle<StandardMaterial>,
    lure_edge: Handle<StandardMaterial>,
    pool_fill: Handle<StandardMaterial>,
    pool_edge: Handle<StandardMaterial>,
}

impl Looks {
    fn new(materials: &mut Assets<StandardMaterial>) -> Self {
        let mut flat = |color: Color| {
            materials.add(StandardMaterial {
                base_color: color,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                depth_bias: 50.0,
                ..default()
            })
        };
        Self {
            danger_fill: flat(Color::srgba(0.72, 0.18, 0.95, 0.32)),
            danger_edge: flat(Color::srgba(0.85, 0.35, 1.0, 0.9)),
            lure_fill: flat(Color::srgba(0.3, 1.0, 0.35, 0.25)),
            lure_edge: flat(Color::srgba(0.45, 1.0, 0.5, 0.9)),
            pool_fill: flat(Color::srgba(0.85, 0.05, 0.08, 0.35)),
            pool_edge: flat(Color::srgba(1.0, 0.2, 0.2, 0.85)),
        }
    }
}

/// Which bar at the top: the boss's, or a rare's under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BarSlot {
    Boss = 0,
    Rare = 1,
}

#[derive(Component)]
pub(super) struct BossBar(BarSlot);

#[derive(Component)]
pub(super) struct BossName(BarSlot);

#[derive(Component)]
pub(super) struct BossFill(BarSlot);

#[derive(Component)]
pub(super) struct BossHint(BarSlot);

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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time<Real>>,
    mut looks: Local<Option<Looks>>,
) {
    let fallback_y = sim.world.zone().center[2];
    for event in pending.0.drain(..) {
        match event {
            Event::Telegraph {
                id,
                pos,
                shape,
                kind,
                secs,
            } => {
                let look = looks.get_or_insert_with(|| Looks::new(&mut materials));
                // A thing to run to is green; a danger, purple.
                // A Sanguine pool is red and, like a lure, there from the start.
                let pool = kind == together_sim::Mechanic::Sanguine;
                let lure = kind.is_lure() || pool;
                let (fill, edge) = if pool {
                    (look.pool_fill.clone(), look.pool_edge.clone())
                } else if lure {
                    (look.lure_fill.clone(), look.lure_edge.clone())
                } else {
                    (look.danger_fill.clone(), look.danger_edge.clone())
                };
                let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                let mut parts = Vec::new();
                match shape {
                    Shape::Circle { radius } => {
                        let at = on_ground(&world_point, pos, fallback_y) + Vec3::Y * LIFT;
                        let ring = Annulus::new((radius - EDGE).max(0.05), radius);
                        parts.push(
                            commands
                                .spawn((
                                    Mesh3d(meshes.add(ring)),
                                    MeshMaterial3d(edge),
                                    Transform::from_translation(at).with_rotation(flat),
                                ))
                                .id(),
                        );
                        let start = if lure { 1.0 } else { 0.05 };
                        let mut disc = commands.spawn((
                            Mesh3d(meshes.add(Circle::new(radius))),
                            MeshMaterial3d(fill),
                            Transform::from_translation(at - Vec3::Y * 0.01)
                                .with_rotation(flat)
                                .with_scale(Vec3::splat(start)),
                        ));
                        if !lure {
                            disc.insert(Filling {
                                born: time.elapsed_secs(),
                                secs: secs.max(0.1),
                            });
                        }
                        parts.push(disc.id());
                    }
                    Shape::Line { to, width } => {
                        let a = on_ground(&world_point, pos, fallback_y) + Vec3::Y * LIFT;
                        let b = on_ground(&world_point, to, fallback_y) + Vec3::Y * LIFT;
                        let d = b - a;
                        let len = Vec2::new(d.x, d.z).length().max(0.1);
                        let yaw = (-d.z).atan2(d.x);
                        let turn = Quat::from_rotation_y(yaw) * flat;
                        let mid = (a + b) * 0.5;
                        parts.push(
                            commands
                                .spawn((
                                    Mesh3d(meshes.add(Rectangle::new(len, width))),
                                    MeshMaterial3d(fill),
                                    Transform::from_translation(mid).with_rotation(turn),
                                ))
                                .id(),
                        );
                        parts.push(
                            commands
                                .spawn((
                                    Mesh3d(meshes.add(Rectangle::new(len, EDGE))),
                                    MeshMaterial3d(edge),
                                    Transform::from_translation(mid + Vec3::Y * 0.01)
                                        .with_rotation(turn),
                                ))
                                .id(),
                        );
                    }
                }
                shown.0.insert(id, parts);
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
                // An affix's blast has no enemy behind it: it plays from our hero, at its spot.
                let Some(&from) = views
                    .enemies
                    .get(&enemy)
                    .or_else(|| views.heroes.get(&sim.me))
                else {
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
            // Announced by the run panel's warning (`super::runpanel`).
            Event::AffixGained { .. } => {}
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

/// Every telegraph's shapes gone, at the end of a run.
pub(super) fn clear(mut commands: Commands, mut shown: ResMut<Telegraphs>) {
    for e in shown.0.drain().flat_map(|(_, parts)| parts) {
        commands.entity(e).despawn();
    }
}

/// Each danger's fill grows to its edge over its wind-up, so the moment it goes off is plain.
pub(super) fn grow_fills(time: Res<Time<Real>>, mut fills: Query<(&Filling, &mut Transform)>) {
    let now = time.elapsed_secs();
    for (fill, mut tf) in &mut fills {
        let t = ((now - fill.born) / fill.secs).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(0.05 + 0.95 * t);
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
    // Top centre, small: the boss's name and health, and under it a rare's in the fight. Hovered,
    // what it does drops down under it; clicked, it is our target.
    for (slot, fill_color, border) in [
        (
            BarSlot::Boss,
            Color::srgb(0.75, 0.08, 0.05),
            Color::srgb(0.72, 0.6, 0.32),
        ),
        (
            BarSlot::Rare,
            Color::srgb(0.45, 0.5, 0.8),
            Color::srgb(0.7, 0.72, 0.85),
        ),
    ] {
        commands
            .spawn((
                BossBar(slot),
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Px(4.0),
                    width: Val::Px(BAR_W + 16.0),
                    margin: UiRect::left(Val::Px(-(BAR_W + 16.0) / 2.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.025, 0.06, 0.82)),
                BorderColor::all(border),
                Visibility::Hidden,
            ))
            .with_children(|b| {
                b.spawn((text(12.0, Color::srgb(1.0, 0.82, 0.0)), BossName(slot)));
                b.spawn((
                    Node {
                        width: Val::Px(BAR_W),
                        height: Val::Px(9.0),
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
                        BackgroundColor(fill_color),
                        BossFill(slot),
                    ));
                });
                b.spawn((
                    text(10.0, Color::WHITE),
                    Node {
                        max_width: Val::Px(BAR_W + 4.0),
                        margin: UiRect::top(Val::Px(3.0)),
                        display: Display::None,
                        ..default()
                    },
                    BossHint(slot),
                ));
            });
    }
}

/// How near our hero a rare at rest must be for its bar to show.
const RARE_BAR_NEAR: f32 = 30.0;

/// The bars at the top: the boss's while a boss lives, and under it the nearest rare's that is in
/// the fight (hurt, or near our hero). Each shows its name and health; hovered, what it does;
/// clicked, it becomes our target.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn boss_bar(
    phase: Res<State<Phase>>,
    sim: Option<Res<Sim>>,
    views: Res<Views>,
    guids: Query<&crate::net::Guid>,
    mut select: crate::target::SelectCommit,
    mut bar: Query<(&BossBar, &mut Visibility, &Interaction, &ComputedNode, &mut Node)>,
    mut fill: Query<(&BossFill, &mut Node), (Without<BossHint>, Without<BossBar>)>,
    mut name: Query<(&BossName, &mut Text), Without<BossHint>>,
    mut hint: Query<
        (&BossHint, &mut Text, &mut Node),
        (Without<BossName>, Without<BossFill>, Without<BossBar>),
    >,
    mut was: Local<[Interaction; 2]>,
) {
    let world = (*phase.get() == Phase::Playing)
        .then_some(())
        .and(sim.as_ref())
        .map(|sim| (&sim.world, sim.me));
    let boss = world.and_then(|(w, _)| w.enemies.iter().find(|e| e.boss).cloned());
    let rare = world.and_then(|(w, me)| {
        let at = w.hero(me).map(|h| h.pos)?;
        w.enemies
            .iter()
            .filter(|e| w.guards.contains(&e.id))
            .filter(|e| e.hp < e.max_hp || e.pos.distance(at) <= RARE_BAR_NEAR)
            .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
            .cloned()
    });
    let shown = |slot: BarSlot| match slot {
        BarSlot::Boss => boss.as_ref(),
        BarSlot::Rare => rare.as_ref(),
    };
    // The rare's bar sits under the boss's when both are up.
    let boss_height = bar
        .iter()
        .find(|(s, ..)| s.0 == BarSlot::Boss)
        .map_or(0.0, |(.., c, _)| c.size().y * c.inverse_scale_factor());
    let mut hovered = [false; 2];
    let mut clicked = [false; 2];
    for (slot, mut v, interaction, _, mut node) in &mut bar {
        let i = slot.0 as usize;
        let want = if shown(slot.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
        if slot.0 == BarSlot::Rare {
            let top = if boss.is_some() {
                Val::Px(4.0 + boss_height + 3.0)
            } else {
                Val::Px(4.0)
            };
            if node.top != top {
                node.top = top;
            }
        }
        hovered[i] = *interaction != Interaction::None;
        clicked[i] = *interaction == Interaction::Pressed && was[i] != Interaction::Pressed;
        was[i] = *interaction;
    }
    for slot in [BarSlot::Boss, BarSlot::Rare] {
        let Some(e) = shown(slot) else {
            continue;
        };
        let i = slot as usize;
        let def = &content().enemies[e.def];
        if clicked[i] {
            // This very unit, by its guid, as clicking its body would: a name could pick another
            // of the same kind.
            let unit = views.enemies.get(&e.id).copied();
            if let Some((entity, guid)) = unit.and_then(|u| Some((u, guids.get(u).ok()?.0))) {
                select.commit(entity, guid);
            }
        }
        let share = (e.hp / e.max_hp.max(1.0)).clamp(0.0, 1.0);
        for (s, mut node) in &mut fill {
            if s.0 == slot {
                node.width = Val::Percent(share * 100.0);
            }
        }
        let title = match slot {
            BarSlot::Boss => format!("{}  {:.0}%", def.name, share * 100.0),
            BarSlot::Rare => format!("{} (rare)  {:.0}%", def.name, share * 100.0),
        };
        for (s, mut t) in &mut name {
            if s.0 == slot && t.0 != title {
                t.0 = title.clone();
            }
        }
        let sentence = super::bossintro::mechanics_text(def);
        for (s, mut t, mut node) in &mut hint {
            if s.0 != slot {
                continue;
            }
            if t.0 != sentence {
                t.0 = sentence.clone();
            }
            let display = if hovered[i] {
                Display::Flex
            } else {
                Display::None
            };
            if node.display != display {
                node.display = display;
            }
        }
    }
}

/// The spells whose WoW state kits show an enemy's control statuses, as its aura slots carry
/// them: Frost Nova's ice (Frozen), Frostbolt's chill (Chilled), Hammer of Justice's daze
/// (Stunned), Fear, Entangling Roots and Hunter's Mark.
const STATUS_AURAS: [(u16, u32); 6] = [
    (together_sim::status_bits::FROZEN, 122),
    (together_sim::status_bits::CHILLED, 116),
    (together_sim::status_bits::STUNNED, 853),
    (together_sim::status_bits::FEARED, 5782),
    (together_sim::status_bits::ROOTED, 339),
    (together_sim::status_bits::MARKED, 1130),
];
/// Stand-ins for damage over time whose spell is not known (a host without the `dots` list):
/// Immolate (Burning), Rend (Bleeding), Deadly Poison and Corruption (Afflicted).
const DOT_AURAS: [(u16, u32); 4] = [
    (together_sim::status_bits::BURNING, 348),
    (together_sim::status_bits::BLEEDING, 772),
    (together_sim::status_bits::POISONED, 2818),
    (together_sim::status_bits::AFFLICTED, 172),
];
/// `UNIT_FIELD_AURA` and `UNIT_FIELD_AURAFLAGS`, and the harmful auras' slots.
const FIELD_AURA: u16 = 47;
const FIELD_AURAFLAGS: u16 = 95;
const FIRST_DEBUFF_SLOT: u16 = 32;
const DEBUFF_SLOTS: usize = 16;
/// An occupied slot's flags nibble.
const AURA_ON: u32 = 0x0E;

/// Each enemy's statuses as auras on its unit, so the engine plays their WoW visuals (the ice
/// block of a Freeze, a Chill's tint, a stun's daze) and its target frame shows them: control
/// first, then each damage over time as the spell that left it (Shadow Word: Pain, not a
/// stand-in).
pub(super) fn status_auras(
    sim: Res<Sim>,
    views: Res<Views>,
    mut stores: Query<&mut crate::net::ObjectStore>,
    mut shown: Local<std::collections::HashMap<u32, (u16, Vec<u16>)>>,
) {
    shown.retain(|id, _| views.enemies.contains_key(id));
    for enemy in &sim.world.enemies {
        let now = (enemy.status.bits(), enemy.status.dots());
        if shown.get(&enemy.id) == Some(&now) {
            continue;
        }
        let Some(mut store) = views
            .enemies
            .get(&enemy.id)
            .and_then(|&e| stores.get_mut(e).ok())
        else {
            continue;
        };
        let (bits, dots) = &now;
        let mut spells: Vec<u32> = STATUS_AURAS
            .iter()
            .filter(|(bit, _)| bits & bit != 0)
            .map(|&(_, spell)| spell)
            .collect();
        let abilities = &content().abilities;
        for &a in dots {
            match abilities.get(usize::from(a)).map(|d| d.visual) {
                Some(v) if v != 0 && !spells.contains(&v) => spells.push(v),
                _ => {}
            }
        }
        if dots.is_empty() {
            spells.extend(
                DOT_AURAS
                    .iter()
                    .filter(|(bit, _)| bits & bit != 0)
                    .map(|&(_, spell)| spell),
            );
        }
        spells.truncate(DEBUFF_SLOTS);
        // Each flags field holds eight slots, a nibble each: 32 to 39, then 40 to 47.
        let mut pairs = Vec::new();
        let mut flags = [0u32; 2];
        for k in 0..DEBUFF_SLOTS {
            let spell = spells.get(k).copied().unwrap_or(0);
            pairs.push((FIELD_AURA + FIRST_DEBUFF_SLOT + k as u16, spell));
            if spell != 0 {
                flags[k / 8] |= AURA_ON << (4 * (k % 8));
            }
        }
        for (i, f) in flags.into_iter().enumerate() {
            pairs.push((FIELD_AURAFLAGS + FIRST_DEBUFF_SLOT / 8 + i as u16, f));
        }
        store
            .0
            .merge(benilla_protocol::messages::ObjectFields::from_pairs(&pairs));
        shown.insert(enemy.id, now);
    }
}

/// While the pointer is on one of our buttons (the boss and rare bars, the meter, the stat
/// sheet), it is over UI as WoW's own frames are, so a click there is not also a click on the
/// world behind, which would drop the target the bar just took.
pub(super) fn buttons_hold_pointer(
    buttons: Query<(&Interaction, &InheritedVisibility)>,
    mut over: ResMut<crate::ui_script::PointerOverUi>,
) {
    if buttons
        .iter()
        .any(|(i, v)| *i != Interaction::None && v.get())
    {
        over.0 = true;
    }
}
