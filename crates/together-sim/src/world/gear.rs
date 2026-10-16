//! Meta equipment in a run: what each hero brought ([`World::equip`]), switching a dormant piece
//! on with gold ([`World::switch_on`], a [`PlayerInput::gear_on`]), pieces dropping from bosses
//! and elites for every hero, and Atiesh's pull. The pieces themselves are [`crate::gear`].

use super::{Event, PlayerId, World};
use crate::content::AbilityDef;
use crate::gear::{
    item_level, roll_drop, Armor, GearPiece, HeroGear, Slot, Unique, ATIESH_PULL, ATIESH_REACH,
    BOSS_DROP, ELITE_DROP,
};
use crate::math::V2;
use crate::spell::CastSpec;
use crate::stats::Stat;

impl World {
    /// `player`'s hero brings `pieces` into the run, all dormant: the first valid one for each
    /// slot, and no off hand beside a two-handed weapon. Whatever it wore before is taken off.
    pub fn equip(&mut self, player: PlayerId, pieces: &[GearPiece]) {
        let Some(h) = self.heroes.iter().position(|x| x.player == player) else {
            return;
        };
        for p in self.heroes[h].gear.active().copied().collect::<Vec<_>>() {
            for (stat, v) in p.stats() {
                self.heroes[h].stats.add(stat, -v);
            }
        }
        self.heroes[h].gear = HeroGear::bring(pieces);
        self.refresh(h);
    }

    /// What switching on `player`'s dormant piece in `slot` costs now; `None` if it has none.
    pub fn gear_price(&self, player: PlayerId, slot: Slot) -> Option<u32> {
        self.hero(player)?.gear.price(slot)
    }

    /// Switch on `player`'s dormant piece in `slot`, paying its price from the hero's purse:
    /// its stats join the hero's, and a unique starts changing its spell. False if there is no
    /// such piece, it is on already, the hero has fallen or the purse is short.
    pub fn switch_on(&mut self, player: PlayerId, slot: Slot) -> bool {
        let Some(h) = self.heroes.iter().position(|x| x.player == player) else {
            return false;
        };
        let Some(price) = self.heroes[h].gear.price(slot) else {
            return false;
        };
        if !self.heroes[h].alive || self.heroes[h].gold < price {
            return false;
        }
        self.heroes[h].gold -= price;
        self.wake(h, slot);
        true
    }

    /// Switch on every dormant piece `player` brought, for nothing: for test runs and showcases.
    pub fn switch_all_on(&mut self, player: PlayerId) {
        let Some(h) = self.heroes.iter().position(|x| x.player == player) else {
            return;
        };
        for slot in Slot::ALL {
            if self.heroes[h].gear.price(slot).is_some() {
                self.wake(h, slot);
            }
        }
    }

    /// Hero `h`'s piece in `slot` goes on: its stats into the hero's, health and speed refreshed.
    fn wake(&mut self, h: usize, slot: Slot) {
        let hero = &mut self.heroes[h];
        let Some(w) = hero.gear.worn.iter_mut().find(|w| w.piece.slot == slot) else {
            return;
        };
        w.on = true;
        let piece = w.piece;
        for (stat, v) in piece.stats() {
            hero.stats.add(stat, v);
        }
        let player = hero.player;
        self.refresh(h);
        self.events.push(Event::GearOn { player, slot });
    }

    /// Each hero's [`crate::PlayerInput::gear_on`]: the slot it asked to switch on. Taken even
    /// while the run waits on a card screen, as picks are.
    pub(super) fn switch_gear(&mut self) {
        for h in 0..self.heroes.len() {
            let player = self.heroes[h].player;
            let Some(slot) = self
                .inputs
                .get_mut(&player)
                .and_then(|i| i.gear_on.take())
                .and_then(|s| Slot::from_index(usize::from(s)))
            else {
                continue;
            };
            self.switch_on(player, slot);
        }
    }

    /// A boss (`boss`) or an elite died: every hero rolls for a piece of their own, so nobody
    /// races a friend for loot. Pieces are of the zone's item level (a boss's a little higher),
    /// in the hero's own armour, a rarity rolled with its luck. They wait in the hero's
    /// [`HeroGear::found`] for the stash.
    pub(super) fn drop_gear(&mut self, boss: bool) {
        let chance = if boss { BOSS_DROP } else { ELITE_DROP };
        let level = item_level(self.zone().item_level, boss, 0);
        let content = self.content.clone();
        for h in 0..self.heroes.len() {
            if self.rng.f32() >= chance {
                continue;
            }
            let hero = &self.heroes[h];
            let class = &content.classes[hero.class];
            let spells: Vec<&str> = class.pool.iter().map(String::as_str).collect();
            let luck = hero.stats.get(Stat::Luck);
            let piece = roll_drop(
                &mut self.rng,
                boss,
                luck,
                level,
                Armor::for_class(&class.key),
                &spells,
            );
            let player = hero.player;
            self.heroes[h].gear.found.push(piece);
            self.events.push(Event::GearFound { player, piece });
        }
    }

