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
use nightfall_sim::{AbilityDef, Aim, Effect, Hero, HeroFight, Moving, UnitRef, V3};

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
/// autofight presses. Left out: an ability that hurts nothing (a control-only Polymorph, Slow) or
/// that only works on some kinds of creature (the enemy at hand may be another kind, and the host
/// refuses "Invalid target"). With `walking` (autoplay never stands still) the abilities the host
/// would refuse "Can't do that while moving" are left out too: those that need the caster still,
/// and the rooted ones that take time to cast or channel (a rooted instant is no hindrance).
pub(super) fn attacks(kit: &[(u8, usize)], abilities: &[AbilityDef], walking: bool) -> Vec<u8> {
    kit.iter()
        .filter(|&&(s, a)| {
            let ability = &abilities[a];
            let rooted =
                ability.moving == Moving::Rooted && (ability.cast > 0.0 || ability.channel > 0.0);
            let hurts = ability.has(|e| {
                matches!(
                    e,
                    Effect::Damage { .. } | Effect::Weapon { .. } | Effect::Dot { .. }
                )
            });
            s <= 12
                && ability.aim == Aim::Enemy
                && hurts
                && ability.needs.creatures.is_empty()
                && !ability.needs.stealth
                && !(walking && (rooted || ability.needs.still))
        })
        .map(|&(s, _)| s)
        .collect()
}

/// Whether the hero may press anything: not mid-cast (a press would be refused "Another action is
/// in progress") and not within the global cooldown.
pub(super) fn can_press(fight: &HeroFight) -> bool {
    fight.casting.is_none() && fight.gcd <= 0.0
}

/// Whether `ability` is still waiting out its own cooldown.
pub(super) fn on_cooldown(fight: &HeroFight, ability: usize) -> bool {
    fight
        .cooldowns
        .iter()
        .any(|c| usize::from(c.ability) == ability && c.left > 0.0)
}

/// Whether the host would not turn `ability` down for what the hero has: the power it costs, and,
/// for a finisher, combo points on `target` (the host's `NeedCombo`, which wants the points on
/// that very enemy). The autofight leaves out what fails this, so it presses what lands.
pub(super) fn affordable(ability: &AbilityDef, fight: &HeroFight, target: u32) -> bool {
    let combo = !ability.needs.combo || (fight.combo > 0 && fight.combo_on == Some(target));
    combo && fight.power + 1e-3 >= ability.cost
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
/// button counts a Stealth press. A press of an empty button does nothing. A ground press whose
/// cursor is over the interface (a bar button clicked), or whose ray finds no ground, goes to
/// [`ahead`] instead: never to the terrain under the bar.
pub(super) fn presses(
    script: Option<NonSendMut<UiScript>>,
    sim: Res<Sim>,
    selection: Res<Selection>,
    over_ui: Res<crate::ui_script::PointerOverUi>,
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
            (!over_ui.0)
                .then(|| cursor_ground(&windows, &cameras, &world_point))
                .flatten()
                .unwrap_or_else(|| ahead(hero, a.range))
        });
        let target = selection.guid.and_then(units::unit_of);
        input.press(u.action as u8, target, at);
    }
}

