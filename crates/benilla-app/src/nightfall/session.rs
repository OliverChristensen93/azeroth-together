//! Co-op sessions for Nightfall: hosting or joining, the lobby before a run, and the run's traffic.
//! The pattern is Survivors' (`together/session.rs`), copied: the host steps the world and every
//! [`SEND_EVERY`] steps sends each client a snapshot; a client mirrors it and sends where its own
//! controller put its hero. A slow line never builds a queue: a client whose connection still
//! holds unsent snapshots is skipped until it catches up.
//!
//! On the wire Nightfall is the game `"nightfall"` ([`GAME`]): it shares Survivors' port and invite
//! code, and a friend who opened the other game is told so.

use std::sync::Mutex;

use bevy::prelude::*;
use nightfall_sim::messages::{ClientMsg, HeroPick, LobbyPlayer, ServerMsg, PROTOCOL};
use nightfall_sim::{PlayerId, PlayerInput, Snapshot, World, DT};
use together_net::{content_hash, new_host_key, ConnId, GameTag, Invite, DEFAULT_PORT};
use web_time::Instant;

use super::content;
use super::menu::Selection;

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

#[derive(Resource, Default)]
pub(super) struct Net {
    pub role: Role,
    /// A line for the menu: what is happening, or what went wrong.
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
    pub peers: Vec<Peer>,
    /// The lobby last sent, so it goes again only when it changes.
    sent: Option<Vec<LobbyPlayer>>,
    /// Steps since the run began, which pace the snapshots.
    steps: u32,
    /// The invite code has been written to the log (for test runs to read).
    logged_code: bool,
}

pub(super) struct Peer {
    conn: ConnId,
    pub player: PlayerId,
    pub hero: HeroPick,
}

pub(super) struct ClientSide {
    client: Mutex<Client>,
    pub addr: String,
    /// Our player, once the host welcomed us.
    pub me: Option<PlayerId>,
    pub lobby: Vec<LobbyPlayer>,
    /// The host started a run; [`super::run::start_run`] begins ours.
    pub start: bool,
    /// The pick we last told the host.
    sent_pick: Option<HeroPick>,
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
            format!("{}  ·  {}{you}", p.hero.name, p.hero.class)
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
            start: false,
            sent_pick: None,
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
                let names: Vec<String> = h.peers.iter().map(|p| p.hero.name.clone()).collect();
                format!(
                    "Hosting  ·  {code}  ·  same network: {}:{}\nJoined: {}\n{}",
                    h.lan,
                    h.port,
                    if names.is_empty() { "nobody yet".into() } else { names.join(", ") },
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
    pub fn broadcast_start(&mut self) {
        if let Role::Host(h) = &mut self.role {
            h.steps = 0;
            h.host.lock().unwrap().broadcast(&ServerMsg::Start);
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
fn hero_name() -> String {
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

/// The local pick as the wire carries it.
pub(super) fn own_pick(selection: &Selection) -> HeroPick {
    let content = content();
    let class = &content.classes[selection.class];
    HeroPick {
        name: hero_name(),
        class: class.key.clone(),
        race: class.race,
        sex: class.sex,
    }
}

/// The content files' hash, which a client and its host must share.
fn our_content_hash() -> u64 {
    let c = super::content_texts();
    content_hash(&[&c.0, &c.1, &c.2])
}

/// At the menu: the host takes players in and keeps their lobby current; a client follows its
/// host's lobby and tells it its pick.
pub(super) fn lobby(mut net: ResMut<Net>, selection: Res<Selection>) {
    let own = own_pick(&selection);
    let net = &mut *net;
    match &mut net.role {
        Role::Solo => {}
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
                        h.peers.push(Peer { conn, player, hero });
                    }
                    HostEvent::Message(conn, ClientMsg::Pick(mut hero)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            hero.name = clean_name(&hero.name);
                            p.hero = hero;
                        }
                    }
                    HostEvent::Left(conn) => {
                        if let Some(i) = h.peers.iter().position(|p| p.conn == conn) {
                            net.status = format!("{} left", h.peers[i].hero.name);
                            h.peers.remove(i);
                        }
                    }
                    HostEvent::Message(_, ClientMsg::Input(_)) | HostEvent::Joined(..) => {}
                }
            }
            let mut players = vec![LobbyPlayer { player: 0, hero: own }];
            players.extend(h.peers.iter().map(|p| LobbyPlayer {
                player: p.player,
                hero: p.hero.clone(),
            }));
            if h.sent.as_ref() != Some(&players) {
                host.broadcast(&ServerMsg::Lobby {
                    players: players.clone(),
                });
                drop(host);
                h.sent = Some(players);
            }
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
                            hero: own.clone(),
                        });
                        c.sent_pick = Some(own.clone());
                    }
                    ClientEvent::Message(ServerMsg::Welcome { you }) => {
                        c.me = Some(you);
                        net.status = "Joined: waiting for the host to start".into();
                    }
                    ClientEvent::Message(ServerMsg::Refused(why)) => gone = Some(why),
                    ClientEvent::Message(ServerMsg::Lobby { players }) => c.lobby = players,
                    ClientEvent::Message(ServerMsg::Start) => c.start = true,
                    ClientEvent::Message(ServerMsg::State { .. } | ServerMsg::Ended) => {}
                    ClientEvent::Disconnected(why) => gone = Some(format!("Disconnected: {why}")),
                }
            }
            if let Some(why) = gone {
                net.leave(why);
                return;
            }
            if c.me.is_some() && c.sent_pick.as_ref() != Some(&own) {
                c.client.lock().unwrap().send(&ClientMsg::Pick(own.clone()));
                c.sent_pick = Some(own);
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
                    world.set_input(p.player, input);
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
            HostEvent::Message(_, ClientMsg::Pick(_)) | HostEvent::Joined(..) => {}
        }
    }
}

