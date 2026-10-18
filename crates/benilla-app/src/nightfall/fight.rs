//! The fight, shown (spec §6). [`show_events`] plays what happened each step through the engine's
//! own seams, as a server's packets would: a cast's precast glow and our cast bar, a channel's
//! bar, the cast's visual with its missile and impacts, a stopped cast, melee swings, floating
//! combat text, WoW's own red error text for a press the host turned down, and a Blink's teleport
//! for our hero. [`fields`] writes the fight's state into the units' fields: power, combo points,
//! the bar's cooldown sweeps, the ready stance and the root. It decides nothing: every event and
//! every number is the host's.

use std::collections::HashMap;
use std::time::Duration;

use benilla_protocol::messages::ObjectFields;
use benilla_protocol::MoveMode;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use nightfall_sim::{Event, PlayerId, Refusal, Stop, UnitRef, V2, V3};

use super::enemies::{enemy_guid, EnemyViews};
use super::fx::{Fx, COLOR_EVADE, COLOR_HIT, COLOR_HURT, COLOR_SPELL};
use super::run::Sim;
use super::units;
use super::view::Views;
use crate::creature_anim::Engaged;
use crate::net::{MoveModeMessage, ObjectStore, SelfGuid, TeleportMessage};
use crate::spell::Cooldowns;
use crate::ui_action::{CastErrors, UiError, UiErrorKeys};
use crate::ui_cast::{CastBarEdge, CastBarFeed};

/// `ClassDef::power` of a class whose power is mana.
const MANA: u8 = 0;

/// How a refusal is shown, the way WoW's client shows the same mistake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Route {
    /// A voiced row of the message catalog, by GlobalStrings key, through `UiErrorKeys`
    /// (`DisplayError`'s own message ids).
    Key(&'static str),
    /// A `SMSG_CAST_RESULT` failure reason of WoW 1.12, through `CastErrors`: the engine turns it
    /// into its text, the red line and the voice, as it does for the server's own.
    Wire(u8),
}

// WoW 1.12's `SpellCastResult` reasons (vmangos `SpellDefines.h`), as the engine's own table
// `ui_action/cast_fail.rs::CAST_FAIL_KEYS` indexes them. Hard-coded here (that table is private to
// the engine, which stays untouched); `every_wire_reason_is_its_row_in_the_engines_table` checks
// each against the engine's source.
const AFFECTING_COMBAT: u8 = 0x00;
const BAD_TARGETS: u8 = 0x0A;
const CASTER_DEAD: u8 = 0x13;
const LINE_OF_SIGHT: u8 = 0x2A;
const MOVING: u8 = 0x2E;
const NOT_BEHIND: u8 = 0x33;
const NO_COMBO_POINTS: u8 = 0x46;
const ONLY_STEALTHED: u8 = 0x57;
const OUT_OF_RANGE: u8 = 0x59;
const SPELL_IN_PROGRESS: u8 = 0x61;
const TARGET_AFFECTING_COMBAT: u8 = 0x66;
const UNIT_NOT_INFRONT: u8 = 0x7C;

