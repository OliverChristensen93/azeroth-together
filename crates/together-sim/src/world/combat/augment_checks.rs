//! One check per augment that `combat.rs`'s own tests leave out: each casts the spell with and
//! without the augment in an emptied arena and asserts the difference its description promises.
//! Crits are off unless a check needs them, so damage compares exactly.

use super::*;
use crate::content::ContentSources;

fn shipped() -> Arc<Content> {
    Arc::new(
        Content::from_ron(ContentSources {
            classes: include_str!("../../../../../content/classes.ron"),
            abilities: include_str!("../../../../../content/abilities.ron"),
            enemies: include_str!("../../../../../content/enemies.ron"),
            zones: include_str!("../../../../../content/zones.ron"),
        })
        .expect("shipped content is valid"),
    )
}

/// A mage alone in an emptied arena, facing +x, who never crits.
fn mage() -> World {
    let mut w = World::new(shipped(), "elwynn", 5).unwrap();
    w.add_hero(0, "mage").unwrap();
    w.enemies.clear();
    w.interactables.clear();
    w.camps.clear();
    w.pickups.clear();
    w.spawn_timer = f32::MAX;
    w.next_boss = f32::MAX;
    w.heroes[0].max_hp = 1e6;
    w.heroes[0].hp = 1e6;
    w.heroes[0].facing = V2::new(1.0, 0.0);
    w.heroes[0].stats.add(Stat::Crit, -1.0);
    w
}

fn ability(w: &World, key: &str) -> usize {
    w.content.ability(key).unwrap()
}

/// The mage's spells: each at `rank` with these augments, ready to cast.
fn give(w: &mut World, list: &[(&str, u8, &[Augment])]) {
    w.heroes[0].abilities = list
        .iter()
        .map(|&(key, rank, augments)| Owned {
            level: rank,
            augments: augments.to_vec(),
            cd: 0.0,
            ..Owned::new(ability(w, key), 0.0)
        })
        .collect();
}

/// A standing enemy with `hp`, `dx` and `dy` yards from the hero, that neither moves nor strikes.
fn foe(w: &mut World, dx: f32, dy: f32, hp: f32) -> u32 {
    let pos = w.heroes[0].pos + V2::new(dx, dy);
    let id = w.id();
    w.enemies.push(Enemy {
        id,
        def: 0,
        pos,
        facing: V2::new(1.0, 0.0),
        moving: false,
        hp,
        max_hp: hp,
        damage: 0.0,
        speed: 0.0,
        radius: 0.5,
        elite: false,
        boss: false,
        attack_cd: 99.0,
        last_hit: None,
        home: None,
        status: Status::default(),
        act: Default::default(),
    });
    id
}

fn foe_mut(w: &mut World, id: u32) -> &mut Enemy {
    w.enemies.iter_mut().find(|e| e.id == id).unwrap()
}

fn run(w: &mut World, secs: f32) -> Vec<Event> {
    let mut seen = Vec::new();
    for _ in 0..(secs / DT).round() as usize {
        w.step();
        seen.extend(w.drain_events());
    }
    seen
}

/// Health enemy `id` has lost: all of it once it is dead and gone.
fn lost(w: &World, id: u32, max: f32) -> f32 {
    w.enemy(id).map_or(max, |e| e.max_hp - e.hp)
}

fn casts_of(events: &[Event], ability: usize) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Cast { ability: a, .. } if *a == ability))
        .count()
}

fn crits(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Damage { crit: true, .. }))
        .count()
}

/// The cast of `key` with these augments, as hero 0 would make it.
fn spec(w: &World, key: &str, augments: &[Augment]) -> CastSpec {
    let owned = Owned {
        augments: augments.to_vec(),
        ..Owned::new(ability(w, key), 0.0)
    };
    w.spell_spec(0, &owned)
}

/// What one enemy `dx` yards ahead loses in `secs` to `key` with these augments, set up by
/// `prepare`.
fn dealt(key: &str, augments: &[Augment], secs: f32, prepare: impl Fn(&mut Enemy)) -> f32 {
    let mut w = mage();
    give(&mut w, &[(key, 10, augments)]);
    let id = foe(&mut w, 5.0, 0.0, 1e6);
    prepare(foe_mut(&mut w, id));
    run(&mut w, secs);
    lost(&w, id, 1e6)
}

