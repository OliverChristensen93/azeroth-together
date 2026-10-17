//! What is on each unit, as its aura fields would say (spec §6): a hero's Stealth, Sprint and meal
//! in their buff slots (the buff frame, and Stealth's fade), an enemy's holds, slows and bleeds in
//! its debuff slots (the target frame's debuffs, and each aura's own look: Frost Nova's ice, a
//! stun's stars, Polymorph's sparkle); a hero's stand state (kneeling downed, sitting at a meal),
//! stealth's creep bit, and the Stealth form that turns a rogue's bar to its stealth page (actions
//! 73 to 84, as WoW's own does when `ui_action`'s feed reads the form); and a polymorphed enemy's
//! sheep. Each unit is written only when what is on it changed, ringing the aura watch as a
//! server's values update would.

use std::collections::{HashMap, HashSet};

use benilla_protocol::field::{FIELD_UNIT_AURA, FIELD_UNIT_AURAFLAGS, FIELD_UNIT_BYTES_1};
use benilla_protocol::messages::{ObjectFields, ObjectType};
use bevy::prelude::*;
use nightfall_sim::{ControlKind, Effect, HeroFight, Life, PlayerId};

use super::conceal;
use super::enemies::EnemyViews;
use super::run::Sim;
use super::view::Views;
use crate::net::{FieldChanged, Guid, ObjectStore};

/// `UNIT_FIELD_DISPLAYID`: the model drawn, which the engine swaps in place when it moves.
const FIELD_UNIT_DISPLAYID: u16 = 131;
/// The sheep Polymorph turns an enemy into.
const SHEEP_DISPLAY: u32 = 856;
/// Stealth's aura: Rank 3's, the one whose visual fades a body (sub-project 3's ruling). The
/// bar's own Stealth is another rank, whose aura is not known to fade a body.
const STEALTH_AURA: u32 = 1786;
/// `SpellShapeshiftForm.dbc`'s Stealth: the form whose bonus bar is a rogue's stealth page.
const STEALTH_FORM: u8 = 30;
/// `UNIT_FIELD_BYTES_1` byte 0's stand states.
const STAND: u8 = 0;
const SIT: u8 = 1;
const KNEEL: u8 = 8;
/// Aura slots: 32 buffs, then 16 debuffs.
const BUFF_SLOTS: usize = 32;
const DEBUFF_SLOTS: usize = 16;
/// An aura's `AURAFLAGS` nibble: a cancelable buff's, and a debuff's (an effect bit alone).
const BUFF_FLAGS: u32 = 0x3;
const DEBUFF_FLAGS: u32 = 0x2;

/// Every aura field for `buffs` (from slot 0) and `debuffs` (from slot 32): each slot's spell, 0
/// where empty, and the six flag words.
pub(super) fn aura_fields(buffs: &[u32], debuffs: &[u32]) -> Vec<(u16, u32)> {
    let mut ids = [0u32; BUFF_SLOTS + DEBUFF_SLOTS];
    let mut flags = [0u32; (BUFF_SLOTS + DEBUFF_SLOTS) / 8];
    let slots = buffs
        .iter()
        .take(BUFF_SLOTS)
        .enumerate()
        .map(|(i, &s)| (i, s, BUFF_FLAGS))
        .chain(
            debuffs
                .iter()
                .take(DEBUFF_SLOTS)
                .enumerate()
                .map(|(j, &s)| (BUFF_SLOTS + j, s, DEBUFF_FLAGS)),
        );
    for (i, spell, f) in slots {
        ids[i] = spell;
        flags[i / 8] |= f << ((i % 8) * 4);
    }
    ids.iter()
        .enumerate()
        .map(|(i, &s)| (FIELD_UNIT_AURA + i as u16, s))
        .chain(
            flags
                .iter()
                .enumerate()
                .map(|(w, &f)| (FIELD_UNIT_AURAFLAGS + w as u16, f)),
        )
        .collect()
}

