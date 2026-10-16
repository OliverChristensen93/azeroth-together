//! Azeroth Together: the co-op survivors mode, started by [`crate::run_together`].
//!
//! The game itself is [`together_sim::World`], a headless simulation that knows nothing of Bevy.
//! This module steps it at its fixed rate and draws it: every hero, enemy and gem gets a WoW unit
//! or object ([`view`]), and the events a step emits become spell visuals, swings and numbers.
//!
//! The rest is the real WoW 1.12 client: its interface (player and target frames, minimap, buffs,
//! key bindings) and its character controller (mouselook, strafing, jumping, the camera). The
//! controller moves the local hero, and [`feed_pose`] reports where it went, the way a WoW client
//! reports its moves to the server. A co-op client will draw a host's world the same way, which is
//! why nothing here decides anything about the game.
//!
//! Configuration, for now: `TOGETHER_CLASS` (default `mage`), `TOGETHER_ZONE` (default `elwynn`),
//! `TOGETHER_NAME` (the hero's name), `TOGETHER_CONTENT` (a folder with the RON files; default
//! the repository's `content/`, and the copies compiled in when that is missing).

mod actions;
mod armory;
mod dev;
mod dress;
mod gear_look;
mod gear_menu;
mod gear_run;
mod gear_tip;
mod hazards;
mod hud;
mod inventory;
mod runpanel;
mod levelup;
mod lobby;
mod menu;
mod meter;
mod party;
mod session;
mod talents;
mod tooltip;
mod ui;
mod units;
mod view;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;
use together_sim::{
    Content, ContentSources, Event, HeroPose, PlayerId, PlayerInput, Snapshot, World, DT, V2,
};

/// Set by [`crate::run_together`] before the app builds.
static ACTIVE: AtomicBool = AtomicBool::new(false);
/// The zone's `Map.dbc` id, read before the world map plugin starts.
static START_MAP: AtomicU32 = AtomicU32::new(0);
static CONTENT: OnceLock<Arc<Content>> = OnceLock::new();

pub(crate) fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

pub(crate) fn start_map() -> u32 {
    START_MAP.load(Ordering::Relaxed)
}

/// The content files' text, for the content hash a co-op client and host compare: classes,
/// abilities, enemies, and the zones with the rules and the talent web after them.
static CONTENT_TEXTS: OnceLock<(String, String, String, String)> = OnceLock::new();

pub(super) fn content_texts() -> &'static (String, String, String, String) {
    CONTENT_TEXTS.get().expect("together::prepare ran")
}

fn class_key() -> String {
    std::env::var("TOGETHER_CLASS").unwrap_or_else(|_| "mage".into())
}

fn zone_key() -> String {
    std::env::var("TOGETHER_ZONE").unwrap_or_else(|_| "elwynn".into())
}

/// The folder the RON files are read from, if one exists on disk.
fn content_dir(project_dir: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("TOGETHER_CONTENT").map(PathBuf::from),
        Some(Path::new(project_dir).join("content")),
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("content"))),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|d| d.join("zones.ron").is_file())
}

/// Read the content (from disk when there, so it can be edited without a rebuild) and remember
/// the zone's map; called once by [`crate::run_together`]. Bad content is reported and exits.
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
        include_str!("../../../../content/classes.ron"),
    )?;
    let abilities = read(
        "abilities.ron",
        include_str!("../../../../content/abilities.ron"),
    )?;
    let enemies = read(
        "enemies.ron",
        include_str!("../../../../content/enemies.ron"),
    )?;
    let zones = read("zones.ron", include_str!("../../../../content/zones.ron"))?;
    let rules = read("rules.ron", include_str!("../../../../content/rules.ron"))?;
    let talent_web = read(
        "talents.ron",
        include_str!("../../../../content/talents.ron"),
    )?;
    talents::set_web(together_sim::TalentWeb::from_ron(&talent_web)?);
    let content = Content::from_ron(ContentSources {
        classes: &classes,
        abilities: &abilities,
        enemies: &enemies,
        zones: &zones,
    })
    .and_then(|c| c.with_rules(&rules))
    .map_err(|e| e.to_string())?;
    let zone = content
        .zone(&zone_key())
        .ok_or_else(|| format!("TOGETHER_ZONE: no zone \"{}\" in zones.ron", zone_key()))?;
    if content.class(&class_key()).is_none() {
        return Err(format!(
            "TOGETHER_CLASS: no class \"{}\" in classes.ron",
            class_key()
        ));
    }
    match &dir {
        Some(d) => println!("together: content from {}", d.display()),
        None => println!("together: content compiled in"),
    }
    START_MAP.store(content.zones[zone].map, Ordering::Relaxed);
    let _ = CONTENT.set(Arc::new(content));
    let _ = CONTENT_TEXTS.set((
        classes,
        abilities,
        enemies,
        format!("{zones}\n{rules}\n{talent_web}"),
    ));
    ACTIVE.store(true, Ordering::Relaxed);
    Ok(())
}

