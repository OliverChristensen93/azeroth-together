//! The keys and the mouse, read into the local player's [`PlayerInput`].
//!
//! WASD walks (up on the screen is north), the mouse aims, and each slot has its key: left and
//! right mouse, Q, E, R, F, 1 and Space. Holding a key casts its slot again as soon as it can;
//! of several held, the last one pressed wins. Enter starts over once the hero has fallen.
//! `ARPG_AUTOPLAY=1` plays by itself instead, for test runs: it circles, aims at the nearest
//! enemy and goes through the slots.

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
    Bind::Key(KeyCode::Space),
];

/// The label a slot's key shows on the bar.
pub(super) fn label(slot: usize) -> &'static str {
    ["LMB", "RMB", "Q", "E", "R", "F", "1", "Space"]
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

/// The autoplay's input: circling the start, aiming at the nearest enemy, each slot in turn,
/// starting over when it falls.
fn autoplay(world: &arpg_sim::World) -> PlayerInput {
    let Some(hero) = world.hero(ME) else {
        return PlayerInput::default();
    };
    let t = world.time();
    let around = world.start() + V2::from_angle(t * 0.15, 8.0);
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
    PlayerInput {
        move_dir: around - hero.pos,
        aim,
        cast: cast.or((near < 25.0).then_some(0)),
        restart: world.is_over(),
    }
}

pub(super) fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    aim: Res<AimPoint>,
    mut held: ResMut<Held>,
    mut sim: ResMut<Sim>,
    mut auto: Local<Option<bool>>,
) {
    if *auto.get_or_insert_with(|| std::env::var_os("ARPG_AUTOPLAY").is_some()) {
        let input = autoplay(&sim.world);
        sim.world.set_input(ME, input);
        return;
    }
    let mut dir = V2::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        dir.x += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        dir.x -= 1.0;
    }
    // WoW's +y is west: left on a north-up screen.
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        dir.y += 1.0;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        dir.y -= 1.0;
    }

    if let Some(slot) = BINDS
        .iter()
        .position(|b| b.just_pressed(&keys, &mouse))
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
        restart: false,
    };
    sim.world.set_input(ME, input);
    if keys.just_pressed(KeyCode::Enter) && sim.world.is_over() {
        sim.world.restart();
    }
}
