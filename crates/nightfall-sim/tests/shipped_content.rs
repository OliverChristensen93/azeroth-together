//! The content Nightfall ships (`content/nightfall/`) loads and holds what the game expects.

use nightfall_sim::{Content, ContentSources, V2};

pub fn shipped() -> Content {
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

#[test]
fn duskwoods_creatures_are_placed_inside_it() {
    let c = shipped();
    assert!(c.enemy("nightbane_worgen").is_some());
    assert!(!c.spawns.is_empty());
    assert_eq!(c.rules.world.share_sight_within, 10.0);
}

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
                z: 36.5,
            }),
            ..Default::default()
        },
    );
    w.step();
    let h = w.hero(0).unwrap();
    assert!(content.map.bounds.contains(h.pos) || content.map.bounds.clamp(h.pos) == h.pos);
    assert_ne!(h.pos, far, "a pose far off the map was kept inside it");
}

#[test]
fn the_shipped_senses_are_the_specs_and_the_rogue_stealths() {
    let c = shipped();
    let s = &c.rules.senses;
    assert_eq!(s.notice_range, 35.0);
    assert_eq!(s.stealth_speed, 0.7);
    assert_eq!(s.visibility.sneak_still, 8.0);
    assert_eq!(s.awareness.lose_after, 5.0);
    assert!(c.classes[c.class("rogue").unwrap()].stealth);
    assert!(!c.classes[c.class("mage").unwrap()].stealth);
    assert!(c
        .spawns
        .iter()
        .any(|sp| matches!(sp.behaviour, nightfall_sim::Behaviour::Sleeper { .. })));
}

#[test]
fn the_shipped_kits_are_on_their_bars_and_enemies_fight_back() {
    use nightfall_sim::{Aim, ClassDef, Effect, Moving};

    let c = shipped();
    let rogue = &c.classes[c.class("rogue").unwrap()];
    let mage = &c.classes[c.class("mage").unwrap()];
    let on_bar =
        |class: &ClassDef, key: &str| class.kit.iter().any(|&(_, a)| c.abilities[a].key == key);
    for key in [
        "stealth",
        "sap",
        "cheap_shot",
        "garrote",
        "ambush",
        "sinister_strike",
        "eviscerate",
        "gouge",
        "sprint",
        "distract",
        "vanish",
        "throw_stone",
        "eat_and_drink",
    ] {
        assert!(on_bar(rogue, key), "rogue: {key}");
    }
    for key in [
        "frostbolt",
        "fireball",
        "fire_blast",
        "frost_nova",
        "arcane_missiles",
        "polymorph",
        "blink",
        "conjure_water",
        "throw_stone",
        "eat_and_drink",
    ] {
        assert!(on_bar(mage, key), "mage: {key}");
    }
    let a = |key: &str| &c.abilities[c.ability(key).unwrap()];
    assert_eq!(a("frostbolt").cast, 2.5);
    assert_eq!(a("frostbolt").moving, Moving::Slowed);
    assert_eq!(a("fireball").moving, Moving::Rooted);
    assert_eq!(a("arcane_missiles").moving, Moving::Rooted);
    assert_eq!(a("blink").cooldown, 15.0);
    assert_eq!(a("throw_stone").range, 25.0);
    assert_eq!(a("throw_stone").cooldown, 20.0);
    assert_eq!(a("throw_stone").aim, Aim::Ground);
    assert!(a("vanish").has(|e| matches!(e, Effect::Vanish)));
    assert_eq!(c.rules.combat.downed_for, 45.0);
    assert_eq!(rogue.power_max, 100.0);
    assert!(rogue.swing.is_some());
    assert!(mage.power_max > 0.0);
    let weaver = &c.enemies[c.enemy("nightbane_shadow_weaver").unwrap()];
    assert!(!weaver.kit.is_empty());
    assert!(c.enemies.iter().filter(|e| e.elite).count() >= 2);
    for e in c.enemies.iter().filter(|e| e.elite) {
        assert!(
            e.kit
                .iter()
                .any(|&(m, _)| c.abilities[m].telegraph.is_some()),
            "{}",
            e.key
        );
    }
}

