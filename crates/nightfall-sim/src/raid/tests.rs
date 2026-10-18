//! The raid in whole worlds on small hand-made maps: carrying and acts first; containers, exits,
//! deaths and outcomes after.

use std::sync::Arc;

use super::*;
use crate::heightmap::testmap::flat;
use crate::{CastPress, ContentSources, Gait, HeroPose, Life, PlayerInput, DT};

const CLASSES: &str = r#"[
    (key: "rogue", name: "Rogue", class_id: 4, power: 3, race: 1, sex: 0, base_hp: 1000.0,
     speed: 7.0, stealth: true, power_max: 100.0, power_regen: 10.0,
     free_kit: [("knife", 1), ("bread", 2)], bar: [(1, "sprint")]),
    (key: "mage", name: "Mage", class_id: 8, power: 0, race: 1, sex: 1, base_hp: 800.0,
     speed: 7.0, power_max: 1000.0, bar: [(8, "water"), (12, "eat")]),
]"#;
const MAP: &str = r#"(key: "test", name: "Test", map: 0,
    bounds: [(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)],
    entries: [(5.0, 5.0, 0.0)],
    exits: [(key: "bridge", name: "The Bridge", at: (50.0, 50.0)),
            (key: "cart", name: "The Cart", at: (50.0, 10.0), late: true)])"#;
const ENEMIES: &str = r#"[
    (key: "brute", name: "Brute", display: 200, level: 28, hp: 1000.0, walk_speed: 2.5,
     run_speed: 7.0, swing: Some((min: 40.0, max: 40.0, every: 2.0))),
]"#;
const ABILITIES: &str = r#"[
    (key: "sprint", name: "Sprint", spell: 2983, aim: Me, cooldown: 300.0, sound: Silent,
     keeps_stealth: true, effects: [Sprint(by: 0.5, secs: 8.0)]),
    (key: "water", name: "Water", spell: 6127, aim: Me, cost: 50.0, cast: 3.0, gcd: 1.5,
     moving: Rooted, needs: (out_of_combat: true), effects: [ConjureWater(count: 2)]),
    (key: "eat", name: "Eat", spell: 2639, aim: Me, sound: Quiet,
     needs: (out_of_combat: true, still: true), effects: [Eat(secs: 15.0)]),
]"#;
const ITEMS: &str = r#"[
    (key: "knife", name: "Knife", kind: Weapon, icon: "a", weight: 1.0, slot: Some(MainHand),
     bonus: (damage: 0.5)),
    (key: "helm", name: "Helm", kind: Armour, icon: "a", weight: 2.0, slot: Some(Head),
     bonus: (health: 100.0)),
    (key: "cap", name: "Cap", kind: Armour, icon: "a", weight: 1.0, slot: Some(Head),
     bonus: (health: 40.0)),
    (key: "bread", name: "Bread", kind: Food, icon: "a", weight: 0.5, stack: 10, restore: 0.6),
    (key: "water", name: "Water", kind: Drink, icon: "a", weight: 0.5, stack: 10),
    (key: "conjured", name: "Conjured Water", kind: Drink, icon: "a", weight: 0.1, stack: 20),
    (key: "potion", name: "Potion", kind: Potion, icon: "a", weight: 0.2, stack: 5,
     on_use: Some(Heal(300.0))),
    (key: "gem", name: "Gem", kind: Valuable, icon: "a", weight: 1.0, stack: 10, value: 40),
    (key: "rock", name: "Rock", kind: Material, icon: "a", weight: 10.0, stack: 5),
    (key: "pouch", name: "Pouch", kind: Bag, icon: "a", weight: 0.5, bag_slots: 4),
    (key: "lantern", name: "Lantern", kind: Lantern, icon: "a", weight: 1.0,
     light: Some((range: 18.0, intensity: 1.4))),
    (key: "key", name: "Key", kind: Key, icon: "a", weight: 0.1),
]"#;
const LOOT: &str = r#"(
    kinds: [(key: "chest", name: "Chest", display: 259, search: 2.0, sound: Open, slots: 8),
            (key: "grave", name: "Grave", display: 52, search: 6.0, sound: Dig, slots: 8)],
    tables: [(key: "gems", rolls: (2, 2), items: [(item: "gem", weight: 1.0, count: (3, 3))]),
             (key: "mixed", rolls: (1, 4), nothing: 1.0,
              items: [(item: "gem", weight: 1.0), (item: "bread", weight: 2.0, count: (1, 3)),
                      (item: "potion", weight: 1.0)])],
    containers: [(kind: "chest", table: "gems", at: (12.0, 5.0)),
                 (kind: "grave", table: "mixed", at: (12.0, 20.0)),
                 (kind: "chest", table: "gems", at: (30.0, 30.0), lock: Some("key"))],
    bodies: [(enemy: "brute", table: "gems")],
)"#;

