//! Co-op sessions for Nightfall: hosting or joining, the lobby before a run, and the run's traffic.
//! The pattern is Survivors' (`together/session.rs`), copied: the host steps the world and every
//! [`SEND_EVERY`] steps sends each client a snapshot; a client mirrors it and sends where its own
//! controller put its hero. A slow line never builds a queue: a client whose connection still
//! holds unsent snapshots is skipped until it catches up.
//!
//! On the wire Nightfall is the game `"nightfall"` ([`GAME`]): it shares Survivors' port and invite
//! code, and a friend who opened the other game is told so.

use std::collections::HashMap;
use std::sync::Mutex;

use bevy::prelude::*;
use nightfall_sim::messages::{ClientMsg, HeroPick, LobbyPlayer, ServerMsg, PROTOCOL};
use nightfall_sim::{
    Event, HeroOutcome, HeroPose, Loadout, Mark, PlayerId, PlayerInput, RaidRoll, Snapshot,
    UnitRef, World, DT,
};
use together_net::{content_hash, new_host_key, ConnId, GameTag, Invite, DEFAULT_PORT};
use web_time::Instant;

use super::menu::Selection;
use super::stash::Stash;
use super::{content, inn};

/// This game on the wire.
pub(super) const GAME: GameTag = "nightfall";
type Host = together_net::Host<ClientMsg, ServerMsg>;
type Client = together_net::Client<ClientMsg, ServerMsg>;
type HostEvent = together_net::HostEvent<ClientMsg>;
type ClientEvent = together_net::ClientEvent<ServerMsg>;

/// The most players in a run, the host included.
pub(super) const MAX_PLAYERS: usize = 4;
/// Steps per snapshot sent: 15 a second, which the client's playback smooths.
pub(super) const SEND_EVERY: u32 = 2;
/// A connection still writing more snapshots than this is skipped until it catches up.
const MAX_BACKLOG: usize = 2;
/// Events a client that keeps falling behind may have waiting; past it the oldest go, so a stalled
/// connection never grows the host's memory without end.
const MAX_PENDING: usize = 4096;
/// The lobby's least gap between two sends, seconds: friends' poses at most ten times a second.
const LOBBY_EVERY: f32 = 0.1;

/// Our own part of the inn's lobby: where our hero stands, and whether we are ready. Ready is
/// the lobby's lock for a joined player: ready, their stash and loadout are frozen
/// ([`super::stash_panel::lobby_frozen`]), and not ready they are free.
#[derive(Resource, Default)]
pub(super) struct InnState {
    pub pose: Option<HeroPose>,
    pub ready: bool,
}

impl InnState {
    /// Not ready (and no pose): a raid began, the session ended, or none was joined yet.
    pub fn reset(&mut self) {
        self.ready = false;
    }
}

/// Whether the host takes a joiner's Ready: only once their pick is in (a hello carries no
/// loadout).
fn ready_after(ready: bool, picked: bool) -> bool {
    ready && picked
}

/// A joiner's Ready as the host holds it after their pick `now` arrives: gone when what they bring
/// changed from `was`.
fn ready_after_pick(ready: bool, was: &HeroPick, now: &HeroPick) -> bool {
    ready && same_bringing(was, now)
}

/// Our own Ready after our pick changed: gone, unless it is the first pick (`sent_before` false).
fn ready_after_own_pick(ready: bool, sent_before: bool) -> bool {
    ready && !sent_before
}

/// The Ready to tell the host now, if any: not before our pick is sent (so the host holds what we
/// bring), and again whenever it differs from what we last told it (`sent_ready` is cleared when a
/// raid ends, so a Ready for the next raid is sent even if it equals the last).
fn ready_to_send(ready: bool, pick_sent: bool, sent_ready: Option<bool>) -> Option<bool> {
    (sent_ready != Some(ready) && (!ready || pick_sent)).then_some(ready)
}

/// Whether `l` is how the lobby lists a player with these parts. Where they stand is not part of
/// it: poses go in their own message ([`ServerMsg::InnPoses`]), so walking never resends a loadout.
fn listed(l: &LobbyPlayer, player: PlayerId, hero: &HeroPick, ready: bool) -> bool {
    l.player == player && l.hero == *hero && l.ready == ready
}

/// A pose from a client made safe: finite, a unit facing, a height in WoW's range, the place
/// kept by `clamp` (the inn's bounds). `None` when it is not finite.
fn sane_pose(
    pose: HeroPose,
    clamp: impl Fn(nightfall_sim::V2) -> nightfall_sim::V2,
) -> Option<HeroPose> {
    if !(pose.pos.is_finite() && pose.facing.is_finite() && pose.z.is_finite()) {
        return None;
    }
    Some(HeroPose {
        pos: clamp(pose.pos),
        facing: pose.facing.normalize_or_zero(),
        z: pose.z.clamp(-1000.0, 3000.0),
    })
}

/// Whether the lobby goes out now: it changed, and the last went at least [`LOBBY_EVERY`] ago.
fn lobby_due(changed: bool, since_last: Option<f32>) -> bool {
    changed && since_last.is_none_or(|s| s >= LOBBY_EVERY)
}

/// Which of the host's two inn messages go now, as (lobby, poses). The lobby, with every
/// loadout, goes only when something it holds changed. The poses go when they changed (at most
/// ten times a second, like the lobby) and with every lobby, so a player just welcomed sees
/// where everyone stands without waiting for them to move.
fn inn_sends(
    lobby_changed: bool,
    poses_changed: bool,
    since_lobby: Option<f32>,
    since_poses: Option<f32>,
) -> (bool, bool) {
    let lobby = lobby_due(lobby_changed, since_lobby);
    (lobby, lobby || lobby_due(poses_changed, since_poses))
}

/// The poses a host sent, each made safe by `sane_pose` and at most one per seat, as a client
/// keeps them: the host is trusted to be well-behaved, a corrupt message is not.
fn sane_poses(
    poses: Vec<(PlayerId, HeroPose)>,
    clamp: impl Fn(nightfall_sim::V2) -> nightfall_sim::V2,
) -> HashMap<PlayerId, HeroPose> {
    poses
        .into_iter()
        .take(MAX_PLAYERS)
        .filter_map(|(id, p)| sane_pose(p, &clamp).map(|p| (id, p)))
        .collect()
}

/// The poses of the party that has one, the host's (player 0) first.
fn pose_list(
    own: Option<HeroPose>,
    peers: impl IntoIterator<Item = (PlayerId, Option<HeroPose>)>,
) -> Vec<(PlayerId, HeroPose)> {
    own.map(|p| (0, p))
        .into_iter()
        .chain(peers.into_iter().filter_map(|(id, p)| p.map(|p| (id, p))))
        .collect()
}

/// Whether a raid may start: we are ready, and so is every friend present.
pub(super) fn all_ready(own: bool, friends: impl IntoIterator<Item = bool>) -> bool {
    own && friends.into_iter().all(|r| r)
}

/// A pose worth sending: moved or turned since the last one sent.
fn moved(last: Option<HeroPose>, now: HeroPose) -> bool {
    last.is_none_or(|l| {
        l.pos.distance(now.pos) > 0.05
            || l.facing.distance(now.facing) > 0.05
            || (l.z - now.z).abs() > 0.05
    })
}

/// The next raid: the map's standard roll from a fresh seed (`NIGHTFALL_SEED` for a test run).
/// Sub-project 6 rolls its exits, weather and monsters here.
pub(super) fn plan_raid() -> RaidRoll {
    let seed = std::env::var("NIGHTFALL_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
    RaidRoll::standard(&content(), seed)
}

#[derive(Resource, Default)]
pub(super) struct Net {
    pub role: Role,
    /// A line for the menu: what is happening, or what went wrong.
    pub status: String,
    /// Alone: the next raid, planned as the inn is entered.
    pub solo_raid: Option<RaidRoll>,
}

/// A friend in the inn, as [`Net::friends`] lends them.
pub(super) struct Friend<'a> {
    pub player: PlayerId,
    pub hero: &'a HeroPick,
    pub pose: Option<HeroPose>,
}

#[derive(Default)]
pub(super) enum Role {
    #[default]
    Solo,
    Host(HostSide),
    Client(ClientSide),
}

