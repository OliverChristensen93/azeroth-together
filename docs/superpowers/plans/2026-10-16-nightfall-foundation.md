# Nightfall Sub-project 1 (Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** From a new pick-game screen, Nightfall's own menu starts a run (solo, hosted, or joined by invite code) in which Rogue and Mage heroes walk dark, foggy Duskwood at midnight with the real WoW controller and UI.

**Architecture:** A new headless crate `nightfall-sim` holds the game rules (heroes, poses, the map's bounds, content, wire messages). The client module `benilla-app/src/nightfall/` steps or mirrors it and draws it, copying the patterns of `together/` rather than sharing its code. `together-net` becomes generic over each game's messages and refuses a client of the other game during a handshake. A `GameMode` state picks Survivors or Nightfall on a first screen; every survivors system is gated on it. One small engine change: a public `FogOverride` resource in `benilla-world`.

**Tech Stack:** Rust (x86_64-pc-windows-gnu), Bevy 0.18, serde + ron (content), postcard (wire), iroh (invite codes), WGSL (the night grade).

**Spec:** `docs/superpowers/specs/2026-10-16-nightfall-design.md` (§3 in full; §10 for the atmosphere; §11 for the settings).

## Global Constraints

- Work in the worktree `D:\Games\azeroth-together-stealth` on branch `stealth-extraction`. `D:` is exFAT, so git needs `git config --global --add safe.directory D:/Games/azeroth-together-stealth` once, or `git -c safe.directory=D:/Games/azeroth-together-stealth …` on every call.
- Builds need MinGW on `PATH` first: `export PATH="/c/Users/Olivs/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`. The worktree has its own `target/`: the first build is cold (~17 min release).
- `nightfall-sim` depends on neither `together-sim` nor Bevy (spec §3.1).
- The client copies what it needs from `together/` instead of sharing it (spec §3.1).
- `together-net` no longer depends on `together-sim` (spec §3.3).
- Engine crates change only where this plan says: `benilla-world/src/lighting/mod.rs` (the fog override) and `benilla-app/src/shaders.rs` (one embedded shader). Keep those diffs small.
- Never launch a normal (fullscreen) run without asking the user. Test windows use `WOW_BG=1 WOW_WIN=1280x720`.
- Nothing Blizzard-derived goes into `content/` or a commit; content names WoW ids only.
- Survivors behaves as before, except: the pick screen comes first (skipped when any `TOGETHER_*` variable is set), and its menu's Exit button becomes Back.
- Comments follow the surrounding code: full sentences that say why, doc comments on every item.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A Survivors client joins a Nightfall host (or the reverse) with the shared invite code.** Expect a clear refusal naming the host's game, and the host's lobby untouched. Pinned in Task 1.
2. **A connection that never sends a greeting** (an old client, a port scanner) must not show up as a joined player or stop the next friend from joining. Pinned in Task 1.
3. **A client sends a pose with `NaN` or far outside the map.** The hero stays where it was (NaN) or is clamped to the edge, and the world is never poisoned. Pinned in Task 4.
4. **Pick → Survivors → Back → Nightfall → Back → Survivors.** Survivors' menu, preview hero and daylight must not show or fight Nightfall's night, and Nightfall's menu must not show over Survivors. Pinned in Task 6 (`menu_visible`) and Task 8 (`roots_visible`).
5. **Survivors after Nightfall must not stay foggy or short-sighted.** Leaving Nightfall restores the view distance and drops the fog override. Pinned in Task 10.

**Not in this plan** (spec §3.4, later sub-projects): the `Concealed` marker for Tab-targeting and nameplates, the stealthed-nameplate fix and the public line-of-sight query belong to sub-projects 2 and 3. Ground mist, the heartbeat and the vignette that follows visibility come with the senses (sub-project 3).

## File Structure

| File | Responsibility |
|---|---|
| `crates/together-net/src/handshake.rs` (new) | The greeting and verdict both ends exchange first; the refusal text |
| `crates/together-net/src/{lib,host,client,wire}.rs` | Generic `Host<Up, Down>`, `Client<Up, Down>`; game tag |
| `crates/together-sim/src/messages.rs` (new) | Survivors' wire messages, moved out of `together-net` |
| `crates/together-sim/tests/snapshot_size.rs` (moved) | Survivors' snapshot-size check, now beside its messages |
| `crates/nightfall-sim/src/math.rs` | `V2` |
| `crates/nightfall-sim/src/bounds.rs` | The run's edge: contains, clamp |
| `crates/nightfall-sim/src/content.rs` | Classes, the map, the rules; parsing and validation |
| `crates/nightfall-sim/src/world.rs` | Heroes, inputs, step, snapshot |
| `crates/nightfall-sim/src/messages.rs` | Nightfall's wire messages |
| `crates/nightfall-sim/tests/shipped_content.rs` | `content/nightfall/` parses and plays |
| `content/nightfall/{classes,map,rules}.ron` | Rogue and Mage, Duskwood, the atmosphere |
| `crates/benilla-app/src/game_pick.rs` (new) | `GameMode`, the pick screen |
| `crates/benilla-app/src/together/{mod,menu,session,lobby}.rs` | Gating on `GameMode::Survivors`; Back; the new message paths |
| `crates/benilla-app/src/nightfall/mod.rs` | `prepare`, content, `Phase`, `NightfallSet`, the plugin |
| `crates/benilla-app/src/nightfall/menu.rs` | The menu and the run's Leave button |
| `crates/benilla-app/src/nightfall/session.rs` | Hosting, joining, the lobby, run traffic |
| `crates/benilla-app/src/nightfall/run.rs` | `Sim`, start, pose, stepping, leaving, test hooks |
| `crates/benilla-app/src/nightfall/view.rs` | Hero bodies, vitals, placement |
| `crates/benilla-app/src/nightfall/{units,dress}.rs` | Copied from `together/` |
| `crates/benilla-app/src/nightfall/atmosphere.rs` | Night, fog, view distance, hero lights, the menu camera |
| `crates/benilla-app/src/nightfall/grade.rs`, `shaders/nightfall_grade.wgsl` | The night colour pass |
| `crates/benilla-world/src/lighting/mod.rs` | `FogOverride` and its system |

---

### Task 1: Generic netcode with a game handshake

**Files:**
- Create: `crates/together-net/src/handshake.rs`
- Create: `crates/together-sim/src/messages.rs`
- Move: `crates/together-net/tests/snapshot_size.rs` → `crates/together-sim/tests/snapshot_size.rs`
- Modify: `crates/together-net/src/lib.rs`, `src/host.rs`, `src/client.rs`, `src/wire.rs`, `Cargo.toml`
- Modify: `crates/together-sim/src/lib.rs`, `crates/together-sim/Cargo.toml`
- Modify: `crates/benilla-app/src/together/session.rs:18-22,155,180`, `crates/benilla-app/src/together/mod.rs:181`

**Interfaces:**
- Produces:
  - `together_net::GameTag` (`&'static str`).
  - `Host<Up, Down>::listen(port: u16, game: GameTag) -> io::Result<Self>`, with `open_invites`, `invite`, `local_addr`, `poll() -> Vec<HostEvent<Up>>`, `send(ConnId, &Down)`, `broadcast(&Down)`, `kick`, `backlog`, `connections`.
  - `HostEvent<Up> { Joined(ConnId, String), Message(ConnId, Up), Left(ConnId) }`.
  - `Client<Up, Down>::connect(to: &str, game: GameTag) -> Self`, with `poll() -> Vec<ClientEvent<Down>>` and `send(&Up)`.
  - `ClientEvent<Down> { Connected, Message(Down), Disconnected(String) }`.
  - `together_sim::messages::{ClientMsg, ServerMsg, HeroPick, LobbyPlayer, PROTOCOL}`.
  - `content_hash`, `with_default_port`, `DEFAULT_PORT`, `Invite`, `code_for`, `new_host_key` and `parse_code` are unchanged.

- [ ] **Step 1: Write the handshake with its tests**

Create `crates/together-net/src/handshake.rs`:

```rust
//! The first frames on every connection, before any game message: the client says which game it
//! plays, and the host answers whether it plays the same one. Survivors and Nightfall share the
//! port and the invite code, so a friend who opens the wrong game is told so, not left hanging on
//! a lobby that never comes.

use serde::{Deserialize, Serialize};

/// Bumped whenever the greeting or the verdict changes shape.
pub(crate) const HANDSHAKE: u32 = 1;

/// The client's first frame.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Greeting {
    pub handshake: u32,
    /// The client's game, a [`crate::GameTag`].
    pub game: String,
}

/// The host's answer; anything but [`Verdict::Welcome`] is followed by the host closing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Verdict {
    Welcome,
    /// The host plays `host`.
    OtherGame { host: String },
    /// The client's handshake is another version.
    OtherHandshake,
}

/// The host's verdict on a greeting, for a host playing `game`.
pub(crate) fn judge(greeting: &Greeting, game: &str) -> Verdict {
    if greeting.handshake != HANDSHAKE {
        Verdict::OtherHandshake
    } else if greeting.game != game {
        Verdict::OtherGame {
            host: greeting_name(game),
        }
    } else {
        Verdict::Welcome
    }
}

/// What the joining player reads when the host refuses them, for a client playing `ours`.
pub(crate) fn refusal(verdict: &Verdict, ours: &str) -> String {
    match verdict {
        Verdict::Welcome => String::new(),
        Verdict::OtherGame { host } => format!(
            "That host is playing {host}, not {}.",
            greeting_name(ours)
        ),
        Verdict::OtherHandshake => "That host runs another version of the game.".into(),
    }
}

/// A game tag as players read it: "nightfall" is "Nightfall".
fn greeting_name(tag: &str) -> String {
    let mut chars = tag.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn greeting(game: &str) -> Greeting {
        Greeting {
            handshake: HANDSHAKE,
            game: game.into(),
        }
    }

    #[test]
    fn the_same_game_is_welcome() {
        assert_eq!(judge(&greeting("nightfall"), "nightfall"), Verdict::Welcome);
    }

    #[test]
    fn another_game_is_refused_with_the_hosts_name() {
        let v = judge(&greeting("nightfall"), "survivors");
        assert_eq!(
            v,
            Verdict::OtherGame {
                host: "Survivors".into()
            }
        );
        assert_eq!(
            refusal(&v, "nightfall"),
            "That host is playing Survivors, not Nightfall."
        );
    }

    #[test]
    fn another_handshake_is_refused() {
        let old = Greeting {
            handshake: HANDSHAKE + 1,
            game: "survivors".into(),
        };
        let v = judge(&old, "survivors");
        assert_eq!(v, Verdict::OtherHandshake);
        assert!(refusal(&v, "survivors").contains("another version"));
    }
}
```

- [ ] **Step 2: Make `lib.rs` game-neutral**

Replace `crates/together-net/src/lib.rs` with this. The messages move to `together_sim::messages` in Step 6, and `content_hash` and `with_default_port` keep their bodies and tests word for word:

```rust
//! Azeroth Together's co-op wire, shared by every game the launcher plays.
//!
//! A game's host runs its world and every client mirrors it. Each game brings its own messages:
//! `Up`, what a client sends, and `Down`, what the host sends; this crate carries them and knows
//! nothing of either game. Before any game message, the two ends exchange a handshake
//! ([`handshake`]): a host refuses a client of another game with a reason, since Survivors and
//! Nightfall share the port and the invite code.
//!
//! Messages travel over TCP, or over QUIC for a game joined by invite code ([`invite`]), as a
//! little-endian `u32` length and a `postcard` body. [`Host`] and [`Client`] run the sockets on
//! their own threads; the game polls them once a frame and never blocks.

mod client;
mod handshake;
mod host;
mod invite;
mod wire;

pub use client::{Client, ClientEvent};
pub use host::{ConnId, Host, HostEvent, Invite};
pub use invite::{code_for, new_host_key, parse_code};

/// Which game a host or client plays, the same on both ends: `"survivors"`, `"nightfall"`.
pub type GameTag = &'static str;

/// The port a host listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7777;

// `content_hash`, `with_default_port` and the `tests` module: unchanged from before.
```

Then paste the existing `content_hash`, `with_default_port` and `#[cfg(test)] mod tests` below that comment, word for word, and delete the comment line.

- [ ] **Step 3: Rewrite `host.rs` generically, with the handshake on each connection's own thread**

In `crates/together-net/src/host.rs` make these changes:

```rust
use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::handshake::{self, Greeting, Verdict};
use crate::invite::{self, Link};
use crate::wire;
use crate::GameTag;
```

(Remove `use crate::{ClientMsg, ServerMsg};`.)

```rust
/// What happened on the host's sockets since the last poll.
#[derive(Debug)]
pub enum HostEvent<Up> {
    /// Someone of our game joined (from this address, or by invite code).
    Joined(ConnId, String),
    Message(ConnId, Up),
    /// The connection closed, or failed.
    Left(ConnId),
}

/// What the accept paths share: where events go, the writers, the next connection's number and
/// the game this host plays.
struct Intake<Up> {
    events: Sender<HostEvent<Up>>,
    writers: Writers,
    next: Arc<AtomicU32>,
    game: GameTag,
}

// Written out: a derived `Clone` would ask `Up: Clone`, which a sender does not need.
impl<Up> Clone for Intake<Up> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            writers: self.writers.clone(),
            next: self.next.clone(),
            game: self.game,
        }
    }
}

impl<Up: DeserializeOwned + Send + 'static> Intake<Up> {
    /// Number a new connection and start its thread, which greets it and announces it.
    fn admit(&self, link: Link, from: String) {
        let conn = self.next.fetch_add(1, Ordering::Relaxed);
        let intake = self.clone();
        let _ = thread::Builder::new()
            .name(format!("together-host-read-{conn}"))
            .spawn(move || serve(conn, link, from, intake));
    }
}

/// A listening host for a game whose clients send `Up` and are sent `Down`. Dropping it stops
/// listening and closes every connection.
pub struct Host<Up, Down> {
    events: Receiver<HostEvent<Up>>,
    intake: Intake<Up>,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    invite: Arc<Mutex<Invite>>,
    /// The invite endpoint, once open, to close with the host.
    endpoint: Arc<Mutex<Option<iroh::Endpoint>>>,
    _down: PhantomData<fn(&Down)>,
}

impl<Up: DeserializeOwned + Send + 'static, Down: Serialize> Host<Up, Down> {
    /// Listen on every interface at `port` (0 picks a free one), for clients playing `game`.
    pub fn listen(port: u16, game: GameTag) -> io::Result<Self> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (tx, events) = mpsc::channel();
        let intake = Intake {
            events: tx,
            writers: Arc::default(),
            next: Arc::new(AtomicU32::new(1)),
            game,
        };
        let stop = Arc::new(AtomicBool::new(false));
        {
            let (intake, stop) = (intake.clone(), stop.clone());
            thread::Builder::new()
                .name("together-host-accept".into())
                .spawn(move || accept_loop(listener, intake, stop))?;
        }
        Ok(Host {
            events,
            intake,
            addr,
            stop,
            invite: Arc::new(Mutex::new(Invite::Off)),
            endpoint: Arc::default(),
            _down: PhantomData,
        })
    }

    // `open_invites`, `invite`, `local_addr`, `kick`, `backlog`, `connections`: bodies unchanged.

    /// Everything since the last poll, oldest first.
    pub fn poll(&self) -> Vec<HostEvent<Up>> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, to: ConnId, msg: &Down) {
        // body unchanged
    }

    /// The same message to everyone, encoded once.
    pub fn broadcast(&self, msg: &Down) {
        // body unchanged
    }
}

impl<Up, Down> Drop for Host<Up, Down> {
    // body unchanged
}
```

(`open_invites`, `invite`, `local_addr`, `kick`, `backlog`, `connections`, `send`, `broadcast` and `Drop` keep their bodies as they are now, only moved into the new `impl` blocks. `tcp_link` is unchanged.)

Change `accept_loop` to take `intake: Intake<Up>` with the bound `Up: DeserializeOwned + Send + 'static`. Replace `start_connection` with `serve`:

```rust
/// One connection's life, on its own thread: the handshake, then a writer thread and the messages
/// in. A connection that never greets, greets wrongly or plays another game never joins: no
/// [`HostEvent`] is sent for it and nothing is broadcast to it.
fn serve<Up: DeserializeOwned + Send + 'static>(
    conn: ConnId,
    link: Link,
    from: String,
    intake: Intake<Up>,
) {
    let Link {
        reader,
        writer,
        close,
    } = link;
    let mut input = BufReader::new(reader);
    let mut out = BufWriter::new(writer);
    let verdict = match wire::read::<Greeting>(&mut input) {
        Ok(greeting) => handshake::judge(&greeting, intake.game),
        Err(_) => {
            close();
            return;
        }
    };
    let told = wire::encode(&verdict).and_then(|frame| wire::write_frame(&mut out, &frame));
    if verdict != Verdict::Welcome || told.is_err() {
        close();
        return;
    }

    let (tx, rx) = mpsc::channel::<Arc<Vec<u8>>>();
    let queued = Arc::new(AtomicUsize::new(0));
    intake.writers.lock().unwrap().insert(
        conn,
        Writer {
            tx,
            queued: queued.clone(),
        },
    );
    // Writer: frames out until the host drops its sender (kick, or the host goes).
    let writer = thread::Builder::new()
        .name(format!("together-host-write-{conn}"))
        .spawn(move || {
            for frame in rx {
                let sent = wire::write_frame(&mut out, &frame);
                queued.fetch_sub(1, Ordering::Relaxed);
                if sent.is_err() {
                    break;
                }
            }
            close();
        });
    if writer.is_err() {
        intake.writers.lock().unwrap().remove(&conn);
        return;
    }
    let _ = intake.events.send(HostEvent::Joined(conn, from));
    while let Ok(msg) = wire::read::<Up>(&mut input) {
        if intake.events.send(HostEvent::Message(conn, msg)).is_err() {
            break;
        }
    }
    intake.writers.lock().unwrap().remove(&conn);
    let _ = intake.events.send(HostEvent::Left(conn));
}
```

In `open_invites`, the spawned task calls `intake.admit(link, format!("invite {who}"))` as before. It now needs `Up: DeserializeOwned + Send + 'static`, which the `impl` block provides.

- [ ] **Step 4: Rewrite `client.rs` generically, greeting first**

Replace the top part of `crates/together-net/src/client.rs`, everything above `fn connect(addr: &str)`, with:

```rust
//! The client's side: one connection to a host, by address or by invite code, read and written
//! on its own threads. It greets the host with its game first; a host of another game answers
//! with a refusal, which arrives as [`ClientEvent::Disconnected`] with the reason.

use std::io::{BufReader, BufWriter};
use std::marker::PhantomData;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::handshake::{self, Greeting, Verdict, HANDSHAKE};
use crate::invite::{self, Link};
use crate::{wire, with_default_port, GameTag};

/// How long to try a host before giving up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

/// What happened on the connection since the last poll.
#[derive(Debug)]
pub enum ClientEvent<Down> {
    /// The host plays our game and took us in.
    Connected,
    Message(Down),
    /// The connection could not be made, was refused, or ended; the reason is for the player.
    Disconnected(String),
}

/// A connection to a host of a game whose clients send `Up` and are sent `Down`, being made or
/// made. Dropping it closes the connection.
pub struct Client<Up, Down> {
    events: Receiver<ClientEvent<Down>>,
    out: Sender<Vec<u8>>,
    _up: PhantomData<fn(&Up)>,
}

impl<Up: Serialize, Down: DeserializeOwned + Send + 'static> Client<Up, Down> {
    /// Start connecting to `to`, an invite code or an address (`host` or `host:port`), as a
    /// player of `game`. [`ClientEvent::Connected`] or [`ClientEvent::Disconnected`] tells how it
    /// went.
    pub fn connect(to: &str, game: GameTag) -> Self {
        let to = to.to_owned();
        let (tx, events) = mpsc::channel();
        let (out, outgoing) = mpsc::channel::<Vec<u8>>();
        let _ = thread::Builder::new()
            .name("together-client".into())
            .spawn(move || run::<Down>(to, game, tx, outgoing));
        Client {
            events,
            out,
            _up: PhantomData,
        }
    }

    /// Everything since the last poll, oldest first.
    pub fn poll(&self) -> Vec<ClientEvent<Down>> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, msg: &Up) {
        if let Ok(frame) = wire::encode(msg) {
            let _ = self.out.send(frame);
        }
    }
}

fn run<Down: DeserializeOwned>(
    to: String,
    game: GameTag,
    events: Sender<ClientEvent<Down>>,
    outgoing: Receiver<Vec<u8>>,
) {
    let link = match invite::parse_code(&to) {
        Some(host) => invite::dial(host),
        None => connect(&with_default_port(&to)),
    };
    let Link {
        reader,
        writer,
        close,
    } = match link {
        Ok(l) => l,
        Err(why) => {
            let _ = events.send(ClientEvent::Disconnected(why));
            return;
        }
    };
    let mut input = BufReader::new(reader);
    let mut out = BufWriter::new(writer);
    let greeting = Greeting {
        handshake: HANDSHAKE,
        game: game.to_owned(),
    };
    if wire::encode(&greeting)
        .and_then(|frame| wire::write_frame(&mut out, &frame))
        .is_err()
    {
        let _ = events.send(ClientEvent::Disconnected("could not greet the host".into()));
        close();
        return;
    }
    match wire::read::<Verdict>(&mut input) {
        Ok(Verdict::Welcome) => {}
        Ok(refused) => {
            let _ = events.send(ClientEvent::Disconnected(handshake::refusal(&refused, game)));
            close();
            return;
        }
        Err(_) => {
            let _ = events.send(ClientEvent::Disconnected(
                "the host closed the connection".into(),
            ));
            close();
            return;
        }
    }
    let _ = events.send(ClientEvent::Connected);

    // Writer: until the game drops the client.
    let _ = thread::Builder::new()
        .name("together-client-write".into())
        .spawn(move || {
            for frame in outgoing {
                if wire::write_frame(&mut out, &frame).is_err() {
                    break;
                }
            }
            close();
        });

    loop {
        match wire::read::<Down>(&mut input) {
            Ok(msg) => {
                if events.send(ClientEvent::Message(msg)).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = events.send(ClientEvent::Disconnected(
                    "the host closed the connection".into(),
                ));
                return;
            }
        }
    }
}
```

(`fn connect(addr: &str) -> Result<Link, String>` is unchanged.)

- [ ] **Step 5: Rewrite the connection tests with test messages, and add the refusal tests**

Replace the `#[cfg(test)] mod tests` of `client.rs` with:

```rust
#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::Instant;

    use serde::Deserialize;

    use super::*;
    use crate::{Host, HostEvent};

    const GAME: GameTag = "test";

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Up {
        Hello(String),
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    enum Down {
        Welcome(u8),
        Start(String),
    }

    type TestHost = Host<Up, Down>;
    type TestClient = Client<Up, Down>;

    /// Poll until `f` finds something, for up to two seconds.
    fn wait<T>(f: impl FnMut() -> Option<T>) -> T {
        wait_for(Duration::from_secs(2), f)
    }

    /// [`wait`], for up to `limit`.
    fn wait_for<T>(limit: Duration, mut f: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(start.elapsed() < limit, "timed out");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn local(host: &TestHost) -> String {
        format!("127.0.0.1:{}", host.local_addr().port())
    }

    #[test]
    fn a_client_joins_talks_and_leaves() {
        let host = TestHost::listen(0, GAME).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Connected))
        });
        let conn = wait(|| {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Joined(c, _) => Some(c),
                _ => None,
            })
        });

        let hello = Up::Hello("Thrall".into());
        client.send(&hello);
        let got = wait(|| {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Message(c, m) if c == conn => Some(m),
                _ => None,
            })
        });
        assert_eq!(got, hello);

        host.send(conn, &Down::Welcome(1));
        host.broadcast(&Down::Start("duskwood".into()));
        let mut seen = Vec::new();
        wait(|| {
            for e in client.poll() {
                if let ClientEvent::Message(m) = e {
                    seen.push(m);
                }
            }
            (seen.len() == 2).then_some(())
        });
        assert_eq!(seen[0], Down::Welcome(1));

        drop(client);
        wait(|| {
            host.poll()
                .into_iter()
                .find(|e| matches!(e, HostEvent::Left(c) if *c == conn))
        });
        assert_eq!(host.connections(), 0);
    }

    #[test]
    fn a_client_of_another_game_is_told_which_game_the_host_plays() {
        let host = TestHost::listen(0, "survivors").unwrap();
        let client = TestClient::connect(&local(&host), "nightfall");
        let why = wait(|| {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Disconnected(why) => Some(why),
                ClientEvent::Connected => panic!("connected to another game's host"),
                ClientEvent::Message(_) => None,
            })
        });
        assert_eq!(why, "That host is playing Survivors, not Nightfall.");
        thread::sleep(Duration::from_millis(100));
        assert!(host.poll().is_empty(), "the host announced a refused client");
        assert_eq!(host.connections(), 0);
    }

    #[test]
    fn a_connection_that_never_greets_never_joins_and_blocks_nobody() {
        let host = TestHost::listen(0, GAME).unwrap();
        // Not a greeting: a length prefix far over the limit, as a stray program might send.
        let mut stray = TcpStream::connect(local(&host)).unwrap();
        stray.write_all(&u32::MAX.to_le_bytes()).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Connected))
        });
        let joined: Vec<crate::ConnId> = wait(|| {
            let j: Vec<_> = host
                .poll()
                .into_iter()
                .filter_map(|e| match e {
                    HostEvent::Joined(c, _) => Some(c),
                    _ => None,
                })
                .collect();
            (!j.is_empty()).then_some(j)
        });
        assert_eq!(joined.len(), 1, "only the real client joined");
        assert_eq!(host.connections(), 1);
    }

    #[test]
    fn nobody_listening_is_a_disconnect_with_a_reason() {
        // Bind and drop, so the port is very likely closed.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let client = TestClient::connect(&format!("127.0.0.1:{port}"), GAME);
        // Windows retries a refused localhost connection for a couple of seconds.
        let why = wait_for(CONNECT_TIMEOUT + Duration::from_secs(2), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Disconnected(why) => Some(why),
                _ => None,
            })
        });
        assert!(why.contains("cannot reach"), "{why}");
    }

    #[test]
    fn the_host_going_away_disconnects_the_client() {
        let host = TestHost::listen(0, GAME).unwrap();
        let client = TestClient::connect(&local(&host), GAME);
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Connected))
        });
        wait(|| {
            host.poll()
                .into_iter()
                .find(|e| matches!(e, HostEvent::Joined(..)))
        });
        drop(host);
        wait(|| {
            client
                .poll()
                .into_iter()
                .find(|e| matches!(e, ClientEvent::Disconnected(_)))
        });
    }

    /// The whole invite path over the internet: number 0's address lookup and relays. Needs a
    /// network, so it runs only when asked: `cargo test -p together-net -- --ignored`.
    #[test]
    #[ignore]
    fn a_client_joins_by_invite_code() {
        let host = TestHost::listen(0, GAME).unwrap();
        let key = crate::new_host_key();
        host.open_invites(key);
        let code = wait_for(Duration::from_secs(30), || match host.invite() {
            crate::Invite::Ready(code) => Some(code),
            crate::Invite::Failed(why) => panic!("{why}"),
            _ => None,
        });
        assert_eq!(code, crate::code_for(&key));
        let client = TestClient::connect(&code, GAME);
        let mut why = None;
        wait_for(Duration::from_secs(40), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Connected => Some(()),
                ClientEvent::Disconnected(w) => {
                    why = Some(w);
                    Some(())
                }
                _ => None,
            })
        });
        assert_eq!(why, None);
        let hello = Up::Hello("Jaina".into());
        client.send(&hello);
        let (conn, got) = wait_for(Duration::from_secs(20), || {
            host.poll().into_iter().find_map(|e| match e {
                HostEvent::Message(c, m) => Some((c, m)),
                _ => None,
            })
        });
        assert_eq!(got, hello);
        host.send(conn, &Down::Welcome(1));
        let welcome = wait_for(Duration::from_secs(20), || {
            client.poll().into_iter().find_map(|e| match e {
                ClientEvent::Message(m) => Some(m),
                _ => None,
            })
        });
        assert_eq!(welcome, Down::Welcome(1));
    }
}
```

In `wire.rs`'s tests, replace `use crate::{ClientMsg, HeroPick};` and the `ClientMsg::Pick(HeroPick {..})` value with a local message:

```rust
#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Msg {
        Pick { name: String, class: String },
    }

    #[test]
    fn a_message_survives_the_round_trip() {
        let msg = Msg::Pick {
            name: "Jaina".into(),
            class: "mage".into(),
        };
        let frame = encode(&msg).unwrap();
        let back: Msg = read(&mut frame.as_slice()).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn an_oversized_length_is_refused() {
        let frame = (u32::MAX).to_le_bytes();
        assert!(read::<Msg>(&mut frame.as_slice()).is_err());
    }
}
```

(Add `use serde::Serialize;` at the test module's top if the compiler asks: `super::*` brings `Serialize` from the file's own imports.)

- [ ] **Step 6: Move Survivors' messages into `together-sim`**

Create `crates/together-sim/src/messages.rs`. It holds the `PROTOCOL`, `HeroPick`, `LobbyPlayer`, `ClientMsg` and `ServerMsg` items cut from the old `together-net/src/lib.rs`, word for word, under this header:

```rust
//! Survivors' co-op messages: what a client tells its host and what the host tells its clients.
//! `together-net` carries them without knowing them.

use serde::{Deserialize, Serialize};

use crate::{Event, PlayerId, PlayerInput, Snapshot};
```

In `crates/together-sim/src/lib.rs` add `pub mod messages;` after `mod math;`.

In `crates/together-sim/Cargo.toml` add:

```toml
[dev-dependencies]
postcard = { version = "1", features = ["use-std"] }
```

Move `crates/together-net/tests/snapshot_size.rs` to `crates/together-sim/tests/snapshot_size.rs`. Replace its two `use` lines with:

```rust
use together_sim::messages::ServerMsg;
use together_sim::{Content, ContentSources, PlayerInput, World, TICK_HZ, V2};
```

In `crates/together-net/Cargo.toml` delete the `together-sim = { path = "../together-sim" }` line and change the description to `"Azeroth Together's co-op wire: the TCP and invite-code threads that carry each game's messages between a host and its clients"`.

- [ ] **Step 7: Point Survivors' session at the new paths**

In `crates/benilla-app/src/together/session.rs` replace the `use together_net::{…};` block (lines 18-21) with:

```rust
use together_net::{content_hash, new_host_key, ConnId, GameTag, Invite, DEFAULT_PORT};
use together_sim::messages::{ClientMsg, HeroPick, LobbyPlayer, ServerMsg, PROTOCOL};

/// This game on the wire: a Nightfall host refuses us, and we refuse its players.
const GAME: GameTag = "survivors";
type Host = together_net::Host<ClientMsg, ServerMsg>;
type Client = together_net::Client<ClientMsg, ServerMsg>;
type HostEvent = together_net::HostEvent<ClientMsg>;
type ClientEvent = together_net::ClientEvent<ServerMsg>;
```

Then make these replacements:
- `Host::listen(DEFAULT_PORT).or_else(|_| Host::listen(0))` becomes `Host::listen(DEFAULT_PORT, GAME).or_else(|_| Host::listen(0, GAME))`.
- `Client::connect(addr)` becomes `Client::connect(addr, GAME)`.
- In `crates/benilla-app/src/together/mod.rs:181`, `together_net::LobbyPlayer` becomes `together_sim::messages::LobbyPlayer`.

- [ ] **Step 8: Run the netcode and sim tests**

Run: `cargo test -p together-net -p together-sim`
Expected: PASS, including `the_same_game_is_welcome`, `another_game_is_refused_with_the_hosts_name`, `another_handshake_is_refused`, `a_client_of_another_game_is_told_which_game_the_host_plays`, `a_connection_that_never_greets_never_joins_and_blocks_nobody` and `a_crowded_snapshot_is_small`.

- [ ] **Step 9: Check the client still builds**

Run (with MinGW on `PATH`): `cargo check -p benilla-app`
Expected: no errors.

- [ ] **Step 10: Commit**

```bash
git add crates/together-net crates/together-sim crates/benilla-app/src/together/session.rs crates/benilla-app/src/together/mod.rs
git commit -m "Net: generic over each game's messages, with a handshake that names the game

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `nightfall-sim`: the crate, `V2` and the map's bounds

**Files:**
- Create: `crates/nightfall-sim/Cargo.toml`, `crates/nightfall-sim/src/lib.rs`, `src/math.rs`, `src/bounds.rs`

**Interfaces:**
- Produces:
  - `nightfall_sim::V2 { x: f32, y: f32 }` with `ZERO`, `new`, `length`, `distance`, `dot`, `normalize_or_zero`, `from_angle`, `is_finite`, and `+`, `+=`, `-`, `*f32`.
  - `nightfall_sim::Bounds` with `new(Vec<V2>) -> Result<Bounds, String>`, `points(&self) -> &[V2]`, `contains(&self, V2) -> bool` and `clamp(&self, V2) -> V2`.

- [ ] **Step 1: Create the crate**

`crates/nightfall-sim/Cargo.toml`:

```toml
[package]
name = "nightfall-sim"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Nightfall's game rules: a headless, fixed-tick co-op stealth extraction simulation with no engine or rendering dependencies"

[lints]
workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
ron = "0.12"

[dev-dependencies]
postcard = { version = "1", features = ["use-std"] }
```

(The workspace's `members = ["crates/*"]` picks it up.)

`crates/nightfall-sim/src/lib.rs`:

```rust
//! Nightfall's game rules: heroes in Duskwood at night, as a plain-Rust simulation stepped at a
//! fixed [`TICK_HZ`].
//!
//! Nothing here knows about Bevy, rendering, WoW's file formats or the survivors games
//! (`together-sim`). The world is WoW's own `(x, y)` ground plane in yards; the client puts every
//! unit on the terrain. That split lets one machine (the co-op host) run the game while every
//! player's client only draws it. Sub-project 1 has heroes, their poses and the map's edge; walls,
//! senses, combat and loot come in later sub-projects (`docs/superpowers/specs/2026-10-16-nightfall-design.md`).

mod bounds;
mod math;

pub use bounds::Bounds;
pub use math::V2;
```

- [ ] **Step 2: Write `math.rs`**

`crates/nightfall-sim/src/math.rs`: a copy of `together-sim/src/math.rs`, with `dot` and `is_finite` added:

```rust
//! A two-component vector in WoW's ground plane: `x` north, `y` west, in yards.

use std::ops::{Add, AddAssign, Mul, Sub};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, other: V2) -> f32 {
        (other - self).length()
    }

    pub fn dot(self, other: V2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Both components are numbers: a pose from a broken or hostile client may not be.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// This vector scaled to length 1, or zero when it is (nearly) zero.
    pub fn normalize_or_zero(self) -> V2 {
        let len = self.length();
        if len > 1e-6 {
            self * (1.0 / len)
        } else {
            V2::ZERO
        }
    }

    /// The point `len` yards along `angle` (radians) from the origin.
    pub fn from_angle(angle: f32, len: f32) -> V2 {
        V2::new(angle.cos() * len, angle.sin() * len)
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}

impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_has_unit_length_and_zero_stays_zero() {
        assert!((V2::new(3.0, 4.0).normalize_or_zero().length() - 1.0).abs() < 1e-6);
        assert_eq!(V2::ZERO.normalize_or_zero(), V2::ZERO);
    }

    #[test]
    fn dot_and_finiteness() {
        assert_eq!(V2::new(1.0, 2.0).dot(V2::new(3.0, 4.0)), 11.0);
        assert!(V2::new(1.0, 2.0).is_finite());
        assert!(!V2::new(f32::NAN, 0.0).is_finite());
        assert!(!V2::new(0.0, f32::INFINITY).is_finite());
    }
}
```

- [ ] **Step 3: Write the failing tests for `Bounds`**

`crates/nightfall-sim/src/bounds.rs`:

```rust
//! A run's edge: a simple polygon in WoW's ground plane. Heroes are kept inside it, so nobody
//! walks off the end of the map or out of the run.