/// A brute standing far from every spot the tests use, past its notice range from all of them
/// and past the loudest noise they make.
pub(super) const BRUTE_FAR: &str =
    r#"(place: "Far", kind: "brute", behaviour: Sentry(at: (5.0, 55.0), facing: 0.0)),"#;

fn rules(run: &str) -> String {
    format!(
        r#"(hero_level: 30, atmosphere: (night_minute: 0, fog_start: 10.0, fog_end: 40.0,
        fog_color: (0.1, 0.1, 0.1), view_distance: 120.0,
        hero_light: (color: (1.0, 0.8, 0.6), intensity: 0.6, range: 9.0)),
        combat: (crit_chance: 0.0),
        run: (backpack_slots: 4, safe_slots: 1, stash_slots: 6, cart_opens_at: 20.0,
              conjured_drink: "conjured", {run}))"#
    )
}

/// A world on flat ground 60 yards across, its map attached, with `run` rules on top of the
/// tests' own and `spawns` (RON list items, each ending in a comma).
pub(super) fn world_with(run: &str, spawns: &str) -> World {
    let content = crate::Content::from_ron(ContentSources {
        classes: CLASSES,
        map: MAP,
        rules: &rules(run),
        enemies: ENEMIES,
        spawns: &format!("[{spawns}]"),
        abilities: ABILITIES,
        items: ITEMS,
        loot: LOOT,
    })
    .unwrap();
    let mut w = World::new(Arc::new(content));
    w.attach_map(Arc::new(flat(60.0).finish()));
    w
}

pub(super) fn world() -> World {
    world_with("", "")
}

pub(super) fn item(w: &World, key: &str) -> usize {
    w.content().item(key).unwrap()
}

fn pose(x: f32, y: f32) -> Option<HeroPose> {
    Some(HeroPose {
        pos: V2::new(x, y),
        facing: V2::new(1.0, 0.0),
        z: 0.0,
    })
}

/// `player`'s next input: their hero at `(x, y)`, still.
pub(super) fn stand(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.set_input(
        player,
        PlayerInput {
            pose: pose(x, y),
            gait: Gait::Still,
            ..Default::default()
        },
    );
}

/// `player`'s next input: their hero walking, at `(x, y)`.
pub(super) fn walk(w: &mut World, player: PlayerId, x: f32, y: f32) {
    w.set_input(
        player,
        PlayerInput {
            pose: pose(x, y),
            gait: Gait::Walk,
            ..Default::default()
        },
    );
}

/// `player`'s hero of `class` standing at `(x, y)`, its first input taken.
pub(super) fn join(w: &mut World, player: PlayerId, class: &str, x: f32, y: f32) {
    w.add_hero(player, class, "Hero", 1, 0).unwrap();
    stand(w, player, x, y);
    w.step();
}

/// `player`'s next input: act `a` (press number `seq`), standing where they are.
pub(super) fn act(w: &mut World, player: PlayerId, seq: u8, a: Act) {
    let h = w.hero(player).unwrap();
    let at = HeroPose {
        pos: h.pos,
        facing: h.facing,
        z: h.z,
    };
    w.set_input(
        player,
        PlayerInput {
            pose: Some(at),
            act: Some(ActPress { seq, act: a }),
            ..Default::default()
        },
    );
}

pub(super) fn steps(w: &mut World, n: usize) {
    for _ in 0..n {
        w.step();
    }
}