pub(super) struct HostSide {
    host: Mutex<Host>,
    /// The machine's address on the local network, for friends to type.
    pub lan: String,
    pub port: u16,
    pub peers: Vec<Peer>,
    /// The lobby last sent, so it goes again only when it changes.
    sent: Option<Vec<LobbyPlayer>>,
    /// Steps since the run began, which pace the snapshots.
    steps: u32,
    /// The number of the run begun last ([`Net::broadcast_start`]): 0 before the first. Inputs
    /// tagged with another are from another raid and are not taken.
    raid: u32,
    /// The next raid as planned for the lobby; sent in every [`ServerMsg::Lobby`] and the run that
    /// starts is this one. Planned anew each time a raid ends.
    pub planned: RaidRoll,
    /// When the lobby last went out ([`LOBBY_EVERY`] paces it).
    sent_at: Option<Instant>,
    /// The poses last sent, so they go again only when one changes.
    sent_poses: Option<Vec<(PlayerId, HeroPose)>>,
    /// When the poses last went out ([`LOBBY_EVERY`] paces them).
    sent_poses_at: Option<Instant>,
    /// The invite code has been written to the log (for test runs to read).
    logged_code: bool,
}

pub(super) struct Peer {
    conn: ConnId,
    pub player: PlayerId,
    /// Who they are; what they bring is empty until their first pick comes ([`Peer::picked`]).
    pub hero: HeroPick,
    /// Their first [`ClientMsg::Pick`] has come, so `hero` holds what they bring. A hello carries
    /// no loadout (see [`nightfall_sim::messages::Who`]), so until it a run must not start.
    /// Cleared again when each raid starts: the next one needs a pick made after this one ended.
    pub picked: bool,
    /// Where their hero stands in the inn, as they last said.
    pub pose: Option<HeroPose>,
    /// Ready at the raid board. A ready player's stash and loadout are frozen on their side, so
    /// the pick the host holds is the one they bring; cleared when a raid ends.
    pub ready: bool,
    /// The events since this player's last state, sent with the next one it keeps up for.
    pending: Vec<Event>,
}

pub(super) struct ClientSide {
    client: Mutex<Client>,
    pub addr: String,
    /// Our player, once the host welcomed us.
    pub me: Option<PlayerId>,
    pub lobby: Vec<LobbyPlayer>,
    /// Where each player's hero stands, from the host's last [`ServerMsg::InnPoses`].
    poses: HashMap<PlayerId, HeroPose>,
    /// The host started a run; [`super::run::start_run`] begins ours.
    pub start: bool,
    /// The number of the run the host started last, which our inputs are tagged with.
    raid: u32,
    /// The next raid as the host planned it, from its last lobby.
    pub planned: Option<RaidRoll>,
    /// The pick we last told the host.
    sent_pick: Option<HeroPick>,
    /// The inn pose we last told the host, and when.
    sent_pose: Option<(HeroPose, Instant)>,
    /// Whether we last told the host we are ready.
    sent_ready: Option<bool>,
    /// When our hero's pose last went to the host.
    last_pose: Option<Instant>,
}

/// Why a host turns a hello away, if it does.
fn refusal(protocol: u32, content: u64, ours: u64, peers: usize) -> Option<String> {
    if protocol != PROTOCOL {
        Some("The host has a different version of the game.".into())
    } else if content != ours {
        Some("The host has different game data (content files).".into())
    } else if peers + 1 >= MAX_PLAYERS {
        Some(format!("The party is full ({MAX_PLAYERS} players)."))
    } else {
        None
    }
}

/// Whether the host's `lobby` lists player `me` with the class and loadout of `own`.
fn lobby_holds(lobby: &[LobbyPlayer], me: Option<PlayerId>, own: &HeroPick) -> bool {
    lobby
        .iter()
        .find(|p| Some(p.player) == me)
        .is_some_and(|p| same_bringing(&p.hero, own))
}

/// Whether two picks are the same hero bringing the same: class and loadout, not the name (the
/// host cleans that).
fn same_bringing(a: &HeroPick, b: &HeroPick) -> bool {
    a.class == b.class && a.loadout == b.loadout
}

/// The name of the first of `peers` (picked yet, name) whose first pick has not come.
fn first_unpicked<'a>(peers: impl IntoIterator<Item = (bool, &'a str)>) -> Option<&'a str> {
    peers
        .into_iter()
        .find(|(picked, _)| !picked)
        .map(|(_, name)| name)
}

/// The name of the first of `peers` (ready, name) who is not ready.
fn first_unready<'a>(peers: impl IntoIterator<Item = (bool, &'a str)>) -> Option<&'a str> {
    peers
        .into_iter()
        .find(|(ready, _)| !ready)
        .map(|(_, name)| name)
}

/// The name of the first of `peers` (player, pick) whose pick `lobby` (the last the host sent)
/// does not hold: the host's word to its clients is behind what the host holds.
fn first_behind<'a>(
    lobby: &[LobbyPlayer],
    peers: impl IntoIterator<Item = (PlayerId, &'a HeroPick)>,
) -> Option<&'a str> {
    peers
        .into_iter()
        .find(|(player, hero)| !lobby_holds(lobby, Some(*player), hero))
        .map(|(_, hero)| hero.name.as_str())
}

/// A new raid needs a fresh pick from every joiner: what a joiner brought last time is settled
/// (and may have changed) before they pick again, so none counts for the next raid.
fn clear_picks<'a>(picked: impl IntoIterator<Item = &'a mut bool>) {
    for p in picked {
        *p = false;
    }
}

/// The lowest player number not taken (the host is 0).
fn free_player(taken: impl Iterator<Item = PlayerId> + Clone) -> Option<PlayerId> {
    (1..MAX_PLAYERS as PlayerId).find(|p| !taken.clone().any(|t| t == *p))
}

