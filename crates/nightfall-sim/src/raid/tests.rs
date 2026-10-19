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
    (key: "shutter", name: "Shuttered Lantern", kind: Lantern, icon: "a", weight: 1.0,
     light: Some((range: 10.0, intensity: 1.0, seen: Some(1.2)))),
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
        pet: None,
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

#[test]
fn a_hero_who_leaves_before_any_input_drops_nothing() {
    // Their client declined the raid or crashed before its stash committed the loadout: what they
    // were given is still in their stash, so a friend must not find it on the ground.
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.add_hero(1, "rogue", "Late", 1, 0).unwrap();
    w.give_loadout(1, &loadout(&w, &[("rock", 2)], &[], &[("bread", 1)]));
    stand(&mut w, 0, 5.0, 5.0);
    w.step();
    assert!(w.pack(1).is_some());
    w.remove_hero(1);
    assert!(
        w.raid.containers.iter().all(|c| c.holder != Holder::Pile),
        "no pile of the loadout they never gave up"
    );
    assert!(w.pack(1).is_none());
    assert!(w.raid.entered.iter().all(|p| *p != 1));
}

#[test]
fn a_hero_who_sent_a_pose_has_entered_and_drops_what_a_death_would() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.add_hero(1, "rogue", "Here", 1, 0).unwrap();
    w.give_loadout(1, &loadout(&w, &[("rock", 2)], &[], &[]));
    // An input with no pose (a client still loading) is not entering.
    w.set_input(1, PlayerInput::default());
    w.step();
    assert!(!w.raid.entered.contains(&1));
    stand(&mut w, 1, 8.0, 5.0);
    w.step();
    assert!(w.raid.entered.contains(&1));
    w.remove_hero(1);
    let held: Vec<Stack> = w
        .raid
        .containers
        .iter()
        .filter(|c| c.holder == Holder::Pile)
        .flat_map(|c| c.stacks.iter().flatten().copied())
        .collect();
    assert_eq!(held, vec![Stack::new(item(&w, "rock"), 2)]);
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

/// `player`'s input: their hero at `(x, y)` and act `a` (press `seq`) in the same input.
fn act_at(w: &mut World, player: PlayerId, seq: u8, (x, y): (f32, f32), a: Act) {
    w.set_input(
        player,
        PlayerInput {
            pose: pose(x, y),
            act: Some(ActPress { seq, act: a }),
            ..Default::default()
        },
    );
    w.step();
    assert_eq!(w.hero(player).unwrap().carry.acted, seq);
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
    let mut seq = 0;
    // Each act on its own. The pile is open and the hero by it; then the act comes in an input
    // that also has the hero far away. The act is taken before the raid closes what the hero
    // has walked off from, so only the reach check at the take or put refuses it.
    for a in [Act::TakeAll, Act::Take(0), Act::Put(Place::Pack(0))] {
        seq += 1;
        act_at(&mut w, 0, seq, (5.0, 5.0), Act::Open(id));
        assert_eq!(w.raid.open_of(0), Some(id), "{a:?}: open before the act");
        let (pack, held) = (
            w.pack(0).unwrap().clone(),
            w.raid.containers[0].stacks.clone(),
        );
        seq += 1;
        act_at(&mut w, 0, seq, (40.0, 40.0), a);
        assert!(w.hero(0).unwrap().pos.distance(V2::new(5.0, 5.0)) > 30.0);
        assert_eq!(w.pack(0), Some(&pack), "{a:?} took or put something");
        assert_eq!(w.raid.containers[0].stacks, held, "{a:?} changed the pile");
    }
    // Walking off closed it; back in reach, they open it again and the same act works.
    act_at(&mut w, 0, seq + 1, (5.0, 5.0), Act::Open(id));
    act_at(&mut w, 0, seq + 2, (5.0, 5.0), Act::TakeAll);
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
            pet: None,
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
    assert_eq!(w.raid.bodied, BTreeSet::from([id]));
    // Its body taken away, the dead enemy does not make another.
    w.raid.containers.retain(|c| c.holder != Holder::Body(id));
    steps(&mut w, 10);
    assert_eq!(w.raid.bodied, BTreeSet::from([id]));
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

/// One whole scripted raid: loadouts, acts, a search, a drop, a kill, a body searched and
/// emptied. Its end state, for the tests to compare and to check the script did its work.
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
    // Hero 1 walks to the body (a step to get there: `act` takes the hero's position as it is),
    // searches it, and empties it.
    stand(&mut w, 1, 6.0, 54.0);
    w.step();
    assert!(w.hero(1).unwrap().pos.distance(V2::new(6.0, 54.0)) < 0.5);
    act(&mut w, 1, 2, Act::Open(body));
    steps(&mut w, 60);
    act(&mut w, 1, 3, Act::TakeAll);
    w.step();
    (
        w.raid.containers.clone(),
        w.raid.packs.clone(),
        w.events().to_vec(),
    )
}

#[test]
fn a_scripted_raid_plays_out_the_same_twice() {
    let first = scripted_raid(1234);
    assert_eq!(first, scripted_raid(1234));
    assert_ne!(first.0, scripted_raid(99).0);
}

#[test]
fn the_scripted_raid_does_what_it_says() {
    let w = world_with("", BRUTE_FAR);
    let gem = item(&w, "gem");
    let (containers, packs, _) = scripted_raid(1234);
    let held = |p: PlayerId| packs.iter().find(|(q, _)| *q == p).unwrap().1.count_of(gem);
    // Hero 0 emptied the chest; hero 1 the body.
    assert_eq!(held(0), 6);
    assert_eq!(held(1), 6);
    let body = containers
        .iter()
        .find(|c| matches!(c.holder, Holder::Body(_)))
        .expect("the enemy left a body");
    assert!(body.searched, "hero 1 searched it");
    assert!(body.stacks.iter().all(Option::is_none));
    let pile = containers
        .iter()
        .find(|c| c.holder == Holder::Pile)
        .expect("hero 1 dropped their rocks");
    assert_eq!(
        pile.stacks.iter().flatten().copied().collect::<Vec<_>>(),
        vec![Stack::new(item(&w, "rock"), 2)]
    );
}