/// A loadout: `pack` into the backpack, `worn` worn, `safe` in the safe slots.
pub(super) fn loadout(
    w: &World,
    pack: &[(&str, u16)],
    worn: &[(&str, EquipSlot)],
    safe: &[(&str, u16)],
) -> Loadout {
    let c = w.content();
    let mut inv = Inventory::new(c);
    for &(key, slot) in worn {
        inv.equip[slot.index()] = Some(Stack::new(item(w, key), 1));
    }
    for &(key, n) in pack {
        assert!(inv.add(Stack::new(item(w, key), n), c).is_none());
    }
    for (i, &(key, n)) in safe.iter().enumerate() {
        inv.safe[i] = Some(Stack::new(item(w, key), n));
    }
    Loadout {
        inventory: inv,
        recovery: None,
    }
}

#[test]
fn a_loadout_is_worn_weighed_and_counted() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    let lo = loadout(
        &w,
        &[("rock", 5), ("bread", 2)],
        &[("knife", EquipSlot::MainHand), ("helm", EquipSlot::Head)],
        &[],
    );
    assert!(w.give_loadout(0, &lo).is_empty());
    let h = w.hero(0).unwrap();
    assert!((h.carry.load - (50.0 + 1.0 + 1.0 + 2.0)).abs() < 1e-4);
    assert_eq!(
        h.carry.worn,
        vec![
            (EquipSlot::Head, item(&w, "helm") as u16),
            (EquipSlot::MainHand, item(&w, "knife") as u16)
        ]
    );
    assert_eq!((h.max_hp, h.hp), (1100.0, 1100.0));
    assert!((w.gear_damage(0) - 1.5).abs() < 1e-6);
    assert!((w.gear_damage(7) - 1.0).abs() < 1e-6);
}

#[test]
fn weight_slows_the_pace_and_makes_steps_louder() {
    let r = RunRules::default();
    let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5;
    assert!(near(burden(0.0, &r), (1.0, 1.0)));
    assert!(near(burden(20.0, &r), (1.0, 1.0)));
    assert!(near(burden(40.0, &r), (0.85, 1.25)));
    assert!(near(burden(60.0, &r), (0.7, 1.5)));
    assert!(near(burden(500.0, &r), (0.7, 1.5)));
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("rock", 5)], &[], &[]));
    for i in 0..30 {
        walk(&mut w, 0, 5.0 + i as f32 * 0.1, 5.0);
        w.step();
    }
    let h = w.hero(0).unwrap();
    // 50 carried: three quarters of the way from weight_free to weight_full.
    assert!((h.speed - 7.0 * 0.775).abs() < 1e-3, "{}", h.speed);
    assert!((h.noise - 4.0 * 1.375).abs() < 1e-3, "{}", h.noise);
}

#[test]
fn an_act_is_taken_once_however_often_it_is_sent() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1), ("bread", 1)], &[], &[]));
    let lantern = item(&w, "lantern") as u16;
    // Sent twice, as a client repeats it until the host answers: it lights the lantern once, not
    // on and off.
    for _ in 0..2 {
        act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
        w.step();
    }
    assert_eq!(w.hero(0).unwrap().carry.lantern, Some(lantern));
    assert_eq!(w.hero(0).unwrap().carry.acted, 1);
    act(&mut w, 0, 2, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().carry.lantern, None);
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    let c = &w.hero(0).unwrap().carry;
    assert_eq!((c.lantern, c.acted), (None, 2));
}

#[test]
fn the_downed_cannot_act_but_are_answered() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1)], &[], &[]));
    w.heroes[0].fight.life = Life::Downed {
        left: 40.0,
        revive: 0.0,
    };
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    w.step();
    let c = &w.hero(0).unwrap().carry;
    assert_eq!((c.lantern, c.acted), (None, 1));
}

#[test]
fn a_potion_heals_at_once_then_waits() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("potion", 2)], &[], &[]));
    let potion = item(&w, "potion");
    w.heroes[0].hp = 400.0;
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().hp, 700.0);
    act(&mut w, 0, 2, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().hp, 700.0);
    assert_eq!(w.pack(0).unwrap().count_of(potion), 1);
    steps(&mut w, (30.0 / DT) as usize);
    act(&mut w, 0, 3, Act::Use(Place::Pack(0)));
    w.step();
    assert_eq!(w.hero(0).unwrap().hp, 1000.0);
    assert_eq!(w.pack(0).unwrap().count_of(potion), 0);
}