use crate::V2;

#[derive(Clone, Debug, PartialEq)]
pub struct Bounds {
    points: Vec<V2>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Bounds {
        Bounds::new(vec![
            V2::new(0.0, 0.0),
            V2::new(100.0, 0.0),
            V2::new(100.0, 100.0),
            V2::new(0.0, 100.0),
        ])
        .unwrap()
    }

    /// An L: the square without its top-right quarter.
    fn ell() -> Bounds {
        Bounds::new(vec![
            V2::new(0.0, 0.0),
            V2::new(100.0, 0.0),
            V2::new(100.0, 50.0),
            V2::new(50.0, 50.0),
            V2::new(50.0, 100.0),
            V2::new(0.0, 100.0),
        ])
        .unwrap()
    }

    #[test]
    fn fewer_than_three_points_is_no_polygon() {
        assert!(Bounds::new(vec![V2::ZERO, V2::new(1.0, 0.0)]).is_err());
    }

    #[test]
    fn inside_and_outside() {
        let b = square();
        assert!(b.contains(V2::new(50.0, 50.0)));
        assert!(!b.contains(V2::new(150.0, 50.0)));
        assert!(!b.contains(V2::new(-1.0, 50.0)));
        assert!(!ell().contains(V2::new(75.0, 75.0)));
        assert!(ell().contains(V2::new(25.0, 75.0)));
    }