/// The lobby as lines: each player, ours marked.
fn roster_lines(players: &[LobbyPlayer], me: Option<PlayerId>) -> String {
    players
        .iter()
        .map(|p| {
            let you = if Some(p.player) == me { "  (you)" } else { "" };
            let ready = if p.ready { "  (ready)" } else { "" };
            format!("{}  ·  {}{you}{ready}", p.hero.name, p.hero.class)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl HostSide {
    pub fn invite(&self) -> Invite {
        self.host.lock().unwrap().invite()
    }
}

impl Net {
    pub fn is_solo(&self) -> bool {
        matches!(self.role, Role::Solo)
    }

    pub fn is_client(&self) -> bool {
        matches!(self.role, Role::Client(_))
    }

    /// Start hosting: on [`DEFAULT_PORT`] (any free port if it is taken) for the local network, and
    /// by invite code ([`host_key`]) for everyone else.
    pub fn host(&mut self) {
        match Host::listen(DEFAULT_PORT, GAME).or_else(|_| Host::listen(0, GAME)) {
            Ok(host) => {
                let port = host.local_addr().port();
                host.open_invites(host_key());
                self.role = Role::Host(HostSide {
                    host: Mutex::new(host),
                    lan: lan_address().unwrap_or_else(|| "this computer's address".into()),
                    port,
                    peers: Vec::new(),
                    sent: None,
                    steps: 0,
                    raid: 0,
                    planned: plan_raid(),
                    sent_at: None,
                    sent_poses: None,
                    sent_poses_at: None,
                    logged_code: false,
                });
                self.status = "Waiting for friends to join".into();
            }
            Err(e) => {
                self.role = Role::Solo;
                self.status = format!("Could not host on port {DEFAULT_PORT}: {e}");
            }
        }
    }

    /// Start joining the host an invite code or address names.
    pub fn join(&mut self, addr: &str) {
        self.role = Role::Client(ClientSide {
            client: Mutex::new(Client::connect(addr, GAME)),
            addr: addr.to_owned(),
            me: None,
            lobby: Vec::new(),
            poses: HashMap::new(),
            start: false,
            raid: 0,
            planned: None,
            sent_pick: None,
            sent_pose: None,
            sent_ready: None,
            last_pose: None,
        });
        self.status = if together_net::parse_code(addr).is_some() {
            "Connecting by invite code…".into()
        } else {
            format!("Connecting to {addr}…")
        };
    }

    /// End the session: stop hosting (dropping every connection) or leave.
    pub fn leave(&mut self, why: impl Into<String>) {
        self.role = Role::Solo;
        self.status = why.into();
    }

    /// The invite code, once the host has one.
    pub fn invite_code(&self) -> Option<String> {
        match &self.role {
            Role::Host(h) => match h.invite() {
                Invite::Ready(code) => Some(code),
                _ => None,
            },
            _ => None,
        }
    }

    /// The menu's lines under the buttons.
    pub fn lobby_text(&self) -> String {
        match &self.role {
            Role::Solo => self.status.clone(),
            Role::Host(h) => {
                let code = match h.invite() {
                    Invite::Ready(_) => "Invite code ready: Copy Invite Code".to_string(),
                    Invite::Opening | Invite::Off => "Opening the invite code…".into(),
                    Invite::Failed(why) => format!("No invite code ({why})"),
                };
                let names: Vec<String> = h
                    .peers
                    .iter()
                    .map(|p| {
                        if p.ready {
                            format!("{} (ready)", p.hero.name)
                        } else {
                            p.hero.name.clone()
                        }
                    })
                    .collect();
                format!(
                    "Hosting  ·  {code}  ·  same network: {}:{}\nJoined: {}\n{}",
                    h.lan,
                    h.port,
                    if names.is_empty() {
                        "nobody yet".into()
                    } else {
                        names.join(", ")
                    },
                    self.status
                )
            }
            Role::Client(c) => format!(
                "Joined {}\n{}\n{}",
                if together_net::parse_code(&c.addr).is_some() {
                    "by invite code".to_string()
                } else {
                    c.addr.clone()
                },
                roster_lines(&c.lobby, c.me),
                self.status
            ),
        }
    }

    /// The players a run starts with, the host first.
    pub fn roster(&self, own: &HeroPick) -> Vec<LobbyPlayer> {
        let mut out = vec![LobbyPlayer {
            player: 0,
            hero: own.clone(),
            pose: None,
            ready: false,
        }];
        if let Role::Host(h) = &self.role {
            out.extend(h.peers.iter().map(|p| LobbyPlayer {
                player: p.player,
                hero: p.hero.clone(),
                pose: None,
                ready: false,
            }));
        }
        out
    }

    /// Joined, and the host has welcomed us: we may ready.
    pub fn welcomed(&self) -> bool {
        matches!(&self.role, Role::Client(c) if c.me.is_some())
    }

    /// The next raid as planned: the host's, the one the host told us, or ours alone.
    pub fn planned_raid(&self) -> Option<RaidRoll> {
        match &self.role {
            Role::Solo => self.solo_raid.clone(),
            Role::Host(h) => Some(h.planned.clone()),
            Role::Client(c) => c.planned.clone(),
        }
    }

    /// The friends in the lobby (never us) and where they stand, borrowed: it is read every frame,
    /// so no loadout is cloned.
    pub fn friends(&self) -> Vec<Friend<'_>> {
        match &self.role {
            Role::Solo => Vec::new(),
            Role::Host(h) => h
                .peers
                .iter()
                .map(|p| Friend {
                    player: p.player,
                    hero: &p.hero,
                    pose: p.pose,
                })
                .collect(),
            Role::Client(c) => c
                .lobby
                .iter()
                .filter(|p| Some(p.player) != c.me)
                .map(|p| Friend {
                    player: p.player,
                    hero: &p.hero,
                    pose: c.poses.get(&p.player).copied(),
                })
                .collect(),
        }
    }

    /// Whether the party may start: alone, when we are ready; hosting, when everyone present is.
    pub fn party_ready(&self, own: bool) -> bool {
        match &self.role {
            Role::Solo => own,
            Role::Host(h) => all_ready(own, h.peers.iter().map(|p| p.ready)),
            Role::Client(_) => false,
        }
    }

    /// The host: the first joined player who is not ready, whom a start waits for.
    pub fn first_unready(&self) -> Option<String> {
        match &self.role {
            Role::Host(h) => first_unready(h.peers.iter().map(|p| (p.ready, p.hero.name.as_str())))
                .map(str::to_owned),
            _ => None,
        }
    }

    /// The host: the first joined player whose pick the lobby last sent does not hold yet. A start
    /// waits for that lobby to go out (a moment, [`LOBBY_EVERY`]): their client would otherwise
    /// find the host started without its loadout, and leave.
    pub fn lobby_behind(&self) -> Option<String> {
        match &self.role {
            Role::Host(h) => first_behind(
                h.sent.as_deref().unwrap_or(&[]),
                h.peers.iter().map(|p| (p.player, &p.hero)),
            )
            .map(str::to_owned),
            _ => None,
        }
    }

    /// Joined: whether the host's lobby holds our pick, class and loadout, as `own` has it now.
    /// The host's last lobby is the only word on what it will give our hero, so a pick that
    /// differs is one still on its way, or one the host started the run without. Alone or
    /// hosting there is no host to disagree, and this is true.
    ///
    /// [`super::run::start_run`] refuses to enter a raid while this is false, so the loadout the
    /// stash commits is the one the host's hero is given. The host, for its part, starts only when
    /// its last lobby holds every joiner's pick ([`Net::lobby_behind`]), and a joiner's moves are
    /// frozen while they are ready, so the two agree.
    pub fn host_holds(&self, own: &HeroPick) -> bool {
        match &self.role {
            Role::Client(c) => lobby_holds(&c.lobby, c.me, own),
            _ => true,
        }
    }

    /// The host: the first joined player whose loadout has not come yet (a hello carries none).
    /// A run waits for them, or it would begin with their hands empty.
    pub fn awaiting_pick(&self) -> Option<String> {
        match &self.role {
            Role::Host(h) => {
                first_unpicked(h.peers.iter().map(|p| (p.picked, p.hero.name.as_str())))
                    .map(str::to_owned)
            }
            _ => None,
        }
    }

    /// The host: tell every client the run begins.
    pub fn broadcast_start(&mut self) {
        if let Role::Host(h) = &mut self.role {
            h.steps = 0;
            h.raid = h.raid.wrapping_add(1);
            // The next Start waits for each joiner's pick after this raid, not the one before it.
            clear_picks(h.peers.iter_mut().map(|p| &mut p.picked));
            let raid = h.raid;
            h.host.lock().unwrap().broadcast(&ServerMsg::Start { raid });
        }
    }

    /// The host: tell every client the run is over, and the lobby is back.
    pub fn broadcast_ended(&mut self) {
        if let Role::Host(h) = &mut self.role {
            h.host.lock().unwrap().broadcast(&ServerMsg::Ended);
            h.sent = None;
            h.sent_poses = None;
            // The next raid is planned for the lobby this ends in, and nobody is ready for it yet.
            h.planned = plan_raid();
            for p in &mut h.peers {
                p.ready = false;
            }
        }
    }

    /// Back in the inn alone: the next raid planned anew.
    pub fn replan_alone(&mut self) {
        if matches!(self.role, Role::Solo) {
            self.solo_raid = Some(plan_raid());
        }
    }
}

/// This player's host key, `benilla-config/together-host-key.txt`: the same file Survivors uses,
/// so a friend's saved invite code reaches whichever game the host is playing. A key that cannot
/// be saved still hosts, under a new code.
fn host_key() -> [u8; 32] {
    let path = crate::local_state::home().map(|h| h.join("together-host-key.txt"));
    let saved = path
        .as_ref()
        .and_then(|p| crate::local_state::read_to_string(p).ok())
        .and_then(|text| decode_hex(text.trim()));
    if let Some(key) = saved {
        return key;
    }
    let key = new_host_key();
    if let Some(p) = &path {
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        let _ = crate::local_state::write_atomic(p, &hex);
    }
    key
}

/// 64 hex digits as 32 bytes.
fn decode_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

/// This machine's address on the local network: the one a connection out would leave from (no
/// packet is sent to find it).
fn lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// The name the player's hero goes by: `NIGHTFALL_NAME`, `TOGETHER_NAME`, else the Windows user.
pub(super) fn hero_name() -> String {
    std::env::var("NIGHTFALL_NAME")
        .or_else(|_| std::env::var("TOGETHER_NAME"))
        .or_else(|_| std::env::var("USERNAME"))
        .map_or_else(|_| "Hero".into(), |n| clean_name(&n))
}

/// A hero's name as players see it: control characters removed, spaces trimmed, at most 12
/// characters, and "Hero" when nothing is left. Our own and every joining player's.
fn clean_name(raw: &str) -> String {
    let visible: String = raw.chars().filter(|c| !c.is_control()).collect();
    let name: String = visible.trim().chars().take(12).collect();
    if name.is_empty() {
        "Hero".into()
    } else {
        name
    }
}

/// The local pick as the wire carries it, bringing `loadout`.
pub(super) fn own_pick(selection: &Selection, loadout: Loadout) -> HeroPick {
    let content = content();
    let class = &content.classes[selection.class];
    HeroPick {
        name: hero_name(),
        class: class.key.clone(),
        race: class.race,
        sex: class.sex,
        loadout,
    }
}

/// What the local hero brings into a raid now: the profile's current
/// [`nightfall_sim::Profile::raid_loadout`] (the loadout, the class's free kit when it is empty,
/// and the body waiting on this map). One place for the lobby's pick (what a joined player tells
/// the host) and the host's own run, so what a hero is told to bring and what
/// [`super::stash::Stash::begin_raid`] commits are the same.
pub(super) fn own_loadout(selection: &Selection, stash: &Stash) -> Loadout {
    stash.profile.raid_loadout(selection.class, &content())
}

/// The content files' hash, which a client and its host must share.
fn our_content_hash() -> u64 {
    let texts: Vec<&str> = super::content_texts().iter().map(String::as_str).collect();
    content_hash(&texts)
}

/// At the menu: the host takes players in and keeps their lobby current (where they stand, who is
/// ready, the planned raid); a client follows its host's lobby and tells it its pick, where its
/// hero stands and whether it is ready.
pub(super) fn lobby(
    mut net: ResMut<Net>,
    selection: Res<Selection>,
    stash: Res<Stash>,
    mut inn_state: ResMut<InnState>,
) {
    let own = own_pick(&selection, own_loadout(&selection, &stash));
    let net = &mut *net;
    match &mut net.role {
        Role::Solo => {
            if net.solo_raid.is_none() {
                net.solo_raid = Some(plan_raid());
            }
        }
        Role::Host(h) => {
            let host = h.host.lock().unwrap();
            if !h.logged_code {
                if let Invite::Ready(code) = host.invite() {
                    info!("nightfall: invite code {code}");
                    h.logged_code = true;
                }
            }
            for event in host.poll() {
                match event {
                    HostEvent::Message(
                        conn,
                        ClientMsg::Hello {
                            protocol,
                            content,
                            mut hero,
                        },
                    ) => {
                        // A second hello from a player already in is not a new player.
                        if h.peers.iter().any(|p| p.conn == conn) {
                            continue;
                        }
                        hero.name = clean_name(&hero.name);
                        let hero = hero.bringing(Loadout::default());
                        if let Some(why) =
                            refusal(protocol, content, our_content_hash(), h.peers.len())
                        {
                            host.send(conn, &ServerMsg::Refused(why));
                            host.kick(conn);
                            continue;
                        }
                        let player = free_player(h.peers.iter().map(|p| p.player))
                            .expect("refusal checked for room");
                        host.send(conn, &ServerMsg::Welcome { you: player });
                        net.status = format!("{} joined", hero.name);
                        // A new seat is a Lobby even if it looks like the one just left.
                        h.sent = None;
                        h.peers.push(Peer {
                            conn,
                            player,
                            hero,
                            picked: false,
                            pose: None,
                            ready: false,
                            pending: Vec::new(),
                        });
                    }
                    HostEvent::Message(conn, ClientMsg::Pick(mut hero)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            hero.name = clean_name(&hero.name);
                            // What they bring changed: whatever they were ready with is not
                            // this. (A client freezes its loadout while ready, so only one that
                            // does not keep to that gets here; it must ready again.)
                            p.ready = ready_after_pick(p.ready, &p.hero, &hero);
                            p.hero = hero;
                            p.picked = true;
                        }
                    }
                    HostEvent::Message(conn, ClientMsg::InnPose(pose)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            if let Some(pose) = sane_pose(pose, |p| inn().place.bounds.clamp(p)) {
                                p.pose = Some(pose);
                            }
                        }
                    }
                    HostEvent::Message(conn, ClientMsg::Ready(ready)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            // Only with a pick in: a hello carries no loadout.
                            p.ready = ready_after(ready, p.picked);
                        }
                    }
                    HostEvent::Left(conn) => {
                        if let Some(i) = h.peers.iter().position(|p| p.conn == conn) {
                            net.status = format!("{} left", h.peers[i].hero.name);
                            h.peers.remove(i);
                            h.sent = None;
                        }
                    }
                    HostEvent::Message(_, ClientMsg::Input { .. }) | HostEvent::Joined(..) => {}
                }
            }
            // Compared with what went last before any loadout is cloned: most frames nothing changed.
            let same = h.sent.as_ref().is_some_and(|s| {
                s.len() == 1 + h.peers.len()
                    && listed(&s[0], 0, &own, inn_state.ready)
                    && h.peers
                        .iter()
                        .zip(&s[1..])
                        .all(|(p, l)| listed(l, p.player, &p.hero, p.ready))
            });
            let poses = pose_list(inn_state.pose, h.peers.iter().map(|p| (p.player, p.pose)));
            let poses_same = h.sent_poses.as_ref() == Some(&poses);
            let (send_lobby, send_poses) = inn_sends(
                !same,
                !poses_same,
                h.sent_at.map(|t| t.elapsed().as_secs_f32()),
                h.sent_poses_at.map(|t| t.elapsed().as_secs_f32()),
            );
            if send_lobby {
                let mut players = vec![LobbyPlayer {
                    player: 0,
                    hero: own,
                    pose: None,
                    ready: inn_state.ready,
                }];
                players.extend(h.peers.iter().map(|p| LobbyPlayer {
                    player: p.player,
                    hero: p.hero.clone(),
                    pose: None,
                    ready: p.ready,
                }));
                host.broadcast(&ServerMsg::Lobby {
                    players: players.clone(),
                    raid: h.planned.clone(),
                });
                h.sent = Some(players);
                h.sent_at = Some(Instant::now());
            }
            if send_poses {
                host.broadcast(&ServerMsg::InnPoses(poses.clone()));
                h.sent_poses = Some(poses);
                h.sent_poses_at = Some(Instant::now());
            }
            drop(host);
        }
        Role::Client(c) => {
            let events = c.client.lock().unwrap().poll();
            let mut gone = None;
            for event in events {
                match event {
                    ClientEvent::Connected => {
                        net.status = "Connected, joining…".into();
                        c.client.lock().unwrap().send(&ClientMsg::Hello {
                            protocol: PROTOCOL,
                            content: our_content_hash(),
                            hero: own.who(),
                        });
                    }
                    ClientEvent::Message(ServerMsg::Welcome { you }) => {
                        c.me = Some(you);
                        net.status = "Joined: press Ready when your loadout is set".into();
                    }
                    ClientEvent::Message(ServerMsg::Refused(why)) => gone = Some(why),
                    ClientEvent::Message(ServerMsg::Lobby { players, raid }) => {
                        c.poses.retain(|id, _| players.iter().any(|p| p.player == *id));
                        c.lobby = players;
                        c.planned = Some(raid);
                    }
                    ClientEvent::Message(ServerMsg::InnPoses(poses)) => {
                        c.poses = sane_poses(poses, |p| inn().place.bounds.clamp(p));
                    }
                    ClientEvent::Message(ServerMsg::Start { raid }) => {
                        c.raid = raid;
                        c.start = true;
                    }
                    ClientEvent::Message(
                        ServerMsg::State { .. }
                        | ServerMsg::Ended
                        | ServerMsg::Outcome(_)
                        | ServerMsg::RunOver { .. },
                    ) => {}
                    ClientEvent::Disconnected(why) => gone = Some(format!("Disconnected: {why}")),
                }
            }
            if let Some(why) = gone {
                net.leave(why);
                inn_state.reset();
                return;
            }
            if c.me.is_none() {
                // Not welcomed (yet): nothing of an earlier session's Ready carries over.
                inn_state.reset();
            }
            if c.me.is_some() && c.sent_pick.as_ref() != Some(&own) {
                // A pick that changes after it was sent (the class, say) is no longer the one we
                // were ready with.
                inn_state.ready = ready_after_own_pick(inn_state.ready, c.sent_pick.is_some());
                c.client.lock().unwrap().send(&ClientMsg::Pick(own.clone()));
                c.sent_pick = Some(own);
            }
            if c.me.is_some() {
                // A test run is ready as soon as its pick is in.
                if super::run::autoplay() && c.sent_pick.is_some() {
                    inn_state.ready = true;
                }
                if let Some(pose) = inn_state.pose {
                    let last = c.sent_pose.map(|(p, _)| p);
                    let due = c
                        .sent_pose
                        .is_none_or(|(_, t)| t.elapsed().as_secs_f32() >= LOBBY_EVERY);
                    if due && moved(last, pose) {
                        c.client.lock().unwrap().send(&ClientMsg::InnPose(pose));
                        c.sent_pose = Some((pose, Instant::now()));
                    }
                }
                // Ready goes after the pick on the wire (above), so the host holds what we bring.
                if let Some(ready) =
                    ready_to_send(inn_state.ready, c.sent_pick.is_some(), c.sent_ready)
                {
                    c.client.lock().unwrap().send(&ClientMsg::Ready(ready));
                    c.sent_ready = Some(ready);
                }
            }
        }
    }
}