#[test]
fn wearing_and_dropping_by_act() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("knife", 1), ("bread", 2)], &[], &[]));
    act(
        &mut w,
        0,
        1,
        Act::Move {
            from: Place::Pack(0),
            to: Place::Equip(EquipSlot::MainHand),
        },
    );
    w.step();
    let knife = item(&w, "knife") as u16;
    assert_eq!(
        w.hero(0).unwrap().carry.worn,
        vec![(EquipSlot::MainHand, knife)]
    );
    act(&mut w, 0, 2, Act::Drop(Place::Pack(1)));
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "bread")), 0);
    assert!((w.hero(0).unwrap().carry.load - 1.0).abs() < 1e-5);
    let view = w.loot_view(Some(0));
    let pile = view
        .marks
        .iter()
        .find(|m| m.holder == Holder::Pile)
        .unwrap();
    assert!(pile.searched && !pile.empty);
    assert!(pile.at.distance(V2::new(5.0, 5.0)) < 1e-4);
}

const HELM_ON: Act = Act::Move {
    from: Place::Pack(0),
    to: Place::Equip(EquipSlot::Head),
};
const HELM_OFF: Act = Act::Move {
    from: Place::Equip(EquipSlot::Head),
    to: Place::Pack(0),
};

/// A rogue (1000 health) with a helm (+100) in the backpack, at `hp`.
fn with_helm(hp: f32) -> World {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("helm", 1)], &[], &[]));
    w.heroes[0].hp = hp;
    w
}

fn health(w: &World) -> (f32, f32) {
    (w.heroes[0].max_hp, w.heroes[0].hp)
}

#[test]
fn swapping_gear_heals_nobody() {
    // A hero at 1050/1100 who takes the +100 helm off and puts it back gains nothing.
    let mut w = with_helm(1000.0);
    act(&mut w, 0, 1, HELM_ON);
    w.step();
    assert_eq!(health(&w), (1100.0, 1100.0));
    w.heroes[0].hp = 1050.0;
    act(&mut w, 0, 2, HELM_OFF);
    w.step();
    assert_eq!(health(&w), (1000.0, 950.0));
    act(&mut w, 0, 3, HELM_ON);
    w.step();
    assert_eq!(health(&w), (1100.0, 1050.0));
}

#[test]
fn the_missing_health_stays_the_same_when_gear_changes_the_most() {
    let mut w = with_helm(1000.0);
    act(&mut w, 0, 1, HELM_ON);
    w.step();
    w.heroes[0].hp = 600.0;
    act(&mut w, 0, 2, HELM_OFF);
    w.step();
    assert_eq!(health(&w), (1000.0, 500.0));
    act(&mut w, 0, 3, HELM_ON);
    w.step();
    assert_eq!(health(&w), (1100.0, 600.0));
}

#[test]
fn the_downed_are_unchanged_by_a_recount_of_their_gear() {
    // A downed hero's health and state stay as they are through a recount of their gear.
    let mut w = with_helm(1000.0);
    act(&mut w, 0, 1, HELM_ON);
    w.step();
    w.heroes[0].hp = 0.0;
    w.heroes[0].fight.life = Life::Downed {
        left: 40.0,
        revive: 0.0,
    };
    let content = w.content().clone();
    w.raid.pack_mut(0, &content).equip[EquipSlot::Head.index()] = None;
    w.refresh_carry(0);
    assert_eq!(health(&w), (1000.0, 0.0));
    assert!(matches!(w.heroes[0].fight.life, Life::Downed { .. }));
    w.heroes[0].fight.life = Life::Dead;
    w.refresh_carry(0);
    assert_eq!(health(&w), (1000.0, 0.0));
}

/// `with_helm`'s hero, the helm worn at full health.
fn helmed() -> World {
    let mut w = with_helm(1000.0);
    act(&mut w, 0, 1, HELM_ON);
    w.step();
    assert_eq!(health(&w), (1100.0, 1100.0));
    w
}