#[test]
fn raid_seeds_that_differ_in_one_bit_roll_different_loot() {
    use containers::seed_for;
    let w = world();
    let c = w.content();
    // The dice set bit 0 of any seed, so seeds 2k and 2k+1 must differ before that.
    for k in 0..200u64 {
        for i in 0..4 {
            assert_ne!(
                seed_for(2 * k, i) | 1,
                seed_for(2 * k + 1, i) | 1,
                "{k} {i}"
            );
        }
    }
    // And the loot of a container whose table has chance in it differs for most such pairs.
    let mixed = &c.loot.tables[1];
    let roll = |seed: u64| roll_table(mixed, &mut Dice::new(seed_for(seed, 1)), c);
    let differing = (0..50u64)
        .filter(|&k| roll(2 * k) != roll(2 * k + 1))
        .count();
    assert!(
        differing >= 25,
        "only {differing} of 50 neighbouring seeds differ"
    );
}

/// A grave at (12, 20) and its searcher at (12, 18): the long search, so several beats of it.
/// Counts the beats of the search by running its step by hand, clearing the noises the senses
/// would have listened to between one step and the next.
#[test]
fn a_search_is_heard_every_noise_every_until_it_ends_or_is_given_up() {
    let beats = |w: &mut World, steps: usize| -> Vec<usize> {
        (0..steps)
            .filter(|_| {
                w.senses.noises.clear();
                w.step_searches();
                !w.senses.noises.is_empty()
            })
            .collect()
    };
    let mut w = world();
    join(&mut w, 0, "rogue", 12.0, 18.0);
    let grave = w.raid.containers[1].id;
    act(&mut w, 0, 1, Act::Open(grave));
    w.step();
    // The first beat came in that step. The next are two seconds apart (the rules' default).
    assert_eq!(w.content().rules.run.noise_every, 2.0);
    let (total, sound) = search_of(Holder::Placed(1), w.content());
    assert_eq!((total, sound), (6.0, Some(crate::Loudness::Dig)));
    let at: Vec<usize> = beats(&mut w, 400);
    // A second beat at 2 s and a third at 4 s from the first step, give or take a step of
    // rounding; none once it ends at 6 s.
    assert_eq!(at.len(), 2, "{at:?}");
    for (got, want) in at.iter().zip([59usize, 119]) {
        assert!(got.abs_diff(want) <= 1, "{at:?}");
    }
    assert!(w.raid.searches.is_empty());
    assert!(w.raid.containers[1].searched);
    // Given up: begin a search of the chest, walk off, and no beat is heard again.
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    assert!(!w.raid.searches.is_empty());
    stand(&mut w, 0, 11.0, 8.0);
    w.step();
    assert!(w.raid.searches.is_empty());
    assert!(beats(&mut w, 200).is_empty());
}

#[test]
fn a_search_is_heard_by_a_calm_enemy_in_range_in_the_same_step() {
    // A brute 14 yards from the chest and 15 from the searcher, facing away from both.
    let post =
        r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (26.0, 5.0), facing: 0.0)),"#;
    let mut w = world_with("", post);
    join(&mut w, 0, "rogue", 11.0, 5.0);
    let noise = w.content().rules.senses.awareness.noise;
    assert_eq!(w.senses.suspicion_of(0, 0), 0.0);
    // Without a search, a few steps of standing there rouse nothing.
    steps(&mut w, 10);
    assert_eq!(w.senses.suspicion_of(0, 0), 0.0);
    let id = chest(&w);
    act(&mut w, 0, 1, Act::Open(id));
    w.step();
    assert_eq!(w.senses.suspicion_of(0, 0), noise);
}

#[test]
fn enemies_killed_in_one_step_each_leave_exactly_one_body() {
    let spawns = r#"
        (place: "A", kind: "brute", behaviour: Sentry(at: (5.0, 55.0), facing: 0.0)),
        (place: "B", kind: "brute", behaviour: Sentry(at: (15.0, 55.0), facing: 0.0)),
        (place: "C", kind: "brute", behaviour: Sentry(at: (25.0, 55.0), facing: 0.0)),"#;
    let mut w = world_with("", spawns);
    join(&mut w, 0, "rogue", 6.0, 40.0);
    let ids: Vec<u32> = w.enemies.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 3);
    for i in 0..3 {
        w.kill_enemy(i, Some(0));
    }
    steps(&mut w, 3);
    for id in &ids {
        let n = w
            .raid
            .containers
            .iter()
            .filter(|c| c.holder == Holder::Body(*id))
            .count();
        assert_eq!(n, 1, "enemy {id}");
    }
    assert_eq!(
        w.raid.bodied,
        ids.iter().copied().collect::<BTreeSet<u32>>()
    );
}

// Exits and extracting, deaths and bodies, the raid's end and its outcomes.

fn body_of(w: &World, player: PlayerId) -> &Container {
    w.raid
        .containers
        .iter()
        .find(|c| c.holder == Holder::Hero(player))
        .unwrap()
}

fn die(w: &mut World, player: PlayerId) {
    let hi = w.hero_slot(player).unwrap();
    w.heroes[hi].fight.life = Life::Dead;
    w.step();
}

#[test]
fn ten_seconds_in_an_open_exit_gets_a_hero_out() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 2)], &[], &[]));
    steps(&mut w, (5.0 / DT) as usize);
    assert!((w.hero(0).unwrap().carry.extracting - 5.0).abs() < 0.1);
    steps(&mut w, (5.1 / DT) as usize);
    assert!(w.hero(0).is_none());
    let out = w.take_outcomes();
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].fate,
        Fate::Extracted {
            exit: "The Bridge".into()
        }
    );
    assert_eq!(out[0].carried.count_of(item(&w, "bread")), 2);
    assert!(out[0].time > 9.9);
    assert!(w.is_over());
}

#[test]
fn extracting_is_heard_every_second() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    steps(&mut w, 3);
    assert!(w.hero(0).unwrap().noise >= 25.0);
}

#[test]
fn extracting_is_made_at_the_ring_once_a_second_in_the_heros_name() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    // The senses clear the noises at the end of a step: count the beats by running the exits by
    // hand, as the step does, over four seconds.
    let mut beats = 0;
    for _ in 0..(4.0 / DT) as usize {
        w.senses.noises.clear();
        w.step_exits();
        if let Some(n) = w.senses.noises.last() {
            assert_eq!(n.by, 0);
            assert_eq!(n.radius, 25.0);
            beats += 1;
        }
        w.elapsed += DT;
    }
    assert!((3..=4).contains(&beats), "{beats} beats in four seconds");
}