#[test]
fn the_shipped_content_has_its_items_loot_exits_and_kits() {
    let c = shipped();
    assert!(c.items.len() >= 20);
    assert!(c.conjured.is_some());
    assert!(c.map.exits.len() >= 3);
    assert!(c.map.exits.iter().any(|e| e.late));
    assert!(!c.loot.containers.is_empty());
    assert!(!c.loot.bodies.is_empty());
    for class in &c.classes {
        assert!(!class.free_items.is_empty(), "{} has a free kit", class.key);
    }
}

fn shipped_inn(c: &Content) -> nightfall_sim::InnContent {
    use nightfall_sim::{InnContent, InnSources};
    InnContent::from_ron(
        InnSources {
            hub: include_str!("../../../content/nightfall/hub.ron"),
            contracts: include_str!("../../../content/nightfall/contracts.ron"),
            stations: include_str!("../../../content/nightfall/stations.ron"),
        },
        c,
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_shipped_inn_loads_against_the_shipped_raid() {
    let c = shipped();
    let inn = shipped_inn(&c);
    assert_eq!(inn.place.map, 0);
    assert!(inn.contracts.len() >= 3);
    assert!(inn.stations.iter().any(|s| s.open));
    assert!(!inn.companions.is_empty());
    assert!(inn
        .trader
        .stock
        .iter()
        .all(|&(i, p)| i < c.items.len() && p > 0));
}

/// A kill contract with a circle can only be done if some enemy of that kind spawns inside it,
/// or the contracts behind it (the kennel's, and so the companion) can never be reached.
#[test]
fn every_shipped_kill_contract_has_its_enemies_inside_its_circle() {
    use nightfall_sim::Objective;
    let c = shipped();
    let inn = shipped_inn(&c);
    for contract in &inn.contracts {
        for o in &contract.objectives {
            let Objective::Kill {
                enemy,
                count,
                near: Some((at, radius)),
            } = o
            else {
                continue;
            };
            let kind = &c.enemies[*enemy].key;
            let near: u32 = c
                .spawns
                .iter()
                .filter(|s| &s.kind == kind)
                .filter(|s| {
                    let place = match &s.behaviour {
                        nightfall_sim::Behaviour::Sentry { at, .. }
                        | nightfall_sim::Behaviour::Sleeper { at, .. } => Some(*at),
                        nightfall_sim::Behaviour::Wander { center, .. } => Some(*center),
                        nightfall_sim::Behaviour::Patrol { route, .. } => route.first().copied(),
                        // A road walk is a patrol by the time the content has loaded.
                        nightfall_sim::Behaviour::Road { .. } => None,
                    };
                    let Some((x, y)) = place else {
                        return false;
                    };
                    V2::new(x, y).distance(*at) <= *radius
                })
                .map(|s| s.count)
                .sum();
            assert!(
                near >= *count,
                "\"{}\": {near} {kind} spawn within its circle, {count} asked",
                contract.key
            );
        }
    }
}

/// Nothing in the shipped inn is out of reach: every contract can be offered once those it needs
/// are done, every station that is not open is unlocked by a contract, every recipe is taught by
/// a level, and every companion is granted by a gift.
#[test]
fn everything_in_the_shipped_inn_can_be_reached() {
    use nightfall_sim::Gift;
    let c = shipped();
    let inn = shipped_inn(&c);

    let mut done = vec![false; inn.contracts.len()];
    while let Some(i) =
        (0..done.len()).find(|&i| !done[i] && inn.contracts[i].needs.iter().all(|&n| done[n]))
    {
        done[i] = true;
    }
    for (i, d) in done.iter().enumerate() {
        assert!(d, "contract \"{}\" is never offered", inn.contracts[i].key);
    }

    for (i, s) in inn.stations.iter().enumerate() {
        assert!(
            s.open || inn.contracts.iter().any(|c| c.reward.unlocks.contains(&i)),
            "station \"{}\" is not open and no contract unlocks it",
            s.key
        );
    }

    let gifts: Vec<&Gift> = inn
        .stations
        .iter()
        .flat_map(|s| &s.levels)
        .flat_map(|l| &l.gives)
        .collect();
    for (i, r) in inn.recipes.iter().enumerate() {
        assert!(
            gifts.contains(&&Gift::Recipe(i)),
            "recipe \"{}\" is taught by no level",
            r.key
        );
    }
    for (i, k) in inn.companions.iter().enumerate() {
        assert!(
            gifts.contains(&&Gift::Companion(i)),
            "companion \"{}\" is granted by no gift",
            k.key
        );
    }
}

/// Buying from the trader and selling it straight back must never make gold: the trader pays
/// less for an item than it asks, one at a time and by the full stack.
#[test]
fn buying_from_the_shipped_trader_and_selling_back_never_profits() {
    use nightfall_sim::Profile;
    let c = shipped();
    let inn = shipped_inn(&c);
    for &(item, price) in &inn.trader.stock {
        let def = &c.items[item];
        assert!(
            price as f32 > def.value as f32 * inn.trader.sells_for,
            "\"{}\" costs {price} and sells back for more",
            def.key
        );
        let count = def.stack.max(1);
        let mut p = Profile {
            gold: price * u32::from(count),
            ..Profile::default()
        };
        let start = p.gold;
        let line = inn.trader.stock.iter().position(|s| s.0 == item).unwrap();
        for _ in 0..count {
            p.buy(line, &inn, &c).unwrap();
        }
        assert_eq!(p.gold, 0);
        assert_eq!(p.stash.len(), 1, "\"{}\" fills one stack", def.key);
        match p.sell_to_trader(0, &inn, &c) {
            Ok(gold) => assert!(gold < start, "\"{}\": {start} in, {gold} back", def.key),
            // Worth nothing in gold: it fetches nothing at all.
            Err(e) => assert_eq!(e, nightfall_sim::StationError::Worthless),
        }
    }
}

/// Nothing the trader sells is a material a recipe needs: otherwise gold could buy materials,
/// craft them into something worth more, and sell that, a loop that needs no raid.
#[test]
fn no_shipped_trader_stock_is_a_recipe_material() {
    let c = shipped();
    let inn = shipped_inn(&c);
    for &(item, _) in &inn.trader.stock {
        for r in &inn.recipes {
            assert!(
                r.needs.iter().all(|&(n, _)| n != item),
                "the trader sells \"{}\", which \"{}\" needs",
                c.items[item].key,
                r.key
            );
        }
    }
}

#[test]
fn the_night_is_in_the_shipped_content() {
    let c = shipped();
    let n = &c.rules.night;
    assert_eq!(n.stages.len(), 5);
    assert!(n.stages[1..]
        .iter()
        .all(|s| s.cue.is_some() && s.line.is_some()));
    assert!(n.weather.len() >= 3);
    for key in ["morladim", "trackers", "banshee", "dark_riders", "stitches"] {
        assert!(c.monster(key).is_some(), "{key}");
    }
    assert!(c.monsters[c.monster("stitches").unwrap()].always);
    assert!(!c.map.roads.is_empty() && !c.map.zones.is_empty() && !c.map.flocks.is_empty());
    assert!(c.enemies[c.enemy("stitches").unwrap()].elite);
    assert!(c.enemies[c.enemy("morladim").unwrap()].watched.is_some());
    assert!(
        c.spawns
            .iter()
            .any(|s| s.stage_at.is_some() && s.monster.is_none()),
        "the howling brings more wanderers"
    );
}

#[test]
fn the_westfall_road_is_open_from_the_start_in_every_raid() {
    // Ladim's tome is handed in at the Westfall road: no roll may shut it, open it late or
    // close it with the hunt.
    let c = shipped();
    let road = c
        .map
        .exits
        .iter()
        .position(|e| e.key == "westfall_road")
        .expect("the Westfall road");
    assert!(c.map.exits[road].pinned);
    let mut hunt_closed_another = false;
    for seed in 0..2000 {
        let r = nightfall_sim::RaidRoll::roll(&c, seed);
        assert!(r.exit_open(road, 0.0), "seed {seed}");
        assert!(
            r.closes.is_none_or(|(x, _)| usize::from(x) != road),
            "seed {seed}"
        );
        hunt_closed_another |= r.closes.is_some();
    }
    assert!(hunt_closed_another, "the hunt still closes other exits");
}
