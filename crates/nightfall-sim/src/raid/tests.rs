//! The raid in whole worlds on small hand-made maps: carrying and acts first; containers, exits,
//! deaths and outcomes after.

use std::sync::Arc;

use super::*;
use crate::combat::Dice;
use crate::heightmap::testmap::flat;
use crate::{CastPress, ContentSources, Gait, HeroPose, Life, PlayerInput, Table, DT};
use containers::roll_table;

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
    let before = w.raid.containers.clone();
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
    assert_eq!(
        w.raid.containers, before,
        "a bad act dropped or took nothing"
    );
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
    // Only the piles made here: the content's own containers would be in view too.
    w.raid.containers.clear();
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
    // The pile made below is the only container, at index 0.
    w.raid.containers.clear();
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
    // Walking off closed it; back in reach, they open it again and the same act works.
    for (seq, a) in [(5, Act::Open(id)), (6, Act::TakeAll)] {
        w.set_input(
            0,
            PlayerInput {
                pose: pose(5.0, 5.0),
                act: Some(ActPress { seq, act: a }),
                ..Default::default()
            },
        );
        w.step();
    }
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

#[test]
fn eating_eats_food_and_drinks_a_drink_from_the_pack() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2), ("water", 1)], &[], &[]));
    assert_eq!(w.hero(0).unwrap().fight.drinks, 1);
    let eat = w.content().ability("eat").unwrap();
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    // Bread gives back 60% of health over the meal, water all the mana.
    assert!((meal.health - 800.0 * 0.6 / 15.0).abs() < 1e-3);
    assert!((meal.mana - 1000.0 / 15.0).abs() < 1e-3);
    let p = w.pack(0).unwrap();
    assert_eq!(p.count_of(item(&w, "bread")), 1);
    assert_eq!(p.count_of(item(&w, "water")), 0);
    // The count the hero shows follows the meal at once, not at their next act.
    assert_eq!(w.hero(0).unwrap().fight.drinks, 0);
}

#[test]
fn a_rogue_eats_and_keeps_the_water() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 1), ("water", 1)], &[], &[]));
    w.land(0, w.content().ability("eat").unwrap(), None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert_eq!(meal.mana, 0.0);
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "water")), 1);
}

#[test]
fn water_conjures_drinks_into_the_pack_up_to_the_cap() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[], &[], &[]));
    let water = w.content().ability("water").unwrap();
    let conjured = item(&w, "conjured");
    w.land(0, water, None, None);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), 2);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 2);
    assert!(w.pack(0).unwrap().stacks().iter().all(|(_, s)| s.found));
    for _ in 0..12 {
        w.land(0, water, None, None);
    }
    let cap = u32::from(w.content().rules.combat.max_drinks);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), cap);
}

#[test]
fn with_nothing_to_eat_eating_is_refused() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("knife", 1)], &[], &[]));
    let eat = w.content().ability("eat").unwrap();
    let hi = w.hero_slot(0).unwrap();
    assert_eq!(
        w.check(hi, eat, None, None).err(),
        Some(crate::Refusal::NoFood)
    );
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    assert!(w.check(hi, eat, None, None).is_ok());
    let mut free = world_with("free_meal: true", "");
    join(&mut free, 0, "rogue", 5.0, 5.0);
    free.give_loadout(0, &loadout(&free, &[], &[], &[]));
    let hi = free.hero_slot(0).unwrap();
    assert!(free.check(hi, eat, None, None).is_ok());
}

/// The drinks a hero shows are the drinks the backpack holds: after every conjure and every
/// drink, with no lag until their next act.
#[test]
fn the_drinks_shown_follow_the_pack_through_conjuring_and_drinking() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[], &[], &[]));
    let water = w.content().ability("water").unwrap();
    let eat = w.content().ability("eat").unwrap();
    let conjured = item(&w, "conjured");
    let cap = u32::from(w.content().rules.combat.max_drinks);
    let agree = |w: &World, want: u32| {
        assert_eq!(w.pack(0).unwrap().count_of(conjured), want);
        assert_eq!(u32::from(w.hero(0).unwrap().fight.drinks), want);
    };
    w.land(0, water, None, None);
    agree(&w, 2);
    // Drinking one (the mage has no food; the meal is the drink alone) leaves one.
    w.land(0, eat, None, None);
    agree(&w, 1);
    w.land(0, water, None, None);
    agree(&w, 3);
    // Up to the cap, and conjuring at the cap adds nothing.
    for _ in 0..12 {
        w.land(0, water, None, None);
    }
    agree(&w, cap);
    w.land(0, water, None, None);
    agree(&w, cap);
}

