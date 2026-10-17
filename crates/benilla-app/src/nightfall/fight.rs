//! The fight, shown (spec §6). [`show_events`] plays what happened each step through the engine's
//! own seams, as a server's packets would: a cast's precast glow and our cast bar, a channel's
//! bar, the cast's visual with its missile and impacts, a stopped cast, melee swings, floating
//! combat text, WoW's own red error text for a press the host turned down, and a Blink's teleport
//! for our hero. It decides nothing: every event is the host's.

use bevy::prelude::*;
use nightfall_sim::{Event, Refusal, Stop, UnitRef};

use super::enemies::EnemyViews;
use super::fx::{Fx, COLOR_EVADE, COLOR_HIT, COLOR_HURT, COLOR_SPELL};
use super::run::Sim;
use super::units;
use super::view::Views;
use crate::net::{SelfGuid, TeleportMessage};
use crate::ui_action::{UiError, UiErrorKeys};
use crate::ui_cast::{CastBarEdge, CastBarFeed};

/// `ClassDef::power` of a class whose power is mana.
const MANA: u8 = 0;

/// WoW's own GlobalStrings key for a refusal, the one its client shows for the same mistake
/// (`NoPower` by the class's power). Every refusal has one: none is silent.
pub(super) fn refusal_key(why: Refusal, power: u8) -> &'static str {
    match why {
        Refusal::Down => "SPELL_FAILED_CASTER_DEAD",
        Refusal::Busy => "SPELL_FAILED_SPELL_IN_PROGRESS",
        Refusal::NotReady => "SPELL_FAILED_NOT_READY",
        Refusal::NoPower if power == MANA => "ERR_OUT_OF_MANA",
        Refusal::NoPower => "ERR_OUT_OF_ENERGY",
        Refusal::Moving => "SPELL_FAILED_MOVING",
        Refusal::NeedStealth => "SPELL_FAILED_ONLY_STEALTHED",
        Refusal::InCombat => "SPELL_FAILED_AFFECTING_COMBAT",
        Refusal::NeedCombo => "SPELL_FAILED_NO_COMBO_POINTS",
        Refusal::NoTarget => "ERR_GENERIC_NO_TARGET",
        Refusal::BadTarget => "SPELL_FAILED_BAD_TARGETS",
        Refusal::Aware => "SPELL_FAILED_TARGET_AFFECTING_COMBAT",
        Refusal::OutOfRange => "SPELL_FAILED_OUT_OF_RANGE",
        Refusal::Facing => "SPELL_FAILED_UNIT_NOT_INFRONT",
        Refusal::Behind => "SPELL_FAILED_NOT_BEHIND",
        Refusal::NotInSight => "SPELL_FAILED_LINE_OF_SIGHT",
    }
}

/// Our cast bar's edge for a cast of ours that stopped: a channel's end empties it, a lost
/// target reads "Failed", anything else "Interrupted".
pub(super) fn stop_edge(why: Stop) -> CastBarEdge {
    match why {
        Stop::Finished => CastBarEdge::ChannelUpdate { remaining_ms: 0 },
        Stop::TargetLost => CastBarEdge::Failed,
        Stop::Moved | Stop::Cancelled | Stop::Interrupted => CastBarEdge::Interrupted,
    }
}

/// Seconds as the cast bar's milliseconds.
pub(super) fn ms(secs: f32) -> u32 {
    (secs * 1000.0).round().max(0.0) as u32
}

/// Whether a stopped cast must release its caster's precast pose and glow. A cast that stopped
/// before going off never did; a channel holds its pose to the end, and its ticks go off on
/// another spell, so even its normal finish leaves the hold up. (A `Fail` is a quiet release,
/// with no flash.)
pub(super) fn releases_hold(channel: bool, why: Stop) -> bool {
    channel || why != Stop::Finished
}

