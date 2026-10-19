//! The night in whole worlds on small hand-made maps: the roll, the clock, newcomers and zones,
//! the hunt, scent, Mor'Ladim and the banshee, the riders and Stitches, the ravens.

use std::sync::Arc;

#[allow(unused_imports)]
use crate::heightmap::testmap::{flat, pond};
use crate::heightmap::HeightMap;
#[allow(unused_imports)]
use crate::{
    Content, ContentSources, Event, Gait, HeroPose, PlayerId, PlayerInput, RaidRoll, World,
    TICK_HZ, V2, V3,
};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0),
]"#;
/// 120 yards square: a road across it along y = 60, a risk zone in the far corner, two flocks
/// of ravens, three exits (the last one late).
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (120.0, 0.0), (120.0, 120.0), (0.0, 120.0)],
    entries: [(5.0, 5.0, 0.0)],
    exits: [(key: "north", name: "The North Gate", at: (115.0, 5.0)),
            (key: "west", name: "The West Gate", at: (5.0, 115.0)),
            (key: "cart", name: "The Cart", at: (115.0, 115.0), late: true)],
    zones: [(key: "hill", name: "The Hill", hp: 1.5, damage: 1.25, loot_rolls: 1,
             area: [(90.0, 90.0), (120.0, 90.0), (120.0, 120.0), (90.0, 120.0)])],
    roads: [(key: "road", name: "The Road", points: [(1.0, 60.0), (60.0, 60.0), (119.0, 60.0)])],
    ravens: [(30.0, 30.0), (90.0, 30.0)])"#;
const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 1000.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 40.0, max: 40.0, every: 2.0))),
    (key: "worgen", name: "Worgen", display: 657, level: 27, hp: 900.0, walk_speed: 2.5,
     run_speed: 8.0, hunter: true),
    (key: "tracker", name: "Tracker", display: 736, level: 29, hp: 900.0, walk_speed: 4.5,
     run_speed: 8.5, tracker: true),
    (key: "morladim", name: "Mor'Ladim", display: 200, level: 35, hp: 4000.0, walk_speed: 2.5,
     run_speed: 6.5, watched: Some(90.0), swing: Some((min: 100.0, max: 100.0, every: 2.0))),
    (key: "banshee", name: "Banshee", display: 203, level: 30, hp: 700.0, walk_speed: 2.2,
     run_speed: 6.0, moves: [(ability: "wail", every: 40.0, first: Some(0.0))]),
    (key: "rider", name: "Rider", display: 200, level: 31, hp: 1400.0, walk_speed: 10.0,
     run_speed: 12.0, road_sight: 70.0),
    (key: "stitches", name: "Stitches", display: 559, level: 35, hp: 20000.0, walk_speed: 2.2,
     run_speed: 5.5, elite: true, reach: 4.0,
     swing: Some((min: 250.0, max: 250.0, every: 2.5)), moves: [(ability: "smash", every: 6.0)]),
]"#;
const ABILITIES: &str = r#"[
    (key: "wail", name: "Wail", spell: 8122, aim: Enemy, range: 40.0, cast: 2.0, sound: Loud,
     effects: [Alarm(radius: 60.0)], tags: [Control]),
    (key: "smash", name: "Smash", spell: 45, aim: Me, radius: 8.0, sound: Loud,
     telegraph: Some((shape: Circle(radius: 8.0), windup: 1.5, on_self: true)),
     effects: [Damage(min: 500.0, max: 500.0)], tags: [Nuke]),
]"#;
const ITEMS: &str = r#"[
    (key: "conjured_water", name: "Water", kind: Drink, icon: "a", weight: 0.1, stack: 20),
    (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10, value: 40),
]"#;
/// One chest outside the risk zone and one inside, each holding one roll of one gem.
const LOOT: &str = r#"(
    kinds: [(key: "chest", name: "Chest", display: 259, search: 2.0, sound: Open, slots: 8)],
    tables: [(key: "one", rolls: (1, 1), items: [(item: "gem", weight: 1.0)])],
    containers: [(kind: "chest", table: "one", at: (10.0, 10.0)),
                 (kind: "chest", table: "one", at: (100.0, 100.0))],
    bodies: [],
)"#;

/// The tests' rules with `night` inside `night: ( … )` (the spec's stages when empty).
pub(super) fn rules(night: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 200.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0),
        night: ({night}))"#
    )
}

/// The tests' content: `spawns` and `monsters` are RON list items, each ending in a comma.
pub(super) fn content(night: &str, spawns: &str, monsters: &str) -> Content {
    content_on(MAP, night, spawns, monsters)
}

