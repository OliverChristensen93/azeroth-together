//! The real `content/talents.ron`: it parses, every class has its start, the benchmark build fits
//! in a full set of points, and no build gets past the power budget.

use std::sync::Arc;

use together_sim::{
    Augment, Content, ContentSources, NodeKind, Tag, Tags, TalentEffect, TalentWeb, World,
};

fn content() -> Content {
    Content::from_ron(ContentSources {
        classes: include_str!("../../../content/classes.ron"),
        abilities: include_str!("../../../content/abilities.ron"),
        enemies: include_str!("../../../content/enemies.ron"),
        zones: include_str!("../../../content/zones.ron"),
    })
    .and_then(|c| c.with_rules(include_str!("../../../content/rules.ron")))
    .unwrap_or_else(|e| panic!("content/ is invalid: {e}"))
}

fn web() -> TalentWeb {
    TalentWeb::from_ron(include_str!("../../../content/talents.ron"))
        .unwrap_or_else(|e| panic!("{e}"))
}

/// How much a node is worth to a spell with `tags`.
fn power(web: &TalentWeb, n: usize, tags: Tags) -> f32 {
    web.nodes[n].effects.iter().map(|e| e.power(tags)).sum()
}

/// A strong allocation by `worth`, about as a player chasing power would make it:
/// again and again, the path with the most power per point, until the points run out.
fn greedy_by(
    web: &TalentWeb,
    class: &str,
    points: usize,
    worth: impl Fn(usize) -> f32,
) -> (Vec<usize>, f32) {
    let mut taken: Vec<usize> = Vec::new();
    loop {
        let mut best: Option<(f32, Vec<usize>)> = None;
        for n in 0..web.nodes.len() {
            let Some(path) = web.path_to(class, &taken, n) else {
                continue;
            };
            if taken.len() + path.len() > points {
                continue;
            }
            let value: f32 = path.iter().map(|&p| worth(p)).sum();
            let rate = value / path.len() as f32;
            if value > 0.0 && best.as_ref().is_none_or(|b| rate > b.0) {
                best = Some((rate, path));
            }
        }
        match best {
            Some((_, path)) => taken.extend(path),
            None => break,
        }
    }
    let total = taken.iter().map(|&n| worth(n)).sum();
    (taken, total)
}

#[test]
fn the_web_is_valid_and_every_class_has_a_start() {
    let web = web();
    let c = content();
    for class in &c.classes {
        assert!(
            web.start_of(&class.key).is_some(),
            "{} has no start in talents.ron",
            class.key
        );
    }
    assert!(
        (150..=250).contains(&web.nodes.len()),
        "{} nodes",
        web.nodes.len()
    );
    for n in &web.nodes {
        assert!(!n.name.is_empty(), "{} has no name", n.id);
        if n.kind != NodeKind::Start {
            assert!(!n.effects.is_empty(), "{} gives nothing", n.id);
            assert!(!n.lines().is_empty());
        }
    }
    let keystones = web
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Keystone)
        .count();
    assert!(keystones >= 5, "{keystones} keystones");
}

#[test]
fn nodes_do_not_sit_on_each_other() {
    let web = web();
    for (i, a) in web.nodes.iter().enumerate() {
        for b in &web.nodes[i + 1..] {
            let d = ((a.pos.0 - b.pos.0).powi(2) + (a.pos.1 - b.pos.1).powi(2)).sqrt();
            assert!(d >= 40.0, "{} and {} are {d:.0} apart", a.id, b.id);
        }
    }
}

#[test]
fn steering_names_general_augments_and_streaks_have_spells() {
    let web = web();
    let c = content();
    for n in &web.nodes {
        for e in &n.effects {
            match e {
                TalentEffect::Unlock(a, _) | TalentEffect::AugmentWeight(a, _) => {
                    assert!(
                        Augment::GENERAL.contains(a),
                        "{}: {a:?} is a capstone",
                        n.id
                    )
                }
                TalentEffect::CritStreak(tag) | TalentEffect::OfferWeight(tag, _) => assert!(
                    c.abilities.iter().any(|a| a.tag_set.has(*tag)),
                    "{}: no spell is {tag:?}",
                    n.id
                ),
                _ => {}
            }
        }
    }
}

