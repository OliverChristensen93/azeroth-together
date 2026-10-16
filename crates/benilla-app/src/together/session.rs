//! Co-op sessions: hosting or joining, the lobby before a run, and the run's traffic.
//!
//! The host runs the game for everyone ([`together_sim::World`]); every [`SEND_EVERY`] steps it
//! sends each client the world and the events since its last ([`ServerMsg::State`]). A client
//! never steps the world: it plays the host's snapshots back at an even pace, a little behind, so a
//! late or bunched packet does not show ([`super::step_sim`]), mirrors them
//! ([`World::apply_snapshot`]) and draws them with the same code the host draws with. It sends
//! where its own controller put its hero and its card choices ([`ClientMsg::Input`]), as a WoW
//! client tells its server its moves.
//!
//! A slow line never builds a queue: while a client's connection still holds unsent snapshots,
//! the host skips it and keeps its events for the next one, so the client sees the newest world,
//! late, rather than an ever older one.

use std::sync::Mutex;

use bevy::prelude::*;
use together_net::{
    content_hash, new_host_key, Client, ClientEvent, ClientMsg, ConnId, HeroPick, Host, HostEvent,
    Invite, LobbyPlayer, ServerMsg, DEFAULT_PORT, PROTOCOL,
};
use together_sim::{Event, PlayerId, PlayerInput, Snapshot, World, DT};
use web_time::Instant;

use super::content;
use super::menu::Selection;

/// The most players in a run, the host included.
pub(super) const MAX_PLAYERS: usize = 4;

/// Steps per snapshot sent: 15 a second, which the client's playback smooths.
pub(super) const SEND_EVERY: u32 = 2;

/// A connection still writing more snapshots than this is skipped until it catches up.
const MAX_BACKLOG: usize = 2;

/// Seconds between pings.
const PING_EVERY: f32 = 1.0;

/// The co-op session, if any: what the menu's lobby shows and what a run talks over.
#[derive(Resource, Default)]
pub(super) struct Net {
    pub role: Role,
    /// A line for the lobby: what is happening, or what went wrong.
    pub status: String,
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
    /// Players who said hello, by connection.
    pub peers: Vec<Peer>,
    /// The lobby last sent, so it is sent again only when it changes.
    sent: Option<(String, Vec<LobbyPlayer>)>,
    /// Steps since the run began, which pace the snapshots.
    steps: u32,
    /// The invite code has been written to the log (for test runs to read).
    logged_code: bool,
}

impl HostSide {
    /// Where the invite code stands.
    pub fn invite(&self) -> Invite {
        self.host.lock().unwrap().invite()
    }
}

pub(super) struct Peer {
    conn: ConnId,
    pub player: PlayerId,
    pub hero: HeroPick,
    /// Events not yet sent to this player: since its last snapshot.
    pending: Vec<Event>,
}

pub(super) struct ClientSide {
    client: Mutex<Client>,
    pub addr: String,
    pub connected: bool,
    /// Our player, once the host welcomed us.
    pub me: Option<PlayerId>,
    pub zone: Option<String>,
    pub lobby: Vec<LobbyPlayer>,
    /// The host started a run in this zone; [`super::start_run`] begins ours.
    pub start: Option<String>,
    /// The pick we last told the host.
    sent_pick: Option<HeroPick>,
    /// When the session began, the clock for ping stamps.
    epoch: Instant,
    /// When our hero's pose last went to the host.
    last_pose: Option<Instant>,
    /// The ping in flight (its stamp), and when the next goes.
    ping_out: Option<u32>,
    next_ping: f32,
    /// The last round trip to the host, in milliseconds.
    pub ping_ms: Option<u32>,
}

/// Time the round trip now and then: a ping when none is in flight and its time has come.
fn send_ping(client: &Client, epoch: Instant, ping_out: &mut Option<u32>, next_ping: &mut f32) {
    let now = epoch.elapsed().as_secs_f32();
    if ping_out.is_none() && now >= *next_ping {
        let stamp = (now * 1000.0) as u32;
        client.send(&ClientMsg::Ping(stamp));
        *ping_out = Some(stamp);
        *next_ping = now + PING_EVERY;
    }
}

impl ClientSide {
    /// Seconds since the session began.
    fn now(&self) -> f32 {
        self.epoch.elapsed().as_secs_f32()
    }