    #[test]
    fn a_point_inside_is_left_alone() {
        assert_eq!(square().clamp(V2::new(20.0, 30.0)), V2::new(20.0, 30.0));
    }

    #[test]
    fn a_point_outside_goes_to_the_nearest_edge() {
        let b = square();
        assert_eq!(b.clamp(V2::new(150.0, 40.0)), V2::new(100.0, 40.0));
        assert_eq!(b.clamp(V2::new(-10.0, -10.0)), V2::new(0.0, 0.0));
        // In the L's missing corner, the nearer of its two inner edges.
        let c = ell().clamp(V2::new(60.0, 90.0));
        assert_eq!(c, V2::new(50.0, 90.0));
    }
}
```

- [ ] **Step 4: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile with `no function or associated item named 'new' found for struct 'Bounds'`.

- [ ] **Step 5: Implement `Bounds`**

Add above the tests in `bounds.rs`:

```rust
impl Bounds {
    /// A polygon through `points` in order (either winding); at least three.
    pub fn new(points: Vec<V2>) -> Result<Self, String> {
        if points.len() < 3 {
            return Err(format!("bounds need 3 points or more, got {}", points.len()));
        }
        if points.iter().any(|p| !p.is_finite()) {
            return Err("bounds have a point that is not a number".into());
        }
        Ok(Self { points })
    }

    pub fn points(&self) -> &[V2] {
        &self.points
    }

    /// Whether `p` lies inside (even-odd rule; a point on an edge may go either way).
    pub fn contains(&self, p: V2) -> bool {
        let mut inside = false;
        let n = self.points.len();
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + n - 1) % n];
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    /// `p` itself when inside, else the nearest point on the edge.
    pub fn clamp(&self, p: V2) -> V2 {
        if self.contains(p) {
            return p;
        }
        let n = self.points.len();
        let mut best = self.points[0];
        let mut best_d = f32::INFINITY;
        for i in 0..n {
            let q = nearest_on_segment(p, self.points[i], self.points[(i + 1) % n]);
            let d = q.distance(p);
            if d < best_d {
                best_d = d;
                best = q;
            }
        }
        best
    }
}

/// The point of the segment `a`–`b` nearest `p`.
fn nearest_on_segment(p: V2, a: V2, b: V2) -> V2 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= 1e-12 {
        return a;
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    a + ab * t
}
```

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS (6 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: the crate, V2 and the map's bounds

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Content: classes, the Duskwood map, the rules

**Files:**
- Create: `crates/nightfall-sim/src/content.rs`
- Create: `content/nightfall/classes.ron`, `content/nightfall/map.ron`, `content/nightfall/rules.ron`
- Create: `crates/nightfall-sim/tests/shipped_content.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `Bounds`, `V2` (Task 2).
- Produces:
  - `ContentSources<'a> { classes: &'a str, map: &'a str, rules: &'a str }`.
  - `Content { classes: Vec<ClassDef>, map: MapDef, rules: Rules }`, with `Content::from_ron(ContentSources) -> Result<Content, ContentError>` and `Content::class(&self, key: &str) -> Option<usize>`.
  - `ClassDef { key, name, description, color: u32, class_id: u8, power: u8, race: u8, sex: u8, weapons: Vec<WeaponDef>, sheath: u8, base_hp: f32, speed: f32 }`.
  - `WeaponDef { slot: u16, display: u32, class: u8, subclass: u8, material: u8, inventory_type: u8, sheath: u8 }`.
  - `MapDef { key, name, map: u32, bounds: Bounds, entries: Vec<[f32; 3]> }`.
  - `Rules { hero_level: u32, atmosphere: Atmosphere }`.
  - `Atmosphere { night_minute: u32, fog_start: f32, fog_end: f32, fog_color: (f32, f32, f32), view_distance: f32, hero_light: HeroLight }`.
  - `HeroLight { color: (f32, f32, f32), intensity: f32, range: f32 }`.
  - `ContentError` (`Display`).

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/content.rs` starts with the types (no logic) and the tests:

```rust
//! Nightfall's content: its classes, its map and its rules, read from RON (`content/nightfall/`)
//! so they change without a rebuild, and checked as they load so a mistake is a clear message at
//! startup rather than a strange run.

use std::fmt;

use serde::Deserialize;

use crate::{Bounds, V2};

/// One of the class's weapons as WoW's virtual item fields want it.
#[derive(Clone, Debug, Deserialize)]
pub struct WeaponDef {
    /// 0 main hand, 1 off hand, 2 ranged.
    pub slot: u16,
    /// `ItemDisplayInfo.dbc` id.
    pub display: u32,
    pub class: u8,
    pub subclass: u8,
    pub material: u8,
    pub inventory_type: u8,
    pub sheath: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClassDef {
    pub key: String,
    pub name: String,
    /// A line or two for the menu.
    #[serde(default)]
    pub description: String,
    /// The class colour (`RAID_CLASS_COLORS`), `0xRRGGBB`.
    #[serde(default = "white")]
    pub color: u32,
    /// `ChrClasses.dbc` id.
    pub class_id: u8,
    /// `UNIT_FIELD_BYTES_0` byte 3: 0 mana, 1 rage, 3 energy.
    pub power: u8,
    /// `ChrRaces.dbc` id and sex (0 male, 1 female) every hero of the class wears for now.
    pub race: u8,
    pub sex: u8,
    #[serde(default)]
    pub weapons: Vec<WeaponDef>,
    /// The sheath state the hero stands in: 1 melee, 2 ranged.
    #[serde(default = "one_u8")]
    pub sheath: u8,
    pub base_hp: f32,
    /// Running speed, yards per second.
    pub speed: f32,
}

fn white() -> u32 {
    0xFF_FF_FF
}

fn one_u8() -> u8 {
    1
}

/// The map a run plays on.
#[derive(Clone, Debug)]
pub struct MapDef {
    pub key: String,
    pub name: String,
    /// `Map.dbc` id.
    pub map: u32,
    pub bounds: Bounds,
    /// Where a party can start, WoW `(x, y, z)`.
    pub entries: Vec<[f32; 3]>,
}

#[derive(Deserialize)]
struct MapFile {
    key: String,
    name: String,
    map: u32,
    bounds: Vec<(f32, f32)>,
    entries: Vec<(f32, f32, f32)>,
}

/// The light a hero carries: a small glow so a player sees their feet.
#[derive(Clone, Debug, Deserialize)]
pub struct HeroLight {
    /// Linear RGB, 0..1.
    pub color: (f32, f32, f32),
    pub intensity: f32,
    /// Yards.
    pub range: f32,
}

/// How dark and how near: the night, the fog and the view distance.
#[derive(Clone, Debug, Deserialize)]
pub struct Atmosphere {
    /// Minute of the day the light stands at (0 is midnight).
    pub night_minute: u32,
    /// Fog from `fog_start` to `fog_end` yards, in `fog_color` (sRGB 0..1).
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_color: (f32, f32, f32),
    /// How far the world is drawn, yards.
    pub view_distance: f32,
    pub hero_light: HeroLight,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rules {
    /// Every hero's level: fixed, there is no levelling in a run.
    pub hero_level: u32,
    pub atmosphere: Atmosphere,
}

/// The text of each content file.
pub struct ContentSources<'a> {
    pub classes: &'a str,
    pub map: &'a str,
    pub rules: &'a str,
}

#[derive(Clone, Debug)]
pub struct Content {
    pub classes: Vec<ClassDef>,
    pub map: MapDef,
    pub rules: Rules,
}

/// Content that does not load, and why.
#[derive(Debug, PartialEq)]
pub enum ContentError {
    Parse { file: &'static str, error: String },
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASSES: &str = r#"[
        (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 100.0, speed: 7.0),
        (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 90.0, speed: 7.0),
    ]"#;
    const MAP: &str = r#"(key: "test", name: "Test", map: 0,
        bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
        entries: [(50.0, 50.0, 10.0)])"#;
    const RULES: &str = r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
        fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#;

    fn load(classes: &str, map: &str, rules: &str) -> Result<Content, ContentError> {
        Content::from_ron(ContentSources {
            classes,
            map,
            rules,
        })
    }

    #[test]
    fn good_content_loads() {
        let c = load(CLASSES, MAP, RULES).unwrap();
        assert_eq!(c.class("mage"), Some(1));
        assert_eq!(c.class("warlock"), None);
        assert_eq!(c.map.entries, vec![[50.0, 50.0, 10.0]]);
        assert!(c.map.bounds.contains(V2::new(50.0, 50.0)));
        assert_eq!(c.rules.hero_level, 30);
    }

