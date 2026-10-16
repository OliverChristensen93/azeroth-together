//! The druid's capstones (docs/DRUID-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Sunfire,
//! Plague of Locusts, Mass Entanglement, Thrash and the rest) are in `Augment::shape`.

use super::combat::{Death, Ground};
use super::*;
use crate::augments::Augment;
use crate::spell::{control_secs, BuffFx, Tag};

/// Moonkin Aura's reach for allies; Tornado's pull; Thorn Volley's and Blood in the Water's reach.
const MOONKIN_REACH: f32 = 20.0;
const TORNADO_REACH: f32 = 10.0;
const VOLLEY_REACH: f32 = 8.0;
const BLOOD_REACH: f32 = 5.0;
/// Efflorescence's flowers: reach and seconds.
const FLOWER_REACH: f32 = 5.0;
const FLOWER_SECS: f32 = 4.0;

impl World {
    /// Whether enemy `ei` carries `player`'s damage over time from spell `key`.
    fn carries(&self, ei: usize, player: PlayerId, key: &str) -> bool {
        let Some(ability) = self.content.ability(key) else {
            return false;
        };
        self.enemies[ei]
            .status
            .burns
            .iter()
            .any(|b| b.by == player && b.ability == ability)
    }

    /// The druid capstones that change a cast with the hero's state: Eclipse, Starsurge, Bear
    /// Form.
    pub(super) fn druid_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let hero = &self.heroes[h];
        if s.tags.has(Tag::Arcane) {
            s.damage *= 1.0 + hero.spell.fx.boon(Augment::Eclipse);
        }
        if owned.has(Augment::Starsurge) {
            let player = hero.player;
            let lit = (0..self.enemies.len())
                .filter(|&i| self.enemies[i].pos.distance(hero.pos) <= s.range)
                .filter(|&i| self.carries(i, player, "moonfire"))
                .count();
            s.damage *= 1.0 + (0.2 * lit as f32).min(1.0);
        }
        if self.knows(h, Augment::BearForm) {
            if Some(owned.ability) == self.content.ability("swipe") {
                s.damage *= 1.3;
            } else if s.tags.any(&[Tag::Arcane, Tag::Nature]) {
                s.damage *= 0.8;
            }
        }
    }

    /// How fast a druid's spell comes back: Rampant Growth when low.
    pub(super) fn druid_rate(&self, h: usize, owned: &Owned) -> f32 {
        let hero = &self.heroes[h];
        if owned.has(Augment::RampantGrowth) && hero.hp < hero.max_hp * 0.5 {
            2.0
        } else {
            1.0
        }
    }

    /// Before a druid spell fires on its own cooldown: Maul's every 3rd Swipe.
    pub(super) fn druid_before(&mut self, _h: usize, owned: &Owned, spec: &mut CastSpec) {
        if owned.has(Augment::Maul) && owned.casts % 3 == 2 {
            spec.stun = spec.stun.max(1.0);
            spec.tags = spec.tags.with(Tag::Stun);
        }
    }

    /// After a druid spell fired: Eclipse's every 6th Wrath.
    pub(super) fn druid_after(&mut self, h: usize, owned: &Owned, _spec: &CastSpec) {
        if owned.has(Augment::Eclipse) && owned.casts % 6 == 5 {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::Eclipse, 0.4, 0.4, 8.0);
        }
    }

    /// Extra crit chance: Moonkin Aura, Moonstrike, Improved Faerie Fire.
    pub(super) fn druid_crit(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let player = self.heroes[h].player;
        let mut c = self.heroes[h].spell.fx.boon(Augment::MoonkinAura);
        if Some(spec.ability) == self.content.ability("starfire")
            && self.knows(h, Augment::Moonstrike)
            && self.carries(ei, player, "moonfire")
        {
            c += 1.0;
        }
        if self.enemies[ei].status.fx.mark_secs > 0.0 && self.knows(h, Augment::ImprovedFaerieFire)
        {
            c += 0.1;
        }
        c
    }

    /// How much harder a druid's hit lands: Nature's Torment on swarmed enemies.
    pub(super) fn druid_scale(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let player = self.heroes[h].player;
        if spec.tags.has(Tag::Nature)
            && self.knows(h, Augment::NaturesTorment)
            && self.carries(ei, player, "insect_swarm")
        {
            1.15
        } else {
            1.0
        }
    }

    /// What is left of a blow at a druid: Bear Form, and a hurricane's eye or a Tranquility
    /// hymn around it.
    pub(super) fn druid_guard(&self, h: usize) -> f32 {
        let pos = self.heroes[h].pos;
        let mut m = if self.knows(h, Augment::BearForm) {
            0.85
        } else {
            1.0
        };
        for g in &self.grounds {
            if g.pos.distance(pos) > g.spec.radius {
                continue;
            }
            let eye = g.by == self.heroes[h].player
                && self.spell_has(g.by, g.spec.ability, Augment::EyeOfTheStorm);
            if eye || self.spell_has(g.by, g.spec.ability, Augment::HymnOfLife) {
                m *= 0.8;
            }
        }
        m
    }

    /// How much more a druid's healing ground of `spec` heals hero `t`: Restoration.
    pub(super) fn druid_heal_bonus(&self, h: usize, spec: &CastSpec, t: usize) -> f32 {
        let player = self.heroes[h].player;
        let low = self.heroes[t].hp < self.heroes[t].max_hp * 0.5;
        if low && self.spell_has(player, spec.ability, Augment::Restoration) {
            1.5
        } else {
            1.0
        }
    }

    /// Thorns from `by`'s spell `ability` struck enemy `enemy`: Briar Patch roots it.
    pub(super) fn druid_thorns(&mut self, by: PlayerId, ability: usize, enemy: u32) {
        if !self.spell_has(by, ability, Augment::BriarPatch) {
            return;
        }
        if let Some(e) = self.enemies.iter_mut().find(|e| e.id == enemy) {
            e.status.fx.root = e.status.fx.root.max(control_secs(1.0, e.elite, e.boss));
        }
    }

    /// A blow of `amount` from enemy `from` reached a druid: Nature's Grasp, Thorn Volley.
    pub(super) fn druid_blow(&mut self, h: usize, _amount: f32, from: Option<u32>) {
        if self.knows(h, Augment::NaturesGrasp) {
            if let Some(e) = from.and_then(|id| self.enemies.iter_mut().find(|e| e.id == id)) {
                if e.status.fx.grasp_cd <= 0.0 {
                    e.status.fx.grasp_cd = 10.0;
                    e.status.fx.root = e.status.fx.root.max(control_secs(2.0, e.elite, e.boss));
                }
            }
        }
        if self.knows(h, Augment::ThornVolley)
            && self.heroes[h].spell.fx.bump(Augment::ThornVolley) % 5 == 0
        {
            let Some(thorns) = self.content.ability("thorns") else {
                return;
            };
            let damage = self.spell_spec(h, &self.owned_or_new(h, thorns)).damage * 2.0;
            let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
            let near: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= VOLLEY_REACH)
                .map(|e| e.id)
                .collect();
            for id in near {
                self.damage_enemy(player, thorns, id, damage, false);
            }
        }
    }

    /// A druid's full Ward struck instead: Gift of the Wild heals the heroes around.
    pub(super) fn druid_overflow(&mut self, h: usize, spec: &CastSpec) {
        let player = self.heroes[h].player;
        if !self.spell_has(player, spec.ability, Augment::GiftOfTheWild) {
            return;
        }
        let pos = self.heroes[h].pos;
        for hero in &mut self.heroes {
            if hero.alive && hero.pos.distance(pos) <= 20.0 {
                hero.hp = (hero.hp + hero.max_hp * 0.05).min(hero.max_hp);
            }
        }
    }

    /// Hero `h` spent a heal charge of `ability` on hero `t`: Efflorescence's flowers.
    pub(super) fn druid_charge_spent(&mut self, h: usize, ability: usize, t: usize) {
        let player = self.heroes[h].player;
        if !self.spell_has(player, ability, Augment::Efflorescence) {
            return;
        }
        let mut flowers = self.spell_spec(h, &self.owned_or_new(h, ability));
        flowers.fx.ward = None;
        flowers.fx.heal = 0.02;
        flowers.fx.root = 1.0;
        flowers.damage = 0.01;
        flowers.radius = FLOWER_REACH;
        let at = self.heroes[t].pos;
        self.add_ground(player, &flowers, at, FLOWER_SECS);
    }

    /// A hit of a druid's spell `spec` on enemy `id`: the capstones that act on a hit or a crit.
    pub(super) fn druid_strike(
        &mut self,
        h: usize,
        id: u32,
        spec: &CastSpec,
        _damage: f32,
        crit: bool,
        _scale: f32,
    ) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        let Some(ei) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        if Some(spec.ability) == self.content.ability("faerie_fire") {
            self.enemies[ei].status.fx.faerie_by = Some(player);
        }
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        let mine = |b: &&mut super::combat::Burn| b.by == player && b.ability == spec.ability;
        for a in augments {
            match a {
                Augment::NaturesGrace if crit => {
                    let fx = BuffFx {
                        secs: 3.0,
                        cd_rate: 0.3,
                        ..BuffFx::default()
                    };
                    self.grant(h, player, spec.ability, fx);
                }
                Augment::MoonkinAura => {
                    for hero in &mut self.heroes {
                        if hero.alive && hero.pos.distance(pos) <= MOONKIN_REACH {
                            hero.spell.fx.add_boon(Augment::MoonkinAura, 0.02, 0.1, 8.0);
                        }
                    }
                }
                Augment::Rip => {
                    let e = &mut self.enemies[ei];
                    if e.elite || e.boss {
                        if let Some(b) = e.status.burns.iter_mut().find(mine) {
                            b.dps *= 1.5;
                        }
                    }
                }
                Augment::FerociousBite => {
                    let e = &self.enemies[ei];
                    if !e.boss && e.hp > 0.0 && e.hp < e.max_hp * 0.25 {
                        let rest = e.hp;
                        self.damage_enemy(player, spec.ability, id, rest, false);
                    }
                }
                _ => {}
            }
        }
        if self.enemies[ei].status.fx.mark_secs > 0.0 && self.knows(h, Augment::FaeTransfusion) {
            let before = self.heroes[h].spell.fx.boon(Augment::FaeTransfusion);
            if before < 0.05 {
                let hero = &mut self.heroes[h];
                hero.hp = (hero.hp + hero.max_hp * 0.005).min(hero.max_hp);
                hero.spell
                    .fx
                    .add_boon(Augment::FaeTransfusion, 0.005, 0.05, 1.0);
            }
        }
    }

    /// A tick of a druid's damage over time: Lunar Shower readies Starfire.
    pub(super) fn druid_tick(&mut self, h: usize, ability: usize) {
        let player = self.heroes[h].player;
        if Some(ability) == self.content.ability("moonfire")
            && self.spell_has(player, ability, Augment::LunarShower)
            && self.rng.f32() < 0.1
        {
            let starfire = self.content.ability("starfire");
            if let Some(o) = self.heroes[h]
                .abilities
                .iter_mut()
                .find(|o| Some(o.ability) == starfire)
            {
                o.cd = 0.0;
            }
        }
    }

    /// Each step for a druid: Cyclone lifts and Tornado pulls around its hurricanes.
    pub(super) fn druid_tend(&mut self, h: usize) {
        let player = self.heroes[h].player;
        let Some(hurricane) = self.content.ability("hurricane") else {
            return;
        };
        let storms: Vec<(V2, f32, bool, bool)> = self
            .grounds
            .iter()
            .filter(|g: &&Ground| g.by == player && g.spec.ability == hurricane)
            .map(|g| {
                (
                    g.pos,
                    g.spec.radius,
                    self.spell_has(player, hurricane, Augment::Cyclone),
                    self.spell_has(player, hurricane, Augment::Tornado),
                )
            })
            .collect();
        for (at, radius, cyclone, tornado) in storms {
            if cyclone {
                let mut near: Vec<(usize, f32)> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| !e.boss && e.hp > 0.0 && e.pos.distance(at) <= radius)
                    .map(|(i, e)| (i, e.pos.distance(at)))
                    .collect();
                near.sort_by(|a, b| a.1.total_cmp(&b.1));
                for (i, _) in near.into_iter().take(3) {
                    let st = &mut self.enemies[i].status;
                    st.stun = st.stun.max(0.2);
                }
            }
            if tornado {
                for e in self.enemies.iter_mut().filter(|e| !e.boss) {
                    let off = at - e.pos;
                    let d = off.length();
                    if d <= TORNADO_REACH && d > 1.0 {
                        e.pos += off.normalize_or_zero() * DT;
                    }
                }
            }
        }
    }

    /// What dying enemies set off for druids: Swarming, Blood in the Water, Faerie Spread.
    pub(super) fn druid_deaths(&mut self, deaths: &[Death]) {
        let (swarm, rake) = (
            self.content.ability("insect_swarm"),
            self.content.ability("rake"),
        );
        for d in deaths {
            for b in &d.status.burns {
                if Some(b.ability) == swarm && self.spell_has(b.by, b.ability, Augment::Swarming) {
                    let mut fresh = b.clone();
                    fresh.left = fresh.left.max(fresh.every * 4.0);
                    self.spread_dot(&fresh, d.pos, 2);
                }
            }
            if let (Some(by), Some(rake)) = (d.killer, rake) {
                let bled = d
                    .status
                    .burns
                    .iter()
                    .any(|b| b.by == by && b.tags.has(Tag::Bleed));
                if bled && self.spell_has(by, rake, Augment::BloodInTheWater) {
                    if let Some(h) = self.hero_ix(by) {
                        let near: Vec<u32> = self
                            .enemies
                            .iter()
                            .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= BLOOD_REACH)
                            .map(|e| e.id)
                            .collect();
                        for id in near {
                            self.cast_on(h, "rake", id);
                        }
                    }
                }
            }
            if let Some(by) = d.status.fx.faerie_by {
                if let Some(h) = self.hero_ix(by) {
                    if self.knows(h, Augment::FaerieSpread) {
                        let next = self
                            .enemies
                            .iter()
                            .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= 30.0)
                            .min_by(|a, c| a.pos.distance(d.pos).total_cmp(&c.pos.distance(d.pos)))
                            .map(|e| e.id);
                        if let Some(next) = next {
                            self.cast_on(h, "faerie_fire", next);
                        }
                    }
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
    fn barkskin_softens_blows_and_roots_hold() {
        let mut w = hero_of("druid");
        spells(&mut w, &[]);
        press_defense(&mut w);
        let took = w.hurt_hero_from(0, 100.0, None);
        assert!((took - 60.0).abs() < 1e-3, "{took}");

        let mut w = hero_of("druid");
        spells(&mut w, &[("entangling_roots", 1, &[])]);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.root > 2.0);
    }

    #[test]
    fn rejuvenation_tranquility_and_thorns() {
        let mut w = hero_of("druid");
        spells(&mut w, &[("rejuvenation", 1, &[])]);
        w.step();
        let max = w.heroes[0].max_hp;
        w.heroes[0].hp = max * 0.3;
        w.step();
        assert!(w.heroes[0].hp > max * 0.4);

        let mut w = hero_of("druid");
        spells(&mut w, &[("tranquility", 1, &[])]);
        foe(&mut w, 5.0, 0.0, 1e6);
        w.heroes[0].hp = 100.0;
        run(&mut w, 1.2);
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("druid");
        spells(&mut w, &[("thorns", 1, &[])]);
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(id).unwrap().hp < 1e6);
    }

    #[test]
    fn wrath_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("wrath", 10, &[Augment::Precise, Augment::NaturesGrace])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 8.0, 0.0, 1e6);
        run(&mut w, 0.6);
        assert!(w.heroes[0].spell.fx.cd_rate() >= 0.3);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[
                ("wrath", 10, &[Augment::Precise, Augment::Eclipse]),
                ("starfire", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].abilities[0].casts = 5;
        let plain = w.spec_of(0, "starfire").unwrap().damage;
        foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        assert!((w.spec_of(0, "starfire").unwrap().damage / plain - 1.4).abs() < 1e-4);

        let w = hero_of("druid");
        let mut s = w.spec_of(0, "wrath").unwrap();
        Augment::WrathOfCenarius.shape(&mut s);
        assert_eq!(s.fx.vs_rooted, 0.5);
    }

    #[test]
    fn moonfire_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[
                ("moonfire", 10, &[Augment::Precise, Augment::LunarShower]),
                ("starfire", 1, &[]),
            ],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        let starfire = ability(&w, "starfire");
        let mut readied = false;
        for _ in 0..40 {
            ready(&mut w, 0);
            w.heroes[0].abilities[1].cd = 50.0;
            readied |= casts_of(&run(&mut w, 1.5), starfire) > 0;
        }
        assert!(readied);

        let mut w = hero_of("druid");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(
            &mut w,
            &[("moonfire", 10, &[Augment::Precise, Augment::MoonkinAura])],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[1].spell.fx.boon(Augment::MoonkinAura) > 0.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("moonfire", 10, &[Augment::Precise, Augment::Sunfire])],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        let near = foe(&mut w, 30.0, 0.0, 1e6);
        let at = w.enemies[0].pos;
        w.enemies[1].pos = at + V2::new(0.0, 4.0);
        w.heroes[0].abilities[0].mods.projectiles = 0.0;
        w.step();
        assert!(w.enemy(near).unwrap().hp < 1e6);
    }

    #[test]
    fn starfire_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[
                ("moonfire", 1, &[]),
                ("starfire", 10, &[Augment::Precise, Augment::Moonstrike]),
            ],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "starfire").unwrap();
        assert!(w.druid_crit(0, ei, &s) >= 1.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[
                ("moonfire", 1, &[]),
                ("starfire", 10, &[Augment::Precise, Augment::Starsurge]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let plain = w.spec_of(0, "starfire").unwrap().damage;
        foe(&mut w, 8.0, 0.0, 1e6);
        foe(&mut w, 9.0, 0.0, 1e6);
        w.step();
        assert!((w.spec_of(0, "starfire").unwrap().damage / plain - 1.4).abs() < 1e-4);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "starfire",
                10,
                &[Augment::Precise, Augment::ImprovedStarfire],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.stun > 0.0);
    }

    #[test]
    fn insect_swarm_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("insect_swarm", 10, &[Augment::Quickened, Augment::Swarming])],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let near = foe(&mut w, 10.0, 0.0, 1e6);
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        assert!(w.enemy(near).unwrap().status.burning());

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "insect_swarm",
                10,
                &[Augment::Quickened, Augment::PlagueOfLocusts],
            )],
        );
        let ids: Vec<u32> = (0..5)
            .map(|i| foe(&mut w, 3.0, i as f32 - 2.0, 1e6))
            .collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().status.burning()));

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "insect_swarm",
                10,
                &[Augment::Quickened, Augment::NaturesTorment],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "wrath").unwrap();
        assert!((w.druid_scale(0, ei, &s) - 1.15).abs() < 1e-4);
    }

    #[test]
    fn hurricane_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("hurricane", 10, &[Augment::Wide, Augment::Cyclone])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, DT * 2.0);
        assert!(w.enemy(id).unwrap().status.stun > 0.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("hurricane", 10, &[Augment::Wide, Augment::EyeOfTheStorm])],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, DT * 2.0);
        assert!((w.druid_guard(0) - 0.8).abs() < 1e-4);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("hurricane", 10, &[Augment::Lingering, Augment::Tornado])],
        );
        for i in 0..3 {
            foe(&mut w, 15.0, i as f32 * 0.5, 1e6);
        }
        let edge = foe(&mut w, 15.0, 9.0, 1e6);
        w.step();
        let centre = w.grounds[0].pos;
        let d0 = w.enemy(edge).unwrap().pos.distance(centre);
        run(&mut w, 1.0);
        let d1 = w.enemy(edge).unwrap().pos.distance(w.grounds[0].pos);
        assert!(d1 < d0 - 0.5, "no pull: {d0} -> {d1}");
    }

    #[test]
    fn roots_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "entangling_roots",
                10,
                &[Augment::Precise, Augment::NaturesGrasp],
            )],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(id).unwrap().status.fx.root > 0.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "entangling_roots",
                10,
                &[Augment::Precise, Augment::MassEntanglement],
            )],
        );
        let ids: Vec<u32> = (0..6)
            .map(|i| foe(&mut w, 3.0, i as f32 - 3.0, 1e6))
            .collect();
        w.step();
        assert!(ids
            .iter()
            .all(|&id| w.enemy(id).unwrap().status.fx.root > 0.0));

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "entangling_roots",
                10,
                &[Augment::Precise, Augment::ThornedVines],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.burning());
    }

    #[test]
    fn rejuvenation_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "rejuvenation",
                10,
                &[Augment::Hasted, Augment::Efflorescence],
            )],
        );
        w.step();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.3;
        w.step();
        assert_eq!(w.grounds.len(), 1, "no flowers");
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        run(&mut w, 1.2);
        assert!(w.enemy(id).unwrap().status.fx.root > 0.0);

        let mut w = hero_of("druid");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(
            &mut w,
            &[(
                "rejuvenation",
                10,
                &[Augment::Hasted, Augment::GiftOfTheWild],
            )],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        for _ in 0..6 {
            ready(&mut w, 0);
            w.step();
        }
        w.heroes[1].hp = 50.0;
        ready(&mut w, 0);
        w.step();
        assert!(w.heroes[1].hp > 50.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "rejuvenation",
                10,
                &[Augment::Hasted, Augment::RampantGrowth],
            )],
        );
        w.heroes[0].hp = w.heroes[0].max_hp * 0.4;
        let owned = w.heroes[0].abilities[0].clone();
        assert_eq!(w.druid_rate(0, &owned), 2.0);
    }

    #[test]
    fn tranquility_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("tranquility", 10, &[Augment::Hasted, Augment::TranquilMind])],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.fx.fear > 0.0);

        let heal = |restore: bool| {
            let mut w = hero_of("druid");
            let augments: &[Augment] = if restore {
                &[Augment::Hasted, Augment::Restoration]
            } else {
                &[Augment::Hasted]
            };
            spells(&mut w, &[("tranquility", 10, augments)]);
            w.heroes[0].max_hp = 1000.0;
            w.heroes[0].hp = 100.0;
            foe(&mut w, 5.0, 0.0, 1e6);
            run(&mut w, 0.1);
            w.heroes[0].hp - 100.0
        };
        assert!((heal(true) / heal(false) - 1.5).abs() < 1e-3);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("tranquility", 10, &[Augment::Hasted, Augment::HymnOfLife])],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.1);
        assert!((w.druid_guard(0) - 0.8).abs() < 1e-4);
    }

    #[test]
    fn thorns_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("thorns", 10, &[Augment::Heavy, Augment::BriarPatch])],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(id).unwrap().status.fx.root > 0.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("thorns", 10, &[Augment::Heavy, Augment::ThornVolley])],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        let near = foe(&mut w, -5.0, 0.0, 1e6);
        w.step();
        for _ in 0..5 {
            w.hurt_hero_from(0, 1.0, Some(id));
        }
        assert!(w.enemy(near).unwrap().hp < 1e6, "no volley");

        let mut w = hero_of("druid");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(
            &mut w,
            &[("thorns", 10, &[Augment::Heavy, Augment::NaturesWard])],
        );
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(1, 1.0, Some(id));
        assert!(w.enemy(id).unwrap().hp < 1e6, "the friend has no thorns");
    }

    #[test]
    fn swipe_capstones() {
        let mut w = hero_of("druid");
        spells(&mut w, &[("swipe", 10, &[Augment::Wide, Augment::Maul])]);
        w.heroes[0].abilities[0].casts = 2;
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.stun > 0.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[
                ("swipe", 10, &[Augment::Wide, Augment::BearForm]),
                ("wrath", 1, &[]),
            ],
        );
        let mut plain = hero_of("druid");
        spells(
            &mut plain,
            &[("swipe", 10, &[Augment::Wide]), ("wrath", 1, &[])],
        );
        let ratio =
            |key: &str| w.spec_of(0, key).unwrap().damage / plain.spec_of(0, key).unwrap().damage;
        assert!((ratio("swipe") - 1.3).abs() < 1e-4);
        assert!((ratio("wrath") - 0.8).abs() < 1e-4);
        assert!((w.druid_guard(0) - 0.85).abs() < 1e-4);

        let mut w = hero_of("druid");
        spells(&mut w, &[("swipe", 10, &[Augment::Wide, Augment::Thrash])]);
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));
    }

    #[test]
    fn rake_capstones() {
        let dps = |elite: bool| {
            let mut w = hero_of("druid");
            spells(&mut w, &[("rake", 10, &[Augment::Lacerate, Augment::Rip])]);
            let id = foe(&mut w, 2.0, 0.0, 1e6);
            w.enemies[0].elite = elite;
            w.step();
            w.enemy(id).unwrap().status.burns[0].dps
        };
        assert!((dps(true) / dps(false) - 1.5).abs() < 1e-3);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("rake", 10, &[Augment::Lacerate, Augment::FerociousBite])],
        );
        let id = foe(&mut w, 2.0, 0.0, 1000.0);
        w.enemies[0].hp = 200.0;
        w.step();
        assert!(w.enemy(id).is_none(), "the bite did not finish it");

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[("rake", 10, &[Augment::Lacerate, Augment::BloodInTheWater])],
        );
        foe(&mut w, 2.0, 0.0, 5.0);
        let near = foe(&mut w, 30.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let at = w.enemies[0].pos;
        w.enemies[1].pos = at + V2::new(3.0, 0.0);
        run(&mut w, 6.0);
        assert!(w.enemy(near).unwrap().status.has_dot(0, Tag::Bleed));
    }

    #[test]
    fn faerie_fire_capstones() {
        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "faerie_fire",
                10,
                &[Augment::Precise, Augment::ImprovedFaerieFire],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "wrath").unwrap();
        assert!((w.druid_crit(0, ei, &s) - 0.1).abs() < 1e-4);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[
                (
                    "faerie_fire",
                    10,
                    &[Augment::Precise, Augment::FaeTransfusion],
                ),
                ("wrath", 1, &[]),
            ],
        );
        w.heroes[0].hp = 100.0;
        foe(&mut w, 8.0, 0.0, 1e6);
        run(&mut w, 1.0);
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("druid");
        spells(
            &mut w,
            &[(
                "faerie_fire",
                10,
                &[Augment::Precise, Augment::FaerieSpread],
            )],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let next = foe(&mut w, 12.0, 0.0, 1e6);
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        assert!(w.enemy(next).unwrap().status.fx.mark > 0.0);
    }

    #[test]
    fn every_druid_spell_has_three_capstones() {
        let w = hero_of("druid");
        let class = &w.content.classes[w.content.class("druid").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