#[test]
fn extracting_is_heard_by_a_calm_enemy_in_range() {
    // A brute 19 yards from the ring, facing away.
    let post =
        r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (50.0, 31.0), facing: 0.0)),"#;
    let mut away = world_with("", post);
    join(&mut away, 0, "rogue", 38.0, 49.0);
    steps(&mut away, 10);
    assert_eq!(
        away.senses.suspicion_of(0, 0),
        0.0,
        "nothing rouses it outside the ring"
    );
    let mut w = world_with("", post);
    join(&mut w, 0, "rogue", 50.0, 49.0);
    let noise = w.content().rules.senses.awareness.noise;
    assert_eq!(w.senses.suspicion_of(0, 0), noise);
}

#[test]
fn a_blow_starts_the_count_over() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    steps(&mut w, (9.0 / DT) as usize);
    w.hurt_hero(0, 10.0);
    w.step();
    assert!(w.hero(0).unwrap().carry.extracting < 0.1);
    steps(&mut w, (9.0 / DT) as usize);
    assert!(w.hero(0).is_some());
    steps(&mut w, (1.2 / DT) as usize);
    assert!(w.hero(0).is_none());
}

#[test]
fn the_cart_opens_late() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 11.0);
    steps(&mut w, (12.0 / DT) as usize);
    assert_eq!(w.hero(0).unwrap().carry.extracting, 0.0);
    let cart = w.loot_view(Some(0)).exits[1];
    assert!(!cart.open);
    assert!((cart.opens_in.unwrap() - 8.0).abs() < 0.2);
    steps(&mut w, (18.5 / DT) as usize);
    assert!(
        w.hero(0).is_none(),
        "out once the cart opened and ten seconds passed"
    );
}

#[test]
fn the_downed_do_not_extract() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.heroes[0].fight.life = Life::Downed {
        left: 40.0,
        revive: 0.0,
    };
    steps(&mut w, (11.0 / DT) as usize);
    let h = w.hero(0).unwrap();
    assert_eq!(h.carry.extracting, 0.0);
}

#[test]
fn a_shut_exit_or_a_missing_roll_never_lets_a_hero_out_and_never_panics() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.raid.roll.exits[0].open = false;
    steps(&mut w, (11.0 / DT) as usize);
    assert!(w.hero(0).is_some());
    assert_eq!(w.hero(0).unwrap().carry.extracting, 0.0);
    // The roll has no exits at all: nothing to stand in.
    w.raid.roll.exits.clear();
    steps(&mut w, (11.0 / DT) as usize);
    assert!(w.hero(0).is_some());
    assert!(!w.is_over());
}

#[test]
fn a_bad_exit_index_still_gets_the_hero_out() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.extract(0, 99);
    assert!(w.hero(0).is_none());
    let out = w.take_outcomes();
    assert_eq!(
        out[0].fate,
        Fate::Extracted {
            exit: String::new()
        }
    );
    // Nobody left to take out: nothing happens, and nothing is settled twice.
    w.extract(0, 0);
    assert!(w.take_outcomes().is_empty());
}

#[test]
fn a_hero_removed_mid_extraction_leaves_the_other_to_finish() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    join(&mut w, 1, "rogue", 51.0, 49.0);
    stand(&mut w, 0, 50.0, 49.0);
    stand(&mut w, 1, 51.0, 49.0);
    steps(&mut w, (5.0 / DT) as usize);
    w.remove_hero(0);
    assert!(w.raid.exiting.iter().all(|e| e.player != 0 || e.t == 0.0));
    stand(&mut w, 1, 51.0, 49.0);
    steps(&mut w, (5.5 / DT) as usize);
    assert!(w.hero(1).is_none());
    let out = w.take_outcomes();
    assert_eq!(out.len(), 1, "the hero who left the session has no outcome");
    assert_eq!(out[0].player, 1);
    assert!(w.is_over());
}

#[test]
fn what_was_found_is_simply_carried_once_out() {
    let mut w = world();
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    let c = chest(&w);
    act(&mut w, 0, 1, Act::Open(c));
    steps(&mut w, (2.2 / DT) as usize);
    act(&mut w, 0, 2, Act::Take(0));
    w.step();
    assert!(w.pack(0).unwrap().stacks().iter().any(|(_, s)| s.found));
    stand(&mut w, 0, 50.0, 49.0);
    steps(&mut w, (10.3 / DT) as usize);
    let out = w.take_outcomes();
    assert_eq!(out[0].carried.count_of(item(&w, "gem")), 6);
    assert_eq!(out[0].carried.count_of(item(&w, "bread")), 1);
    assert!(out[0].carried.stacks().iter().all(|(_, s)| !s.found));
}

#[test]
fn death_takes_the_gear_to_the_body_and_keeps_the_safe_slot() {
    let mut w = world_with("death_loss: everything", "");
    join(&mut w, 0, "rogue", 20.0, 20.0);
    w.give_loadout(
        0,
        &loadout(
            &w,
            &[("bread", 1)],
            &[("knife", EquipSlot::MainHand)],
            &[("potion", 1)],
        ),
    );
    die(&mut w, 0);
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(
        body,
        vec![
            Stack::new(item(&w, "knife"), 1),
            Stack::new(item(&w, "bread"), 1)
        ]
    );
    assert!(w.is_over());
    let out = w.take_outcomes();
    assert_eq!(out[0].fate, Fate::Died);
    assert_eq!(
        out[0].carried.stacks(),
        vec![(Place::Safe(0), Stack::new(item(&w, "potion"), 1))]
    );
    assert_eq!(out[0].recovery, None);
}

#[test]
fn everything_one_recovery_leaves_the_body_for_the_next_raid() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    w.give_loadout(
        0,
        &loadout(&w, &[("gem", 3)], &[("knife", EquipSlot::MainHand)], &[]),
    );
    die(&mut w, 0);
    let out = w.take_outcomes();
    let r = out[0].recovery.clone().unwrap();
    assert_eq!(r.map, "test");
    assert!(r.at.ground().distance(V2::new(20.0, 20.0)) < 1e-4);
    assert_eq!(r.items.len(), 2);
    // The next raid: the remains lie where the hero fell, seen only by their player.
    let mut next = world();
    join(&mut next, 0, "rogue", 19.0, 20.0);
    next.give_loadout(
        0,
        &Loadout {
            inventory: Inventory::new(next.content()),
            recovery: Some(r.clone()),
            pet: None,
        },
    );
    let mine = next.loot_view(Some(0));
    let remains = mine
        .marks
        .iter()
        .find(|m| m.holder == Holder::Remains(0))
        .unwrap()
        .id;
    assert!(next
        .loot_view(Some(1))
        .marks
        .iter()
        .all(|m| m.holder != Holder::Remains(0)));
    act(&mut next, 0, 1, Act::Open(remains));
    next.step();
    assert!(next.raid.containers.iter().all(|c| c.id != remains));
    stand(&mut next, 0, 50.0, 49.0);
    steps(&mut next, (10.3 / DT) as usize);
    let out = next.take_outcomes();
    assert_eq!(out[0].recovered.len(), 2);
    assert!(out[0].recovered.iter().all(|s| !s.found));
    assert_eq!(out[0].recovery, None);
}