    #[test]
    fn a_parse_error_names_its_file() {
        match load(CLASSES, "(key: ", RULES) {
            Err(ContentError::Parse { file, .. }) => assert_eq!(file, "map.ron"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_entry_outside_the_bounds_is_refused() {
        let map = MAP.replace("(50.0, 50.0, 10.0)", "(150.0, 50.0, 10.0)");
        assert!(matches!(load(CLASSES, &map, RULES), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn a_map_needs_an_entry() {
        let map = MAP.replace("[(50.0, 50.0, 10.0)]", "[]");
        assert!(matches!(load(CLASSES, &map, RULES), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn fog_must_start_before_it_ends() {
        let rules = RULES.replace("fog_start: 10.0", "fog_start: 50.0");
        assert!(matches!(load(CLASSES, MAP, &rules), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn the_night_minute_is_a_minute_of_the_day() {
        let rules = RULES.replace("night_minute: 0", "night_minute: 1440");
        assert!(matches!(load(CLASSES, MAP, &rules), Err(ContentError::Invalid(_))));
    }

    #[test]
    fn class_keys_are_unique_and_present() {
        let twice = CLASSES.replace("\"mage\", name", "\"rogue\", name");
        assert!(matches!(load(&twice, MAP, RULES), Err(ContentError::Invalid(_))));
        assert!(matches!(load("[]", MAP, RULES), Err(ContentError::Invalid(_))));
    }
}
```

In `lib.rs` add `mod content;` and:

```rust
pub use content::{
    Atmosphere, ClassDef, Content, ContentError, ContentSources, HeroLight, MapDef, Rules,
    WeaponDef,
};
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile with `no function or associated item named 'from_ron' found for struct 'Content'`.

- [ ] **Step 3: Implement loading and checking**

Add to `content.rs` above the tests:

```rust
impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Parse { file, error } => write!(f, "content/nightfall/{file}: {error}"),
            ContentError::Invalid(why) => write!(f, "content/nightfall: {why}"),
        }
    }
}

impl std::error::Error for ContentError {}

fn parse<T: for<'de> Deserialize<'de>>(file: &'static str, text: &str) -> Result<T, ContentError> {
    ron::from_str(text).map_err(|e| ContentError::Parse {
        file,
        error: e.to_string(),
    })
}

impl Content {
    pub fn from_ron(src: ContentSources) -> Result<Self, ContentError> {
        let classes: Vec<ClassDef> = parse("classes.ron", src.classes)?;
        let map: MapFile = parse("map.ron", src.map)?;
        let rules: Rules = parse("rules.ron", src.rules)?;
        let invalid = |why: String| Err(ContentError::Invalid(why));

        if classes.is_empty() {
            return invalid("classes.ron has no class".into());
        }
        for (i, c) in classes.iter().enumerate() {
            if classes[..i].iter().any(|d| d.key == c.key) {
                return invalid(format!("class \"{}\" is listed twice", c.key));
            }
        }
        let bounds = Bounds::new(map.bounds.iter().map(|&(x, y)| V2::new(x, y)).collect())
            .map_err(|why| ContentError::Invalid(format!("map.ron: {why}")))?;
        if map.entries.is_empty() {
            return invalid("map.ron has no entry point".into());
        }
        for &(x, y, _) in &map.entries {
            if !bounds.contains(V2::new(x, y)) {
                return invalid(format!("map.ron: entry ({x}, {y}) is outside the bounds"));
            }
        }
        let a = &rules.atmosphere;
        if a.night_minute >= 1440 {
            return invalid(format!("rules.ron: night_minute {} is past 1439", a.night_minute));
        }
        if !(a.fog_start < a.fog_end) {
            return invalid("rules.ron: fog_start must be less than fog_end".into());
        }
        Ok(Self {
            classes,
            map: MapDef {
                key: map.key,
                name: map.name,
                map: map.map,
                bounds,
                entries: map.entries.iter().map(|&(x, y, z)| [x, y, z]).collect(),
            },
            rules,
        })
    }

    /// The class with this key, by index.
    pub fn class(&self, key: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.key == key)
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 5: Write the shipped content**

`content/nightfall/classes.ron`:

```ron
// Nightfall's classes. `race` and `sex` are the body every hero of the class wears for now
// (ChrRaces ids: 1 human; sex 0 male, 1 female). `weapons` are the virtual item slots (slot 0 main
// hand, 1 off hand, 2 ranged), the same looks the survivors classes use. Abilities come in
// sub-project 4.
[
    (
        key: "rogue",
        name: "Rogue",
        description: "A blade in the dark: slips past what it cannot kill, and kills what it reaches unseen.",
        color: 0xFFF569,
        class_id: 4,
        power: 3,
        race: 1,
        sex: 0,
        weapons: [
            (slot: 0, display: 7492, class: 2, subclass: 7, material: 1, inventory_type: 13, sheath: 3),
            (slot: 1, display: 6443, class: 2, subclass: 15, material: 1, inventory_type: 13, sheath: 3),
        ],
        sheath: 1,
        base_hp: 135.0,
        speed: 7.0,
    ),
    (
        key: "mage",
        name: "Mage",
        description: "A scholar of frost and fire: deadly at range, and loud about it.",
        color: 0x69CCF0,
        class_id: 8,
        power: 0,
        race: 1,
        sex: 1,
        weapons: [
            (slot: 0, display: 1927, class: 2, subclass: 10, material: 2, inventory_type: 17, sheath: 2),
        ],
        sheath: 1,
        base_hp: 120.0,
        speed: 7.0,
    ),
]
```

`content/nightfall/map.ron`:

```ron
// Duskwood at night. `map` is the Map.dbc id (0, the Eastern Kingdoms). `bounds` is the run's edge
// in WoW yards (x north, y west): for now a rectangle around the zone, traced along its real edge
// in sub-project 2. `entries` are where a party starts, (x, y, z); the first is the zone centre the
// survivors content already plays in. More entries (the north bridge, the west and east roads) go
// here once measured in the game with NIGHTFALL_TRACE_POS=1.
(
    key: "duskwood",
    name: "Duskwood",
    map: 0,
    bounds: [
        (-9880.0, 800.0),
        (-9880.0, -1850.0),
        (-11500.0, -1850.0),
        (-11500.0, 800.0),
    ],
    entries: [
        (-10051.8, -792.5, 36.5),
    ],
)
```

`content/nightfall/rules.ron`:

```ron
// Nightfall's rules. Sub-project 1 has the atmosphere (spec §10, §11); later sub-projects add their
// own settings here.
(
    // Every hero's level, matching Duskwood's creatures.
    hero_level: 30,
    atmosphere: (
        // Midnight.
        night_minute: 0,
        // Yards; the colour is sRGB 0..1, a dark blue-green.
        fog_start: 10.0,
        fog_end: 40.0,
        fog_color: (0.04, 0.07, 0.08),
        // How far the world is drawn, yards: past the fog, short of the stock 350.
        view_distance: 120.0,
        // A warm glow at each hero, linear RGB.
        hero_light: (color: (1.0, 0.82, 0.6), intensity: 0.6, range: 9.0),
    ),
)
```

- [ ] **Step 6: Write the shipped-content test**

`crates/nightfall-sim/tests/shipped_content.rs`:

```rust
//! The content Nightfall ships (`content/nightfall/`) loads and holds what the game expects.

use nightfall_sim::{Content, ContentSources, V2};

pub fn shipped() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/nightfall/classes.ron"),
        map: include_str!("../../../content/nightfall/map.ron"),
        rules: include_str!("../../../content/nightfall/rules.ron"),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_shipped_content_loads_with_rogue_and_mage_in_duskwood() {
    let c = shipped();
    assert!(c.class("rogue").is_some());
    assert!(c.class("mage").is_some());
    assert_eq!(c.map.key, "duskwood");
    assert_eq!(c.map.map, 0);
    for e in &c.map.entries {
        assert!(c.map.bounds.contains(V2::new(e[0], e[1])));
    }
}
```

- [ ] **Step 7: Run all the crate's tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/nightfall-sim content/nightfall
git commit -m "nightfall-sim: content for Rogue, Mage, Duskwood at night

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The world: heroes, poses, steps and snapshots

**Files:**
- Create: `crates/nightfall-sim/src/world.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`, `crates/nightfall-sim/tests/shipped_content.rs`

**Interfaces:**
- Consumes: `Content`, `Bounds`, `V2` (Tasks 2–3).
- Produces:
  - `PlayerId = u8`, `TICK_HZ: u32 = 30`, `DT: f32`.
  - `HeroPose { pos: V2, facing: V2 }` and `PlayerInput { pose: Option<HeroPose> }`.
  - `Hero { player, class: usize, name: String, race: u8, sex: u8, pos: V2, facing: V2, hp: f32, max_hp: f32, speed: f32 }`.
  - `Snapshot { elapsed: f32, heroes: Vec<Hero> }`.
  - `WorldError { NoSuchClass(String), PlayerTaken(PlayerId) }`.
  - `World` with:
    - `new(Arc<Content>) -> World` and `content(&self) -> &Arc<Content>`.
    - `add_hero(&mut self, player, class_key: &str, name: &str, race: u8, sex: u8) -> Result<(), WorldError>`, `remove_hero(&mut self, player)` and `hero(&self, player) -> Option<&Hero>`.
    - `set_input(&mut self, player, PlayerInput)` and `step(&mut self)`.
    - `snapshot(&self) -> Snapshot` and `apply_snapshot(&mut self, Snapshot)`.
    - Public fields `heroes: Vec<Hero>` and `elapsed: f32`.

- [ ] **Step 1: Write the failing tests**

`crates/nightfall-sim/src/world.rs`:

```rust
//! The world of a run: its heroes and where they stand. A hero's position comes from its player's
//! client (WoW's controller moves it there, as a WoW client reports its moves to a server); the
//! world keeps it inside the map. Sub-project 1 has nothing else to simulate; later sub-projects
//! add enemies, senses, combat and loot here.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{Content, V2};

/// A player, numbered from 0 (the host).
pub type PlayerId = u8;

/// Steps a second.
pub const TICK_HZ: u32 = 30;
/// Seconds a step.
pub const DT: f32 = 1.0 / TICK_HZ as f32;

/// How far apart heroes who start at the same entry stand, yards.
const SPAWN_SPACING: f32 = 2.0;

/// Where a player's client put its hero, and the way it faces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeroPose {
    pub pos: V2,
    pub facing: V2,
}

/// What a player sends a step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub pose: Option<HeroPose>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hero {
    pub player: PlayerId,
    /// Index into [`Content::classes`].
    pub class: usize,
    pub name: String,
    pub race: u8,
    pub sex: u8,
    pub pos: V2,
    pub facing: V2,
    pub hp: f32,
    pub max_hp: f32,
    /// Running speed, yards a second.
    pub speed: f32,
}

/// Everything a client needs to draw the world as the host has it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub elapsed: f32,
    pub heroes: Vec<Hero>,
}

#[derive(Debug, PartialEq)]
pub enum WorldError {
    NoSuchClass(String),
    PlayerTaken(PlayerId),
}

pub struct World {
    content: Arc<Content>,
    pub heroes: Vec<Hero>,
    /// Seconds since the run began.
    pub elapsed: f32,
    /// Each player's latest input since the last step.
    inputs: Vec<(PlayerId, PlayerInput)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContentSources;

    fn content() -> Arc<Content> {
        Arc::new(
            Content::from_ron(ContentSources {
                classes: r#"[
                    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 135.0, speed: 7.0),
                    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 120.0, speed: 7.0),
                ]"#,
                map: r#"(key: "test", name: "Test", map: 0,
                    bounds: [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)],
                    entries: [(50.0, 50.0, 10.0)])"#,
                rules: r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0,
                    fog_end: 40.0, fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
                    hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)))"#,
            })
            .unwrap(),
        )
    }

    fn pose(x: f32, y: f32) -> PlayerInput {
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing: V2::new(0.0, 2.0),
            }),
        }
    }

    #[test]
    fn heroes_start_at_the_entry_a_little_apart() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "Valeera", 1, 0).unwrap();
        w.add_hero(1, "mage", "Jaina", 1, 1).unwrap();
        let a = w.hero(0).unwrap();
        let b = w.hero(1).unwrap();
        assert_eq!(a.pos, V2::new(50.0, 50.0));
        assert!((a.pos.distance(b.pos) - SPAWN_SPACING).abs() < 1e-4);
        assert_eq!(a.hp, 135.0);
        assert_eq!(b.max_hp, 120.0);
        assert_eq!(b.name, "Jaina");
    }

    #[test]
    fn an_unknown_class_or_a_taken_player_is_refused() {
        let mut w = World::new(content());
        assert_eq!(
            w.add_hero(0, "warlock", "X", 1, 0),
            Err(WorldError::NoSuchClass("warlock".into()))
        );
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        assert_eq!(
            w.add_hero(0, "mage", "B", 1, 1),
            Err(WorldError::PlayerTaken(0))
        );
    }

    #[test]
    fn a_step_takes_the_pose_and_normalizes_the_facing() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(60.0, 40.0));
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.pos, V2::new(60.0, 40.0));
        assert_eq!(h.facing, V2::new(0.0, 1.0));
        assert!((w.elapsed - DT).abs() < 1e-6);
    }

    #[test]
    fn a_pose_outside_the_map_is_kept_inside() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(130.0, 40.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(100.0, 40.0));
    }

    #[test]
    fn a_pose_that_is_not_a_number_is_ignored() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(f32::NAN, 40.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(50.0, 50.0));
        let bad_facing = PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(55.0, 50.0),
                facing: V2::new(f32::INFINITY, 0.0),
            }),
        };
        w.set_input(0, bad_facing);
        w.step();
        let h = w.hero(0).unwrap();
        assert_eq!(h.pos, V2::new(50.0, 50.0));
        assert!(h.facing.is_finite());
    }

    #[test]
    fn no_input_leaves_a_hero_where_it_stands() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(60.0, 40.0));
        w.step();
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(60.0, 40.0));
    }

    #[test]
    fn the_latest_input_before_a_step_wins() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.set_input(0, pose(60.0, 40.0));
        w.set_input(0, pose(70.0, 40.0));
        w.step();
        assert_eq!(w.hero(0).unwrap().pos, V2::new(70.0, 40.0));
    }

    #[test]
    fn input_for_a_player_without_a_hero_does_nothing() {
        let mut w = World::new(content());
        w.set_input(3, pose(60.0, 40.0));
        w.step();
        assert!(w.heroes.is_empty());
    }

    #[test]
    fn a_snapshot_rebuilds_the_world() {
        let mut host = World::new(content());
        host.add_hero(0, "rogue", "A", 1, 0).unwrap();
        host.add_hero(1, "mage", "B", 1, 1).unwrap();
        host.set_input(1, pose(20.0, 30.0));
        host.step();
        let mut mirror = World::new(content());
        mirror.apply_snapshot(host.snapshot());
        assert_eq!(mirror.snapshot(), host.snapshot());
    }

    #[test]
    fn the_same_inputs_give_the_same_world() {
        let run = || {
            let mut w = World::new(content());
            w.add_hero(0, "rogue", "A", 1, 0).unwrap();
            for i in 0..90 {
                w.set_input(0, pose(50.0 + i as f32, 50.0));
                w.step();
            }
            w.snapshot()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_hero_who_leaves_is_gone() {
        let mut w = World::new(content());
        w.add_hero(0, "rogue", "A", 1, 0).unwrap();
        w.add_hero(1, "mage", "B", 1, 1).unwrap();
        w.remove_hero(1);
        assert!(w.hero(1).is_none());
        assert!(w.hero(0).is_some());
    }
}
```

In `lib.rs` add `mod world;` and:

```rust
pub use world::{
    Hero, HeroPose, PlayerId, PlayerInput, Snapshot, World, WorldError, DT, TICK_HZ,
};
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p nightfall-sim`
Expected: FAIL to compile with `no function or associated item named 'new' found for struct 'World'`.

- [ ] **Step 3: Implement the world**

Add to `world.rs` above the tests:

```rust
impl World {
    /// An empty run on the content's map.
    pub fn new(content: Arc<Content>) -> Self {
        Self {
            content,
            heroes: Vec::new(),
            elapsed: 0.0,
            inputs: Vec::new(),
        }
    }

    pub fn content(&self) -> &Arc<Content> {
        &self.content
    }

    /// `player`'s hero, of the class with `class_key`, at the first entry; each later hero stands
    /// [`SPAWN_SPACING`] yards from it, around a circle.
    pub fn add_hero(
        &mut self,
        player: PlayerId,
        class_key: &str,
        name: &str,
        race: u8,
        sex: u8,
    ) -> Result<(), WorldError> {
        let class = self
            .content
            .class(class_key)
            .ok_or_else(|| WorldError::NoSuchClass(class_key.to_owned()))?;
        if self.hero(player).is_some() {
            return Err(WorldError::PlayerTaken(player));
        }
        let map = &self.content.map;
        let entry = V2::new(map.entries[0][0], map.entries[0][1]);
        let k = self.heroes.len();
        let pos = if k == 0 {
            entry
        } else {
            let angle = (k - 1) as f32 * std::f32::consts::TAU / 4.0;
            map.bounds.clamp(entry + V2::from_angle(angle, SPAWN_SPACING))
        };
        let def = &self.content.classes[class];
        self.heroes.push(Hero {
            player,
            class,
            name: name.to_owned(),
            race,
            sex,
            pos,
            facing: V2::new(1.0, 0.0),
            hp: def.base_hp,
            max_hp: def.base_hp,
            speed: def.speed,
        });
        Ok(())
    }

    pub fn remove_hero(&mut self, player: PlayerId) {
        self.heroes.retain(|h| h.player != player);
        self.inputs.retain(|(p, _)| *p != player);
    }

    pub fn hero(&self, player: PlayerId) -> Option<&Hero> {
        self.heroes.iter().find(|h| h.player == player)
    }

    /// `player`'s input for the next step, replacing any earlier one since the last step.
    pub fn set_input(&mut self, player: PlayerId, input: PlayerInput) {
        match self.inputs.iter_mut().find(|(p, _)| *p == player) {
            Some(slot) => slot.1 = input,
            None => self.inputs.push((player, input)),
        }
    }

    /// One step: each hero takes its player's pose, kept inside the map. A pose that is not a
    /// number is dropped, so a broken client cannot poison the world.
    pub fn step(&mut self) {
        let bounds = &self.content.map.bounds;
        for (player, input) in self.inputs.drain(..) {
            let Some(pose) = input.pose else { continue };
            if !pose.pos.is_finite() || !pose.facing.is_finite() {
                continue;
            }
            if let Some(hero) = self.heroes.iter_mut().find(|h| h.player == player) {
                hero.pos = bounds.clamp(pose.pos);
                let facing = pose.facing.normalize_or_zero();
                if facing != V2::ZERO {
                    hero.facing = facing;
                }
            }
        }
        self.elapsed += DT;
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            elapsed: self.elapsed,
            heroes: self.heroes.clone(),
        }
    }

    /// A client's mirror takes the host's world.
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.elapsed = snapshot.elapsed;
        self.heroes = snapshot.heroes;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 5: Play the shipped content**

Append to `crates/nightfall-sim/tests/shipped_content.rs`:

```rust
#[test]
fn both_classes_play_in_duskwood() {
    use std::sync::Arc;

    use nightfall_sim::{HeroPose, PlayerInput, World};

    let content = Arc::new(shipped());
    let mut w = World::new(content.clone());
    w.add_hero(0, "rogue", "Valeera", 1, 0).unwrap();
    w.add_hero(1, "mage", "Jaina", 1, 1).unwrap();
    let far = V2::new(0.0, 0.0);
    w.set_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos: far,
                facing: V2::new(1.0, 0.0),
            }),
        },
    );
    w.step();
    let h = w.hero(0).unwrap();
    assert!(content.map.bounds.contains(h.pos) || content.map.bounds.clamp(h.pos) == h.pos);
    assert_ne!(h.pos, far, "a pose far off the map was kept inside it");
}
```

- [ ] **Step 6: Run all the crate's tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: heroes, poses kept inside the map, steps and snapshots

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Nightfall's wire messages

**Files:**
- Create: `crates/nightfall-sim/src/messages.rs`
- Modify: `crates/nightfall-sim/src/lib.rs`

**Interfaces:**
- Consumes: `PlayerId`, `PlayerInput`, `Snapshot`, `World` (Task 4).
- Produces:
  - `nightfall_sim::messages::PROTOCOL: u32 = 1`.
  - `HeroPick { name: String, class: String, race: u8, sex: u8 }` and `LobbyPlayer { player: PlayerId, hero: HeroPick }`.
  - `ClientMsg { Hello { protocol: u32, content: u64, hero: HeroPick }, Pick(HeroPick), Input(PlayerInput) }`.
  - `ServerMsg { Welcome { you: PlayerId }, Refused(String), Lobby { players: Vec<LobbyPlayer> }, Start, State { snapshot: Snapshot }, Ended }`.

- [ ] **Step 1: Write the messages and their tests**

`crates/nightfall-sim/src/messages.rs`:

```rust
//! Nightfall's co-op messages: what a client tells its host and what the host tells its clients.
//! `together-net` carries them (under the game tag `"nightfall"`) without knowing them.

use serde::{Deserialize, Serialize};

use crate::{PlayerId, PlayerInput, Snapshot};

/// Bumped whenever a message changes shape: a client and host must agree.
pub const PROTOCOL: u32 = 1;

/// The hero a player brings: their name and pick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroPick {
    pub name: String,
    /// A class key from the content.
    pub class: String,
    pub race: u8,
    pub sex: u8,
}

/// A player in the host's lobby.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub player: PlayerId,
    pub hero: HeroPick,
}

/// What a client tells the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// The first message: who is joining, and whether they play the same version with the same
    /// content (`content` is a hash of the content files).
    Hello {
        protocol: u32,
        content: u64,
        hero: HeroPick,
    },
    /// A new pick, made in the lobby.
    Pick(HeroPick),
    /// This step's input, during a run.
    Input(PlayerInput),
}

/// What the host tells a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Joined: the client plays as `you`.
    Welcome { you: PlayerId },
    /// Not joined, and why; the host closes the connection after it.
    Refused(String),
    /// The lobby as it stands.
    Lobby { players: Vec<LobbyPlayer> },
    /// The run begins.
    Start,
    /// The world after a step.
    State { snapshot: Snapshot },
    /// The host ended the run and went back to its menu.
    Ended,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{Content, ContentSources, World};

    #[test]
    fn messages_survive_the_wire() {
        let hello = ClientMsg::Hello {
            protocol: PROTOCOL,
            content: 7,
            hero: HeroPick {
                name: "Valeera".into(),
                class: "rogue".into(),
                race: 1,
                sex: 0,
            },
        };
        let bytes = postcard::to_stdvec(&hello).unwrap();
        assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).unwrap(), hello);
    }

    #[test]
    fn a_full_partys_state_is_small() {
        let content = Arc::new(
            Content::from_ron(ContentSources {
                classes: include_str!("../../../content/nightfall/classes.ron"),
                map: include_str!("../../../content/nightfall/map.ron"),
                rules: include_str!("../../../content/nightfall/rules.ron"),
            })
            .unwrap(),
        );
        let mut w = World::new(content);
        for p in 0..4 {
            w.add_hero(p, if p % 2 == 0 { "rogue" } else { "mage" }, "Twelve chars", 1, 0)
                .unwrap();
        }
        let msg = ServerMsg::State {
            snapshot: w.snapshot(),
        };
        let bytes = postcard::to_stdvec(&msg).unwrap().len();
        // Four heroes at 15 snapshots a second stay far under a kilobyte each.
        assert!(bytes < 400, "{bytes} bytes");
    }
}
```

In `lib.rs` add `pub mod messages;`.

- [ ] **Step 2: Run the tests**

Run: `cargo test -p nightfall-sim`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/nightfall-sim
git commit -m "nightfall-sim: its co-op messages

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The pick-game screen, and Survivors gated behind it

**Files:**
- Create: `crates/benilla-app/src/game_pick.rs`
- Modify: `crates/benilla-app/src/lib.rs` (the module list; `launch`, where `together` adds its plugin, lines ~463-468)
- Modify: `crates/benilla-app/src/together/mod.rs:283-377` (`TogetherPlugin`)
- Modify: `crates/benilla-app/src/together/menu.rs:87-97` (`MenuAction`), `:395-418` (Exit button), `:921-972` (`menu_clicks`), `:1357-1372` (`show_menu`)

**Interfaces:**
- Produces:
  - `crate::game_pick::GameMode { Pick, Survivors, Nightfall }` (`States`).
  - `crate::game_pick::initial_mode(names) -> GameMode`.
  - `crate::game_pick::GamePickPlugin`.
  - `crate::together::SurvivorsSet` (`SystemSet`).
  - `together::menu::menu_visible(GameMode, Phase) -> bool`.

- [ ] **Step 1: Write `game_pick.rs` with its tests first**

```rust
//! The launcher's first screen: which game to play. **Survivors** is the arena and its extraction
//! mode ([`crate::together`]); **Nightfall** is the stealth extraction game
//! ([`crate::nightfall`]). Each has its own main menu, whose Back button returns here.
//!
//! Test runs skip the screen: any `TOGETHER_*` variable starts Survivors and any `NIGHTFALL*` one
//! starts Nightfall, so the existing co-op and screenshot hooks keep working.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::glue::art::GlueArt;
use crate::glue::widgets::{glue_button, GlueBtnKind};
use crate::glue::GlueClicks;

/// Which game the launcher is in.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum GameMode {
    /// The pick screen.
    #[default]
    Pick,
    Survivors,
    Nightfall,
}

/// The game a launch starts in, from the names of the environment's variables.
pub(crate) fn initial_mode<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> GameMode {
    let mut mode = GameMode::Pick;
    for name in names {
        let name = name.as_ref();
        if name == "NIGHTFALL" || name.starts_with("NIGHTFALL_") {
            return GameMode::Nightfall;
        }
        if name.starts_with("TOGETHER_") {
            mode = GameMode::Survivors;
        }
    }
    mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_launch_shows_the_pick_screen() {
        assert_eq!(initial_mode(["PATH", "USERNAME", "WOW_BG"]), GameMode::Pick);
    }

    #[test]
    fn together_variables_start_survivors() {
        assert_eq!(
            initial_mode(["PATH", "TOGETHER_AUTOPLAY"]),
            GameMode::Survivors
        );
    }

    #[test]
    fn nightfall_variables_start_nightfall_even_beside_together_ones() {
        assert_eq!(initial_mode(["NIGHTFALL"]), GameMode::Nightfall);
        assert_eq!(
            initial_mode(["TOGETHER_NAME", "NIGHTFALL_AUTOPLAY"]),
            GameMode::Nightfall
        );
        assert_eq!(initial_mode(["NIGHTFALLEN"]), GameMode::Pick);
    }
}
```

- [ ] **Step 2: Run the tests**

Add `mod game_pick;` to `crates/benilla-app/src/lib.rs` beside `mod together;`.
Run: `cargo test -p benilla-app --lib game_pick`
Expected: PASS (3 tests). Step 1 wrote `initial_mode` together with its tests, because it is one small pure function; this run checks the module compiles before the screen goes in.

- [ ] **Step 3: Add the screen and the plugin**

Append to `game_pick.rs` (above the tests):

```rust
/// The screen's root; [`build_screen`] fills it once the glue art is in.
#[derive(Component)]
struct PickRoot;

#[derive(Component)]
struct PickBuilt;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum PickAction {
    Survivors,
    Nightfall,
    Exit,
}

pub(crate) struct GamePickPlugin;

impl Plugin for GamePickPlugin {
    fn build(&self, app: &mut App) {
        let names = std::env::vars_os().map(|(k, _)| k.to_string_lossy().into_owned());
        app.insert_state(initial_mode(names))
            .add_systems(Startup, spawn_root)
            .add_systems(
                Update,
                (build_screen, clicks, show_screen, apply_ui_scale),
            );
    }
}

fn spawn_root(mut commands: Commands) {
    commands.spawn((
        PickRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            ..default()
        },
        // Nearly opaque: nothing drives the camera on this screen.
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.92)),
        GlobalZIndex(430),
        Visibility::Hidden,
    ));
}

/// Build the screen once the archives are readable, so the glue art is there.
fn build_screen(
    mut commands: Commands,
    roots: Query<Entity, (With<PickRoot>, Without<PickBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    asset_server: Res<AssetServer>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let font = crate::char_select::wow_font(&asset_server);
    commands.entity(root).insert(PickBuilt).with_children(|ui| {
        if let Some(logo) = &art.logo {
            ui.spawn((
                ImageNode::new(logo.clone()),
                Node {
                    width: Val::Px(256.0),
                    height: Val::Px(128.0),
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
        }
        for (action, caption, line) in [
            (
                PickAction::Survivors,
                "Survivors",
                "The arena and its extraction mode: hordes, cards and broken builds.",
            ),
            (
                PickAction::Nightfall,
                "Nightfall",
                "Duskwood at night: sneak in, take what you can, get out alive.",
            ),
        ] {
            glue_button(ui, &art, &font, action, caption, 240.0, 45.0, GlueBtnKind::Normal, 1.0);
            ui.spawn((
                Text::new(line),
                TextFont {
                    font: font.clone(),
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.85, 0.7)),
                Node {
                    margin: UiRect::bottom(Val::Px(16.0)),
                    ..default()
                },
            ));
        }
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(c, &art, &font, PickAction::Exit, "Exit", 120.0, 32.0, GlueBtnKind::Small, 1.0);
        });
    });
}

fn clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &PickAction)>,
    mode: Res<State<GameMode>>,
    mut next: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
) {
    if *mode.get() != GameMode::Pick {
        return;
    }
    let Some(action) = actions.iter().find(|(e, _)| clicks.hit(*e)).map(|(_, a)| *a) else {
        return;
    };
    match action {
        PickAction::Survivors => next.set(GameMode::Survivors),
        PickAction::Nightfall => next.set(GameMode::Nightfall),
        PickAction::Exit => {
            exit.write(AppExit::Success);
        }
    }
}

/// The screen shows only while picking.
fn show_screen(mode: Res<State<GameMode>>, mut roots: Query<&mut Visibility, With<PickRoot>>) {
    let want = if *mode.get() == GameMode::Pick {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}

/// Glue screens lay out on a screen 768 units tall at any resolution (as `together::menu` does);
/// this one runs in every game so each menu has it.
fn apply_ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut scale: ResMut<UiScale>) {
    let want = windows.single().map(|w| w.height() / 768.0).unwrap_or(1.0);
    if want > 0.0 && (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}
```

- [ ] **Step 4: Add the plugin before Survivors**

In `crates/benilla-app/src/lib.rs`, where `launch` adds `together::TogetherPlugin`:

```rust
    if together {
        app.add_plugins(game_pick::GamePickPlugin);
        app.add_plugins(together::TogetherPlugin);
    }
```

- [ ] **Step 5: Write the menu-visibility test**

In `crates/benilla-app/src/together/menu.rs`, add a test module at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_pick::GameMode;

    #[test]
    fn the_menu_shows_only_in_survivors_at_the_menu() {
        assert!(menu_visible(GameMode::Survivors, Phase::Menu));
        assert!(!menu_visible(GameMode::Survivors, Phase::Playing));
        assert!(!menu_visible(GameMode::Pick, Phase::Menu));
        assert!(!menu_visible(GameMode::Nightfall, Phase::Menu));
    }
}
```

Run: `cargo test -p benilla-app --lib together::menu`
Expected: FAIL to compile with `cannot find function 'menu_visible'`.

- [ ] **Step 6: Gate the menu on the game, and turn Exit into Back**

In `together/menu.rs`, replace `show_menu` with:

```rust
/// Whether Survivors' menu shows: in Survivors, at the menu.
pub(super) fn menu_visible(mode: crate::game_pick::GameMode, phase: Phase) -> bool {
    mode == crate::game_pick::GameMode::Survivors && phase == Phase::Menu
}

