//! Co-op sessions: hosting or joining, the lobby before a run, and the run's traffic.
//!
//! The host runs the game for everyone ([`together_sim::World`]); after each step it sends every
//! client the world and the step's events ([`ServerMsg::State`]). A client never steps the world:
//! it mirrors the host's ([`World::apply_snapshot`]) and draws it with the same code the host
//! draws with, and it sends where its own controller put its hero and its card choices
//! ([`ClientMsg::Input`]), as a WoW client tells its server its moves.

use std::sync::Mutex;

use bevy::prelude::*;
use together_net::{
    content_hash, Client, ClientEvent, ClientMsg, ConnId, HeroPick, Host, HostEvent, LobbyPlayer,
    ServerMsg, DEFAULT_PORT, PROTOCOL,
};
use together_sim::{PlayerId, PlayerInput, World};

use super::content;
use super::menu::Selection;

/// The most players in a run, the host included.
pub(super) const MAX_PLAYERS: usize = 4;

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
}

pub(super) struct Peer {
    conn: ConnId,
    pub player: PlayerId,
    pub hero: HeroPick,
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
}

impl Net {
    pub fn is_solo(&self) -> bool {
        matches!(self.role, Role::Solo)
    }

    pub fn is_client(&self) -> bool {
        matches!(self.role, Role::Client(_))
    }

    /// Start hosting on [`DEFAULT_PORT`].
    pub fn host(&mut self) {
        match Host::listen(DEFAULT_PORT) {
            Ok(host) => {
                let port = host.local_addr().port();
                self.role = Role::Host(HostSide {
                    host: Mutex::new(host),
                    lan: lan_address().unwrap_or_else(|| "this computer's address".into()),
                    port,
                    peers: Vec::new(),
                    sent: None,
                });
                self.status = "Waiting for friends to join".into();
            }
            Err(e) => {
                self.role = Role::Solo;
                self.status = format!("Could not host on port {DEFAULT_PORT}: {e}");
            }
        }
    }

    /// Start joining the host at `addr`.
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
        });
        self.status = format!("Connecting to {addr}…");
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
    pub fn broadcast_start(&self, zone: &str) {
        if let Role::Host(h) = &self.role {
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

/// The name the player's hero goes by: `TOGETHER_NAME`, else the Windows user name.
pub(super) fn hero_name() -> String {
    std::env::var("TOGETHER_NAME")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .map(|n| n.chars().filter(|c| !c.is_control()).take(12).collect())
        .filter(|n: &String| !n.is_empty())
        .unwrap_or_else(|| "Hero".into())
}

/// The local pick as the wire carries it.
pub(super) fn own_pick(selection: &Selection) -> Option<HeroPick> {
    let (_, race, sex) = selection.hero_look()?;
    Some(HeroPick {
        name: hero_name(),
        class: content().classes[selection.class].key.clone(),
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
pub(super) fn lobby(mut net: ResMut<Net>, selection: Res<Selection>) {
    let own = own_pick(&selection);
    let content = content();
    let zone_key = content.zones[selection.zone].key.clone();
    let net = &mut *net;
    match &mut net.role {
        Role::Solo => {}
        Role::Host(h) => {
            let host = h.host.lock().unwrap();
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
                        h.peers.push(Peer { conn, player, hero });
                    }
                    HostEvent::Message(conn, ClientMsg::Pick(hero)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
                            p.hero = hero;
                        }
                    }
                    HostEvent::Message(_, ClientMsg::Input(_)) => {}
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
                    ClientEvent::Message(ServerMsg::Start { zone }) => c.start = Some(zone),
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

/// The host, after a step: the world and that step's events to every client.
pub(super) fn host_send(net: &Net, world: &World, events_from: usize) {
    let Role::Host(h) = &net.role else {
        return;
    };
    if h.peers.is_empty() {
        return;
    }
    let events = world.events()[events_from..].to_vec();
    h.host.lock().unwrap().broadcast(&ServerMsg::State {
        snapshot: world.snapshot(),
        events,
    });
}

/// What a client heard from its host during a run.
pub(super) enum Heard {
    /// A step, applied to the mirror.
    Stepped,
    /// The host ended the run.
    Ended,
    /// The connection is gone, and why.
    Lost(String),
}

/// A client, during a run: the host's steps into the mirror, and this frame's input to the host.
pub(super) fn client_exchange(net: &mut Net, world: &mut World, input: PlayerInput) -> Vec<Heard> {
    let Role::Client(c) = &mut net.role else {
        return Vec::new();
    };
    let client = c.client.lock().unwrap();
    if input.pose.is_some() || input.pick.is_some() || input.reroll || input.skip {
        client.send(&ClientMsg::Input(input));
    }
    let mut heard = Vec::new();
    for event in client.poll() {
        match event {
            ClientEvent::Message(ServerMsg::State { snapshot, events }) => {
                world.apply_snapshot(snapshot, events);
                heard.push(Heard::Stepped);
            }
            ClientEvent::Message(ServerMsg::Ended) => heard.push(Heard::Ended),
            ClientEvent::Disconnected(why) => heard.push(Heard::Lost(why)),
            ClientEvent::Message(_) | ClientEvent::Connected => {}
        }
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
            Heard::Stepped => {}
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