#[test]
fn found_only_loses_only_what_was_found() {
    let mut w = world_with("death_loss: found_only", "");
    join(&mut w, 0, "rogue", 11.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    let c = chest(&w);
    act(&mut w, 0, 1, Act::Open(c));
    steps(&mut w, (2.2 / DT) as usize);
    act(&mut w, 0, 2, Act::Take(0));
    w.step();
    die(&mut w, 0);
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(body, vec![Stack::found(item(&w, "gem"), 6)]);
    let out = w.take_outcomes();
    assert_eq!(out[0].carried.count_of(item(&w, "bread")), 1);
    assert_eq!(out[0].carried.count_of(item(&w, "gem")), 0);
    assert_eq!(out[0].recovery, None);
}

#[test]
fn a_brought_stack_a_found_bag_no_longer_holds_is_kept_by_the_dead_not_lost() {
    let mut w = world_with("death_loss: found_only", "");
    join(&mut w, 0, "rogue", 20.0, 20.0);
    // A found pouch makes the backpack 8 slots; five brought stacks fill five of them. With the
    // pouch gone the backpack is 4, and one brought stack has no slot.
    let mut inv = Inventory::new(w.content());
    inv.bags[0] = Some(Stack::found(item(&w, "pouch"), 1));
    inv.pack = vec![None; 8];
    let keys = ["bread", "water", "potion", "rock", "gem"];
    for (i, key) in keys.iter().enumerate() {
        inv.pack[i] = Some(Stack::new(item(&w, key), 1));
    }
    let lo = Loadout {
        inventory: inv,
        recovery: None,
        pet: None,
    };
    assert!(w.give_loadout(0, &lo).is_empty());
    assert_eq!(w.pack(0).unwrap().stacks().len(), 6);
    die(&mut w, 0);
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(
        body,
        vec![Stack::found(item(&w, "pouch"), 1)],
        "only the found bag is lost"
    );
    let out = w.take_outcomes();
    assert_eq!(out[0].fate, Fate::Died);
    let carried = &out[0].carried;
    assert_eq!(carried.pack.len(), 4, "the backpack is its own size again");
    assert_eq!(carried.stacks().len(), 4);
    assert_eq!(
        out[0].overflow,
        vec![Stack::new(item(&w, "gem"), 1)],
        "the brought stack with no slot is kept apart"
    );
    for key in keys {
        let n = carried.count_of(item(&w, key));
        let o: u32 = out[0]
            .overflow
            .iter()
            .filter(|s| usize::from(s.item) == item(&w, key))
            .map(|s| u32::from(s.count))
            .sum();
        assert_eq!(n + o, 1, "{key} is kept, in the pack or beside it");
    }
    assert!(carried.stacks().iter().all(|(_, s)| !s.found));
    assert_eq!(out[0].recovery, None);
}

#[test]
fn a_friend_loots_a_fallen_heros_body() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    join(&mut w, 1, "rogue", 21.0, 20.0);
    stand(&mut w, 0, 20.0, 20.0);
    stand(&mut w, 1, 21.0, 20.0);
    w.step();
    w.give_loadout(
        0,
        &loadout(&w, &[("gem", 6)], &[("knife", EquipSlot::MainHand)], &[]),
    );
    w.give_loadout(1, &loadout(&w, &[], &[], &[]));
    die(&mut w, 0);
    let body = body_of(&w, 0).id;
    act(&mut w, 1, 1, Act::Open(body));
    steps(&mut w, (1.7 / DT) as usize);
    // The body holds the worn knife first, then the backpack's gems.
    act(&mut w, 1, 2, Act::Take(1));
    w.step();
    assert_eq!(w.pack(1).unwrap().count_of(item(&w, "gem")), 6);
    stand(&mut w, 1, 50.0, 49.0);
    steps(&mut w, (10.3 / DT) as usize);
    assert!(w.is_over());
    let out = w.take_outcomes();
    let friend = out.iter().find(|o| o.player == 1).unwrap();
    let fallen = out.iter().find(|o| o.player == 0).unwrap();
    assert_eq!(friend.carried.count_of(item(&w, "gem")), 6);
    assert_eq!(fallen.fate, Fate::Died);
    let left = &fallen.recovery.as_ref().unwrap().items;
    assert_eq!(left, &vec![Stack::new(item(&w, "knife"), 1)]);
}

#[test]
fn a_dead_hero_stays_where_they_fell_whatever_their_client_reports() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    join(&mut w, 1, "rogue", 21.0, 20.0);
    stand(&mut w, 0, 20.0, 20.0);
    stand(&mut w, 1, 21.0, 20.0);
    w.step();
    w.give_loadout(0, &loadout(&w, &[("gem", 1)], &[], &[]));
    die(&mut w, 0);
    let spot = V2::new(20.0, 20.0);
    stand(&mut w, 0, 30.0, 30.0);
    steps(&mut w, 5);
    assert!(w.hero(0).unwrap().pos.distance(spot) < 1e-4);
    assert!(body_of(&w, 0).at.distance(spot) < 1e-4);
    // The living are not frozen by it.
    stand(&mut w, 1, 25.0, 25.0);
    w.step();
    assert!(w.hero(1).unwrap().pos.distance(V2::new(25.0, 25.0)) < 1e-4);
}

#[test]
fn each_outcome_is_taken_once() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    join(&mut w, 1, "rogue", 20.0, 20.0);
    stand(&mut w, 0, 50.0, 49.0);
    stand(&mut w, 1, 20.0, 20.0);
    w.step();
    die(&mut w, 1);
    assert!(!w.is_over());
    assert!(
        w.take_outcomes().is_empty(),
        "the dead are settled when the raid is over"
    );
    steps(&mut w, (10.3 / DT) as usize);
    let out = w.take_outcomes();
    assert_eq!(out.len(), 2);
    assert!(matches!(out[0].fate, Fate::Extracted { .. }));
    assert_eq!(out[1].fate, Fate::Died);
    assert!(w.take_outcomes().is_empty());
    assert_eq!(w.outcomes().len(), 2);
}