/// The menu's root shows only at Survivors' menu.
pub(super) fn show_menu(
    mode: Res<State<crate::game_pick::GameMode>>,
    phase: Res<State<Phase>>,
    mut roots: Query<&mut Visibility, With<MenuRoot>>,
) {
    let want = if menu_visible(*mode.get(), *phase.get()) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut roots {
        if *v != want {
            *v = want;
        }
    }
}
```

In `MenuAction`, rename `Exit` to `Back`. In `build_menu`'s bottom-right corner, change the `glue_button` there to `MenuAction::Back` with the caption `"Back"`, and replace its comment with `// Back to the pick-game screen, where Exit now lives.` In `menu_clicks`, replace the parameter `mut exit: MessageWriter<AppExit>,` with `mut next_mode: ResMut<NextState<crate::game_pick::GameMode>>,` and the `MenuAction::Exit` arm with:

```rust
        MenuAction::Back => {
            // Hosting or joined: the session ends with the menu.
            net.leave("");
            dialog.open = false;
            next_mode.set(crate::game_pick::GameMode::Pick);
        }
```

- [ ] **Step 7: Gate every Survivors system on `GameMode::Survivors`**

In `crates/benilla-app/src/together/mod.rs` add, beside `TogetherEntity`:

```rust
/// Every Survivors system that runs each frame; they run only while Survivors is the game picked,
/// so its daylight, its map and its menu never fight Nightfall's.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SurvivorsSet;
```

In `TogetherPlugin::build`:
- After `app.init_state::<Phase>()` add `.configure_sets(Update, SurvivorsSet.run_if(in_state(crate::game_pick::GameMode::Survivors)))`.
- Remove `menu::show_menu,` from the first `Update` tuple and add it as its own, ungated: `.add_systems(Update, menu::show_menu)`.
- Add `.in_set(SurvivorsSet)` to each of the other four `Update` registrations:
  - the first tuple, after its closing `)`;
  - the menu chain, before `.run_if(in_state(Phase::Menu))`;
  - `menu::menu_camera`;
  - both run chains.
- Add `.add_systems(OnExit(crate::game_pick::GameMode::Survivors), menu::clear_preview)`.

- [ ] **Step 8: Run the tests and check the build**

Run: `cargo test -p benilla-app --lib game_pick together::menu`
Expected: PASS (4 tests).
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 9: See it in a background window**

Run:

```bash
WOW_BG=1 WOW_WIN=1280x720 WOW_LIVE_SHOT=target/pick.png WOW_LIVE_SHOT_AT=25 ./target/release/azeroth-together.exe
```

Expected: `target/pick.png` shows the logo, Survivors and Nightfall buttons with their lines, and Exit. Then run `WOW_BG=1 WOW_WIN=1280x720 TOGETHER_AUTOPLAY=1 WOW_LIVE_SHOT=target/survivors.png WOW_LIVE_SHOT_AT=40 ./target/release/azeroth-together.exe`. Expected: an autoplayed Survivors run in daylight, as before: the pick screen was skipped. Read both PNGs to check.

- [ ] **Step 10: Commit**

```bash
git add crates/benilla-app/src/game_pick.rs crates/benilla-app/src/lib.rs crates/benilla-app/src/together
git commit -m "A pick-game screen before Survivors; Survivors' systems run only when picked

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The fog override in the engine

**Files:**
- Modify: `crates/benilla-world/src/lighting/mod.rs` (the resource, its system, its registration in `LightingPlugin::build` near line 221, a test)

**Interfaces:**
- Produces:
  - `benilla_world::lighting::FogOverride { start: f32, end: f32, color: [f32; 3] }` (`Resource`, public).
  - When present, the resolved `WowLighting` fog (scene and interior) equals it each frame.

- [ ] **Step 1: Write the failing test**

Append to `crates/benilla-world/src/lighting/mod.rs`:

```rust
#[cfg(test)]
mod fog_override_tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use super::*;

    #[test]
    fn a_fog_override_replaces_the_scene_and_interior_fog() {
        let mut world = World::new();
        world.insert_resource(WowLighting {
            fog_start: 125.0,
            fog_end: 500.0,
            fog_color: [0.5, 0.5, 0.5],
            wmo_fog_start: 60.0,
            wmo_fog_end: 200.0,
            wmo_fog_color: [0.4, 0.4, 0.4],
            ..default()
        });
        world.insert_resource(FogOverride {
            start: 10.0,
            end: 40.0,
            color: [0.04, 0.07, 0.08],
        });
        world.run_system_once(apply_fog_override).unwrap();
        let l = world.resource::<WowLighting>();
        assert_eq!((l.fog_start, l.fog_end, l.fog_color), (10.0, 40.0, [0.04, 0.07, 0.08]));
        assert_eq!(
            (l.wmo_fog_start, l.wmo_fog_end, l.wmo_fog_color),
            (10.0, 40.0, [0.04, 0.07, 0.08])
        );
    }

    #[test]
    fn without_an_override_the_fog_is_untouched() {
        let mut world = World::new();
        world.insert_resource(WowLighting {
            fog_start: 125.0,
            fog_end: 500.0,
            ..default()
        });
        world.run_system_once(apply_fog_override).unwrap();
        let l = world.resource::<WowLighting>();
        assert_eq!((l.fog_start, l.fog_end), (125.0, 500.0));
    }
}
```

Run: `cargo test -p benilla-world --lib fog_override`
Expected: FAIL to compile with `cannot find type 'FogOverride'`.

- [ ] **Step 2: Add the resource and its system**

Add to `crates/benilla-world/src/lighting/mod.rs`, after the `WowLighting` definition:

```rust
/// A game's own fog over the `Light.dbc` one (Nightfall's night): `start` and `end` in yards,
/// `color` sRGB 0..1 as [`WowLighting::fog_color`]. Absent, the world's own fog stands. It sets the
/// interior fog too, so a building is as murky as the woods outside.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FogOverride {
    pub start: f32,
    pub end: f32,
    pub color: [f32; 3],
}

/// [`FogOverride`] onto this frame's resolved lighting, right after the resolve writes it.
pub(crate) fn apply_fog_override(fog: Option<Res<FogOverride>>, mut lighting: ResMut<WowLighting>) {
    let Some(fog) = fog else { return };
    let mut l = *lighting;
    l.fog_start = fog.start;
    l.fog_end = fog.end;
    l.fog_color = fog.color;
    l.wmo_fog_start = fog.start;
    l.wmo_fog_end = fog.end;
    l.wmo_fog_color = fog.color;
    if l != *lighting {
        *lighting = l;
    }
}
```

In `LightingPlugin::build`, change `(update_time_lighting, apply_sky_backdrop)` to `(update_time_lighting, apply_fog_override, apply_sky_backdrop)`.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p benilla-world --lib fog_override`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/benilla-world/src/lighting/mod.rs
git commit -m "Lighting: a public fog override a game can set over Light.dbc's

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Nightfall's client module: content, plugin and menu

**Files:**
- Create: `crates/benilla-app/src/nightfall/mod.rs`, `crates/benilla-app/src/nightfall/menu.rs`
- Modify: `crates/benilla-app/src/lib.rs` (`mod nightfall;`; `run_together` calls `nightfall::prepare`; `launch` adds `nightfall::NightfallPlugin`), `crates/benilla-app/Cargo.toml` (add `nightfall-sim`)

**Interfaces:**
- Consumes:
  - `GameMode` (Task 6).
  - `Content`, `ContentSources` (Task 3).
- Produces:
  - `nightfall::prepare(project_dir: &str) -> Result<(), String>`.
  - `nightfall::content() -> Arc<Content>` and `nightfall::content_texts() -> &'static (String, String, String)`.
  - `nightfall::Phase { Menu, Playing }`, `nightfall::NightfallSet`, `nightfall::NightfallEntity` and `nightfall::NightfallPlugin`.
  - `menu::Selection { class: usize }` and `menu::StartRequest(bool)`.
  - `menu::MenuAction`, `menu::LeaveRun`, `menu::roots_visible(GameMode, Phase) -> (bool, bool)`.

This task builds the menu with its buttons wired to a `session::Net` that Task 9 writes. To keep the task compiling on its own, Step 3 adds a minimal `session.rs` stub (Solo only), which Task 9 replaces whole.