/// A burn on an enemy, as if a Fire spell had left it.
fn burning(e: &mut Enemy, dps: f32) {
    e.status.burns.push(Burn {
        by: 0,
        ability: 0,
        dps,
        left: 4.0,
        tick: 1.0,
        every: 1.0,
        contagion: false,
    });
}

// ----- General augments -----

#[test]
fn frostfire_makes_frostbolt_burn() {
    let mut w = mage();
    give(&mut w, &[("frostbolt", 5, &[Augment::Frostfire])]);
    let id = foe(&mut w, 6.0, 0.0, 1e6);
    run(&mut w, 0.8);
    assert!(w.enemy(id).unwrap().status.burning());
}

#[test]
fn number_augments_change_their_numbers() {
    let w = mage();
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3 * b.abs().max(1.0);
    let (base, s) = (
        spec(&w, "fireball", &[]),
        spec(&w, "fireball", &[Augment::Spellfire]),
    );
    assert!(close(s.radius, base.radius * 1.15) && s.tags.has(Tag::Arcane));
    let (base, s) = (
        spec(&w, "frostbolt", &[]),
        spec(&w, "frostbolt", &[Augment::Spellfrost]),
    );
    assert!(close(s.cooldown, base.cooldown * 0.85) && s.tags.has(Tag::Arcane));
    let (base, s) = (
        spec(&w, "frost_nova", &[]),
        spec(&w, "frost_nova", &[Augment::Wide]),
    );
    assert!(close(s.radius, base.radius * 1.4) && close(s.damage, base.damage * 0.8));
    let s = spec(&w, "frost_nova", &[Augment::Concentrated]);
    assert!(close(s.radius, base.radius * 0.7) && close(s.damage, base.damage * 1.4));
    let (base, s) = (
        spec(&w, "frostbolt", &[]),
        spec(&w, "frostbolt", &[Augment::Heavy]),
    );
    assert!(close(s.damage, base.damage * 1.6) && close(s.cooldown, base.cooldown * 1.4));
    let s = spec(&w, "frostbolt", &[Augment::Hasted]);
    assert!(close(s.damage, base.damage * 0.85) && close(s.cooldown, base.cooldown * 0.75));
    let s = spec(&w, "frostbolt", &[Augment::Multishot]);
    assert!(s.count == base.count + 2 && close(s.damage, base.damage * 0.75));
    let (base, s) = (
        spec(&w, "blizzard", &[]),
        spec(&w, "blizzard", &[Augment::Lingering]),
    );
    assert!(close(s.ground, base.ground * 1.5));
}

#[test]
fn multishot_hits_three_enemies_with_one_cast() {
    let hit = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frostbolt", 5, augments)]);
        let ids = [
            foe(&mut w, 6.0, 0.0, 1e6),
            foe(&mut w, 0.0, 7.0, 1e6),
            foe(&mut w, -8.0, 0.0, 1e6),
        ];
        run(&mut w, 0.8);
        ids.iter().filter(|&&id| lost(&w, id, 1e6) > 0.0).count()
    };
    assert_eq!(hit(&[]), 1);
    assert_eq!(hit(&[Augment::Multishot]), 3);
}

#[test]
fn piercing_passes_through_to_the_enemy_behind() {
    let behind = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frostbolt", 5, augments)]);
        foe(&mut w, 6.0, 0.0, 1e6);
        let back = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.8);
        lost(&w, back, 1e6)
    };
    assert_eq!(behind(&[]), 0.0);
    assert!(behind(&[Augment::Piercing]) > 0.0);
}

#[test]
fn returning_hits_its_target_again() {
    let plain = dealt("frostbolt", &[], 1.2, |_| {});
    let back = dealt("frostbolt", &[Augment::Returning], 1.2, |_| {});
    assert!(back > plain * 1.8, "{plain} vs {back}");
}