/// `docs/DESIGN-IDEAS.md` section D: slows that Freeze and Shatter on the Frost side, Ignite on
/// the Fire side, and the Frostfire notable between, in reach of the mage with points to spare.
#[test]
fn the_frostfire_benchmark_is_in_reach() {
    let web = web();
    let want = [
        "mage_p_notable",
        "winter_notable",
        "mage_n_notable",
        "mage_deep",
    ];
    let mut taken = Vec::new();
    for id in want {
        let n = web.node(id).unwrap_or_else(|| panic!("no {id}"));
        if let Some(path) = web.path_to("mage", &taken, n) {
            taken.extend(path);
        }
    }
    let b = web.bonuses(&taken);
    assert!(b.chill_freeze > 0.0 && b.frozen_crit > 0.0);
    assert!(b.crit_burn.iter().any(|&(t, _)| t == Tag::Fire));
    assert!(
        taken.len() as u32 <= web.points.max / 2,
        "the benchmark takes {} of {} points",
        taken.len(),
        web.points.max
    );
}

#[test]
fn every_keystone_is_in_reach_of_some_class() {
    let web = web();
    let c = content();
    for (k, n) in web.nodes.iter().enumerate() {
        if n.kind != NodeKind::Keystone {
            continue;
        }
        let nearest = c
            .classes
            .iter()
            .filter_map(|cl| web.path_to(&cl.key, &[], k).map(|p| p.len()))
            .min()
            .unwrap();
        assert!(nearest <= 10, "{} is {nearest} points from any class", n.id);
    }
}

/// The power budget (`docs/DESIGN-IDEAS.md` 9i): talents and gear together add at most about
/// 40% late in progression, when the web meets in-run picks that fit it. A run is a handful of
/// spells, so the check is a full web chased greedily for a class's whole spell pool (each node
/// worth its average over the pool): that stays well inside the budget, leaving gear its share.
/// Chasing one spell alone, with every condition counted as met, is an upper bound that may go
/// higher, but not without limit.
#[test]
fn a_full_web_stays_within_the_power_budget() {
    let web = web();
    let c = content();
    let points = web.points.max as usize;
    let mut worst_class = 0.0f32;
    let mut worst_spell = 0.0f32;
    let mut sum = 0.0;
    for class in &c.classes {
        let pool: Vec<Tags> = class
            .pool_ix
            .iter()
            .map(|&a| c.abilities[a].tag_set)
            .collect();
        let (taken, total) = greedy_by(&web, &class.key, points, |n| {
            pool.iter().map(|&t| power(&web, n, t)).sum::<f32>() / pool.len() as f32
        });
        assert!(taken.len() <= points && !taken.is_empty());
        println!("{:8} whole pool {:.2}", class.key, total);
        worst_class = worst_class.max(total);
        sum += total;
        for &a in &class.pool_ix {
            let tags = c.abilities[a].tag_set;
            let (_, one) = greedy_by(&web, &class.key, points, |n| power(&web, n, tags));
            worst_spell = worst_spell.max(one);
        }
    }
    let mean = sum / c.classes.len() as f32;
    println!("whole pool: best {worst_class:.2}, mean {mean:.2}; one spell: best {worst_spell:.2}");
    assert!(
        worst_class <= 0.50,
        "a class's build reaches {worst_class:.2}"
    );
    assert!(mean >= 0.12, "the web is worth too little: {mean:.2}");
    assert!(worst_spell <= 0.70, "one spell reaches {worst_spell:.2}");
}

/// Every class with a full web of its own runs a short game without trouble.
#[test]
fn every_class_plays_with_a_full_web() {
    let web = web();
    let c = Arc::new(content());
    for class in &c.classes {
        let start = web.start_of(&class.key).unwrap();
        let mut world = World::new(c.clone(), &c.zones[0].key, 3).unwrap();
        world.add_hero(0, &class.key).unwrap();
        // Everything near the start, breadth first, as far as the points go.
        let mut order = vec![start];
        let mut i = 0;
        while i < order.len() {
            for &j in &web.nodes[order[i]].adjacent {
                if !order.contains(&j) && web.nodes[j].kind != NodeKind::Start {
                    order.push(j);
                }
            }
            i += 1;
        }
        let given = world.apply_talents(0, &web, &order[1..]);
        assert_eq!(given as u32, web.points.max, "{}", class.key);
        for _ in 0..600 {
            world.step();
            for h in 0..world.heroes.len() {
                let player = world.heroes[h].player;
                if !world.heroes[h].offers.is_empty() {
                    world.set_input(
                        player,
                        together_sim::PlayerInput {
                            pick: Some(0),
                            ..Default::default()
                        },
                    );
                }
            }
        }
        assert!(world.elapsed > 1.0);
    }
}