- [ ] **Step 1: Add the dependency and the module skeleton**

In `crates/benilla-app/Cargo.toml`, under the `together-net` line:

```toml
# Nightfall's game rules, which the `nightfall` game draws.
nightfall-sim = { path = "../nightfall-sim" }
```

`crates/benilla-app/src/nightfall/mod.rs`:

```rust
//! Nightfall: the co-op stealth extraction game, picked on the launcher's first screen
//! ([`crate::game_pick`]). Its rules are [`nightfall_sim::World`], a headless simulation; this
//! module steps it (alone or hosting) or mirrors a host's (joined) and draws it with the real WoW
//! client: its interface, its character controller, Duskwood's terrain. It decides nothing about
//! the game.
//!
//! It shares no code with the survivors games (`crate::together`): what both need (units, dress,
//! sessions) is copied here, so the two can change apart. Configuration: `NIGHTFALL_CLASS`
//! (`rogue` or `mage`), `NIGHTFALL_NAME`, `NIGHTFALL_CONTENT` (a folder with the RON files; default
//! the repository's `content/nightfall/`, then the copies compiled in).

mod menu;
mod session;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bevy::prelude::*;
use nightfall_sim::{Content, ContentSources};

use crate::game_pick::GameMode;

static CONTENT: OnceLock<Arc<Content>> = OnceLock::new();
/// The content files' text (classes, map, rules), for the hash a client and host compare.
static CONTENT_TEXTS: OnceLock<(String, String, String)> = OnceLock::new();

pub(super) fn content_texts() -> &'static (String, String, String) {
    CONTENT_TEXTS.get().expect("nightfall::prepare ran")
}

/// The game's content, as [`prepare`] loaded it.
pub(super) fn content() -> Arc<Content> {
    CONTENT.get().expect("nightfall::prepare ran").clone()
}

/// The class a launch starts on: `NIGHTFALL_CLASS`, else the first.
fn class_key() -> Option<String> {
    std::env::var("NIGHTFALL_CLASS").ok()
}

/// The folder the RON files are read from, if one exists on disk.
fn content_dir(project_dir: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("NIGHTFALL_CONTENT").map(PathBuf::from),
        Some(Path::new(project_dir).join("content").join("nightfall")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("content").join("nightfall"))),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("map.ron").is_file())
}

/// Read the content (from disk when there, so it can be edited without a rebuild); called once
/// by [`crate::run_together`] beside Survivors' own. Bad content is reported and exits.
pub(crate) fn prepare(project_dir: &str) -> Result<(), String> {
    let dir = content_dir(project_dir);
    let read = |name: &str, built_in: &'static str| -> Result<String, String> {
        match &dir {
            Some(d) => std::fs::read_to_string(d.join(name))
                .map_err(|e| format!("{}: {e}", d.join(name).display())),
            None => Ok(built_in.to_owned()),
        }
    };
    let classes = read(
        "classes.ron",
        include_str!("../../../../content/nightfall/classes.ron"),
    )?;
    let map = read("map.ron", include_str!("../../../../content/nightfall/map.ron"))?;
    let rules = read(
        "rules.ron",
        include_str!("../../../../content/nightfall/rules.ron"),
    )?;
    let content = Content::from_ron(ContentSources {
        classes: &classes,
        map: &map,
        rules: &rules,
    })
    .map_err(|e| e.to_string())?;
    if let Some(key) = class_key() {
        if content.class(&key).is_none() {
            return Err(format!("NIGHTFALL_CLASS: no class \"{key}\" in classes.ron"));
        }
    }
    match &dir {
        Some(d) => println!("nightfall: content from {}", d.display()),
        None => println!("nightfall: content compiled in"),
    }
    let _ = CONTENT.set(Arc::new(content));
    let _ = CONTENT_TEXTS.set((classes, map, rules));
    Ok(())
}

/// The screen Nightfall is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    /// The main menu: pick a class, play alone, host or join.
    #[default]
    Menu,
    /// A run.
    Playing,
}

/// Every Nightfall system that runs each frame; they run only while Nightfall is the game picked.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct NightfallSet;

/// Everything a run spawned, despawned when it ends.
#[derive(Component)]
pub(super) struct NightfallEntity;

pub(crate) struct NightfallPlugin;

impl Plugin for NightfallPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Phase>()
            .configure_sets(Update, NightfallSet.run_if(in_state(GameMode::Nightfall)))
            .insert_resource(menu::Selection::new(&content()))
            .init_resource::<menu::StartRequest>()
            .init_resource::<session::Net>()
            .add_systems(Startup, menu::spawn_roots)
            // Ungated: hides Nightfall's screens whenever another game is up.
            .add_systems(Update, menu::show_roots)
            .add_systems(
                Update,
                (
                    menu::build_menu,
                    menu::menu_clicks,
                    menu::refresh_menu,
                )
                    .chain()
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Menu)),
            );
    }
}
```

In `crates/benilla-app/src/lib.rs`:
- Add `mod nightfall;` beside `mod together;`.
- In `run_together`, after `together::prepare` succeeds:

```rust
    if let Err(e) = nightfall::prepare(build.project_dir) {
        eprintln!("azeroth-together: {e}");
        return AppExit::error();
    }
```

- In `launch`, after `app.add_plugins(together::TogetherPlugin);` add `app.add_plugins(nightfall::NightfallPlugin);`.

- [ ] **Step 2: Write the menu's visibility test**

`crates/benilla-app/src/nightfall/menu.rs` starts with:

```rust
//! Nightfall's main menu, built from the glue screens' own widgets and art ([`crate::glue`]): the
//! title, the class buttons and the class's line, Play Solo, Host Co-op, Join (an invite code or an
//! address from the clipboard), the lobby's status, and Back to the pick screen. During a run, a
//! Leave Run button at the top right.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};
use nightfall_sim::Content;

use super::session::{Net, Role};
use super::{content, Phase};
use crate::game_pick::GameMode;
use crate::glue::art::GlueArt;
use crate::glue::widgets::{glue_button, GlueBtnKind};
use crate::glue::GlueClicks;
use crate::textinput::{wayland_display, HostClipboard};

/// Which (menu, run bar) show: the menu at Nightfall's menu, the run bar during its runs.
pub(super) fn roots_visible(mode: GameMode, phase: Phase) -> (bool, bool) {
    let ours = mode == GameMode::Nightfall;
    (ours && phase == Phase::Menu, ours && phase == Phase::Playing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nightfall_screens_show_only_in_nightfall() {
        assert_eq!(roots_visible(GameMode::Nightfall, Phase::Menu), (true, false));
        assert_eq!(roots_visible(GameMode::Nightfall, Phase::Playing), (false, true));
        assert_eq!(roots_visible(GameMode::Pick, Phase::Menu), (false, false));
        assert_eq!(roots_visible(GameMode::Survivors, Phase::Playing), (false, false));
    }

    #[test]
    fn a_pasted_code_is_cleaned_of_what_surrounds_it() {
        assert_eq!(clean_address("  abc123xyz\n"), Some("abc123xyz".into()));
        assert_eq!(clean_address("192.168.1.5:7777"), Some("192.168.1.5:7777".into()));
        assert_eq!(clean_address(" \n "), None);
    }
}
```

- [ ] **Step 3: Add the session stub, so the module compiles**

`crates/benilla-app/src/nightfall/session.rs` (replaced whole in Task 9):

```rust
//! Co-op for Nightfall. Task 9 of the foundation plan fills this in; until then a run is solo.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub(super) struct Net {
    pub role: Role,
    pub status: String,
}

#[derive(Default)]
pub(super) enum Role {
    #[default]
    Solo,
}

impl Net {
    pub fn is_solo(&self) -> bool {
        matches!(self.role, Role::Solo)
    }

    pub fn host(&mut self) {
        self.status = "Hosting arrives with the session code.".into();
    }

    pub fn join(&mut self, _addr: &str) {
        self.status = "Joining arrives with the session code.".into();
    }

    pub fn leave(&mut self, why: impl Into<String>) {
        self.role = Role::Solo;
        self.status = why.into();
    }

    /// The invite code, once ready.
    pub fn invite_code(&self) -> Option<String> {
        None
    }

    /// The lines the menu shows under the buttons: who is in the lobby and what is happening.
    pub fn lobby_text(&self) -> String {
        self.status.clone()
    }
}
```

Run: `cargo test -p benilla-app --lib nightfall::menu`
Expected: FAIL to compile with `cannot find function 'clean_address'`.

- [ ] **Step 4: Write the menu**

Add to `menu.rs` above the tests:

```rust
/// The longest code or address the join takes.
const MAX_ADDRESS: usize = 128;

/// The class picked.
#[derive(Resource)]
pub(super) struct Selection {
    pub class: usize,
}

impl Selection {
    pub(super) fn new(content: &Content) -> Self {
        let class = super::class_key()
            .and_then(|k| content.class(&k))
            .unwrap_or(0);
        Self { class }
    }
}

/// Play Solo or Start Run was clicked: the run starts next frame.
#[derive(Resource, Default)]
pub(super) struct StartRequest(pub bool);

#[derive(Component)]
pub(super) struct MenuRoot;

#[derive(Component)]
pub(super) struct RunBar;

#[derive(Component)]
struct MenuBuilt;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuAction {
    Class(usize),
    PlaySolo,
    Host,
    Join,
    CopyCode,
    StartRun,
    Leave,
    Back,
}

/// The run bar's button: end the run (a host takes everyone back to the lobby).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LeaveRun;

#[derive(Component)]
struct ClassLine;

#[derive(Component)]
struct StatusText;

/// The code or address in pasted text, whatever surrounds it.
fn clean_address(text: &str) -> Option<String> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || ".:-[]".contains(*c))
        .take(MAX_ADDRESS)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

pub(super) fn spawn_roots(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(410),
        Visibility::Hidden,
    ));
    commands.spawn((
        RunBar,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(16.0),
            top: Val::Px(16.0),
            ..default()
        },
        GlobalZIndex(410),
        Visibility::Hidden,
    ));
}

pub(super) fn show_roots(
    mode: Res<State<GameMode>>,
    phase: Res<State<Phase>>,
    mut menus: Query<&mut Visibility, (With<MenuRoot>, Without<RunBar>)>,
    mut bars: Query<&mut Visibility, (With<RunBar>, Without<MenuRoot>)>,
) {
    let (menu, bar) = roots_visible(*mode.get(), *phase.get());
    let vis = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut menus {
        if *v != vis(menu) {
            *v = vis(menu);
        }
    }
    for mut v in &mut bars {
        if *v != vis(bar) {
            *v = vis(bar);
        }
    }
}

/// Build both screens once the glue art is in.
pub(super) fn build_menu(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, Without<MenuBuilt>)>,
    bars: Query<Entity, With<RunBar>>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    asset_server: Res<AssetServer>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let font = crate::char_select::wow_font(&asset_server);
    let content = content();
    let text = |s: &str, size: f32, color: Color| {
        (
            Text::new(s),
            TextFont {
                font: font.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
        )
    };
    commands.entity(root).insert(MenuBuilt).with_children(|ui| {
        ui.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(40.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|panel| {
            panel.spawn(text("Nightfall", 32.0, Color::srgb(0.85, 0.8, 0.6)));
            panel
                .spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    for (i, class) in content.classes.iter().enumerate() {
                        glue_button(row, &art, &font, MenuAction::Class(i), &class.name, 150.0, 38.0, GlueBtnKind::Small, 1.0);
                    }
                });
            panel.spawn((text("", 13.0, Color::WHITE), ClassLine));
            panel
                .spawn(Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    for (action, caption) in [
                        (MenuAction::PlaySolo, "Play Solo"),
                        (MenuAction::Host, "Host Co-op"),
                        (MenuAction::Join, "Join (paste code)"),
                        (MenuAction::CopyCode, "Copy Invite Code"),
                        (MenuAction::StartRun, "Start Run"),
                        (MenuAction::Leave, "Leave"),
                    ] {
                        glue_button(row, &art, &font, action, caption, 170.0, 38.0, GlueBtnKind::Small, 1.0);
                    }
                });
            panel.spawn((text("", 13.0, Color::srgb(0.9, 0.85, 0.7)), StatusText));
        });
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            right: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        })
        .with_children(|c| {
            glue_button(c, &art, &font, MenuAction::Back, "Back", 120.0, 32.0, GlueBtnKind::Small, 1.0);
        });
    });
    if let Ok(bar) = bars.single() {
        commands.entity(bar).with_children(|c| {
            glue_button(c, &art, &font, LeaveRun, "Leave Run", 140.0, 32.0, GlueBtnKind::Small, 1.0);
        });
    }
}

pub(super) fn menu_clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &MenuAction)>,
    mut selection: ResMut<Selection>,
    mut start: ResMut<StartRequest>,
    mut net: ResMut<Net>,
    mut next_mode: ResMut<NextState<GameMode>>,
    mut clipboard: NonSendMut<HostClipboard>,
    window: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    let Some(action) = actions.iter().find(|(e, _)| clicks.hit(*e)).map(|(_, a)| *a) else {
        return;
    };
    let display = || wayland_display(window.iter().next());
    match action {
        MenuAction::Class(i) => selection.class = i,
        MenuAction::PlaySolo => {
            if net.is_solo() {
                start.0 = true;
            }
        }
        MenuAction::Host => {
            if net.is_solo() {
                net.host();
            }
        }
        MenuAction::Join => {
            if net.is_solo() {
                match clipboard.read(display()).as_deref().and_then(clean_address) {
                    Some(addr) => net.join(&addr),
                    None => {
                        net.status =
                            "Copy a friend's invite code (or address) first, then Join.".into();
                    }
                }
            }
        }
        MenuAction::CopyCode => {
            if let Some(code) = net.invite_code() {
                clipboard.write(display(), &code);
                net.status = "Invite code copied: paste it to your friends".into();
            }
        }
        MenuAction::StartRun => start.0 = true,
        MenuAction::Leave => net.leave(""),
        MenuAction::Back => {
            net.leave("");
            next_mode.set(GameMode::Pick);
        }
    }
}

/// Which actions show for the session's role: alone, hosting or joined.
fn shown(role: &Role) -> &'static [MenuAction] {
    match role {
        Role::Solo => &[MenuAction::PlaySolo, MenuAction::Host, MenuAction::Join],
    }
}

/// The class line, the status lines and which buttons show.
pub(super) fn refresh_menu(
    selection: Res<Selection>,
    net: Res<Net>,
    mut lines: Query<&mut Text, (With<ClassLine>, Without<StatusText>)>,
    mut status: Query<&mut Text, (With<StatusText>, Without<ClassLine>)>,
    mut buttons: Query<(&MenuAction, &mut Node)>,
) {
    let content = content();
    let class = &content.classes[selection.class];
    let line = format!("{}: {}", class.name, class.description);
    for mut t in &mut lines {
        if t.0 != line {
            t.0 = line.clone();
        }
    }
    let body = net.lobby_text();
    for mut t in &mut status {
        if t.0 != body {
            t.0 = body.clone();
        }
    }
    let visible = shown(&net.role);
    for (action, mut node) in &mut buttons {
        let on = matches!(action, MenuAction::Class(_) | MenuAction::Back) || visible.contains(action);
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    }
}
```

- [ ] **Step 5: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall::menu game_pick together::menu`
Expected: PASS.
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 6: See the menu**

Run: `WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL=1 WOW_LIVE_SHOT=target/nightfall-menu.png WOW_LIVE_SHOT_AT=25 ./target/release/azeroth-together.exe`
Expected: the screenshot shows Nightfall's panel (title, Rogue and Mage, the class line, Play Solo, Host Co-op, Join, Back), and no Survivors menu. The world behind it is still day; the night comes in Task 10.

- [ ] **Step 7: Commit**

```bash
git add crates/benilla-app/Cargo.toml crates/benilla-app/src/lib.rs crates/benilla-app/src/nightfall
git commit -m "Nightfall: its content in the client, its plugin and its menu

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Runs: the sim stepped, heroes drawn, co-op sessions

**Files:**
- Create: `crates/benilla-app/src/nightfall/run.rs`, `view.rs`, `units.rs`, `dress.rs`
- Replace: `crates/benilla-app/src/nightfall/session.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs` (modules, resources, systems), `menu.rs` (`shown`)

**Interfaces:**
- Consumes:
  - `World`, `Snapshot`, `PlayerInput`, `HeroPose`, `DT`, `messages::*` (Tasks 4–5).
  - `Host<Up, Down>`, `Client<Up, Down>`, `GameTag` (Task 1).
  - `menu::{Selection, StartRequest, LeaveRun}` (Task 8).
- Produces:
  - `run::Sim` (a `Resource` with `world: World`, `me: PlayerId`, and methods `alpha() -> f32` and `span() -> f32`).
  - `run::LocalInput`, `run::EndRun`.
  - `view::Views { heroes: HashMap<PlayerId, Entity> }`.
  - `session::{Net, Role, GAME}`.
  - `session::own_pick(&Selection) -> HeroPick`.

- [ ] **Step 1: Copy units and dress**