    /// A pong for the ping in flight times the round trip.
    fn pong(&mut self, stamp: u32) {
        if self.ping_out == Some(stamp) {
            self.ping_out = None;
            self.ping_ms = Some(((self.now() * 1000.0) as u32).saturating_sub(stamp));
        }
    }
}

impl Net {
    pub fn is_solo(&self) -> bool {
        matches!(self.role, Role::Solo)
    }

    pub fn is_client(&self) -> bool {
        matches!(self.role, Role::Client(_))
    }

    /// Joined: the last round trip to the host, in milliseconds.
    pub fn ping_ms(&self) -> Option<u32> {
        match &self.role {
            Role::Client(c) => c.ping_ms,
            _ => None,
        }
    }

    /// Start hosting: on [`DEFAULT_PORT`] (any free port if it is taken) for the local network, and
    /// by invite code ([`host_key`]) for everyone else.
    pub fn host(&mut self) {
        match Host::listen(DEFAULT_PORT).or_else(|_| Host::listen(0)) {
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
            client: Mutex::new(Client::connect(addr)),
            addr: addr.to_owned(),
            connected: false,
            me: None,
            zone: None,
            lobby: Vec::new(),
            start: None,
            sent_pick: None,
            epoch: Instant::now(),
            last_pose: None,
            ping_out: None,
            next_ping: 0.0,
            ping_ms: None,
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

    /// The players a run starts with, the host first.
    pub fn roster(&self, own: &HeroPick) -> Vec<LobbyPlayer> {
        let mut out = vec![LobbyPlayer {
            player: 0,
            hero: own.clone(),
        }];
        if let Role::Host(h) = &self.role {
            out.extend(h.peers.iter().map(|p| LobbyPlayer {
                player: p.player,
                hero: p.hero.clone(),
            }));
        }
        out
    }

    /// The host: tell every client the run begins.
    pub fn broadcast_start(&mut self, zone: &str) {
        if let Role::Host(h) = &mut self.role {
            h.steps = 0;
            for p in &mut h.peers {
                p.pending.clear();
            }
            h.host.lock().unwrap().broadcast(&ServerMsg::Start {
                zone: zone.to_owned(),
            });
        }
    }

    /// The host: tell every client the run is over, and the lobby is back.
    pub fn broadcast_ended(&mut self) {
        if let Role::Host(h) = &mut self.role {
            h.host.lock().unwrap().broadcast(&ServerMsg::Ended);
            h.sent = None;
        }
    }
}

/// This player's host key, `benilla-config/together-host-key.txt`: made once, so the invite code
/// a friend saved keeps working. A key that cannot be saved still hosts, under a new code.
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

/// The name the player's hero goes by: `TOGETHER_NAME`, else the Windows user name.
pub(super) fn hero_name() -> String {
    std::env::var("TOGETHER_NAME")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .map(|n| n.chars().filter(|c| !c.is_control()).take(12).collect())
        .filter(|n: &String| !n.is_empty())
        .unwrap_or_else(|| "Hero".into())
}

/// The local pick as the wire carries it, with the talents taken for its class.
pub(super) fn own_pick(
    selection: &Selection,
    talents: &super::talents::Talents,
) -> Option<HeroPick> {
    let (_, race, sex) = selection.hero_look()?;
    let class = content().classes[selection.class].key.clone();
    Some(HeroPick {
        name: hero_name(),
        talents: talents.pick(&class),
        class,
        race,
        sex,
    })
}

/// The content files' hash, which a client and its host must share.
pub(super) fn our_content_hash() -> u64 {
    let c = super::content_texts();
    content_hash(&[&c.0, &c.1, &c.2, &c.3])
}

/// This machine's address on the local network: the one a connection out would leave from (no
/// packet is sent to find it).
fn lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

/// At the menu: the host takes players in and keeps their lobby current; a client follows its
/// host's lobby and tells it its pick.
pub(super) fn lobby(
    mut net: ResMut<Net>,
    selection: Res<Selection>,
    talents: Res<super::talents::Talents>,
) {
    let own = own_pick(&selection, &talents);
    let content = content();
    let zone_key = content.zones[selection.zone].key.clone();
    let net = &mut *net;
    match &mut net.role {
        Role::Solo => {}
        Role::Host(h) => {
            let host = h.host.lock().unwrap();
            if !h.logged_code {
                if let Invite::Ready(code) = host.invite() {
                    info!("together: invite code {code}");
                    h.logged_code = true;
                }
            }
            for event in host.poll() {
                match event {
                    HostEvent::Joined(..) => {}
                    HostEvent::Message(
                        conn,
                        ClientMsg::Hello {
                            protocol,
                            content,
                            hero,
                        },
                    ) => {
                        let refuse = if protocol != PROTOCOL {
                            Some("a different version of the game".to_string())
                        } else if content != our_content_hash() {
                            Some("different game data (content files)".into())
                        } else if h.peers.len() + 1 >= MAX_PLAYERS {
                            Some(format!("the party is full ({MAX_PLAYERS} players)"))
                        } else {
                            None
                        };
                        if let Some(why) = refuse {
                            host.send(conn, &ServerMsg::Refused(format!("The host has {why}.")));
                            host.kick(conn);
                            continue;
                        }
                        let player = (1..MAX_PLAYERS as PlayerId)
                            .find(|p| !h.peers.iter().any(|x| x.player == *p))
                            .expect("checked for room");
                        host.send(conn, &ServerMsg::Welcome { you: player });
                        net.status = format!("{} joined", hero.name);
                        h.peers.push(Peer {
                            conn,
                            player,
                            hero,
                            pending: Vec::new(),
                        });
                    }
                    HostEvent::Message(conn, ClientMsg::Pick(hero)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            p.hero = hero;
                        }
                    }
                    HostEvent::Message(_, ClientMsg::Input(_)) => {}
                    HostEvent::Message(conn, ClientMsg::Ping(stamp)) => {
                        host.send(conn, &ServerMsg::Pong(stamp));
                    }
                    HostEvent::Left(conn) => {
                        if let Some(i) = h.peers.iter().position(|p| p.conn == conn) {
                            net.status = format!("{} left", h.peers[i].hero.name);
                            h.peers.remove(i);
                        }
                    }
                }
            }
            let Some(own) = own else { return };
            let mut players = vec![LobbyPlayer {
                player: 0,
                hero: own,
            }];
            players.extend(h.peers.iter().map(|p| LobbyPlayer {
                player: p.player,
                hero: p.hero.clone(),
            }));
            let now = (zone_key, players);
            if h.sent.as_ref() != Some(&now) {
                host.broadcast(&ServerMsg::Lobby {
                    zone: now.0.clone(),
                    players: now.1.clone(),
                });
                drop(host);
                h.sent = Some(now);
            }
        }
        Role::Client(c) => {
            let events = c.client.lock().unwrap().poll();
            if c.me.is_some() {
                let client = c.client.lock().unwrap();
                send_ping(&client, c.epoch, &mut c.ping_out, &mut c.next_ping);
            }
            let mut gone = None;
            for event in events {
                match event {
                    ClientEvent::Connected => {
                        c.connected = true;
                        net.status = "Connected, joining…".into();
                        if let Some(hero) = own.clone() {
                            c.client.lock().unwrap().send(&ClientMsg::Hello {
                                protocol: PROTOCOL,
                                content: our_content_hash(),
                                hero: hero.clone(),
                            });
                            c.sent_pick = Some(hero);
                        }
                    }
                    ClientEvent::Message(ServerMsg::Welcome { you }) => {
                        c.me = Some(you);
                        net.status = "Joined: waiting for the host to start".into();
                    }
                    ClientEvent::Message(ServerMsg::Refused(why)) => gone = Some(why),
                    ClientEvent::Message(ServerMsg::Lobby { zone, players }) => {
                        c.zone = Some(zone);
                        c.lobby = players;
                    }
                    ClientEvent::Message(ServerMsg::Start { zone }) => {
                        c.start = Some(zone);
                        // A ping across the loading screen would time the load, not the line.
                        c.ping_out = None;
                    }
                    ClientEvent::Message(ServerMsg::Pong(stamp)) => c.pong(stamp),
                    ClientEvent::Message(ServerMsg::State { .. } | ServerMsg::Ended) => {}
                    ClientEvent::Disconnected(why) => {
                        gone = Some(format!("Disconnected: {why}"));
                    }
                }
            }
            if let Some(why) = gone {
                net.leave(why);
                return;
            }
            // A new pick at the menu goes to the host.
            if c.me.is_some() && own.is_some() && c.sent_pick != own {
                if let Some(hero) = own {
                    c.client
                        .lock()
                        .unwrap()
                        .send(&ClientMsg::Pick(hero.clone()));
                    c.sent_pick = Some(hero);
                }
            }
        }
    }
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
            HostEvent::Message(conn, ClientMsg::Input(input)) => {
                if let Some(p) = h.peers.iter().find(|p| p.conn == conn) {
                    world.queue_input(p.player, input);
                }
            }
            HostEvent::Message(conn, ClientMsg::Hello { .. }) => {
                host.send(
                    conn,
                    &ServerMsg::Refused("A run is under way; join when it ends.".into()),
                );
                host.kick(conn);
            }
            HostEvent::Message(conn, ClientMsg::Ping(stamp)) => {
                host.send(conn, &ServerMsg::Pong(stamp));
            }
            HostEvent::Message(_, ClientMsg::Pick(_)) | HostEvent::Joined(..) => {}
            HostEvent::Left(conn) => {
                if let Some(i) = h.peers.iter().position(|p| p.conn == conn) {
                    world.remove_hero(h.peers[i].player);
                    net.status = format!("{} left", h.peers[i].hero.name);
                    h.peers.remove(i);
                }
            }
        }
    }
}