#[test]
fn gear_that_would_leave_a_hero_at_one_or_less_stays_on() {
    let mut w = helmed();
    w.heroes[0].hp = 50.0;
    let pack = w.pack(0).unwrap().clone();
    act(&mut w, 0, 2, HELM_OFF);
    w.step();
    // Refused, nothing moved, and the act is still answered.
    assert_eq!(health(&w), (1100.0, 50.0));
    assert_eq!(w.pack(0), Some(&pack));
    assert_eq!(w.hero(0).unwrap().carry.acted, 2);
    // So is dropping it, as is a hero at exactly 101 (it would leave them at 1).
    act(&mut w, 0, 3, Act::Drop(Place::Equip(EquipSlot::Head)));
    w.step();
    assert_eq!(w.pack(0), Some(&pack));
    w.heroes[0].hp = 101.0;
    act(&mut w, 0, 4, HELM_OFF);
    w.step();
    assert_eq!(w.pack(0), Some(&pack));
    assert_eq!(health(&w), (1100.0, 101.0));
}

#[test]
fn gear_that_leaves_a_hero_above_one_comes_off_and_on_without_a_gain() {
    let mut w = helmed();
    w.heroes[0].hp = 150.0;
    act(&mut w, 0, 2, HELM_OFF);
    w.step();
    assert_eq!(health(&w), (1000.0, 50.0));
    act(&mut w, 0, 3, HELM_ON);
    w.step();
    assert_eq!(health(&w), (1100.0, 150.0));
}

#[test]
fn a_swap_of_worn_pieces_that_costs_health_obeys_the_same_refusal() {
    let mut w = helmed();
    // A cap (+40) beside the helm in the backpack: swapping it in costs 60 health.
    let cap = Stack::new(item(&w, "cap"), 1);
    let content = w.content().clone();
    assert!(w.raid.pack_mut(0, &content).add(cap, &content).is_none());
    let swap = Act::Move {
        from: Place::Pack(0),
        to: Place::Equip(EquipSlot::Head),
    };
    w.heroes[0].hp = 60.0;
    let pack = w.pack(0).unwrap().clone();
    act(&mut w, 0, 2, swap);
    w.step();
    assert_eq!(health(&w), (1100.0, 60.0));
    assert_eq!(w.pack(0), Some(&pack));
    // With health to spare the same swap goes through, and costs exactly those 60.
    w.heroes[0].hp = 61.0 + 100.0;
    act(&mut w, 0, 3, swap);
    w.step();
    assert_eq!(health(&w), (1040.0, 101.0));
    assert_eq!(
        w.hero(0).unwrap().carry.worn,
        vec![(EquipSlot::Head, cap.item)]
    );
}

#[test]
fn a_raid_must_name_an_entry_and_every_exit() {
    let mut w = world();
    let standard = RaidRoll::standard(w.content(), 9);
    assert_eq!(
        standard.exits[0],
        ExitPlan {
            open: true,
            opens_at: 0.0
        }
    );
    assert_eq!(
        standard.exits[1],
        ExitPlan {
            open: true,
            opens_at: 20.0
        }
    );
    let far = RaidRoll {
        entry: 3,
        ..standard.clone()
    };
    assert!(w.begin_raid(far).is_err());
    let shut = RaidRoll {
        exits: Vec::new(),
        ..standard.clone()
    };
    assert!(w.begin_raid(shut).is_err());
    assert!(w.begin_raid(standard).is_ok());
    assert_eq!(w.raid.roll.seed, 9);
}

#[test]
fn a_mirror_sees_its_own_pack_and_the_loot_near_it() {
    let mut host = world();
    join(&mut host, 0, "rogue", 5.0, 5.0);
    host.give_loadout(0, &loadout(&host, &[("bread", 2)], &[], &[]));
    act(&mut host, 0, 1, Act::Drop(Place::Pack(0)));
    host.step();
    let mut mirror = World::new(host.content().clone());
    mirror.apply_snapshot(host.snapshot_for(0));
    assert_eq!(mirror.pack(0), host.pack(0));
    assert_eq!(mirror.loot_view(Some(0)), host.loot_view(Some(0)));
    let mut whole = World::new(host.content().clone());
    whole.apply_snapshot(host.snapshot());
    assert_eq!(whole.snapshot(), host.snapshot());
}