/// [`content`] on another map.
pub(super) fn content_on(map: &str, night: &str, spawns: &str, monsters: &str) -> Content {
    Content::from_ron(ContentSources {
        classes: CLASSES,
        map,
        rules: &rules(night),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
        monsters: &format!("[{monsters}]"),
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

/// The roll every raid of the earlier plans had.
pub(super) fn standard(c: &Content) -> RaidRoll {
    RaidRoll::standard(c, 1)
}

/// Flat ground 120 yards across.
pub(super) fn open() -> HeightMap {
    flat(120.0).finish()
}

/// A world on `map` with this content, its raid begun from `roll`, then its map attached.
pub(super) fn world_on(
    map: HeightMap,
    night: &str,
    spawns: &str,
    monsters: &str,
    roll: impl FnOnce(&Content) -> RaidRoll,
) -> World {
    let c = content(night, spawns, monsters);
    let r = roll(&c);
    let mut w = World::new(Arc::new(c));
    w.begin_raid(r).unwrap();
    w.attach_map(Arc::new(map));
    w
}

/// [`world_on`] open ground, no monsters, the standard roll.
pub(super) fn world(night: &str, spawns: &str) -> World {
    world_on(open(), night, spawns, "", standard)
}

/// A sentry spawn line of `kind` at `(x, y)` facing `facing` degrees (0 is +x).
pub(super) fn sentry(kind: &str, x: f32, y: f32, facing: f32) -> String {
    format!(
        r#"(place: "Post", kind: "{kind}", behaviour: Sentry(at: ({x:.1}, {y:.1}), facing: {facing:.1})),"#
    )
}

/// `player`'s next input: their hero at `(x, y)` facing `facing`, moving at `gait`.
pub(super) fn put(w: &mut World, player: PlayerId, x: f32, y: f32, facing: V2, gait: Gait) {
    w.set_input(
        player,
        PlayerInput {
            pose: Some(HeroPose {
                pos: V2::new(x, y),
                facing,
                z: 0.0,
            }),
            gait,
            ..Default::default()
        },
    );
}

/// `player`'s rogue standing at `(x, y)`, facing +x, its first input taken.
pub(super) fn join(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.add_hero(player, "rogue", "Hero", 1, 0).unwrap();
    put(w, player, x, y, V2::new(1.0, 0.0), Gait::Still);
    w.step();
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

/// Steps in `s` seconds.
pub(super) fn secs(s: f32) -> usize {
    (s * TICK_HZ as f32).round() as usize
}

/// The events since the last drain that `pick` keeps.
#[allow(dead_code)]
pub(super) fn events_of<T>(w: &mut World, pick: impl Fn(&Event) -> Option<T>) -> Vec<T> {
    w.drain_events().iter().filter_map(pick).collect()
}

const TWO_MONSTERS: &str = r#"
    (key: "ghost", name: "Ghost", odds: 0.5, spawns: [
        (place: "Ghost", kind: "brute", behaviour: Sentry(at: (50.0, 50.0), facing: 0.0))]),
    (key: "wraith", name: "Wraith", odds: 0.5, spawns: [
        (place: "Wraith", kind: "brute", behaviour: Sentry(at: (70.0, 50.0), facing: 0.0))]),
    (key: "walker", name: "Walker", always: true, spawns: [
        (place: "Walker", kind: "stitches", stage: Some("stitches"), behaviour: Road(road: "road"))]),
"#;

#[test]
fn the_same_seed_rolls_the_same_raid_and_seeds_differ() {
    let c = content(
        r#"weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
                     (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0)]"#,
        "",
        TWO_MONSTERS,
    );
    assert_eq!(RaidRoll::roll(&c, 42), RaidRoll::roll(&c, 42));
    let rolls: Vec<RaidRoll> = (0..40).map(|s| RaidRoll::roll(&c, s)).collect();
    assert!(rolls.iter().any(|r| r.weather != rolls[0].weather));
    assert!(rolls.iter().any(|r| r.monsters != rolls[0].monsters));
    assert!(rolls.iter().any(|r| r.exits != rolls[0].exits));
    // The loot's seed is the raid's own.
    assert!(rolls.iter().enumerate().all(|(s, r)| r.seed == s as u64));
}

#[test]
fn a_thousand_raids_each_have_a_way_out() {
    let c = content("exits: (shut: 0.9, late: 0.9, open_least: 2)", "", "");
    for seed in 0..1000 {
        let r = RaidRoll::roll(&c, seed);
        assert_eq!(r.exits.len(), 3);
        assert!(
            r.exits.iter().any(|p| p.open && p.opens_at <= 0.0),
            "seed {seed}: no way out from the start: {:?}",
            r.exits
        );
        assert!(
            r.exits.iter().filter(|p| p.open).count() >= 2,
            "seed {seed}"
        );
        // The cart opens within its jitter of cart_opens_at (1080 s by default).
        assert!(
            (960.0..=1200.0).contains(&r.exits[2].opens_at),
            "seed {seed}"
        );
        for p in &r.exits[..2] {
            assert!(!p.open || p.opens_at <= 0.0 || (900.0..=1320.0).contains(&p.opens_at));
        }
    }
}

#[test]
fn the_hunt_never_closes_the_last_way_out() {
    let c = content(
        "exits: (shut: 0.6, late: 0.0, hunt_close: 1.0, open_least: 1)",
        "",
        "",
    );
    let hunt = c.rules.night.stages[3].at;
    let mut closed = 0;
    for seed in 0..1000 {
        let r = RaidRoll::roll(&c, seed);
        let Some((x, at)) = r.closes else { continue };
        closed += 1;
        assert_eq!(at, hunt);
        assert!(
            !c.map.exits[usize::from(x)].late,
            "seed {seed}: the cart never closes"
        );
        assert!(r.exit_open(usize::from(x), at - 1.0) && !r.exit_open(usize::from(x), at));
        let after = (0..3)
            .filter(|&i| i != usize::from(x) && r.exits[i].open)
            .count();
        assert!(after >= 1, "seed {seed}: {r:?} closed its last way out");
    }
    assert!(
        closed > 100,
        "the hunt closes an exit when it can: {closed}"
    );
}

#[test]
fn monsters_out_stay_within_the_rules_and_the_always_ones_come() {
    let c = content("monsters_out: (1, 1)", "", TWO_MONSTERS);
    for seed in 0..300 {
        let r = RaidRoll::roll(&c, seed);
        assert!(
            r.monsters.contains(&2),
            "seed {seed}: the walker always comes"
        );
        assert_eq!(
            r.monsters.len(),
            2,
            "seed {seed}: exactly one of the other two"
        );
        assert!(r.monsters.windows(2).all(|w| w[0] < w[1]));
    }
    let both = (0..300).any(|s| RaidRoll::roll(&c, s).monsters.contains(&0))
        && (0..300).any(|s| RaidRoll::roll(&c, s).monsters.contains(&1));
    assert!(both);
    assert_eq!(RaidRoll::standard(&c, 7).monsters, vec![2]);
}

#[test]
fn the_weather_follows_its_weights() {
    let c = content(
        r#"weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
                     (key: "never", name: "Never", weight: 0.0, fog_start: 4.0, fog_end: 28.0),
                     (key: "rain", name: "Rain", weight: 3.0, fog_start: 6.0, fog_end: 32.0, rain: 0.7)]"#,
        "",
        "",
    );
    let counts = (0..1000).fold([0; 3], |mut n, s| {
        n[usize::from(RaidRoll::roll(&c, s).weather)] += 1;
        n
    });
    assert_eq!(counts[1], 0);
    assert!(counts[2] > counts[0] * 2, "{counts:?}");
    assert_eq!(RaidRoll::roll(&content("", "", ""), 3).weather, 0);
}

#[test]
fn a_closed_exit_takes_nobody_out_and_a_roll_must_fit_its_content() {
    let mut w = world_on(open(), "", "", TWO_MONSTERS, |c| RaidRoll {
        closes: Some((0, 1.0)),
        ..standard(c)
    });
    join(&mut w, 0, 115.0, 5.0);
    steps(&mut w, secs(12.0));
    assert!(
        w.hero(0).is_some(),
        "the north gate closed before the count ended"
    );
    assert!(!w.loot_view(Some(0)).exits[0].open);
    assert_eq!(w.loot_view(Some(0)).exits[0].opens_in, None);
    let c = content("", "", TWO_MONSTERS);
    let mut w = World::new(Arc::new(c.clone()));
    assert!(w
        .begin_raid(RaidRoll {
            monsters: vec![3],
            ..standard(&c)
        })
        .is_err());
    assert!(w
        .begin_raid(RaidRoll {
            weather: 1,
            ..standard(&c)
        })
        .is_err());
}

#[test]
fn seeds_that_differ_in_one_bit_roll_different_raids() {
    // `Dice::new` sets bit 0, so an unhashed seed `s` and `s ^ 1` once rolled the same raid.
    let c = content("exits: (shut: 0.5, late: 0.5)", "", TWO_MONSTERS);
    let same = (0..200u64)
        .map(|s| s << 1)
        .filter(|&s| {
            let (a, b) = (RaidRoll::roll(&c, s), RaidRoll::roll(&c, s ^ 1));
            a.exits == b.exits && a.monsters == b.monsters && a.weather == b.weather
        })
        .count();
    assert!(
        same < 40,
        "{same} of 200 pairs of seeds rolled the same raid"
    );
}

/// Two gates and no cart: the hunt can close one only when the other is open.
const TWO_GATES: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (120.0, 0.0), (120.0, 120.0), (0.0, 120.0)],
    entries: [(5.0, 5.0, 0.0)],
    exits: [(key: "north", name: "The North Gate", at: (115.0, 5.0)),
            (key: "west", name: "The West Gate", at: (5.0, 115.0))])"#;