/// A client's input into the world, if it was made in this raid (`now`): one tagged with another
/// raid's number (still on its way when the last raid ended) is dropped, so it can neither move a
/// hero of this raid nor mark their client as having entered it. Whether it was taken.
fn take_input(
    world: &mut World,
    player: PlayerId,
    now: u32,
    sent: u32,
    input: PlayerInput,
) -> bool {
    if sent != now {
        return false;
    }
    world.set_input(player, input);
    true
}

/// The host, during a run: the clients' inputs into the world, and a player who leaves out of it.
/// A newcomer is turned away until the run ends.
pub(super) fn host_receive(net: &mut Net, world: &mut World) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    let host = h.host.lock().unwrap();
    for event in host.poll() {
        match event {
            HostEvent::Message(conn, ClientMsg::Input { raid, input }) => {
                if let Some(p) = h.peers.iter().find(|p| p.conn == conn) {
                    take_input(world, p.player, h.raid, raid, input);
                }
            }
            HostEvent::Message(conn, ClientMsg::Hello { .. }) => {
                host.send(
                    conn,
                    &ServerMsg::Refused("A run is under way; join when it ends.".into()),
                );
                host.kick(conn);
            }
            HostEvent::Left(conn) => {
                if let Some(i) = h.peers.iter().position(|p| p.conn == conn) {
                    world.remove_hero(h.peers[i].player);
                    net.status = format!("{} left", h.peers[i].hero.name);
                    h.peers.remove(i);
                }
            }
            HostEvent::Message(
                _,
                ClientMsg::Pick(_) | ClientMsg::InnPose(_) | ClientMsg::Ready(_),
            )
            | HostEvent::Joined(..) => {}
        }
    }
}