#[test]
fn a_hero_who_leaves_drops_what_a_death_would() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 8.0, 5.0);
    stand(&mut w, 0, 5.0, 5.0);
    stand(&mut w, 1, 8.0, 5.0);
    w.step();
    w.give_loadout(1, &loadout(&w, &[("rock", 2)], &[], &[("bread", 1)]));
    w.remove_hero(1);
    let piles: Vec<&Container> = w
        .raid
        .containers
        .iter()
        .filter(|c| c.holder == Holder::Pile)
        .collect();
    assert_eq!(piles.len(), 1);
    let held: Vec<Stack> = piles[0].stacks.iter().flatten().copied().collect();
    assert_eq!(held, vec![Stack::new(item(&w, "rock"), 2)]);
    assert!(w.pack(1).is_none());
}

/// A rogue at (5, 5) carrying 50 weight: three quarters of the way to the full penalty, so
/// 0.775 of the pace and 1.375 times the footsteps' reach.
fn loaded() -> World {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("rock", 5)], &[], &[]));
    w
}

const PACE: f32 = 7.0 * 0.775;

#[test]
fn weight_keeps_slowing_across_steps_and_after_a_seam_between_them() {
    let mut w = loaded();
    for _ in 0..3 {
        stand(&mut w, 0, 5.0, 5.0);
        w.step();
        assert!((w.hero(0).unwrap().speed - PACE).abs() < 1e-3);
    }
    // The seams that work the senses' speed out afresh between steps must not drop the weight.
    w.hurt_hero(0, 10.0);
    assert!((w.snapshot().heroes[0].speed - PACE).abs() < 1e-3);
    w.break_stealth(0);
    assert!((w.snapshot().heroes[0].speed - PACE).abs() < 1e-3);
    w.flash(0);
    assert!((w.snapshot().heroes[0].speed - PACE).abs() < 1e-3);
    // Sneaking is slowed by the weight as well.
    w.enter_stealth(0);
    let sneak = w.content().rules.senses.stealth_speed;
    assert!((w.snapshot().heroes[0].speed - PACE * sneak).abs() < 1e-3);
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    assert!((w.hero(0).unwrap().speed - PACE * sneak).abs() < 1e-3);
}

#[test]
fn a_sprinting_loaded_hero_keeps_both_factors_and_loud_steps() {
    let mut w = loaded();
    w.set_input(
        0,
        PlayerInput {
            pose: pose(5.0, 5.0),
            cast: Some(CastPress {
                seq: 1,
                slot: 1,
                target: None,
                at: None,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert!((w.hero(0).unwrap().speed - PACE * 1.5).abs() < 1e-3);
    w.hurt_hero(0, 10.0);
    assert!((w.snapshot().heroes[0].speed - PACE * 1.5).abs() < 1e-3);
    // Running, a loaded hero's footsteps carry 12 yards times 1.375, and the lantern in the
    // next test lights them.
    for i in 0..4 {
        w.set_input(
            0,
            PlayerInput {
                pose: pose(5.0 + i as f32 * 0.3, 5.0),
                gait: Gait::Run,
                ..Default::default()
            },
        );
        w.step();
    }
    assert!((w.hero(0).unwrap().noise - 12.0 * 1.375).abs() < 1e-3);
}

#[test]
fn a_lit_lantern_makes_a_hero_easier_to_see() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1)], &[], &[]));
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    let dark = w.hero(0).unwrap().visibility;
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    w.step();
    let lit = w.hero(0).unwrap().visibility;
    assert!(lit > dark, "{lit} against {dark}");
}

#[test]
fn worn_gear_adds_to_every_blow() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 5.0, 5.0);
    let hp = |w: &World| w.enemies[0].hp;
    let start = hp(&w);
    w.strike(0, 0, 100.0, false, true, false);
    assert!((start - hp(&w) - 100.0).abs() < 1e-3);
    w.give_loadout(0, &loadout(&w, &[], &[("knife", EquipSlot::MainHand)], &[]));
    let before = hp(&w);
    w.strike(0, 0, 100.0, false, true, false);
    assert!((before - hp(&w) - 150.0).abs() < 1e-3);
    // A blow that is not a number is still no blow, whatever is worn.
    let before = hp(&w);
    w.strike(0, 0, f32::NAN, false, true, false);
    w.strike(0, 0, f32::INFINITY, false, true, false);
    assert_eq!(hp(&w), before);
}