#[test]
fn ring_misses_the_middle_and_reaches_twelve_yards() {
    let hits = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frost_nova", 5, augments)]);
        let (near, far) = (foe(&mut w, 2.0, 0.0, 1e6), foe(&mut w, 11.0, 0.0, 1e6));
        run(&mut w, 0.3);
        (lost(&w, near, 1e6) > 0.0, lost(&w, far, 1e6) > 0.0)
    };
    assert_eq!(hits(&[]), (true, false));
    assert_eq!(hits(&[Augment::Ring]), (false, true));
}

#[test]
fn mobile_ground_follows_the_hero() {
    let mut w = mage();
    give(&mut w, &[("blizzard", 5, &[Augment::Mobile])]);
    foe(&mut w, 8.0, 0.0, 1e6);
    run(&mut w, 0.3);
    assert_eq!(w.grounds.len(), 1, "Blizzard was not placed");
    w.heroes[0].pos += V2::new(4.0, 3.0);
    run(&mut w, 0.2);
    assert!(w.grounds[0].pos.distance(w.heroes[0].pos) < 0.5);
}

#[test]
fn targeted_lands_on_a_crowd_away_from_the_hero() {
    let far = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("arcane_explosion", 5, augments)]);
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        run(&mut w, 0.5);
        lost(&w, id, 1e6)
    };
    assert_eq!(far(&[]), 0.0);
    assert!(far(&[Augment::Targeted]) > 0.0);
}

#[test]
fn contagion_passes_a_burn_on_when_its_enemy_dies() {
    let spread = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("pyroblast", 5, augments)]);
        // Pyroblast's hit leaves it alive; its burn finishes it.
        foe(&mut w, 6.0, 0.0, 80.0);
        let next = foe(&mut w, 7.5, 2.0, 1e6);
        let mut caught = false;
        for _ in 0..(5.0 / DT) as usize {
            w.step();
            w.drain_events();
            caught |= w.enemy(next).unwrap().status.burning();
        }
        caught
    };
    assert!(!spread(&[]));
    assert!(spread(&[Augment::Contagion]));
}

#[test]
fn quickened_burns_out_sooner_for_the_same_damage() {
    let after = |augments: &[Augment], secs: f32| {
        let mut w = mage();
        give(&mut w, &[("pyroblast", 5, augments)]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        run(&mut w, secs);
        let e = w.enemy(id).unwrap();
        (e.status.burning(), e.max_hp - e.hp)
    };
    assert!(after(&[], 3.7).0, "a plain burn lasts 4 seconds");
    assert!(!after(&[Augment::Quickened], 3.7).0);
    let (plain, quick) = (after(&[], 5.5).1, after(&[Augment::Quickened], 5.5).1);
    assert!((quick - plain).abs() < plain * 0.03, "{plain} vs {quick}");
}

#[test]
fn molten_burns_twice_as_long() {
    let burning_at = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("pyroblast", 5, augments)]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        run(&mut w, 5.5);
        w.enemy(id).unwrap().status.burning()
    };
    assert!(!burning_at(&[]));
    assert!(burning_at(&[Augment::Molten]));
}

#[test]
fn executioner_doubles_damage_on_the_wounded() {
    let low = |e: &mut Enemy| e.hp = e.max_hp * 0.2;
    // `dealt` counts the 80% it had already lost.
    let plain = dealt("frostbolt", &[], 0.8, low) - 0.8e6;
    let exec = dealt("frostbolt", &[Augment::Executioner], 0.8, low) - 0.8e6;
    assert!(
        (exec - plain * 2.0).abs() < 0.01 * plain,
        "{plain} vs {exec}"
    );
}

#[test]
fn ignite_leaves_burning() {
    let mut w = mage();
    give(&mut w, &[("frostbolt", 5, &[Augment::Ignite])]);
    let id = foe(&mut w, 6.0, 0.0, 1e6);
    run(&mut w, 0.8);
    assert!(w.enemy(id).unwrap().status.burning());
}

#[test]
fn deep_chill_sometimes_freezes() {
    let froze = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frostbolt", 5, augments)]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        let mut seen = false;
        for _ in 0..(40.0 / DT) as usize {
            w.step();
            w.drain_events();
            seen |= w.enemy(id).unwrap().status.freeze > 0.0;
        }
        seen
    };
    assert!(!froze(&[]));
    assert!(froze(&[Augment::DeepChill]));
}