/// The game's content, as [`prepare`] loaded it.
pub(super) fn content() -> Arc<Content> {
    CONTENT.get().expect("together::prepare ran").clone()
}

/// The screen the game is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    /// The main menu: pick a class, a look and a zone.
    #[default]
    Menu,
    /// A run.
    Playing,
}

/// The run being played, and the clock that steps it. Present only during a run.
#[derive(Resource)]
pub(super) struct Sim {
    /// The world: stepped here when playing alone or hosting, a mirror of the host's when joined.
    pub world: World,
    /// Our player: 0 alone or hosting, the host's pick when joined.
    pub me: PlayerId,
    /// Solo or host: real seconds not yet simulated, always under one [`DT`] after a frame's
    /// steps. Joined: playback seconds since the last snapshot was applied.
    acc: f32,
    /// Seconds between the states the views interpolate: a step, or a host's snapshot.
    span: f32,
    /// Joined: the host's snapshots not yet played, oldest first.
    inbox: VecDeque<(Snapshot, Vec<Event>)>,
    /// Joined: playback has begun (the inbox first fills to [`PLAYBACK_DEPTH`]).
    playing: bool,
    /// Steps taken (or arrived) this frame, so the views roll their interpolation forward.
    pub stepped: bool,
}

/// Snapshots a client holds in hand before playing them: the delay that hides a late packet.
const PLAYBACK_DEPTH: usize = 2;
/// More than this waiting (a burst after a stall) and the oldest are played at once.
const PLAYBACK_MAX: usize = 8;

impl Sim {
    /// A run for everyone in `roster` (just us, alone), stepped here, with the dev panel's test
    /// settings applied.
    fn hosted(zone: &str, roster: &[together_net::LobbyPlayer], dev: &dev::DevTuning) -> Self {
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mut world = World::new(content(), zone, seed).expect("zone from the content");
        world.xp_rate = xp_rate();
        for p in roster {
            // A class the content lacks (a mismatched client) is left out rather than crash.
            let _ = world.add_hero_as(
                p.player,
                &p.hero.class,
                &p.hero.name,
                p.hero.race,
                p.hero.sex,
            );
            // Each hero's own talents, as far as they connect and its points go.
            let taken: Vec<usize> = p.hero.talents.iter().map(|&n| usize::from(n)).collect();
            let took = world.apply_talents(p.player, &talents::web(), &taken);
            println!("together: {} takes {took} talent node(s)", p.hero.name);
            // The gear each player brings, dormant until they pay for it.
            world.equip(p.player, &p.hero.gear);
        }
        give_loadout(&mut world);
        dev.apply(&mut world);
        // `TOGETHER_AFFIXES=sanguine,volcanic`: a test run with these affixes from the start.
        if let Ok(list) = std::env::var("TOGETHER_AFFIXES") {
            for affix in list.split(',').filter_map(together_sim::Affix::from_name) {
                world.add_affix(affix);
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
        }
    }

    /// A joined run: an empty world in the host's zone, filled by the host's steps.
    fn mirror(zone: &str, me: PlayerId) -> Option<Self> {
        Some(Self {
            world: World::new(content(), zone, 0).ok()?,
            me,
            acc: 0.0,
            span: DT * session::SEND_EVERY as f32,
            inbox: VecDeque::new(),
            playing: false,
            stepped: false,
        })
    }

    /// How far between the last two states the present lies, 0..1, for smooth movement.
    pub fn alpha(&self) -> f32 {
        (self.acc / self.span).clamp(0.0, 1.0)
    }

    /// Joined: the host's snapshots into the mirror, one per [`Self::span`] of playback, played a
    /// little behind so they come out evenly however the network delivered them. A fuller inbox
    /// plays a touch faster and an emptier one slower, which keeps the delay near
    /// [`PLAYBACK_DEPTH`]; an empty one holds the last state.
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
        if let Some((snapshot, events)) = self.inbox.pop_front() {
            self.world.apply_snapshot(snapshot, events);
            self.stepped = true;
        }
    }
}