#[test]
fn drinks_of_any_kind_count_against_the_cap_and_the_shown_count_is_the_packs() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    let cap = u32::from(w.content().rules.combat.max_drinks);
    let conjured = item(&w, "conjured");
    // More drinks than the cap: the hero shows what the pack really holds.
    w.give_loadout(0, &loadout(&w, &[("water", 25)], &[], &[]));
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "water")), 25);
    assert_eq!(u32::from(w.hero(0).unwrap().fight.drinks), 25);
    // Conjuring above the cap adds none, and at it neither; one short of it, one.
    let water = w.content().ability("water").unwrap();
    w.land(0, water, None, None);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), 0);
    w.give_loadout(0, &loadout(&w, &[("water", cap as u16)], &[], &[]));
    assert_eq!(u32::from(w.hero(0).unwrap().fight.drinks), cap);
    w.land(0, water, None, None);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), 0);
    w.give_loadout(0, &loadout(&w, &[("water", cap as u16 - 1)], &[], &[]));
    w.land(0, water, None, None);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), 1);
    assert_eq!(u32::from(w.hero(0).unwrap().fight.drinks), cap);
}

#[test]
fn conjuring_into_a_full_pack_puts_the_drinks_on_the_ground_at_the_feet() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    // Four slots of rock, five to a slot: nothing is left to put a drink in.
    w.give_loadout(0, &loadout(&w, &[("rock", 20)], &[], &[]));
    assert!(w.pack(0).unwrap().first_empty(false).is_none());
    let water = w.content().ability("water").unwrap();
    let conjured = item(&w, "conjured");
    w.land(0, water, None, None);
    let p = w.pack(0).unwrap();
    assert_eq!(p.count_of(conjured), 0);
    assert_eq!(p.count_of(item(&w, "rock")), 20);
    // Not carried, so not shown as carried; but they were made, and lie where the hero stands.
    assert_eq!(w.hero(0).unwrap().fight.drinks, 0);
    let piles: Vec<&Container> = w
        .raid
        .containers
        .iter()
        .filter(|c| c.holder == Holder::Pile)
        .collect();
    assert_eq!(piles.len(), 1);
    assert!(piles[0].at.distance(V2::new(5.0, 5.0)) < 1e-4);
    let held: Vec<Stack> = piles[0].stacks.iter().flatten().copied().collect();
    assert_eq!(held, vec![Stack::found(conjured, 2)]);
    // A second conjuring adds to the same stack of the same pile, not a slot of its own.
    w.land(0, water, None, None);
    let piles: Vec<&Container> = w
        .raid
        .containers
        .iter()
        .filter(|c| c.holder == Holder::Pile)
        .collect();
    assert_eq!(piles.len(), 1);
    let held: Vec<Stack> = piles[0].stacks.iter().flatten().copied().collect();
    assert_eq!(held, vec![Stack::found(conjured, 4)]);
    // With room made in the pack the next drinks go there, and the pile stays as it was.
    let mut inv = w.pack(0).unwrap().clone();
    let at = inv.first_of(ItemKind::Material, w.content()).unwrap();
    inv.take(at);
    w.give_loadout(
        0,
        &Loadout {
            inventory: inv,
            recovery: None,
        },
    );
    w.land(0, water, None, None);
    assert_eq!(w.pack(0).unwrap().count_of(conjured), 2);
    assert_eq!(
        w.raid
            .containers
            .iter()
            .filter(|c| c.holder == Holder::Pile)
            .flat_map(|c| c.stacks.iter().flatten())
            .map(|s| u32::from(s.count))
            .sum::<u32>(),
        4
    );
}