#[test]
fn the_hunt_never_closes_the_only_open_exit_when_there_is_no_cart() {
    // With a cart (always counted open) the guard against closing the last way out is never
    // needed; here a raid can have just one exit open, and the hunt must leave it.
    let c = content_on(
        TWO_GATES,
        "exits: (shut: 0.6, late: 0.0, hunt_close: 1.0, open_least: 1)",
        "",
        "",
    );
    let (mut lone, mut closed) = (0, 0);
    for seed in 0..1000 {
        let r = RaidRoll::roll(&c, seed);
        let open = r.exits.iter().filter(|p| p.open).count();
        match r.closes {
            None => lone += usize::from(open == 1),
            Some((x, _)) => {
                closed += 1;
                assert_eq!(open, 2, "seed {seed}: closed one of {open} open exits");
                assert!(r.exits[1 - usize::from(x)].open);
            }
        }
    }
    assert!(lone > 100, "raids with one exit open: {lone}");
    assert!(closed > 100, "raids where the hunt closes one: {closed}");
}

#[test]
fn a_pinned_exit_is_never_shut_late_or_closed() {
    // The Westfall road of the shipped map: Ladim's tome is handed in there.
    let map = MAP.replace(
        r#"(key: "west", name: "The West Gate", at: (5.0, 115.0))"#,
        r#"(key: "west", name: "The West Gate", at: (5.0, 115.0), pinned: true)"#,
    );
    let c = content_on(
        &map,
        "exits: (shut: 0.9, late: 0.9, hunt_close: 1.0, open_least: 1)",
        "",
        "",
    );
    let mut closed_other = 0;
    for seed in 0..1000 {
        let r = RaidRoll::roll(&c, seed);
        assert_eq!(
            r.exits[1],
            crate::ExitPlan {
                open: true,
                opens_at: 0.0
            },
            "seed {seed}"
        );
        assert_ne!(r.closes.map(|(x, _)| x), Some(1), "seed {seed}");
        closed_other += usize::from(r.closes.is_some());
    }
    assert!(closed_other > 0, "the hunt still closes the unpinned gate");
}