/// Everything a run spawned, despawned when it ends.
#[derive(Component)]
pub(super) struct TogetherEntity;

/// What the local player sends the next step: the hero's pose as the controller left it, and a
/// card pick, reroll or skip, each used once.
#[derive(Resource, Default)]
pub(super) struct LocalInput {
    pose: Option<HeroPose>,
    pub(super) pick: Option<u8>,
    pub(super) reroll: bool,
    pub(super) skip: bool,
    /// The defensive button was pressed this frame ([`defend`]).
    pub(super) dash: bool,
    /// Freeze or unfreeze the level-up hand; banish the spell on a card.
    pub(super) freeze: bool,
    pub(super) banish: Option<u8>,
    /// Switch on the dormant gear piece in this slot ([`gear_run`]).
    pub(super) gear_on: Option<u8>,
}

/// Play Solo was clicked: [`start_run`] begins the run.
#[derive(Resource, Default)]
pub(super) struct StartRequest(pub bool);

pub(crate) struct TogetherPlugin;

impl Plugin for TogetherPlugin {
    fn build(&self, app: &mut App) {
        let playing = || in_state(Phase::Playing).and(resource_exists::<Sim>);
        app.init_state::<Phase>()
            .insert_resource(menu::Selection::new(&content()))
            .init_resource::<StartRequest>()
            .init_resource::<EndRun>()
            .init_resource::<session::Net>()
            .init_resource::<lobby::JoinDialog>()
            .init_resource::<dev::DevSettings>()
            .init_resource::<inventory::SheetOpen>()
            .init_resource::<hazards::Pending>()
            .init_resource::<levelup::BanishMode>()
            .init_resource::<hazards::Telegraphs>()
            .add_plugins(talents::TalentsPlugin)
            .init_resource::<armory::GearStore>()
            .init_resource::<gear_menu::GearScreen>()
            .init_resource::<LocalInput>()
            .init_resource::<view::Views>()
            .add_systems(
                Startup,
                (
                    hud::setup,
                    levelup::setup,
                    meter::setup,
                    tooltip::setup,
                    inventory::setup,
                    hazards::setup,
                    runpanel::setup,
                    gear_tip::setup,
                    menu::spawn_root,
                ),
            )
            .add_systems(
                Update,
                (
                    daylight,
                    follow_map,
                    menu::apply_ui_scale,
                    menu::show_menu,
                    menu::fill_icons,
                    levelup::fill_texture_icons,
                    hud::show_hud,
                    levelup::refresh,
                    levelup::clicks,
                    meter::refresh,
                    ui::tune_ui,
                    session::close_on_exit,
                    tooltip::show_tip,
                    inventory::refresh,
                    hazards::boss_bar,
                    hazards::grow_fills,
                    runpanel::refresh,
                    party::feed.before(crate::ui_script::UiFeed),
                ),
            )
            // Equipment: the stash on disk, icons, tooltips and the run's gear bar.
            .add_systems(
                Update,
                (
                    armory::collect_found,
                    gear_run::setup,
                    gear_run::refresh,
                    gear_look::fill_gear_icons,
                    gear_tip::show,
                ),
            )
            // The menu.
            .add_systems(
                Update,
                (
                    menu::restore_pick,
                    menu::build_menu,
                    menu::menu_clicks,
                    lobby::type_address,
                    lobby::clicks,
                    lobby::refresh.before(crate::glue::GlueVisuals),
                    dev::clicks,
                    dev::refresh.before(crate::glue::GlueVisuals),
                    gear_menu::build,
                    gear_menu::clicks,
                    gear_menu::refresh.before(crate::glue::GlueVisuals),
                    session::lobby,
                    menu::refresh_menu_ui.before(crate::glue::GlueVisuals),
                    menu::preview_hero,
                    gear_look::dress_preview,
                    start_run,
                )
                    .chain()
                    .run_if(in_state(Phase::Menu)),
            )
            .add_systems(
                Update,
                menu::menu_camera
                    .in_set(WorldStage::Input)
                    .after(crate::player::PlayerControlSet)
                    .run_if(in_state(Phase::Menu)),
            )
            .add_systems(OnExit(Phase::Menu), menu::clear_preview)
            .add_systems(OnEnter(Phase::Menu), hazards::clear)
            // The run.
            .add_systems(
                Update,
                (
                    read_keys,
                    defend,
                    feed_pose,
                    step_sim,
                    view::settle_shrines,
                    view::apply_events,
                    hazards::show,
                    view::sync,
                    view::write_vitals,
                    hazards::status_auras,
                    gear_look::dress_heroes,
                    view::place,
                )
                    .chain()
                    .in_set(WorldStage::Input)
                    // The controller has moved the hero this frame before its pose is read.
                    .after(crate::player::PlayerControlSet)
                    .run_if(playing()),
            )
            .add_systems(
                Update,
                (
                    actions::fill_action_bar,
                    actions::drop_manual_casts,
                    hud::refresh,
                    hud::prompt,
                    back_to_menu,
                )
                    .chain()
                    .after(WorldStage::Input)
                    .run_if(playing()),
            );
    }
}