#[test]
fn cold_bite_hits_chilled_enemies_harder() {
    let chilled = |e: &mut Enemy| e.status.chill = 100.0;
    let plain = dealt("frostbolt", &[], 0.8, chilled);
    let bite = dealt("frostbolt", &[Augment::ColdBite], 0.8, chilled);
    assert!(
        (bite - plain * 1.25).abs() < 0.01 * plain,
        "{plain} vs {bite}"
    );
}

#[test]
fn lasting_frost_freezes_a_second_longer() {
    let freeze = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frost_nova", 5, augments)]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.1);
        w.enemy(id).unwrap().status.freeze
    };
    let (plain, lasting) = (freeze(&[]), freeze(&[Augment::LastingFrost]));
    assert!((lasting - plain - 1.0).abs() < 0.05, "{plain} vs {lasting}");
}

#[test]
fn permafrost_leaves_them_chilled_after_the_thaw() {
    let chilled = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frost_nova", 5, augments)]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 3.0);
        let st = &w.enemy(id).unwrap().status;
        (st.freeze, st.chill)
    };
    assert_eq!(chilled(&[]), (0.0, 0.0));
    let (freeze, chill) = chilled(&[Augment::Permafrost]);
    assert!(freeze == 0.0 && chill > 0.0);
}

#[test]
fn shatter_crits_frozen_enemies() {
    let count = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frostbolt", 5, augments)]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        foe_mut(&mut w, id).status.freeze = 1000.0;
        crits(&run(&mut w, 12.0))
    };
    assert_eq!(count(&[]), 0);
    assert!(count(&[Augment::Shatter]) > 0);
}

#[test]
fn concussive_stuns_and_knocks_farther() {
    let after = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("blast_wave", 5, augments)]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.2);
        let stun = w.enemy(id).unwrap().status.stun;
        run(&mut w, 1.0);
        let e = w.enemy(id).unwrap();
        (stun, e.pos.distance(w.heroes[0].pos))
    };
    let (plain_stun, plain_far) = after(&[]);
    let (stun, far) = after(&[Augment::Concussive]);
    assert!(plain_stun == 0.0 && stun > 0.0);
    assert!(far > plain_far + 1.0, "{plain_far} vs {far}");
}

/// An enemy beside the hero that swings at it every moment it can.
fn striker(w: &mut World) -> u32 {
    let id = foe(w, 1.0, 0.0, 1e6);
    let e = foe_mut(w, id);
    e.damage = 20.0;
    e.attack_cd = 0.0;
    id
}

#[test]
fn reflective_deals_back_part_of_what_the_shield_absorbs() {
    let hurt = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("ice_barrier", 5, augments)]);
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 1000.0;
        let id = striker(&mut w);
        run(&mut w, 3.0);
        lost(&w, id, 1e6)
    };
    assert_eq!(hurt(&[]), 0.0);
    assert!(hurt(&[Augment::Reflective]) > 0.0);
}

#[test]
fn twin_charges_casts_twice_in_a_row() {
    let casts = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frost_nova", 5, augments)]);
        let nova = ability(&w, "frost_nova");
        foe(&mut w, 3.0, 0.0, 1e6);
        casts_of(&run(&mut w, 1.0), nova)
    };
    assert_eq!(casts(&[]), 1);
    assert_eq!(casts(&[Augment::TwinCharges]), 2);
}

#[test]
fn twin_casts_two_weaker_copies() {
    let plain = dealt("frostbolt", &[], 1.0, |_| {});
    let twin = dealt("frostbolt", &[Augment::Twin], 1.0, |_| {});
    assert!(
        (twin - plain * 1.2).abs() < 0.02 * plain,
        "{plain} vs {twin}"
    );
}

// ----- Capstones -----

/// Whether enemy `id` is Frozen at any step of `secs`.
fn froze_within(w: &mut World, id: u32, secs: f32) -> bool {
    let mut seen = false;
    for _ in 0..(secs / DT) as usize {
        w.step();
        w.drain_events();
        seen |= w.enemy(id).is_some_and(|e| e.status.freeze > 0.0);
    }
    seen
}

