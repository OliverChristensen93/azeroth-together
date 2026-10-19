//! The rogue's capstones (docs/ROGUE-DRAFT.md): the mechanics of its 36 rank-10 augments, called
//! from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Opportunity,
//! Serrated Blades, Lethal Fan's reach, Dirty Tricks and the rest) are in `Augment::shape`.

use super::combat::{Death, Shot, Trigger};
use super::*;
use crate::augments::Augment;
use crate::spell::{BuffFx, Tag, MAX_COMBO};

/// Cold Blood's and Kidney Burst's reach around the target at 5 combo points.
const COLD_BLOOD_REACH: f32 = 6.0;
const KIDNEY_BURST_REACH: f32 = 5.0;
/// Crimson Tempest's reach around its target.
const TEMPEST_REACH: f32 = 8.0;
/// Toxic Cloud's and Smoke Bomb's clouds: reach and seconds.
const TOXIC_REACH: f32 = 5.0;
const TOXIC_SECS: f32 = 4.0;
const SMOKE_REACH: f32 = 6.0;
const SMOKE_SECS: f32 = 5.0;

impl World {
    /// The rogue capstones that change a cast with the hero's state: Ambush after the defensive
    /// button.
    pub(super) fn rogue_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let fx = &self.heroes[h].spell.fx;
        if owned.has(Augment::Ambush) && fx.since_defense < 4.0 && fx.defense_cd > 0.0 {
            s.damage *= 3.0;
            s.crit = 1.0;
        }
    }

    /// How fast a rogue's spell comes back: Flowing Steel runs on movement.
    pub(super) fn rogue_rate(&self, h: usize, owned: &Owned) -> f32 {
        if owned.has(Augment::FlowingSteel) {
            if self.heroes[h].moving {
                2.0
            } else {
                0.5
            }
        } else {
            1.0
        }
    }

    /// Before a rogue spell fires on its own cooldown: at 5 combo points Cold Blood and Kidney
    /// Burst reach around the target; Marked for Death lasts the stun.
    pub(super) fn rogue_before(&mut self, h: usize, owned: &Owned, spec: &mut CastSpec) {
        let full = self.heroes[h].spell.fx.combo >= MAX_COMBO;
        if full && owned.has(Augment::ColdBlood) {
            spec.radius = spec.radius.max(COLD_BLOOD_REACH);
        }
        if full && owned.has(Augment::KidneyBurst) {
            spec.radius = spec.radius.max(KIDNEY_BURST_REACH);
        }
        if owned.has(Augment::MarkedForDeath) {
            spec.fx.mark_secs = spec.fx.mark_secs.max(spec.stun + 4.0);
        }
    }

    /// A Finisher of hero `h` spent `spent` combo points: Restless Blades, Ruthlessness, Quick
    /// Recovery.
    pub(super) fn rogue_spent(&mut self, h: usize, owned: &Owned, _spec: &CastSpec, spent: u8) {
        if owned.has(Augment::RestlessBlades) {
            let fx = &mut self.heroes[h].spell.fx;
            fx.defense_cd = (fx.defense_cd - 2.0 * f32::from(spent)).max(0.0);
        }
        if owned.has(Augment::Ruthlessness) {
            self.give_combo(h, 1);
        }
        if owned.has(Augment::QuickRecovery) {
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + hero.max_hp * 0.02 * f32::from(spent)).min(hero.max_hp);
        }
    }

    /// How much harder a rogue's hit lands: Gouged Eyes on stunned enemies, Sap on a blinded one.
    pub(super) fn rogue_scale(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let e = &self.enemies[ei];
        let player = self.heroes[h].player;
        let mut m = 1.0;
        if spec.tags.has(Tag::Finisher) && e.status.stun > 0.0 && self.knows(h, Augment::GougedEyes)
        {
            m *= 1.3;
        }
        let blind = self.content.ability("blind");
        if e.status.fx.sapped_by == Some(player) && Some(spec.ability) != blind {
            m *= 2.0;
        }
        m
    }

    /// A hit of a rogue's spell `spec` on enemy `id`: the capstones that act on a hit or a crit.
    pub(super) fn rogue_strike(
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
        let blind = self.content.ability("blind");
        if Some(spec.ability) != blind && self.enemies[ei].status.fx.sapped_by == Some(player) {
            self.enemies[ei].status.fx.sapped_by = None;
        }
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        let at = self.enemies[ei].pos;
        for a in augments {
            match a {
                Augment::CombatPotency if self.rng.f32() < 0.25 => self.give_combo(h, 1),
                // A bounce of Blade Dance is a jump of the chain: its spec has no jumps left.
                Augment::BladeDance if spec.fx.jumps == 0 => self.give_combo(h, 1),
                Augment::RelentlessStrikes => {
                    let content = self.content.clone();
                    for o in &mut self.heroes[h].abilities {
                        if content.abilities[o.ability].tag_set.has(Tag::Finisher) {
                            o.cd = (o.cd - 0.3).max(0.0);
                        }
                    }
                }
                Augment::SealFate if crit => self.give_combo(h, 1),
                Augment::VenomousWounds => {
                    let st = &mut self.enemies[ei].status;
                    if st.has_dot(player, Tag::Poison) {
                        if let Some(b) = st
                            .burns
                            .iter_mut()
                            .find(|b| b.by == player && b.ability == spec.ability)
                        {
                            b.dps *= 1.5;
                        }
                    }
                }
                Augment::CrimsonTempest if scale > 0.99 => {
                    let bleed = self.enemies[ei]
                        .status
                        .burns
                        .iter()
                        .find(|b| b.by == player && b.ability == spec.ability)
                        .cloned();
                    if let Some(bleed) = bleed {
                        for e in self.enemies.iter_mut().filter(|e| {
                            e.id != id && e.hp > 0.0 && e.pos.distance(at) <= TEMPEST_REACH
                        }) {
                            match e
                                .status
                                .burns
                                .iter_mut()
                                .find(|b| b.by == player && b.ability == spec.ability)
                            {
                                Some(old) => *old = bleed.clone(),
                                None => e.status.burns.push(bleed.clone()),
                            }
                        }
                    }
                }
                Augment::PoisonedKnives => self.cast_on(h, "deadly_poison", id),
                Augment::MindNumbingPoison => {
                    let st = &mut self.enemies[ei].status;
                    let stacks = st
                        .burns
                        .iter()
                        .filter(|b| b.by == player && b.tags.has(Tag::Poison))
                        .count();
                    if stacks >= 3 {
                        st.fx.weaken = st.fx.weaken.max(2.0);
                    }
                }
                _ => {}
            }
        }
        if spec.tags.has(Tag::Finisher) && self.knows(h, Augment::Envenom) {
            let st = &mut self.enemies[ei].status;
            let left: f32 = st
                .burns
                .iter()
                .filter(|b| b.by == player && b.tags.has(Tag::Poison))
                .map(|b| b.dps * b.left)
                .sum();
            st.burns
                .retain(|b| !(b.by == player && b.tags.has(Tag::Poison)));
            if left > 0.0 {
                let poison = self
                    .content
                    .ability("deadly_poison")
                    .unwrap_or(spec.ability);
                self.damage_enemy(player, poison, id, left * 2.0, false);
            }
        }
        if Some(spec.ability) == blind && self.knows(h, Augment::Sap) {
            if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                e.status.fx.sapped_by = Some(player);
            }
        }
    }

    /// A blow dodged by a Riposte charge of `spec`: Counterattack's Blade Flurry, Nimble's
    /// speed.
    pub(super) fn rogue_blocked(&mut self, h: usize, spec: &CastSpec, _from: Option<u32>) {
        let player = self.heroes[h].player;
        if self.spell_has(player, spec.ability, Augment::Counterattack) {
            if let Some(flurry) = self.content.ability("blade_flurry") {
                self.triggers.push(Trigger {
                    player,
                    ability: flurry,
                    shot: Shot {
                        primary: false,
                        ..Shot::plain()
                    },
                });
                self.run_triggers();
            }
        }
        if self.spell_has(player, spec.ability, Augment::Nimble) {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::Nimble, 0.1, 0.3, 3.0);
            self.refresh(h);
        }
    }

    /// A killing blow at a rogue: Cheat Death leaves it at 10% and refills its dodges, once a
    /// minute. Whether it did.
    pub(super) fn rogue_cheat(&mut self, h: usize) -> bool {
        if !self.knows(h, Augment::CheatDeath) || !self.heroes[h].spell.ready(Augment::CheatDeath) {
            return false;
        }
        self.heroes[h].spell.spend(Augment::CheatDeath, 60.0);
        let hero = &mut self.heroes[h];
        hero.hp = hero.max_hp * 0.1;
        if let Some(riposte) = self.content.ability("riposte") {
            let cap = self
                .spell_spec(h, &self.owned_or_new(h, riposte))
                .charges_held();
            for _ in 0..cap {
                self.add_charge(h, riposte);
            }
        }
        true
    }

    /// After a rogue spell fired: Smoke Bomb's cloud.
    pub(super) fn rogue_after(&mut self, h: usize, owned: &Owned, spec: &CastSpec) {
        if owned.has(Augment::SmokeBomb) {
            let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
            let mut smoke = spec.clone();
            smoke.damage = 0.0;
            smoke.radius = SMOKE_REACH;
            smoke.fx.fear = 0.0;
            smoke.fx.lost = 1.2;
            smoke.fx.mark = smoke.fx.mark.max(0.3);
            smoke.fx.mark_secs = smoke.fx.mark_secs.max(1.2);
            smoke.follows = false;
            self.add_ground(player, &smoke, pos, SMOKE_SECS);
        }
    }

    /// Each step for a rogue: nothing of its own yet.
    pub(super) fn rogue_tend(&mut self, _h: usize) {}

    /// What dying enemies set off for rogues: Bloodletter's combo points, Toxic Cloud, Restless
    /// Tempo.
    pub(super) fn rogue_deaths(&mut self, deaths: &[Death]) {
        let (rupture, poison, slice) = (
            self.content.ability("rupture"),
            self.content.ability("deadly_poison"),
            self.content.ability("slice_and_dice"),
        );
        for d in deaths {
            for h in 0..self.heroes.len() {
                let player = self.heroes[h].player;
                let mine = |b: &&super::combat::Burn| b.by == player;
                if let Some(rupture) = rupture {
                    let bled = d
                        .status
                        .burns
                        .iter()
                        .filter(mine)
                        .any(|b| b.ability == rupture);
                    if bled && self.spell_has(player, rupture, Augment::Bloodletter) {
                        self.give_combo(h, 2);
                    }
                }
                let poisoned = d
                    .status
                    .burns
                    .iter()
                    .filter(mine)
                    .any(|b| b.tags.has(Tag::Poison));
                if poisoned && self.knows(h, Augment::ToxicCloud) {
                    if let Some(poison) = poison {
                        let mut cloud = self.spell_spec(h, &self.owned_or_new(h, poison));
                        cloud.radius = TOXIC_REACH;
                        self.add_ground(player, &cloud, d.pos, TOXIC_SECS);
                    }
                }
            }
            let Some(h) = d.killer.and_then(|k| self.hero_ix(k)) else {
                continue;
            };
            if self.knows(h, Augment::RestlessTempo) {
                if let Some(b) = self.heroes[h]
                    .spell
                    .fx
                    .buffs
                    .iter_mut()
                    .find(|b| Some(b.ability) == slice)
                {
                    b.left += 1.0;
                }
            }
        }
    }

    /// A rogue's nova hit `hit`: Adrenaline Rush on a big crowd, Fan the Pack's combo points.
    pub(super) fn rogue_nova(&mut self, h: usize, spec: &CastSpec, _center: V2, hit: &[u32]) {
        let player = self.heroes[h].player;
        if hit.len() >= 8
            && self.spell_has(player, spec.ability, Augment::AdrenalineRush)
            && self.heroes[h].spell.ready(Augment::AdrenalineRush)
        {
            self.heroes[h].spell.spend(Augment::AdrenalineRush, 12.0);
            let fx = BuffFx {
                secs: 4.0,
                cd_rate: 0.5,
                ..BuffFx::default()
            };
            self.grant(h, player, spec.ability, fx);
        }
        if self.spell_has(player, spec.ability, Augment::FanThePack) {
            self.give_combo(h, (hit.len() / 3).min(usize::from(MAX_COMBO)) as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::classes::testkit::*;

    /// Hero 0's spell at `slot` fires on the next step.
    fn ready(w: &mut World, slot: usize) {
        w.heroes[0].abilities[slot].cd = 0.0;
    }

    fn lost(w: &World, id: u32) -> f32 {
        1e6 - w.enemy(id).unwrap().hp
    }

    #[test]
    fn builders_give_combo_points_and_finishers_spend_them() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("sinister_strike", 1, &[]), ("eviscerate", 1, &[])],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.heroes[0].spell.fx.combo, 1);
        w.heroes[0].abilities[0].cd = 99.0;
        let before = lost(&w, id);
        w.heroes[0].spell.fx.combo = 5;
        ready(&mut w, 1);
        w.step();
        let dealt = lost(&w, id) - before;
        let base = w.spec_of(0, "eviscerate").unwrap().damage;
        assert!(dealt >= base * 2.25 - 0.01, "five points dealt {dealt}");
        assert_eq!(w.heroes[0].spell.fx.combo, 0);
    }

    #[test]
    fn deadly_poison_stacks_up_to_five_without_a_hit() {
        let mut w = hero_of("rogue");
        spells(&mut w, &[("deadly_poison", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        for _ in 0..7 {
            ready(&mut w, 0);
            w.step();
        }
        let e = w.enemy(id).unwrap();
        assert_eq!(e.status.burns.len(), 5);
        assert_eq!(e.hp, 1e6, "the poison hit directly");
    }

    #[test]
    fn vanish_loses_the_horde_and_sharpens_the_next_hits() {
        let mut w = hero_of("rogue");
        spells(&mut w, &[]);
        press_defense(&mut w);
        let fx = &w.heroes[0].spell.fx;
        assert!(fx.hidden > 3.0 && fx.power() >= 0.5);
        assert!(fx.defense_cd > 40.0);
    }

    #[test]
    fn slice_and_dice_lasts_longer_for_its_points_and_speeds_cooldowns() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("slice_and_dice", 1, &[]), ("sinister_strike", 1, &[])],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].spell.fx.combo = 2;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        let fx = &w.heroes[0].spell.fx;
        assert!((fx.cd_rate() - 0.3).abs() < 1e-4);
        let left = fx.buffs[0].left;
        assert!(left > 14.5, "two points should make it 15 sec: {left}");
        let owned = w.heroes[0].abilities[1].clone();
        assert!((w.cast_rate(0, &owned) - 1.3).abs() < 1e-4);
    }

    #[test]
    fn riposte_dodges_and_blind_fears_and_backstab_punishes_the_held() {
        let mut w = hero_of("rogue");
        spells(&mut w, &[("riposte", 1, &[])]);
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.hurt_hero_from(0, 40.0, Some(id)), 0.0);
        assert!(w.enemy(id).unwrap().hp < 1e6);

        let mut w = hero_of("rogue");
        spells(&mut w, &[("blind", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.fear > 3.0);

        let hit = |stunned: bool| {
            let mut w = hero_of("rogue");
            spells(&mut w, &[("backstab", 1, &[])]);
            w.heroes[0].stats.add(Stat::Crit, -1.0);
            let id = foe(&mut w, 2.0, 0.0, 1e6);
            if stunned {
                w.enemies[0].status.stun = 5.0;
            }
            w.step();
            lost(&w, id)
        };
        assert!((hit(true) / hit(false) - 2.0).abs() < 1e-3);
    }

    #[test]
    fn combat_potency_sometimes_gives_an_extra_point() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "sinister_strike",
                10,
                &[Augment::Precise, Augment::CombatPotency],
            )],
        );
        foe(&mut w, 2.0, 0.0, 1e7);
        let mut extra = false;
        for _ in 0..60 {
            w.heroes[0].spell.fx.combo = 0;
            ready(&mut w, 0);
            w.step();
            extra |= w.heroes[0].spell.fx.combo == 2;
        }
        assert!(extra);
    }

    #[test]
    fn blade_dance_bounces_through_the_pack_for_points() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "sinister_strike",
                10,
                &[Augment::Precise, Augment::BladeDance],
            )],
        );
        let ids: Vec<u32> = (0..5)
            .map(|i| foe(&mut w, 2.0 + 3.0 * i as f32, 0.0, 1e6))
            .collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));
        assert_eq!(w.heroes[0].spell.fx.combo, 5);
    }

    #[test]
    fn relentless_strikes_hurries_the_finishers() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[
                (
                    "sinister_strike",
                    10,
                    &[Augment::Precise, Augment::RelentlessStrikes],
                ),
                ("eviscerate", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 2.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        let cd = w.heroes[0].abilities[1].cd;
        assert!((cd - (2.0 - DT - 0.3)).abs() < 1e-3, "{cd}");
    }

    #[test]
    fn ambush_triples_backstab_after_vanish() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("backstab", 10, &[Augment::Precise, Augment::Ambush])],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        let id = foe(&mut w, 2.0, 0.0, 1e7);
        press_defense(&mut w);
        ready(&mut w, 0);
        w.step();
        // Its damage with Vanish's +50%, tripled, and a sure crit.
        let base = 30.0 * 1.5;
        assert!(1e7 - w.enemy(id).unwrap().hp >= base * 3.0 * 2.0 - 1.0);
    }

    #[test]
    fn opportunity_and_seal_fate() {
        let w = hero_of("rogue");
        let mut s = w.spec_of(0, "backstab").unwrap();
        Augment::Opportunity.shape(&mut s);
        assert_eq!(s.fx.vs_held, 2.0);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("backstab", 10, &[Augment::Precise, Augment::SealFate])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.heroes[0].spell.fx.combo, 2);
    }

    #[test]
    fn eviscerate_capstones_act_on_its_points() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "eviscerate",
                10,
                &[Augment::Precise, Augment::RestlessBlades],
            )],
        );
        w.heroes[0].spell.fx.defense_cd = 30.0;
        w.heroes[0].spell.fx.combo = 5;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].spell.fx.defense_cd < 20.1);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("eviscerate", 10, &[Augment::Precise, Augment::ColdBlood])],
        );
        w.heroes[0].spell.fx.combo = 5;
        foe(&mut w, 2.0, 0.0, 1e6);
        let near = foe(&mut w, 4.0, 3.0, 1e6);
        w.step();
        assert!(w.enemy(near).unwrap().hp < 1e6, "Cold Blood did not reach");

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("eviscerate", 10, &[Augment::Precise, Augment::Ruthlessness])],
        );
        w.heroes[0].spell.fx.combo = 5;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.heroes[0].spell.fx.combo, 1);
    }

    #[test]
    fn rupture_capstones_spread_feed_and_pay_back() {
        let bleed = |venom: bool| {
            let mut w = hero_of("rogue");
            spells(
                &mut w,
                &[
                    ("deadly_poison", 1, &[]),
                    ("rupture", 10, &[Augment::Lacerate, Augment::VenomousWounds]),
                ],
            );
            w.heroes[0].abilities[1].cd = 99.0;
            let id = foe(&mut w, 2.0, 0.0, 1e6);
            if venom {
                w.step();
            }
            w.heroes[0].abilities[0].cd = 99.0;
            ready(&mut w, 1);
            w.step();
            let rupture = ability(&w, "rupture");
            w.enemy(id)
                .unwrap()
                .status
                .burns
                .iter()
                .find(|b| b.ability == rupture)
                .unwrap()
                .dps
        };
        assert!((bleed(true) / bleed(false) - 1.5).abs() < 1e-3);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("rupture", 10, &[Augment::Lacerate, Augment::CrimsonTempest])],
        );
        foe(&mut w, 2.0, 0.0, 1e6);
        w.enemies[0].elite = true;
        let near = foe(&mut w, 2.0, 6.0, 1e6);
        w.step();
        assert!(w.enemy(near).unwrap().status.has_dot(0, Tag::Bleed));

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("rupture", 10, &[Augment::Lacerate, Augment::Bloodletter])],
        );
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.enemies[0].hp = 0.5;
        run(&mut w, 1.5);
        assert_eq!(w.heroes[0].spell.fx.combo, 2);
    }

    #[test]
    fn blade_flurry_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "blade_flurry",
                10,
                &[Augment::Wide, Augment::AdrenalineRush],
            )],
        );
        for i in 0..8 {
            foe(&mut w, 2.0, i as f32 * 0.5 - 2.0, 1e6);
        }
        w.step();
        assert!(w.heroes[0].spell.fx.cd_rate() >= 0.5);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("blade_flurry", 10, &[Augment::Wide, Augment::FlowingSteel])],
        );
        let owned = w.heroes[0].abilities[0].clone();
        w.heroes[0].moving = true;
        assert_eq!(w.cast_rate(0, &owned), 2.0);
        w.heroes[0].moving = false;
        assert_eq!(w.cast_rate(0, &owned), 0.5);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "blade_flurry",
                10,
                &[Augment::Wide, Augment::SerratedBlades],
            )],
        );
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));
    }

    #[test]
    fn fan_of_knives_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "fan_of_knives",
                10,
                &[Augment::Wide, Augment::PoisonedKnives],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Poison));

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("fan_of_knives", 10, &[Augment::Wide, Augment::LethalFan])],
        );
        let clean = foe(&mut w, 3.0, 0.0, 1e6);
        let dotted = foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.enemy(clean).unwrap().hp, 1e6, "a clean enemy was hit");
        w.cast_on(0, "deadly_poison", dotted);
        ready(&mut w, 0);
        w.step();
        assert!(
            w.enemy(dotted).unwrap().hp < 1e6,
            "the poisoned one was missed"
        );

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("fan_of_knives", 10, &[Augment::Wide, Augment::FanThePack])],
        );
        for i in 0..9 {
            foe(&mut w, 3.0, i as f32 * 0.5 - 2.0, 1e6);
        }
        w.step();
        assert_eq!(w.heroes[0].spell.fx.combo, 4);
    }

    #[test]
    fn deadly_poison_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[
                ("deadly_poison", 10, &[Augment::Virulent, Augment::Envenom]),
                ("eviscerate", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let id = foe(&mut w, 2.0, 0.0, 1e7);
        w.enemies[0].elite = true;
        for _ in 0..3 {
            ready(&mut w, 0);
            w.step();
        }
        w.heroes[0].abilities[0].cd = 99.0;
        let before = w.enemy(id).unwrap().hp;
        ready(&mut w, 1);
        w.step();
        let e = w.enemy(id).unwrap();
        assert!(!e.status.has_dot(0, Tag::Poison), "the stacks stayed");
        let base = w.spec_of(0, "eviscerate").unwrap().damage;
        assert!(before - e.hp > base * 2.0 + 10.0, "no envenom burst");

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "deadly_poison",
                10,
                &[Augment::Virulent, Augment::ToxicCloud],
            )],
        );
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.enemies[0].hp = 0.1;
        run(&mut w, 1.0);
        let walker = foe(&mut w, 2.0, 0.0, 1e6);
        run(&mut w, 1.2);
        assert!(
            w.enemy(walker).unwrap().status.has_dot(0, Tag::Poison),
            "no cloud"
        );

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "deadly_poison",
                10,
                &[Augment::Virulent, Augment::MindNumbingPoison],
            )],
        );
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        for _ in 0..3 {
            ready(&mut w, 0);
            w.step();
        }
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0);
    }

    #[test]
    fn gouge_capstones() {
        let w = hero_of("rogue");
        let mut s = w.spec_of(0, "gouge").unwrap();
        Augment::ImprovedGouge.shape(&mut s);
        assert_eq!(s.stun, 2.5);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("gouge", 10, &[Augment::Precise, Augment::DirtyTricks])],
        );
        foe(&mut w, 1.0, 0.0, 1e6);
        let elite = foe(&mut w, 4.0, 0.0, 1e6);
        w.enemies[1].elite = true;
        w.step();
        let e = w.enemy(elite).unwrap();
        assert!(
            e.status.stun > 1.4,
            "the elite was not held fully: {}",
            e.status.stun
        );

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("gouge", 10, &[Augment::Precise, Augment::GougedEyes])],
        );
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.enemies[0].status.stun = 2.0;
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let fin = w.spec_of(0, "eviscerate").unwrap();
        assert!((w.hit_scale(0, ei, &fin) - 1.3).abs() < 1e-4);
    }

    #[test]
    fn kidney_shot_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "kidney_shot",
                10,
                &[Augment::Precise, Augment::MarkedForDeath],
            )],
        );
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.enemies[0].boss = true;
        w.step();
        let e = w.enemy(id).unwrap();
        // A boss takes a quarter of a stun.
        assert!(e.status.stun > 0.0 && e.status.stun < 0.5, "{}", e.status.stun);
        assert!(e.status.fx.mark >= 0.25 && e.status.fx.mark_secs > 4.0);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("kidney_shot", 10, &[Augment::Precise, Augment::KidneyBurst])],
        );
        w.heroes[0].spell.fx.combo = 5;
        foe(&mut w, 2.0, 0.0, 1e6);
        let near = foe(&mut w, 2.0, 3.0, 1e6);
        w.step();
        assert!(w.enemy(near).unwrap().status.stun > 0.0);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "kidney_shot",
                10,
                &[Augment::Precise, Augment::QuickRecovery],
            )],
        );
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 100.0;
        w.heroes[0].spell.fx.combo = 5;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!((w.heroes[0].hp - 200.0).abs() < 1e-3);
    }

    #[test]
    fn slice_and_dice_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "slice_and_dice",
                10,
                &[Augment::Hasted, Augment::Adrenaline],
            )],
        );
        let speed = w.heroes[0].speed;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].speed > speed * 1.19);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[
                (
                    "slice_and_dice",
                    10,
                    &[Augment::Hasted, Augment::RestlessTempo],
                ),
                ("sinister_strike", 1, &[]),
            ],
        );
        foe(&mut w, 2.0, 0.0, 1.0);
        w.step();
        let left = w.heroes[0].spell.fx.buffs[0].left;
        assert!(left > 10.5, "no time for the kill: {left}");

        let w = hero_of("rogue");
        let mut s = w.spec_of(0, "slice_and_dice").unwrap();
        Augment::ImprovedSlice.shape(&mut s);
        assert_eq!(s.fx.buff.cd_rate, 0.45);
    }

    #[test]
    fn riposte_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[(
                "riposte",
                10,
                &[Augment::DeepReserves, Augment::Counterattack],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.drain_events();
        w.hurt_hero_from(0, 10.0, Some(id));
        let flurry = ability(&w, "blade_flurry");
        assert!(casts_of(&w.drain_events(), flurry) > 0, "no Blade Flurry");

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("riposte", 10, &[Augment::DeepReserves, Augment::Nimble])],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        let speed = w.heroes[0].speed;
        w.hurt_hero_from(0, 10.0, Some(id));
        assert!(w.heroes[0].speed > speed * 1.09);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("riposte", 10, &[Augment::DeepReserves, Augment::CheatDeath])],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        w.heroes[0].max_hp = 100.0;
        w.heroes[0].hp = 5.0;
        w.hurt_hero_from(0, 50.0, None);
        assert!(w.heroes[0].alive);
        assert!((w.heroes[0].hp - 10.0).abs() < 1e-3);
        let riposte = ability(&w, "riposte");
        assert_eq!(w.heroes[0].spell.fx.charges(riposte), 5);
    }

    #[test]
    fn blind_capstones() {
        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("blind", 10, &[Augment::Panic, Augment::BlindingPowder])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        let st = &w.enemy(id).unwrap().status;
        assert!(st.chill > st.fx.fear + 3.0);

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[("blind", 10, &[Augment::Panic, Augment::SmokeBomb])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.2);
        let st = &w.enemy(id).unwrap().status;
        assert_eq!(st.fx.fear, 0.0);
        assert!(st.fx.lost > 0.0 && st.fx.mark >= 0.3, "not in the smoke");

        let mut w = hero_of("rogue");
        spells(
            &mut w,
            &[
                ("blind", 10, &[Augment::Panic, Augment::Sap]),
                ("sinister_strike", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].stats.add(Stat::Crit, -1.0);
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.enemy(id).unwrap().status.fx.sapped_by, Some(0));
        let before = lost(&w, id);
        ready(&mut w, 1);
        w.step();
        let dealt = lost(&w, id) - before;
        let base = w.spec_of(0, "sinister_strike").unwrap().damage;
        assert!((dealt - base * 2.0).abs() < 0.01, "{dealt}");
        assert_eq!(w.enemy(id).unwrap().status.fx.sapped_by, None);
    }

    #[test]
    fn every_rogue_spell_has_three_capstones() {
        let w = hero_of("rogue");
        let class = &w.content.classes[w.content.class("rogue").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
