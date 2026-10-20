//! The playtest settings turn into the rules and the roll a raid is played with, and the
//! default settings change nothing.

use std::sync::Arc;

use nightfall_sim::{
    Content, ContentSources, DeathLoss, KitDef, MonstersOut, NightSpeed, Playtest, RaidRoll,
    RivalKits, RivalsOut, World, DT,
};

fn shipped() -> Arc<Content> {
    Arc::new(
        Content::from_ron(ContentSources {
            classes: include_str!("../../../content/nightfall/classes.ron"),
            map: include_str!("../../../content/nightfall/map.ron"),
            rules: include_str!("../../../content/nightfall/rules.ron"),
            enemies: include_str!("../../../content/nightfall/enemies.ron"),
            spawns: include_str!("../../../content/nightfall/spawns.ron"),
            abilities: include_str!("../../../content/nightfall/abilities.ron"),
            items: include_str!("../../../content/nightfall/items.ron"),
            loot: include_str!("../../../content/nightfall/loot.ron"),
            monsters: include_str!("../../../content/nightfall/monsters.ron"),
            rivals: include_str!("../../../content/nightfall/rivals.ron"),
        })
        .unwrap_or_else(|e| panic!("{e}")),
    )
}

#[test]
fn the_default_settings_change_nothing() {
    let base = shipped();
    let p = Playtest::default();
    assert!(p.is_default());
    let c = p.content_for(&base).unwrap();
    assert!(
        Arc::ptr_eq(&c, &base),
        "the content comes back as it was, not a copy"
    );
    for seed in 0..60 {
        assert_eq!(p.roll(&c, seed), RaidRoll::roll(&base, seed), "seed {seed}");
    }
    assert_eq!(p.night_factor(), 1.0);
    assert_eq!(p.to_text(), "", "nothing is saved for the defaults");
    assert_eq!(Playtest::from_text(""), p);
    // A world under them is the plain world: the clock runs one second a second.
    let mut w = World::new(c);
    for _ in 0..30 {
        w.step();
    }
    assert!((w.elapsed - 30.0 * DT).abs() < 1e-4);
}

#[test]
fn the_death_rule_is_the_settings_and_only_when_set() {
    let base = shipped();
    assert_eq!(base.rules.run.death_loss, DeathLoss::EverythingOneRecovery);
    for d in [
        DeathLoss::Everything,
        DeathLoss::EverythingOneRecovery,
        DeathLoss::FoundOnly,
    ] {
        let p = Playtest {
            death_loss: Some(d),
            ..Default::default()
        };
        assert_eq!(p.content_for(&base).unwrap().rules.run.death_loss, d);
    }
    // The base is left alone: a copy was changed.
    assert_eq!(base.rules.run.death_loss, DeathLoss::EverythingOneRecovery);
}

#[test]
fn rivals_are_rolled_off_or_exactly_one() {
    let base = shipped();
    let off = Playtest {
        rivals: RivalsOut::Off,
        ..Default::default()
    };
    let one = Playtest {
        rivals: RivalsOut::One,
        ..Default::default()
    };
    let (c_off, c_one) = (
        off.content_for(&base).unwrap(),
        one.content_for(&base).unwrap(),
    );
    let mut rolled_two = false;
    for seed in 0..80 {
        assert!(off.roll(&c_off, seed).rivals.is_empty(), "seed {seed}");
        assert_eq!(one.roll(&c_one, seed).rivals.len(), 1, "seed {seed}");
        rolled_two |= RaidRoll::roll(&base, seed).rivals.len() == 2;
    }
    assert!(
        rolled_two,
        "the plain roll does bring two parties sometimes"
    );
    // Everything else about the raid is the plain roll's.
    let plain = RaidRoll::roll(&base, 7);
    let mut mine = off.roll(&c_off, 7);
    mine.rivals = plain.rivals.clone();
    assert_eq!(mine, plain);
}

#[test]
fn monsters_are_rolled_all_or_only_the_ones_that_always_come() {
    let base = shipped();
    let always: Vec<u8> = base
        .monsters
        .iter()
        .enumerate()
        .filter(|(_, m)| m.always)
        .map(|(i, _)| i as u8)
        .collect();
    let can: Vec<u8> = (0..base.monsters.len() as u8).collect();
    let none = Playtest {
        monsters: MonstersOut::None,
        ..Default::default()
    };
    let all = Playtest {
        monsters: MonstersOut::All,
        ..Default::default()
    };
    let (c_none, c_all) = (
        none.content_for(&base).unwrap(),
        all.content_for(&base).unwrap(),
    );
    for seed in 0..40 {
        assert_eq!(none.roll(&c_none, seed).monsters, always, "seed {seed}");
        assert_eq!(all.roll(&c_all, seed).monsters, can, "seed {seed}");
    }
}