#[test]
fn glacial_spike_freezes_on_the_fourth_frostbolt() {
    let froze = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frostbolt", 10, augments)]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        froze_within(&mut w, id, 5.0)
    };
    assert!(!froze(&[]));
    assert!(froze(&[Augment::GlacialSpike]));
}

#[test]
fn winters_chill_stacks_on_its_target() {
    let mut w = mage();
    give(&mut w, &[("frostbolt", 10, &[Augment::WintersChill])]);
    let id = foe(&mut w, 6.0, 0.0, 1e6);
    run(&mut w, 3.0);
    assert!(w.enemy(id).unwrap().status.winters >= 2);
}

#[test]
fn critical_mass_adds_half_again_to_fire_crits() {
    let w = mage();
    let base = spec(&w, "fireball", &[]);
    let mut w = w;
    give(&mut w, &[("fireball", 10, &[Augment::CriticalMass])]);
    let s = w.spell_spec(0, &w.heroes[0].abilities[0].clone());
    assert!((s.crit_mult - base.crit_mult - 0.5).abs() < 1e-4);
}

#[test]
fn arcane_charge_builds_charges() {
    let mut w = mage();
    give(&mut w, &[("arcane_missiles", 10, &[Augment::ArcaneCharge])]);
    foe(&mut w, 6.0, 0.0, 1e6);
    run(&mut w, 2.5);
    assert!(w.heroes[0].spell.arcane >= 1);
}

#[test]
fn missile_storm_reaches_every_enemy_in_range() {
    let hit = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("arcane_missiles", 10, augments)]);
        let ids = [
            foe(&mut w, 5.0, 0.0, 1e6),
            foe(&mut w, -12.0, 0.0, 1e6),
            foe(&mut w, 0.0, 14.0, 1e6),
            foe(&mut w, 0.0, -16.0, 1e6),
        ];
        run(&mut w, 12.0);
        ids.iter().filter(|&&id| lost(&w, id, 1e6) > 0.0).count()
    };
    assert_eq!(hit(&[]), 1);
    assert_eq!(hit(&[Augment::MissileStorm]), 4);
}

#[test]
fn channelled_focus_fires_faster_standing_still() {
    let casts = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("arcane_missiles", 10, augments)]);
        let am = ability(&w, "arcane_missiles");
        foe(&mut w, 6.0, 0.0, 1e6);
        casts_of(&run(&mut w, 8.0), am)
    };
    let (plain, focus) = (casts(&[]), casts(&[Augment::ChannelledFocus]));
    assert!(focus as f32 >= plain as f32 * 1.6, "{plain} vs {focus}");
}

#[test]
fn frostbite_shatters_a_frozen_enemy_that_dies() {
    let splash = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("frost_nova", 10, augments)]);
        let frozen = foe(&mut w, 3.0, 0.0, 200.0);
        let near = foe(&mut w, 3.0, 2.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(frozen).unwrap().status.freeze > 0.0);
        // Something else finishes it while it is Frozen.
        let at = w.enemy(frozen).unwrap().pos;
        w.strike(
            0,
            &spec(&w, "frostbolt", &[]),
            frozen,
            100.0,
            at,
            Crit::Never,
        );
        run(&mut w, 0.2);
        lost(&w, near, 1e6)
    };
    let (plain, bite) = (splash(&[]), splash(&[Augment::Frostbite]));
    assert!(bite > plain + 50.0, "{plain} vs {bite}");
}

#[test]
fn frozen_core_saves_a_hero_at_low_health() {
    let mut w = mage();
    give(&mut w, &[("frost_nova", 10, &[Augment::FrozenCore])]);
    let nova = ability(&w, "frost_nova");
    w.heroes[0].abilities[0].cd = 60.0;
    w.heroes[0].max_hp = 1000.0;
    w.heroes[0].hp = 250.0;
    // It answers a blow that leaves the hero below 30%.
    striker(&mut w);
    let events = run(&mut w, 0.5);
    assert!(
        casts_of(&events, nova) >= 1,
        "Frost Nova did not cast itself"
    );
    assert!(w.heroes[0].spell.guard > 0.0);
}