#[test]
fn a_drink_in_a_safe_slot_is_neither_counted_nor_drunk() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    let water = item(&w, "water");
    w.give_loadout(0, &loadout(&w, &[], &[], &[("water", 3)]));
    assert_eq!(w.pack(0).unwrap().count_of(water), 3);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 0);
    // No meal: the mage has only the safe slot's water, which the check does not see.
    let eat = w.content().ability("eat").unwrap();
    let hi = w.hero_slot(0).unwrap();
    assert_eq!(
        w.check(hi, eat, None, None).err(),
        Some(crate::Refusal::NoFood)
    );
    // Eaten anyway (a stray landing), it drinks nothing and leaves the water where it is.
    w.land(0, eat, None, None);
    assert_eq!(w.combat.hero(0).unwrap().meal.unwrap().mana, 0.0);
    assert_eq!(w.pack(0).unwrap().count_of(water), 3);
    // And the cap for Conjure Water does not count it: a full 2 arrive.
    w.land(0, w.content().ability("water").unwrap(), None, None);
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "conjured")), 2);
    assert_eq!(w.hero(0).unwrap().fight.drinks, 2);
}

#[test]
fn a_meal_for_a_rogue_is_food_and_for_a_mage_food_or_a_drink() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "mage", 5.0, 5.0);
    let eat = w.content().ability("eat").unwrap();
    let may = |w: &World, p: PlayerId| {
        let hi = w.hero_slot(p).unwrap();
        w.check(hi, eat, None, None).is_ok()
    };
    // Only water: no meal for the rogue, a drink for the mage.
    for p in [0, 1] {
        w.give_loadout(p, &loadout(&w, &[("water", 1)], &[], &[]));
    }
    assert!(!may(&w, 0));
    assert_eq!(
        w.check(w.hero_slot(0).unwrap(), eat, None, None).err(),
        Some(crate::Refusal::NoFood)
    );
    assert!(may(&w, 1));
    // Food: a meal for both.
    for p in [0, 1] {
        w.give_loadout(p, &loadout(&w, &[("bread", 1)], &[], &[]));
    }
    assert!(may(&w, 0) && may(&w, 1));
    // Nothing: no meal for either.
    for p in [0, 1] {
        w.give_loadout(p, &loadout(&w, &[], &[], &[]));
    }
    assert!(!may(&w, 0) && !may(&w, 1));
}

#[test]
fn landing_a_meal_or_a_conjuring_on_an_absent_hero_does_nothing() {
    let mut w = world();
    // No hero: landing the spells does nothing and does not panic.
    w.land(7, w.content().ability("eat").unwrap(), None, None);
    w.land(7, w.content().ability("water").unwrap(), None, None);
    assert!(w.pack(7).is_none());
}

/// Every stack lying in piles, with the number of piles.
fn pile_stacks(raid: &Raid) -> (usize, Vec<Stack>) {
    let piles: Vec<&Container> = raid
        .containers
        .iter()
        .filter(|c| c.holder == Holder::Pile)
        .collect();
    let stacks = piles
        .iter()
        .flat_map(|c| c.stacks.iter().flatten().copied())
        .collect();
    (piles.len(), stacks)
}

#[test]
fn repeated_conjuring_into_a_full_pack_fills_one_piles_stacks_not_a_slot_a_cast() {
    let mut w = world();
    join(&mut w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("rock", 20)], &[], &[]));
    let water = w.content().ability("water").unwrap();
    let conjured = item(&w, "conjured");
    for _ in 0..5 {
        w.land(0, water, None, None);
    }
    assert_eq!(pile_stacks(&w.raid), (1, vec![Stack::found(conjured, 10)]));
    // Past a full stack (20), the rest starts the next one in the same pile.
    for _ in 0..6 {
        w.land(0, water, None, None);
    }
    assert_eq!(
        pile_stacks(&w.raid),
        (
            1,
            vec![Stack::found(conjured, 20), Stack::found(conjured, 2)]
        )
    );
    // A brought stack of the same item is not merged with found ones.
    let mut raid = w.raid.clone();
    raid.drop_on_ground(
        V3::at(V2::new(5.0, 5.0), 0.0),
        Stack::new(conjured, 3),
        w.content(),
    );
    let (n, stacks) = pile_stacks(&raid);
    assert_eq!(n, 1);
    assert_eq!(stacks.len(), 3);
}

/// A mage at (5, 5) with `pack`, hurt and out of mana, who has eaten (the meal begun by a
/// landing). Health 100 of 800, mana 0 of 1000.
fn hungry(w: &mut World, pack: &[(&str, u16)]) {
    join(w, 0, "mage", 5.0, 5.0);
    w.give_loadout(0, &loadout(w, pack, &[], &[]));
    w.heroes[0].hp = 100.0;
    w.heroes[0].fight.power = 0.0;
}