/// The host, after a step: the step's events kept for every client, and every [`SEND_EVERY`]
/// steps the world and those events to each client whose connection keeps up.
pub(super) fn host_send(net: &mut Net, world: &World, events_from: usize) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    if h.peers.is_empty() {
        return;
    }
    let events = &world.events()[events_from..];
    for p in &mut h.peers {
        p.pending.extend_from_slice(events);
    }
    h.steps += 1;
    if h.steps % SEND_EVERY != 0 {
        return;
    }
    let snapshot = world.snapshot();
    let host = h.host.lock().unwrap();
    for p in &mut h.peers {
        if host.backlog(p.conn) > MAX_BACKLOG {
            continue;
        }
        host.send(
            p.conn,
            &ServerMsg::State {
                snapshot: snapshot.clone(),
                events: std::mem::take(&mut p.pending),
            },
        );
    }
}

/// What a client heard from its host during a run.
pub(super) enum Heard {
    /// A snapshot, for the playback to apply in its turn.
    State(Snapshot, Vec<Event>),
    /// The host ended the run.
    Ended,
    /// The connection is gone, and why.
    Lost(String),
}

/// A client, during a run: this frame's input to the host, and what the host sent. The pose goes
/// at most once a step's time; a card choice goes at once.
pub(super) fn client_exchange(net: &mut Net, input: PlayerInput) -> Vec<Heard> {
    let Role::Client(c) = &mut net.role else {
        return Vec::new();
    };
    let client = c.client.lock().unwrap();
    let choice = input.pick.is_some() || input.reroll || input.skip || input.dash;
    let pose_due =
        input.pose.is_some() && c.last_pose.is_none_or(|t| t.elapsed().as_secs_f32() >= DT);
    if choice || pose_due {
        client.send(&ClientMsg::Input(input));
        if input.pose.is_some() {
            c.last_pose = Some(Instant::now());
        }
    }
    let mut heard = Vec::new();
    let mut pongs = Vec::new();
    for event in client.poll() {
        match event {
            ClientEvent::Message(ServerMsg::State { snapshot, events }) => {
                heard.push(Heard::State(snapshot, events));
            }
            ClientEvent::Message(ServerMsg::Ended) => heard.push(Heard::Ended),
            ClientEvent::Message(ServerMsg::Pong(stamp)) => pongs.push(stamp),
            ClientEvent::Disconnected(why) => heard.push(Heard::Lost(why)),
            ClientEvent::Message(_) | ClientEvent::Connected => {}
        }
    }
    send_ping(&client, c.epoch, &mut c.ping_out, &mut c.next_ping);
    drop(client);
    for stamp in pongs {
        c.pong(stamp);
    }
    heard
}

/// The phase the client should be in: back to the menu with the session kept (the host ended the
/// run) or dropped (the connection is gone).
pub(super) fn client_left_run(net: &mut Net, heard: &[Heard]) -> bool {
    for h in heard {
        match h {
            Heard::Ended => {
                net.status = "The host ended the run".into();
                if let Role::Client(c) = &mut net.role {
                    c.start = None;
                }
                return true;
            }
            Heard::Lost(why) => {
                net.leave(format!("Disconnected: {why}"));
                return true;
            }
            Heard::State(..) => {}
        }
    }
    false
}

/// Leaving the menu for good: no lingering session.
pub(super) fn close_on_exit(mut exits: MessageReader<AppExit>, mut net: ResMut<Net>) {
    if exits.read().next().is_some() {
        net.leave("");
    }
}