#[test]
fn abandoning_counts_everyone_still_in_as_missing() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    join(&mut w, 1, "rogue", 20.0, 20.0);
    stand(&mut w, 0, 5.0, 5.0);
    stand(&mut w, 1, 20.0, 20.0);
    w.step();
    w.give_loadout(
        0,
        &loadout(&w, &[], &[("knife", EquipSlot::MainHand)], &[("bread", 1)]),
    );
    w.give_loadout(1, &loadout(&w, &[("gem", 2)], &[], &[]));
    die(&mut w, 1);
    let bodies = w.raid.containers.len();
    w.abandon();
    assert!(w.is_over());
    assert_eq!(w.raid.containers.len(), bodies, "the missing leave no body");
    let out = w.take_outcomes();
    let missing = out.iter().find(|o| o.player == 0).unwrap();
    assert_eq!(missing.fate, Fate::Missing);
    assert_eq!(missing.carried.stacks().len(), 1);
    assert_eq!(missing.carried.count_of(item(&w, "bread")), 1);
    assert_eq!(missing.recovery, None);
    let dead = out.iter().find(|o| o.player == 1).unwrap();
    assert_eq!(dead.fate, Fate::Died);
    assert!(dead.recovery.is_some());
    // Abandoning twice settles nothing twice.
    w.abandon();
    assert!(w.take_outcomes().is_empty());
}

#[test]
fn a_kill_counts_for_its_hero() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    w.kill_enemy(0, Some(0));
    w.step();
    w.abandon();
    assert_eq!(w.take_outcomes()[0].kills, 1);
}

#[test]
fn kills_by_a_far_teammate_count_for_them_however_the_events_are_drained() {
    const TWO: &str = r#"
        (place: "A", kind: "brute", behaviour: Sentry(at: (5.0, 55.0), facing: 0.0)),
        (place: "B", kind: "brute", behaviour: Sentry(at: (15.0, 55.0), facing: 0.0)),"#;
    let mut w = world_with("", TWO);
    // Hero 0 stands far past any reach of the kills; hero 1 makes them.
    join(&mut w, 0, "rogue", 50.0, 10.0);
    join(&mut w, 1, "rogue", 8.0, 54.0);
    // A kill between steps, drained before any step tallied it (a feed polling the host).
    w.kill_enemy(0, Some(1));
    assert!(w
        .drain_events()
        .iter()
        .any(|e| matches!(e, crate::Event::EnemyDied { by: Some(1), .. })));
    // A kill in a step, then drained twice (the host's and a peer's), and stepped on.
    w.kill_enemy(1, Some(1));
    w.step();
    w.drain_events();
    w.drain_events();
    w.step();
    w.abandon();
    let out = w.take_outcomes();
    let killer = out.iter().find(|o| o.player == 1).unwrap();
    let far = out.iter().find(|o| o.player == 0).unwrap();
    assert_eq!(killer.kills, 2, "each kill counted once");
    assert_eq!(far.kills, 0);
}

#[test]
fn nothing_goes_on_after_the_raid_is_abandoned() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    join(&mut w, 1, "rogue", 20.0, 20.0);
    stand(&mut w, 0, 50.0, 49.0);
    stand(&mut w, 1, 20.0, 20.0);
    w.step();
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    w.give_loadout(
        1,
        &loadout(&w, &[("gem", 3)], &[("knife", EquipSlot::MainHand)], &[]),
    );
    die(&mut w, 1);
    steps(&mut w, (3.0 / DT) as usize);
    w.abandon();
    let first = w.take_outcomes();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].fate, Fate::Missing);
    let recovery = first[1].recovery.clone().unwrap();
    assert_eq!(recovery.items.len(), 2);
    let body = body_of(&w, 1).clone();
    // The missing hero walks to the body, opens it and takes all.
    stand(&mut w, 0, 21.0, 20.0);
    act(&mut w, 0, 1, Act::Open(body.id));
    steps(&mut w, (2.0 / DT) as usize);
    act(&mut w, 0, 2, Act::TakeAll);
    w.step();
    assert_eq!(body_of(&w, 1), &body, "the body is as it was left");
    assert!(w.pack(0).is_none(), "no pack was made for them again");
    // The act was answered, though nothing was done.
    assert_eq!(w.hero(0).unwrap().carry.acted, 2);
    // And back in the ring for longer than the count: no second outcome.
    stand(&mut w, 0, 50.0, 49.0);
    steps(&mut w, (11.0 / DT) as usize);
    assert!(w.hero(0).is_some(), "an abandoned raid lets nobody out");
    assert!(w.take_outcomes().is_empty());
    assert_eq!(w.outcomes().len(), 2);
    assert_eq!(w.outcomes()[1].recovery, Some(recovery));
}

#[test]
fn a_dead_heros_time_is_when_they_died() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    join(&mut w, 1, "rogue", 20.0, 20.0);
    stand(&mut w, 0, 50.0, 49.0);
    stand(&mut w, 1, 20.0, 20.0);
    steps(&mut w, 30);
    die(&mut w, 1);
    steps(&mut w, (11.0 / DT) as usize);
    let out = w.take_outcomes();
    assert_eq!(out.len(), 2);
    let out_by_exit = out.iter().find(|o| o.player == 0).unwrap();
    let dead = out.iter().find(|o| o.player == 1).unwrap();
    assert!(out_by_exit.time > 9.9, "{}", out_by_exit.time);
    assert!(
        (0.9..1.5).contains(&dead.time),
        "settled at the end, but dead at about a second: {}",
        dead.time
    );
}

#[test]
fn a_blow_restarts_the_count_even_when_a_potion_lands_in_the_same_step() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.give_loadout(0, &loadout(&w, &[("potion", 1)], &[], &[]));
    w.heroes[0].hp = 500.0;
    stand(&mut w, 0, 50.0, 49.0);
    steps(&mut w, (5.0 / DT) as usize);
    assert!(w.hero(0).unwrap().carry.extracting > 4.9);
    w.hurt_hero(0, 10.0);
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    w.step();
    let h = w.hero(0).unwrap();
    assert!(h.hp > 500.0, "the potion left them healthier than before");
    assert!(h.carry.extracting < 0.1, "{}", h.carry.extracting);
}