#[test]
fn a_second_eat_press_in_the_middle_of_a_meal_is_busy_and_eats_nothing() {
    let mut w = world();
    hungry(&mut w, &[("bread", 3), ("water", 3)]);
    let press = |w: &mut World, seq: u8| {
        let h = w.hero(0).unwrap();
        let at = HeroPose {
            pos: h.pos,
            facing: h.facing,
            z: h.z,
        };
        w.set_input(
            0,
            PlayerInput {
                pose: Some(at),
                gait: Gait::Still,
                cast: Some(CastPress {
                    seq,
                    slot: 12,
                    target: None,
                    at: None,
                }),
                ..Default::default()
            },
        );
        w.step();
    };
    let (bread, water) = (item(&w, "bread"), item(&w, "water"));
    press(&mut w, 1);
    assert!(w.hero(0).unwrap().fight.eating > 0.0);
    assert_eq!(w.pack(0).unwrap().count_of(bread), 2);
    assert_eq!(w.pack(0).unwrap().count_of(water), 2);
    w.drain_events();
    stand(&mut w, 0, 5.0, 5.0);
    steps(&mut w, 30);
    // The second press: refused as Busy, and nothing more is eaten or drunk.
    press(&mut w, 2);
    assert!(w.drain_events().iter().any(|e| matches!(
        e,
        crate::Event::Refused {
            why: crate::Refusal::Busy,
            ..
        }
    )));
    assert_eq!(w.pack(0).unwrap().count_of(bread), 2);
    assert_eq!(w.pack(0).unwrap().count_of(water), 2);
    assert_eq!(u32::from(w.hero(0).unwrap().fight.drinks), 2);
    assert!(w.hero(0).unwrap().fight.eating > 0.0);
}

#[test]
fn a_meal_is_eaten_at_the_start_and_an_interrupted_one_is_only_part_restored() {
    let mut w = world();
    hungry(&mut w, &[("bread", 1), ("water", 1)]);
    let eat = w.content().ability("eat").unwrap();
    let (bread, water) = (item(&w, "bread"), item(&w, "water"));
    w.land(0, eat, None, None);
    // Gone at once, before a bite of it is felt.
    assert_eq!(w.pack(0).unwrap().count_of(bread), 0);
    assert_eq!(w.pack(0).unwrap().count_of(water), 0);
    assert_eq!(w.hero(0).unwrap().hp, 100.0);
    // Sit for a second, then walk off.
    for _ in 0..30 {
        stand(&mut w, 0, 5.0, 5.0);
        w.step();
    }
    walk(&mut w, 0, 5.2, 5.0);
    w.step();
    steps(&mut w, 60);
    let h = w.hero(0).unwrap();
    assert_eq!(h.fight.eating, 0.0);
    assert!(w.combat.hero(0).unwrap().meal.is_none());
    // The whole bread is 480 of health and the whole water 1000 of mana; a second of the
    // 15-second meal gave a small part, and the food stays gone.
    assert!(h.hp > 100.0 && h.hp < 100.0 + 480.0 * 0.5, "{}", h.hp);
    assert!(
        h.fight.power > 0.0 && h.fight.power < 500.0,
        "{}",
        h.fight.power
    );
    assert_eq!(w.pack(0).unwrap().count_of(bread), 0);
    assert_eq!(w.pack(0).unwrap().count_of(water), 0);
}

#[test]
fn a_free_meal_restores_all_health_without_food_and_no_mana_and_still_eats_food_when_there_is() {
    let mut w = world_with("free_meal: true", "");
    hungry(&mut w, &[]);
    let eat = w.content().ability("eat").unwrap();
    let hi = w.hero_slot(0).unwrap();
    assert!(w.check(hi, eat, None, None).is_ok());
    w.land(0, eat, None, None);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    // All of the maximum over the meal, and not a drop of mana: no drink was drunk.
    assert!((meal.health * 15.0 - 800.0).abs() < 1e-2);
    assert_eq!(meal.mana, 0.0);
    for _ in 0..(16 * 30) {
        stand(&mut w, 0, 5.0, 5.0);
        w.step();
    }
    assert_eq!(w.hero(0).unwrap().hp, 800.0);
    // With food in the pack the food is eaten, and gives its own share rather than all.
    let mut w = world_with("free_meal: true", "");
    hungry(&mut w, &[("bread", 2)]);
    w.land(0, w.content().ability("eat").unwrap(), None, None);
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "bread")), 1);
    let meal = w.combat.hero(0).unwrap().meal.unwrap();
    assert!((meal.health - 800.0 * 0.6 / 15.0).abs() < 1e-3);
    assert_eq!(meal.mana, 0.0);
}