/// Whether `player` (`me`) is told of `event`: it involves their own hero, or a unit `near` them
/// (within their snapshot's reach, [`World::in_reach`]). An event with several units is told if
/// any is. A refusal is told only to its own player, whoever is near. So a far-off fight is never
/// sent, and a modified client learns nothing of it.
pub(super) fn event_for(event: &Event, me: PlayerId, near: impl Fn(UnitRef) -> bool) -> bool {
    let me_ref = UnitRef::Hero(me);
    let involves = |u: UnitRef| u == me_ref || near(u);
    let hero = |p: PlayerId| involves(UnitRef::Hero(p));
    match event {
        Event::Refused { player, .. } => *player == me,
        Event::CastStart { by, target, .. } | Event::ChannelStart { by, target, .. } => {
            involves(*by) || target.is_some_and(involves)
        }
        Event::CastGo { by, hits, .. } => involves(*by) || hits.iter().any(|&u| involves(u)),
        Event::CastStop { by, .. } => involves(*by),
        Event::Damage { to, by, .. } => involves(*to) || by.is_some_and(involves),
        Event::Swing { by, to } => involves(*by) || involves(*to),
        Event::Evaded { player, enemy } => hero(*player) || involves(UnitRef::Enemy(*enemy)),
        Event::Dodged { player }
        | Event::Blinked { player, .. }
        | Event::Downed { player }
        | Event::Died { player } => hero(*player),
        Event::Revived { player, by } => hero(*player) || hero(*by),
        Event::EnemyDied { id, by } => involves(UnitRef::Enemy(*id)) || by.is_some_and(hero),
    }
}

/// The host, after a step: the step's events (from `events_from` on) kept for each client they
/// concern ([`event_for`]), and every [`SEND_EVERY`] steps each client whose connection keeps up
/// gets the world as its player sees it, with the events since its last state.
pub(super) fn host_send(net: &mut Net, world: &World, events_from: usize) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    if h.peers.is_empty() {
        return;
    }
    let events = &world.events()[events_from..];
    for p in &mut h.peers {
        // Only what happened near their hero, as their snapshot holds only the enemies there.
        p.pending.extend(
            events
                .iter()
                .filter(|e| event_for(e, p.player, |u| world.in_reach(p.player, u)))
                .cloned(),
        );
        let over = p.pending.len().saturating_sub(MAX_PENDING);
        p.pending.drain(..over);
    }
    h.steps += 1;
    if h.steps % SEND_EVERY != 0 {
        return;
    }
    // Each player gets the enemies near their own hero and which of them they see (spec §4.3).
    let host = h.host.lock().unwrap();
    for p in &mut h.peers {
        if host.backlog(p.conn) > MAX_BACKLOG {
            continue;
        }
        host.send(
            p.conn,
            &ServerMsg::State {
                snapshot: world.snapshot_for(p.player),
                seen: world.seen_by(p.player),
                marks: world.marks_for(p.player),
                events: std::mem::take(&mut p.pending),
            },
        );
    }
}

/// The host: `outcome` to its player, once.
pub(super) fn send_outcome(net: &mut Net, outcome: &HeroOutcome) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    if let Some(p) = h.peers.iter().find(|p| p.player == outcome.player) {
        h.host
            .lock()
            .unwrap()
            .send(p.conn, &ServerMsg::Outcome(outcome.clone()));
    }
}

/// The host: the raid is over; everyone's outcome to everyone, for the end screen.
pub(super) fn broadcast_run_over(net: &mut Net, outcomes: &[HeroOutcome]) {
    if let Role::Host(h) = &mut net.role {
        h.host.lock().unwrap().broadcast(&ServerMsg::RunOver {
            outcomes: outcomes.to_vec(),
        });
    }
}

/// What a client heard from its host during a run.
pub(super) enum Heard {
    /// A state, which enemies in it our hero sees, what they think of our hero, and what
    /// happened since the last.
    State(Snapshot, Vec<u32>, Vec<Mark>, Vec<Event>),
    /// Our raid's outcome.
    Outcome(HeroOutcome),
    /// Every hero's, at the raid's end.
    RunOver(Vec<HeroOutcome>),
    Ended,
    /// The host's lobby, which may come with (or after) `Ended` in one poll, before the lobby
    /// system has run.
    Lobby(Vec<LobbyPlayer>, RaidRoll),
    /// Where everyone stands in the inn: sent once as the host returns to it, so one that follows
    /// `Ended` is kept like the lobby.
    InnPoses(Vec<(PlayerId, HeroPose)>),
    /// The host started the next run (number `raid`), possibly in the same poll as `Ended`.
    Start(u32),
    Lost(String),
}

/// A client, during a run: this frame's pose to the host (at most once a step's time), and what
/// the host sent.
pub(super) fn client_exchange(net: &mut Net, input: PlayerInput) -> Vec<Heard> {
    let Role::Client(c) = &mut net.role else {
        return Vec::new();
    };
    let client = c.client.lock().unwrap();
    let due = input.pose.is_some() && c.last_pose.is_none_or(|t| t.elapsed().as_secs_f32() >= DT);
    if due {
        client.send(&ClientMsg::Input {
            raid: c.raid,
            input,
        });
        c.last_pose = Some(Instant::now());
    }
    let mut heard = Vec::new();
    for event in client.poll() {
        match event {
            ClientEvent::Message(ServerMsg::State {
                snapshot,
                seen,
                marks,
                events,
            }) => {
                heard.push(Heard::State(snapshot, seen, marks, events));
            }
            ClientEvent::Message(ServerMsg::Ended) => heard.push(Heard::Ended),
            ClientEvent::Message(ServerMsg::Lobby { players, raid }) => {
                heard.push(Heard::Lobby(players, raid));
            }
            ClientEvent::Message(ServerMsg::InnPoses(poses)) => heard.push(Heard::InnPoses(poses)),
            ClientEvent::Message(ServerMsg::Start { raid }) => heard.push(Heard::Start(raid)),
            ClientEvent::Message(ServerMsg::Outcome(o)) => heard.push(Heard::Outcome(o)),
            ClientEvent::Message(ServerMsg::RunOver { outcomes }) => {
                heard.push(Heard::RunOver(outcomes));
            }
            ClientEvent::Disconnected(why) => heard.push(Heard::Lost(why)),
            ClientEvent::Message(_) | ClientEvent::Connected => {}
        }
    }
    heard
}

