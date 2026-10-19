//! A run: the world stepped here (alone or hosting) or mirrored from the host (joined), the local
//! hero's pose fed to it, and the way back to the menu. Test hooks: `NIGHTFALL_HOST=1` hosts and
//! `NIGHTFALL_JOIN=<code or address>` joins once the menu is up; `NIGHTFALL_AUTOPLAY=1` starts a
//! run (hosting: once someone joined and every joiner is ready; a joiner's autoplay readies as
//! soon as its pick is in) and walks a slow circle; `NIGHTFALL_TRACE_POS=1` logs the
//! hero's WoW position every 2 seconds, for measuring entry points; `NIGHTFALL_FRAIL=<player>`
//! drops that hero 15 s into the run (the host's); `NIGHTFALL_AUTOPLAY_ABANDON=<seconds>` presses
//! Abandon Raid that many seconds in.

use std::collections::VecDeque;
use std::sync::OnceLock;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::messages::LobbyPlayer;
use nightfall_sim::{
    Act, ActPress, CastPress, Event, Gait, HeroOutcome, HeroPose, Mark, PlayerId, PlayerInput,
    RaidRoll, Snapshot, UnitRef, World, DT, V2, V3,
};

use super::menu::{LeaveRun, Selection, StartRequest};
use super::session::{self, Heard, Net, Role};
use super::stash::Stash;
use super::view::Views;
use super::{content, units, NightfallEntity, Phase};
use crate::glue::GlueClicks;

/// Snapshots a client holds before playing them: the delay that hides a late packet.
const PLAYBACK_DEPTH: usize = 2;
/// More than this waiting (a burst after a stall) and the oldest are played at once.
const PLAYBACK_MAX: usize = 8;

/// Acts waiting their turn at most: a click storm never grows without end.
const MAX_QUEUED: usize = 32;

/// The run being played. Present only during a run.
#[derive(Resource)]
pub(super) struct Sim {
    pub world: World,
    /// Our player: 0 alone or hosting, the host's pick when joined.
    pub me: PlayerId,
    /// Alone or hosting: real seconds not yet simulated. Joined: playback seconds since the last
    /// snapshot was applied.
    acc: f32,
    /// Seconds between the states the views glide between: a step, or a host's snapshot.
    span: f32,
    /// Joined: the host's snapshots not yet played, oldest first, each with what our hero sees
    /// in it, what its enemies think of our hero and what happened since the last.
    inbox: VecDeque<(Snapshot, Vec<u32>, Vec<Mark>, Vec<Event>)>,
    /// Joined: playback has begun.
    playing: bool,
    /// A step was taken (or a snapshot applied) this frame.
    pub stepped: bool,
    /// The enemies our hero sees (spec §4.3): worked out here alone or hosting, the host's word
    /// when joined. Sub-project 3 hides the others from the interface.
    pub seen: Vec<u32>,
    /// What the enemies think of our hero (spec §5.5): worked out here alone or hosting, the
    /// host's word when joined.
    pub marks: Vec<Mark>,
    /// What happened in the steps taken (or the states played) this frame, oldest first, for
    /// the fight's look.
    pub events: Vec<Event>,
    /// Hosting: how many of the world's warnings (content the map could not give, paths that found
    /// nothing) are in the log already; they keep coming while the run is stepped.
    warned: usize,
    /// Our outcome, once the host settled it, for the stash to take ([`super::stash`]).
    pub mine: Option<HeroOutcome>,
    /// Every hero's outcome, once the raid is over: the end screen shows while it is here.
    pub over: Option<Vec<HeroOutcome>>,
    /// Where this raid enters the map (WoW coordinates): the entry the host's roll names, which a
    /// joined client learns from the lobby's planned raid, so it starts where the host's heroes do.
    pub entry: [f32; 3],
}

/// Where the raid `roll` enters the map: its entry, or the first when there is no roll (a client that
/// never heard the host's plan) or it names an entry the map has not.
fn entry_of(roll: Option<&RaidRoll>) -> [f32; 3] {
    let content = content();
    let entries = &content.map.entries;
    roll.and_then(|r| entries.get(r.entry))
        .unwrap_or(&entries[0])
        .to_owned()
}

impl Sim {
    /// A run for everyone in `roster` (just us, alone), stepped here: the raid as the host (or we,
    /// alone) planned it in the inn.
    fn hosted(roster: &[LobbyPlayer], roll: RaidRoll) -> Self {
        let content = content();
        let mut world = World::new(content.clone());
        let entry = entry_of(Some(&roll));
        // The raid first, so the heroes start at its entry.
        // The standard roll fits the map the content shipped (the content test checks it); a
        // content folder that breaks that is told, not crashed on.
        if let Err(e) = world.begin_raid(roll) {
            debug_assert!(false, "nightfall: the raid could not begin: {e}");
            warn!("nightfall: the raid could not begin: {e}");
        }
        for p in roster {
            // A class the content lacks (a mismatched client) is left out rather than crash.
            if world
                .add_hero(
                    p.player,
                    &p.hero.class,
                    &p.hero.name,
                    p.hero.race,
                    p.hero.sex,
                )
                .is_ok()
            {
                let left = world.give_loadout(p.player, &p.hero.loadout);
                if !left.is_empty() {
                    warn!(
                        "nightfall: {} brought {} stacks with no room for them; left out",
                        p.hero.name,
                        left.len()
                    );
                }
            }
        }
        Self {
            world,
            me: 0,
            acc: 0.0,
            span: DT,
            inbox: VecDeque::new(),
            playing: false,
            stepped: false,
            seen: Vec::new(),
            marks: Vec::new(),
            events: Vec::new(),
            warned: 0,
            mine: None,
            over: None,
            entry,
        }
    }