    /// Hero `h`'s uniques' changes to a cast of `def`.
    pub(super) fn gear_shape(&self, h: usize, def: &AbilityDef, s: &mut CastSpec) {
        self.heroes[h].gear.shape(&def.key, s);
    }

    /// Atiesh: before hero `h`'s Arcane Explosion (`spec`) bursts at `center`, every enemy within
    /// [`ATIESH_REACH`] is dragged [`ATIESH_PULL`] yards toward it (an elite half as far, a boss
    /// not at all), never closer than a yard and a half.
    pub(super) fn gear_pull(&mut self, h: usize, spec: &CastSpec, center: V2) {
        let unique = Unique::Atiesh;
        if !self.heroes[h].gear.has(unique)
            || self.content.abilities[spec.ability].key != unique.spell()
        {
            return;
        }
        for e in self.enemies.iter_mut().filter(|e| !e.boss && e.hp > 0.0) {
            let off = e.pos - center;
            let d = off.length();
            if d > ATIESH_REACH || d <= 1.5 {
                continue;
            }
            let pull = if e.elite {
                ATIESH_PULL * 0.5
            } else {
                ATIESH_PULL
            };
            e.pos = center + off.normalize_or_zero() * (d - pull).max(1.5);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::cards::Rarity;
    use crate::world::{Enemy, PlayerInput, Status};

    /// A mage in Elwynn with the shipped content, the arena empty.
    fn world() -> World {
        let content = Arc::new(
            crate::Content::from_ron(crate::ContentSources {
                classes: include_str!("../../../../content/classes.ron"),
                abilities: include_str!("../../../../content/abilities.ron"),
                enemies: include_str!("../../../../content/enemies.ron"),
                zones: include_str!("../../../../content/zones.ron"),
            })
            .expect("shipped content is valid"),
        );
        let mut w = World::new(content, "elwynn", 1).unwrap();
        w.add_hero(0, "mage").unwrap();
        w.interactables.clear();
        w.camps.clear();
        w.spawn_timer = f32::MAX;
        w.next_boss = f32::MAX;
        w
    }

    /// The hero cannot fall.
    fn sturdy(w: &mut World) {
        w.heroes[0].stats.add(Stat::Health, 1e6);
        w.refresh(0);
        w.heroes[0].hp = w.heroes[0].max_hp;
    }

    /// An enemy with `hp` at `pos` that never moves or swings.
    fn dummy(w: &mut World, id: u32, pos: V2, hp: f32) {
        w.enemies.push(Enemy {
            id,
            def: 0,
            pos,
            facing: V2::ZERO,
            moving: false,
            hp,
            max_hp: hp.max(1.0),
            damage: 0.0,
            speed: 0.0,
            radius: 0.6,
            elite: false,
            boss: false,
            attack_cd: 100.0,
            last_hit: None,
            home: None,
            status: Status::default(),
        });
    }

    fn hood() -> GearPiece {
        GearPiece::new(Slot::Head, Rarity::Rare, 30, Armor::Cloth, 11)
    }

    #[test]
    fn brought_gear_is_dormant_until_its_gold_is_paid() {
        let mut w = world();
        w.equip(0, &[hood()]);
        let before = w.heroes[0].stats;
        let price = w.gear_price(0, Slot::Head).unwrap();
        assert!(!w.switch_on(0, Slot::Head), "no gold yet");
        assert_eq!(w.heroes[0].stats, before);
        w.heroes[0].gold = price + 5;
        assert!(w.switch_on(0, Slot::Head));
        assert_eq!(w.heroes[0].gold, 5);
        assert_ne!(w.heroes[0].stats, before);
        assert!(w.gear_price(0, Slot::Head).is_none(), "on already");
        assert!(!w.switch_on(0, Slot::Head));
        assert!(w.drain_events().iter().any(|e| matches!(
            e,
            Event::GearOn {
                player: 0,
                slot: Slot::Head
            }
        )));
    }

    #[test]
    fn a_switch_comes_from_the_players_input_even_while_cards_wait() {
        let mut w = world();
        w.equip(0, &[hood()]);
        w.heroes[0].gold = 1000;
        // A card screen is up: the run waits, but the purse can still be spent.
        w.heroes[0].offers.push(crate::world::Offer {
            cards: Vec::new(),
            source: crate::world::Source::LevelUp,
        });
        assert!(w.paused());
        w.set_input(
            0,
            PlayerInput {
                gear_on: Some(Slot::Head.index() as u8),
                ..Default::default()
            },
        );
        w.step();
        assert!(w.heroes[0].gear.worn[0].on);
        w.heroes[0].offers.clear();
        // Each switch is used once: a second piece is not switched on by the same input.
        w.equip(0, &[hood()]);
        w.step();
        assert!(!w.heroes[0].gear.worn[0].on);
    }

    #[test]
    fn a_guests_switch_survives_a_newer_movement_input() {
        // A co-op guest's inputs arrive faster than steps: the switch must not be lost.
        let mut w = world();
        w.equip(0, &[hood()]);
        w.heroes[0].gold = 1000;
        w.queue_input(
            0,
            PlayerInput {
                gear_on: Some(Slot::Head.index() as u8),
                ..Default::default()
            },
        );
        w.queue_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                ..Default::default()
            },
        );
        w.step();
        assert!(w.heroes[0].gear.worn[0].on);
    }