- Copy `crates/benilla-app/src/together/units.rs` to `crates/benilla-app/src/nightfall/units.rs`.
  - Change `use super::TogetherEntity;` to `use super::NightfallEntity;` and both `TogetherEntity,` to `NightfallEntity,`.
  - Change `together_sim::V2` to `nightfall_sim::V2` (two places).
  - Delete `spawn_object`, `kill_unit`, `Fx` with its `impl`, `COLOR_SPELL` and `COLOR_HURT`.
  - Then delete whatever the compiler reports unused: the gameobject constants and the `creature_anim` and `combat_text` imports.
  - Change the module doc's last sentence to: "Copied from `together::units`, which this game does not depend on."
- Copy `crates/benilla-app/src/together/dress.rs` to `crates/benilla-app/src/nightfall/dress.rs`, and change `together_sim::WeaponDef` and `together_sim::ClassDef` to `nightfall_sim::WeaponDef` and `nightfall_sim::ClassDef`.

- [ ] **Step 2: Write the session's pure parts with tests**

Replace `crates/benilla-app/src/nightfall/session.rs` with the file below. It mirrors `together/session.rs` without zones, cards or pings:

```rust
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

#[cfg(test)]
mod tests {
    use super::*;

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
```

The menu (Task 8) calls `Net`'s methods, which Step 3 adds, so the crate compiles again only after Step 3; run these tests there.

- [ ] **Step 3: Write the rest of the session**

Append to `session.rs` above the tests:

```rust
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
        .ok()
        .map(|n| n.chars().filter(|c| !c.is_control()).take(12).collect())
        .filter(|n: &String| !n.is_empty())
        .unwrap_or_else(|| "Hero".into())
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
                    HostEvent::Message(conn, ClientMsg::Hello { protocol, content, hero }) => {
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
                    HostEvent::Message(conn, ClientMsg::Pick(hero)) => {
                        if let Some(p) = h.peers.iter_mut().find(|p| p.conn == conn) {
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
```

In `menu.rs` replace `shown` with:

```rust
/// Which actions show for the session's role: alone, hosting or joined.
fn shown(role: &Role) -> &'static [MenuAction] {
    match role {
        Role::Solo => &[MenuAction::PlaySolo, MenuAction::Host, MenuAction::Join],
        Role::Host(_) => &[MenuAction::CopyCode, MenuAction::StartRun, MenuAction::Leave],
        Role::Client(_) => &[MenuAction::Leave],
    }
}
```

and make the `StartRun` arm of `menu_clicks` host-only:

```rust
        MenuAction::StartRun => {
            if matches!(net.role, Role::Host(_)) {
                start.0 = true;
            }
        }
```

Run: `cargo test -p benilla-app --lib nightfall::session`
Expected: PASS (3 tests: `a_hello_is_refused_for_version_content_or_a_full_party`, `players_are_numbered_from_one_without_reuse`, `the_roster_marks_us`).

- [ ] **Step 4: Write the view**

`crates/benilla-app/src/nightfall/view.rs`:

```rust
//! The world, drawn: a WoW player unit per hero, where the simulation says. The local hero is a
//! real WoW player that WoW's character controller moves ([`super::run::feed_pose`] reports where);
//! every other hero glides from where it was at the last state to where it is now ([`SimPos`]).

use std::collections::HashMap;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::{PlayerId, V2};

use super::run::Sim;
use super::units::{self, ShownVitals, UnitKind};
use super::{content, dress};
use crate::creature_anim::{move_flags, MovementState};
use crate::names::NameCache;
use crate::net::{GuidIndex, ObjectStore, SelfGuid};

/// Player guids are this plus the player's number.
const HERO_SERIAL_BASE: u64 = 0x10;

/// Which entity draws which hero, and the run speed last given our controller.
#[derive(Resource, Default)]
pub(super) struct Views {
    pub heroes: HashMap<PlayerId, Entity>,
    speed: f32,
}

/// Where another hero was at the last two states, for gliding between them.
#[derive(Component)]
pub(super) struct SimPos {
    prev: V2,
    cur: V2,
    facing: V2,
    speed: f32,
}

impl SimPos {
    fn new(at: V2, facing: V2) -> Self {
        Self {
            prev: at,
            cur: at,
            facing,
            speed: 0.0,
        }
    }

    /// A new state `span` seconds after the last.
    fn advance(&mut self, at: V2, facing: V2, span: f32) {
        self.speed = self.cur.distance(at) / span.max(1e-3);
        self.prev = self.cur;
        self.cur = at;
        if facing != V2::ZERO {
            self.facing = facing;
        }
    }
}

/// Every hero gets a body, filed in the [`GuidIndex`] and [`NameCache`] as the net layer files a
/// streamed one; ours becomes the [`SelfGuid`], which hands it to WoW's controller.
pub(super) fn sync(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    char_create: Option<Res<crate::entities::CharCreate>>,
    world_point: WorldPoint,
    mut self_guid: ResMut<SelfGuid>,
    mut names: ResMut<NameCache>,
    mut index: ResMut<GuidIndex>,
    mut items: ResMut<crate::items::Items>,
    mut positions: Query<&mut SimPos>,
) {
    let world = &sim.world;
    let content = content();
    let fallback_y = content.map.entries[0][2];
    for hero in &world.heroes {
        if let Some(&e) = views.heroes.get(&hero.player) {
            if sim.stepped {
                if let Ok(mut p) = positions.get_mut(e) {
                    p.advance(hero.pos, hero.facing, sim.span());
                }
            }
            continue;
        }
        // The body and outfit tables load after boot.
        let Some(char_create) = char_create.as_deref() else {
            continue;
        };
        let class = &content.classes[hero.class];
        let serial = HERO_SERIAL_BASE + u64::from(hero.player);
        let guid = units::unit_guid(UnitKind::Player, serial);
        let mut at = units::ground_to_bevy(hero.pos, fallback_y);
        if let Some(h) = world_point.terrain_height_under(at) {
            at.y = h;
        }
        let Some(e) = dress::spawn_player_body(
            &mut commands,
            &mut items,
            char_create,
            class,
            (hero.race, hero.sex),
            serial,
            content.rules.hero_level,
            at,
            units::yaw_of(hero.facing),
        ) else {
            continue;
        };
        index.0.insert(guid, e);
        names.insert_player(guid, hero.name.clone(), None);
        if hero.player == sim.me {
            self_guid.0 = Some(guid);
        } else {
            commands.entity(e).insert(SimPos::new(hero.pos, hero.facing));
        }
        views.heroes.insert(hero.player, e);
    }
    // A player who left: their hero goes with them.
    views.heroes.retain(|player, e| {
        let keep = world.hero(*player).is_some();
        if !keep {
            index.0.retain(|_, v| v != e);
            commands.entity(*e).despawn();
        }
        keep
    });
}

/// Health and level into every hero's store, for WoW's player, party and target frames; and the
/// local hero's run speed into its controller.
pub(super) fn write_vitals(
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<Views>,
    mut units: Query<(&mut ObjectStore, &mut ShownVitals)>,
) {
    let world = &sim.world;
    let level = content().rules.hero_level;
    if let (Some(hero), Some(&e)) = (world.hero(sim.me), views.heroes.get(&sim.me)) {
        if hero.speed != views.speed {
            views.speed = hero.speed;
            commands
                .entity(e)
                .insert(crate::net::UnitSpeeds(run_speeds(hero.speed)));
        }
    }
    for hero in &world.heroes {
        if let Some((mut store, mut shown)) = views
            .heroes
            .get(&hero.player)
            .and_then(|&e| units.get_mut(e).ok())
        {
            units::set_vitals(&mut store, &mut shown, hero.hp, hero.max_hp, level, None);
        }
    }
}

/// WoW's speed set for a hero who runs at `run` yards a second, at the stock ratios.
fn run_speeds(run: f32) -> benilla_protocol::MoveSpeeds {
    benilla_protocol::MoveSpeeds {
        walk: run * 2.5 / 7.0,
        run,
        run_back: run * 4.5 / 7.0,
        swim: run * 4.722_222 / 7.0,
        swim_back: run * 2.5 / 7.0,
        turn_rate: std::f32::consts::PI,
    }
}

/// Every other hero between its last two states, on the ground, turned the way it goes, running
/// when it moves.
pub(super) fn place(
    sim: Res<Sim>,
    world_point: WorldPoint,
    time: Res<Time<Real>>,
    mut bodies: Query<(&SimPos, &mut Transform, &mut MovementState)>,
) {
    let alpha = sim.alpha();
    let turn = (time.delta_secs() * 12.0).min(1.0);
    for (p, mut tf, mut mv) in &mut bodies {
        let at = p.prev + (p.cur - p.prev) * alpha;
        let mut pos = units::ground_to_bevy(at, tf.translation.y);
        if let Some(h) = world_point.terrain_height_under(pos) {
            pos.y = h;
        }
        if tf.translation != pos {
            tf.translation = pos;
        }
        if p.facing != V2::ZERO {
            let target = Quat::from_rotation_y(units::yaw_of(p.facing));
            tf.rotation = tf.rotation.slerp(target, turn);
        }
        let flags = if p.speed > 0.1 { move_flags::FORWARD } else { 0 };
        if mv.speed != p.speed || mv.flags != flags {
            mv.speed = p.speed;
            mv.flags = flags;
        }
    }
}
```

- [ ] **Step 5: Write the run**

`crates/benilla-app/src/nightfall/run.rs`:

```rust
//! A run: the world stepped here (alone or hosting) or mirrored from the host (joined), the local
//! hero's pose fed to it, and the way back to the menu. Test hooks: `NIGHTFALL_HOST=1` hosts and
//! `NIGHTFALL_JOIN=<code or address>` joins once the menu is up; `NIGHTFALL_AUTOPLAY=1` starts a
//! run (hosting: once someone joined) and walks a slow circle; `NIGHTFALL_TRACE_POS=1` logs the
//! hero's WoW position every 2 seconds, for measuring entry points.

use std::collections::VecDeque;
use std::sync::OnceLock;

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::messages::LobbyPlayer;
use nightfall_sim::{HeroPose, PlayerId, PlayerInput, Snapshot, World, DT, V2};

use super::menu::{LeaveRun, Selection, StartRequest};
use super::session::{self, Heard, Net, Role};
use super::view::Views;
use super::{content, units, NightfallEntity, Phase};
use crate::glue::GlueClicks;

/// Snapshots a client holds before playing them: the delay that hides a late packet.
const PLAYBACK_DEPTH: usize = 2;
/// More than this waiting (a burst after a stall) and the oldest are played at once.
const PLAYBACK_MAX: usize = 8;

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
    /// Joined: the host's snapshots not yet played, oldest first.
    inbox: VecDeque<Snapshot>,
    /// Joined: playback has begun.
    playing: bool,
    /// A step was taken (or a snapshot applied) this frame.
    pub stepped: bool,
}

impl Sim {
    /// A run for everyone in `roster` (just us, alone), stepped here.
    fn hosted(roster: &[LobbyPlayer]) -> Self {
        let mut world = World::new(content());
        for p in roster {
            // A class the content lacks (a mismatched client) is left out rather than crash.
            let _ = world.add_hero(p.player, &p.hero.class, &p.hero.name, p.hero.race, p.hero.sex);
        }
        Self {
            world,
            me: 0,
            acc: 0.0,
            span: DT,
            inbox: VecDeque::new(),
            playing: false,
            stepped: false,
        }
    }

    /// A joined run: an empty world, filled by the host's snapshots.
    fn mirror(me: PlayerId) -> Self {
        Self {
            world: World::new(content()),
            me,
            acc: 0.0,
            span: DT * session::SEND_EVERY as f32,
            inbox: VecDeque::new(),
            playing: false,
            stepped: false,
        }
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
        if let Some(snapshot) = self.inbox.pop_front() {
            self.world.apply_snapshot(snapshot);
            self.stepped = true;
        }
    }
}

/// The local hero's pose for the next step.
#[derive(Resource, Default)]
pub(super) struct LocalInput {
    pose: Option<HeroPose>,
}

/// The run is over (left, ended by the host, or the connection went): [`back_to_menu`] leaves it.
#[derive(Resource, Default)]
pub(super) struct EndRun(pub bool);

fn autoplay() -> bool {
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
) {
    if net.is_solo() && net.status.is_empty() && time.elapsed_secs() > 8.0 {
        if std::env::var_os("NIGHTFALL_HOST").is_some() {
            net.host();
        } else if let Ok(addr) = std::env::var("NIGHTFALL_JOIN") {
            net.join(&addr);
        }
    }
    if let Role::Client(c) = &mut net.role {
        let (true, Some(me)) = (c.start, c.me) else {
            return;
        };
        c.start = false;
        commands.insert_resource(Sim::mirror(me));
        player.active = false;
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
    if !std::mem::take(&mut start.0) && !auto {
        return;
    }
    let own = session::own_pick(&selection);
    let roster = net.roster(&own);
    net.broadcast_start();
    commands.insert_resource(Sim::hosted(&roster));
    player.active = false;
    next.set(Phase::Playing);
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
    mut player: ResMut<crate::player::Player>,
    mut input: ResMut<LocalInput>,
    mut bodies: Query<&mut Transform>,
    mut last_trace: Local<f32>,
) {
    let content = content();
    let map = &content.map;
    let entry = map.entries[0];
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
    if autoplay() {
        let speed = sim.world.hero(sim.me).map_or(7.0, |h| h.speed);
        pos = autowalk(V2::new(entry[0], entry[1]), pos, sim.world.elapsed, speed, time.delta_secs());
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
    });
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
) {
    sim.stepped = false;
    let sim = &mut *sim;
    if net.is_client() {
        let heard = session::client_exchange(&mut net, PlayerInput { pose: input.pose });
        if session::client_left_run(&mut net, &heard) {
            end.0 = true;
        }
        for h in heard {
            if let Heard::State(snapshot) = h {
                sim.inbox.push_back(snapshot);
            }
        }
        sim.play_back(time.delta_secs());
        return;
    }
    session::host_receive(&mut net, &mut sim.world);
    let Some(pose) = input.pose else {
        return;
    };
    if loading.covering() {
        return;
    }
    // A long hitch would otherwise replay seconds of play in one frame.
    sim.acc = (sim.acc + time.delta_secs()).min(0.25);
    while sim.acc >= DT {
        sim.acc -= DT;
        sim.world.set_input(sim.me, PlayerInput { pose: Some(pose) });
        sim.world.step();
        session::host_send(&mut net, &sim.world);
        sim.stepped = true;
    }
}

/// Leave Run: alone or hosting, the run ends (a host takes its party back to the lobby); joined,
/// we leave the session.
pub(super) fn leave_run(
    clicks: Res<GlueClicks>,
    buttons: Query<Entity, With<LeaveRun>>,
    mut net: ResMut<Net>,
    mut end: ResMut<EndRun>,
) {
    if !buttons.iter().any(|e| clicks.hit(e)) {
        return;
    }
    if net.is_client() {
        net.leave("You left the run");
    } else {
        net.broadcast_ended();
    }
    end.0 = true;
}

/// Back to the menu once the run is over: everything it spawned goes, and the controller lets go.
pub(super) fn back_to_menu(
    mut commands: Commands,
    mut end: ResMut<EndRun>,
    mut views: ResMut<Views>,
    mut index: ResMut<crate::net::GuidIndex>,
    mut player: ResMut<crate::player::Player>,
    mut next: ResMut<NextState<Phase>>,
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
    commands.remove_resource::<Sim>();
    player.active = false;
    next.set(Phase::Menu);
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn bevy_and_wow_ground_agree() {
        let p = V2::new(-10051.8, -792.5);
        assert_eq!(bevy_to_ground(units::ground_to_bevy(p, 36.5)), p);
    }
}
```

- [ ] **Step 6: Register the run in the plugin**

In `nightfall/mod.rs` add the modules beside `mod menu;`:

```rust
mod dress;
mod run;
mod units;
mod view;
```

In `NightfallPlugin::build`:
- After `.init_resource::<session::Net>()` add `.init_resource::<run::LocalInput>()`, `.init_resource::<run::EndRun>()` and `.init_resource::<view::Views>()`.
- Replace the menu chain with `(menu::build_menu, menu::menu_clicks, session::lobby, menu::refresh_menu, run::start_run)`, keeping its `.chain().in_set(NightfallSet).run_if(in_state(Phase::Menu))`.
- Add:

```rust
            .add_systems(Update, session::close_on_exit.in_set(NightfallSet))
            .add_systems(
                Update,
                (
                    run::feed_pose,
                    run::step_sim,
                    view::sync,
                    view::write_vitals,
                    view::place,
                )
                    .chain()
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    // The controller has moved the hero this frame before its pose is read.
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
            .add_systems(
                Update,
                (run::leave_run, run::back_to_menu)
                    .chain()
                    .after(benilla_world::schedule::WorldStage::Input)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Playing).and(resource_exists::<run::Sim>)),
            )
```

