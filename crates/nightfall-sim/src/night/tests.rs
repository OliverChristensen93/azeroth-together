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