#[test]
fn a_bad_act_from_a_client_changes_nothing_and_is_answered() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2), ("lantern", 1)], &[], &[]));
    let pack = w.pack(0).unwrap().clone();
    let bad = [
        Act::Open(9999),
        Act::Take(60000),
        Act::TakeAll,
        Act::Put(Place::Pack(3)),
        Act::Put(Place::Pack(60000)),
        Act::Use(Place::Pack(60000)),
        Act::Use(Place::Safe(200)),
        Act::Drop(Place::Pack(60000)),
        Act::Drop(Place::Bag(9)),
        Act::Move {
            from: Place::Pack(60000),
            to: Place::Pack(0),
        },
        Act::Move {
            from: Place::Pack(0),
            to: Place::Equip(EquipSlot::Head),
        },
        Act::Close,
    ];
    for (i, a) in bad.into_iter().enumerate() {
        act(&mut w, 0, i as u8 + 1, a);
        w.step();
        assert_eq!(w.hero(0).unwrap().carry.acted, i as u8 + 1, "{a:?}");
    }
    assert_eq!(w.pack(0), Some(&pack));
    assert!(w.loot_view(Some(0)).marks.is_empty());
    // An act for a hero who is not there is dropped.
    w.set_input(
        9,
        PlayerInput {
            act: Some(ActPress {
                seq: 1,
                act: Act::Close,
            }),
            ..Default::default()
        },
    );
    w.step();
    // A stale number is not new: the same seq again does nothing, a different one does.
    let lantern = item(&w, "lantern") as u16;
    act(&mut w, 0, 40, Act::Use(Place::Pack(1)));
    w.step();
    assert_eq!(w.hero(0).unwrap().carry.lantern, Some(lantern));
    act(&mut w, 0, 40, Act::Use(Place::Pack(1)));
    w.step();
    assert_eq!(w.hero(0).unwrap().carry.lantern, Some(lantern));
}

#[test]
fn the_snapshot_of_one_player_holds_only_their_own_pack() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 8.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2)], &[], &[]));
    w.give_loadout(1, &loadout(&w, &[("gem", 3)], &[], &[]));
    let (a, b) = (w.snapshot_for(0), w.snapshot_for(1));
    assert_eq!(a.pack.as_ref().map(|p| p.0), Some(0));
    assert_eq!(b.pack.as_ref().map(|p| p.0), Some(1));
    assert_ne!(a.pack, b.pack);
    assert!(w.snapshot().pack.is_none());
    assert!(w.snapshot_for(7).pack.is_none());
}

#[test]
fn loot_is_only_seen_within_a_players_reach() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    // The same reach as the enemies of their snapshot and the events they are sent. The spots
    // are only data to the raid, so they may lie past the 60 yard map.
    let reach = w.reach();
    let gem = Stack::new(item(&w, "gem"), 1);
    let near = w.raid.add_container(
        Holder::Pile,
        V2::new(5.0 + reach - 1.0, 5.0),
        Some(0.0),
        vec![gem],
        4,
        true,
        None,
    );
    w.raid.add_container(
        Holder::Pile,
        V2::new(5.0 + reach + 1.0, 5.0),
        Some(0.0),
        vec![gem],
        4,
        true,
        None,
    );
    let seen: Vec<u32> = w.loot_view(Some(0)).marks.iter().map(|m| m.id).collect();
    assert_eq!(seen, vec![near]);
    assert_eq!(w.loot_view(None).marks.len(), 2);
    assert_eq!(w.snapshot_for(0).loot.marks.len(), 1);
}

/// A searched pile at `at` holding `stacks`; its id.
fn pile(w: &mut World, at: V2, stacks: Vec<Stack>) -> u32 {
    w.raid
        .add_container(Holder::Pile, at, Some(0.0), stacks, 4, true, None)
}

