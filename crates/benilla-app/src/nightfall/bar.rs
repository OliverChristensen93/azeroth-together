//! The action bar (spec §6.1): the hero's abilities on WoW's own bar as a server's
//! `SMSG_INITIAL_SPELLS` and `SMSG_ACTION_BUTTONS` would put them there (the main bar, and the bar
//! a stealthed rogue sees), and its presses as the host needs them. A press is WoW's own
//! `UseAction`, from the keys 1 to = or a click, taken out of the script's calls before the
//! engine's own cast path sees it ([`presses`]): it becomes a [`CastPress`] at the current target,
//! or at the ground under the mouse for an ability aimed at the ground; Stealth's button counts a
//! Stealth press instead. `NIGHTFALL_AUTOFIGHT=1` targets the nearest enemy, presses the hero's
//! attacks in turn and dodges telegraphs, for test runs.
//!
//! [`CastPress`]: nightfall_sim::CastPress

use std::sync::OnceLock;

use benilla_protocol::messages::{ActionButton, ACTION_KIND_SPELL};
use benilla_ui::script::UiScript;
use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use nightfall_sim::{AbilityDef, Aim, Effect, Hero, Moving, UnitRef, V3};

use super::run::{autoplay, LocalInput, Sim};
use super::units;
use crate::target::Selection;
use crate::ui_action::PlayerActions;

/// Yards between the cursor ray's samples as it looks for the ground.
const AIM_STEP: f32 = 0.25;
/// How far the cursor ray looks for the ground, yards.
const AIM_REACH: f32 = 120.0;
/// Seconds between the autofight's presses.
const AUTO_EVERY: f32 = 1.6;
/// Seconds between the autofight's dodges.
const AUTO_DODGE_EVERY: f32 = 0.5;

/// The ability at WoW action `action` on a class's bar.
pub(super) fn slot_ability(kit: &[(u8, usize)], action: u32) -> Option<usize> {
    kit.iter()
        .find(|&&(s, _)| u32::from(s) == action)
        .map(|&(_, a)| a)
}

/// The main bar's actions that strike an enemy and need no stealth, in bar order: what the
/// autofight presses. With `walking` (autoplay never stands still) the abilities that root their
/// caster are left out, as the host would refuse them "Can't do that while moving".
pub(super) fn attacks(kit: &[(u8, usize)], abilities: &[AbilityDef], walking: bool) -> Vec<u8> {
    kit.iter()
        .filter(|&&(s, a)| {
            let ability = &abilities[a];
            s <= 12
                && ability.aim == Aim::Enemy
                && !ability.needs.stealth
                && !(walking && ability.moving == Moving::Rooted)
        })
        .map(|&(s, _)| s)
        .collect()
}

/// The first point along the ray from `origin` the way `dir` goes, within `max` yards, at or
/// below the ground `height` gives (Bevy's coordinates, y up): where a cursor ray meets the
/// terrain.
pub(super) fn ground_hit(
    origin: Vec3,
    dir: Vec3,
    max: f32,
    height: impl Fn(Vec3) -> Option<f32>,
) -> Option<Vec3> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let mut t = 0.0;
    while t <= max {
        let p = origin + dir * t;
        if let Some(h) = height(p) {
            if p.y <= h {
                return Some(Vec3::new(p.x, h, p.z));
            }
        }
        t += AIM_STEP;
    }
    None
}

/// Where an ability aimed at the ground goes when the cursor finds none (a background test
/// window): four fifths of its range straight ahead.
pub(super) fn ahead(hero: &Hero, range: f32) -> V3 {
    V3::at(hero.pos + hero.facing * (range * 0.8), hero.z)
}

fn autofight_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_AUTOFIGHT").is_some())
}

/// The hero's abilities on WoW's bar, each at its action, and in the spellbook; written again only
/// when the hero's class changes (a new run as another class).
pub(super) fn fill(
    sim: Res<Sim>,
    mut shown: Local<Option<usize>>,
    mut actions: ResMut<PlayerActions>,
) {
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    if *shown == Some(hero.class) {
        return;
    }
    let content = sim.world.content();
    actions.buttons.clear();
    actions.spells.clear();
    for &(slot, ability) in &content.classes[hero.class].kit {
        let spell = content.abilities[ability].spell;
        let index = slot - 1;
        actions.buttons.insert(
            index,
            ActionButton {
                slot: index,
                action: spell,
                kind: ACTION_KIND_SPELL,
            },
        );
        actions.spells.insert(spell);
    }
    actions.dirty = true;
    *shown = Some(hero.class);
}

/// Where the mouse cursor points on the ground, in WoW's coordinates.
fn cursor_ground(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    world_point: &WorldPoint,
) -> Option<V3> {
    let cursor = windows.single().ok()?.cursor_position()?;
    let (camera, tf) = cameras.single().ok()?;
    let ray = camera.viewport_to_world(tf, cursor).ok()?;
    let p = ground_hit(ray.origin, *ray.direction, AIM_REACH, |p| {
        world_point.terrain_height_under(p)
    })?;
    Some(V3::new(-p.z, -p.x, p.y))
}