/// The world stands in the zone being played, or last played: picking another at the menu
/// changes nothing until Play, and entering one on another continent raises the loading screen.
fn follow_map(
    selection: Res<menu::Selection>,
    current: Option<ResMut<benilla_world::world_map::CurrentMap>>,
) {
    let Some(mut current) = current else {
        return;
    };
    let want = content().zones[selection.stage].map;
    if current.0 != want {
        current.0 = want;
    }
}

/// Play Solo, or the host's Start Run: a fresh run in the picked zone for us and everyone who
/// joined, the controller taking our new hero. Joined: our mirror of the run the host started.
#[allow(clippy::too_many_arguments)]
fn start_run(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut start: ResMut<StartRequest>,
    mut selection: ResMut<menu::Selection>,
    mut player: ResMut<crate::player::Player>,
    mut next: ResMut<NextState<Phase>>,
    mut net: ResMut<session::Net>,
    dev: Res<dev::DevSettings>,
    talents: Res<talents::Talents>,
    gear: Res<armory::GearStore>,
) {
    let content = content();
    let mut begin = |commands: &mut Commands, sim: Sim| {
        commands.insert_resource(sim);
        player.active = false;
        next.set(Phase::Playing);
    };
    // Test hooks: `TOGETHER_HOST` hosts and `TOGETHER_JOIN=<address>` joins once the menu is up.
    if net.is_solo() && net.status.is_empty() && time.elapsed_secs() > 8.0 {
        if std::env::var_os("TOGETHER_HOST").is_some() {
            net.host();
        } else if let Ok(addr) = std::env::var("TOGETHER_JOIN") {
            net.join(&addr);
        }
    }
    if let session::Role::Client(c) = &mut net.role {
        let (Some(zone), Some(me)) = (c.start.take(), c.me) else {
            return;
        };
        let Some(zone_ix) = content.zone(&zone) else {
            return;
        };
        // The world goes to the host's zone.
        selection.stage = zone_ix;
        if let Some(sim) = Sim::mirror(&zone, me) {
            begin(&mut commands, sim);
        }
        return;
    }
    // `TOGETHER_AUTOPLAY` plays on its own once the menu has stood a few seconds; hosting, once
    // someone has joined.
    let auto = autoplay()
        && match &net.role {
            session::Role::Solo => {
                time.elapsed_secs() > 12.0 && std::env::var_os("TOGETHER_HOST").is_none()
            }
            session::Role::Host(h) => !h.peers.is_empty() && time.elapsed_secs() > 20.0,
            session::Role::Client(_) => false,
        };
    if !std::mem::take(&mut start.0) && !auto {
        return;
    }
    let Some(own) = session::own_pick(&selection, &talents, &gear) else {
        return;
    };
    selection.stage = selection.zone;
    let zone = content.zones[selection.zone].key.clone();
    let roster = net.roster(&own);
    net.broadcast_start(&zone);
    let mut sim = Sim::hosted(&zone, &roster, &dev.tuning);
    showcase_for_tests(&mut sim.world);
    armory::gear_on_for_tests(&mut sim.world);
    begin(&mut commands, sim);
}