fn chest(w: &World) -> u32 {
    w.raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Placed(0) && c.lock.is_none())
        .unwrap()
        .id
}

#[test]
fn the_content_places_its_containers_filled_from_their_tables() {
    let w = world();
    let placed: Vec<&Container> = w
        .raid
        .containers
        .iter()
        .filter(|c| matches!(c.holder, Holder::Placed(_)))
        .collect();
    assert_eq!(placed.len(), 3);
    let gems: Vec<Stack> = placed[0].stacks.iter().flatten().copied().collect();
    // Two rolls of three gems each, one stack, and found: world loot counts as found.
    assert_eq!(gems, vec![Stack::found(item(&w, "gem"), 6)]);
    assert_eq!(placed[0].stacks.len(), 8);
    assert!(!placed[0].searched);
    assert_eq!(placed[2].lock, Some(item(&w, "key") as u16));
    // Whatever any table rolled, in any container, is found.
    assert!(placed
        .iter()
        .flat_map(|c| c.stacks.iter().flatten())
        .all(|s| s.found));
}

#[test]
fn the_same_seed_rolls_the_same_loot_and_seeds_differ() {
    let grave = |seed: u64| {
        let mut w = world();
        let roll = RaidRoll::standard(w.content(), seed);
        w.begin_raid(roll).unwrap();
        w.raid.containers[1].stacks.clone()
    };
    assert_eq!(grave(7), grave(7));
    let rolls: Vec<_> = (0..20).map(grave).collect();
    assert!(
        rolls.iter().any(|r| *r != rolls[0]),
        "twenty seeds all rolled the same"
    );
}

#[test]
fn searching_a_chest_takes_its_time_and_is_heard() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[], &[], &[]));
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    let view = w.loot_view(Some(0)).open.unwrap();
    assert_eq!(view.id, id);
    assert!(view.stacks.is_empty());
    let (left, total) = view.search.unwrap();
    assert_eq!(total, 2.0);
    assert!(left < 2.0);
    steps(&mut w, 3);
    assert!(
        w.hero(0).unwrap().noise >= 15.0,
        "a chest is heard 15 yards off"
    );
    steps(&mut w, (2.0 / DT) as usize);
    let open = w.loot_view(Some(0)).open.unwrap();
    assert!(open.search.is_none());
    assert_eq!(open.stacks[0], Some(Stack::found(item(&w, "gem"), 6)));
    act(&mut w, 0, 2, Act::Take(0));
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 6);
    assert!(w.pack(0).unwrap().stacks()[0].1.found);
    let mark = w
        .loot_view(Some(0))
        .marks
        .into_iter()
        .find(|m| m.id == id)
        .unwrap();
    assert!(mark.searched && mark.empty);
}

#[test]
fn the_search_noise_is_made_at_the_container_in_the_searchers_name() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    // The senses have listened and cleared the noises by the end of a step: call the search again
    // by hand, with its beat due, and see what it made.
    w.raid.searches[0].noise_in = 0.0;
    w.step_searches();
    let n = w
        .senses
        .noises
        .last()
        .expect("a beat of the search is heard");
    assert_eq!(n.by, 0);
    assert_eq!((n.at.x, n.at.y), (12.0, 5.0));
    assert_eq!(n.radius, 15.0);
}

#[test]
fn walking_off_stops_a_search_and_closes_what_is_open() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    stand(&mut w, 0, 11.0, 7.0);
    w.step();
    assert!(w.loot_view(Some(0)).open.is_none());
    assert!(
        !w.raid
            .containers
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .searched
    );
    // Searched, then walked away from: it closes.
    stand(&mut w, 0, 11.0, 5.0);
    w.step();
    act(&mut w, 0, 2, Act::Open(id));
    steps(&mut w, (2.2 / DT) as usize);
    assert!(w.loot_view(Some(0)).open.is_some());
    stand(&mut w, 0, 20.0, 5.0);
    w.step();
    assert!(w.loot_view(Some(0)).open.is_none());
}