- [ ] **Step 7: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall game_pick`
Expected: PASS.
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 8: Play a solo run in the background**

Run:

```bash
WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_TRACE_POS=1 WOW_LIVE_SHOT=target/nightfall-run.png WOW_LIVE_SHOT_AT=40 ./target/release/azeroth-together.exe
```

Expected:
- The log has `nightfall: hero at (-100xx.x, -7xx.x, …)` lines close to the entry.
- The screenshot shows a Rogue (the first class) in Duskwood with WoW's player frame and the Leave Run button.
- It's still daylight; the night comes in Task 10.

- [ ] **Step 9: Play co-op on one PC**

Start the host in the background with the command below, and a few seconds later the client:

```bash
WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_HOST=1 NIGHTFALL_AUTOPLAY=1 NIGHTFALL_NAME=Host WOW_LIVE_SHOT=target/host.png WOW_LIVE_SHOT_AT=45 ./target/release/azeroth-together.exe
```

```bash
WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_JOIN=127.0.0.1 NIGHTFALL_CLASS=mage NIGHTFALL_NAME=Friend WOW_LIVE_SHOT=target/client.png WOW_LIVE_SHOT_AT=40 ./target/release/azeroth-together.exe
```

Expected: the host's log shows `nightfall: invite code …` and `Friend joined`, and the run starts about 20 s in. Both screenshots show two heroes, a Rogue and a Mage.
Then check the refusal: start a Survivors client against the Nightfall host with `TOGETHER_JOIN=127.0.0.1 WOW_BG=1 …`. Its lobby status reads `Disconnected: That host is playing Nightfall, not Survivors.`

- [ ] **Step 10: Commit**

```bash
git add crates/benilla-app/src/nightfall
git commit -m "Nightfall: runs alone or in co-op, heroes drawn and kept inside Duskwood

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The night: time, fog, view distance, hero lights, the menu camera

**Files:**
- Create: `crates/benilla-app/src/nightfall/atmosphere.rs`
- Modify: `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes:
  - `FogOverride` (Task 7).
  - `Atmosphere`, `HeroLight` (Task 3).
  - `Views` (Task 9).
  - `GameMode` (Task 6).
- Produces:
  - `atmosphere::{night, follow_map, hero_lights, menu_camera, enter, leave}`.
  - The pure helpers `atmosphere::apply(&Atmosphere, &mut ViewDistance) -> (SavedView, FogOverride)` and `atmosphere::restore(&SavedView, &mut ViewDistance)`.
  - `enter` runs each frame in Nightfall and acts once; `leave` runs `OnExit(GameMode::Nightfall)`.

- [ ] **Step 1: Write the failing tests**

`crates/benilla-app/src/nightfall/atmosphere.rs`:

```rust
//! Nightfall's night: the light stands at midnight, the fog closes in at 40 yards, the world is
//! drawn only a little past it, and every hero carries a small warm light so a player sees their
//! feet. All of it comes from `content/nightfall/rules.ron` and is undone when the player goes
//! back to the pick screen, so Survivors is never left in the dark.

use benilla_world::lighting::{FogOverride, WorldPointLight};
use benilla_world::view::{ViewDistance, WorldCamera};
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use nightfall_sim::Atmosphere;

use super::view::Views;
use super::{content, units};

/// The view distance before Nightfall changed it, to give back on leaving.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(super) struct SavedView {
    farclip: f32,
}

/// A hero that already carries its light.
#[derive(Component)]
pub(super) struct HeroLit;

#[cfg(test)]
mod tests {
    use super::*;

    fn atmosphere() -> Atmosphere {
        Atmosphere {
            night_minute: 0,
            fog_start: 10.0,
            fog_end: 40.0,
            fog_color: (0.04, 0.07, 0.08),
            view_distance: 120.0,
            hero_light: nightfall_sim::HeroLight {
                color: (1.0, 0.8, 0.6),
                intensity: 0.6,
                range: 9.0,
            },
        }
    }

    #[test]
    fn entering_sets_the_fog_and_view_and_leaving_gives_the_view_back() {
        let mut view = ViewDistance {
            farclip: 350.0,
            nearclip: 0.2,
        };
        let (saved, fog) = apply(&atmosphere(), &mut view);
        assert_eq!(view.farclip, 120.0);
        assert_eq!(
            fog,
            FogOverride {
                start: 10.0,
                end: 40.0,
                color: [0.04, 0.07, 0.08]
            }
        );
        restore(&saved, &mut view);
        assert_eq!(view.farclip, 350.0);
        assert_eq!(view.nearclip, 0.2);
    }
}
```

Add `mod atmosphere;` in `nightfall/mod.rs`.
Run: `cargo test -p benilla-app --lib nightfall::atmosphere`
Expected: FAIL to compile with `cannot find function 'apply'`.

- [ ] **Step 2: Implement the atmosphere**

Add above the tests:

```rust
/// Nightfall's view distance and fog, from `a`; what to give back on leaving.
pub(super) fn apply(a: &Atmosphere, view: &mut ViewDistance) -> (SavedView, FogOverride) {
    let saved = SavedView {
        farclip: view.farclip,
    };
    view.farclip = a.view_distance;
    let fog = FogOverride {
        start: a.fog_start,
        end: a.fog_end,
        color: [a.fog_color.0, a.fog_color.1, a.fog_color.2],
    };
    (saved, fog)
}

/// The view distance as it was before Nightfall.
pub(super) fn restore(saved: &SavedView, view: &mut ViewDistance) {
    view.farclip = saved.farclip;
}

/// In Nightfall: the fog and the short view, set once (until [`leave`] gives them back). A frame
/// system rather than `OnEnter`, because a launch straight into Nightfall enters before the engine
/// has made its `ViewDistance`.
pub(super) fn enter(
    mut commands: Commands,
    saved: Option<Res<SavedView>>,
    view: Option<ResMut<ViewDistance>>,
) {
    if saved.is_some() {
        return;
    }
    let Some(mut view) = view else { return };
    let (saved, fog) = apply(&content().rules.atmosphere, &mut view);
    commands.insert_resource(saved);
    commands.insert_resource(fog);
}

/// Leaving Nightfall: the engine's own fog and view distance again.
pub(super) fn leave(
    mut commands: Commands,
    saved: Option<Res<SavedView>>,
    view: Option<ResMut<ViewDistance>>,
) {
    if let (Some(saved), Some(mut view)) = (saved, view) {
        restore(&saved, &mut view);
    }
    commands.remove_resource::<SavedView>();
    commands.remove_resource::<FogOverride>();
}

/// The light stands at the night's minute, whatever the clock says.
pub(super) fn night(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    let minute = content().rules.atmosphere.night_minute;
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = minute;
    }
}

/// The world stands on Nightfall's map.
pub(super) fn follow_map(current: Option<ResMut<benilla_world::world_map::CurrentMap>>) {
    let Some(mut current) = current else { return };
    let want = content().map.map;
    if current.0 != want {
        current.0 = want;
    }
}

/// Each hero carries a small warm light, 1.6 yards up (about the chest).
pub(super) fn hero_lights(
    mut commands: Commands,
    views: Res<Views>,
    lit: Query<(), With<HeroLit>>,
) {
    let l = &content().rules.atmosphere.hero_light;
    for &e in views.heroes.values() {
        if lit.get(e).is_ok() {
            continue;
        }
        commands.entity(e).insert(HeroLit).with_children(|c| {
            c.spawn((
                WorldPointLight {
                    color: [l.color.0, l.color.1, l.color.2],
                    // `4π × intensity`, the point-light convention the packer undoes.
                    intensity: 4.0 * std::f32::consts::PI * l.intensity,
                    range: l.range,
                },
                Transform::from_xyz(0.0, 1.6, 0.0),
                Visibility::default(),
            ));
        });
    }
}

/// At the menu: a slow, wide orbit over the first entry, so the dark woods stand behind it and
/// the terrain streams there.
pub(super) fn menu_camera(
    time: Res<Time<Real>>,
    world: WorldPoint,
    mut player: ResMut<crate::player::Player>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
    mut orbit: Local<f32>,
) {
    let mut focus = units::wow_to_bevy(content().map.entries[0]);
    if let Some(h) = world.terrain_height_under(focus) {
        focus.y = h;
    }
    if !player.active {
        player.pos = focus;
    }
    *orbit += time.delta_secs() * 0.05;
    let d = 14.0;
    let eye = focus + Vec3::new(orbit.sin() * d, 4.0, orbit.cos() * d);
    let placed = Transform::from_translation(eye).looking_at(focus + Vec3::Y * 1.5, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}
```

- [ ] **Step 3: Register it**

In `NightfallPlugin::build` add:

```rust
            .add_systems(OnExit(GameMode::Nightfall), atmosphere::leave)
            .add_systems(
                Update,
                (
                    atmosphere::enter,
                    atmosphere::night,
                    atmosphere::follow_map,
                    atmosphere::hero_lights,
                )
                    .in_set(NightfallSet),
            )
            .add_systems(
                Update,
                atmosphere::menu_camera
                    .in_set(benilla_world::schedule::WorldStage::Input)
                    .after(crate::player::PlayerControlSet)
                    .in_set(NightfallSet)
                    .run_if(in_state(Phase::Menu)),
            )
```

`enter` runs every frame in Nightfall but acts once: `SavedView` marks it done, and `leave` removes it, so the next visit to Nightfall applies the night again.

- [ ] **Step 4: Run the tests and build**

Run: `cargo test -p benilla-app --lib nightfall`
Expected: PASS.
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 5: See the night, and Survivors after it**

Run: `WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_AUTOPLAY=1 WOW_LIVE_SHOT=target/night.png WOW_LIVE_SHOT_AT=40 ./target/release/azeroth-together.exe`
Expected: a dark Duskwood with the fog closing in at about 40 yards and a warm glow at the hero.

The way back (Nightfall → Back → Survivors in daylight, no fog, full view) is pinned by `entering_sets_the_fog_and_view_and_leaving_gives_the_view_back`. It needs clicks to see in the game, so list it in the hand-off for the user to try. Do not open a normal window yourself.

- [ ] **Step 6: Commit**

```bash
git add crates/benilla-app/src/nightfall
git commit -m "Nightfall: midnight, 40-yard fog, a short view and a light at each hero

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The night grade

**Files:**
- Create: `crates/benilla-app/src/nightfall/grade.rs`, `crates/benilla-app/src/shaders/nightfall_grade.wgsl`
- Modify: `crates/benilla-app/src/shaders.rs` (register the shader; update "five" in its doc comment), `crates/benilla-app/src/nightfall/mod.rs`

**Interfaces:**
- Consumes: `GameMode` (Task 6).
- Produces: `grade::plugin(&mut App)`. While Nightfall is up, the world camera carries `NightGrade`.

- [ ] **Step 1: Write the shader**

`crates/benilla-app/src/shaders/nightfall_grade.wgsl`:

```wgsl
// Nightfall's grade: the world a little drained of colour, cooled toward moonlight, darkened, with
// a vignette that closes the corners in. It runs on the world camera before the HUD, which draws
// over it untouched. `a.x` is the master amount: 0 is the identity.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Params {
    // x amount, y desaturation 0..1, z vignette strength, w darkening 0..1.
    a: vec4<f32>,
    // xyz tint multiplier, w unused.
    b: vec4<f32>,
}
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSample(screen_texture, screen_sampler, in.uv);
    let amount = params.a.x;
    if (amount <= 0.0) {
        return src;
    }
    let l = dot(src.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    var c = mix(src.rgb, vec3<f32>(l), params.a.y) * params.b.xyz * (1.0 - params.a.w);
    let d = distance(in.uv, vec2<f32>(0.5, 0.5)) * 1.41421356;
    c = c * (1.0 - params.a.z * smoothstep(0.35, 1.0, d));
    return vec4<f32>(mix(src.rgb, c, amount), src.a);
}
```

In `crates/benilla-app/src/shaders.rs` add `bevy::asset::embedded_asset!(app, "shaders/nightfall_grade.wgsl");` after the survivors line. Change "five" to "eight" in the two doc comments, which counts the files in `src/shaders/`. The existing test `every_game_shader_answers_at_its_embedded_path` reads every `.wgsl` in the directory back from its embedded path, so it covers the new one.

- [ ] **Step 2: Write the grade**

`crates/benilla-app/src/nightfall/grade.rs`:

```rust
//! Nightfall's grade on the world camera (`shaders/nightfall_grade.wgsl`): Bevy's
//! [`FullscreenMaterial`], placed where the survivors grade sits, between FFXGlow and the end of
//! the world's post-processing. It is on the camera only while Nightfall is up.

use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{
    InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph,
};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

use benilla_world::view::WorldCamera;

use crate::game_pick::GameMode;

/// The grade's uniform: eight floats, the shader's two `vec4`s.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub(super) struct NightGrade {
    pub amount: f32,
    pub desaturate: f32,
    pub vignette: f32,
    pub darken: f32,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    pub unused: f32,
}

impl Default for NightGrade {
    fn default() -> Self {
        Self {
            amount: 1.0,
            desaturate: 0.35,
            vignette: 0.45,
            darken: 0.1,
            tint_r: 0.85,
            tint_g: 0.95,
            tint_b: 1.12,
            unused: 0.0,
        }
    }
}

impl FullscreenMaterial for NightGrade {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/shaders/nightfall_grade.wgsl".into()
    }

    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }

    /// After the glow's filter passes, before the world's post-processing ends.
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(FullscreenMaterialPlugin::<NightGrade>::default())
        .add_systems(Update, attach.run_if(in_state(GameMode::Nightfall)))
        .add_systems(OnExit(GameMode::Nightfall), detach);
}

fn attach(
    mut commands: Commands,
    cams: Query<Entity, (With<WorldCamera>, With<Camera3d>, Without<NightGrade>)>,
) {
    for e in &cams {
        commands.entity(e).insert(NightGrade::default());
    }
}

fn detach(mut commands: Commands, cams: Query<Entity, With<NightGrade>>) {
    for e in &cams {
        commands.entity(e).remove::<NightGrade>();
    }
}
```

In `nightfall/mod.rs` add `mod grade;`, and `.add_plugins(grade::plugin)` in `NightfallPlugin::build`.

- [ ] **Step 3: Run the shader test and build**

Run: `cargo test -p benilla-app --lib shaders`
Expected: PASS (the new file answers at its embedded path).
Run: `cargo build --release -p azeroth-together`
Expected: builds.

- [ ] **Step 4: See it**

Run: `WOW_BG=1 WOW_WIN=1280x720 NIGHTFALL_AUTOPLAY=1 WOW_LIVE_SHOT=target/graded.png WOW_LIVE_SHOT_AT=40 ./target/release/azeroth-together.exe`
Expected: compared to `target/night.png` from Task 10, colder and less saturated, with darker corners; the WoW frames and the Leave Run button are not graded.

- [ ] **Step 5: Commit**

```bash
git add crates/benilla-app/src/nightfall crates/benilla-app/src/shaders.rs crates/benilla-app/src/shaders/nightfall_grade.wgsl
git commit -m "Nightfall: a cold, desaturated night grade with a vignette

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Docs and the full check

**Files:**
- Modify: `CLAUDE.md` (a Nightfall section), `docs/superpowers/specs/2026-10-16-nightfall-design.md` (status line), `scripts/package.sh` (only if it copies `content/` selectively)

- [ ] **Step 1: Check packaging takes the new content**

Read `scripts/package.sh`. If it copies `content/` whole, nothing changes. If it lists files, add `content/nightfall/` to what it copies.

- [ ] **Step 2: Document Nightfall in `CLAUDE.md`**

Under "Where things live", after the `crates/together-sim/` entry, add:

```markdown
- `crates/nightfall-sim/` — Nightfall's rules: a second game, a co-op stealth extraction game in
  Duskwood at night (`docs/superpowers/specs/2026-10-16-nightfall-design.md`). Headless, 30 Hz, no
  Bevy and no `together-sim`: heroes, poses kept inside the map's bounds, content
  (`content/nightfall/*.ron`), its wire messages. Its client is `crates/benilla-app/src/nightfall/`,
  which copies what it needs from `together/` instead of sharing it.
- The launcher's first screen (`benilla-app/src/game_pick.rs`) picks Survivors or Nightfall; every
  Survivors system is in `together::SurvivorsSet`, gated on `GameMode::Survivors`.
- `together-net` is generic over each game's messages (`Host<Up, Down>`, `Client<Up, Down>`) and
  starts every connection with a handshake naming the game; Survivors' messages live in
  `together_sim::messages`.
```

Under "Build, test, run", add:

```markdown
- Nightfall: `NIGHTFALL=1` skips the pick screen into Nightfall (any `TOGETHER_*` variable skips it
  into Survivors). `NIGHTFALL_CLASS` (`rogue`, `mage`), `NIGHTFALL_NAME`, `NIGHTFALL_CONTENT`,
  `NIGHTFALL_AUTOPLAY=1` (starts a run and walks a circle), `NIGHTFALL_TRACE_POS=1` (logs the
  hero's WoW position every 2 s, for measuring entry points), and co-op with `NIGHTFALL_HOST=1` /
  `NIGHTFALL_JOIN=<code or address>`. `cargo test -p nightfall-sim` runs its rules.
```

- [ ] **Step 3: Mark the spec**

In the spec's status line (`Status: approved; revised the same day …`), add after its first sentence: `Sub-project 1 built (plan docs/superpowers/plans/2026-10-16-nightfall-foundation.md).`

- [ ] **Step 4: Run every test this work touches**

Run: `cargo test -p together-net -p together-sim -p nightfall-sim`
Expected: PASS.
Run: `cargo test -p benilla-world --lib fog_override`
Expected: PASS.
Run: `cargo test -p benilla-app --lib game_pick together::menu nightfall shaders`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-10-16-nightfall-design.md scripts/package.sh
git commit -m "Docs: Nightfall's crates, the pick screen and its test hooks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
