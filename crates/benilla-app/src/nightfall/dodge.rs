//! The dodge (spec §6.1): a 6-yard hop the way the hero moves, on a key of its own. The key is a
//! WoW key binding, `NIGHTFALL_DODGE`, listed under "Nightfall" in the game's own Key Bindings
//! window and saved with the player's other bindings; it starts on Caps Lock and the mouse's back
//! button, which 1.12's `DefaultBindings.wtf` leaves free. The client hops at once when its mirror
//! shows a charge (the host's word would come a round trip later), as a knockback the controller
//! flies with its own collision, and counts the press into the input; the host spends the charge
//! and makes telegraphed blows miss.

use benilla_protocol::JumpInfo;
use benilla_ui::bindings_xml::Binding;
use benilla_ui::script::UiScript;
use bevy::prelude::*;
use nightfall_sim::V2;

use super::run::{LocalInput, Sim};
use super::view::Views;
use crate::creature_anim::move_flags::{BACKWARD, FORWARD, STRAFE_LEFT, STRAFE_RIGHT};
use crate::net::{KnockBackMessage, SelfGuid};
use crate::ui_script::PendingEntryUiLoad;

/// The binding's command.
const BINDING: &str = "NIGHTFALL_DODGE";
/// The Lua global the binding counts its presses in.
pub(super) const COUNTER: &str = "NIGHTFALL_DODGES";
/// Run once per UI session: the binding's names in the Key Bindings window, its counter, and its
/// default keys unless the player has bound it already.
const SETUP: &str = r#"
BINDING_HEADER_NIGHTFALL = "Nightfall"
BINDING_NAME_NIGHTFALL_DODGE = "Dodge"
NIGHTFALL_DODGES = NIGHTFALL_DODGES or 0
if not GetBindingKey("NIGHTFALL_DODGE") then
    SetBinding("CAPSLOCK", "NIGHTFALL_DODGE")
    SetBinding("BUTTON5", "NIGHTFALL_DODGE")
end
"#;
/// Yards a second up the hop starts at; with WoW's gravity it lasts about half a second.
const HOP_UP: f32 = 5.0;
/// WoW's gravity, yards a second squared.
const GRAVITY: f32 = 19.291_105;

/// The binding as the Key Bindings window lists it: a press counts into [`COUNTER`].
pub(super) fn binding() -> Binding {
    Binding {
        name: BINDING.into(),
        header: Some("NIGHTFALL".into()),
        run_on_up: false,
        hidden: false,
        platform: None,
        body: format!("{COUNTER} = ({COUNTER} or 0) + 1"),
    }
}

/// The way a dodge goes: where the movement keys held point, turned with `facing` (WoW's ground
/// plane, x north, y west); straight back with none held.
pub(super) fn dodge_way(facing: V2, flags: u32) -> V2 {
    let f = facing.normalize_or_zero();
    let left = V2::new(-f.y, f.x);
    let mut way = V2::ZERO;
    if flags & FORWARD != 0 {
        way += f;
    }
    if flags & BACKWARD != 0 {
        way += f * -1.0;
    }
    if flags & STRAFE_LEFT != 0 {
        way += left;
    }
    if flags & STRAFE_RIGHT != 0 {
        way += left * -1.0;
    }
    let way = way.normalize_or_zero();
    if way == V2::ZERO {
        f * -1.0
    } else {
        way
    }
}

/// The launch of a hop of `distance` yards the way `way` goes, in the knockback's terms.
pub(super) fn hop(way: V2, distance: f32) -> JumpInfo {
    let air = 2.0 * HOP_UP / GRAVITY;
    JumpInfo {
        zspeed: -HOP_UP,
        cos_angle: way.x,
        sin_angle: way.y,
        xy_speed: distance / air,
    }
}