#[test]
fn taking_off_gear_in_the_ring_is_no_blow() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.give_loadout(0, &loadout(&w, &[], &[("helm", EquipSlot::Head)], &[]));
    assert_eq!(w.hero(0).unwrap().hp, 1100.0);
    steps(&mut w, (5.0 / DT) as usize);
    act(
        &mut w,
        0,
        1,
        Act::Move {
            from: Place::Equip(EquipSlot::Head),
            to: Place::Pack(0),
        },
    );
    w.step();
    let h = w.hero(0).unwrap();
    assert_eq!(h.max_hp, 1000.0);
    assert!(h.hp <= 1000.0, "less health, as the helm's is gone");
    assert!(h.carry.extracting > 4.9, "{}", h.carry.extracting);
}

#[test]
fn a_kill_in_the_step_a_hero_extracts_counts() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 50.0, 49.0);
    while w.hero(0).unwrap().carry.extracting + 1.5 * DT < w.content().rules.run.extract_for {
        w.step();
    }
    w.kill_enemy(0, Some(0));
    w.step();
    assert!(w.hero(0).is_none());
    assert_eq!(w.take_outcomes()[0].kills, 1);
}

/// How a conservation scenario ends the hero's raid.
#[derive(Clone, Copy, Debug)]
enum End {
    Extract,
    Die,
    /// The raid is abandoned while they are up and about.
    Missing,
}

/// Every unit of `key` the raid's settled outcomes, bodies and piles account for.
fn accounted(w: &World, out: &[HeroOutcome], key: &str) -> u32 {
    let i = item(w, key);
    let of = |s: &Stack| {
        if usize::from(s.item) == i {
            u32::from(s.count)
        } else {
            0
        }
    };
    let mut total = 0;
    for o in out {
        total += o.carried.count_of(i);
        total += o.overflow.iter().map(of).sum::<u32>();
        total += o.recovered.iter().map(of).sum::<u32>();
        // The recovery is a copy of the body's remains: the body stands for it, once.
        if let Some(r) = &o.recovery {
            total += r.items.iter().map(of).sum::<u32>();
        }
    }
    for c in &w.raid.containers {
        let remains_counted = matches!(c.holder, Holder::Hero(p)
            if out.iter().any(|o| o.player == p && o.recovery.is_some()));
        if matches!(c.holder, Holder::Pile)
            || (matches!(c.holder, Holder::Hero(_)) && !remains_counted)
        {
            total += c.stacks.iter().flatten().map(of).sum::<u32>();
        }
    }
    total
}

#[test]
fn nothing_is_made_or_lost_by_the_end_of_a_raid_in_any_death_mode() {
    for mode in ["everything", "everything_one_recovery", "found_only"] {
        for end in [End::Extract, End::Die, End::Missing] {
            let mut w = world_with(&format!("death_loss: {mode}"), "");
            join(&mut w, 0, "rogue", 11.0, 5.0);
            // Brought: 3 gems, 2 bread, a worn knife, a potion in the safe slot. Found: 6 gems.
            w.give_loadout(
                0,
                &loadout(
                    &w,
                    &[("gem", 3), ("bread", 2)],
                    &[("knife", EquipSlot::MainHand)],
                    &[("potion", 1)],
                ),
            );
            let c = chest(&w);
            act(&mut w, 0, 1, Act::Open(c));
            steps(&mut w, (2.2 / DT) as usize);
            act(&mut w, 0, 2, Act::Take(0));
            w.step();
            let want = [("gem", 9), ("bread", 2), ("knife", 1), ("potion", 1)];
            // The missing lose what a death would, with no body to hold it: it is gone by design.
            let mut gone = [0u32; 4];
            match end {
                End::Extract => {
                    stand(&mut w, 0, 50.0, 49.0);
                    steps(&mut w, (10.3 / DT) as usize);
                }
                End::Die => die(&mut w, 0),
                End::Missing => {
                    let content = w.content().clone();
                    let mut probe = w.pack(0).unwrap().clone();
                    let death = probe.split_for_death(content.rules.run.death_loss, &content);
                    for (g, (key, _)) in gone.iter_mut().zip(want) {
                        *g = death
                            .lost
                            .iter()
                            .filter(|s| usize::from(s.item) == item(&w, key))
                            .map(|s| u32::from(s.count))
                            .sum();
                    }
                    w.abandon();
                }
            }
            assert!(w.is_over(), "{mode} {end:?}");
            let out = w.take_outcomes();
            assert_eq!(out.len(), 1, "{mode} {end:?}");
            for ((key, n), g) in want.iter().zip(gone) {
                assert_eq!(
                    accounted(&w, &out, key),
                    n - g,
                    "{key} by {mode}, ended {end:?}"
                );
            }
            if matches!(end, End::Missing) && mode != "found_only" {
                assert!(
                    gone.iter().any(|g| *g > 0),
                    "{mode}: the missing lost things"
                );
            }
        }
    }
}

#[test]
fn two_heroes_finishing_in_the_same_step_are_both_out() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    join(&mut w, 1, "rogue", 21.0, 20.0);
    w.give_loadout(0, &loadout(&w, &[("bread", 1)], &[], &[]));
    w.give_loadout(1, &loadout(&w, &[("gem", 2)], &[], &[]));
    stand(&mut w, 0, 50.0, 49.0);
    stand(&mut w, 1, 51.0, 49.0);
    steps(&mut w, (10.3 / DT) as usize);
    assert!(w.heroes.is_empty());
    assert!(w.is_over());
    let out = w.take_outcomes();
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|o| matches!(o.fate, Fate::Extracted { .. })));
    let a = out.iter().find(|o| o.player == 0).unwrap();
    let b = out.iter().find(|o| o.player == 1).unwrap();
    assert_eq!(a.carried.count_of(item(&w, "bread")), 1);
    assert_eq!(b.carried.count_of(item(&w, "gem")), 2);
    assert_eq!(a.time, b.time);
}

#[test]
fn a_downed_hero_cannot_be_extracted() {
    let mut w = world();
    join(&mut w, 0, "rogue", 50.0, 49.0);
    w.heroes[0].fight.life = Life::Downed {
        left: 40.0,
        revive: 0.0,
    };
    w.extract(0, 0);
    assert!(w.hero(0).is_some());
    assert!(w.take_outcomes().is_empty());
}

