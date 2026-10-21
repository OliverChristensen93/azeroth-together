# Atmosphere: weather, lightning, sun shafts, the hour

`crates/benilla-app/src/atmosphere/` is one Bevy plugin, `AtmospherePlugin`, that gives a game
that is its own server (Azeroth Together, Nightfall) a "remaster" sky on top of the 1.12 engine:

| piece | file | what it does |
|---|---|---|
| weather | `weather.rs` | turns the request into the engine's own `WeatherMessage` (`benilla_world::weather`: 1.12's rain, snow, sandstorm, storm sky, rain fog), as a server's `SMSG_WEATHER` would |
| schedule | `schedule.rs` | `ZoneSky` (content) + seed + run clock -> `SkyRequest`, pure; the same for every co-op player |
| lightning | `lightning.rs` | in a rain storm: strikes flash the light, sky and fog (two or three pulses) and the thunder follows 0.3-4.5 s later from that side (SoundEntries 4936, the Blasted Lands bolt) |
| post pass | `post_node.rs`, `shafts.rs`, `atmos_post.wgsl` | one pass on the world camera that reads the scene's depth (Bevy's FullscreenMaterial node grown to bind the depth texture and `View`): polish (CAS-style sharpen, vibrance, contrast), ambient occlusion from depth, height fog (ground mist that pools and drifts, aerial haze, glowing toward the sun), sun shafts from open sky only (moon at 35 %), the storm grade and the lightning's screen flash |
| clock | `clock.rs` | the request's minute of the day into the engine's `WorldTime` |
| bolts | `bolt.rs` | each strike draws a jagged, forking bolt in the sky, 60-170 yd off, three in four within a third of a turn of where the camera looks, lit with the flash's pulses |
| lens drops (off by default, `+lens`) | `atmos_post.wgsl` | raindrops on the camera in rain: two layers of hashed drops that bend the picture, slide and dry |
| wind | `wind.rs` | a veering, gusting wind while something falls (gale in a storm, strong in sand), fed to the engine's `WeatherGust` so rain slants and mist streams |

It depends on bevy and benilla-world only (plus `crate::sound::SoundCue`, copied from Nightfall),
never on together-sim, `together/` or `nightfall/`.

## Players' options

`options.rs` + `options.lua`: an "Azeroth Together Graphics" panel in WoW's own 1.12 style,
opened from the Game Menu ("Azeroth Graphics", under Video Options), from a button in Video
Options and from the main menu's Graphics button (`together/atmos.rs`). Nine switches (weather,
thunderstorms, fog, sun shafts, contact shadows, sharpen and colour, wind, time of day,
raindrops) and three strength sliders, each a CVar registered by the panel (`atmosWeather`,
`atmosFogStrength`, ...), saved with the client's settings; "All off (original look)" and
"Defaults". The Rust side copies the CVars into `AtmosphereSettings` whenever they change,
unless a test run set `WOW_ATMOS`/`WOW_RAYS`/`WOW_MIST`/`WOW_AO`. Time of day off is midday.
`TOGETHER_GRAPHICS_PANEL=1` opens it for a test shot.

## The interface

- `SkyRequest` (resource), written by the game every frame or when it changes:
  `weather: WeatherKind`, `intensity: 0..1`, `storm: 0..1` (lightning only in rain with storm > 0),
  `mist: 0..1` (the zone's own ground mist), `minute: Option<f32>` (0..1440, `None` leaves the
  clock alone), `active: bool` (false: the
  plugin clears the weather it set and stops touching the clock).
- `AtmosphereSettings` (resource): `weather`, `lightning`, `rays`, `grade`, `clock` on/off and
  `mist` (the fog), `ao`, `polish`, `lens` (off by default), `wind`, and `ray_strength`,
  `mist_strength`, `ao_strength`. Starts from `WOW_ATMOS` (`off`, or a list
  like `-rays,-lightning`, `+lens`; names: weather, lightning, rays, grade, clock, mist, ao,
  polish, lens, wind), `WOW_MIST=<n>` (fog density), `WOW_AO=<n>` (occlusion) and
  `WOW_RAYS=<n>`. A game may change it at any time (a settings screen).
- `schedule::sky_at(&ZoneSky, seed, seconds, &settings) -> SkyRequest` for a game that wants
  weather in spells: `ZoneSky { precip, wet, storms, spell, intensity, dry_start, minute,
  day_speed }`, every field defaulted, so it reads from RON (`content/atmosphere.ron`).