/// The actions the autofight may press at enemy `target` now: its plain attacks ([`attacks`]) that
/// are off cooldown, that the hero can pay for ([`affordable`]) and whose facing the hero meets
/// (the host's own rule, [`nightfall_sim::World::stands_for`]). An attack the hero stands wrong for
/// is left out rather than pressed into a refusal; the autoplay keeps walking, and so turns, so
/// the enemy comes into its front in time.
pub(super) fn autofight_moves(
    world: &nightfall_sim::World,
    me: nightfall_sim::PlayerId,
    hero: &Hero,
    target: u32,
    walking: bool,
) -> Vec<u8> {
    let content = world.content();
    let kit = &content.classes[hero.class].kit;
    attacks(kit, &content.abilities, walking)
        .into_iter()
        .filter(|&s| {
            slot_ability(kit, u32::from(s)).is_some_and(|a| {
                !on_cooldown(&hero.fight, a)
                    && affordable(&content.abilities[a], &hero.fight, target)
                    && world.stands_for(me, a, target)
            })
        })
        .collect()
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
    if now - *last < AUTO_EVERY || !hero.fight.up() || !can_press(&hero.fight) {
        return;
    }
    *last = now;
    let target = match selection.guid.and_then(units::unit_of) {
        Some(UnitRef::Enemy(id)) if sim.world.enemy(id).is_some_and(|e| e.hp > 0.0) => id,
        _ => {
            let _ = script.run("TargetNearestEnemy()");
            return;
        }
    };
    let moves = autofight_moves(&sim.world, sim.me, hero, target, autoplay());
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
                bar: [(1, "stealth"), (2, "strike"), (3, "bolt"), (4, "snap"), (5, "stand"),
                (11, "stone"), (74, "ambush"), (7, "poly"), (8, "hold")])]"#,
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
                 moving: Rooted, effects: [Damage(min: 1.0, max: 2.0)]),
                (key: "stone", name: "Stone", spell: 4164, aim: Ground, range: 25.0, effects: []),
                (key: "ambush", name: "Ambush", spell: 8724, aim: Enemy, range: 5.0,
                 needs: (stealth: true), effects: [Weapon(percent: 250.0)]),
                (key: "snap", name: "Snap", spell: 5, aim: Enemy, range: 30.0, moving: Rooted,
                 effects: [Damage(min: 1.0, max: 2.0)]),
                (key: "stand", name: "Stand", spell: 6, aim: Enemy, range: 30.0,
                 needs: (still: true), effects: [Damage(min: 1.0, max: 2.0)]),
                (key: "poly", name: "Poly", spell: 118, aim: Enemy, range: 30.0,
                 needs: (creatures: [Beast]), effects: [Damage(min: 1.0, max: 2.0)]),
                (key: "hold", name: "Hold", spell: 119, aim: Enemy, range: 30.0,
                 effects: [Control(kind: Polymorph, secs: 5.0)]),
            ]"#,
            items: "[]",
            loot: "()",
            monsters: "[]",
        })
        .unwrap()
    }

    #[test]
    fn an_action_names_its_ability_and_the_autofight_takes_plain_attacks() {
        let c = content();
        let kit = &c.classes[0].kit;
        assert_eq!(slot_ability(kit, 2), Some(1));
        assert_eq!(slot_ability(kit, 74), Some(4));
        assert_eq!(slot_ability(kit, 6), None);
        // Walking, the host refuses a rooted cast and one that needs the caster still, so the
        // autofight leaves them out; a rooted instant (slot 4) costs the caster nothing.
        assert_eq!(attacks(kit, &c.abilities, false), vec![2, 3, 4, 5]);
        assert_eq!(attacks(kit, &c.abilities, true), vec![2, 4]);
    }

    #[test]
    fn the_autofight_leaves_out_what_hurts_nothing_and_what_only_some_creatures_take() {
        let c = content();
        // Poly (slot 7) needs a beast and Hold (slot 8) only holds: neither is a plain attack,
        // however the enemy at hand is built.
        let kit = &c.classes[0].kit;
        assert_eq!(slot_ability(kit, 7), Some(7));
        assert_eq!(slot_ability(kit, 8), Some(8));
        for walking in [false, true] {
            let moves = attacks(kit, &c.abilities, walking);
            assert!(!moves.contains(&7) && !moves.contains(&8), "{moves:?}");
        }
    }

    #[test]
    fn the_autofight_waits_out_a_cast_the_gcd_and_cooldowns() {
        use nightfall_sim::{Casting, Cooldown};
        let mut fight = HeroFight::default();
        assert!(can_press(&fight));
        fight.gcd = 0.4;
        assert!(!can_press(&fight));
        fight.gcd = 0.0;
        fight.casting = Some(Casting {
            ability: 2,
            left: 1.0,
            total: 3.0,
            channel: false,
            target: None,
        });
        assert!(!can_press(&fight));
        fight.casting = None;
        fight.cooldowns = vec![
            Cooldown {
                ability: 3,
                left: 5.0,
            },
            Cooldown {
                ability: 4,
                left: 0.0,
            },
        ];
        assert!(on_cooldown(&fight, 3));
        assert!(!on_cooldown(&fight, 4), "a spent cooldown is ready");
        assert!(!on_cooldown(&fight, 1));
    }

    #[test]
    fn the_autofight_leaves_out_what_the_hero_stands_wrong_for() {
        use nightfall_sim::{Enemy, EnemyFight, Facing, V2};

        let mut world = nightfall_sim::World::new(super::super::shipped_content());
        world.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        let hero_at = world.hero(0).unwrap().pos;
        let enemy_at = |dx: f32| Enemy {
            id: 1,
            kind: 0,
            pos: hero_at + V2::new(dx, 0.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 1000.0,
            max_hp: 1000.0,
            speed: 0.0,
            asleep: false,
            fight: EnemyFight::default(),
        };
        // The hero faces +x: an enemy ahead of them is theirs to strike, one behind is not.
        world.enemies = vec![enemy_at(3.0)];
        let ahead = autofight_moves(&world, 0, world.hero(0).unwrap(), 1, false);
        world.enemies = vec![enemy_at(-3.0)];
        let behind = autofight_moves(&world, 0, world.hero(0).unwrap(), 1, false);
        assert!(!ahead.is_empty());
        assert!(behind.len() < ahead.len(), "{ahead:?} against {behind:?}");
        let content = world.content();
        let kit = &content.classes[world.hero(0).unwrap().class].kit;
        for s in &behind {
            let a = slot_ability(kit, u32::from(*s)).unwrap();
            assert_eq!(content.abilities[a].facing, Facing::Any, "slot {s}");
        }
    }

    #[test]
    fn the_autofight_skips_what_it_lacks_the_power_or_the_combo_points_for() {
        let mut c = content();
        // A finisher costing 25, and a cheap strike.
        c.abilities[1].cost = 25.0;
        let strike = &c.abilities[1];
        let mut finisher = c.abilities[1].clone();
        finisher.needs.combo = true;
        let mut fight = HeroFight {
            power: 30.0,
            ..HeroFight::default()
        };
        assert!(affordable(strike, &fight, 7));
        fight.power = 24.0;
        assert!(!affordable(strike, &fight, 7), "the power falls short");
        fight.power = 25.0;
        assert!(affordable(strike, &fight, 7), "exactly its cost");

        // A finisher wants points, and on this very enemy.
        assert!(!affordable(&finisher, &fight, 7), "no points");
        fight.combo = 3;
        assert!(!affordable(&finisher, &fight, 7), "points on no one");
        fight.combo_on = Some(8);
        assert!(!affordable(&finisher, &fight, 7), "points on another");
        fight.combo_on = Some(7);
        assert!(affordable(&finisher, &fight, 7));
        fight.power = 10.0;
        assert!(!affordable(&finisher, &fight, 7), "the points cannot pay");
    }

    /// A one-hero world with a Selection, an empty bar and the presses' other inputs.
    fn press_world(selected: Option<u64>) -> bevy::prelude::World {
        let mut world = bevy::prelude::World::new();
        benilla_world::world_point::init_world_point_resources(&mut world);
        let mut sim = nightfall_sim::World::new(std::sync::Arc::new(content()));
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        world.insert_resource(Sim::for_test(sim));
        world.insert_resource(Selection {
            guid: selected,
            ..Default::default()
        });
        world.insert_resource(crate::ui_script::PointerOverUi(true));
        world.init_resource::<LocalInput>();
        world.insert_non_send_resource(UiScript::new().unwrap());
        world
    }

    fn use_action(world: &mut bevy::prelude::World, n: u32) {
        let script = world.non_send_resource_mut::<UiScript>();
        script.run(&format!("UseAction({n})")).unwrap();
    }

    #[test]
    fn a_queued_use_action_becomes_one_fresh_press_and_leaves_the_engines_calls() {
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        use bevy::ecs::system::RunSystemOnce;

        let serial = super::super::enemies::enemy_guid_serial(3, 7);
        let guid = units::unit_guid(units::UnitKind::Creature { entry: 3 }, serial);
        let mut world = press_world(Some(guid));

        use_action(&mut world, 2);
        world.run_system_once(presses).unwrap();
        let first = world.resource::<LocalInput>().cast.unwrap();
        assert_eq!(
            (first.slot, first.target, first.at),
            (2, Some(UnitRef::Enemy(7)), None)
        );
        assert!(
            world
                .non_send_resource_mut::<UiScript>()
                .take_action_uses()
                .is_empty(),
            "the engine's cast path must not see the press"
        );

        // The same button again is the next press, not a repeat of the last.
        use_action(&mut world, 2);
        world.run_system_once(presses).unwrap();
        assert_eq!(
            world.resource::<LocalInput>().cast.unwrap().seq,
            first.seq.wrapping_add(1)
        );

        // Stealth's button counts a Stealth press and casts nothing; an empty button does nothing.
        let cast = world.resource::<LocalInput>().cast;
        use_action(&mut world, 1);
        use_action(&mut world, 9);
        world.run_system_once(presses).unwrap();
        let input = world.resource::<LocalInput>();
        assert_eq!((input.stealth_presses, input.cast), (1, cast));

        // A ground press with the pointer over the interface goes straight ahead of the hero.
        use_action(&mut world, 11);
        world.run_system_once(presses).unwrap();
        let sim = world.resource::<Sim>();
        let want = ahead(sim.world.hero(sim.me).unwrap(), 25.0);
        assert_eq!(world.resource::<LocalInput>().cast.unwrap().at, Some(want));
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