/// `player` opens the pile `id` and takes everything, standing where they are.
fn loot_pile(w: &mut World, player: PlayerId, id: u32, seq: u8) {
    act(w, player, seq, Act::Open(id));
    w.step();
    act(w, player, seq + 1, Act::TakeAll);
    w.step();
}

#[test]
fn a_thing_keeps_its_found_flag_through_a_container() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2)], &[], &[]));
    let (gem, bread) = (item(&w, "gem"), item(&w, "bread"));
    // What the world made is found.
    let id = pile(&mut w, V2::new(5.0, 5.0), vec![Stack::found(gem, 1)]);
    loot_pile(&mut w, 0, id, 1);
    // A brought thing dropped and picked up again stays brought.
    act(&mut w, 0, 3, Act::Drop(Place::Pack(0)));
    w.step();
    let dropped = w
        .loot_view(Some(0))
        .marks
        .iter()
        .find(|m| m.holder == Holder::Pile && !m.empty)
        .unwrap()
        .id;
    loot_pile(&mut w, 0, dropped, 4);
    let stacks: Vec<Stack> = w
        .pack(0)
        .unwrap()
        .stacks()
        .into_iter()
        .map(|s| s.1)
        .collect();
    assert!(stacks.contains(&Stack::found(gem, 1)), "{stacks:?}");
    assert!(stacks.contains(&Stack::new(bread, 2)), "{stacks:?}");
    // Under found_only a death takes the gem and leaves the bread.
    let content = w.content().clone();
    let mut inv = w.pack(0).unwrap().clone();
    let death = inv.split_for_death(crate::DeathLoss::FoundOnly, &content);
    assert_eq!(death.lost, vec![Stack::found(gem, 1)]);
    assert_eq!(inv.count_of(bread), 2);
}

#[test]
fn nothing_moves_to_or_from_a_container_the_hero_has_walked_off_from() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2)], &[], &[]));
    let gem = Stack::new(item(&w, "gem"), 2);
    let id = pile(&mut w, V2::new(5.0, 5.0), vec![gem]);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    let (pack, held) = (
        w.pack(0).unwrap().clone(),
        w.raid.containers[0].stacks.clone(),
    );
    // The same step they walk away, they ask for everything and offer their bread.
    for (seq, a) in [
        (2, Act::TakeAll),
        (3, Act::Take(0)),
        (4, Act::Put(Place::Pack(0))),
    ] {
        w.set_input(
            0,
            PlayerInput {
                pose: pose(40.0, 40.0),
                act: Some(ActPress { seq, act: a }),
                ..Default::default()
            },
        );
        w.step();
        assert_eq!(w.hero(0).unwrap().carry.acted, seq);
        assert_eq!(w.pack(0), Some(&pack));
        assert_eq!(w.raid.containers[0].stacks, held);
    }
    // Back in reach, the same act works.
    w.set_input(
        0,
        PlayerInput {
            pose: pose(5.0, 5.0),
            act: Some(ActPress {
                seq: 5,
                act: Act::TakeAll,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 2);
}

#[test]
fn an_unsearched_container_does_not_say_whether_it_is_empty() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    let at = V2::new(6.0, 5.0);
    let full = w
        .raid
        .add_container(Holder::Placed(0), at, Some(0.0), Vec::new(), 4, false, None);
    let marks = w.loot_view(Some(0)).marks;
    let m = marks.iter().find(|m| m.id == full).unwrap();
    assert!(!m.searched && !m.empty);
    let done = pile(&mut w, at, Vec::new());
    let marks = w.loot_view(Some(0)).marks;
    let m = marks.iter().find(|m| m.id == done).unwrap();
    assert!(m.searched && m.empty);
}

#[test]
fn an_act_for_a_player_with_no_hero_is_not_remembered() {
    let mut w = world();
    w.set_input(
        4,
        PlayerInput {
            act: Some(ActPress {
                seq: 1,
                act: Act::Close,
            }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(w.raid.last_act(4), None);
    // So the hero who joins later has their first act, with that number, taken.
    join(&mut w, 4, "rogue", 5.0, 5.0);
    act(&mut w, 4, 1, Act::Close);
    w.step();
    assert_eq!(w.raid.last_act(4), Some(1));
}