/// What `pick` finds in the last of `heard` that came after the last `Ended` (nothing without one).
/// A host that ends a run and starts the next within one poll of ours sends `Ended`, its lobby and
/// `Start` together: the lobby system, which reads them, has not run yet.
fn after_ended<'a, T>(heard: &'a [Heard], pick: impl Fn(&'a Heard) -> Option<T>) -> Option<T> {
    let at = heard.iter().rposition(|h| matches!(h, Heard::Ended))?;
    heard[at + 1..].iter().rev().find_map(pick)
}

/// Whether the client's run is over: the host ended it (the session stays) or the connection is
/// gone (the session ends). A lobby and a start that came right after the end are kept for the
/// menu, so a quick next run is not missed.
pub(super) fn client_left_run(net: &mut Net, heard: &[Heard]) -> bool {
    for h in heard {
        match h {
            Heard::Ended => {
                net.status = "The host ended the run".into();
                let lobby = after_ended(heard, |h| match h {
                    Heard::Lobby(players, raid) => Some((players.clone(), raid.clone())),
                    _ => None,
                });
                let start = after_ended(heard, |h| match h {
                    Heard::Start(raid) => Some(*raid),
                    _ => None,
                });
                let poses = after_ended(heard, |h| match h {
                    Heard::InnPoses(poses) => Some(poses.clone()),
                    _ => None,
                });
                if let Role::Client(c) = &mut net.role {
                    c.start = false;
                    // Where friends stood before the raid is not where they stand now; the host
                    // sends the new ones once, with or just after the end.
                    c.poses = sane_poses(poses.unwrap_or_default(), |p| inn().place.bounds.clamp(p));
                    // The host wants a pick from us after this raid, even if it is the same one: it
                    // cleared ours when the raid started.
                    c.sent_pick = None;
                    // The host cleared our Ready too: the next one must go out even if it equals
                    // the last we sent, or a second raid never starts.
                    c.sent_ready = None;
                    if let Some((players, raid)) = lobby {
                        c.lobby = players;
                        c.planned = Some(raid);
                    }
                    if let Some(raid) = start {
                        c.raid = raid;
                        c.start = true;
                    }
                }
                return true;
            }
            Heard::Lost(why) => {
                net.leave(format!("Disconnected: {why}"));
                return true;
            }
            Heard::State(..)
            | Heard::Outcome(_)
            | Heard::RunOver(_)
            | Heard::Lobby(..)
            | Heard::InnPoses(_)
            | Heard::Start(_) => {}
        }
    }
    false
}