#[test]
fn full_kits_give_every_kind_its_base_classs_whole_bar() {
    let base = shipped();
    let p = Playtest {
        rival_kits: RivalKits::Full,
        ..Default::default()
    };
    let full = p.content_for(&base).unwrap();
    assert!(
        base.rivals
            .kinds
            .iter()
            .any(|k| matches!(k.kit, KitDef::Abilities(_))),
        "the shipped kinds are mostly simplified"
    );
    for (before, after) in base.rivals.kinds.iter().zip(&full.rivals.kinds) {
        assert!(matches!(after.kit, KitDef::Class(_)), "{}", after.key);
        let hero = &full.classes[full.class(&after.base).unwrap()];
        let class = &full.classes[after.class];
        for slot in &hero.bar {
            assert!(class.bar.contains(slot), "{}: has {slot:?}", after.key);
        }
        // The kind keeps its own look, health and gear, and its big move stays on the bar.
        assert_eq!(class.name, base.classes[before.class].name);
        assert_eq!(class.base_hp, base.classes[before.class].base_hp);
        assert_eq!(after.gear, before.gear);
        assert_eq!(after.big_ability, before.big_ability);
        if let Some(b) = after.big.as_ref() {
            assert!(
                class.bar.iter().any(|(_, k)| k == b),
                "{}: big move",
                after.key
            );
        }
        assert!(class.kit.len() >= hero.kit.len());
    }
    // The kinds that were already full are the same; the base is untouched.
    for (before, after) in base.rivals.kinds.iter().zip(&full.rivals.kinds) {
        if matches!(before.kit, KitDef::Class(_)) {
            assert_eq!(
                base.classes[before.class].bar,
                full.classes[after.class].bar
            );
        }
    }
    let robber = base.rival_kind("grave_robber").unwrap();
    assert!(matches!(
        base.rivals.kinds[robber].kit,
        KitDef::Abilities(_)
    ));
}

#[test]
fn a_raid_under_every_setting_at_once_begins() {
    let base = shipped();
    let p = Playtest {
        death_loss: Some(DeathLoss::FoundOnly),
        rival_kits: RivalKits::Full,
        rivals: RivalsOut::One,
        monsters: MonstersOut::All,
        night_speed: NightSpeed::X4,
        weather: Some("rain".into()),
    };
    let c = p.content_for(&base).unwrap();
    let roll = p.roll(&c, 11);
    assert_eq!(c.rules.night.weather[usize::from(roll.weather)].key, "rain");
    let mut w = World::new(c.clone());
    w.begin_raid(roll).unwrap();
    w.set_night_speed(p.night_factor());
    w.add_hero(0, "rogue", "A", 1, 0).unwrap();
    for _ in 0..30 {
        w.step();
    }
    assert!((w.elapsed - 30.0 * DT * 4.0).abs() < 1e-3);
}

#[test]
fn the_weather_is_the_settings_or_the_rolled_one() {
    let base = shipped();
    let keys: Vec<String> = base
        .rules
        .night
        .weather
        .iter()
        .map(|w| w.key.clone())
        .collect();
    assert!(keys.len() >= 3);
    for (i, key) in keys.iter().enumerate() {
        let p = Playtest {
            weather: Some(key.clone()),
            ..Default::default()
        };
        assert_eq!(p.roll(&base, 3).weather, i as u8);
    }
    // A key the content lacks leaves the rolled weather.
    let p = Playtest {
        weather: Some("hail".into()),
        ..Default::default()
    };
    assert_eq!(p.roll(&base, 3), RaidRoll::roll(&base, 3));
    // The panel's ring: rolled, each in turn, rolled again.
    let mut p = Playtest::default();
    let mut seen = Vec::new();
    for _ in 0..=keys.len() {
        p.cycle_weather(&base);
        seen.push(p.weather.clone());
    }
    let mut want: Vec<Option<String>> = keys.iter().cloned().map(Some).collect();
    want.push(None);
    assert_eq!(seen, want);
}

#[test]
fn the_death_rule_cycles_through_the_content_and_the_three() {
    let mut p = Playtest::default();
    let mut seen = Vec::new();
    for _ in 0..4 {
        p.cycle_death_loss();
        seen.push(p.death_loss);
    }
    assert_eq!(
        seen,
        vec![
            Some(DeathLoss::Everything),
            Some(DeathLoss::EverythingOneRecovery),
            Some(DeathLoss::FoundOnly),
            None
        ]
    );
}

#[test]
fn the_settings_save_and_read_back_and_a_bad_line_keeps_its_default() {
    let p = Playtest {
        death_loss: Some(DeathLoss::FoundOnly),
        rival_kits: RivalKits::Full,
        rivals: RivalsOut::Off,
        monsters: MonstersOut::None,
        night_speed: NightSpeed::X2,
        weather: Some("mist".into()),
    };
    assert_eq!(Playtest::from_text(&p.to_text()), p);
    let odd = Playtest::from_text(
        "rivals=sometimes\nnight_speed=9\ndeath_loss=maybe\nnot a setting\nmonsters=all\n\nweather=\n",
    );
    assert_eq!(
        odd,
        Playtest {
            monsters: MonstersOut::All,
            ..Default::default()
        }
    );
}

#[test]
fn a_night_speed_that_is_not_a_number_above_zero_is_one() {
    let base = shipped();
    for bad in [0.0, -2.0, f32::NAN, f32::INFINITY] {
        let mut w = World::new(base.clone());
        w.set_night_speed(bad);
        w.step();
        assert!((w.elapsed - DT).abs() < 1e-6, "{bad}");
    }
}