/// Once per UI session (a `/reload` is a new one), after its load: the binding registered under
/// "Nightfall" and [`SETUP`] run.
pub(super) fn setup(
    script: Option<NonSendMut<UiScript>>,
    loading: Option<Res<PendingEntryUiLoad>>,
    mut done: Local<Option<u64>>,
) {
    let Some(mut script) = script else {
        return;
    };
    if loading.is_some() {
        return;
    }
    let session = script.session();
    if *done == Some(session) {
        return;
    }
    script.register_addon_bindings("Nightfall", &[binding()]);
    if script.run(SETUP).is_ok() {
        *done = Some(session);
    }
}

/// Whether the binding's count `now` in UI `session` is a new press, given the `(session, count)`
/// last seen. A count is only comparable within its session (each VM starts it at 0), so a new
/// session, or a count that went down, is a new baseline and no press; so is the first count read.
fn pressed(seen: &mut Option<(u64, u32)>, session: u64, now: u32) -> bool {
    matches!(seen.replace((session, now)), Some((s, last)) if s == session && now > last)
}

/// A dodge press (the binding's count went up): when our mirror shows the hero up and with a
/// charge, the press goes into the input; and when they are free-footed the controller also hops
/// them [`DodgeRules::distance`] yards the way they move. A rooted caster (Fireball, Arcane
/// Missiles) is not hopped here, since the host's dodge cancels the cast, but the press must reach
/// it. The first count read, and the first of a new session, is a baseline ([`pressed`]).
///
/// [`DodgeRules::distance`]: nightfall_sim::DodgeRules
pub(super) fn key(
    script: Option<NonSend<UiScript>>,
    sim: Res<Sim>,
    player: Res<crate::player::Player>,
    self_guid: Res<SelfGuid>,
    views: Res<Views>,
    bodies: Query<&Transform>,
    mut knocks: MessageWriter<KnockBackMessage>,
    mut input: ResMut<LocalInput>,
    mut seen: Local<Option<(u64, u32)>>,
    mut counter: Local<u32>,
) {
    let Some(script) = script else {
        return;
    };
    // [`SETUP`] makes the counter a number: until it has run in this session there is none to
    // read, and nothing to baseline.
    let Some(now) = script
        .lua()
        .globals()
        .get::<Option<u32>>(COUNTER)
        .ok()
        .flatten()
    else {
        return;
    };
    if !pressed(&mut seen, script.session(), now) {
        return;
    }
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    let f = &hero.fight;
    if !(f.up() && f.dodge.have > 0 && player.active) {
        return;
    }
    // A rooted hero (mid-cast) does not hop here, but the press still goes up: the host cancels the
    // cast, spends the charge and opens the window, and the root lifts with its next snapshot.
    if f.rooted {
        input.dodge_presses = input.dodge_presses.wrapping_add(1);
        return;
    }
    let (Some(guid), Some(tf)) = (
        self_guid.0,
        views.heroes.get(&sim.me).and_then(|&e| bodies.get(e).ok()),
    ) else {
        return;
    };
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    let way = dodge_way(V2::new(yaw.cos(), yaw.sin()), player.move_flags());
    *counter = counter.wrapping_add(1);
    knocks.write(KnockBackMessage {
        guid,
        counter: *counter,
        launch: hop(way, sim.world.content().rules.combat.dodge.distance),
    });
    input.dodge_presses = input.dodge_presses.wrapping_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dodge_goes_the_way_the_keys_point_and_back_with_none() {
        let north = V2::new(1.0, 0.0);
        assert_eq!(dodge_way(north, FORWARD), north);
        assert_eq!(dodge_way(north, 0), V2::new(-1.0, 0.0));
        // Left of north is west, +y.
        assert_eq!(dodge_way(north, STRAFE_LEFT), V2::new(0.0, 1.0));
        let d = dodge_way(north, FORWARD | STRAFE_RIGHT);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((d.x - s).abs() < 1e-3 && (d.y + s).abs() < 1e-3);
        // Forward and back cancel: back it is.
        assert_eq!(dodge_way(north, FORWARD | BACKWARD), V2::new(-1.0, 0.0));
    }

    #[test]
    fn the_hop_covers_its_distance_in_the_air() {
        let j = hop(V2::new(0.0, 1.0), 6.0);
        let air = 2.0 * -j.zspeed / GRAVITY;
        assert!((j.xy_speed * air - 6.0).abs() < 1e-4);
        assert_eq!((j.cos_angle, j.sin_angle), (0.0, 1.0));
    }

    #[test]
    fn a_count_presses_only_when_it_rises_within_its_session() {
        let mut seen = None;
        // The first count read is a baseline, wherever it stands.
        assert!(!pressed(&mut seen, 1, 4));
        assert!(!pressed(&mut seen, 1, 4));
        // One more is one press, and several at once are one too.
        assert!(pressed(&mut seen, 1, 5));
        assert!(!pressed(&mut seen, 1, 5));
        assert!(pressed(&mut seen, 1, 8));
        // A new session starts its count over: no press, even for a count above the last.
        assert!(!pressed(&mut seen, 2, 0));
        assert!(!pressed(&mut seen, 3, 9));
        // A count that went down re-baselines.
        assert!(!pressed(&mut seen, 3, 2));
        assert!(pressed(&mut seen, 3, 3));
    }

    /// A stage for [`key`]: a rogue (player 0) the controller drives, at a body in the views, the
    /// binding's counter at 0 and read once as the baseline.
    fn key_stage(rooted: bool) -> (bevy::app::App, bevy::ecs::system::SystemId) {
        let mut app = bevy::app::App::new();
        app.add_message::<KnockBackMessage>();
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        let fight = &mut sim.heroes[0].fight;
        fight.rooted = rooted;
        if rooted {
            fight.casting = Some(nightfall_sim::Casting {
                ability: 0,
                left: 2.0,
                total: 3.0,
                channel: false,
                target: None,
            });
        }
        assert!(fight.dodge.have > 0);
        app.insert_resource(Sim::for_test(sim));
        let mut player = crate::player::Player::default();
        player.active = true;
        app.insert_resource(player);
        app.insert_resource(SelfGuid(Some(7)));
        let body = app.world_mut().spawn(Transform::default()).id();
        let mut views = Views::default();
        views.heroes.insert(0, body);
        app.insert_resource(views);
        app.init_resource::<LocalInput>();
        let script = UiScript::new().unwrap();
        script.run(&format!("{COUNTER} = 0")).unwrap();
        app.world_mut().insert_non_send_resource(script);
        let id = app.world_mut().register_system(key);
        // The first count read is the baseline, not a press.
        app.world_mut().run_system(id).unwrap();
        (app, id)
    }

    /// Raises the binding's counter once, as a press of the key does, and runs [`key`].
    fn press_dodge(app: &mut bevy::app::App, id: bevy::ecs::system::SystemId, count: u32) {
        app.world()
            .non_send_resource::<UiScript>()
            .run(&format!("{COUNTER} = {count}"))
            .unwrap();
        app.world_mut().run_system(id).unwrap();
    }

    #[test]
    fn a_free_footed_hero_hops_and_counts_the_press() {
        let (mut app, id) = key_stage(false);
        press_dodge(&mut app, id, 1);
        assert_eq!(app.world().resource::<LocalInput>().dodge_presses, 1);
        assert_eq!(
            app.world().resource::<Messages<KnockBackMessage>>().len(),
            1
        );
    }

    #[test]
    fn a_rooted_caster_counts_the_press_for_the_host_but_does_not_hop() {
        // The host's dodge cancels the cast; were the press not sent, a mage channelling beside a
        // winding-up stomp would be locked in.
        let (mut app, id) = key_stage(true);
        press_dodge(&mut app, id, 1);
        assert_eq!(app.world().resource::<LocalInput>().dodge_presses, 1);
        assert_eq!(
            app.world().resource::<Messages<KnockBackMessage>>().len(),
            0
        );
    }

    #[test]
    fn the_binding_counts_its_presses() {
        let b = binding();
        assert_eq!(b.name, "NIGHTFALL_DODGE");
        assert_eq!(b.body, "NIGHTFALL_DODGES = (NIGHTFALL_DODGES or 0) + 1");
    }
}