/// The way a refusal is shown. Power, the lack of a target and a cooldown are catalog rows with a
/// voice. `NoPower` and `NotReady` go by the class's power, as the host's refusal names no spell
/// for the engine to look at: a mana class casts spells ("Spell is not ready yet."), an energy
/// class performs abilities ("Ability is not ready yet."), the two lines WoW's `0x3C` picks. Every
/// other is a cast-failure reason. The text of "Out of range." is the engine's own mapping of
/// [`OUT_OF_RANGE`]. Every refusal has a route: none is silent.
pub(super) fn refusal_route(why: Refusal, power: u8) -> Route {
    match why {
        Refusal::NoPower if power == MANA => Route::Key("ERR_OUT_OF_MANA"),
        Refusal::NoPower => Route::Key("ERR_OUT_OF_ENERGY"),
        Refusal::NoTarget => Route::Key("ERR_GENERIC_NO_TARGET"),
        Refusal::NotReady if power == MANA => Route::Key("ERR_SPELL_COOLDOWN"),
        Refusal::NotReady => Route::Key("ERR_ABILITY_COOLDOWN"),
        Refusal::Down => Route::Wire(CASTER_DEAD),
        Refusal::Busy => Route::Wire(SPELL_IN_PROGRESS),
        Refusal::Moving => Route::Wire(MOVING),
        Refusal::NeedStealth => Route::Wire(ONLY_STEALTHED),
        Refusal::InCombat => Route::Wire(AFFECTING_COMBAT),
        Refusal::NeedCombo => Route::Wire(NO_COMBO_POINTS),
        Refusal::BadTarget => Route::Wire(BAD_TARGETS),
        Refusal::Aware => Route::Wire(TARGET_AFFECTING_COMBAT),
        Refusal::OutOfRange => Route::Wire(OUT_OF_RANGE),
        Refusal::Facing => Route::Wire(UNIT_NOT_INFRONT),
        Refusal::Behind => Route::Wire(NOT_BEHIND),
        Refusal::NotInSight => Route::Wire(LINE_OF_SIGHT),
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

/// Yards a Blink must move the hero to be shown as a teleport.
const BLINK_MIN: f32 = 0.05;

/// Whether a Blink that ended at `to` moves a hero standing at `from`: one a wall stopped at once
/// ends where it began, and snapping the controller there would only jolt it.
pub(super) fn blink_moves(from: V2, to: V3) -> bool {
    from.distance(to.ground()) >= BLINK_MIN
}

/// Whether a stopped cast must release its caster's precast pose and glow. A cast that stopped
/// before going off never did; a channel holds its pose to the end, and its ticks go off on
/// another spell, so even its normal finish leaves the hold up. (A `Fail` is a quiet release,
/// with no flash.)
pub(super) fn releases_hold(channel: bool, why: Stop) -> bool {
    channel || why != Stop::Finished
}

/// Which damages in a frame's events are melee blows, and which damage each swing dealt, found in
/// one pass ([`pair_blows`]).
pub(super) struct Blows {
    /// At a swing's index, the index of the `Damage` it dealt; `None` elsewhere.
    dealt: Vec<Option<usize>>,
    /// Whether the event at an index is some swing's damage.
    blow: Vec<bool>,
}

impl Blows {
    /// The `Damage` a melee `Swing` at `events[swing]` dealt: the next damage to its victim by its
    /// attacker, before the same pair swings again (the sim pushes a blow's damage right after its
    /// swing). `None` for any other event, or a swing that did none, which WoW's portrait reads as
    /// a miss.
    pub(super) fn swing_blow(&self, swing: usize) -> Option<usize> {
        self.dealt.get(swing).copied().flatten()
    }

    /// Whether `events[damage]` is the damage of a melee blow (some swing's), not a spell's.
    pub(super) fn is_blow(&self, damage: usize) -> bool {
        self.blow.get(damage).copied().unwrap_or(false)
    }
}

/// A unit as a map key: [`UnitRef`] is not hashable.
fn unit_key(unit: UnitRef) -> (bool, u32) {
    match unit {
        UnitRef::Hero(p) => (false, u32::from(p)),
        UnitRef::Enemy(id) => (true, id),
    }
}

/// Pairs the frame's swings with their damage in one pass over `events`: each (attacker, victim)
/// remembers its last swing still without damage, the next damage of that pair is its blow, and a
/// swing by the same pair before any damage did none. (A scan of the events per damage would be
/// quadratic: a stalled client has thousands pending.)
pub(super) fn pair_blows(events: &[Event]) -> Blows {
    let mut blows = Blows {
        dealt: vec![None; events.len()],
        blow: vec![false; events.len()],
    };
    let mut waiting: HashMap<((bool, u32), (bool, u32)), usize> = HashMap::new();
    for (i, event) in events.iter().enumerate() {
        match event {
            Event::Swing { by, to } => {
                waiting.insert((unit_key(*by), unit_key(*to)), i);
            }
            Event::Damage {
                to, by: Some(by), ..
            } => {
                if let Some(swing) = waiting.remove(&(unit_key(*by), unit_key(*to))) {
                    blows.dealt[swing] = Some(i);
                    blows.blow[i] = true;
                }
            }
            _ => {}
        }
    }
    blows
}

/// [`Blows::swing_blow`] for tests that hold only the events.
#[cfg(test)]
pub(super) fn swing_blow(events: &[Event], swing: usize) -> Option<usize> {
    pair_blows(events).swing_blow(swing)
}

/// [`Blows::is_blow`] for tests that hold only the events.
#[cfg(test)]
pub(super) fn is_blow(events: &[Event], damage: usize) -> bool {
    pair_blows(events).is_blow(damage)
}

/// The amount of a `Damage` event; 0 for any other.
fn damage_of(event: &Event) -> f32 {
    match event {
        Event::Damage { amount, .. } => *amount,
        _ => 0.0,
    }
}

/// `UNIT_FIELD_POWER1` (the first of five power slots: mana, rage, focus, energy, happiness) and
/// `UNIT_FIELD_MAXPOWER1`.
const FIELD_POWER1: u16 = 23;
const FIELD_MAXPOWER1: u16 = 29;
/// `PLAYER_FIELD_BYTES`, whose byte 1 is the combo points, and `PLAYER_FIELD_COMBO_TARGET`, the
/// guid they sit on (two words).
const FIELD_PLAYER_BYTES: u16 = 1222;
const FIELD_COMBO_TARGET: u16 = 714;

/// A hero's power of type `ty` as its fields hold it.
pub(super) fn power_pairs(ty: u8, now: f32, max: f32) -> [(u16, u32); 2] {
    [
        (FIELD_POWER1 + u16::from(ty), now.max(0.0).round() as u32),
        (FIELD_MAXPOWER1 + u16::from(ty), max.max(0.0).round() as u32),
    ]
}

/// `PLAYER_FIELD_BYTES` with `combo` points in byte 1 and the action bar `toggles` in byte 2,
/// as they were; byte 0's flags (a ghost's release timer, tracking stealth) a hero never has.
pub(super) fn combo_bytes(toggles: u8, combo: u8) -> u32 {
    u32::from(toggles) << 16 | u32::from(combo) << 8
}

/// Each spell on the bar and the seconds it still waits: its own cooldown, or the global one
/// for an ability that starts it; a spell on two slots (Stealth on both bars) once.
pub(super) fn waits(
    kit: &[(u8, usize)],
    abilities: &[nightfall_sim::AbilityDef],
    f: &nightfall_sim::HeroFight,
) -> Vec<(u32, f32)> {
    let mut out: Vec<(u32, f32)> = Vec::new();
    for &(_, a) in kit {
        let own = f
            .cooldowns
            .iter()
            .find(|c| usize::from(c.ability) == a)
            .map_or(0.0, |c| c.left);
        let gcd = if abilities[a].gcd > 0.0 { f.gcd } else { 0.0 };
        let spell = abilities[a].spell;
        let wait = own.max(gcd);
        match out.iter_mut().find(|(s, _)| *s == spell) {
            Some(w) => w.1 = w.1.max(wait),
            None => out.push((spell, wait)),
        }
    }
    out
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
    mut cast_errors: ResMut<CastErrors>,
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
    let blows = pair_blows(&sim.events);
    for (i, event) in sim.events.iter().enumerate() {
        match event {
            Event::CastStart {
                by, ability, secs, ..
            } => {
                // An ability index the content lacks (a mismatched host) shows nothing.
                let Some(spell) = content
                    .abilities
                    .get(usize::from(*ability))
                    .map(|a| a.spell)
                else {
                    continue;
                };
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
                let Some(spell) = content
                    .abilities
                    .get(usize::from(*ability))
                    .map(|a| a.spell)
                else {
                    continue;
                };
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
                let Some(a) = content.abilities.get(usize::from(*ability)) else {
                    continue;
                };
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
                let Some(a) = content.abilities.get(usize::from(*ability)) else {
                    continue;
                };
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
                } else if matches!(by, Some(UnitRef::Hero(_))) && !blows.is_blow(i) {
                    COLOR_SPELL
                } else {
                    COLOR_HIT
                };
                fx.number(e, *amount, *crit, color);
            }
            Event::Swing { by, to } => {
                if let (Some(a), Some(v)) = (body(*by), body(*to)) {
                    let dealt = blows
                        .swing_blow(i)
                        .map_or(0.0, |d| damage_of(&sim.events[d]));
                    fx.swing(a, v, dealt);
                }
            }
            Event::Evaded { player, .. } => {
                if let Some(e) = body(UnitRef::Hero(*player)) {
                    fx.text(e, "Evade".into(), COLOR_EVADE);
                }
            }
            Event::Refused { player, why } if *player == sim.me => {
                match refusal_route(*why, power) {
                    Route::Key(key) => errors.0.push(UiError::key(key)),
                    // Spell 0: the refusal names no ability, so there is no combat-log line.
                    Route::Wire(reason) => cast_errors.push_local(0, reason),
                }
            }
            Event::Blinked { player, to } if *player == sim.me => {
                let (Some(guid), Some(hero)) = (self_guid.0, sim.world.hero(sim.me)) else {
                    continue;
                };
                // A Blink the walls stopped at once ends where it began: nothing to snap to.
                if !blink_moves(hero.pos, *to) {
                    continue;
                }
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

/// What `fields` last wrote, so it writes only changes.
#[derive(Default)]
pub(super) struct Written {
    /// The body of our hero these were written for: another (a new run) starts them all over.
    body: Option<Entity>,
    power: HashMap<PlayerId, (Entity, [(u16, u32); 2])>,
    combo: Option<(u8, Option<u32>)>,
    /// When each bar spell's sweep ends, as written into the engine's cooldowns.
    ready_at: HashMap<u32, Instant>,
    attacking: Option<u32>,
    rooted: bool,
    counter: u32,
}

/// The guid enemy `id` is drawn under, if the world has it.
fn target_guid(sim: &Sim, id: Option<u32>) -> Option<u64> {
    let id = id?;
    sim.world.enemy(id).map(|e| enemy_guid(e.kind, id))
}

/// How far a sweep's end may drift before it is written again: the sim's steps and the frames
/// do not tick in lockstep, and a sweep restarted every frame would flicker.
const SWEEP_SLACK: Duration = Duration::from_millis(150);

/// Every hero's power into their fields (the player, party and target frames); and for our hero:
/// combo points on their target (the combo frame), the bar's cooldown sweeps (written into the
/// engine's own cooldowns when they change by more than a blink), the ready stance while
/// auto-attacking (`Engaged`), and WoW's root while a rooted cast or death holds their feet.
pub(super) fn fields(
    mut commands: Commands,
    sim: Res<Sim>,
    views: Res<Views>,
    self_guid: Res<SelfGuid>,
    mut stores: Query<&mut ObjectStore>,
    mut cooldowns: ResMut<Cooldowns>,
    mut modes: MessageWriter<MoveModeMessage>,
    mut written: Local<Written>,
) {
    let content = sim.world.content();
    let body = views.heroes.get(&sim.me).copied();
    if written.body != body {
        // A new body (a new run) has none of it: nothing of the last is still written.
        let counter = written.counter;
        *written = Written {
            body,
            counter,
            ..Written::default()
        };
    }
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let ty = content.classes[hero.class].power;
        let pairs = power_pairs(ty, hero.fight.power, hero.fight.max_power);
        if written.power.get(&hero.player) == Some(&(e, pairs)) {
            continue;
        }
        if let Ok(mut store) = stores.get_mut(e) {
            store.0.merge(ObjectFields::from_pairs(&pairs));
            written.power.insert(hero.player, (e, pairs));
        }
    }
    let (Some(me), Some(body)) = (sim.world.hero(sim.me), body) else {
        return;
    };
    let f = &me.fight;
    if written.combo != Some((f.combo, f.combo_on)) {
        if let Ok(mut store) = stores.get_mut(body) {
            let toggles = store.0.player_action_bar_toggles().unwrap_or(0);
            let on = target_guid(&sim, f.combo_on).unwrap_or(0);
            store.0.merge(ObjectFields::from_pairs(&[
                (FIELD_PLAYER_BYTES, combo_bytes(toggles, f.combo)),
                (FIELD_COMBO_TARGET, on as u32),
                (FIELD_COMBO_TARGET + 1, (on >> 32) as u32),
            ]));
            written.combo = Some((f.combo, f.combo_on));
        }
    }
    let now = Instant::now();
    for (spell, wait) in waits(&content.classes[me.class].kit, &content.abilities, f) {
        let end = (wait > 0.05).then(|| now + Duration::from_secs_f32(wait));
        let old = written.ready_at.get(&spell).copied();
        let moved = match (old, end) {
            (None, None) => false,
            (Some(a), Some(b)) => (if a > b { a - b } else { b - a }) > SWEEP_SLACK,
            _ => true,
        };
        if !moved {
            continue;
        }
        cooldowns.clear_spell(spell);
        match end {
            Some(end) => {
                cooldowns.apply_wire_cooldown(spell, ms(wait), None, now);
                written.ready_at.insert(spell, end);
            }
            None => {
                written.ready_at.remove(&spell);
            }
        }
    }
    if written.attacking != f.attacking {
        // An enemy not in the world (yet) is waited for, not engaged with nothing: each frame
        // looks for it again (a lookup), and only when it is there is anything queued. A stance
        // already written is taken down once, never once a frame.
        match target_guid(&sim, f.attacking) {
            Some(guid) => {
                commands.entity(body).insert(Engaged(guid));
                written.attacking = f.attacking;
            }
            None if written.attacking.is_some() => {
                commands.entity(body).remove::<Engaged>();
                written.attacking = None;
            }
            None => {}
        }
    }
    if written.rooted != f.rooted {
        if let Some(guid) = self_guid.0 {
            written.counter = written.counter.wrapping_add(1);
            modes.write(MoveModeMessage {
                guid,
                counter: written.counter,
                mode: MoveMode::Root,
                apply: f.rooted,
            });
            written.rooted = f.rooted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_action::CastFail;

    /// Every refusal's place in [`READS`]; written without a catch-all, so a new `Refusal` fails to
    /// compile here until it is added to that list.
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

    /// What every refusal reads as, by the GlobalStrings key of WoW's own line: the catalog key it
    /// is shown by, or the key of its cast-failure reason's row in the engine's table.
    const READS: [(Refusal, &str); 15] = [
        (Refusal::Down, "SPELL_FAILED_CASTER_DEAD"),
        (Refusal::Busy, "SPELL_FAILED_SPELL_IN_PROGRESS"),
        (Refusal::NotReady, "ERR_ABILITY_COOLDOWN"),
        (Refusal::NoPower, "ERR_OUT_OF_ENERGY"),
        (Refusal::Moving, "SPELL_FAILED_MOVING"),
        (Refusal::NeedStealth, "SPELL_FAILED_ONLY_STEALTHED"),
        (Refusal::InCombat, "SPELL_FAILED_AFFECTING_COMBAT"),
        (Refusal::NeedCombo, "SPELL_FAILED_NO_COMBO_POINTS"),
        (Refusal::NoTarget, "ERR_GENERIC_NO_TARGET"),
        (Refusal::BadTarget, "SPELL_FAILED_BAD_TARGETS"),
        (Refusal::Aware, "SPELL_FAILED_TARGET_AFFECTING_COMBAT"),
        (Refusal::OutOfRange, "SPELL_FAILED_OUT_OF_RANGE"),
        (Refusal::Facing, "SPELL_FAILED_UNIT_NOT_INFRONT"),
        (Refusal::Behind, "SPELL_FAILED_NOT_BEHIND"),
        (Refusal::NotInSight, "SPELL_FAILED_LINE_OF_SIGHT"),
    ];

    /// The engine's cast-failure table, as source: its rows read `"KEY", // 0xNN`.
    const ENGINE_TABLE: &str = include_str!("../ui_action/cast_fail.rs");

    /// The key the engine's table has at wire reason `reason`.
    fn engine_key(reason: u8) -> Option<&'static str> {
        let tag = format!("// {reason:#04x}");
        ENGINE_TABLE.lines().find_map(|l| {
            let l = l.trim();
            let key = l.strip_prefix('"')?.split('"').next()?;
            (l.ends_with(&tag) && key.starts_with("SPELL_FAILED_")).then_some(key)
        })
    }

    #[test]
    fn no_refusal_is_silent() {
        for (i, (why, key)) in READS.into_iter().enumerate() {
            assert_eq!(place(why), i, "{why:?} is out of order in READS");
            match refusal_route(why, 3) {
                Route::Key(k) => {
                    assert_eq!(k, key, "{why:?}");
                    assert!(
                        benilla_ui::messages::by_key(k).is_some(),
                        "{why:?}: {k} is not a voiced catalog row, so the engine would warn"
                    );
                }
                Route::Wire(reason) => {
                    assert_eq!(
                        engine_key(reason),
                        Some(key),
                        "{why:?}: reason {reason:#04x} is not {key} in the engine's table"
                    );
                }
            }
        }
        // The two powers.
        assert_eq!(
            refusal_route(Refusal::NoPower, 0),
            Route::Key("ERR_OUT_OF_MANA")
        );
        assert_eq!(
            refusal_route(Refusal::NoPower, 3),
            Route::Key("ERR_OUT_OF_ENERGY")
        );
        for key in [
            "ERR_OUT_OF_MANA",
            "ERR_OUT_OF_ENERGY",
            "ERR_SPELL_COOLDOWN",
            "ERR_ABILITY_COOLDOWN",
        ] {
            assert!(benilla_ui::messages::by_key(key).is_some(), "{key}");
        }
    }

    #[test]
    fn a_mana_class_reads_a_spell_cooldown() {
        assert_eq!(
            refusal_route(Refusal::NotReady, 0),
            Route::Key("ERR_SPELL_COOLDOWN")
        );
    }

    #[test]
    fn an_energy_class_reads_an_ability_cooldown() {
        assert_eq!(
            refusal_route(Refusal::NotReady, 3),
            Route::Key("ERR_ABILITY_COOLDOWN")
        );
    }

    #[test]
    fn range_reads_out_of_range_and_the_engine_maps_it_so() {
        assert_eq!(refusal_route(Refusal::OutOfRange, 0), Route::Wire(0x59));
        // The engine gives reason 0x59 its own text, "Out of range.", ahead of the table's.
        // (Found loosely, so a line wrap in the engine's source does not break this.)
        let lines: Vec<&str> = ENGINE_TABLE.lines().collect();
        assert!(
            lines.iter().enumerate().any(|(i, l)| {
                l.contains("0x59 =>")
                    && lines[i..(i + 4).min(lines.len())]
                        .iter()
                        .any(|n| n.contains("ERR_SPELL_OUT_OF_RANGE"))
            }),
            "0x59 no longer reads ERR_SPELL_OUT_OF_RANGE"
        );
    }

    #[test]
    fn a_swing_shows_the_damage_the_number_does() {
        // 0.4 of damage reads 1 in the number; the portrait agrees, and no damage reads MISS.
        let mut stage = Stage::new(vec![
            Event::Swing {
                by: UnitRef::Hero(0),
                to: UnitRef::Enemy(4),
            },
            damage(UnitRef::Enemy(4), Some(UnitRef::Hero(0)), 0.4),
        ]);
        stage.show();
        let dealt: Vec<u32> = stage
            .drain::<crate::creature_anim::SwingMessage>()
            .iter()
            .map(|s| s.damage)
            .collect();
        assert_eq!(dealt, [1]);
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

    /// A world with one mage (player 0), one drawn enemy (id 4), the `events` waiting to be shown
    /// and every resource `show_events` writes to; `show` runs it once.
    struct Stage {
        world: bevy::prelude::World,
        hero: Entity,
        enemy: Entity,
    }

    impl Stage {
        fn new(events: Vec<Event>) -> Self {
            let content = nightfall_sim::Content::from_ron(nightfall_sim::ContentSources {
                classes: r#"[(key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 0,
                    base_hp: 1000.0, speed: 7.0, power_max: 100.0, bar: [(1, "ray")])]"#,
                map: r#"(key: "t", name: "T", map: 0,
                    bounds: [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)],
                    entries: [(5.0, 5.0, 0.0)])"#,
                rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                    fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                    hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
                enemies: "[]",
                spawns: "[]",
                abilities: r#"[
                    (key: "ray", name: "Ray", spell: 11, tick_spell: Some(12), aim: Enemy,
                     range: 30.0, channel: 3.0, ticks: 3, effects: [Damage(min: 1.0, max: 2.0)]),
                ]"#,
            })
            .unwrap();
            let mut sim = nightfall_sim::World::new(std::sync::Arc::new(content));
            sim.add_hero(0, "mage", "Test", 1, 0).unwrap();
            let mut sim = Sim::for_test(sim);
            sim.events = events;
            let mut world = bevy::prelude::World::new();
            let (hero, enemy) = (world.spawn_empty().id(), world.spawn_empty().id());
            let mut views = Views::default();
            views.heroes.insert(0, hero);
            let mut enemy_views = EnemyViews::default();
            enemy_views.drawn.insert(4, enemy);
            world.insert_resource(sim);
            world.insert_resource(views);
            world.insert_resource(enemy_views);
            world.insert_resource(SelfGuid(Some(77)));
            world.init_resource::<crate::creature_anim::PlaySeq>();
            world.init_resource::<Messages<crate::creature_anim::CastEvent>>();
            world.init_resource::<Messages<crate::creature_anim::SpellGoTargets>>();
            world.init_resource::<Messages<crate::creature_anim::SwingMessage>>();
            world.init_resource::<Messages<crate::combat_text::CombatTextSpawn>>();
            world.init_resource::<Messages<TeleportMessage>>();
            world.init_resource::<CastBarFeed>();
            world.init_resource::<UiErrorKeys>();
            world.init_resource::<CastErrors>();
            Self { world, hero, enemy }
        }

        fn show(&mut self) {
            use bevy::ecs::system::RunSystemOnce;
            self.world.run_system_once(show_events).unwrap();
        }

        fn drain<M: Message>(&mut self) -> Vec<M> {
            self.world.resource_mut::<Messages<M>>().drain().collect()
        }
    }

    #[test]
    fn a_blink_snaps_our_controller_unless_a_wall_stopped_it_at_once() {
        let at = |s: &Stage| s.world.resource::<Sim>().world.hero(0).unwrap().pos;
        let mut stage = Stage::new(vec![]);
        let from = at(&stage);
        let far = V3::new(from.x + 10.0, from.y, 0.0);
        let same = V3::new(from.x + 0.01, from.y, 0.0);
        assert!(blink_moves(from, far));
        assert!(!blink_moves(from, same));
        stage.world.resource_mut::<Sim>().events = vec![
            Event::Blinked { player: 0, to: far },
            Event::Blinked {
                player: 0,
                to: same,
            },
            // Another hero's Blink is theirs to snap.
            Event::Blinked { player: 1, to: far },
        ];
        stage.show();
        let snaps: Vec<([f32; 3], u32, u64)> = stage
            .drain::<TeleportMessage>()
            .iter()
            .map(|t| (t.position, t.counter, t.guid))
            .collect();
        assert_eq!(snaps, [([far.x, far.y, far.z], 1, 77)]);
    }

    #[test]
    fn a_channel_that_finishes_ends_with_a_quiet_release_of_its_hold() {
        use crate::creature_anim::{CastEvent, CastEventKind};
        let by = UnitRef::Hero(0);
        let mut stage = Stage::new(vec![
            Event::ChannelStart {
                by,
                ability: 0,
                target: None,
                secs: 3.0,
            },
            Event::CastGo {
                by,
                ability: 0,
                hits: vec![],
                at: None,
            },
            Event::CastStop {
                by,
                ability: 0,
                why: Stop::Finished,
            },
        ]);
        stage.show();
        let casts: Vec<(CastEventKind, u32)> = stage
            .drain::<CastEvent>()
            .iter()
            .map(|c| (c.kind, c.spell_id))
            .collect();
        // The hold is on the channel's own spell (11), its tick plays another (12), and the end
        // lets the hold go.
        assert_eq!(
            casts,
            [
                (CastEventKind::Start, 11),
                (CastEventKind::Go, 12),
                (CastEventKind::Fail, 11)
            ]
        );
    }

    #[test]
    fn our_refusals_reach_wows_error_text_each_by_its_route() {
        let refused = |player, why| Event::Refused { player, why };
        let mut stage = Stage::new(vec![
            refused(0, Refusal::NoPower),
            refused(0, Refusal::NotReady),
            refused(0, Refusal::OutOfRange),
            refused(0, Refusal::Down),
            // Someone else's refusal is not ours to show.
            refused(1, Refusal::OutOfRange),
        ]);
        stage.show();
        let keys = &stage.world.resource::<UiErrorKeys>().0;
        assert_eq!(
            keys,
            &[
                UiError::key("ERR_OUT_OF_MANA"),
                UiError::key("ERR_SPELL_COOLDOWN")
            ]
        );
        assert_eq!(
            stage.world.resource::<CastErrors>().0,
            [CastFail::local(0, 0x59), CastFail::local(0, 0x13)]
        );
    }

    #[test]
    fn a_swing_shows_its_blows_damage_and_numbers_are_white_for_melee_yellow_for_spells() {
        use crate::combat_text::CombatTextSpawn;
        use crate::creature_anim::SwingMessage;
        let (hero, enemy) = (UnitRef::Hero(0), UnitRef::Enemy(4));
        let mut stage = Stage::new(vec![
            damage(enemy, Some(hero), 90.0),
            Event::Swing {
                by: hero,
                to: enemy,
            },
            damage(enemy, Some(hero), 33.0),
            Event::Swing {
                by: enemy,
                to: hero,
            },
            damage(hero, Some(enemy), 12.4),
            // A swing that did no harm.
            Event::Swing {
                by: enemy,
                to: hero,
            },
        ]);
        stage.show();
        let swings: Vec<(Entity, u32)> = stage
            .drain::<SwingMessage>()
            .iter()
            .map(|s| (s.attacker, s.damage))
            .collect();
        assert_eq!(
            swings,
            [(stage.hero, 33), (stage.enemy, 12), (stage.enemy, 0)]
        );
        let numbers: Vec<(String, Option<u32>)> = stage
            .drain::<CombatTextSpawn>()
            .into_iter()
            .map(|t| (t.text, t.color))
            .collect();
        assert_eq!(
            numbers,
            [
                ("90".to_string(), Some(COLOR_SPELL)),
                ("33".to_string(), Some(COLOR_HIT)),
                ("12".to_string(), Some(COLOR_HURT)),
            ]
        );
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
    fn swings_of_several_pairs_in_one_frame_each_take_their_own_blow() {
        let (hero, a, b) = (UnitRef::Hero(0), UnitRef::Enemy(4), UnitRef::Enemy(5));
        let events = [
            Event::Swing { by: a, to: hero },
            Event::Swing { by: b, to: hero },
            damage(hero, Some(b), 7.0),
            damage(hero, Some(a), 9.0),
            // A spell of a third party's, and a second blow of b's with no swing before it.
            damage(hero, Some(UnitRef::Enemy(6)), 3.0),
            damage(hero, Some(b), 5.0),
        ];
        let blows = pair_blows(&events);
        assert_eq!(blows.swing_blow(0), Some(3));
        assert_eq!(blows.swing_blow(1), Some(2));
        assert!(blows.is_blow(2) && blows.is_blow(3));
        assert!(!blows.is_blow(4) && !blows.is_blow(5));
        assert!(!blows.is_blow(99) && blows.swing_blow(99).is_none());
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

    #[test]
    fn power_and_combo_points_go_in_their_fields() {
        assert_eq!(power_pairs(3, 72.4, 100.0), [(26, 72), (32, 100)]);
        assert_eq!(power_pairs(0, 1300.0, 1300.0), [(23, 1300), (29, 1300)]);
        assert_eq!(combo_bytes(3, 4), 0x0003_0400);
    }

    #[test]
    fn a_button_waits_out_its_cooldown_or_the_global_one() {
        use nightfall_sim::{Cooldown, HeroFight};
        let c = super::super::bar_test_content();
        let kit = &c.classes[0].kit;
        let f = HeroFight {
            gcd: 1.0,
            cooldowns: vec![Cooldown {
                ability: 2,
                left: 15.0,
            }],
            ..HeroFight::default()
        };
        // Stealth (no global cooldown), Strike (the global one), Stone (its own 15 s).
        assert_eq!(
            waits(kit, &c.abilities, &f),
            vec![(1785, 0.0), (1760, 1.0), (4164, 15.0)]
        );
    }

    /// A rogue (player 0, power energy) on the shipped content, an enemy (id 7, kind 0) in the
    /// world, and everything `fields` writes to.
    fn rogue_stage() -> (bevy::prelude::World, Entity) {
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        sim.enemies.push(nightfall_sim::Enemy {
            id: 7,
            kind: 0,
            pos: V2::new(0.0, 0.0),
            z: 0.0,
            facing: V2::new(1.0, 0.0),
            hp: 10.0,
            max_hp: 10.0,
            speed: 0.0,
            asleep: false,
            fight: Default::default(),
        });
        let mut world = bevy::prelude::World::new();
        let body = world.spawn(ObjectStore(ObjectFields::from_pairs(&[]))).id();
        let mut views = Views::default();
        views.heroes.insert(0, body);
        world.insert_resource(Sim::for_test(sim));
        world.insert_resource(views);
        world.insert_resource(SelfGuid(Some(77)));
        world.init_resource::<Cooldowns>();
        world.init_resource::<Messages<MoveModeMessage>>();
        (world, body)
    }

    fn fight_of(world: &mut bevy::prelude::World) -> &mut nightfall_sim::HeroFight {
        &mut world.resource_mut::<Sim>().into_inner().world.heroes[0].fight
    }

    fn changed_stores(world: &mut bevy::prelude::World) -> usize {
        world
            .query_filtered::<Entity, Changed<ObjectStore>>()
            .iter(world)
            .count()
    }

    /// How long a spell's sweep on the bar still has to run, milliseconds.
    fn sweep(world: &bevy::prelude::World, spell: u32) -> u32 {
        world
            .resource::<Cooldowns>()
            .info(spell, 0, None, Instant::now())
            .remaining_ms
    }

    fn moves(world: &mut bevy::prelude::World) -> Vec<(u64, u32, bool)> {
        world
            .resource_mut::<Messages<MoveModeMessage>>()
            .drain()
            .map(|m| (m.guid, m.counter, m.apply))
            .collect()
    }

    #[test]
    fn our_hero_shows_power_combo_points_and_sweeps_and_a_steady_state_writes_nothing() {
        let (mut world, body) = rogue_stage();
        let run = world.register_system(fields);
        let (gouge, gouge_spell) = {
            let c = world.resource::<Sim>().world.content().clone();
            let i = c.abilities.iter().position(|a| a.key == "gouge").unwrap();
            (i, c.abilities[i].spell)
        };
        {
            let f = fight_of(&mut world);
            f.power = 72.4;
            f.max_power = 100.0;
            f.combo = 3;
            f.combo_on = Some(7);
            f.cooldowns = vec![nightfall_sim::Cooldown {
                ability: gouge as u16,
                left: 8.0,
            }];
        }
        world.run_system(run).unwrap();
        let s = &world.get::<ObjectStore>(body).unwrap().0;
        assert_eq!(s.unit_power(3), Some(72), "energy");
        assert_eq!(s.unit_max_power(3), Some(100));
        assert_eq!(s.player_combo_points(), Some(3));
        assert_eq!(
            s.player_combo_target(),
            enemy_guid(0, 7),
            "the points sit on the enemy's own guid"
        );
        let swept = sweep(&world, gouge_spell);
        assert!((7_000..=8_000).contains(&swept), "{swept}");

        // The second frame: the same state writes nothing at all.
        world.clear_trackers();
        let generation = world.resource::<Cooldowns>().generation;
        world.run_system(run).unwrap();
        assert_eq!(changed_stores(&mut world), 0);
        assert_eq!(world.resource::<Cooldowns>().generation, generation);
        assert!(moves(&mut world).is_empty());

        // A spent cooldown clears its sweep, and the points are spent.
        {
            let f = fight_of(&mut world);
            f.cooldowns.clear();
            f.combo = 0;
            f.combo_on = None;
        }
        world.run_system(run).unwrap();
        assert_eq!(sweep(&world, gouge_spell), 0);
        let s = &world.get::<ObjectStore>(body).unwrap().0;
        assert_eq!(
            (s.player_combo_points(), s.player_combo_target()),
            (Some(0), 0)
        );
    }

    #[test]
    fn the_global_cooldown_sweeps_the_strikes_not_the_spells_that_ignore_it() {
        let (mut world, _) = rogue_stage();
        let run = world.register_system(fields);
        let c = world.resource::<Sim>().world.content().clone();
        let bar = c.classes[0].kit.iter().map(|&(_, a)| &c.abilities[a]);
        let strike = bar.clone().find(|a| a.gcd > 0.0).unwrap();
        let free = bar.clone().find(|a| a.gcd == 0.0).unwrap();
        fight_of(&mut world).gcd = 1.0;
        world.run_system(run).unwrap();
        assert!(sweep(&world, strike.spell) > 500, "{}", strike.key);
        assert_eq!(sweep(&world, free.spell), 0, "{}", free.key);
    }

    #[test]
    fn a_root_and_the_ready_stance_come_once_and_go_once() {
        let (mut world, body) = rogue_stage();
        let run = world.register_system(fields);
        fight_of(&mut world).rooted = true;
        fight_of(&mut world).attacking = Some(7);
        world.run_system(run).unwrap();
        world.run_system(run).unwrap();
        assert_eq!(
            moves(&mut world),
            [(77, 1, true)],
            "one root, not one a frame"
        );
        assert_eq!(
            world.get::<Engaged>(body).map(|e| e.0),
            Some(enemy_guid(0, 7))
        );

        fight_of(&mut world).rooted = false;
        fight_of(&mut world).attacking = None;
        world.run_system(run).unwrap();
        assert_eq!(moves(&mut world), [(77, 2, false)]);
        assert!(world.get::<Engaged>(body).is_none());
    }

    #[test]
    fn a_new_run_starts_the_writes_over() {
        let (mut world, first) = rogue_stage();
        let run = world.register_system(fields);
        let (gouge, gouge_spell) = {
            let c = world.resource::<Sim>().world.content().clone();
            let i = c.abilities.iter().position(|a| a.key == "gouge").unwrap();
            (i, c.abilities[i].spell)
        };
        {
            let f = fight_of(&mut world);
            f.power = 40.0;
            f.max_power = 100.0;
            f.rooted = true;
            f.attacking = Some(7);
            f.cooldowns = vec![nightfall_sim::Cooldown {
                ability: gouge as u16,
                left: 8.0,
            }];
        }
        world.run_system(run).unwrap();
        assert_eq!(moves(&mut world), [(77, 1, true)]);
        assert!(world.get::<Engaged>(first).is_some());
        assert!(sweep(&world, gouge_spell) > 5_000);

        // The next run's body is another entity, with none of it written; the engine's
        // cooldowns are the new run's too (empty).
        world.resource_mut::<Cooldowns>().clear_spell(gouge_spell);
        assert_eq!(sweep(&world, gouge_spell), 0);
        let second = world.spawn(ObjectStore(ObjectFields::from_pairs(&[]))).id();
        world.resource_mut::<Views>().heroes.insert(0, second);
        world.run_system(run).unwrap();
        assert_eq!(
            world.get::<ObjectStore>(second).unwrap().0.unit_power(3),
            Some(40)
        );
        assert!(
            sweep(&world, gouge_spell) > 5_000,
            "the new run's engine gets the sweep again"
        );
        assert_eq!(
            moves(&mut world),
            [(77, 2, true)],
            "and the root, as a second message"
        );
        assert_eq!(
            world.get::<Engaged>(second).map(|e| e.0),
            Some(enemy_guid(0, 7)),
            "and the ready stance"
        );
    }

    #[test]
    fn an_enemy_the_world_lacks_is_waited_for_not_cleared_every_frame() {
        let (mut world, body) = rogue_stage();
        let run = world.register_system(fields);
        fight_of(&mut world).attacking = Some(99);
        // A stance seeded on the body: waiting must not queue its removal.
        world.entity_mut(body).insert(Engaged(1));
        for _ in 0..3 {
            world.run_system(run).unwrap();
        }
        assert!(
            world.get::<Engaged>(body).is_some(),
            "nothing was queued to take the stance off while waiting"
        );
        world
            .resource_mut::<Sim>()
            .world
            .enemies
            .push(nightfall_sim::Enemy {
                id: 99,
                kind: 0,
                pos: V2::new(1.0, 0.0),
                z: 0.0,
                facing: V2::new(1.0, 0.0),
                hp: 10.0,
                max_hp: 10.0,
                speed: 0.0,
                asleep: false,
                fight: Default::default(),
            });
        world.run_system(run).unwrap();
        assert_eq!(
            world.get::<Engaged>(body).map(|e| e.0),
            Some(enemy_guid(0, 99)),
            "engaged the frame it appears"
        );
    }
}