#[test]
fn a_locked_chest_needs_its_key_and_a_far_one_cannot_be_opened() {
    let mut w = world();
    join(&mut w, 0, "rogue", 30.0, 28.0);
    let locked = w.raid.containers[2].id;
    act(&mut w, 0, 1, Act::Open(locked));
    w.step();
    assert!(w.loot_view(Some(0)).open.is_none());
    w.give_loadout(0, &loadout(&w, &[("key", 1)], &[], &[]));
    act(&mut w, 0, 2, Act::Open(locked));
    w.step();
    assert!(w.loot_view(Some(0)).open.is_some());
    let far = chest(&w);
    act(&mut w, 0, 3, Act::Open(far));
    w.step();
    let open = w.loot_view(Some(0)).open.unwrap();
    assert_eq!(
        open.id, locked,
        "the far chest did not open, and the search went on"
    );
}

#[test]
fn what_does_not_fit_stays_in_the_container() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(
        0,
        &loadout(
            &w,
            &[("rock", 5), ("bread", 1), ("water", 1), ("potion", 1)],
            &[],
            &[],
        ),
    );
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    steps(&mut w, (2.2 / DT) as usize);
    act(&mut w, 0, 2, Act::TakeAll);
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 0);
    let c = w.raid.containers.iter().find(|c| c.id == id).unwrap();
    assert_eq!(c.stacks[0], Some(Stack::found(item(&w, "gem"), 6)));
    // Put the rocks in; then the gems fit.
    act(&mut w, 0, 3, Act::Put(Place::Pack(0)));
    w.step();
    act(&mut w, 0, 4, Act::Take(0));
    w.step();
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "gem")), 6);
    let c = w.raid.containers.iter().find(|c| c.id == id).unwrap();
    assert!(c.stacks.contains(&Some(Stack::new(item(&w, "rock"), 5))));
}

#[test]
fn a_dead_enemy_leaves_a_body_holding_its_table() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    let id = w.enemies[0].id;
    w.kill_enemy(0, Some(0));
    w.step();
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Body(id))
        .unwrap();
    assert_eq!(body.stacks[0], Some(Stack::found(item(&w, "gem"), 6)));
    let body = body.id;
    act(&mut w, 0, 1, Act::Open(body));
    w.step();
    let (_, total) = w.loot_view(Some(0)).open.unwrap().search.unwrap();
    assert_eq!(total, 1.5);
    steps(&mut w, 60);
    assert_eq!(
        w.raid
            .containers
            .iter()
            .filter(|c| c.holder == Holder::Body(id))
            .count(),
        1
    );
}

#[test]
fn a_dead_enemy_is_looked_at_once() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    let id = w.enemies[0].id;
    w.kill_enemy(0, Some(0));
    w.step();
    assert_eq!(w.raid.bodied, vec![id]);
    // Its body taken away, the dead enemy does not make another.
    w.raid.containers.retain(|c| c.holder != Holder::Body(id));
    steps(&mut w, 10);
    assert_eq!(w.raid.bodied, vec![id]);
    assert!(w
        .raid
        .containers
        .iter()
        .all(|c| c.holder != Holder::Body(id)));
}

#[test]
fn a_clients_mirror_makes_no_bodies_of_its_own() {
    let mut host = world_with("", BRUTE_FAR);
    join(&mut host, 0, "rogue", 6.0, 54.0);
    host.kill_enemy(0, Some(0));
    let mut mirror = World::new(host.content().clone());
    mirror.apply_snapshot(host.snapshot());
    let placed = mirror.raid.containers.len();
    mirror.step();
    assert_eq!(mirror.enemies[0].hp, 0.0, "the mirror has the dead enemy");
    assert_eq!(
        mirror.raid.containers.len(),
        placed,
        "and makes no body of it"
    );
    assert!(mirror.raid.bodied.is_empty());
}

fn entry(item: usize, weight: f32, count: (u16, u16)) -> crate::TableEntry {
    crate::TableEntry {
        item,
        weight,
        count,
    }
}

fn table(rolls: (u8, u8), nothing: f32, items: Vec<crate::TableEntry>) -> Table {
    Table {
        key: "t".into(),
        rolls,
        nothing,
        items,
    }
}

