//! The night on screen and in the ears (spec §7.2, §8.1): the fog the night has now, eased in as
//! it thickens; the raid's weather through the engine's own rain; each stage's cue and its line
//! in the chat, and no timer anywhere; a line when an exit closes or a banshee's wail lands;
//! Stitches' tread from where he walks; and the next raid planned from the roll, with the test
//! hooks' weather and monsters.
//!
//! All of it reads the sim's world, which a joined client mirrors from its host's snapshots
//! (stage, weather and flocks) and events (every night event is sent to every player,
//! [`super::session::event_for`]), so a host and a joined player see and hear the same night.

use std::collections::HashMap;

use benilla_ui::script::UiScript;
use benilla_world::lighting::FogOverride;
use benilla_world::weather::WeatherMessage;
use bevy::prelude::*;
use nightfall_sim::{Content, CueSound, Event, RaidRoll, WeatherDef};

use super::content;
use super::enemies::EnemyViews;
use super::run::Sim;
use crate::sound::{Cue, SoundCue};

/// Seconds the fog takes to come most of the way to the night's (a stage thickening it).
const FOG_EASE: f32 = 4.0;
/// The chat line's colour, WoW's emote orange.
const LINE_COLOR: (f32, f32, f32) = (1.0, 0.5, 0.25);

/// The next raid: [`RaidRoll::roll`] from `seed`, with the test hooks' weather (a key) and
/// monsters (keys, comma-separated, beside the ones that always come) when given. A key the
/// content lacks is left as rolled, with a warning.
pub(super) fn plan(
    content: &Content,
    seed: u64,
    weather: Option<&str>,
    monsters: Option<&str>,
) -> RaidRoll {
    let mut roll = RaidRoll::roll(content, seed);
    if let Some(key) = weather {
        match content
            .rules
            .night
            .weather
            .iter()
            .position(|w| w.key == key)
        {
            Some(i) => roll.weather = i as u8,
            None => warn!("NIGHTFALL_WEATHER: no weather \"{key}\" in rules.ron"),
        }
    }
    if let Some(list) = monsters {
        let mut out: Vec<u8> = content
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| m.always)
            .map(|(i, _)| i as u8)
            .collect();
        for key in list.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            match content.monster(key) {
                Some(i) if !out.contains(&(i as u8)) => out.push(i as u8),
                Some(_) => {}
                None => warn!("NIGHTFALL_MONSTERS: no monster \"{key}\" in monsters.ron"),
            }
        }
        out.sort_unstable();
        roll.monsters = out;
    }
    roll
}

/// [`plan`] with the hooks from the environment: `NIGHTFALL_WEATHER`, `NIGHTFALL_MONSTERS`.
pub(super) fn plan_from_env(content: &Content, seed: u64) -> RaidRoll {
    plan(
        content,
        seed,
        std::env::var("NIGHTFALL_WEATHER").ok().as_deref(),
        std::env::var("NIGHTFALL_MONSTERS").ok().as_deref(),
    )
}

/// `NIGHTFALL_NIGHT_AT=<seconds>`: a hosted raid's clock starts there, to see the late stages.
pub(super) fn clock_at() -> Option<f32> {
    std::env::var("NIGHTFALL_NIGHT_AT")
        .ok()?
        .parse()
        .ok()
        .filter(|t: &f32| t.is_finite() && *t >= 0.0)
}

/// `cur` a frame of `dt` seconds nearer `target`, most of the way in [`FOG_EASE`]; on it once
/// within a hundredth.
pub(super) fn ease(cur: f32, target: f32, dt: f32) -> f32 {
    if (cur - target).abs() < 0.01 {
        return target;
    }
    cur + (target - cur) * (1.0 - (-dt / FOG_EASE).exp())
}

/// The Lua that puts `line` in the chat in emote orange: between long brackets, so a line
/// holding `]]` is refused (the content checks for it too).
pub(super) fn chat_lua(line: &str) -> Option<String> {
    let (r, g, b) = LINE_COLOR;
    (!line.contains("]]"))
        .then(|| format!("DEFAULT_CHAT_FRAME:AddMessage([[{line}]], {r}, {g}, {b})"))
}