    /// A joined run: an empty world, filled by the host's snapshots. `planned` is the raid the
    /// host told us in the lobby, which says where it enters the map.
    fn mirror(me: PlayerId, planned: Option<&RaidRoll>) -> Self {
        let mut world = World::new(content());
        // Until the host's first snapshot there is no loot to show: not the raid this world rolls.
        world.begin_mirror();
        Self {
            world,
            me,
            acc: 0.0,
            span: DT * session::SEND_EVERY as f32,
            inbox: VecDeque::new(),
            playing: false,
            stepped: false,
            seen: Vec::new(),
            marks: Vec::new(),
            events: Vec::new(),
            warned: 0,
            mine: None,
            over: None,
            entry: entry_of(planned),
        }
    }

    /// A hosted run around `world`, for tests of the systems that read it: nothing seen, no marks.
    #[cfg(test)]
    pub(super) fn for_test(world: World) -> Self {
        Self {
            world,
            me: 0,
            acc: 0.0,
            span: DT,
            inbox: VecDeque::new(),
            playing: false,
            stepped: false,
            seen: Vec::new(),
            marks: Vec::new(),
            events: Vec::new(),
            warned: 0,
            mine: None,
            over: None,
            entry: entry_of(None),
        }
    }

    /// Hosting: the world's warnings not yet in the log, once each.
    fn log_new_warnings(&mut self) {
        let all = self.world.warnings();
        for w in &all[self.warned.min(all.len())..] {
            warn!("nightfall: {w}");
        }
        self.warned = all.len();
    }

    /// How far between the last two states the present lies, 0..1.
    pub fn alpha(&self) -> f32 {
        (self.acc / self.span).clamp(0.0, 1.0)
    }

    /// Seconds between the states the views glide between.
    pub fn span(&self) -> f32 {
        self.span
    }

    /// Joined: the host's snapshots into the mirror at an even pace, a little behind (as
    /// Survivors' `Sim::play_back` does).
    fn play_back(&mut self, dt: f32) {
        while self.inbox.len() > PLAYBACK_MAX {
            self.apply_next();
        }
        if !self.playing {
            if self.inbox.len() < PLAYBACK_DEPTH {
                return;
            }
            self.playing = true;
            self.acc = self.span;
        }
        let depth = self.inbox.len() as f32 - PLAYBACK_DEPTH as f32;
        self.acc += dt * (1.0 + 0.1 * depth).clamp(0.85, 1.3);
        while self.acc >= self.span {
            if self.inbox.is_empty() {
                self.acc = self.span;
                break;
            }
            self.apply_next();
            self.acc -= self.span;
        }
    }

    fn apply_next(&mut self) {
        if let Some((snapshot, seen, marks, events)) = self.inbox.pop_front() {
            self.world.apply_snapshot(snapshot);
            self.world.push_events(events);
            self.seen = seen;
            self.marks = marks;
            self.stepped = true;
        }
    }
}

/// The local hero's pose for the next step.
#[derive(Resource, Default)]
pub(super) struct LocalInput {
    pose: Option<HeroPose>,
    /// How the hero moves, read off the controller ([`super::stance::gait_of`]).
    pub gait: Gait,
    /// Stealth presses this session, counted round past 255 ([`PlayerInput::stealth_presses`]).
    pub stealth_presses: u8,
    /// The latest ability press, repeated in every input until the next ([`PlayerInput::cast`]).
    pub cast: Option<CastPress>,
    /// Dodge presses this session, counted round past 255.
    pub dodge_presses: u8,
    /// The act the host has not taken yet, repeated in every input ([`PlayerInput::act`]).
    pub act: Option<ActPress>,
    /// Acts waiting their turn, oldest first.
    pub acts: VecDeque<Act>,
    /// The last act's number.
    act_seq: u8,
}

impl LocalInput {
    /// The input for the next step, with `pose`.
    fn with_pose(&self, pose: Option<HeroPose>) -> PlayerInput {
        PlayerInput {
            pose,
            gait: self.gait,
            stealth_presses: self.stealth_presses,
            cast: self.cast,
            dodge_presses: self.dodge_presses,
            act: self.act,
        }
    }

    /// A new press of WoW action `slot`, at `target` or the spot `at`: it gets the next number,
    /// so the host tells it from the last.
    pub fn press(&mut self, slot: u8, target: Option<UnitRef>, at: Option<V3>) {
        let seq = self.cast.map_or(1, |c| c.seq.wrapping_add(1));
        self.cast = Some(CastPress {
            seq,
            slot,
            target,
            at,
        });
    }

    /// `act`, after the ones already waiting and the one in flight: a click while the host has not
    /// yet taken the last one waits its turn, and never replaces it.
    pub fn queue(&mut self, act: Act) {
        if self.acts.len() < MAX_QUEUED {
            self.acts.push_back(act);
        }
    }