## Engine carries it needs (already on this branch)

- `crates/benilla-world/src/lighting/atmos.rs` (+ one `mod atmos;` line in `lighting/mod.rs`):
  public doors on `WowLighting`: `celestial_dir`, `sun_day`, `moon_dir`, `moon_night`,
  `horizon_color` and `flash(k, tint)`.
- `crates/benilla-world/src/weather/gust.rs` (+ two lines in `weather/mod.rs`, three in
  `weather/precip/mod.rs`): `WeatherGust`, a game's wind. 1.12's rain leans only with the
  player's motion, so the wind is fed in as the player drifting against the air.
- `crates/benilla-app/src/sound/cue.rs` (+ three lines in `sound/mod.rs`): Nightfall's `SoundCue`,
  copied unchanged, so a merge with Nightfall's branch adds the same file on both sides.

## How Azeroth Together uses it

`together/atmos.rs`: reads `content/atmosphere.ron` (zone key -> `ZoneSky`), and each frame of a
run sets `SkyRequest` from `sky_at(zone sky, world.sky_seed, world.elapsed)`. `sky_seed` is a
`together-sim` `World` field sent in every snapshot, so a joined client schedules the same
weather. Outside a run it asks for 13:00, fine (what the old `daylight` system pinned).
Test switches: `TOGETHER_WEATHER=fine|rain|snow|sand|storm`, `TOGETHER_HOUR=<0..24>`.

## Porting to Nightfall

1. Copy `crates/benilla-app/src/atmosphere/` and `crates/benilla-world/src/lighting/atmos.rs`
   (+ its `mod atmos;` line) if the branch lacks them; `sound/cue.rs` is already there.
   Add `mod atmosphere;` to `lib.rs` and `ron = "0.12"` to benilla-app's Cargo.toml (for
   `ZoneSky`'s test only; drop the test if not wanted).
2. In Nightfall's plugin: `if !app.is_plugin_added::<AtmospherePlugin>() { app.add_plugins(AtmospherePlugin) }`.
3. Write `SkyRequest` from its night rules: e.g. `minute: Some(0.0)` (midnight), rain from its
   own roll, `active: true` during a raid and `active: false` in the inn / on leaving, so the
   pick screen gets its own sky back.
4. Its `FogOverride` (night fog) composes as is: rain's forced fog is the engine's, and the
   plugin never writes fog. Its grade and this pass share the post slot and both run.
5. Check `nightfall_grade` and `AtmosPost` order if the vignette should sit on top (add an edge).

## Testing

`WOW_ATMOS_DUMP=1` logs, once a second, the sun's direction (yaw and height), the camera's, the
shafts' screen position and strength, the rain and storm densities and the flash. To aim a test
camera at the sun, `WOW_PROBE_CAM=<yaw>,<pitch>,<dist>@<s>`: its yaw is the dump's with the sign
flipped (at 18:30 the sun is at dump yaw -45, so `WOW_PROBE_CAM=45,15,12@12`). Westfall shows the
shafts best (open sky); Elwynn's start is under a canopy that hides the sun.

## Cost

Weather: the engine's pooled particle quads (as on a real server). Lightning and clock: nothing.
The post pass: one full-screen pass with 40 taps per pixel while the sun is near the screen,
a plain copy otherwise; `WOW_ATMOS=-rays,-grade,-lightning` removes it from the camera.

## Tried and dropped

- World-space sunbeams (strips along the sunlight around the player): in playtest they looked
  placed at random, glowing through shade onto lit ground. Removed.
- Ground-mist sprites (flat sheets of 1.12's snow mist): replaced by the depth-based height fog.
- Lens drops are kept but off by default (`+lens`): they covered the screen and cleared slowly.

## Ideas not taken yet

- Rain and snow sound loops: the trimmed data lacks `Sound/Ambience/Weather/*.wav` (rain is
  silent until they are added).
- Wet, darker surfaces and puddles in rain (needs engine terrain/WMO shader edits).
- Lens drops and fog ignore being indoors (the engine freezes its rain there; a WMO interior
  check would hide them).
- Clouds do not race with the wind.
- Techniques credited to Corfirean/modern-wow-renderer are ideas only: that repository has no
  licence, so no code or texture of it is used.