/// Leaving the game for good: no lingering session.
pub(super) fn close_on_exit(mut exits: MessageReader<AppExit>, mut net: ResMut<Net>) {
    if exits.read().next().is_some() {
        net.leave("");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_trimmed_to_twelve_visible_characters() {
        assert_eq!(clean_name("Jaina"), "Jaina");
        assert_eq!(clean_name("Bad\u{7}\nName\t"), "BadName");
        assert_eq!(clean_name("  Thrall  "), "Thrall");
        assert_eq!(clean_name("Abcdefghijklmnop"), "Abcdefghijkl");
        assert_eq!(clean_name("Ünïcödé-Ñåmé-plus"), "Ünïcödé-Ñåmé");
        assert_eq!(clean_name("\u{1b}\u{1b}"), "Hero");
        assert_eq!(clean_name(""), "Hero");
    }

    fn pick(name: &str) -> HeroPick {
        HeroPick {
            name: name.into(),
            class: "rogue".into(),
            race: 1,
            sex: 0,
            loadout: Loadout::default(),
        }
    }

    #[test]
    fn a_hello_is_refused_for_version_content_or_a_full_party() {
        assert_eq!(refusal(PROTOCOL, 7, 7, 0), None);
        assert!(refusal(PROTOCOL + 1, 7, 7, 0).unwrap().contains("version"));
        assert!(refusal(PROTOCOL, 8, 7, 0).unwrap().contains("content"));
        assert!(refusal(PROTOCOL, 7, 7, MAX_PLAYERS - 1)
            .unwrap()
            .contains("full"));
    }

    #[test]
    fn players_are_numbered_from_one_without_reuse() {
        assert_eq!(free_player([].into_iter()), Some(1));
        assert_eq!(free_player([1, 2].into_iter()), Some(3));
        assert_eq!(free_player([1, 3].into_iter()), Some(2));
        assert_eq!(free_player([1, 2, 3].into_iter()), None);
    }

    #[test]
    fn the_roster_marks_us() {
        let players = vec![
            LobbyPlayer {
                player: 0,
                hero: pick("Host"),
                pose: None,
                ready: false,
            },
            LobbyPlayer {
                player: 1,
                hero: pick("Me"),
                pose: None,
                ready: false,
            },
        ];
        let text = roster_lines(&players, Some(1));
        assert!(text.contains("Me  ·  rogue  (you)"));
        assert!(!text.contains("Host  ·  rogue  (you)"));
        assert!(!text.contains("ready"));
        // Whoever is ready says so.
        let mut ready = players;
        ready[0].ready = true;
        let text = roster_lines(&ready, Some(1));
        assert!(text.contains("Host  ·  rogue  (ready)"), "{text}");
        assert!(text.contains("Me  ·  rogue  (you)\n") || text.ends_with("Me  ·  rogue  (you)"));
    }

    #[test]
    fn the_host_holds_our_pick_only_when_its_lobby_lists_it_for_us() {
        let mut ours = pick("Me");
        let lobby = |hero: HeroPick| {
            vec![
                LobbyPlayer {
                    player: 0,
                    hero: pick("Host"),
                    pose: None,
                    ready: false,
                },
                LobbyPlayer {
                    player: 1,
                    hero,
                    pose: None,
                    ready: false,
                },
            ]
        };
        assert!(lobby_holds(&lobby(ours.clone()), Some(1), &ours));
        // Not listed yet, or not welcomed yet.
        assert!(!lobby_holds(&[], Some(1), &ours));
        assert!(!lobby_holds(&lobby(ours.clone()), None, &ours));
        // The host holds an older loadout or class than ours.
        let old = ours.clone();
        ours.loadout.inventory.pack = vec![Some(nightfall_sim::Stack::new(0, 1))];
        assert!(!lobby_holds(&lobby(old.clone()), Some(1), &ours));
        let mut other_class = old.clone();
        other_class.class = "mage".into();
        assert!(!lobby_holds(&lobby(other_class), Some(1), &old));
        // Only class and loadout count: the name is the host's to clean.
        let mut renamed = old.clone();
        renamed.name = "Cleaned".into();
        assert!(lobby_holds(&lobby(renamed), Some(1), &old));
        // Alone or hosting there is no host to disagree.
        assert!(Net::default().host_holds(&ours));
    }

    #[test]
    fn a_new_raid_waits_for_every_joiners_fresh_pick() {
        let mut picks = [(true, "A"), (true, "B")];
        assert_eq!(first_unpicked(picks), None, "both have picked");
        let mut flags = [true, true];
        clear_picks(flags.iter_mut());
        picks = [(flags[0], "A"), (flags[1], "B")];
        assert_eq!(first_unpicked(picks), Some("A"), "the next start waits");
        // One picks again after the raid, the other has not yet.
        flags[0] = true;
        picks = [(flags[0], "A"), (flags[1], "B")];
        assert_eq!(first_unpicked(picks), Some("B"));
    }

    #[test]
    fn a_run_waits_for_the_first_joiner_who_has_not_picked() {
        assert_eq!(first_unpicked([]), None);
        assert_eq!(first_unpicked([(true, "A"), (true, "B")]), None);
        assert_eq!(
            first_unpicked([(true, "A"), (false, "B"), (false, "C")]),
            Some("B")
        );
        assert_eq!(Net::default().awaiting_pick(), None);
    }

    #[test]
    fn an_input_from_the_last_raid_neither_moves_nor_enters_a_hero_of_this_one() {
        use nightfall_sim::{HeroPose, V2};
        let content = super::super::shipped_content();
        let mut world = World::new(content.clone());
        world.add_hero(1, "rogue", "Joiner", 1, 0).unwrap();
        let start = world.hero(1).unwrap().pos;
        let input = PlayerInput {
            pose: Some(HeroPose {
                pos: start + V2::new(3.0, 0.0),
                facing: V2::new(1.0, 0.0),
                z: 0.0,
            }),
            ..PlayerInput::default()
        };
        // Raid 2 is on; the input was tagged raid 1.
        assert!(!take_input(&mut world, 1, 2, 1, input));
        world.step();
        assert!(!world.entered(1), "a stale input does not enter");
        assert_eq!(world.hero(1).unwrap().pos, start, "nor move");
        // One tagged with this raid does both.
        assert!(take_input(&mut world, 1, 2, 2, input));
        world.step();
        assert!(world.entered(1));
        assert_ne!(world.hero(1).unwrap().pos, start);
    }

    #[test]
    fn a_start_that_comes_with_the_end_is_kept_for_the_menu() {
        let plan = RaidRoll {
            seed: 9,
            entry: 0,
            exits: Vec::new(),
        };
        let lobby = |n: &str| {
            Heard::Lobby(
                vec![LobbyPlayer {
                    player: 1,
                    hero: pick(n),
                    pose: None,
                    ready: false,
                }],
                plan.clone(),
            )
        };
        let start_of = |heard: &[Heard]| {
            after_ended(heard, |h| match h {
                Heard::Start(raid) => Some(*raid),
                _ => None,
            })
        };
        // The host ended the run and started the next in one poll.
        let heard = [Heard::Ended, lobby("A"), Heard::Start(2)];
        assert_eq!(start_of(&heard), Some(2));
        // The lobby that came with the end brings the planned raid with it.
        let players = after_ended(&heard, |h| match h {
            Heard::Lobby(p, raid) => Some((p.len(), raid.seed)),
            _ => None,
        });
        assert_eq!(players, Some((1, 9)));
        // A start before the end belongs to the run that ended; none is no start.
        assert_eq!(start_of(&[Heard::Start(1), Heard::Ended]), None);
        assert_eq!(start_of(&[Heard::Ended]), None);
        assert_eq!(start_of(&[]), None);
        // After two ends, the last start after the last end.
        let heard = [Heard::Ended, Heard::Start(2), Heard::Ended, Heard::Start(3)];
        assert_eq!(start_of(&heard), Some(3));
        let heard = [Heard::Ended, Heard::Start(2), Heard::Ended];
        assert_eq!(start_of(&heard), None);
    }

    #[test]
    fn the_poses_that_come_with_the_end_are_kept_and_the_old_ones_forgotten() {
        use nightfall_sim::V2;
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let _ = super::super::INN.set(super::super::shipped_inn());
        let mut net = Net::default();
        net.join("127.0.0.1:9");
        let at = inn().place.spawn;
        let pose = |x: f32| HeroPose {
            pos: V2::new(at.x + x, at.y),
            facing: V2::new(1.0, 0.0),
            z: at.z,
        };
        if let Role::Client(c) = &mut net.role {
            c.poses.insert(1, pose(0.0));
        }
        // The inn's poses arrive in the poll that carries the end.
        let heard = [Heard::Ended, Heard::InnPoses(vec![(1, pose(1.0))])];
        assert!(client_left_run(&mut net, &heard));
        let Role::Client(c) = &net.role else { panic!() };
        assert_eq!(c.poses.get(&1).map(|p| p.pos.x), Some(at.x + 1.0));
        // And with none after it, the stale ones are gone, not left standing.
        if let Role::Client(c) = &mut net.role {
            c.poses.insert(2, pose(0.0));
        }
        assert!(client_left_run(&mut net, &[Heard::Ended]));
        let Role::Client(c) = &net.role else { panic!() };
        assert!(c.poses.is_empty());
    }

    #[test]
    fn the_planned_raid_that_comes_with_the_end_is_kept_for_the_inn() {
        // `join` only starts a connection attempt in the background; nothing is polled here.
        let mut net = Net::default();
        net.join("127.0.0.1:9");
        assert_eq!(
            net.planned_raid(),
            None,
            "nothing planned before the host's lobby"
        );
        let plan = RaidRoll {
            seed: 5,
            entry: 0,
            exits: Vec::new(),
        };
        let heard = [Heard::Ended, Heard::Lobby(Vec::new(), plan.clone())];
        assert!(client_left_run(&mut net, &heard));
        assert_eq!(net.planned_raid(), Some(plan));
    }

    #[test]
    fn a_raid_ending_makes_the_client_tell_its_ready_again() {
        let mut net = Net::default();
        net.join("127.0.0.1:9");
        if let Role::Client(c) = &mut net.role {
            c.sent_ready = Some(true);
            c.sent_pick = Some(pick("Me"));
        }
        assert!(client_left_run(&mut net, &[Heard::Ended]));
        let Role::Client(c) = &net.role else { panic!() };
        assert_eq!(c.sent_ready, None);
        assert_eq!(c.sent_pick, None);
        // So an autoplay joiner's Ready(true) for the second raid goes out.
        assert_eq!(ready_to_send(true, true, c.sent_ready), Some(true));
    }

    #[test]
    fn the_four_ready_rules() {
        // The host takes Ready only after a pick.
        assert!(!ready_after(true, false));
        assert!(ready_after(true, true));
        assert!(!ready_after(false, true));
        // The host clears Ready on a pick that changes what they bring (not on a rename).
        let a = pick("Me");
        let mut renamed = a.clone();
        renamed.name = "X".into();
        let mut other = a.clone();
        other.class = "mage".into();
        assert!(ready_after_pick(true, &a, &renamed));
        assert!(!ready_after_pick(true, &a, &other));
        assert!(!ready_after_pick(false, &a, &a));
        // The client holds Ready back until its pick is sent, sends changes, and re-sends after a
        // reset; Not Ready needs no pick.
        assert_eq!(ready_to_send(true, false, None), None);
        assert_eq!(ready_to_send(true, true, None), Some(true));
        assert_eq!(ready_to_send(true, true, Some(true)), None);
        assert_eq!(ready_to_send(false, true, Some(true)), Some(false));
        assert_eq!(ready_to_send(false, false, Some(true)), Some(false));
        assert_eq!(ready_to_send(false, false, None), Some(false));
        // The client un-readies when its pick changes after one was sent.
        assert!(!ready_after_own_pick(true, true));
        assert!(ready_after_own_pick(true, false));
        assert!(!ready_after_own_pick(false, true));
    }

    #[test]
    fn an_inn_pose_is_made_sane() {
        use nightfall_sim::V2;
        let clamp = |p: V2| V2::new(p.x.clamp(-10.0, 10.0), p.y);
        let bad = HeroPose {
            pos: V2::new(f32::NAN, 0.0),
            facing: V2::new(1.0, 0.0),
            z: 0.0,
        };
        assert_eq!(sane_pose(bad, clamp), None);
        let wild = HeroPose {
            pos: V2::new(50.0, 1.0),
            facing: V2::new(40.0, 0.0),
            z: 1.0e9,
        };
        let sane = sane_pose(wild, clamp).unwrap();
        assert_eq!(sane.pos, V2::new(10.0, 1.0));
        assert!((sane.facing.length() - 1.0).abs() < 1e-4);
        assert_eq!(sane.z, 3000.0);
    }

    #[test]
    fn alone_the_inn_plans_the_next_raid_and_replans_it_after_each() {
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let mut net = Net::default();
        assert_eq!(net.planned_raid(), None);
        net.solo_raid = Some(RaidRoll {
            seed: 1,
            entry: 0,
            exits: Vec::new(),
        });
        assert_eq!(net.planned_raid().map(|r| r.seed), Some(1));
        // Alone, the Start is the Ready; hosting needs the party, a client never starts.
        assert!(net.party_ready(true));
        assert!(!net.party_ready(false));
        assert_eq!(net.first_unready(), None);
        assert_eq!(net.lobby_behind(), None);
        assert!(!net.welcomed());
        net.replan_alone();
        assert!(net.solo_raid.is_some());
    }

    #[test]
    fn a_pose_only_change_sends_no_loadouts() {
        use nightfall_sim::V2;
        let at = |x: f32| HeroPose {
            pos: V2::new(x, 0.0),
            facing: V2::new(1.0, 0.0),
            z: 0.0,
        };
        // The lobby as sent lists nobody's pose, and it still holds a player who has since walked.
        let sent = LobbyPlayer {
            player: 1,
            hero: pick("A"),
            pose: None,
            ready: false,
        };
        // A walk alone: nothing the lobby lists differs, the poses do, so only InnPoses goes.
        let lobby_changed = !listed(&sent, 1, &pick("A"), false);
        let poses_changed = pose_list(Some(at(0.0)), [(1, Some(at(1.0)))])
            != pose_list(Some(at(0.0)), [(1, Some(at(2.0)))]);
        assert_eq!(
            inn_sends(lobby_changed, poses_changed, Some(5.0), Some(5.0)),
            (false, true)
        );
        assert!(!listed(&sent, 1, &pick("B"), false), "a new loadout changes it");
        assert!(!listed(&sent, 1, &pick("A"), true), "so does ready");
        // So a walk changes the poses (not the lobby): only the poses go.
        let before = pose_list(Some(at(0.0)), [(1, Some(at(1.0))), (2, None)]);
        let after = pose_list(Some(at(0.0)), [(1, Some(at(2.0))), (2, None)]);
        assert_eq!(before.len(), 2, "a player with no pose is left out");
        assert_ne!(before, after);
        assert_eq!(inn_sends(false, true, Some(5.0), Some(5.0)), (false, true));
        assert_eq!(inn_sends(false, false, Some(5.0), Some(5.0)), (false, false));
        // Poses keep the ten-a-second cap.
        assert_eq!(inn_sends(false, true, Some(5.0), Some(0.05)), (false, false));
        // A lobby carries the poses with it, so someone just welcomed sees everyone at once.
        assert_eq!(inn_sends(true, false, None, Some(0.0)), (true, true));
    }

    #[test]
    fn the_lobby_goes_when_it_changes_and_no_faster() {
        assert!(lobby_due(true, None));
        assert!(!lobby_due(false, None));
        assert!(!lobby_due(true, Some(0.05)));
        assert!(lobby_due(true, Some(0.1)));
        assert!(!lobby_due(false, Some(5.0)));
    }

    #[test]
    fn all_ready_counts_everyone_present() {
        assert!(all_ready(true, []));
        assert!(!all_ready(false, []));
        assert!(!all_ready(true, [true, false]));
        assert!(all_ready(true, [true, true]));
    }

    #[test]
    fn a_pose_goes_only_when_it_moved() {
        use nightfall_sim::V2;
        let at = |x: f32| HeroPose {
            pos: V2::new(x, 0.0),
            facing: V2::new(1.0, 0.0),
            z: 0.0,
        };
        assert!(moved(None, at(0.0)));
        assert!(!moved(Some(at(0.0)), at(0.01)));
        assert!(moved(Some(at(0.0)), at(0.2)));
        // A turn on the spot, or a step up, is also worth sending.
        let turned = HeroPose {
            facing: V2::new(0.0, 1.0),
            ..at(0.0)
        };
        assert!(moved(Some(at(0.0)), turned));
        let up = HeroPose { z: 0.5, ..at(0.0) };
        assert!(moved(Some(at(0.0)), up));
    }

    #[test]
    fn a_start_waits_for_the_first_who_is_not_ready() {
        assert_eq!(first_unready([]), None);
        assert_eq!(first_unready([(true, "A"), (true, "B")]), None);
        assert_eq!(
            first_unready([(true, "A"), (false, "B"), (false, "C")]),
            Some("B")
        );
    }

    #[test]
    fn a_start_waits_for_the_lobby_to_hold_every_picks() {
        let mut brought = pick("Me");
        let mut listed = vec![LobbyPlayer {
            player: 1,
            hero: pick("Me"),
            pose: None,
            ready: true,
        }];
        fn peers(hero: &HeroPick) -> Vec<(PlayerId, &HeroPick)> {
            vec![(1, hero)]
        }
        assert_eq!(first_behind(&listed, peers(&brought)), None);
        // The host holds a new loadout (or class) the last lobby did not tell: wait for it.
        brought.loadout.inventory.pack = vec![Some(nightfall_sim::Stack::new(0, 1))];
        assert_eq!(first_behind(&listed, peers(&brought)), Some("Me"));
        listed[0].hero = brought.clone();
        assert_eq!(first_behind(&listed, peers(&brought)), None);
        // And a player no lobby has listed yet.
        assert_eq!(first_behind(&[], peers(&brought)), Some("Me"));
    }

    #[test]
    fn a_changed_pick_is_not_what_they_were_ready_with() {
        let a = pick("Me");
        let mut renamed = a.clone();
        renamed.name = "Other".into();
        assert!(
            same_bringing(&a, &renamed),
            "the name is the host's to clean"
        );
        let mut other = a.clone();
        other.class = "mage".into();
        assert!(!same_bringing(&a, &other));
        let mut fuller = a.clone();
        fuller.loadout.inventory.pack = vec![Some(nightfall_sim::Stack::new(0, 1))];
        assert!(!same_bringing(&a, &fuller));
    }

    fn damage(to: UnitRef, by: Option<UnitRef>) -> Event {
        Event::Damage {
            to,
            by,
            amount: 10.0,
            crit: false,
            ambush: false,
        }
    }

    #[test]
    fn a_far_enemys_death_is_not_sent_and_a_near_ones_is() {
        let died = |id| Event::EnemyDied { id, by: None };
        assert!(!event_for(&died(1), 0, |u| u == UnitRef::Enemy(2)));
        assert!(event_for(&died(2), 0, |u| u == UnitRef::Enemy(2)));
        // Credit to a hero other than us does not bring a far death with it, but ours does.
        let by = |p| Event::EnemyDied { id: 1, by: Some(p) };
        assert!(!event_for(&by(5), 0, |_| false));
        assert!(event_for(&by(0), 0, |_| false));
    }

    #[test]
    fn our_own_heros_damage_is_always_sent() {
        let blow = damage(UnitRef::Hero(0), Some(UnitRef::Enemy(9)));
        assert!(event_for(&blow, 0, |_| false));
        let ours = damage(UnitRef::Enemy(9), Some(UnitRef::Hero(0)));
        assert!(event_for(&ours, 0, |_| false));
        let theirs = damage(UnitRef::Enemy(9), Some(UnitRef::Hero(1)));
        assert!(!event_for(&theirs, 0, |_| false));
    }

    #[test]
    fn an_event_with_several_units_is_sent_if_any_is_near() {
        let go = Event::CastGo {
            by: UnitRef::Hero(1),
            ability: 0,
            hits: vec![UnitRef::Enemy(3), UnitRef::Enemy(4)],
            at: None,
        };
        assert!(!event_for(&go, 0, |_| false));
        assert!(event_for(&go, 0, |u| u == UnitRef::Enemy(4)));
        assert!(event_for(&go, 0, |u| u == UnitRef::Hero(1)));
        let swing = Event::Swing {
            by: UnitRef::Enemy(3),
            to: UnitRef::Hero(1),
        };
        assert!(event_for(&swing, 0, |u| u == UnitRef::Enemy(3)));
    }

    #[test]
    fn a_refusal_goes_only_to_its_player() {
        let no = Event::Refused {
            player: 1,
            why: nightfall_sim::Refusal::Down,
        };
        assert!(event_for(&no, 1, |_| false));
        assert!(!event_for(&no, 0, |_| true));
    }

    #[test]
    fn what_our_own_hero_does_or_suffers_is_always_sent() {
        let me = UnitRef::Hero(0);
        let at = nightfall_sim::V3::new(1.0, 2.0, 3.0);
        let events = [
            Event::CastStart {
                by: me,
                ability: 0,
                target: None,
                secs: 1.0,
            },
            Event::ChannelStart {
                by: me,
                ability: 0,
                target: None,
                secs: 1.0,
            },
            Event::CastStop {
                by: me,
                ability: 0,
                why: nightfall_sim::Stop::Moved,
            },
            Event::Blinked { player: 0, to: at },
            Event::Dodged { player: 0 },
            Event::Downed { player: 0 },
            Event::Died { player: 0 },
            // Revived by a friend far away, or reviving one.
            Event::Revived { player: 0, by: 1 },
            Event::Revived { player: 1, by: 0 },
            Event::Evaded {
                player: 0,
                enemy: 9,
            },
        ];
        for e in &events {
            assert!(event_for(e, 0, |_| false), "{e:?}");
            // And the same of a hero who is not us and far away is not.
            assert!(!event_for(e, 2, |_| false), "{e:?} for a stranger");
        }
    }
}