/// Three stages, a second apart, each cued half a second before.
const QUICK_NIGHT: &str = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
    (key: "one", name: "One", at: 2.0, warn: 0.5),
    (key: "two", name: "Two", at: 4.0, warn: 0.5, fog: 0.75, notice: 1.2)]"#;

/// The night's events since the last drain, with the step each came on.
fn night_events(w: &mut World, log: &mut Vec<(usize, Event)>, step: usize) {
    for e in w.drain_events() {
        if matches!(
            e,
            Event::NightCue { .. } | Event::NightFell { .. } | Event::ExitClosed { .. }
        ) {
            log.push((step, e));
        }
    }
}

#[test]
fn each_stage_is_cued_before_it_lands_and_only_once() {
    let mut w = world(QUICK_NIGHT, "");
    let mut log = Vec::new();
    for step in 0..secs(6.0) {
        w.step();
        night_events(&mut w, &mut log, step);
    }
    let kinds: Vec<&Event> = log.iter().map(|(_, e)| e).collect();
    assert_eq!(
        kinds,
        vec![
            &Event::NightCue { stage: 1 },
            &Event::NightFell { stage: 1 },
            &Event::NightCue { stage: 2 },
            &Event::NightFell { stage: 2 },
        ]
    );
    // Cued half a second (15 steps, give or take one for the clock's rounding) before landing.
    assert!((14..=16).contains(&(log[1].0 - log[0].0)), "{log:?}");
    assert!((14..=16).contains(&(log[3].0 - log[2].0)), "{log:?}");
    assert_eq!(w.night_view().stage, 2);
}

#[test]
fn a_clock_started_late_gives_every_passed_stage_once_in_order() {
    let mut w = world(QUICK_NIGHT, "");
    w.elapsed = 5.0;
    let mut log = Vec::new();
    for step in 0..secs(2.0) {
        w.step();
        night_events(&mut w, &mut log, step);
    }
    let kinds: Vec<&Event> = log.iter().map(|(_, e)| e).collect();
    assert_eq!(
        kinds,
        vec![
            &Event::NightCue { stage: 1 },
            &Event::NightCue { stage: 2 },
            &Event::NightFell { stage: 1 },
            &Event::NightFell { stage: 2 },
        ]
    );
    assert!(log.iter().all(|(s, _)| *s == 0), "all in the first step");
}