#[test]
fn a_table_that_cannot_roll_rolls_nothing_and_does_not_panic() {
    let w = world();
    let c = w.content();
    let mut dice = Dice::new(5);
    let mut roll = |t: Table| roll_table(&t, &mut dice, c);
    // No entries, nothing to roll.
    assert!(roll(table((3, 3), 0.0, vec![])).is_empty());
    // All weight on "nothing".
    assert!(roll(table((3, 3), 5.0, vec![])).is_empty());
    // Not a number.
    assert!(roll(table((3, 3), f32::NAN, vec![entry(0, 1.0, (1, 3))])).is_empty());
    // An item the content does not have.
    assert!(roll(table((3, 3), 0.0, vec![entry(999, 1.0, (1, 3))])).is_empty());
    // Zero rolls.
    assert!(roll(table((0, 0), 0.0, vec![entry(0, 1.0, (1, 3))])).is_empty());
    // A reversed range gives its low end.
    let gem = item(&w, "gem");
    assert_eq!(
        roll(table((1, 1), 0.0, vec![entry(gem, 1.0, (5, 2))])),
        vec![Stack::found(gem, 5)]
    );
}

#[test]
fn a_roll_past_a_stack_size_spills_into_the_next_stack() {
    let w = world();
    let gem = item(&w, "gem");
    let t = table((6, 6), 0.0, vec![entry(gem, 1.0, (4, 4))]);
    let got = roll_table(&t, &mut Dice::new(9), w.content());
    // Twenty-four gems, ten to a stack.
    assert_eq!(
        got,
        vec![
            Stack::found(gem, 10),
            Stack::found(gem, 10),
            Stack::found(gem, 4)
        ]
    );
}

#[test]
fn a_container_whose_kind_the_content_lacks_opens_at_once_and_is_quiet() {
    let w = world();
    assert_eq!(search_of(Holder::Placed(99), w.content()), (0.0, None));
}

#[test]
fn a_friend_picks_up_what_was_dropped() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 6.0, 5.0);
    stand(&mut w, 0, 5.0, 5.0);
    stand(&mut w, 1, 6.0, 5.0);
    w.step();
    w.give_loadout(0, &loadout(&w, &[("bread", 3)], &[], &[]));
    w.give_loadout(1, &loadout(&w, &[], &[], &[]));
    act(&mut w, 0, 1, Act::Drop(Place::Pack(0)));
    stand(&mut w, 1, 6.0, 5.0);
    w.step();
    let pile = w
        .loot_view(Some(1))
        .marks
        .iter()
        .find(|m| m.holder == Holder::Pile)
        .unwrap()
        .id;
    act(&mut w, 1, 1, Act::Open(pile));
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    act(&mut w, 1, 2, Act::TakeAll);
    w.step();
    assert_eq!(w.pack(1).unwrap().count_of(item(&w, "bread")), 3);
    assert_eq!(w.pack(0).unwrap().count_of(item(&w, "bread")), 0);
}

/// One whole scripted raid: loadouts, acts, a search, a body, a drop. Its end state, for the
/// determinism test to compare.
fn scripted_raid(
    seed: u64,
) -> (
    Vec<Container>,
    Vec<(PlayerId, Inventory)>,
    Vec<crate::Event>,
) {
    let mut w = world_with("", BRUTE_FAR);
    let roll = RaidRoll::standard(w.content(), seed);
    w.begin_raid(roll).unwrap();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    join(&mut w, 1, "rogue", 13.0, 5.0);
    w.give_loadout(
        0,
        &loadout(
            &w,
            &[("bread", 3), ("potion", 2)],
            &[("knife", EquipSlot::MainHand)],
            &[],
        ),
    );
    w.give_loadout(1, &loadout(&w, &[("rock", 2)], &[], &[("key", 1)]));
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    steps(&mut w, (2.2 / DT) as usize);
    act(&mut w, 0, 2, Act::TakeAll);
    w.step();
    act(&mut w, 1, 1, Act::Drop(Place::Pack(0)));
    w.step();
    w.kill_enemy(0, Some(1));
    steps(&mut w, 5);
    let body = w
        .raid
        .containers
        .iter()
        .find(|c| matches!(c.holder, Holder::Body(_)))
        .unwrap()
        .id;
    stand(&mut w, 1, 6.0, 54.0);
    act(&mut w, 1, 2, Act::Open(body));
    steps(&mut w, 60);
    (
        w.raid.containers.clone(),
        w.raid.packs.clone(),
        w.events().to_vec(),
    )
}

#[test]
fn a_scripted_raid_plays_out_the_same_twice() {
    assert_eq!(scripted_raid(1234), scripted_raid(1234));
    assert_ne!(scripted_raid(1234).0, scripted_raid(99).0);
}