    /// The host has taken our act when our hero's `acted` (`Carry::acted`) says its number: the
    /// next one goes. One at a time, so an input the host replaced loses none and none is taken
    /// twice.
    pub fn advance(&mut self, acted: Option<u8>) {
        if let Some(p) = self.act {
            if acted != Some(p.seq) {
                return;
            }
            self.act = None;
        }
        if let Some(act) = self.acts.pop_front() {
            // Numbers go 1 to 255 and round: 0 is a hero who has taken nothing yet.
            self.act_seq = if self.act_seq == u8::MAX {
                1
            } else {
                self.act_seq + 1
            };
            self.act = Some(ActPress {
                seq: self.act_seq,
                act,
            });
        }
    }

    /// A new run: nothing in flight, numbering from the start, as a new world has taken nothing.
    pub fn new_run(&mut self) {
        self.act = None;
        self.acts.clear();
        self.act_seq = 0;
    }
}

/// The run is over (left, ended by the host, or the connection went): [`back_to_menu`] leaves it.
#[derive(Resource, Default)]
pub(super) struct EndRun(pub bool);

pub(super) fn autoplay() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_AUTOPLAY").is_some())
}

fn trace_pos() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NIGHTFALL_TRACE_POS").is_some())
}

/// Bevy's position to WoW's ground plane.
fn bevy_to_ground(p: Vec3) -> V2 {
    V2::new(-p.z, -p.x)
}

/// `NIGHTFALL_AUTOPLAY`: a point on a slow 15-yard circle around the first entry, walked toward.
fn autowalk(entry: V2, from: V2, elapsed: f32, speed: f32, dt: f32) -> V2 {
    let goal = entry + V2::from_angle(elapsed * 0.15, 15.0);
    let to = goal - from;
    let dist = to.length();
    from + to.normalize_or_zero() * (speed * 0.5 * dt).min(dist)
}

/// `NIGHTFALL_AUTOPLAY`'s orders from the loot and the exits: stand still (searching), or walk to
/// a spot instead of the circle.
#[derive(Resource, Default)]
pub(super) struct Autoplay {
    /// Stand still (searching): the walk takes no step.
    pub hold: bool,
    /// Walk to this spot instead of the circle. The loot's detour to a container sets it first
    /// ([`super::loot_view::autoloot`]); with no such detour it is left to whoever else wants it.
    pub goal: Option<V2>,
}

/// `NIGHTFALL_AUTOPLAY`: a step from `from` toward `goal`, at half speed, never past it.
pub(super) fn autowalk_to(goal: V2, from: V2, speed: f32, dt: f32) -> V2 {
    let to = goal - from;
    from + to.normalize_or_zero() * (speed * 0.5 * dt).min(to.length())
}