#[test]
fn moonrise_and_the_weather_close_the_fog_and_sharpen_the_eyes() {
    let night = format!(
        r#"{QUICK_NIGHT}, weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
            (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0, fog_color: Some((0.2, 0.2, 0.2)))]"#
    );
    // A walking hero (visibility 55) 22 yards in front of a sentry: past its 19.25-yard notice
    // range under a clear sky, inside its 23.1 yards once the eyes sharpen.
    let mut w = world_on(
        open(),
        &night,
        &sentry("brute", 30.0, 30.0, 0.0),
        "",
        standard,
    );
    assert_eq!(w.fog().end, 40.0);
    join(&mut w, 0, 52.0, 30.0);
    for _ in 0..secs(3.5) {
        put(&mut w, 0, 52.0, 30.0, V2::new(-1.0, 0.0), Gait::Walk);
        w.step();
    }
    assert!(
        w.marks_for(0).is_empty(),
        "not noticed before the second stage"
    );
    for _ in 0..secs(6.0) {
        put(&mut w, 0, 52.0, 30.0, V2::new(-1.0, 0.0), Gait::Walk);
        w.step();
    }
    assert_eq!(w.fog().end, 30.0);
    assert!((w.notice_scale() - 1.2).abs() < 1e-6);
    assert!(!w.marks_for(0).is_empty(), "noticed once the eyes sharpen");
    let mist = world_on(open(), &night, "", "", |c| RaidRoll {
        weather: 1,
        ..standard(c)
    });
    assert_eq!(
        mist.fog(),
        crate::Fog {
            start: 4.0,
            end: 28.0,
            color: (0.2, 0.2, 0.2)
        }
    );
}

#[test]
fn a_hero_sees_no_further_than_the_fog_now() {
    let night = r#"weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
        (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0)]"#;
    let post = sentry("brute", 65.0, 30.0, 0.0);
    let mut clear = world_on(open(), night, &post, "", standard);
    join(&mut clear, 0, 30.0, 30.0);
    assert_eq!(clear.seen_by(0), vec![1]);
    let mut mist = world_on(open(), night, &post, "", |c| RaidRoll {
        weather: 1,
        ..standard(c)
    });
    join(&mut mist, 0, 30.0, 30.0);
    assert!(mist.seen_by(0).is_empty());
}

#[test]
fn the_mirror_draws_the_hosts_fog() {
    let night = format!(
        r#"{QUICK_NIGHT}, weather: [(key: "clear", name: "Clear", weight: 1.0, fog_start: 10.0, fog_end: 40.0),
            (key: "mist", name: "Mist", weight: 1.0, fog_start: 4.0, fog_end: 28.0)]"#
    );
    let mut host = world_on(open(), &night, "", "", |c| RaidRoll {
        weather: 1,
        ..standard(c)
    });
    steps(&mut host, secs(4.5));
    let mut mirror = World::new(host.content().clone());
    mirror.apply_snapshot(host.snapshot());
    assert_eq!(mirror.night_view(), host.night_view());
    assert_eq!(mirror.fog(), host.fog());
    assert_eq!(mirror.fog().end, 21.0);
    // A mirror's clock never runs: its night is what the host said.
    steps(&mut mirror, secs(1.0));
    assert!(mirror.drain_events().is_empty());
}

#[test]
fn the_hunt_closes_its_exit_and_says_so_once() {
    let mut w = world_on(open(), QUICK_NIGHT, "", "", |c| RaidRoll {
        closes: Some((1, 2.0)),
        ..standard(c)
    });
    let mut log = Vec::new();
    for step in 0..secs(4.0) {
        w.step();
        night_events(&mut w, &mut log, step);
    }
    let closed: Vec<&Event> = log
        .iter()
        .map(|(_, e)| e)
        .filter(|e| matches!(e, Event::ExitClosed { .. }))
        .collect();
    assert_eq!(closed, vec![&Event::ExitClosed { exit: 1 }]);
}

/// Every list that runs beside the world's enemies, as long as it, ids unique.
fn lists_in_step(w: &World) {
    let n = w.enemies.len();
    assert_eq!(w.brains.len(), n, "brains");
    assert!(w.senses.rows_match(n), "minds and ways");
    assert_eq!(w.combat.enemies.len(), n, "fights");
    let mut ids: Vec<u32> = w.enemies.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "ids are unique");
}