#[test]
fn arcane_potency_primes_after_a_big_hit() {
    let mut w = mage();
    give(
        &mut w,
        &[("arcane_explosion", 10, &[Augment::ArcanePotency])],
    );
    for i in 0..16 {
        let a = i as f32 * std::f32::consts::TAU / 16.0;
        foe(&mut w, 3.0 * a.cos(), 3.0 * a.sin(), 1e6);
    }
    run(&mut w, 0.3);
    assert!(w.heroes[0].spell.potency);
}

/// The hero blinks `by` yards ahead, as the client sends it.
fn blink(w: &mut World, by: V2) {
    let pos = w.heroes[0].pos + by;
    w.queue_input(
        0,
        PlayerInput {
            pose: Some(HeroPose {
                pos,
                facing: V2::new(1.0, 0.0),
            }),
            dash: true,
            ..Default::default()
        },
    );
    w.step();
}

#[test]
fn blink_detonation_explodes_where_the_hero_left_and_landed() {
    let mut w = mage();
    give(
        &mut w,
        &[("arcane_explosion", 10, &[Augment::BlinkDetonation])],
    );
    let ae = ability(&w, "arcane_explosion");
    w.heroes[0].abilities[0].cd = 60.0;
    let (left, landed) = (foe(&mut w, 1.0, 0.0, 1e6), foe(&mut w, 15.0, 0.0, 1e6));
    blink(&mut w, V2::new(14.0, 0.0));
    let events = run(&mut w, 0.3);
    assert!(lost(&w, left, 1e6) > 0.0 && lost(&w, landed, 1e6) > 0.0);
    assert!(casts_of(&events, ae) >= 1);
}

#[test]
fn wake_of_winter_leaves_a_cone_behind() {
    let mut w = mage();
    give(&mut w, &[("cone_of_cold", 10, &[Augment::WakeOfWinter])]);
    w.heroes[0].abilities[0].cd = 60.0;
    let behind = foe(&mut w, 10.0, 0.0, 1e6);
    blink(&mut w, V2::new(14.0, 0.0));
    run(&mut w, 0.3);
    assert!(lost(&w, behind, 1e6) > 0.0);
}

#[test]
fn implosion_pulls_enemies_in_before_it_explodes() {
    let mut w = mage();
    give(&mut w, &[("arcane_explosion", 10, &[Augment::Implosion])]);
    let id = foe(&mut w, 10.0, 0.0, 1e6);
    run(&mut w, 0.3);
    let e = w.enemy(id).unwrap();
    assert!(e.pos.distance(w.heroes[0].pos) < 9.0);
    assert!(e.max_hp - e.hp > 0.0);
}

#[test]
fn final_gale_ends_in_a_frost_nova() {
    let mut w = mage();
    give(&mut w, &[("blizzard", 10, &[Augment::FinalGale])]);
    let nova = ability(&w, "frost_nova");
    foe(&mut w, 8.0, 0.0, 1e6);
    let events = run(&mut w, 8.0);
    assert_eq!(casts_of(&events, nova), 0, "it lasts 50% longer");
    let events = run(&mut w, 2.5);
    assert!(casts_of(&events, nova) >= 1);
}

#[test]
fn whiteout_exposes_enemies_inside() {
    let mut w = mage();
    give(&mut w, &[("blizzard", 10, &[Augment::Whiteout])]);
    let id = foe(&mut w, 8.0, 0.0, 1e6);
    run(&mut w, 1.5);
    assert!(w.enemy(id).unwrap().status.exposed > 0.0);
}

#[test]
fn frozen_orb_rolls_away() {
    let mut w = mage();
    give(&mut w, &[("blizzard", 10, &[Augment::FrozenOrb])]);
    foe(&mut w, 8.0, 0.0, 1e6);
    run(&mut w, 0.2);
    assert_eq!(w.grounds.len(), 1);
    let start = w.grounds[0].pos;
    run(&mut w, 1.0);
    assert!(w.grounds[0].pos.distance(start) > 1.0);
}