/// How a hero stands: kneeling while downed, sitting at a meal, else up.
pub(super) fn stand_of(f: &HeroFight) -> u8 {
    match f.life {
        Life::Downed { .. } => KNEEL,
        _ if f.eating > 0.0 => SIT,
        _ => STAND,
    }
}

/// What a hero's body shows of the fight.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct HeroLook {
    body: Entity,
    buffs: Vec<u32>,
    stand: u8,
    creep: bool,
    form: u8,
}

/// The spell of the first ability on `class`'s bar that `pick` picks.
fn spell_on_bar(
    content: &nightfall_sim::Content,
    class: usize,
    pick: fn(&Effect) -> bool,
) -> Option<u32> {
    content.classes[class]
        .kit
        .iter()
        .map(|&(_, a)| &content.abilities[a])
        .find(|a| a.has(pick))
        .map(|a| a.spell)
}

/// Every hero's body shows their stealth, Sprint and meal as buffs, stands as they stand, creeps
/// in stealth and wears the Stealth form (a rogue's bar turns to its stealth page). Replaces
/// sub-project 3's `conceal::heroes`, which wrote Stealth's aura alone.
pub(super) fn heroes(
    sim: Res<Sim>,
    views: Res<Views>,
    mut bodies: Query<(&mut ObjectStore, &Guid)>,
    mut changed: MessageWriter<FieldChanged>,
    mut shown: Local<HashMap<PlayerId, HeroLook>>,
) {
    let content = sim.world.content();
    for hero in &sim.world.heroes {
        let Some(&e) = views.heroes.get(&hero.player) else {
            continue;
        };
        let sprint = spell_on_bar(content, hero.class, |e| matches!(e, Effect::Sprint { .. }));
        let meal = spell_on_bar(content, hero.class, |e| matches!(e, Effect::Eat { .. }));
        let buffs: Vec<u32> = [
            hero.stealthed.then_some(STEALTH_AURA),
            sprint.filter(|_| hero.fight.sprint > 0.0),
            meal.filter(|_| hero.fight.eating > 0.0),
        ]
        .into_iter()
        .flatten()
        .collect();
        let look = HeroLook {
            body: e,
            buffs,
            stand: stand_of(&hero.fight),
            creep: hero.stealthed,
            form: if hero.stealthed { STEALTH_FORM } else { 0 },
        };
        if shown.get(&hero.player) == Some(&look) {
            continue;
        }
        let Ok((mut store, guid)) = bodies.get_mut(e) else {
            continue;
        };
        let untrackable = store.0.unit_is_untrackable();
        let bytes = conceal::bytes_1(&store.0, Some(look.stand), look.creep, untrackable);
        let bytes = (bytes & !0x00FF_0000) | (u32::from(look.form) << 16);
        let mut pairs = vec![(FIELD_UNIT_BYTES_1, bytes)];
        pairs.extend(aura_fields(&look.buffs, &[]));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        changed.write(FieldChanged {
            entity: e,
            guid: guid.0,
            kind: ObjectType::Player,
            index: FIELD_UNIT_AURA,
            old: 0,
            new: 1,
        });
        shown.insert(hero.player, look);
    }
}