/// The host, after a step: every [`SEND_EVERY`] steps, the world to each client whose connection
/// keeps up.
pub(super) fn host_send(net: &mut Net, world: &World) {
    let Role::Host(h) = &mut net.role else {
        return;
    };
    if h.peers.is_empty() {
        return;
    }
    h.steps += 1;
    if h.steps % SEND_EVERY != 0 {
        return;
    }
    let snapshot = world.snapshot();
    let host = h.host.lock().unwrap();
    for p in &h.peers {
        if host.backlog(p.conn) > MAX_BACKLOG {
            continue;
        }
        host.send(
            p.conn,
            &ServerMsg::State {
                snapshot: snapshot.clone(),
            },
        );
    }
}

/// What a client heard from its host during a run.
pub(super) enum Heard {
    State(Snapshot),
    Ended,
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
        client.send(&ClientMsg::Input(input));
        c.last_pose = Some(Instant::now());
    }
    let mut heard = Vec::new();
    for event in client.poll() {
        match event {
            ClientEvent::Message(ServerMsg::State { snapshot }) => heard.push(Heard::State(snapshot)),
            ClientEvent::Message(ServerMsg::Ended) => heard.push(Heard::Ended),
            ClientEvent::Disconnected(why) => heard.push(Heard::Lost(why)),
            ClientEvent::Message(_) | ClientEvent::Connected => {}
        }
    }
    heard
}

/// Whether the client's run is over: the host ended it (the session stays) or the connection is
/// gone (the session ends).
pub(super) fn client_left_run(net: &mut Net, heard: &[Heard]) -> bool {
    for h in heard {
        match h {
            Heard::Ended => {
                net.status = "The host ended the run".into();
                if let Role::Client(c) = &mut net.role {
                    c.start = false;
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
        }
    }

    #[test]
    fn a_hello_is_refused_for_version_content_or_a_full_party() {
        assert_eq!(refusal(PROTOCOL, 7, 7, 0), None);
        assert!(refusal(PROTOCOL + 1, 7, 7, 0).unwrap().contains("version"));
        assert!(refusal(PROTOCOL, 8, 7, 0).unwrap().contains("content"));
        assert!(refusal(PROTOCOL, 7, 7, MAX_PLAYERS - 1).unwrap().contains("full"));
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
            LobbyPlayer { player: 0, hero: pick("Host") },
            LobbyPlayer { player: 1, hero: pick("Me") },
        ];
        let text = roster_lines(&players, Some(1));
        assert!(text.contains("Me  ·  rogue  (you)"));
        assert!(!text.contains("Host  ·  rogue  (you)"));
    }
}