#[test]
fn firestorm_strikes_two_more_crowds() {
    let hit = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("flamestrike", 10, augments)]);
        let ids = [
            foe(&mut w, 2.0, 0.0, 1e6),
            foe(&mut w, 14.0, 0.0, 1e6),
            foe(&mut w, -14.0, 0.0, 1e6),
        ];
        run(&mut w, 0.5);
        ids.iter().filter(|&&id| lost(&w, id, 1e6) > 0.0).count()
    };
    assert_eq!(hit(&[]), 1);
    assert_eq!(hit(&[Augment::Firestorm]), 3);
}

#[test]
fn meteor_hits_three_times_as_hard_and_stuns() {
    let w = mage();
    let (base, s) = (
        spec(&w, "flamestrike", &[]),
        spec(&w, "flamestrike", &[Augment::Meteor]),
    );
    assert!((s.damage - base.damage * 3.0).abs() < 1e-3 && s.cooldown > base.cooldown * 1.9);
    let mut w = mage();
    give(&mut w, &[("flamestrike", 10, &[Augment::Meteor])]);
    let id = foe(&mut w, 2.0, 0.0, 1e6);
    run(&mut w, 0.2);
    assert!(w.enemy(id).unwrap().status.stun > 0.0);
}

#[test]
fn phoenix_ground_heals_the_hero_standing_in_it() {
    let health = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("flamestrike", 10, augments)]);
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 500.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        run(&mut w, 3.0);
        w.heroes[0].hp
    };
    let (plain, phoenix) = (health(&[]), health(&[Augment::PhoenixGround]));
    assert!(phoenix > plain + 30.0, "{plain} vs {phoenix}");
}

#[test]
fn impact_stuns_and_spreads_the_burn() {
    let mut w = mage();
    give(&mut w, &[("fire_blast", 10, &[Augment::Impact])]);
    let target = foe(&mut w, 5.0, 0.0, 1e6);
    burning(foe_mut(&mut w, target), 5.0);
    let others = [
        foe(&mut w, 7.0, 0.0, 1e6),
        foe(&mut w, 5.0, 2.0, 1e6),
        foe(&mut w, 5.0, -2.0, 1e6),
    ];
    run(&mut w, 0.2);
    assert!(w.enemy(target).unwrap().status.stun > 0.0);
    for id in others {
        assert!(
            w.enemy(id).unwrap().status.burning(),
            "the burn did not spread"
        );
    }
}

#[test]
fn inferno_casts_again_on_a_kill() {
    let killed = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("fire_blast", 10, augments)]);
        let ids = [foe(&mut w, 4.0, 0.0, 10.0), foe(&mut w, 8.0, 0.0, 10.0)];
        run(&mut w, 0.4);
        ids.iter().filter(|&&id| w.enemy(id).is_none()).count()
    };
    assert_eq!(killed(&[]), 1);
    assert_eq!(killed(&[Augment::Inferno]), 2);
}

#[test]
fn ignition_always_crits_burning_enemies() {
    let count = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("fire_blast", 10, augments)]);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        burning(foe_mut(&mut w, id), 1.0);
        let events = run(&mut w, 0.2);
        events
            .iter()
            .filter(|e| {
                matches!(e, Event::Damage { target: Target::Enemy(t), crit: true, .. } if *t == id)
            })
            .count()
    };
    assert_eq!(count(&[]), 0);
    assert!(count(&[Augment::Ignition]) >= 1);
}

#[test]
fn frozen_cone_freezes_what_it_hits() {
    let mut w = mage();
    give(&mut w, &[("cone_of_cold", 10, &[Augment::FrozenCone])]);
    let id = foe(&mut w, 5.0, 0.0, 1e6);
    run(&mut w, 0.2);
    assert!(w.enemy(id).unwrap().status.freeze > 0.0);
}

#[test]
fn shard_burst_doubles_damage_on_the_frozen() {
    let frozen = |e: &mut Enemy| e.status.freeze = 100.0;
    let plain = dealt("cone_of_cold", &[], 0.2, frozen);
    let burst = dealt("cone_of_cold", &[Augment::ShardBurst], 0.2, frozen);
    assert!(
        (burst - plain * 2.0).abs() < 0.01 * plain,
        "{plain} vs {burst}"
    );
}