/// The raid's seed: `NIGHTFALL_SEED` when set (a test run rolls the same loot every time), else
/// the clock.
pub(super) fn raid_seed(time: &Time<Real>) -> u64 {
    std::env::var("NIGHTFALL_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| time.elapsed().as_nanos() as u64 ^ 0x5EED_0F01_0000_0001)
}

/// Play Solo, the host's Start Run, or (joined) the host's start: a run, the controller taking our
/// new hero.
pub(super) fn start_run(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut start: ResMut<StartRequest>,
    selection: Res<Selection>,
    mut player: ResMut<crate::player::Player>,
    mut next: ResMut<NextState<Phase>>,
    mut net: ResMut<Net>,
    mut input: ResMut<LocalInput>,
    mut stash: ResMut<Stash>,
    mut inn_state: ResMut<session::InnState>,
    mut held: Local<bool>,
) {
    if net.is_solo() && net.status.is_empty() && time.elapsed_secs() > 8.0 {
        if std::env::var_os("NIGHTFALL_HOST").is_some() {
            net.host();
        } else if let Ok(addr) = std::env::var("NIGHTFALL_JOIN") {
            net.join(&addr);
        }
    }
    // A start held for a joiner's loadout dies with the party: never a solo run by surprise.
    if *held && !matches!(net.role, Role::Host(_)) {
        *held = false;
        start.0 = false;
    }
    if let Role::Client(c) = &mut net.role {
        let (true, Some(me)) = (c.start, c.me) else {
            return;
        };
        c.start = false;
        let own = session::own_pick(&selection, session::own_loadout(&selection, &stash));
        // The host gave our hero the pick it last had of us. A stash that has changed since (a
        // move the host had not heard of when it started) cannot be committed: it would leave the
        // stash with something other than what the raid brings. Nothing has left the stash yet,
        // so leaving costs nothing.
        if !net.host_holds(&own) {
            net.leave(
                "The host started before your loadout change reached it: nothing was taken from \
                 your stash. Join again.",
            );
            return;
        }
        // A raid that cannot be saved cannot begin: leaving costs nothing (nothing has left the
        // stash), and the host drops nothing for a hero that never sent an input.
        if let Err(why) = begin_raid(&mut stash, &selection, &own) {
            net.leave(why);
            return;
        }
        // The raid as the host planned it in the lobby: where it enters the map.
        commands.insert_resource(Sim::mirror(me, net.planned_raid().as_ref()));
        player.active = false;
        input.new_run();
        inn_state.ready = false;
        next.set(Phase::Playing);
        return;
    }
    let auto = autoplay()
        && match &net.role {
            Role::Solo => {
                time.elapsed_secs() > 12.0 && std::env::var_os("NIGHTFALL_HOST").is_none()
            }
            Role::Host(h) => !h.peers.is_empty() && time.elapsed_secs() > 20.0,
            Role::Client(_) => false,
        };
    if !start.0 && !auto {
        return;
    }
    // A joined player's loadout arrives with their first pick, just after the welcome: a start
    // in that moment waits, never beginning with their hands empty. The host's click is
    // remembered (`held`), and the run starts the frame the pick comes.
    // Everyone joined must be ready (the host's own Start is its Ready): a friend still setting up
    // their loadout is waited for, and the click is not kept.
    if !net.party_ready(true) {
        let name = net.first_unready().unwrap_or_default();
        net.status = format!("Waiting for {name} to be ready…");
        *held = false;
        start.0 = false;
        return;
    }
    // A ready friend's pick, though, is a moment from the lobby that tells their client: that start
    // is remembered (`held`) like one waiting for a first pick.
    if let Some(name) = net.awaiting_pick().or_else(|| net.lobby_behind()) {
        net.status = format!("Waiting for {name}'s loadout…");
        *held = start.0;
        return;
    }
    *held = false;
    start.0 = false;
    let own = session::own_pick(&selection, session::own_loadout(&selection, &stash));
    let roster = net.roster(&own);
    // On disk before the raid starts: whatever happens in it, the stash does not hold it too. A
    // begin that cannot be saved starts nothing; the stash's note says why.
    if begin_raid(&mut stash, &selection, &own).is_err() {
        return;
    }
    net.broadcast_start();
    // The raid the host (or we, alone) planned in the inn; one planned now if none was.
    let roll = net
        .planned_raid()
        .unwrap_or_else(|| RaidRoll::standard(&content(), raid_seed(&time)));
    net.solo_raid = None;
    commands.insert_resource(Sim::hosted(&roster, roll));
    player.active = false;
    input.new_run();
    inn_state.ready = false;
    next.set(Phase::Playing);
}

/// The raid begins for our hero: the pick `own` leaves the stash for it, on disk at once. What the
/// stash commits is what `own` told the host we bring (the pick came from the same
/// [`session::own_loadout`]); if it somehow is not, the raid still goes, and the log says so. A
/// begin that could not be saved is an error, and the raid must not start.
fn begin_raid(
    stash: &mut Stash,
    selection: &Selection,
    own: &nightfall_sim::messages::HeroPick,
) -> Result<(), String> {
    if !stash.begin_raid(selection.class, &own.loadout, &content())? {
        warn!("nightfall: the stash committed a loadout other than the one the raid was told");
    }
    Ok(())
}

/// Before the pose is read: the act in flight is done once our hero says so, and the next goes.
pub(super) fn advance_acts(sim: Res<Sim>, mut input: ResMut<LocalInput>) {
    let acted = sim.world.hero(sim.me).map(|h| h.carry.acted);
    input.advance(acted);
}

/// Alone or hosting: the outcomes the world settled, ours kept for the stash, each friend's sent
/// to them; and once the raid is over, everyone's to everyone, for the end screen.
pub(super) fn deliver(sim: &mut Sim, net: &mut Net) {
    for o in sim.world.take_outcomes() {
        if o.player == sim.me {
            sim.mine = Some(o);
        } else {
            session::send_outcome(net, &o);
        }
    }
    if sim.world.is_over() && sim.over.is_none() {
        let all = sim.world.outcomes().to_vec();
        session::broadcast_run_over(net, &all);
        sim.over = Some(all);
    }
}

/// Where WoW's controller left the local hero, as the pose the next step takes, kept inside the
/// map's edge (a hero past it is put back on it). Until the controller has the hero, the terrain
/// streams around the first entry.
pub(super) fn feed_pose(
    time: Res<Time>,
    real: Res<Time<Real>>,
    world_point: WorldPoint,
    views: Res<Views>,
    sim: Res<Sim>,
    auto: Res<Autoplay>,
    mut player: ResMut<crate::player::Player>,
    mut input: ResMut<LocalInput>,
    mut bodies: Query<&mut Transform>,
    mut last_trace: Local<f32>,
) {
    let content = content();
    let map = &content.map;
    let entry = sim.entry;
    let Some(mut tf) = views
        .heroes
        .get(&sim.me)
        .and_then(|&e| bodies.get_mut(e).ok())
        .filter(|_| player.active)
    else {
        if !player.active {
            player.pos = units::wow_to_bevy(entry);
        }
        input.pose = None;
        return;
    };
    let at = bevy_to_ground(tf.translation);
    let mut pos = at;
    if autoplay() && !auto.hold {
        let speed = sim.world.hero(sim.me).map_or(7.0, |h| h.speed);
        pos = match auto.goal {
            Some(goal) => autowalk_to(goal, pos, speed, time.delta_secs()),
            None => autowalk(
                V2::new(entry[0], entry[1]),
                pos,
                sim.world.elapsed,
                speed,
                time.delta_secs(),
            ),
        };
    }
    pos = map.bounds.clamp(pos);
    if pos != at {
        let mut back = units::ground_to_bevy(pos, tf.translation.y);
        if let Some(h) = world_point.terrain_height_under(back) {
            back.y = back.y.max(h);
        }
        tf.translation = back;
        player.pos = back;
    }
    if trace_pos() && real.elapsed_secs() - *last_trace >= 2.0 {
        *last_trace = real.elapsed_secs();
        let t = tf.translation;
        info!("nightfall: hero at ({:.1}, {:.1}, {:.1})", -t.z, -t.x, t.y);
    }
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    input.pose = Some(HeroPose {
        pos,
        facing: V2::new(yaw.cos(), yaw.sin()),
        // Bevy's up is WoW's z.
        z: tf.translation.y,
    });
    // The autoplay walk moves the body itself, holding no key: it says its own gait, or stillness
    // while it stands to search.
    input.gait = if autoplay() {
        if auto.hold {
            Gait::Still
        } else {
            super::stance::auto_gait().gait()
        }
    } else {
        super::stance::gait_of(player.move_flags())
    };
}

/// The run's clock. Alone or hosting: fixed steps for the real time that passed, each sent to the
/// clients; it waits while the loading screen is up and until the controller has the hero. Joined:
/// our pose to the host, and the host's snapshots into our mirror.
pub(super) fn step_sim(
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut sim: ResMut<Sim>,
    input: Res<LocalInput>,
    mut net: ResMut<Net>,
    mut end: ResMut<EndRun>,
    terrain: Res<super::terrain::Terrain>,
) {
    sim.stepped = false;
    let sim = &mut *sim;
    if net.is_client() {
        let heard = session::client_exchange(&mut net, input.with_pose(input.pose));
        if session::client_left_run(&mut net, &heard) {
            end.0 = true;
        }
        for h in heard {
            match h {
                Heard::State(snapshot, seen, marks, events) => {
                    sim.inbox.push_back((snapshot, seen, marks, events));
                }
                Heard::Outcome(o) => sim.mine = Some(o),
                Heard::RunOver(all) => sim.over = Some(all),
                Heard::Ended | Heard::Lost(_) | Heard::Lobby(..) | Heard::Start(_) => {}
            }
        }
        sim.play_back(time.delta_secs());
        return;
    }
    session::host_receive(&mut net, &mut sim.world);
    // The host's world learns the map's walls once the bake is done; its enemies come with them.
    if sim.world.map().is_none() {
        if let Some(map) = terrain.ready() {
            sim.world.attach_map(map.clone());
            sim.log_new_warnings();
        }
    }
    // Until the controller has our hero the world waits (the terrain is still streaming); once
    // our hero is out of the raid (extracted), it goes on for the others without us.
    let ours = sim.world.hero(sim.me).is_some();
    if ours && input.pose.is_none() {
        return;
    }
    if loading.covering() {
        return;
    }
    // A long hitch would otherwise replay seconds of play in one frame.
    sim.acc = (sim.acc + time.delta_secs()).min(0.25);
    while sim.acc >= DT {
        sim.acc -= DT;
        if ours {
            sim.world.set_input(sim.me, input.with_pose(input.pose));
        }
        let from = sim.world.events().len();
        sim.world.step();
        if let Some(p) = super::fallen::frail_player() {
            if super::fallen::crossed(super::fallen::FRAIL_AT, sim.world.elapsed) {
                if let Some(blow) = sim.world.hero(p).map(super::fallen::frail_blow) {
                    sim.world.hurt_hero(p, blow);
                }
            }
        }
        sim.seen = sim.world.seen_by(sim.me);
        sim.marks = sim.world.marks_for(sim.me);
        session::host_send(&mut net, &sim.world, from);
        sim.stepped = true;
    }
    deliver(sim, &mut net);
    // Paths that found nothing are warned of during a step, not at the attach.
    sim.log_new_warnings();
}

/// This frame's events out of the world, so they are shown once and never pile up. Alone or
/// hosting, only those a client would be sent ([`session::event_for`]), so the host sees the
/// fight as a joined player does; a client's were filtered by its host.
pub(super) fn take_events(mut sim: ResMut<Sim>, net: Res<Net>) {
    let mut events = sim.world.drain_events();
    if !net.is_client() {
        let me = sim.me;
        events.retain(|e| session::event_for(e, me, |u| sim.world.in_reach(me, u)));
    }
    sim.events = events;
}

/// `NIGHTFALL_AUTOPLAY_ABANDON=<seconds>`: the autoplay presses Abandon Raid that many seconds into
/// the raid, for a shot of what the button does.
fn abandon_at() -> Option<f32> {
    static AT: OnceLock<Option<f32>> = OnceLock::new();
    *AT.get_or_init(|| {
        std::env::var("NIGHTFALL_AUTOPLAY_ABANDON")
            .ok()
            .and_then(|s| s.parse().ok())
    })
}

/// Abandon Raid: alone or hosting, the raid ends now for everyone (spec §8.1: those still in are
/// missing) and the end screen shows; once it is over, the button goes back to the menu. Joined,
/// we walk out of the session, counted missing (no outcome will come for us).
pub(super) fn leave_run(
    clicks: Res<GlueClicks>,
    buttons: Query<Entity, With<LeaveRun>>,
    mut net: ResMut<Net>,
    mut end: ResMut<EndRun>,
    mut sim: ResMut<Sim>,
    mut stash: ResMut<Stash>,
) {
    let auto = sim.over.is_none() && abandon_at().is_some_and(|t| sim.world.elapsed >= t);
    if !auto && !buttons.iter().any(|e| clicks.hit(e)) {
        return;
    }
    abandon_raid(&mut sim, &mut net, &mut end, &mut stash);
}

/// What the Abandon Raid button does. The host (or a player alone) abandons the world and then
/// delivers: every joined player is sent their own outcome (missing) and then everyone's, before
/// anything tells them the run has ended. The end screen is the run being over, not leaving it.
fn abandon_raid(sim: &mut Sim, net: &mut Net, end: &mut EndRun, stash: &mut Stash) {
    if net.is_client() {
        stash.profile.settle_missing(&content());
        stash.save();
        net.leave("You left the raid");
        end.0 = true;
        return;
    }
    if sim.over.is_some() {
        net.broadcast_ended();
        end.0 = true;
        return;
    }
    sim.world.abandon();
    deliver(sim, net);
}

/// Back to the menu once the run is over: everything it spawned goes, and the controller lets go.
pub(super) fn back_to_menu(
    mut commands: Commands,
    mut end: ResMut<EndRun>,
    mut views: ResMut<Views>,
    mut enemy_views: ResMut<super::enemies::EnemyViews>,
    mut telegraph_views: ResMut<super::telegraphs::TelegraphViews>,
    mut index: ResMut<crate::net::GuidIndex>,
    mut player: ResMut<crate::player::Player>,
    mut cooldowns: ResMut<crate::spell::Cooldowns>,
    mut modes: MessageWriter<crate::net::MoveModeMessage>,
    mut next: ResMut<NextState<Phase>>,
    mut net: ResMut<Net>,
    spawned: Query<Entity, With<NightfallEntity>>,
) {
    if !std::mem::take(&mut end.0) {
        return;
    }
    for e in &spawned {
        commands.entity(e).despawn();
    }
    let gone: std::collections::HashSet<Entity> = spawned.iter().collect();
    index.0.retain(|_, e| !gone.contains(e));
    *views = Views::default();
    *enemy_views = super::enemies::EnemyViews::default();
    // The shapes are `NightfallEntity`s, despawned above.
    *telegraph_views = super::telegraphs::TelegraphViews::default();
    commands.remove_resource::<Sim>();
    player.active = false;
    // Nothing of the fight follows the player out: no sweeps on the bar, no root on their feet.
    cooldowns.wipe();
    modes.write(crate::net::MoveModeMessage {
        guid: 0,
        counter: 0,
        mode: benilla_protocol::MoveMode::Root,
        apply: false,
    });
    // Alone, the inn plans the next raid; with friends, the host does.
    net.replan_alone();
    next.set(Phase::Menu);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_mirror_shows_no_containers_before_its_first_snapshot() {
        // The world a mirror is made of rolls the content's own raid, which it must not show.
        assert!(
            !World::new(content()).loot_view(Some(1)).marks.is_empty(),
            "the shipped content places containers"
        );
        let mirror = Sim::mirror(1, None);
        let view = mirror.world.loot_view(Some(1));
        assert!(view.marks.is_empty() && view.open.is_none());
        // The first snapshot of the host replaces it, as before.
        let mut host = World::new(content());
        host.begin_raid(nightfall_sim::RaidRoll::standard(&content(), 3))
            .unwrap();
        let mut mirror = mirror;
        mirror.world.apply_snapshot(host.snapshot_for(0));
        assert_eq!(mirror.world.loot_view(Some(1)), host.loot_view(Some(0)));
    }

    #[test]
    fn autowalk_heads_for_the_circle_without_overshooting() {
        let entry = V2::new(0.0, 0.0);
        let goal = entry + V2::from_angle(0.0, 15.0);
        let next = autowalk(entry, entry, 0.0, 7.0, 1.0);
        assert!((next.distance(entry) - 3.5).abs() < 1e-4);
        let near = autowalk(entry, goal - V2::new(0.1, 0.0), 0.0, 7.0, 1.0);
        assert!(near.distance(goal) < 1e-4);
    }

    #[test]
    fn autowalk_to_a_goal_stops_on_it() {
        let goal = V2::new(10.0, 0.0);
        let next = autowalk_to(goal, V2::ZERO, 7.0, 1.0);
        assert!((next.x - 3.5).abs() < 1e-4);
        assert_eq!(autowalk_to(goal, V2::new(9.9, 0.0), 7.0, 1.0), goal);
    }

    #[test]
    fn a_click_while_an_act_is_in_flight_waits_its_turn() {
        let mut input = LocalInput::default();
        input.queue(Act::Open(1));
        input.advance(Some(0));
        let in_flight = input.act;
        assert_eq!(in_flight.map(|p| p.act), Some(Act::Open(1)));
        // Clicks while it is out queue behind it, in order; none replaces it.
        input.queue(Act::TakeAll);
        input.queue(Act::Close);
        input.advance(Some(0));
        assert_eq!(input.act, in_flight);
        assert_eq!(input.acts, [Act::TakeAll, Act::Close]);
        // Taken: the next goes, and only then.
        input.advance(Some(1));
        assert_eq!(input.act.map(|p| p.act), Some(Act::TakeAll));
        assert_eq!(input.acts, [Act::Close]);
    }

    #[test]
    fn each_press_gets_the_next_number() {
        let mut input = LocalInput::default();
        input.press(2, None, None);
        input.press(2, None, None);
        assert_eq!(input.cast.map(|c| (c.seq, c.slot)), Some((2, 2)));
        input.cast = Some(CastPress {
            seq: 255,
            ..CastPress::default()
        });
        input.press(3, None, None);
        assert_eq!(input.cast.unwrap().seq, 0);
        assert_eq!(input.with_pose(None).cast, input.cast);
    }

    #[test]
    fn acts_go_one_at_a_time_and_only_once_taken() {
        let mut input = LocalInput::default();
        input.queue(Act::TakeAll);
        input.queue(Act::Close);
        input.advance(Some(0));
        assert_eq!(
            input.act,
            Some(ActPress {
                seq: 1,
                act: Act::TakeAll
            })
        );
        // Not taken yet: the same act, the same number.
        input.advance(Some(0));
        assert_eq!(
            input.act,
            Some(ActPress {
                seq: 1,
                act: Act::TakeAll
            })
        );
        input.advance(Some(1));
        assert_eq!(
            input.act,
            Some(ActPress {
                seq: 2,
                act: Act::Close
            })
        );
        input.advance(Some(2));
        assert_eq!(input.act, None);
        input.new_run();
        input.queue(Act::Close);
        input.advance(None);
        assert_eq!(
            input.act.unwrap().seq,
            1,
            "a new run numbers from the start"
        );
        input.act_seq = u8::MAX;
        input.act = None;
        input.queue(Act::TakeAll);
        input.advance(None);
        assert_eq!(
            input.act.unwrap().seq,
            1,
            "the numbers round past 255 without 0"
        );
    }

    #[test]
    fn the_first_act_is_never_number_0_which_a_fresh_hero_has_already_taken() {
        // A new hero's `Carry::acted` is 0: an act numbered 0 would look taken before it was sent.
        let mut input = LocalInput::default();
        input.queue(Act::TakeAll);
        input.advance(Some(0));
        assert_ne!(input.act.unwrap().seq, 0);
        // Every input carries the act in flight.
        assert_eq!(input.with_pose(None).act, input.act);
    }

    #[test]
    fn a_new_run_forgets_acts_still_waiting_or_in_flight() {
        let mut input = LocalInput::default();
        for _ in 0..3 {
            input.queue(Act::TakeAll);
        }
        input.advance(None);
        assert!(input.act.is_some() && input.acts.len() == 2);
        input.new_run();
        assert_eq!(input.act, None, "the act in flight is not replayed");
        assert!(input.acts.is_empty(), "nor the ones waiting");
        // Whatever the last run's hero had taken, the next one numbers from 1 again.
        input.queue(Act::Close);
        input.advance(Some(7));
        assert_eq!(input.act.unwrap().seq, 1);
    }

    #[test]
    fn a_click_storm_queues_only_so_many_acts() {
        let mut input = LocalInput::default();
        for _ in 0..MAX_QUEUED + 10 {
            input.queue(Act::TakeAll);
        }
        assert_eq!(input.acts.len(), MAX_QUEUED);
    }

    #[test]
    fn deliver_keeps_our_outcome_and_the_end_once() {
        use std::sync::Arc;
        let content = Arc::new(
            nightfall_sim::Content::from_ron(nightfall_sim::ContentSources {
                classes: include_str!("../../../../content/nightfall/classes.ron"),
                map: include_str!("../../../../content/nightfall/map.ron"),
                rules: include_str!("../../../../content/nightfall/rules.ron"),
                enemies: include_str!("../../../../content/nightfall/enemies.ron"),
                spawns: include_str!("../../../../content/nightfall/spawns.ron"),
                abilities: include_str!("../../../../content/nightfall/abilities.ron"),
                items: include_str!("../../../../content/nightfall/items.ron"),
                loot: include_str!("../../../../content/nightfall/loot.ron"),
            })
            .expect("the shipped content parses"),
        );
        let mut world = World::new(content.clone());
        world
            .begin_raid(RaidRoll::standard(&content, 1))
            .expect("the standard roll fits the map");
        world.add_hero(0, "rogue", "Us", 1, 0).unwrap();
        world.add_hero(1, "mage", "Friend", 1, 0).unwrap();
        let mut sim = Sim::for_test(world);
        let mut net = Net::default();
        deliver(&mut sim, &mut net);
        assert!(
            sim.mine.is_none() && sim.over.is_none(),
            "a raid under way has settled nothing"
        );
        sim.world.abandon();
        deliver(&mut sim, &mut net);
        assert_eq!(sim.mine.as_ref().map(|o| o.player), Some(0));
        assert_eq!(sim.over.as_ref().map(Vec::len), Some(2));
        // Delivered once: the world hands each outcome over only once, and the end is kept.
        sim.mine = None;
        deliver(&mut sim, &mut net);
        assert!(sim.mine.is_none(), "our outcome came once");
        assert_eq!(sim.over.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn abandoning_a_raid_alone_or_hosting_ends_it_for_everyone_then_goes_back() {
        let content = super::super::shipped_content();
        let mut world = World::new(content.clone());
        world
            .begin_raid(RaidRoll::standard(&content, 1))
            .expect("the standard roll fits the map");
        world.add_hero(0, "rogue", "Us", 1, 0).unwrap();
        world.add_hero(1, "mage", "Friend", 1, 0).unwrap();
        let mut sim = Sim::for_test(world);
        let mut net = Net::default();
        let mut end = EndRun::default();
        let mut stash = Stash::open_at(None, &content);

        // The first press: the raid is over for both heroes, ours and the end screen's all ready,
        // and the session goes on (the end screen is shown, the menu is not).
        abandon_raid(&mut sim, &mut net, &mut end, &mut stash);
        assert!(sim.world.is_over());
        assert!(!end.0, "the end screen shows first");
        assert!(
            matches!(
                sim.mine.as_ref().map(|o| &o.fate),
                Some(nightfall_sim::Fate::Missing)
            ),
            "our own outcome is delivered at once"
        );
        let over = sim.over.as_ref().expect("the end screen has everyone");
        assert_eq!(over.len(), 2);
        assert!(over
            .iter()
            .all(|o| matches!(o.fate, nightfall_sim::Fate::Missing)));
        // The world delivered every outcome: none is left waiting to go out after the run ends.
        assert!(sim.world.take_outcomes().is_empty());

        // Pressed again with the raid over, the button goes back to the menu.
        abandon_raid(&mut sim, &mut net, &mut end, &mut stash);
        assert!(end.0);
    }

    #[test]
    fn the_world_goes_on_for_the_others_once_our_hero_has_extracted() {
        use bevy::ecs::system::RunSystemOnce;
        use std::time::Duration;

        let content = super::super::shipped_content();
        let mut world = World::new(content.clone());
        world
            .begin_raid(RaidRoll::standard(&content, 1))
            .expect("the standard roll fits the map");
        world.add_hero(0, "rogue", "Us", 1, 0).unwrap();
        world.add_hero(1, "mage", "Friend", 1, 0).unwrap();
        let exit = content.map.exits[0].at;

        let mut app = bevy::prelude::World::new();
        app.init_resource::<Time<Real>>();
        app.init_resource::<crate::loading_screen::LoadingScreen>();
        app.init_resource::<super::super::terrain::Terrain>();
        app.init_resource::<Net>();
        app.init_resource::<EndRun>();
        app.insert_resource(Sim::for_test(world));
        app.insert_resource(LocalInput {
            pose: Some(HeroPose {
                pos: exit,
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            ..LocalInput::default()
        });
        let frame = |app: &mut bevy::prelude::World| {
            app.resource_mut::<Time<Real>>()
                .update_with_duration(Duration::from_millis(250));
            app.run_system_once(step_sim).unwrap();
        };

        // Ten seconds in the ring (a frame is a quarter second at most) and we are out.
        for _ in 0..48 {
            frame(&mut app);
        }
        let sim = app.resource::<Sim>();
        assert!(sim.world.hero(0).is_none(), "our hero has extracted");
        assert!(
            matches!(
                sim.mine.as_ref().map(|o| &o.fate),
                Some(nightfall_sim::Fate::Extracted { .. })
            ),
            "and our outcome is ours to settle"
        );
        assert!(sim.world.hero(1).is_some() && sim.over.is_none());

        // The controller no longer has a hero to pose, yet the world goes on for the friend.
        app.resource_mut::<LocalInput>().pose = None;
        let before = app.resource::<Sim>().world.elapsed;
        for _ in 0..8 {
            frame(&mut app);
        }
        let sim = app.resource::<Sim>();
        assert!(
            sim.world.elapsed > before + 1.5,
            "the world waited for a pose that will never come ({} then {})",
            before,
            sim.world.elapsed
        );

        // And what it settles is still delivered: the raid ends, the end screen has everyone.
        app.resource_mut::<Sim>().world.abandon();
        frame(&mut app);
        let sim = app.resource::<Sim>();
        assert_eq!(sim.over.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn bevy_and_wow_ground_agree() {
        let p = V2::new(-10051.8, -792.5);
        assert_eq!(bevy_to_ground(units::ground_to_bevy(p, 36.5)), p);
    }

    #[test]
    fn leaving_a_run_takes_the_fight_out_of_the_engine() {
        use crate::net::MoveModeMessage;
        use crate::spell::Cooldowns;
        use bevy::ecs::system::RunSystemOnce;

        let mut app = bevy::prelude::World::new();
        app.insert_resource(EndRun(true));
        app.init_resource::<Views>();
        app.init_resource::<super::super::enemies::EnemyViews>();
        app.init_resource::<super::super::telegraphs::TelegraphViews>();
        app.init_resource::<crate::net::GuidIndex>();
        app.init_resource::<crate::player::Player>();
        app.init_resource::<NextState<Phase>>();
        app.init_resource::<Net>();
        app.init_resource::<Messages<MoveModeMessage>>();
        let mut cooldowns = Cooldowns::default();
        cooldowns.apply_wire_cooldown(1777, 8_000, None, bevy::platform::time::Instant::now());
        assert_eq!(cooldowns.record_count(), 1);
        app.insert_resource(cooldowns);

        app.run_system_once(back_to_menu).unwrap();
        assert_eq!(app.resource::<Cooldowns>().record_count(), 0, "no sweeps");
        let modes: Vec<_> = app
            .resource_mut::<Messages<MoveModeMessage>>()
            .drain()
            .map(|m| (m.mode, m.apply))
            .collect();
        assert_eq!(
            modes,
            [(benilla_protocol::MoveMode::Root, false)],
            "no root on the player's feet"
        );
    }
}