/// `TOGETHER_SHOWCASE=1`: one interactable of each kind moves onto a ring around the start, one
/// of each power-up lies on a ring inside it, an elite camp stands 25 yards off, and the boss
/// comes 8 seconds in, so a short test run sees them all.
fn showcase_for_tests(world: &mut World) {
    if std::env::var_os("TOGETHER_SHOWCASE").is_none() {
        return;
    }
    world.next_boss = 8.0;
    world.move_camp(0, world.center() + V2::new(25.0, 0.0));
    let ups = together_sim::PowerUp::ALL;
    for (k, p) in ups.into_iter().enumerate() {
        let angle = (k as f32 + 0.5) / ups.len() as f32 * std::f32::consts::TAU;
        let at = world.center() + V2::from_angle(angle, 5.0);
        world.drop_pickup(at, together_sim::PickupKind::PowerUp(p));
    }
    let center = world.center();
    let mut kinds = Vec::new();
    let firsts: Vec<u32> = world
        .interactables
        .iter()
        .filter(|i| {
            let new = !kinds.contains(&i.kind);
            kinds.push(i.kind);
            new
        })
        .map(|i| i.id)
        .collect();
    for (k, &id) in firsts.iter().enumerate() {
        let angle = k as f32 / firsts.len() as f32 * std::f32::consts::TAU;
        world.move_interactable(id, center + V2::from_angle(angle, 10.0));
    }
}

/// Midday, whatever the clock says.
fn daylight(debug: Option<ResMut<benilla_world::dev_state::DebugState>>) {
    const MINUTE_OF_DAY: u32 = 780;
    let Some(mut debug) = debug else { return };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != MINUTE_OF_DAY {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = MINUTE_OF_DAY;
    }
}

/// `TOGETHER_AUTOPLAY=1`: the local hero takes the first card and walks to the shrines and
/// chests ([`autowalk`]), so a test run plays on without anyone at the keys.
fn autoplay() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TOGETHER_AUTOPLAY").is_some())
}

/// `TOGETHER_FRAIL=<player>`: that hero drops to 1 health 15 seconds in, once, so a co-op test
/// sees a fall and a revive.
fn frail_for_tests(world: &mut World) {
    static WHO: OnceLock<Option<PlayerId>> = OnceLock::new();
    let Some(who) = *WHO.get_or_init(|| std::env::var("TOGETHER_FRAIL").ok()?.parse().ok()) else {
        return;
    };
    let due = world.elapsed >= 15.0 && world.elapsed < 15.0 + DT * 1.5;
    if let Some(h) = world.heroes.iter_mut().find(|h| h.player == who && h.alive) {
        if due {
            h.hp = 1.0;
        }
    }
}