#[test]
fn blazing_speed_hastens_the_hero_per_hit() {
    let mut w = mage();
    give(&mut w, &[("blast_wave", 10, &[Augment::BlazingSpeed])]);
    for dy in [-2.0, 0.0, 2.0] {
        foe(&mut w, 3.0, dy, 1e6);
    }
    run(&mut w, 0.2);
    assert!(w.heroes[0].spell.haste > 0.0);
}

#[test]
fn living_wave_burns_and_bursts_from_the_dead() {
    let mut w = mage();
    give(&mut w, &[("blast_wave", 10, &[Augment::LivingWave])]);
    let wave = ability(&w, "blast_wave");
    let doomed = foe(&mut w, 3.0, 0.0, 25.0);
    foe(&mut w, 4.0, 3.0, 1e6);
    let events = run(&mut w, 0.2);
    assert!(w.enemy(doomed).unwrap().status.burning());
    let later = run(&mut w, 5.0);
    assert!(w.enemy(doomed).is_none(), "its burn should finish it");
    assert!(casts_of(&events, wave) + casts_of(&later, wave) >= 2);
}

#[test]
fn dragons_breath_is_a_stunning_cone_ahead() {
    let mut w = mage();
    give(&mut w, &[("blast_wave", 10, &[Augment::DragonsBreath])]);
    let (ahead, behind) = (foe(&mut w, 4.0, 0.0, 1e6), foe(&mut w, -4.0, 0.0, 1e6));
    run(&mut w, 0.2);
    assert!(w.enemy(ahead).unwrap().status.stun > 0.0);
    assert_eq!(lost(&w, behind, 1e6), 0.0);
}

#[test]
fn presence_of_mind_fires_a_second_pyroblast_every_third() {
    let casts = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("pyroblast", 10, augments)]);
        let pyro = ability(&w, "pyroblast");
        foe(&mut w, 6.0, 0.0, 1e6);
        foe(&mut w, 8.0, 3.0, 1e6);
        casts_of(&run(&mut w, 12.5), pyro)
    };
    assert_eq!(casts(&[]), 3);
    assert_eq!(casts(&[Augment::PresenceOfMind]), 4);
}

#[test]
fn kindling_shortens_pyroblast_on_fire_crits() {
    let cd = |augments: &[Augment]| {
        let mut w = mage();
        give(&mut w, &[("pyroblast", 10, augments), ("fireball", 5, &[])]);
        w.heroes[0].stats.add(Stat::Crit, 2.0);
        foe(&mut w, 6.0, 0.0, 1e6);
        run(&mut w, 3.0);
        w.heroes[0].abilities[0].cd
    };
    let (plain, kindled) = (cd(&[]), cd(&[Augment::Kindling]));
    assert!(kindled < plain - 0.4, "{plain} vs {kindled}");
}

#[test]
fn combustion_cashes_in_the_burns_doubled() {
    let burnt = |e: &mut Enemy| burning(e, 10.0);
    let plain = dealt("pyroblast", &[], 0.4, burnt);
    let combust = dealt("pyroblast", &[Augment::Combustion], 0.4, burnt);
    // 4 seconds of 10 a second left, doubled.
    assert!(combust > plain + 60.0, "{plain} vs {combust}");
}

#[test]
fn ice_floes_hastens_frost_spells_while_the_shield_holds() {
    let casts = |augments: &[Augment]| {
        let mut w = mage();
        give(
            &mut w,
            &[("ice_barrier", 10, augments), ("frostbolt", 5, &[])],
        );
        let bolt = ability(&w, "frostbolt");
        foe(&mut w, 6.0, 0.0, 1e6);
        casts_of(&run(&mut w, 10.0), bolt)
    };
    let (plain, floes) = (casts(&[]), casts(&[Augment::IceFloes]));
    assert!(floes > plain, "{plain} vs {floes}");
}

#[test]
fn frost_armor_chills_and_hurts_who_strikes_the_shield() {
    let mut w = mage();
    give(&mut w, &[("ice_barrier", 10, &[Augment::FrostArmor])]);
    w.heroes[0].max_hp = 1000.0;
    w.heroes[0].hp = 1000.0;
    let id = striker(&mut w);
    run(&mut w, 2.0);
    let e = w.enemy(id).unwrap();
    assert!(e.status.chill > 0.0);
    assert!(e.max_hp - e.hp > 0.0);
}