/// Every drawn enemy shows what is on it as debuffs, and a sheep while polymorphed and alive.
pub(super) fn enemies(
    sim: Res<Sim>,
    views: Res<EnemyViews>,
    mut bodies: Query<(&mut ObjectStore, &Guid)>,
    mut changed: MessageWriter<FieldChanged>,
    mut shown: Local<HashMap<u32, (Entity, Vec<u32>, u32)>>,
) {
    let content = sim.world.content();
    for enemy in &sim.world.enemies {
        let Some(&e) = views.drawn.get(&enemy.id) else {
            continue;
        };
        let display = if enemy.hp > 0.0 && enemy.fight.control == Some(ControlKind::Polymorph) {
            SHEEP_DISPLAY
        } else {
            content.enemies[enemy.kind].display
        };
        // Compared in place: a steady enemy costs no allocation.
        if shown
            .get(&enemy.id)
            .is_some_and(|(b, auras, d)| *b == e && *auras == enemy.fight.auras && *d == display)
        {
            continue;
        }
        let Ok((mut store, guid)) = bodies.get_mut(e) else {
            continue;
        };
        let mut pairs = aura_fields(&[], &enemy.fight.auras);
        pairs.push((FIELD_UNIT_DISPLAYID, display));
        store.0.merge(ObjectFields::from_pairs(&pairs));
        changed.write(FieldChanged {
            entity: e,
            guid: guid.0,
            kind: ObjectType::Unit,
            index: FIELD_UNIT_AURA,
            old: 0,
            new: 1,
        });
        shown.insert(enemy.id, (e, enemy.fight.auras.clone(), display));
    }
    // Enemies the world no longer has are forgotten, so the map never outgrows the world's (a
    // new run's enemy with an old id is written afresh). Counting what is still there costs a
    // lookup apiece; the pass that prunes runs only when something is stale.
    let kept = sim
        .world
        .enemies
        .iter()
        .filter(|e| shown.contains_key(&e.id))
        .count();
    if kept < shown.len() {
        let live: HashSet<u32> = sim.world.enemies.iter().map(|e| e.id).collect();
        shown.retain(|id, _| live.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffs_fill_from_the_first_slot_and_debuffs_from_the_thirty_third() {
        let f = aura_fields(&[1786, 2983], &[865]);
        assert_eq!(f.len(), 48 + 6);
        assert_eq!(f[0], (FIELD_UNIT_AURA, 1786));
        assert_eq!(f[1], (FIELD_UNIT_AURA + 1, 2983));
        assert_eq!(f[2], (FIELD_UNIT_AURA + 2, 0));
        assert_eq!(f[32], (FIELD_UNIT_AURA + 32, 865));
        // Slots 0 and 1 in the first flag word, slot 32 in the fifth.
        assert_eq!(f[48], (FIELD_UNIT_AURAFLAGS, 0x33));
        assert_eq!(f[52], (FIELD_UNIT_AURAFLAGS + 4, 0x2));
        assert!(aura_fields(&[], &[]).iter().all(|&(_, v)| v == 0));
    }

    #[test]
    fn more_auras_than_slots_fill_each_half_and_never_spill_into_the_other() {
        let buffs: Vec<u32> = (1..=33).collect();
        let debuffs: Vec<u32> = (101..=117).collect();
        let f = aura_fields(&buffs, &debuffs);
        assert_eq!(f.len(), 48 + 6, "the same fields, however many auras");
        // The 32 buff slots hold the first 32 buffs, the 33rd is dropped.
        for i in 0..BUFF_SLOTS {
            assert_eq!(f[i], (FIELD_UNIT_AURA + i as u16, i as u32 + 1));
        }
        // The 16 debuff slots hold the first 16 debuffs, the 17th is dropped.
        for j in 0..DEBUFF_SLOTS {
            let i = BUFF_SLOTS + j;
            assert_eq!(f[i], (FIELD_UNIT_AURA + i as u16, 101 + j as u32));
        }
        assert!(!f.iter().take(48).any(|&(_, v)| v == 33 || v == 117));
        // Flags: four words of buffs (0x3 a nibble), then two of debuffs (0x2 a nibble).
        for w in 0..4 {
            assert_eq!(f[48 + w], (FIELD_UNIT_AURAFLAGS + w as u16, 0x3333_3333));
        }
        for w in 4..6 {
            assert_eq!(f[48 + w], (FIELD_UNIT_AURAFLAGS + w as u16, 0x2222_2222));
        }
    }

    #[test]
    fn the_downed_kneel_and_the_eating_sit() {
        let mut f = HeroFight::default();
        assert_eq!(stand_of(&f), STAND);
        f.eating = 3.0;
        assert_eq!(stand_of(&f), SIT);
        f.life = Life::Downed {
            left: 30.0,
            revive: 0.0,
        };
        assert_eq!(stand_of(&f), KNEEL);
    }

    /// A rogue's body on the shipped content, with an enemy (id 5, kind 0) beside it.
    fn stage() -> (World, Entity, Entity) {
        let mut sim = nightfall_sim::World::new(super::super::shipped_content());
        sim.add_hero(0, "rogue", "Test", 1, 0).unwrap();
        sim.enemies.push(nightfall_sim::Enemy {
            id: 5,
            kind: 0,
            pos: nightfall_sim::V2::new(0.0, 0.0),
            z: 0.0,
            facing: nightfall_sim::V2::new(1.0, 0.0),
            hp: 10.0,
            max_hp: 10.0,
            speed: 0.0,
            asleep: false,
            fight: Default::default(),
        });
        let mut world = World::new();
        let spawn = |world: &mut World, guid: u64| {
            world
                .spawn((ObjectStore(ObjectFields::from_pairs(&[])), Guid(guid)))
                .id()
        };
        let hero = spawn(&mut world, 0x10);
        let enemy = spawn(&mut world, 0xF130_0000_0000_0005);
        let mut views = Views::default();
        views.heroes.insert(0, hero);
        let mut enemy_views = EnemyViews::default();
        enemy_views.drawn.insert(5, enemy);
        world.insert_resource(Sim::for_test(sim));
        world.insert_resource(views);
        world.insert_resource(enemy_views);
        world.init_resource::<Messages<FieldChanged>>();
        (world, hero, enemy)
    }

    fn store(world: &World, e: Entity) -> &ObjectFields {
        &world.get::<ObjectStore>(e).unwrap().0
    }

    fn changed_stores(world: &mut World) -> usize {
        world
            .query_filtered::<Entity, Changed<ObjectStore>>()
            .iter(world)
            .count()
    }

    fn rings(world: &mut World) -> usize {
        world
            .resource_mut::<Messages<FieldChanged>>()
            .drain()
            .count()
    }

    #[test]
    fn stealth_is_the_creep_bit_the_aura_and_the_stealth_form_and_a_steady_state_writes_nothing() {
        let (mut world, hero, _) = stage();
        let run = world.register_system(heroes);
        world.resource_mut::<Sim>().world.heroes[0].stealthed = true;
        world.run_system(run).unwrap();
        let s = store(&world, hero);
        assert!(s.unit_is_stealthed(), "the creep bit");
        let aura = s.unit_aura(0).expect("Stealth in the first slot");
        assert_eq!((aura.spell_id, aura.flags & 0xF), (1786, 0x3));
        assert_eq!(s.unit_shapeshift_form(), 30, "the stealth page of the bar");
        assert_eq!(rings(&mut world), 1, "the aura watch rings once");

        // Nothing changed: the second frame writes and rings nothing.
        world.clear_trackers();
        world.run_system(run).unwrap();
        assert_eq!(changed_stores(&mut world), 0);
        assert_eq!(rings(&mut world), 0);

        world.resource_mut::<Sim>().world.heroes[0].stealthed = false;
        world.run_system(run).unwrap();
        let s = store(&world, hero);
        assert!(!s.unit_is_stealthed());
        assert!(s.unit_aura(0).is_none());
        assert_eq!(s.unit_shapeshift_form(), 0);
        assert_eq!(rings(&mut world), 1);
    }

    #[test]
    fn a_sprint_and_a_meal_are_buffs_and_the_downed_kneel() {
        let (mut world, hero, _) = stage();
        let run = world.register_system(heroes);
        {
            let mut sim = world.resource_mut::<Sim>();
            let c = sim.world.content().clone();
            let f = &mut sim.world.heroes[0].fight;
            f.sprint = 4.0;
            f.eating = 2.0;
            // The shipped Sprint and Eat and Drink.
            assert!(c.abilities.iter().any(|a| a.spell == 2983));
            assert!(c.abilities.iter().any(|a| a.spell == 2639));
        }
        world.run_system(run).unwrap();
        let s = store(&world, hero);
        let on: Vec<u32> = s.unit_auras().map(|a| a.spell_id).collect();
        assert_eq!(on, [2983, 2639]);
        assert_eq!(s.unit_stand_state(), SIT);

        world.resource_mut::<Sim>().world.heroes[0].fight.life = Life::Downed {
            left: 20.0,
            revive: 0.0,
        };
        world.run_system(run).unwrap();
        assert_eq!(store(&world, hero).unit_stand_state(), KNEEL);
    }

    #[test]
    fn a_held_enemy_shows_its_debuff_and_a_sheep_until_the_hold_ends_or_it_dies() {
        let (mut world, _, enemy) = stage();
        let run = world.register_system(enemies);
        let plain = world.resource::<Sim>().world.content().enemies[0].display;
        world.run_system(run).unwrap();
        assert_eq!(store(&world, enemy).unit_displayid(), Some(plain as i32));
        assert_eq!(store(&world, enemy).unit_auras().count(), 0);

        {
            let mut sim = world.resource_mut::<Sim>();
            let f = &mut sim.world.enemies[0].fight;
            f.control = Some(ControlKind::Polymorph);
            f.auras = vec![118];
        }
        world.run_system(run).unwrap();
        let s = store(&world, enemy);
        assert_eq!(s.unit_displayid(), Some(SHEEP_DISPLAY as i32));
        let debuff = s.unit_aura(32).expect("the first debuff slot");
        assert_eq!((debuff.spell_id, debuff.flags & 0xF), (118, 0x2));

        // Steady: nothing rewritten.
        rings(&mut world);
        world.clear_trackers();
        world.run_system(run).unwrap();
        assert_eq!(changed_stores(&mut world), 0);
        assert_eq!(rings(&mut world), 0);

        // The hold ends: the model is its own again.
        world.resource_mut::<Sim>().world.enemies[0].fight.control = None;
        world.run_system(run).unwrap();
        assert_eq!(store(&world, enemy).unit_displayid(), Some(plain as i32));

        // Or the sheep dies.
        {
            let mut sim = world.resource_mut::<Sim>();
            sim.world.enemies[0].fight.control = Some(ControlKind::Polymorph);
        }
        world.run_system(run).unwrap();
        assert_eq!(
            store(&world, enemy).unit_displayid(),
            Some(SHEEP_DISPLAY as i32)
        );
        world.resource_mut::<Sim>().world.enemies[0].hp = 0.0;
        world.run_system(run).unwrap();
        assert_eq!(store(&world, enemy).unit_displayid(), Some(plain as i32));
    }

    #[test]
    fn an_enemy_that_left_the_world_is_forgotten_so_a_same_id_one_is_written_afresh() {
        let (mut world, _, enemy) = stage();
        let run = world.register_system(enemies);
        world.resource_mut::<Sim>().world.enemies[0].fight.auras = vec![118];
        world.run_system(run).unwrap();
        assert_eq!(store(&world, enemy).unit_auras().count(), 1);

        // It goes; the store is wiped as a new body's would be, and one with its id (and entity)
        // comes back with the same auras: forgotten, it is written again.
        let gone = world.resource_mut::<Sim>().world.enemies.remove(0);
        world.run_system(run).unwrap();
        world
            .get_mut::<ObjectStore>(enemy)
            .unwrap()
            .0
            .merge(ObjectFields::from_pairs(&aura_fields(&[], &[])));
        assert_eq!(store(&world, enemy).unit_auras().count(), 0);
        world.resource_mut::<Sim>().world.enemies.push(gone);
        world.run_system(run).unwrap();
        assert_eq!(store(&world, enemy).unit_auras().count(), 1);
    }
}
