//! The local hero's stance as the host needs it (spec §5.1): how they move, read off WoW's own
//! controller (its walk toggle, `/` on the number pad, and the movement keys held), and the
//! Rogue's Stealth press, counted into [`LocalInput`]. Stealth is a button on the action bar
//! ([`super::bar::presses`] counts its presses). `NIGHTFALL_AUTOPLAY_GAIT` (`walk`, `run` or
//! `sneak`) says how the autoplay walk moves, `sneak` stealthing too ([`auto_stealth`]);
//! `NIGHTFALL_TRACE_SENSES=1` logs the hero's senses every 2 seconds.

use std::sync::OnceLock;

use bevy::prelude::*;
use nightfall_sim::{Alert, Gait};

use super::enemies::EnemyViews;
use super::run::{autoplay, LocalInput, Sim};
use crate::creature_anim::move_flags;
use crate::target::Concealed;

/// The movement bits that carry the body across the ground.
const TRANSLATING: u32 =
    move_flags::FORWARD | move_flags::BACKWARD | move_flags::STRAFE_LEFT | move_flags::STRAFE_RIGHT;
/// Seconds between autoplay's Stealth presses while its rogue is not (yet) in stealth.
const AUTO_PRESS_EVERY: f32 = 2.0;

/// The gait the controller's movement flags say: still with no movement key held (a jump in
/// place included), walking with the walk toggle on, else running.
pub(super) fn gait_of(flags: u32) -> Gait {
    if flags & TRANSLATING == 0 {
        Gait::Still
    } else if flags & move_flags::WALK_MODE != 0 {
        Gait::Walk
    } else {
        Gait::Run
    }
}

/// How the autoplay walk moves, from `NIGHTFALL_AUTOPLAY_GAIT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AutoGait {
    Walk,
    Run,
    /// Walking in stealth: autoplay presses Stealth until the hero is in it.
    Sneak,
}

impl AutoGait {
    /// The setting's value: `walk` (and anything unknown), `run` or `sneak`, any case.
    pub(super) fn parse(value: Option<&str>) -> AutoGait {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("run") => AutoGait::Run,
            Some("sneak") => AutoGait::Sneak,
            _ => AutoGait::Walk,
        }
    }

    /// The gait it reports: the autoplay walk moves the body itself, so no key is held.
    pub(super) fn gait(self) -> Gait {
        match self {
            AutoGait::Run => Gait::Run,
            AutoGait::Walk | AutoGait::Sneak => Gait::Walk,
        }
    }
}

pub(super) fn auto_gait() -> AutoGait {
    static GAIT: OnceLock<AutoGait> = OnceLock::new();
    *GAIT.get_or_init(|| AutoGait::parse(std::env::var("NIGHTFALL_AUTOPLAY_GAIT").ok().as_deref()))
}

fn trace_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_TRACE_SENSES").is_some())
}

/// The autoplay `sneak` gait, or a rogue near an enemy that has not noticed it
/// ([`super::auto_brain`]), presses Stealth every [`AUTO_PRESS_EVERY`] seconds until the hero is in
/// stealth; a player presses Stealth's button on the action bar.
pub(super) fn auto_stealth(
    sim: Res<Sim>,
    plan: Res<super::auto_brain::AutoPlan>,
    real: Res<Time<Real>>,
    mut input: ResMut<LocalInput>,
    mut last_auto: Local<Option<f32>>,
) {
    if !(autoplay() && (auto_gait() == AutoGait::Sneak || plan.0.sneak)) {
        return;
    }
    let now = real.elapsed_secs();
    let out_of_stealth = sim.world.hero(sim.me).is_some_and(|h| !h.stealthed);
    if out_of_stealth && last_auto.is_none_or(|t| now - t >= AUTO_PRESS_EVERY) {
        input.stealth_presses = input.stealth_presses.wrapping_add(1);
        *last_auto = Some(now);
    }
}

/// `NIGHTFALL_TRACE_SENSES=1`: every 2 seconds, our hero's stance, visibility and noise, how many
/// of the drawn enemies are concealed, what the enemies think of us, and how near the nearest
/// enemy alerted to us is.
pub(super) fn trace(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    concealed: Query<(), With<Concealed>>,
    real: Res<Time<Real>>,
    mut last: Local<f32>,
) {
    if !trace_on() || real.elapsed_secs() - *last < 2.0 {
        return;
    }
    *last = real.elapsed_secs();
    let Some(h) = sim.world.hero(sim.me) else {
        return;
    };
    let hidden = views
        .drawn
        .values()
        .filter(|&&e| concealed.contains(e))
        .count();
    let marks: Vec<String> = sim
        .marks
        .iter()
        .map(|m| format!("{} {:?}", m.enemy, m.alert))
        .collect();
    let nearest = sim
        .marks
        .iter()
        .filter(|m| m.alert == Alert::Alerted)
        .filter_map(|m| sim.world.enemy(m.enemy))
        .map(|e| e.pos.distance(h.pos))
        .fold(f32::INFINITY, f32::min);
    info!(
        "nightfall: senses: {:?}{}, visibility {:.0}, noise {:.0} yd, {hidden} of {} drawn enemies concealed, marks [{}], nearest alerted {}",
        h.stance,
        if h.stealthed { " (stealth)" } else { "" },
        h.visibility,
        h.noise,
        views.drawn.len(),
        marks.join(", "),
        if nearest.is_finite() { format!("{nearest:.0} yd") } else { "none".into() },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controllers_flags_give_the_gait() {
        assert_eq!(gait_of(0), Gait::Still);
        assert_eq!(gait_of(move_flags::WALK_MODE), Gait::Still);
        assert_eq!(gait_of(move_flags::FORWARD), Gait::Run);
        assert_eq!(gait_of(move_flags::BACKWARD), Gait::Run);
        assert_eq!(
            gait_of(move_flags::STRAFE_LEFT | move_flags::WALK_MODE),
            Gait::Walk
        );
        // A jump in place carries nobody across the ground.
        assert_eq!(gait_of(move_flags::FALLING), Gait::Still);
        assert_eq!(
            gait_of(move_flags::FORWARD | move_flags::FALLING),
            Gait::Run
        );
    }

    #[test]
    fn the_autoplay_gait_is_read_from_its_setting() {
        assert_eq!(AutoGait::parse(None), AutoGait::Walk);
        assert_eq!(AutoGait::parse(Some("RUN")), AutoGait::Run);
        assert_eq!(AutoGait::parse(Some("sneak")), AutoGait::Sneak);
        assert_eq!(AutoGait::parse(Some("crawl")), AutoGait::Walk);
        assert_eq!(AutoGait::Sneak.gait(), Gait::Walk);
        assert_eq!(AutoGait::Run.gait(), Gait::Run);
    }
}