const GHOST: &str = r#"
    (key: "ghost", name: "Ghost", odds: 0.5, spawns: [
        (place: "Ghost", kind: "brute", behaviour: Sentry(at: (50.0, 50.0), facing: 0.0))]),
"#;

#[test]
fn only_the_monsters_out_are_placed_whenever_the_raid_begins() {
    let none = world_on(open(), "", "", GHOST, standard);
    assert!(none.enemies.is_empty());
    let out = world_on(open(), "", "", GHOST, |c| RaidRoll {
        monsters: vec![0],
        ..standard(c)
    });
    assert_eq!(out.enemies.len(), 1);
    lists_in_step(&out);
    // The map first, the raid after: the raid's own monsters, placed again.
    let c = content("", "", GHOST);
    let mut w = World::new(Arc::new(c.clone()));
    w.attach_map(Arc::new(open()));
    assert!(w.enemies.is_empty());
    w.begin_raid(RaidRoll {
        monsters: vec![0],
        ..standard(&c)
    })
    .unwrap();
    assert_eq!(w.enemies.len(), 1);
    lists_in_step(&w);
    // And the next raid, without it, starts over.
    w.begin_raid(standard(&c)).unwrap();
    assert!(w.enemies.is_empty());
    lists_in_step(&w);
}

#[test]
fn a_content_mistake_is_told_once_however_often_the_enemies_are_placed() {
    // A post over a hole has no floor to stand on.
    let spawns = sentry("brute", 10.0, 10.0, 0.0);
    let c = content("", &spawns, "");
    let mut b = flat(120.0);
    for col in b.columns_in(V2::new(8.0, 8.0), V2::new(12.0, 12.0)) {
        b.set_ground(col, f32::NAN);
    }
    let mut w = World::new(Arc::new(c.clone()));
    w.attach_map(Arc::new(b.finish()));
    let once = w.warnings().len();
    assert!(once >= 1, "{:?}", w.warnings());
    w.begin_raid(standard(&c)).unwrap();
    w.begin_raid(standard(&c)).unwrap();
    assert_eq!(w.warnings().len(), once, "{:?}", w.warnings());
}

const NIGHT_WITH_LATE: &str = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
    (key: "late", name: "Late", at: 1.0, warn: 0.0)], arrive_unseen: 20.0"#;

#[test]
fn newcomers_wait_unseen_and_keep_every_list_in_step() {
    let spawns = format!(
        r#"{}(place: "Late", kind: "brute", stage: Some("late"), behaviour: Sentry(at: (60.0, 30.0), facing: 0.0)),"#,
        sentry("brute", 10.0, 100.0, 0.0)
    );
    let mut w = world(NIGHT_WITH_LATE, &spawns);
    assert_eq!(w.enemies.len(), 1, "the newcomer is not there at the start");
    join(&mut w, 0, 62.0, 30.0);
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 1, "it waits while a hero stands where it comes");
    put(&mut w, 0, 10.0, 10.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(1.5));
    assert_eq!(w.enemies.len(), 2);
    lists_in_step(&w);
    assert_eq!(w.enemies[1].id, 2);
    assert_eq!(w.enemies[1].pos, V2::new(60.0, 30.0));
    // It thinks like the rest: a hero walking in front of it is noticed.
    for _ in 0..secs(5.0) {
        put(&mut w, 0, 70.0, 30.0, V2::new(-1.0, 0.0), Gait::Walk);
        w.step();
    }
    assert!(w.marks_for(0).iter().any(|m| m.enemy == 2));
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 2, "it came once");
}

#[test]
fn a_hero_who_is_down_does_not_keep_a_newcomer_out() {
    let spawns = r#"(place: "Late", kind: "brute", stage: Some("late"), behaviour: Sentry(at: (60.0, 30.0), facing: 0.0)),"#;
    let mut w = world(NIGHT_WITH_LATE, spawns);
    join(&mut w, 0, 62.0, 30.0);
    w.hurt_hero(0, 1.0e6);
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 1, "a downed hero is not looking");
}

#[test]
fn a_stage_brings_only_the_lines_this_raid_wants_and_the_first_stage_is_there_at_the_start() {
    let monsters = r#"
        (key: "ghost", name: "Ghost", odds: 0.5, spawns: [
            (place: "Ghost", kind: "brute", stage: Some("late"),
             behaviour: Sentry(at: (60.0, 30.0), facing: 0.0))]),
        (key: "wraith", name: "Wraith", odds: 0.5, spawns: [
            (place: "Wraith", kind: "brute", stage: Some("late"),
             behaviour: Sentry(at: (30.0, 30.0), facing: 0.0))]),
    "#;
    let spawns = r#"(place: "Dusk", kind: "brute", stage: Some("dusk"),
        behaviour: Sentry(at: (90.0, 30.0), facing: 0.0)),"#;
    let mut w = world_on(open(), NIGHT_WITH_LATE, spawns, monsters, |c| RaidRoll {
        monsters: vec![0],
        ..standard(c)
    });
    assert_eq!(w.enemies.len(), 1, "a line of the first stage is there from the start");
    join(&mut w, 0, 5.0, 100.0);
    steps(&mut w, secs(4.0));
    let at: Vec<V2> = w.enemies.iter().map(|e| e.pos).collect();
    assert_eq!(at, vec![V2::new(90.0, 30.0), V2::new(60.0, 30.0)], "only the ghost came");
    lists_in_step(&w);
}

