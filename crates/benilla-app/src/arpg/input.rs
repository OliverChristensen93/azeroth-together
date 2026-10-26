//! The keys and the mouse, read into the local player's [`PlayerInput`].
//!
//! WASD walks the way the camera looks, the mouse steers the view and the crosshair aims, Space
//! rolls, and each slot has its
//! key: left and right mouse, Q, E, R, F, 1 and Shift. Holding a key casts its slot again as soon
//! as it can;
//! of several held, the last one pressed wins. Enter wakes a fallen hero at the start. While the
//! game menu is up nothing reaches the hero, and a click on WoW's interface casts nothing.
//! `ARPG_AUTOPLAY=1` plays by itself instead, for test runs: it circles, aims at the nearest
//! enemy, goes through the slots and rolls out of telegraphs.

use arpg_sim::{PlayerInput, V2};
use bevy::prelude::*;

use super::camera::AimPoint;
use super::{Sim, ME};

/// What each slot is bound to, in slot order.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Bind {
    Mouse(MouseButton),
    Key(KeyCode),
}

pub(super) const BINDS: [Bind; 8] = [
    Bind::Mouse(MouseButton::Left),
    Bind::Mouse(MouseButton::Right),
    Bind::Key(KeyCode::KeyQ),
    Bind::Key(KeyCode::KeyE),
    Bind::Key(KeyCode::KeyR),
    Bind::Key(KeyCode::KeyF),
    Bind::Key(KeyCode::Digit1),
    Bind::Key(KeyCode::ShiftLeft),
];

/// The label a slot's key shows on the bar.
pub(super) fn label(slot: usize) -> &'static str {
    ["LMB", "RMB", "Q", "E", "R", "F", "1", "Shift"]
        .get(slot)
        .copied()
        .unwrap_or("")
}

/// The slot held down, as last pressed.
#[derive(Resource, Default)]
pub(super) struct Held(Option<u8>);

impl Bind {
    fn pressed(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Bind::Mouse(b) => mouse.pressed(b),
            Bind::Key(k) => keys.pressed(k),
        }
    }

    fn just_pressed(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Bind::Mouse(b) => mouse.just_pressed(b),
            Bind::Key(k) => keys.just_pressed(k),
        }
    }
}

/// The autoplay's input: off to the nearest enemy, circling it while it fights, each slot in
/// turn, a roll out of any telegraph near, and up again when it falls.
fn autoplay(world: &arpg_sim::World) -> PlayerInput {
    let Some(hero) = world.hero(ME) else {
        return PlayerInput::default();
    };
    let t = world.time();
    let nearest = world
        .enemies()
        .iter()
        .min_by(|a, b| a.pos.distance(hero.pos).total_cmp(&b.pos.distance(hero.pos)));
    let aim = nearest.map_or(hero.pos + hero.facing * 10.0, |e| e.pos);
    let near = nearest.map_or(f32::MAX, |e| e.pos.distance(hero.pos));
    // Blizzard and Ice Block are left out: one roots it, the other wastes it.
    let cycle: [u8; 6] = [0, 0, 1, 2, 3, 5];
    let slot = cycle[(t / 1.5) as usize % cycle.len()];
    let cast = (near < 25.0).then_some(slot).filter(|&s| s != 2 || near < 7.0);
    // Closer while far; then round it at a distance.
    let move_dir = match nearest {
        Some(e) if near > 16.0 => e.pos - hero.pos,
        Some(e) => (hero.pos - e.pos).rotate(1.2),
        None => V2::ZERO,
    };
    let threat = world
        .enemies()
        .iter()
        .any(|e| world.telegraph(e).is_some() && e.pos.distance(hero.pos) < 6.0);
    PlayerInput {
        move_dir,
        aim,
        cast: cast.or((near < 25.0).then_some(0)),
        dodge: threat,
        restart: world.is_over(),
    }
}

pub(super) fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimPoint>,
    rig: Res<super::camera::CamRig>,
    mut held: ResMut<Held>,
    mut sim: ResMut<Sim>,
    menu: Res<super::ui::Menu>,
    over_ui: Res<crate::ui_script::PointerOverUi>,
    mut auto: Local<Option<bool>>,
) {
    if menu.open {
        held.0 = None;
        let idle = PlayerInput {
            aim: sim.world.hero(ME).map_or(V2::ZERO, |h| h.pos + h.facing),
            ..Default::default()
        };
        sim.world.set_input(ME, idle);
        return;
    }
    if *auto.get_or_insert_with(|| std::env::var_os("ARPG_AUTOPLAY").is_some()) {
        let input = autoplay(&sim.world);
        sim.world.set_input(ME, input);
        return;
    }
    // Ahead is where the camera looks.
    let (fwd, right) = rig.ground_axes();
    let mut walk = Vec3::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        walk += fwd;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        walk -= fwd;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        walk -= right;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        walk += right;
    }
    let dir = super::view::to_ground(walk);

    if let Some(slot) = BINDS
        .iter()
        .position(|b| b.just_pressed(&keys, &mouse))
        // A click on WoW's interface is the interface's.
        .filter(|&s| !(over_ui.0 && matches!(BINDS[s], Bind::Mouse(_))))
    {
        held.0 = Some(slot as u8);
    }
    if held
        .0
        .is_some_and(|s| !BINDS[s as usize].pressed(&keys, &mouse))
    {
        held.0 = BINDS
            .iter()
            .position(|b| b.pressed(&keys, &mouse))
            .map(|s| s as u8);
    }

    let me = sim.world.hero(ME).map(|h| (h.pos, h.facing));
    let aim = match (aim.0, me) {
        (Some(p), _) => super::view::to_ground(p),
        // With the cursor off the window, straight ahead.
        (None, Some((pos, facing))) => pos + facing * 10.0,
        (None, None) => V2::ZERO,
    };
    let input = PlayerInput {
        move_dir: dir,
        aim,
        cast: held.0,
        dodge: keys.just_pressed(KeyCode::Space),
        restart: false,
    };
    sim.world.set_input(ME, input);
    if keys.just_pressed(KeyCode::Enter) && sim.world.is_over() {
        sim.world.restart();
    }
}