#[test]
fn a_death_not_yet_noticed_is_settled_with_the_loss_taken() {
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    w.give_loadout(0, &loadout(&w, &[("gem", 3)], &[], &[]));
    // Dead, but no step has run to leave the body: abandoning must not settle them whole.
    w.heroes[0].fight.life = Life::Dead;
    w.abandon();
    let out = w.take_outcomes();
    assert_eq!(out[0].fate, Fate::Died);
    assert_eq!(out[0].carried.count_of(item(&w, "gem")), 0);
    assert_eq!(body_of(&w, 0).stacks.iter().flatten().count(), 1);
    assert_eq!(out[0].recovery.as_ref().unwrap().items.len(), 1);
}

/// How many of `key` the stacks hold.
fn count_saved(stacks: &[crate::SavedStack], key: &str) -> u32 {
    stacks
        .iter()
        .filter(|s| s.item == key)
        .map(|s| u32::from(s.count))
        .sum()
}

#[test]
fn a_leaver_who_dropped_a_stack_a_friend_took_is_not_given_it_back() {
    // The host treats a hero who leaves by what they hold when they go; the leaver's stash must
    // settle by the same, or the stack the friend took out would be in two places.
    for mode in ["everything", "found_only"] {
        let mut w = world_with(&format!("death_loss: {mode}"), "");
        join(&mut w, 0, "rogue", 5.0, 5.0);
        join(&mut w, 1, "rogue", 6.0, 5.0);
        let c = w.content().clone();
        let mut p0 = crate::Profile::default();
        p0.loadout.safe.push((
            0,
            crate::SavedStack {
                item: "gem".into(),
                count: 3,
            },
        ));
        p0.loadout.pack.push(crate::SavedStack {
            item: "bread".into(),
            count: 2,
        });
        p0.begin_raid(0, &c);
        w.give_loadout(0, &p0.raid_loadout(0, &c));
        w.give_loadout(1, &loadout(&w, &[], &[], &[]));
        // Hero 0 drops the safe-slot gems; hero 1 takes them and gets out.
        stand(&mut w, 1, 6.0, 5.0);
        act(&mut w, 0, 1, Act::Drop(Place::Safe(0)));
        w.step();
        let pile = w
            .loot_view(Some(1))
            .marks
            .iter()
            .find(|m| m.holder == Holder::Pile)
            .unwrap()
            .id;
        stand(&mut w, 0, 5.0, 5.0);
        loot_pile(&mut w, 1, pile, 1);
        assert_eq!(w.pack(1).unwrap().count_of(item(&w, "gem")), 3, "{mode}");
        w.extract(1, 0);
        let out = w.take_outcomes();
        assert_eq!(out.len(), 1);
        // Hero 0 is then lost to the session: the client noted their last pack, then the raid
        // ends for them with no outcome.
        let last = w.pack(0).unwrap().clone();
        w.remove_hero(0);
        assert!(p0.note_carry(&last, &c));
        assert!(p0.settle_missing(&c));
        for key in ["gem", "bread"] {
            let ground: u32 = w
                .raid
                .containers
                .iter()
                .filter(|k| k.holder == Holder::Pile)
                .flat_map(|k| k.stacks.iter().flatten())
                .filter(|s| c.items[usize::from(s.item)].key == key)
                .map(|s| u32::from(s.count))
                .sum();
            let friend = out[0].carried.count_of(item(&w, key)) as u32;
            let home = count_saved(&p0.stash, key)
                + count_saved(&p0.loadout.pack, key)
                + p0.loadout
                    .safe
                    .iter()
                    .filter(|(_, s)| s.item == key)
                    .map(|(_, s)| u32::from(s.count))
                    .sum::<u32>();
            let brought = if key == "gem" { 3 } else { 2 };
            assert_eq!(home + friend + ground, brought, "{key} under {mode}");
        }
        assert_eq!(count_saved(&p0.stash, "gem"), 0, "{mode}");
        assert!(p0.loadout.safe.is_empty(), "{mode}: the gems are not back");
    }
}

#[test]
fn the_final_beat_of_an_extraction_is_heard_and_then_forgotten() {
    // A brute 18 yards from the ring, facing away. The beat that gets a hero out is made in the
    // step they go, and `remove_hero` forgets every thought of them: the enemy must still have
    // heard it.
    let post =
        r#"(place: "Post", kind: "brute", behaviour: Sentry(at: (50.0, 31.0), facing: 0.0)),"#;
    let mut w = world_with("", post);
    join(&mut w, 0, "rogue", 50.0, 49.0);
    // The count is a step from its end.
    let end = w.content().rules.run.extract_for;
    w.raid.exiting[0].t = end - DT / 2.0;
    w.step();
    assert!(w.hero(0).is_none(), "out");
    assert!(w.is_over());
    let noise = w.content().rules.senses.awareness.noise;
    assert!(
        w.senses.suspicion_of(0, 0) >= noise - 0.01,
        "the brute heard the last beat: {}",
        w.senses.suspicion_of(0, 0)
    );
    // Nobody is there to be heard again: the suspicion of them drains away.
    steps(&mut w, (6.0 / DT) as usize);
    assert_eq!(w.senses.suspicion_of(0, 0), 0.0);
}

#[test]
fn a_world_begun_as_a_mirror_shows_no_loot_until_its_first_snapshot() {
    let host = world();
    assert!(
        !host.loot_view(Some(1)).marks.is_empty(),
        "the test map places containers"
    );
    let mut mirror = World::new(host.content().clone());
    mirror.begin_mirror();
    assert_eq!(mirror.loot_view(Some(1)), crate::LootView::default());
    assert_eq!(mirror.loot_view(None), crate::LootView::default());
    // A view the host sent is not undone by asking again.
    mirror.apply_snapshot(host.snapshot_for(1));
    let sent = mirror.loot_view(Some(1));
    assert!(!sent.marks.is_empty());
    mirror.begin_mirror();
    assert_eq!(mirror.loot_view(Some(1)), sent);
}