    #[test]
    fn a_mirror_sees_what_each_hero_wears_and_found() {
        let mut host = world();
        host.equip(0, &[hood(), GearPiece::unique(Unique::Atiesh, 30, 2)]);
        host.switch_all_on(0);
        host.drop_gear(true);
        host.drop_gear(true);
        host.drop_gear(true);
        let mut mirror = world();
        mirror.apply_snapshot(host.snapshot(), host.drain_events());
        let (h, m) = (&host.heroes[0].gear, &mirror.heroes[0].gear);
        assert_eq!(m, h);
        assert_eq!(m.active().count(), 2);
    }

    #[test]
    fn health_gear_raises_max_health_when_switched_on() {
        let mut w = world();
        let chest = (0..200u32)
            .map(|s| GearPiece::new(Slot::Chest, Rarity::Epic, 60, Armor::Cloth, s))
            .find(|p| p.stats().iter().any(|&(s, _)| s == Stat::Health))
            .expect("some seed rolls health");
        w.equip(0, &[chest]);
        let hp = w.heroes[0].max_hp;
        w.switch_all_on(0);
        assert!(w.heroes[0].max_hp > hp);
        // Taking it off (a new loadout) takes its stats back.
        w.equip(0, &[]);
        assert!((w.heroes[0].max_hp - hp).abs() < 1e-3);
    }

    #[test]
    fn bosses_drop_pieces_for_every_hero_and_elites_seldom() {
        let mut w = world();
        w.add_hero(1, "mage").unwrap();
        let mut boss_drops = 0;
        for _ in 0..200 {
            w.drop_gear(true);
        }
        for h in &w.heroes {
            boss_drops += h.gear.found.len();
            assert!(h.gear.found.iter().all(GearPiece::valid));
            assert!(h
                .gear
                .found
                .iter()
                .all(|p| p.item_level == w.zone().item_level + 3));
        }
        // Half of 400 rolls, give or take.
        assert!((150..250).contains(&boss_drops), "{boss_drops}");
        let mut w = world();
        for _ in 0..1000 {
            w.drop_gear(false);
        }
        let elite_drops = w.heroes[0].gear.found.len();
        assert!((2..30).contains(&elite_drops), "{elite_drops}");
    }

    #[test]
    fn a_dying_boss_rolls_for_gear() {
        let mut w = world();
        sturdy(&mut w);
        let mut found = 0;
        for i in 0..40 {
            let at = w.center() + V2::new(30.0, 0.0);
            dummy(&mut w, 9000 + i, at, 0.0);
            w.enemies.last_mut().unwrap().boss = true;
            w.step();
            found = w.heroes[0].gear.found.len();
        }
        assert!(found > 5, "{found}");
    }

    #[test]
    fn thunderfury_and_atiesh_change_their_spells_only_when_on() {
        let mut w = world();
        w.give_spell(0, "arcane_explosion", 1, &[]);
        let ae = w.heroes[0].abilities[1].clone();
        let plain = w.spell_spec(0, &ae);
        w.equip(0, &[GearPiece::unique(Unique::Atiesh, 30, 1)]);
        assert_eq!(w.spell_spec(0, &ae).radius, plain.radius, "dormant");
        w.switch_all_on(0);
        assert!(w.spell_spec(0, &ae).radius > plain.radius);
        // The pull: an enemy 10 yards off ends up 4 yards nearer; one beyond the reach stays.
        let c = w.heroes[0].pos;
        dummy(&mut w, 501, c + V2::new(10.0, 0.0), 1000.0);
        dummy(&mut w, 502, c + V2::new(0.0, 30.0), 1000.0);
        let spec = w.spell_spec(0, &ae);
        w.gear_pull(0, &spec, c);
        assert!((w.enemy(501).unwrap().pos.distance(c) - 6.0).abs() < 1e-3);
        assert!((w.enemy(502).unwrap().pos.distance(c) - 30.0).abs() < 1e-3);
    }
}