#[test]
fn a_newcomer_has_the_fight_its_kind_begins_with() {
    // The banshee's wail is ready at once (`first: Some(0.0)`); the default is half its wait.
    let spawns = r#"(place: "Late", kind: "banshee", stage: Some("late"),
        behaviour: Sentry(at: (60.0, 30.0), facing: 0.0)),"#;
    let mut w = world(NIGHT_WITH_LATE, spawns);
    join(&mut w, 0, 5.0, 100.0);
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 1);
    assert_eq!(w.combat.enemies[0].moves_in, vec![0.0]);
    lists_in_step(&w);
}

/// The health of a brute at `(x, y)` and the blows it strikes at a hero beside it.
fn brute_at(x: f32, y: f32) -> ((f32, f32), Vec<f32>) {
    let mut w = world("", &sentry("brute", x, y, 180.0));
    let max = (w.enemies[0].hp, w.enemies[0].max_hp);
    join(&mut w, 0, x - 1.5, y);
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, secs(3.0));
    let blows = events_of(&mut w, |e| match e {
        Event::Damage {
            to: crate::UnitRef::Hero(0),
            amount,
            ..
        } => Some(*amount),
        _ => None,
    });
    (max, blows)
}

#[test]
fn a_risk_zone_makes_enemies_tougher_and_its_loot_richer() {
    let (max, blows) = brute_at(100.0, 100.0);
    assert_eq!(max, (1500.0, 1500.0));
    assert!(!blows.is_empty() && blows.iter().all(|&a| a == 50.0), "{blows:?}");
    // Outside the zone: its own health and blows.
    let (max, blows) = brute_at(20.0, 100.0);
    assert_eq!(max, (1000.0, 1000.0));
    assert!(!blows.is_empty() && blows.iter().all(|&a| a == 40.0), "{blows:?}");
    let w = world("", "");
    let stacks = |at: V2| {
        w.raid
            .containers
            .iter()
            .find(|c| c.at == at)
            .map(|c| c.stacks.iter().flatten().count())
    };
    assert_eq!(stacks(V2::new(10.0, 10.0)), Some(1));
    assert_eq!(stacks(V2::new(100.0, 100.0)), Some(2));
}

#[test]
fn a_hurt_from_nowhere_is_not_made_heavier_by_a_zone() {
    let mut w = world("", "");
    join(&mut w, 0, 100.0, 100.0);
    w.hurt_hero(0, 10.0);
    let hurt = events_of(&mut w, |e| match e {
        Event::Damage {
            amount, by: None, ..
        } => Some(*amount),
        _ => None,
    });
    assert_eq!(hurt, vec![10.0]);
}

#[test]
fn a_newcomer_in_a_risk_zone_is_as_tough_and_as_heavy_as_one_placed_at_the_start() {
    let spawns = r#"(place: "Late", kind: "brute", stage: Some("late"),
        behaviour: Sentry(at: (100.0, 100.0), facing: 180.0)),"#;
    let mut w = world(NIGHT_WITH_LATE, spawns);
    join(&mut w, 0, 5.0, 5.0);
    steps(&mut w, secs(3.0));
    assert_eq!(w.enemies.len(), 1);
    assert_eq!((w.enemies[0].hp, w.enemies[0].max_hp), (1500.0, 1500.0));
    assert_eq!(w.brains[0].might(), 1.25);
}

/// The hunt from the first second, the hunters called every second.
const HUNT_NIGHT: &str = r#"stages: [(key: "dusk", name: "Dusk", at: 0.0),
    (key: "hunt", name: "The hunt", at: 1.0, warn: 0.0, hunt: true)],
    hunt: (every: 1.0, memory: 30.0, reach: 200.0, rest: 4.0)"#;