/// This frame's `UseAction` presses, taken before the engine's cast path sees them: each ability
/// on the hero's bar becomes the next [`LocalInput::press`], at the current target (WoW's own
/// selection, Tab included) or, aimed at the ground, at the spot under the mouse; Stealth's
/// button counts a Stealth press. A press of an empty button does nothing.
pub(super) fn presses(
    script: Option<NonSendMut<UiScript>>,
    sim: Res<Sim>,
    selection: Res<Selection>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    world_point: WorldPoint,
    mut input: ResMut<LocalInput>,
) {
    let Some(mut script) = script else {
        return;
    };
    let uses = script.take_action_uses();
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let content = sim.world.content();
    let kit = &content.classes[hero.class].kit;
    for u in uses {
        let Some(ability) = slot_ability(kit, u.action) else {
            continue;
        };
        let a = &content.abilities[ability];
        if a.has(|e| matches!(e, Effect::Stealth)) {
            input.stealth_presses = input.stealth_presses.wrapping_add(1);
            continue;
        }
        let at = (a.aim == Aim::Ground).then(|| {
            cursor_ground(&windows, &cameras, &world_point).unwrap_or_else(|| ahead(hero, a.range))
        });
        let target = selection.guid.and_then(units::unit_of);
        input.press(u.action as u8, target, at);
    }
}

/// `NIGHTFALL_AUTOFIGHT=1`: with no living enemy targeted, WoW's own Tab picks the nearest; else
/// every [`AUTO_EVERY`] seconds the next of the hero's plain attacks is pressed through
/// `UseAction`, as a key would; standing in a telegraph, the dodge binding is pressed.
pub(super) fn autofight(
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    selection: Res<Selection>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
    mut last_dodge: Local<f32>,
    mut turn: Local<usize>,
) {
    if !autofight_on() {
        return;
    }
    let Some(script) = script else {
        return;
    };
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let now = real.elapsed_secs();
    let threatened = sim
        .world
        .telegraphs
        .iter()
        .any(|t| t.shape.contains(t.at.ground(), t.facing, hero.pos));
    if threatened && hero.fight.up() && now - *last_dodge >= AUTO_DODGE_EVERY {
        *last_dodge = now;
        let c = super::dodge::COUNTER;
        let _ = script.run(&format!("{c} = ({c} or 0) + 1"));
    }
    if now - *last < AUTO_EVERY || !hero.fight.up() {
        return;
    }
    *last = now;
    let alive = matches!(
        selection.guid.and_then(units::unit_of),
        Some(UnitRef::Enemy(id)) if sim.world.enemy(id).is_some_and(|e| e.hp > 0.0)
    );
    if !alive {
        let _ = script.run("TargetNearestEnemy()");
        return;
    }
    let content = sim.world.content();
    let moves = attacks(
        &content.classes[hero.class].kit,
        &content.abilities,
        autoplay(),
    );
    if moves.is_empty() {
        return;
    }
    let action = moves[*turn % moves.len()];
    *turn += 1;
    let _ = script.run(&format!("UseAction({action})"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use nightfall_sim::{Content, ContentSources};

    fn content() -> Content {
        Content::from_ron(ContentSources {
            classes: r#"[(key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0,
                base_hp: 1000.0, speed: 7.0, power_max: 100.0,
                bar: [(1, "stealth"), (2, "strike"), (3, "bolt"), (11, "stone"), (74, "ambush")])]"#,
            map: r#"(key: "t", name: "T", map: 0,
                bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                entries: [(5.0, 5.0, 0.0)])"#,
            rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            enemies: "[]",
            spawns: "[]",
            abilities: r#"[
                (key: "stealth", name: "Stealth", spell: 1785, aim: Me, effects: [Stealth]),
                (key: "strike", name: "Strike", spell: 1760, aim: Enemy, range: 5.0,
                 effects: [Weapon(bonus: 22.0)]),
                (key: "bolt", name: "Bolt", spell: 133, aim: Enemy, range: 30.0, cast: 3.0,
                 moving: Rooted, effects: []),
                (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0, effects: []),
                (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0,
                 needs: (stealth: true), effects: [Weapon(percent: 250.0)]),
            ]"#,
        })
        .unwrap()
    }

    #[test]
    fn an_action_names_its_ability_and_the_autofight_takes_plain_attacks() {
        let c = content();
        let kit = &c.classes[0].kit;
        assert_eq!(slot_ability(kit, 2), Some(1));
        assert_eq!(slot_ability(kit, 74), Some(4));
        assert_eq!(slot_ability(kit, 5), None);
        // Walking, a rooted cast would be refused, so the autofight leaves it out.
        assert_eq!(attacks(kit, &c.abilities, false), vec![2, 3]);
        assert_eq!(attacks(kit, &c.abilities, true), vec![2]);
    }

    #[test]
    fn the_cursor_ray_meets_the_ground() {
        let flat = |_: Vec3| Some(10.0);
        let hit = ground_hit(
            Vec3::new(0.0, 20.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            50.0,
            flat,
        )
        .unwrap();
        assert!((hit.x - 10.0).abs() < 0.3, "{hit:?}");
        assert_eq!(hit.y, 10.0);
        assert!(ground_hit(Vec3::new(0.0, 20.0, 0.0), Vec3::Y, 50.0, flat).is_none());
        assert!(ground_hit(Vec3::new(0.0, 20.0, 0.0), Vec3::ZERO, 50.0, flat).is_none());
    }
}