/// The `Damage` a melee `Swing` at `events[swing]` dealt: the next damage to its victim by its
/// attacker, before the same pair swings again (the sim pushes a blow's damage right after its
/// swing). `None` for any other event, or a swing that did none, which WoW's portrait reads as a
/// miss.
pub(super) fn swing_blow(events: &[Event], swing: usize) -> Option<usize> {
    let Some(Event::Swing { by, to }) = events.get(swing) else {
        return None;
    };
    events[swing + 1..]
        .iter()
        .take_while(|e| !matches!(e, Event::Swing { by: b, to: t } if b == by && t == to))
        .position(|e| matches!(e, Event::Damage { to: t, by: Some(b), .. } if t == to && b == by))
        .map(|p| swing + 1 + p)
}

/// Whether `events[damage]` is the damage of a melee blow (some swing's [`swing_blow`]), not a
/// spell's.
pub(super) fn is_blow(events: &[Event], damage: usize) -> bool {
    (0..damage).any(|s| swing_blow(events, s) == Some(damage))
}

/// The amount of a `Damage` event; 0 for any other.
fn damage_of(event: &Event) -> f32 {
    match event {
        Event::Damage { amount, .. } => *amount,
        _ => 0.0,
    }
}

/// This frame's events, shown: every hero's and drawn enemy's casts, swings and numbers; ours
/// also on our cast bar, in WoW's red error text, and (a Blink) in our controller's teleport.
/// An event about a unit not drawn (an enemy past the view distance) shows only what it can.
pub(super) fn show_events(
    sim: Res<Sim>,
    views: Res<Views>,
    enemy_views: Res<EnemyViews>,
    self_guid: Res<SelfGuid>,
    mut fx: Fx,
    mut bar: ResMut<CastBarFeed>,
    mut errors: ResMut<UiErrorKeys>,
    mut teleports: MessageWriter<TeleportMessage>,
    mut teleport_counter: Local<u32>,
) {
    let content = sim.world.content();
    let body = |u: UnitRef| match u {
        UnitRef::Hero(p) => views.heroes.get(&p).copied(),
        UnitRef::Enemy(id) => enemy_views.drawn.get(&id).copied(),
    };
    let me = UnitRef::Hero(sim.me);
    let power = sim
        .world
        .hero(sim.me)
        .map_or(MANA, |h| content.classes[h.class].power);
    for (i, event) in sim.events.iter().enumerate() {
        match event {
            Event::CastStart {
                by, ability, secs, ..
            } => {
                let spell = content.abilities[usize::from(*ability)].spell;
                if let Some(e) = body(*by) {
                    fx.start(e, spell);
                }
                if *by == me {
                    bar.0.push(CastBarEdge::Start {
                        spell_id: spell,
                        cast_time_ms: ms(*secs),
                    });
                }
            }
            Event::ChannelStart {
                by, ability, secs, ..
            } => {
                let spell = content.abilities[usize::from(*ability)].spell;
                if let Some(e) = body(*by) {
                    fx.start(e, spell);
                }
                if *by == me {
                    bar.0.push(CastBarEdge::ChannelStart {
                        spell_id: spell,
                        duration_ms: ms(*secs),
                    });
                }
            }
            Event::CastGo {
                by,
                ability,
                hits,
                at,
            } => {
                let a = &content.abilities[usize::from(*ability)];
                let channel = a.channel > 0.0;
                let spell = if channel {
                    a.tick_spell.unwrap_or(a.spell)
                } else {
                    a.spell
                };
                let struck: Vec<Entity> = hits.iter().filter_map(|u| body(*u)).collect();
                let dest = at.map(|p| units::wow_to_bevy([p.x, p.y, p.z]));
                if let Some(e) = body(*by) {
                    fx.go(e, spell, &struck, dest);
                }
                if *by == me && !channel {
                    bar.0.push(CastBarEdge::Stop);
                }
            }
            Event::CastStop { by, ability, why } => {
                let a = &content.abilities[usize::from(*ability)];
                // The hold is on the ability's own spell, whatever spell its ticks play.
                if let (Some(e), true) = (body(*by), releases_hold(a.channel > 0.0, *why)) {
                    fx.fail(e, a.spell);
                }
                if *by == me {
                    bar.0.push(stop_edge(*why));
                }
            }
            Event::Damage {
                to,
                amount,
                crit,
                by,
                ..
            } => {
                let Some(e) = body(*to) else { continue };
                // A hero's own melee is white in WoW, their spells yellow.
                let color = if *to == me {
                    COLOR_HURT
                } else if matches!(by, Some(UnitRef::Hero(_))) && !is_blow(&sim.events, i) {
                    COLOR_SPELL
                } else {
                    COLOR_HIT
                };
                fx.number(e, *amount, *crit, color);
            }
            Event::Swing { by, to } => {
                if let (Some(a), Some(v)) = (body(*by), body(*to)) {
                    let dealt =
                        swing_blow(&sim.events, i).map_or(0.0, |d| damage_of(&sim.events[d]));
                    fx.swing(a, v, dealt);
                }
            }
            Event::Evaded { player, .. } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    fx.text(e, "Evade".into(), COLOR_EVADE);
                }
            }
            Event::Refused { player, why } if *player == sim.me => {
                errors.0.push(UiError::key(refusal_key(*why, power)));
            }
            Event::Blinked { player, to } if *player == sim.me => {
                let (Some(guid), Some(hero)) = (self_guid.0, sim.world.hero(sim.me)) else {
                    continue;
                };
                *teleport_counter = teleport_counter.wrapping_add(1);
                teleports.write(TeleportMessage {
                    guid,
                    counter: *teleport_counter,
                    position: [to.x, to.y, to.z],
                    orientation: hero.facing.y.atan2(hero.facing.x),
                });
            }
            Event::Downed { player } | Event::Died { player } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    let word = if matches!(event, Event::Died { .. }) {
                        "Dead"
                    } else {
                        "Down!"
                    };
                    fx.text(e, word.into(), COLOR_HURT);
                }
            }
            Event::Revived { player, .. } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    fx.text(e, "Revived".into(), COLOR_EVADE);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every refusal's place in [`ALL`]; written without a catch-all, so a new `Refusal` fails to
    /// compile here until it is added to the list and so to the test below.
    fn place(why: Refusal) -> usize {
        match why {
            Refusal::Down => 0,
            Refusal::Busy => 1,
            Refusal::NotReady => 2,
            Refusal::NoPower => 3,
            Refusal::Moving => 4,
            Refusal::NeedStealth => 5,
            Refusal::InCombat => 6,
            Refusal::NeedCombo => 7,
            Refusal::NoTarget => 8,
            Refusal::BadTarget => 9,
            Refusal::Aware => 10,
            Refusal::OutOfRange => 11,
            Refusal::Facing => 12,
            Refusal::Behind => 13,
            Refusal::NotInSight => 14,
        }
    }

    const ALL: [Refusal; 15] = [
        Refusal::Down,
        Refusal::Busy,
        Refusal::NotReady,
        Refusal::NoPower,
        Refusal::Moving,
        Refusal::NeedStealth,
        Refusal::InCombat,
        Refusal::NeedCombo,
        Refusal::NoTarget,
        Refusal::BadTarget,
        Refusal::Aware,
        Refusal::OutOfRange,
        Refusal::Facing,
        Refusal::Behind,
        Refusal::NotInSight,
    ];

    #[test]
    fn a_refusal_reads_as_wow_says_it() {
        assert_eq!(refusal_key(Refusal::NoPower, 0), "ERR_OUT_OF_MANA");
        assert_eq!(refusal_key(Refusal::NoPower, 3), "ERR_OUT_OF_ENERGY");
        assert_eq!(refusal_key(Refusal::Behind, 3), "SPELL_FAILED_NOT_BEHIND");
        assert_eq!(
            refusal_key(Refusal::Aware, 3),
            "SPELL_FAILED_TARGET_AFFECTING_COMBAT"
        );
    }

    #[test]
    fn no_refusal_is_silent() {
        for (i, why) in ALL.into_iter().enumerate() {
            assert_eq!(place(why), i, "{why:?} is out of order in ALL");
            for power in [0, 1, 3] {
                let key = refusal_key(why, power);
                assert!(
                    key.starts_with("SPELL_FAILED_") || key.starts_with("ERR_"),
                    "{why:?} has no GlobalStrings key: {key:?}"
                );
            }
        }
        let mut keys: Vec<&str> = ALL.iter().map(|&w| refusal_key(w, 3)).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ALL.len(), "two refusals read the same");
    }

    #[test]
    fn a_stopped_cast_ends_its_bar_its_way() {
        assert!(matches!(
            stop_edge(Stop::Finished),
            CastBarEdge::ChannelUpdate { remaining_ms: 0 }
        ));
        assert!(matches!(stop_edge(Stop::TargetLost), CastBarEdge::Failed));
        assert!(matches!(stop_edge(Stop::Moved), CastBarEdge::Interrupted));
        assert_eq!(ms(2.5), 2500);
        assert_eq!(ms(-1.0), 0);
    }

    #[test]
    fn a_channel_that_finishes_still_lets_go_of_its_hold() {
        // A channel releases at every end, its normal one too; any other cast only when it died.
        for why in [
            Stop::Finished,
            Stop::Moved,
            Stop::Cancelled,
            Stop::Interrupted,
        ] {
            assert!(releases_hold(true, why), "channel, {why:?}");
        }
        assert!(!releases_hold(false, Stop::Finished));
        assert!(releases_hold(false, Stop::TargetLost));
        assert!(releases_hold(false, Stop::Moved));
    }

    fn damage(to: UnitRef, by: Option<UnitRef>, amount: f32) -> Event {
        Event::Damage {
            to,
            by,
            amount,
            crit: false,
            ambush: false,
        }
    }

    #[test]
    fn a_swing_takes_the_damage_its_blow_dealt() {
        let (hero, enemy) = (UnitRef::Hero(0), UnitRef::Enemy(4));
        let events = [
            // A spell of ours lands first, then our swing and its blow.
            damage(enemy, Some(hero), 90.0),
            Event::Swing {
                by: hero,
                to: enemy,
            },
            damage(enemy, Some(hero), 33.0),
            // The enemy swings back.
            Event::Swing {
                by: enemy,
                to: hero,
            },
            damage(hero, Some(enemy), 12.0),
        ];
        assert_eq!(swing_blow(&events, 1), Some(2));
        assert_eq!(swing_blow(&events, 3), Some(4));
        assert_eq!(swing_blow(&events, 0), None, "a damage is no swing");
        // The spell's damage is a spell's, the swings' blows are melee.
        assert!(!is_blow(&events, 0));
        assert!(is_blow(&events, 2) && is_blow(&events, 4));
        assert_eq!(damage_of(&events[2]), 33.0);
    }

    #[test]
    fn a_swing_that_did_no_harm_is_a_miss_and_never_takes_the_next_swings_blow() {
        let (hero, enemy) = (UnitRef::Hero(0), UnitRef::Enemy(4));
        let swing = || Event::Swing {
            by: enemy,
            to: hero,
        };
        // The first swing found the hero down (no damage); the next landed.
        let events = [swing(), swing(), damage(hero, Some(enemy), 12.0)];
        assert_eq!(swing_blow(&events, 0), None);
        assert_eq!(swing_blow(&events, 1), Some(2));
        // Damage by someone else, or to someone else, is not this swing's.
        let events = [
            swing(),
            damage(hero, Some(UnitRef::Enemy(5)), 12.0),
            damage(UnitRef::Hero(1), Some(enemy), 12.0),
        ];
        assert_eq!(swing_blow(&events, 0), None);
    }
}