/// A hunting worgen far in the north-east and a brute beside it, a hero far in the south-west
/// who makes a loud noise none of them hears.
fn hunt_world(night: &str) -> World {
    let spawns = format!(
        "{}{}",
        sentry("worgen", 100.0, 100.0, 0.0),
        sentry("brute", 100.0, 90.0, 0.0)
    );
    let mut w = world(night, &spawns);
    join(&mut w, 0, 10.0, 10.0);
    w.make_noise(0, V3::new(10.0, 10.0, 0.0), crate::Loudness::Loud);
    w
}

#[test]
fn the_hunt_sends_hunters_toward_the_loudest_noise() {
    let mut w = hunt_world(HUNT_NIGHT);
    steps(&mut w, secs(8.0));
    let to_noise = |w: &World, i: usize| w.enemies[i].pos.distance(V2::new(10.0, 10.0));
    assert!(
        to_noise(&w, 0) < 100.0,
        "the worgen ran toward it: {:?}",
        w.enemies[0].pos
    );
    assert_eq!(
        w.enemies[1].pos,
        V2::new(100.0, 90.0),
        "the brute is no hunter"
    );
    lists_in_step(&w);
}

#[test]
fn no_hunt_before_its_stage_nor_after_the_noise_is_forgotten_nor_beyond_its_reach() {
    let early = HUNT_NIGHT.replace("at: 1.0, warn: 0.0, hunt", "at: 100.0, warn: 0.0, hunt");
    let mut w = hunt_world(&early);
    steps(&mut w, secs(8.0));
    assert_eq!(w.enemies[0].pos, V2::new(100.0, 100.0));
    let forgetful = HUNT_NIGHT
        .replace("at: 1.0, warn: 0.0, hunt", "at: 3.0, warn: 0.0, hunt")
        .replace("memory: 30.0", "memory: 1.0");
    let mut w = hunt_world(&forgetful);
    steps(&mut w, secs(8.0));
    assert_eq!(w.enemies[0].pos, V2::new(100.0, 100.0));
    // The worgen is 127 yards from the noise.
    let short = HUNT_NIGHT.replace("reach: 200.0", "reach: 100.0");
    let mut w = hunt_world(&short);
    steps(&mut w, secs(8.0));
    assert_eq!(w.enemies[0].pos, V2::new(100.0, 100.0));
}

#[test]
fn the_hunt_goes_to_the_loudest_noise_not_the_latest() {
    let mut w = hunt_world(HUNT_NIGHT);
    steps(&mut w, secs(1.0));
    // A quiet one later, somewhere else.
    w.make_noise(0, V3::new(10.0, 100.0, 0.0), crate::Loudness::Quiet);
    steps(&mut w, secs(1.0));
    let loud = w.night.loudest.expect("a noise is remembered");
    assert_eq!(loud.at, V3::new(10.0, 10.0, 0.0));
}

#[test]
fn a_hunter_who_finds_someone_on_the_way_forgets_the_noise() {
    let mut w = hunt_world(HUNT_NIGHT);
    steps(&mut w, secs(3.0));
    assert!(w.brains[0].on_errand(), "the worgen is running to the noise");
    w.alert(w.enemies[0].id, 0);
    steps(&mut w, secs(1.0));
    assert!(!w.brains[0].on_errand(), "its senses have it now");
}

#[test]
fn a_hunter_rests_where_the_noise_was_then_goes_home() {
    let mut w = hunt_world(HUNT_NIGHT);
    // Far from where it goes, so nothing it sees there holds it.
    put(&mut w, 0, 110.0, 10.0, V2::new(1.0, 0.0), Gait::Still);
    steps(&mut w, secs(30.0));
    let at_noise = w.enemies[0].pos.distance(V2::new(10.0, 10.0));
    let home = w.enemies[0].pos.distance(V2::new(100.0, 100.0));
    assert!(at_noise < 3.0 || home < at_noise, "{:?}", w.enemies[0].pos);
    // The noise is forgotten 30 s after it was made; the worgen rests 4 s more, then walks the
    // 127 yards home at 2.5 yards a second.
    steps(&mut w, secs(65.0));
    assert!(
        w.enemies[0].pos.distance(V2::new(100.0, 100.0)) < 1.0,
        "back at its post"
    );
}

#[test]
fn a_louder_noise_forgotten_gives_way_to_a_quieter_later_one() {
    let mut w = hunt_world(HUNT_NIGHT.replace("memory: 30.0", "memory: 2.0").as_str());
    steps(&mut w, secs(1.0));
    assert_eq!(w.night.loudest.unwrap().at, V3::new(10.0, 10.0, 0.0));
    steps(&mut w, secs(4.0));
    w.make_noise(0, V3::new(10.0, 100.0, 0.0), crate::Loudness::Quiet);
    steps(&mut w, secs(1.0));
    assert_eq!(w.night.loudest.unwrap().at, V3::new(10.0, 100.0, 0.0));
}