/// While the cards are up: 1, 2 or 3 picks one, R rerolls. Everything else (movement, jumping,
/// the camera, the panels) is WoW's own key bindings.
fn read_keys(
    keys: Res<ButtonInput<KeyCode>>,
    real: Res<Time<Real>>,
    sim: Res<Sim>,
    mut input: ResMut<LocalInput>,
    mut banishing: ResMut<levelup::BanishMode>,
    mut up_since: Local<Option<f32>>,
) {
    let choosing = sim
        .world
        .hero(sim.me)
        .is_some_and(|h| h.alive && !h.offers.is_empty());
    if !choosing {
        *up_since = None;
        return;
    }
    if autoplay() {
        // Long enough for a test shot to see the cards.
        let since = *up_since.get_or_insert(real.elapsed_secs());
        if real.elapsed_secs() - since > 3.0 {
            input.pick = Some(0);
            *up_since = None;
        }
        return;
    }
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(key) {
            // In banish mode a card's key banishes its spell instead.
            if std::mem::take(&mut banishing.0) {
                input.banish = Some(i as u8);
            } else {
                input.pick = Some(i as u8);
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        input.reroll = true;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        input.freeze = true;
    }
    if keys.just_pressed(KeyCode::KeyG) {
        banishing.0 = !banishing.0;
    }
}

/// `TOGETHER_XP_RATE=<n>`: experience counts `n` times, so a test run reaches the augment ranks
/// in minutes. 1 when unset.
fn xp_rate() -> f32 {
    std::env::var("TOGETHER_XP_RATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&r: &f32| r > 0.0)
        .unwrap_or(1.0)
}

/// `TOGETHER_LOADOUT=frostbolt@4,fireball@9+frostfire,ice_barrier@10+fortified+shatteringbarrier`:
/// the host's own hero starts with these spells at these ranks and augments, in place of its
/// first spell, so a test run sees augment screens and capstones in minutes.
fn give_loadout(world: &mut World) {
    let Ok(list) = std::env::var("TOGETHER_LOADOUT") else {
        return;
    };
    world.heroes.iter_mut().filter(|h| h.player == 0).for_each(|h| h.abilities.clear());
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let mut parts = entry.split('+');
        let head = parts.next().unwrap_or_default();
        let (key, rank) = head.split_once('@').unwrap_or((head, "1"));
        let augments: Vec<together_sim::Augment> = parts
            .filter_map(together_sim::Augment::from_name)
            .collect();
        if !world.give_spell(0, key, rank.parse().unwrap_or(1), &augments) {
            println!("together: TOGETHER_LOADOUT: could not give \"{entry}\"");
        }
    }
}

/// Seconds a press of the defensive button waits for the world to answer before another counts.
const DEFEND_GRACE: f32 = 0.5;

/// The defensive button, F: the ability the hero's class names as its `defensive` in
/// `content/classes.ron` (the mage's Blink, the warrior's Charge, ...). The world owns its
/// cooldown and its effect, and its visual comes with the world's cast. A movement one is made
/// here first, since WoW's controller owns the hero's position: the leap happens before
/// [`feed_pose`] reads it, and the next input tells the simulation (`PlayerInput::dash`), which
/// acts on where the hero left and landed. `TOGETHER_AUTOPLAY` presses it whenever it is ready.
#[allow(clippy::too_many_arguments)]
fn defend(
    keys: Res<ButtonInput<KeyCode>>,
    real: Res<Time<Real>>,
    sim: Res<Sim>,
    views: Res<view::Views>,
    mut player: ResMut<crate::player::Player>,
    mut input: ResMut<LocalInput>,
    mut bodies: Query<&mut Transform>,
    world_point: benilla_world::world_point::WorldPoint,
    mut asked_until: Local<f32>,
) {
    let Some(hero) = sim.world.hero(sim.me) else {
        return;
    };
    if !hero.alive || !hero.offers.is_empty() || !player.active {
        return;
    }
    let content = sim.world.content();
    let Some(ability) = content.classes[hero.class].defensive_ix else {
        return;
    };
    let now = real.elapsed_secs();
    if hero.spell.fx.defense_cd > 0.0 || now < *asked_until {
        return;
    }
    if !(keys.just_pressed(KeyCode::KeyF) || autoplay()) {
        return;
    }
    *asked_until = now + DEFEND_GRACE;
    input.dash = true;
    let leap = content.abilities[ability].leap;
    if leap <= 0.0 {
        return;
    }
    let Some(&body) = views.heroes.get(&sim.me) else {
        return;
    };
    let Ok(mut tf) = bodies.get_mut(body) else {
        return;
    };
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    let to = bevy_to_ground(tf.translation) + V2::new(yaw.cos(), yaw.sin()) * leap;
    let mut at = units::ground_to_bevy(to, tf.translation.y);
    if let Some(h) = world_point.terrain_height_under(at) {
        at.y = h;
    }
    tf.translation = at;
    player.pos = at;
}

/// Bevy's position to WoW's ground plane.
fn bevy_to_ground(p: Vec3) -> V2 {
    V2::new(-p.z, -p.x)
}

/// `TOGETHER_AUTOPLAY`: where the local hero at `from` walks this frame: with a banner, to its
/// destination; else toward the nearest interactable it could use (a chest only once it can
/// pay), until it stands well inside the circle. `None` when there is nothing to walk to, or
/// cards are up, or when `TOGETHER_STAND` keeps the hero standing for a test shot.
fn autowalk(world: &World, me: PlayerId, from: V2, dt: f32) -> Option<V2> {
    if std::env::var_os("TOGETHER_STAND").is_some() {
        return None;
    }
    use together_sim::{InteractState, InteractableKind};
    let hero = world.hero(me).filter(|h| h.alive && h.offers.is_empty())?;
    let carrying = world.interactables.iter().find_map(|i| match i.state {
        InteractState::Escort { carrier, to, .. } if carrier == me => Some(to),
        _ => None,
    });
    let (goal, near) = match carrying {
        Some(to) => (to, 1.0),
        None => {
            let target = world
                .interactables
                .iter()
                .filter(|i| i.ready())
                .filter(|i| {
                    i.kind != InteractableKind::Chest
                        || hero.gold >= together_sim::chest_price(hero.chests)
                })
                .min_by(|a, b| a.pos.distance(from).total_cmp(&b.pos.distance(from)))?;
            (target.pos, target.kind.radius() * 0.4)
        }
    };
    let to = goal - from;
    let dist = to.length();
    if dist < near {
        return None;
    }
    Some(from + to.normalize_or_zero() * (hero.speed * dt).min(dist))
}

/// Where WoW's controller left the local hero, as the pose the next step takes. The arena's edge
/// is a wall: a hero past the leash is put back on it, so nobody runs off the end of the loaded
/// world. Until the controller has the hero, the terrain streams around the arena's centre.
#[allow(clippy::too_many_arguments)]
fn feed_pose(
    time: Res<Time>,
    world_point: benilla_world::world_point::WorldPoint,
    views: Res<view::Views>,
    sim: Res<Sim>,
    mut player: ResMut<crate::player::Player>,
    mut input: ResMut<LocalInput>,
    mut bodies: Query<&mut Transform>,
) {
    let center = sim.world.center();
    let Some(mut tf) = views
        .heroes
        .get(&sim.me)
        .and_then(|&e| bodies.get_mut(e).ok())
        .filter(|_| player.active)
    else {
        if !player.active {
            player.pos = units::wow_to_bevy(sim.world.zone().center);
        }
        input.pose = None;
        return;
    };
    let at = bevy_to_ground(tf.translation);
    let mut pos = at;
    if autoplay() {
        if let Some(to) = autowalk(&sim.world, sim.me, pos, time.delta_secs()) {
            pos = to;
        }
    }
    let leash = sim.world.zone().leash;
    let off = pos - center;
    if off.length() > leash {
        pos = center + off.normalize_or_zero() * leash;
    }
    if pos != at {
        let mut back = units::ground_to_bevy(pos, tf.translation.y);
        // Onto the ground there, so a hero put up a slope does not end up inside it.
        if let Some(h) = world_point.terrain_height_under(back) {
            back.y = back.y.max(h);
        }
        tf.translation = back;
        player.pos = back;
    }
    let yaw = tf.rotation.to_euler(EulerRot::YXZ).0;
    input.pose = Some(HeroPose {
        pos,
        facing: V2::new(yaw.cos(), yaw.sin()),
    });
}

/// The run's clock. Alone or hosting: the simulation's fixed steps for the real time that passed,
/// each sent to the clients when hosting; it waits while the loading screen is up and until the
/// controller has the hero, so a run never starts unseen. Joined: our input to the host, and the
/// host's steps into our mirror of its world.
#[allow(clippy::too_many_arguments)]
fn step_sim(
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut sim: ResMut<Sim>,
    mut input: ResMut<LocalInput>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut net: ResMut<session::Net>,
    mut end: ResMut<EndRun>,
) {
    sim.stepped = false;
    let sim = &mut *sim;
    if net.is_client() {
        // A key press is sent once.
        let held = PlayerInput {
            pick: input.pick.take(),
            reroll: std::mem::take(&mut input.reroll),
            skip: std::mem::take(&mut input.skip),
            dash: std::mem::take(&mut input.dash),
            freeze: std::mem::take(&mut input.freeze),
            banish: input.banish.take(),
            gear_on: input.gear_on.take(),
            pose: input.pose,
            ..Default::default()
        };
        let heard = session::client_exchange(&mut net, held);
        if session::client_left_run(&mut net, &heard) {
            end.0 = true;
        }
        for h in heard {
            if let session::Heard::State(snapshot, events) = h {
                sim.inbox.push_back((snapshot, events));
            }
        }
        sim.play_back(time.delta_secs());
    } else {
        session::host_receive(&mut net, &mut sim.world);
        frail_for_tests(&mut sim.world);
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
            // A key press picks once: the first step takes it.
            let held = PlayerInput {
                pick: input.pick.take(),
                reroll: std::mem::take(&mut input.reroll),
                skip: std::mem::take(&mut input.skip),
                dash: std::mem::take(&mut input.dash),
                freeze: std::mem::take(&mut input.freeze),
                banish: input.banish.take(),
                gear_on: input.gear_on.take(),
                pose: Some(pose),
                ..Default::default()
            };
            sim.world.set_input(sim.me, held);
            let from = sim.world.events().len();
            sim.world.step();
            session::host_send(&mut net, &sim.world, from);
            sim.stepped = true;
        }
    }
    // Cards up (or the run over): the world holds still, animations and the hero included.
    let hold = sim.world.paused() || sim.world.game_over();
    if hold && !virtual_time.is_paused() {
        virtual_time.pause();
    } else if !hold && virtual_time.is_paused() {
        virtual_time.unpause();
    }
}

