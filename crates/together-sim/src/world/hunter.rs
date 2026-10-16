//! The hunter's capstones (docs/HUNTER-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Trick
//! Shot, Spread Shot, Wyvern Sting, Trueshot Aura and the rest) are in `Augment::shape`.

use super::combat::{Crit, Death, Ground};
use super::*;
use crate::augments::Augment;
use crate::spell::{control_secs, Tag};

/// Daze Wave's reach around the target; Minefield's extra traps' distance from the hunter;
/// Frozen Wake's beat and its patches' reach and seconds.
const DAZE_REACH: f32 = 6.0;
const MINE_SPREAD: f32 = 4.0;
const WAKE_EVERY: f32 = 3.0;
const WAKE_REACH: f32 = 3.0;
const WAKE_SECS: f32 = 3.0;
/// How far a jumping Hunter's Mark looks for its next target.
const MARK_REACH: f32 = 35.0;

impl World {
    /// The hunter's pet ground, if it has one out.
    fn pet(&self, h: usize) -> Option<usize> {
        let pet = self.content.ability("call_pet")?;
        let player = self.heroes[h].player;
        self.grounds
            .iter()
            .position(|g| g.by == player && g.spec.ability == pet)
    }

    /// The hunter capstones that change a cast with the hero's state: Sniper Training standing
    /// still.
    pub(super) fn hunter_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        if owned.has(Augment::SniperTraining) && self.heroes[h].spell.still >= 2.0 {
            s.range *= 1.5;
            s.damage *= 2.0;
        }
    }

    /// Before a hunter spell fires on its own cooldown: Arcane Barrage's every 4th shot.
    pub(super) fn hunter_before(&mut self, _h: usize, owned: &Owned, spec: &mut CastSpec) {
        if owned.has(Augment::ArcaneBarrage) && owned.casts % 4 == 3 {
            spec.count = spec.count.max(3);
        }
    }

    /// After a hunter spell fired: Minefield's two more traps.
    pub(super) fn hunter_after(&mut self, h: usize, owned: &Owned, spec: &CastSpec) {
        if owned.has(Augment::Minefield) {
            let (player, pos, facing) = (
                self.heroes[h].player,
                self.heroes[h].pos,
                self.heroes[h].facing.normalize_or_zero(),
            );
            let side = V2::new(-facing.y, facing.x);
            for dir in [side, side * -1.0] {
                self.add_trap(player, spec, pos + dir * MINE_SPREAD);
            }
        }
    }

    /// Extra crit chance: Careful Aim on a fresh target.
    pub(super) fn hunter_crit(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let e = &self.enemies[ei];
        let player = self.heroes[h].player;
        if e.hp > e.max_hp * 0.8 && self.spell_has(player, spec.ability, Augment::CarefulAim) {
            0.6
        } else {
            0.0
        }
    }

    /// How much harder a hunter's hit lands: Bestial Wrath's bites, Noxious Stings, Hunter's
    /// Prey.
    pub(super) fn hunter_scale(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let e = &self.enemies[ei];
        let fx = &self.heroes[h].spell.fx;
        let player = self.heroes[h].player;
        let mut m = 1.0;
        if Some(spec.ability) == self.content.ability("call_pet") {
            m *= 1.0 + fx.boon(Augment::BestialWrath);
        }
        if spec.tags.has(Tag::Physical) && self.knows(h, Augment::NoxiousStings) {
            let sting = self.content.ability("serpent_sting");
            if e.status
                .burns
                .iter()
                .any(|b| b.by == player && Some(b.ability) == sting)
            {
                m *= 1.1;
            }
        }
        if spec.tags.has(Tag::Projectile)
            && e.status.fx.mark_secs > 0.0
            && self.knows(h, Augment::HuntersPrey)
        {
            m *= 1.2;
        }
        m
    }

    /// Where a hunter's ground stands: Arrowstorm keeps on the thickest crowd.
    pub(super) fn hunter_ground_pos(&self, g: &Ground, pos: V2) -> V2 {
        if !self.spell_has(g.by, g.spec.ability, Augment::Arrowstorm) {
            return pos;
        }
        let Some(h) = self.hero_ix(g.by) else {
            return pos;
        };
        self.densest(self.heroes[h].pos, g.spec.range, g.spec.radius)
            .unwrap_or(pos)
    }

    /// A hunter's trap `g` was sprung at `pos` by `hit`: Freezing Trap holds the first.
    pub(super) fn hunter_trap_sprung(&mut self, g: &Ground, pos: V2, hit: &[u32]) {
        if !self.spell_has(g.by, g.spec.ability, Augment::FreezingTrap) {
            return;
        }
        let first = hit
            .iter()
            .filter_map(|&id| self.enemies.iter().position(|e| e.id == id))
            .min_by(|&a, &b| {
                let d = |i: usize| self.enemies[i].pos.distance(pos);
                d(a).total_cmp(&d(b))
            });
        if let Some(i) = first {
            let e = &mut self.enemies[i];
            e.status.freeze = e.status.freeze.max(control_secs(4.0, e.elite, e.boss));
        }
    }

    /// A hit of a hunter's spell `spec` on enemy `id`: the capstones that act on a hit or a crit.
    pub(super) fn hunter_strike(
        &mut self,
        h: usize,
        id: u32,
        spec: &CastSpec,
        _damage: f32,
        crit: bool,
        scale: f32,
    ) {
        let player = self.heroes[h].player;
        let Some(ei) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        let at = self.enemies[ei].pos;
        if Some(spec.ability) == self.content.ability("hunters_mark") {
            self.enemies[ei].status.fx.marked_by = Some(player);
        }
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        for a in augments {
            match a {
                Augment::LockAndLoad if crit => {
                    let aimed = self.content.ability("aimed_shot");
                    if let Some(o) = self.heroes[h]
                        .abilities
                        .iter_mut()
                        .find(|o| Some(o.ability) == aimed)
                    {
                        o.cd = 0.0;
                    }
                }
                Augment::SerpentSpread => self.cast_on(h, "serpent_sting", id),
                Augment::ImprovedConcussion if self.rng.f32() < 0.2 => {
                    let e = &mut self.enemies[ei];
                    if !e.boss {
                        e.status.stun = e.status.stun.max(control_secs(2.0, e.elite, false));
                    }
                }
                Augment::DazeWave if scale > 0.99 => {
                    for e in self
                        .enemies
                        .iter_mut()
                        .filter(|e| e.id != id && e.pos.distance(at) <= DAZE_REACH)
                    {
                        e.status.chill = e.status.chill.max(spec.chill);
                        e.status.slow = e.status.slow.max(spec.slow);
                    }
                }
                _ => {}
            }
        }
        let pet = self.content.ability("call_pet");
        if Some(spec.ability) == pet && self.spell_has(player, spec.ability, Augment::Intimidation)
        {
            if self.heroes[h].spell.fx.bump(Augment::Intimidation) % 5 == 0 {
                let e = &mut self.enemies[ei];
                if !e.boss {
                    e.status.stun = e.status.stun.max(control_secs(2.0, e.elite, false));
                }
            }
        }
        let shots = [
            self.content.ability("arcane_shot"),
            self.content.ability("aimed_shot"),
        ];
        if crit && shots.contains(&Some(spec.ability)) && self.knows(h, Augment::KillCommand) {
            if let Some(g) = self.pet(h) {
                let bite = self.grounds[g].spec.clone();
                self.strike(player, &bite, id, 2.0, at, Crit::Roll);
            }
        }
    }

    /// Each step for a hunter: Bestial Wrath's beat, Arrowstorm's stance, Frozen Wake.
    pub(super) fn hunter_tend(&mut self, h: usize) {
        let player = self.heroes[h].player;
        if self.pet(h).is_some()
            && self.knows(h, Augment::BestialWrath)
            && self.heroes[h].spell.ready(Augment::BestialWrath)
        {
            let sp = &mut self.heroes[h].spell;
            sp.spend(Augment::BestialWrath, 20.0);
            sp.fx.add_boon(Augment::BestialWrath, 2.0, 2.0, 6.0);
        }
        if let Some(volley) = self.content.ability("volley") {
            if self.spell_has(player, volley, Augment::Arrowstorm) {
                let moving = self.heroes[h].moving;
                for g in self
                    .grounds
                    .iter_mut()
                    .filter(|g| g.by == player && g.spec.ability == volley)
                {
                    g.left = if moving { 0.0 } else { g.left.max(1.0) };
                }
            }
        }
        if self.heroes[h].moving
            && self.knows(h, Augment::FrozenWake)
            && self.heroes[h].spell.ready(Augment::FrozenWake)
        {
            if let Some(mut ice) = self.spec_of(h, "frost_trap") {
                self.heroes[h].spell.spend(Augment::FrozenWake, WAKE_EVERY);
                ice.radius = WAKE_REACH;
                ice.fx.trap = false;
                let behind = self.heroes[h].pos - self.heroes[h].facing.normalize_or_zero() * 2.0;
                self.add_ground(player, &ice, behind, WAKE_SECS);
            }
        }
    }

    /// What dying enemies set off for hunters: Marked for the Kill, Rapid Killing.
    pub(super) fn hunter_deaths(&mut self, deaths: &[Death]) {
        let rapid = self.content.ability("rapid_fire");
        for d in deaths {
            if let Some(by) = d.status.fx.marked_by {
                if let Some(h) = self.hero_ix(by) {
                    if self.knows(h, Augment::MarkedForTheKill) {
                        let next = self
                            .pick_targets(d.pos, MARK_REACH, 1, true, None)
                            .first()
                            .map(|&(id, _)| id);
                        if let Some(next) = next {
                            self.cast_on(h, "hunters_mark", next);
                        }
                    }
                }
            }
            let Some(h) = d.killer.and_then(|k| self.hero_ix(k)) else {
                continue;
            };
            if self.knows(h, Augment::RapidKilling) {
                if let Some(b) = self.heroes[h]
                    .spell
                    .fx
                    .buffs
                    .iter_mut()
                    .find(|b| Some(b.ability) == rapid)
                {
                    b.left += 0.5;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::classes::testkit::*;

    fn ready(w: &mut World, slot: usize) {
        w.heroes[0].abilities[slot].cd = 0.0;
    }

    #[test]
    fn disengage_leaps_and_leaves_ice_behind() {
        let mut w = hero_of("hunter");
        spells(&mut w, &[]);
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let start = w.heroes[0].pos;
        let id = foe(&mut w, -2.0, 0.0, 1e6);
        press_defense(&mut w);
        assert!(w.heroes[0].pos.distance(start + V2::new(12.0, 0.0)) < 0.1);
        run(&mut w, 0.2);
        assert!(
            w.enemy(id).unwrap().status.chill > 0.0,
            "no ice where you left"
        );
    }

    #[test]
    fn a_trap_waits_for_an_enemy_and_the_pet_bites() {
        let mut w = hero_of("hunter");
        spells(&mut w, &[("explosive_trap", 1, &[])]);
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        run(&mut w, 1.0);
        assert_eq!(w.enemy(id).unwrap().hp, 1e6, "the trap went off unsprung");
        assert_eq!(w.grounds.len(), 1);
        let at = w.grounds[0].pos;
        w.enemies[0].pos = at + V2::new(1.0, 0.0);
        w.step();
        assert!(w.enemy(id).unwrap().hp < 1e6 - 30.0, "no blast");

        let mut w = hero_of("hunter");
        spells(&mut w, &[("call_pet", 1, &[])]);
        let id = foe(&mut w, 20.0, 0.0, 1e6);
        w.enemies[0].elite = true;
        run(&mut w, 2.5);
        assert!(w.enemy(id).unwrap().hp < 1e6, "the pet did not bite");
    }

    #[test]
    fn hunters_mark_marks_and_rapid_fire_speeds_up() {
        let mut w = hero_of("hunter");
        spells(&mut w, &[("hunters_mark", 1, &[])]);
        let id = foe(&mut w, 30.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.mark >= 0.15);

        let mut w = hero_of("hunter");
        spells(&mut w, &[("rapid_fire", 1, &[])]);
        foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        assert!((w.heroes[0].spell.fx.cd_rate() - 0.4).abs() < 1e-4);
    }

    #[test]
    fn arcane_shot_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "arcane_shot",
                10,
                &[Augment::Precise, Augment::ArcaneBarrage],
            )],
        );
        w.heroes[0].abilities[0].casts = 3;
        let ids: Vec<u32> = (0..3)
            .map(|i| foe(&mut w, 10.0 + i as f32, 0.0, 1e6))
            .collect();
        run(&mut w, 0.5);
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("arcane_shot", 10, &[Augment::Precise, Augment::TrickShot])],
        );
        let ids: Vec<u32> = (0..3)
            .map(|i| foe(&mut w, 10.0 + 4.0 * i as f32, 0.0, 1e6))
            .collect();
        run(&mut w, 0.5);
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[
                ("arcane_shot", 10, &[Augment::Precise, Augment::LockAndLoad]),
                ("aimed_shot", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(
            w.heroes[0].abilities[1].cd < 90.0,
            "Aimed Shot was not readied"
        );
    }

    #[test]
    fn multi_shot_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "multi_shot",
                10,
                &[Augment::Precise, Augment::SplinterArrows],
            )],
        );
        foe(&mut w, 10.0, 0.0, 1e6);
        let beside = foe(&mut w, 11.0, 1.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(beside).unwrap().hp < 1e6);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("multi_shot", 10, &[Augment::Precise, Augment::SpreadShot])],
        );
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let ahead: Vec<u32> = (0..6)
            .map(|i| foe(&mut w, 5.0 + 2.0 * i as f32, 0.0, 1e6))
            .collect();
        let behind = foe(&mut w, -4.0, 0.0, 1e6);
        run(&mut w, 0.6);
        assert!(ahead.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));
        assert_eq!(w.enemy(behind).unwrap().hp, 1e6);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "multi_shot",
                10,
                &[Augment::Precise, Augment::SerpentSpread],
            )],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Poison));
    }

    #[test]
    fn aimed_shot_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("aimed_shot", 10, &[Augment::Precise, Augment::CarefulAim])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "aimed_shot").unwrap();
        assert_eq!(w.hunter_crit(0, ei, &s), 0.6);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "aimed_shot",
                10,
                &[Augment::Precise, Augment::SniperTraining],
            )],
        );
        let plain = w.spec_of(0, "aimed_shot").unwrap().damage;
        w.heroes[0].spell.still = 2.5;
        let owned = w.heroes[0].abilities[0].clone();
        assert!((w.spell_spec(0, &owned).damage / plain - 2.0).abs() < 1e-4);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "aimed_shot",
                10,
                &[Augment::Precise, Augment::PiercingShots],
            )],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));
    }

    #[test]
    fn serpent_sting_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "serpent_sting",
                10,
                &[Augment::Quickened, Augment::SpreadingVenom],
            )],
        );
        let ids: Vec<u32> = (0..4)
            .map(|i| foe(&mut w, 5.0 + 5.0 * i as f32, 0.0, 1e6))
            .collect();
        run(&mut w, 0.5);
        ready(&mut w, 0);
        run(&mut w, 1.0);
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().status.burning()));

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[
                (
                    "serpent_sting",
                    10,
                    &[Augment::Quickened, Augment::NoxiousStings],
                ),
                ("multi_shot", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.5);
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "multi_shot").unwrap();
        assert!((w.hunter_scale(0, ei, &s) - 1.1).abs() < 1e-4);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "serpent_sting",
                10,
                &[Augment::Quickened, Augment::ScorpidSting],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(id).unwrap().status.fx.weaken > 10.0);
    }

    #[test]
    fn concussive_shot_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "concussive_shot",
                10,
                &[Augment::Precise, Augment::ImprovedConcussion],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e7);
        let mut stunned = false;
        for _ in 0..60 {
            ready(&mut w, 0);
            run(&mut w, 0.3);
            stunned |= w.enemy(id).unwrap().status.stun > 0.0;
        }
        assert!(stunned);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "concussive_shot",
                10,
                &[Augment::Precise, Augment::DazeWave],
            )],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        let near = foe(&mut w, 10.0, 3.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(near).unwrap().status.chill > 0.0);

        let w = hero_of("hunter");
        let plain = w.spec_of(0, "concussive_shot").unwrap();
        let mut s = plain.clone();
        Augment::FrostShot.shape(&mut s);
        assert_eq!(s.chill, plain.chill * 2.0);
    }

    #[test]
    fn volley_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("volley", 10, &[Augment::Wide, Augment::Arrowstorm])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        run(&mut w, 9.0);
        assert_eq!(w.grounds.len(), 1, "it stopped while standing still");
        let hp = w.enemy(id).unwrap().hp;
        w.heroes[0].moving = true;
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(0.0, 1.0),
                ..Default::default()
            },
        );
        run(&mut w, 0.2);
        assert!(w.grounds.is_empty(), "moving did not end it");
        assert!(hp < 1e6);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("volley", 10, &[Augment::Wide, Augment::ExplosiveVolley])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.burning());

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("volley", 10, &[Augment::Wide, Augment::HailOfArrows])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.chill > 0.0);
    }

    #[test]
    fn explosive_trap_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("explosive_trap", 10, &[Augment::Wide, Augment::Minefield])],
        );
        foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.grounds.len(), 3);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "explosive_trap",
                10,
                &[Augment::Wide, Augment::ScorchedEarth],
            )],
        );
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let at = w.grounds[0].pos;
        w.enemies[0].pos = at;
        run(&mut w, 6.0);
        let e = w.enemy(id).unwrap();
        assert!(
            e.status.chill > 0.0 && !w.grounds.is_empty(),
            "no long, chilling fire"
        );

        let w = hero_of("hunter");
        let mut s = w.spec_of(0, "explosive_trap").unwrap();
        Augment::BigGameTrap.shape(&mut s);
        assert_eq!(s.fx.vs_elite, 1.0);
    }

    #[test]
    fn frost_trap_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("frost_trap", 10, &[Augment::Wide, Augment::FreezingTrap])],
        );
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        let at = w.grounds[0].pos;
        w.enemies[0].pos = at;
        w.step();
        assert!(w.enemy(id).unwrap().status.freeze > 3.0);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("frost_trap", 10, &[Augment::Wide, Augment::FrozenWake])],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(1.0, 0.0),
                ..Default::default()
            },
        );
        run(&mut w, 3.5);
        assert!(!w.grounds.is_empty(), "no ice behind");

        let w = hero_of("hunter");
        let mut s = w.spec_of(0, "frost_trap").unwrap();
        Augment::DeepFreeze.shape(&mut s);
        assert_eq!(s.slow, 0.7);
    }

    #[test]
    fn pet_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("call_pet", 10, &[Augment::Frenzy, Augment::BestialWrath])],
        );
        foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, DT * 2.0);
        assert!(w.heroes[0].spell.fx.boon(Augment::BestialWrath) > 0.0);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[
                ("call_pet", 10, &[Augment::Frenzy, Augment::KillCommand]),
                ("arcane_shot", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let before = w.enemy(id).unwrap().hp;
        let pet = ability(&w, "call_pet");
        ready(&mut w, 1);
        run(&mut w, 0.4);
        let pet_damage = w.heroes[0]
            .damage_by
            .iter()
            .find(|(a, _)| *a == pet)
            .map_or(0.0, |&(_, d)| d);
        assert!(pet_damage > 0.0 && w.enemy(id).unwrap().hp < before);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("call_pet", 10, &[Augment::Frenzy, Augment::Intimidation])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        let mut stunned = false;
        for _ in 0..(6.0 / DT) as usize {
            w.step();
            stunned |= w.enemy(id).unwrap().status.stun > 0.0;
        }
        assert!(stunned);
    }

    #[test]
    fn scatter_shot_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "scatter_shot",
                10,
                &[Augment::Precise, Augment::ImprovedScatter],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        run(&mut w, 0.4);
        let st = &w.enemy(id).unwrap().status;
        assert!(st.chill > st.stun + 3.0);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "scatter_shot",
                10,
                &[Augment::Precise, Augment::WyvernSting],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        run(&mut w, 0.4);
        let st = &w.enemy(id).unwrap().status;
        assert!(st.stun > 5.0 && st.burning());

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "scatter_shot",
                10,
                &[Augment::Precise, Augment::ScatterBurst],
            )],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        let near = foe(&mut w, 9.0, 2.0, 1e6);
        run(&mut w, 0.4);
        assert!(w.enemy(near).unwrap().status.stun > 0.0);
    }

    #[test]
    fn hunters_mark_capstones() {
        let w = hero_of("hunter");
        let mut s = w.spec_of(0, "hunters_mark").unwrap();
        Augment::ImprovedMark.shape(&mut s);
        assert_eq!(s.fx.mark, 0.25);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "hunters_mark",
                10,
                &[Augment::Precise, Augment::MarkedForTheKill],
            )],
        );
        foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let next = foe(&mut w, 20.0, 0.0, 1e6);
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        assert!(
            w.enemy(next).unwrap().status.fx.mark > 0.0,
            "the mark did not jump"
        );

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[(
                "hunters_mark",
                10,
                &[Augment::Precise, Augment::HuntersPrey],
            )],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "arcane_shot").unwrap();
        assert!((w.hunter_scale(0, ei, &s) - 1.2).abs() < 1e-4);
    }

    #[test]
    fn rapid_fire_capstones() {
        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[
                ("rapid_fire", 10, &[Augment::Hasted, Augment::RapidKilling]),
                ("arcane_shot", 1, &[]),
            ],
        );
        foe(&mut w, 10.0, 0.0, 1.0);
        run(&mut w, 0.5);
        assert!(w.heroes[0].spell.fx.buffs[0].left > 8.0 - 0.5);

        let mut w = hero_of("hunter");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(
            &mut w,
            &[("rapid_fire", 10, &[Augment::Hasted, Augment::TrueshotAura])],
        );
        w.step();
        assert!(
            w.heroes[1].spell.fx.power() >= 0.1,
            "the friend got no aura"
        );
        assert_eq!(w.heroes[0].spell.fx.cd_rate(), 0.0);

        let mut w = hero_of("hunter");
        spells(
            &mut w,
            &[("rapid_fire", 10, &[Augment::Hasted, Augment::QuickShots])],
        );
        let speed = w.heroes[0].speed;
        w.step();
        assert!(w.heroes[0].speed > speed * 1.19);
    }

    #[test]
    fn every_hunter_spell_has_three_capstones() {
        let w = hero_of("hunter");
        let class = &w.content.classes[w.content.class("hunter").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