/// The engine's weather for a raid's: its rain at its grade with its loop, or fine skies.
pub(super) fn weather_message(w: Option<&WeatherDef>) -> WeatherMessage {
    let rain = w.map_or(0.0, |w| w.rain);
    WeatherMessage {
        weather_type: if rain > 0.0 { 1 } else { 0 },
        grade: rain,
        sound_id: w.map_or(0, |w| w.sound),
        instant: false,
    }
}

/// The engine's cue for the content's.
pub(super) fn cue_of(c: &CueSound) -> Cue {
    match c {
        CueSound::Kit(id) => Cue::Kit(*id),
        CueSound::Name(n) => Cue::Name(n.clone()),
        CueSound::File(f) => Cue::File(f.clone()),
    }
}

/// What the map table says of a planned raid beside its exits (spec §8.5): the weather and the
/// signature monsters out.
pub(super) fn intel_lines(roll: &RaidRoll, content: &Content) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(w) = content.rules.night.weather.get(usize::from(roll.weather)) {
        lines.push(format!("Weather: {}", w.name));
    }
    let out: Vec<&str> = roll
        .monsters
        .iter()
        .filter_map(|&m| content.monsters.get(usize::from(m)))
        .map(|m| m.name.as_str())
        .collect();
    lines.push(if out.is_empty() {
        "Abroad: nothing of note".into()
    } else {
        format!("Abroad: {}", out.join(", "))
    });
    lines
}

/// The weather last given to the engine, so it is given once per change.
#[derive(Resource, Default)]
pub(super) struct SentWeather(Option<u8>);

/// The fog eases toward the night's (the weather's, thinned by Moonrise), so a stage's fog rolls
/// in rather than snapping.
pub(super) fn fog(sim: Res<Sim>, fog: Option<ResMut<FogOverride>>, real: Res<Time<Real>>) {
    let Some(mut f) = fog else {
        return;
    };
    let want = sim.world.fog();
    let dt = real.delta_secs();
    let start = ease(f.start, want.start, dt);
    let end = ease(f.end, want.end, dt);
    let color = [
        ease(f.color[0], want.color.0, dt),
        ease(f.color[1], want.color.1, dt),
        ease(f.color[2], want.color.2, dt),
    ];
    // Read first: only a change marks the override changed.
    if f.start != start || f.end != end || f.color != color {
        f.start = start;
        f.end = end;
        f.color = color;
    }
}

/// The raid's weather into the engine once, and again if a host's snapshot says otherwise.
pub(super) fn weather(
    sim: Res<Sim>,
    mut sent: ResMut<SentWeather>,
    mut out: MessageWriter<WeatherMessage>,
) {
    let w = sim.world.night_view().weather;
    if sent.0 == Some(w) {
        return;
    }
    sent.0 = Some(w);
    out.write(weather_message(
        content().rules.night.weather.get(usize::from(w)),
    ));
}

/// Out of the raid, fine skies again.
pub(super) fn clear_weather(mut sent: ResMut<SentWeather>, mut out: MessageWriter<WeatherMessage>) {
    if sent.0.take().is_some() {
        out.write(weather_message(None));
    }
}

/// The night's events of the frame, heard and read: a stage's cue and line, an exit closing, a
/// banshee's wail landing.
pub(super) fn cues(
    sim: Res<Sim>,
    script: Option<NonSend<UiScript>>,
    mut sounds: MessageWriter<SoundCue>,
) {
    let content = content();
    let say = |line: &str| {
        if let (Some(s), Some(lua)) = (script.as_deref(), chat_lua(line)) {
            let _ = s.run(&lua);
        }
    };
    for e in &sim.events {
        match e {
            Event::NightCue { stage } => {
                let Some(s) = content.rules.night.stages.get(usize::from(*stage)) else {
                    continue;
                };
                if let Some(c) = &s.cue {
                    sounds.write(SoundCue {
                        sound: cue_of(c),
                        at: None,
                    });
                }
                if let Some(l) = &s.line {
                    say(l);
                }
            }
            Event::ExitClosed { exit } => {
                if let Some(x) = content.map.exits.get(usize::from(*exit)) {
                    say(&format!("{} is shut for the rest of the night.", x.name));
                }
            }
            Event::Alarm { .. } => say("A banshee's wail carries across the hills."),
            _ => {}
        }
    }
}