/// A joined client's run ended from outside (the host ended it, or the connection went): the
/// next [`back_to_menu`] leaves it.
#[derive(Resource, Default)]
pub(super) struct EndRun(pub bool);

/// Back to the menu: alone or hosting, Enter after the run ends (a host takes its party back to
/// the lobby with it); joined, when the host ends the run or the connection goes. Everything the
/// run spawned goes, and the hero's controller lets go.
#[allow(clippy::too_many_arguments)]
fn back_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    sim: Res<Sim>,
    mut views: ResMut<view::Views>,
    mut index: ResMut<crate::net::GuidIndex>,
    mut player: ResMut<crate::player::Player>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<Phase>>,
    spawned: Query<Entity, With<TogetherEntity>>,
    real: Res<Time<Real>>,
    mut over_since: Local<Option<f32>>,
    mut net: ResMut<session::Net>,
    mut end: ResMut<EndRun>,
) {
    let ended_outside = std::mem::take(&mut end.0);
    if !ended_outside {
        if !sim.world.game_over() || net.is_client() {
            *over_since = None;
            return;
        }
        let since = *over_since.get_or_insert(real.elapsed_secs());
        // `TOGETHER_AUTOPLAY` goes back by itself after a few seconds of the results.
        let auto = autoplay() && real.elapsed_secs() - since > 4.0;
        if !keys.just_pressed(KeyCode::Enter) && !auto {
            return;
        }
        net.broadcast_ended();
    }
    *over_since = None;
    for e in &spawned {
        commands.entity(e).despawn();
    }
    let gone: std::collections::HashSet<Entity> = spawned.iter().collect();
    index.0.retain(|_, e| !gone.contains(e));
    *views = view::Views::default();
    commands.remove_resource::<Sim>();
    player.active = false;
    virtual_time.unpause();
    next.set(Phase::Menu);
}