#[test]
fn the_companion_comes_into_the_raid_and_out_of_any_death() {
    let mut w = world_with("death_loss: everything", "");
    join(&mut w, 0, "rogue", 20.0, 20.0);
    let mut lo = loadout(&w, &[("bread", 1)], &[], &[]);
    lo.pet = Some(0);
    lo.inventory.pet = Some(Some(Stack::found(item(&w, "gem"), 3)));
    w.give_loadout(0, &lo);
    assert_eq!(w.hero(0).unwrap().carry.pet, Some(0));
    assert!((w.hero(0).unwrap().carry.load - 0.5).abs() < 1e-5);
    die(&mut w, 0);
    let out = w.take_outcomes();
    assert_eq!(
        out[0].carried.get(Place::Pet),
        Some(&Stack::new(item(&w, "gem"), 3))
    );
    let body: Vec<Stack> = body_of(&w, 0).stacks.iter().flatten().copied().collect();
    assert_eq!(body, vec![Stack::new(item(&w, "bread"), 1)]);
    // Without the companion, a stack in its slot comes back as left over, not into the raid.
    let mut w = world();
    join(&mut w, 0, "rogue", 20.0, 20.0);
    let mut lo = loadout(&w, &[], &[], &[]);
    lo.inventory.pet = Some(Some(Stack::new(item(&w, "gem"), 3)));
    assert_eq!(w.give_loadout(0, &lo), vec![Stack::new(item(&w, "gem"), 3)]);
    assert_eq!(w.pack(0).unwrap().pet, None);
}

#[test]
fn a_missing_raid_keeps_the_companions_stack_too() {
    for mode in ["everything", "everything_one_recovery", "found_only"] {
        let mut w = world_with(&format!("death_loss: {mode}"), BRUTE_FAR);
        join(&mut w, 0, "rogue", 20.0, 20.0);
        let mut lo = loadout(&w, &[("bread", 1)], &[], &[]);
        lo.pet = Some(0);
        lo.inventory.pet = Some(Some(Stack::found(item(&w, "gem"), 3)));
        w.give_loadout(0, &lo);
        w.abandon();
        let out = w.take_outcomes();
        assert_eq!(out[0].fate, Fate::Missing, "{mode}");
        assert_eq!(
            out[0].carried.get(Place::Pet),
            Some(&Stack::new(item(&w, "gem"), 3)),
            "{mode}"
        );
        // Kept once: not also on a body, and (weighing nothing) not weighed.
        assert_eq!(out[0].carried.count_of(item(&w, "gem")), 3, "{mode}");
        let bread = if mode == "found_only" { 0.5 } else { 0.0 };
        assert!(
            (out[0].carried.weight(w.content()) - bread).abs() < 1e-5,
            "{mode}"
        );
    }
}

#[test]
fn a_profile_keeps_the_companions_stack_through_a_raid_that_goes_missing() {
    let w = world_with("death_loss: everything", "");
    let c = w.content().clone();
    let pet_gems = || crate::SavedStack {
        item: "gem".into(),
        count: 3,
    };
    for noted in [false, true] {
        let mut p = crate::Profile::default();
        p.loadout.pet = Some(pet_gems());
        p.loadout.pack.push(crate::SavedStack {
            item: "bread".into(),
            count: 2,
        });
        p.begin_raid(0, &c);
        assert_eq!(p.in_raid.as_ref().unwrap().brought.pet, Some(pet_gems()));
        let mut lo = p.raid_loadout(0, &c);
        lo.pet = Some(0);
        if noted {
            let mut inv = lo.inventory.clone();
            // The raid took the bread; the companion still carries the gems.
            inv.take(Place::Pack(0));
            assert!(p.note_carry(&inv, &c));
            assert_eq!(
                p.in_raid.as_ref().unwrap().kept.as_ref().unwrap().pet,
                Some(pet_gems())
            );
        }
        assert!(p.settle_missing(&c));
        assert_eq!(p.loadout.pet, Some(pet_gems()), "noted: {noted}");
        assert_eq!(count_saved(&p.stash, "gem"), 0);
    }
}

#[test]
fn a_lantern_lights_from_the_backpack_not_from_the_companions_slot() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    let mut lo = loadout(&w, &[("lantern", 1)], &[], &[]);
    lo.pet = Some(0);
    w.give_loadout(0, &lo);
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    steps(&mut w, 2);
    assert!(w.hero(0).unwrap().carry.lantern.is_some(), "lit");
    // Into the companion's slot, it goes out: the companion carries things, not light.
    act(
        &mut w,
        0,
        2,
        Act::Move {
            from: Place::Pack(0),
            to: Place::Pet,
        },
    );
    steps(&mut w, 2);
    assert!(w.pack(0).unwrap().get(Place::Pet).is_some(), "it moved");
    assert_eq!(w.hero(0).unwrap().carry.lantern, None);
    assert!((w.hero(0).unwrap().visibility - 35.0).abs() < 1e-3);
}

#[test]
fn a_lantern_sets_how_easy_it_makes_its_hero_to_see() {
    let mut w = world();
    join(&mut w, 0, "rogue", 5.0, 5.0);
    w.give_loadout(0, &loadout(&w, &[("lantern", 1), ("shutter", 1)], &[], &[]));
    act(&mut w, 0, 1, Act::Use(Place::Pack(0)));
    steps(&mut w, 2);
    // Standing still: 35, times the rules' lantern 1.6.
    assert!((w.hero(0).unwrap().visibility - 56.0).abs() < 1e-3);
    act(&mut w, 0, 2, Act::Use(Place::Pack(1)));
    steps(&mut w, 2);
    assert!((w.hero(0).unwrap().visibility - 42.0).abs() < 1e-3);
}

#[test]
fn a_kill_says_what_and_where() {
    let mut w = world_with("", BRUTE_FAR);
    join(&mut w, 0, "rogue", 6.0, 54.0);
    w.kill_enemy(0, Some(0));
    w.step();
    w.abandon();
    assert_eq!(w.take_outcomes()[0].slain, vec![(0, V2::new(5.0, 55.0))]);
}

#[test]
fn a_kill_far_from_its_hero_counts_for_them_and_a_mirror_counts_nothing() {
    let mut host = world_with("", BRUTE_FAR);
    join(&mut host, 0, "rogue", 55.0, 5.0);
    host.kill_enemy(0, Some(0));
    host.step();
    let mut mirror = World::new(host.content().clone());
    mirror.apply_snapshot(host.snapshot());
    mirror.events = host.events.clone();
    mirror.raid.counted = 0;
    mirror.tally_kills();
    assert!(mirror.raid.slain.is_empty());
    assert!(mirror.raid.kills.is_empty());
    host.abandon();
    let out = host.take_outcomes();
    assert_eq!(out[0].kills, 1);
    assert_eq!(out[0].slain, vec![(0, V2::new(5.0, 55.0))]);
}