/// An enemy with a tread (Stitches) is heard as it moves, within its `heard` yards of our hero:
/// from where it is drawn.
pub(super) fn step_sounds(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    bodies: Query<&Transform>,
    real: Res<Time<Real>>,
    mut next: Local<HashMap<u32, f32>>,
    mut sounds: MessageWriter<SoundCue>,
) {
    let content = content();
    let now = real.elapsed_secs();
    let me = sim.world.hero(sim.me).map(|h| h.pos);
    for e in &sim.world.enemies {
        let Some(step) = content
            .enemies
            .get(e.kind)
            .and_then(|d| d.step_sound.as_ref())
        else {
            continue;
        };
        let near = me.is_some_and(|m| m.distance(e.pos) <= step.heard);
        if e.speed <= 0.0 || e.hp <= 0.0 || !near {
            continue;
        }
        let due = next.entry(e.id).or_insert(now);
        if now < *due {
            continue;
        }
        *due = now + step.every;
        let at = views
            .drawn
            .get(&e.id)
            .and_then(|&b| bodies.get(b).ok())
            .map(|t| t.translation);
        if at.is_some() {
            sounds.write(SoundCue {
                sound: cue_of(&step.sound),
                at,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Content {
        Content::from_ron(nightfall_sim::ContentSources {
            classes: include_str!("../../../../content/nightfall/classes.ron"),
            map: include_str!("../../../../content/nightfall/map.ron"),
            rules: include_str!("../../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../../content/nightfall/items.ron"),
            loot: include_str!("../../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../../content/nightfall/monsters.ron"),
        })
        .unwrap()
    }

    #[test]
    fn the_hooks_set_the_weather_and_the_monsters() {
        let c = shipped();
        let rain = c
            .rules
            .night
            .weather
            .iter()
            .position(|w| w.key == "rain")
            .unwrap();
        let r = plan(&c, 3, Some("rain"), Some("morladim, banshee"));
        assert_eq!(usize::from(r.weather), rain);
        let mut want = vec![
            c.monster("morladim").unwrap() as u8,
            c.monster("banshee").unwrap() as u8,
            c.monster("stitches").unwrap() as u8,
        ];
        want.sort_unstable();
        assert_eq!(r.monsters, want);
        let rolled = RaidRoll::roll(&c, 3);
        let unknown = plan(&c, 3, Some("snow"), Some("dragon"));
        assert_eq!(unknown.weather, rolled.weather);
        assert_eq!(unknown.monsters, vec![c.monster("stitches").unwrap() as u8]);
        assert_eq!(plan(&c, 3, None, None), rolled);
    }

    #[test]
    fn the_fog_eases_toward_the_night_and_lands_on_it() {
        assert_eq!(ease(40.0, 30.0, 0.0), 40.0);
        let a = ease(40.0, 30.0, 1.0);
        assert!(a < 40.0 && a > 30.0);
        assert!(ease(a, 30.0, 1.0) < a);
        assert_eq!(ease(30.005, 30.0, 0.016), 30.0);
        assert!((ease(40.0, 30.0, 100.0) - 30.0).abs() < 0.01);
    }

    #[test]
    fn a_line_for_the_chat_is_lua_safe() {
        let l = chat_lua("A bell tolls in Darkshire.").unwrap();
        assert!(l.starts_with("DEFAULT_CHAT_FRAME:AddMessage([[A bell tolls in Darkshire.]]"));
        assert_eq!(chat_lua("nope]] os.exit()"), None);
    }

    #[test]
    fn rain_is_the_engines_rain_and_mist_is_not() {
        let c = shipped();
        let w = &c.rules.night.weather;
        let rain = weather_message(w.iter().find(|w| w.key == "rain"));
        assert_eq!((rain.weather_type, rain.sound_id), (1, 8534));
        assert!(rain.grade > 0.0);
        let mist = weather_message(w.iter().find(|w| w.key == "mist"));
        assert_eq!((mist.weather_type, mist.grade, mist.sound_id), (0, 0.0, 0));
        assert_eq!(weather_message(None).weather_type, 0);
    }

    #[test]
    fn the_map_table_names_the_weather_and_whats_abroad() {
        let c = shipped();
        let mist = c
            .rules
            .night
            .weather
            .iter()
            .position(|w| w.key == "mist")
            .unwrap() as u8;
        let roll = RaidRoll {
            weather: mist,
            monsters: vec![
                c.monster("morladim").unwrap() as u8,
                c.monster("stitches").unwrap() as u8,
            ],
            ..RaidRoll::standard(&c, 1)
        };
        assert_eq!(
            intel_lines(&roll, &c),
            vec![
                "Weather: Mist".to_string(),
                "Abroad: Mor'Ladim, Stitches".to_string()
            ]
        );
    }

    /// What the frame's systems said, for the tests below.
    #[derive(Resource, Default)]
    struct Heard {
        sounds: Vec<SoundCue>,
        /// (type, grade, sound) of each weather command.
        weather: Vec<(u32, f32, u32)>,
    }

    fn hear(
        mut s: MessageReader<SoundCue>,
        mut w: MessageReader<WeatherMessage>,
        mut h: ResMut<Heard>,
    ) {
        h.sounds.extend(s.read().cloned());
        h.weather
            .extend(w.read().map(|w| (w.weather_type, w.grade, w.sound_id)));
    }

    fn app_for(sim: Sim) -> App {
        let mut app = App::new();
        app.add_message::<SoundCue>()
            .add_message::<WeatherMessage>()
            .init_resource::<Heard>()
            .init_resource::<SentWeather>()
            .init_resource::<EnemyViews>()
            .insert_resource(sim);
        let mut time = Time::<Real>::default();
        time.update_with_duration(std::time::Duration::ZERO);
        app.insert_resource(time);
        app
    }

    #[test]
    fn a_joined_player_hears_every_night_event_the_host_sends() {
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let c = content();
        // What the host sends a peer standing far from everything: the night's events all come.
        let me = 1;
        let sent: Vec<Event> = [
            Event::NightCue { stage: 1 },
            Event::NightFell { stage: 1 },
            Event::ExitClosed { exit: 0 },
            Event::RavensFlew { flock: 0 },
            Event::Alarm {
                enemy: 3,
                player: 0,
            },
        ]
        .into_iter()
        .filter(|e| super::super::session::event_for(e, me, |_| false))
        .collect();
        assert_eq!(sent.len(), 5, "none of the night's events is filtered out");
        // The joined client's world is a mirror, with no raid of its own.
        let mut mirror = nightfall_sim::World::new(c.clone());
        mirror.begin_mirror();
        let mut sim = Sim::for_test(mirror);
        sim.me = me;
        sim.events = sent;
        let mut app = app_for(sim);
        app.add_systems(Update, (cues, super::super::ravens::cries, hear).chain());
        app.update();
        let heard = &app.world().resource::<Heard>().sounds;
        let cue = c.rules.night.stages[1].cue.as_ref().expect("stage 1 has a cue");
        assert!(
            heard
                .iter()
                .any(|s| s.sound == cue_of(cue) && s.at.is_none()),
            "the stage's cue plays everywhere: {heard:?}"
        );
        let cry = c.rules.night.ravens.cry.as_ref().expect("the ravens cry");
        let at = c.map.flocks[0];
        assert!(
            heard.iter().any(|s| s.sound == cue_of(cry)
                && s.at == Some(Vec3::new(-at.y, 0.0, -at.x))),
            "the flock's cry comes from where it sat: {heard:?}"
        );
    }

    #[test]
    fn a_joined_clients_fog_and_weather_follow_the_hosts_snapshot() {
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let c = content();
        let rain = c
            .rules
            .night
            .weather
            .iter()
            .position(|w| w.key == "rain")
            .unwrap();
        let mut host = nightfall_sim::World::new(c.clone());
        host.begin_raid(plan(&c, 5, Some("rain"), None)).unwrap();
        host.add_hero(0, "rogue", "Host", 1, 0).unwrap();
        let mut mirror = nightfall_sim::World::new(c.clone());
        mirror.begin_mirror();
        // Before its first snapshot a mirror is not in the hosts weather.
        assert_ne!(usize::from(mirror.night_view().weather), rain);
        mirror.apply_snapshot(host.snapshot_for(0));
        assert_eq!(usize::from(mirror.night_view().weather), rain);
        assert_eq!(mirror.fog(), host.fog());
        assert_eq!(mirror.night_view().flocks, host.night_view().flocks);
        let mut app = app_for(Sim::for_test(mirror));
        app.add_systems(Update, (weather, hear).chain());
        app.update();
        app.update();
        let heard = &app.world().resource::<Heard>().weather;
        // Given once, as rain with its loop.
        assert_eq!(heard.len(), 1, "{heard:?}");
        assert_eq!((heard[0].0, heard[0].2), (1, 8534));
    }

    #[test]
    fn a_tread_is_heard_from_where_the_walker_is_drawn_and_only_near_us() {
        let _ = super::super::CONTENT.set(super::super::shipped_content());
        let c = content();
        let kind = c
            .enemies
            .iter()
            .position(|e| e.step_sound.is_some())
            .expect("an enemy has a tread");
        let step = c.enemies[kind].step_sound.clone().unwrap();
        let mut world = nightfall_sim::World::new(c.clone());
        world.begin_raid(RaidRoll::standard(&c, 1)).unwrap();
        world.add_hero(0, "rogue", "Us", 1, 0).unwrap();
        let here = world.hero(0).unwrap().pos;
        let walker = |pos| nightfall_sim::Enemy {
            id: 9,
            kind,
            pos,
            z: 0.0,
            facing: nightfall_sim::V2::new(1.0, 0.0),
            hp: 10.0,
            max_hp: 10.0,
            speed: 2.0,
            asleep: false,
            fight: Default::default(),
        };
        world.enemies.push(walker(here + nightfall_sim::V2::new(10.0, 0.0)));
        let mut app = app_for(Sim::for_test(world));
        let body = app
            .world_mut()
            .spawn(Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)))
            .id();
        app.world_mut().resource_mut::<EnemyViews>().drawn.insert(9, body);
        app.add_systems(Update, (step_sounds, hear).chain());
        app.update();
        let heard = app.world().resource::<Heard>().sounds.clone();
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].sound, cue_of(&step.sound));
        assert_eq!(heard[0].at, Some(Vec3::new(1.0, 2.0, 3.0)));
        // Not again before its time...
        app.update();
        assert_eq!(app.world().resource::<Heard>().sounds.len(), 1);
        // ...and again once `every` seconds have passed.
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::from_secs_f32(step.every + 0.1));
        app.update();
        assert_eq!(app.world().resource::<Heard>().sounds.len(), 2);
        // Out of earshot, or standing still, it is not heard.
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.world.enemies[0].pos = here + nightfall_sim::V2::new(step.heard + 5.0, 0.0);
        }
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::from_secs_f32(step.every + 0.1));
        app.update();
        assert_eq!(app.world().resource::<Heard>().sounds.len(), 2);
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.world.enemies[0].pos = here + nightfall_sim::V2::new(5.0, 0.0);
            sim.world.enemies[0].speed = 0.0;
        }
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .update_with_duration(std::time::Duration::from_secs_f32(step.every + 0.1));
        app.update();
        assert_eq!(app.world().resource::<Heard>().sounds.len(), 2);
    }
}
