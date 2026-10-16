//! The paladin's capstones (docs/PALADIN-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Wrath of
//! the Executioner, Divine Storm, Avenger's Shield, Devotion and the rest) are in
//! `Augment::shape`.

use super::combat::{Crit, Death, Shot, Trigger};
use super::*;
use crate::augments::Augment;
use crate::spell::{BuffFx, Tag, CONE_HALF_ANGLE};

/// Glimmer of Light's seconds on its target.
const GLIMMER_SECS: f32 = 10.0;
/// Holy Retribution's, Seal of Righteousness's and Martyr's Hands' reach.
const RETRIBUTION_REACH: f32 = 5.0;
const SEAL_REACH: f32 = 4.0;
const MARTYR_REACH: f32 = 10.0;
/// Light of Dawn's cone.
const DAWN_REACH: f32 = 10.0;

impl World {
    /// The paladin capstones that change a cast with the hero's state: Daybreak at low health,
    /// Holy Power's next Holy spell.
    pub(super) fn paladin_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let hero = &self.heroes[h];
        if owned.has(Augment::Daybreak) && hero.hp < hero.max_hp * 0.5 {
            s.fx.heal = s.fx.heal.max(0.15);
            s.cooldown *= 0.5;
        }
        let def = &self.content.abilities[owned.ability];
        if s.tags.has(Tag::Holy) && def.kind != AbilityKind::Ward && def.damage > 0.0 {
            s.damage *= 1.0 + hero.spell.fx.boon(Augment::HolyPower);
        }
    }

    /// After a paladin spell fired: a Holy damage spell spends Holy Power.
    pub(super) fn paladin_after(&mut self, h: usize, owned: &Owned, spec: &CastSpec) {
        let def = &self.content.abilities[owned.ability];
        if spec.tags.has(Tag::Holy) && def.kind != AbilityKind::Ward && def.damage > 0.0 {
            self.heroes[h]
                .spell
                .fx
                .boons
                .retain(|b| b.key != Augment::HolyPower);
        }
    }

    /// What is left of a blow at a hero standing in a Hallowed Ground consecration.
    pub(super) fn paladin_guard(&self, h: usize) -> f32 {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        let hallowed = self.grounds.iter().any(|g| {
            g.by == player
                && g.pos.distance(pos) <= g.spec.radius
                && self.spell_has(g.by, g.spec.ability, Augment::HallowedGround)
        });
        if hallowed {
            0.85
        } else {
            1.0
        }
    }

    /// A blow of `amount` from enemy `from` reached a paladin: Judgement of Light heals it.
    pub(super) fn paladin_blow(&mut self, h: usize, _amount: f32, from: Option<u32>) {
        let player = self.heroes[h].player;
        let judged = from
            .and_then(|id| self.enemies.iter().find(|e| e.id == id))
            .is_some_and(|e| e.status.fx.judged_by == Some(player));
        if judged && self.knows(h, Augment::JudgementOfLight) {
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + hero.max_hp * 0.02).min(hero.max_hp);
        }
    }

    /// Retribution Aura's thorns from `by`'s spell `ability` answered a blow at hero `h` for
    /// `dmg`: Aura Mastery heals the hero struck.
    pub(super) fn paladin_thorns(&mut self, h: usize, by: PlayerId, ability: usize, dmg: f32) {
        if self.spell_has(by, ability, Augment::AuraMastery) {
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + dmg * 0.2).min(hero.max_hp);
        }
    }

    /// A blow blocked by a paladin's Holy Shield charge: Holy Retribution, Reckoning.
    pub(super) fn paladin_blocked(&mut self, h: usize, spec: &CastSpec, from: Option<u32>) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        if self.spell_has(player, spec.ability, Augment::HolyRetribution) {
            let near: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && Some(e.id) != from)
                .filter(|e| e.pos.distance(pos) <= RETRIBUTION_REACH)
                .map(|e| e.id)
                .collect();
            for id in near {
                self.strike(player, spec, id, 1.0, pos, Crit::Roll);
            }
        }
        if self.spell_has(player, spec.ability, Augment::Reckoning) && self.rng.f32() < 0.2 {
            if let Some(judgement) = self.content.ability("judgement") {
                self.triggers.push(Trigger {
                    player,
                    ability: judgement,
                    shot: Shot {
                        primary: false,
                        ..Shot::plain()
                    },
                });
                self.run_triggers();
            }
        }
    }

    /// A paladin's full Ward struck instead: Light of Dawn's cone.
    pub(super) fn paladin_overflow(&mut self, h: usize, spec: &CastSpec) {
        let player = self.heroes[h].player;
        if !self.spell_has(player, spec.ability, Augment::LightOfDawn) {
            return;
        }
        let (pos, facing) = (
            self.heroes[h].pos,
            self.heroes[h].facing.normalize_or_zero(),
        );
        let cone: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| {
                let off = e.pos - pos;
                let d = off.length();
                e.hp > 0.0
                    && d <= DAWN_REACH
                    && (d < 0.5
                        || (off.x * facing.x + off.y * facing.y) / d >= CONE_HALF_ANGLE.cos())
            })
            .map(|e| e.id)
            .collect();
        for id in cone {
            self.strike(player, spec, id, 1.0, pos, Crit::Roll);
        }
    }

    /// Hero `h` spent a heal charge of `ability` on hero `t` for `amount`: Holy Power, Divine
    /// Favor, Lay on Hands' capstones.
    pub(super) fn paladin_charge_spent(&mut self, h: usize, ability: usize, t: usize, amount: f32) {
        let player = self.heroes[h].player;
        if self.spell_has(player, ability, Augment::HolyPower) {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::HolyPower, 0.5, 0.5, 30.0);
        }
        if self.spell_has(player, ability, Augment::DivineFavor) && self.rng.f32() < 0.3 {
            self.heal_hero(t, amount, None);
        }
        let guard = if self.spell_has(player, ability, Augment::HandOfProtection) {
            Some((1.0, 2.0))
        } else if self.spell_has(player, ability, Augment::ImprovedLayOnHands) {
            Some((0.3, 10.0))
        } else {
            None
        };
        if let Some((guard, secs)) = guard {
            let fx = BuffFx {
                secs,
                guard,
                ..BuffFx::default()
            };
            self.grant(t, player, ability, fx);
        }
        if self.spell_has(player, ability, Augment::MartyrsHands) {
            let at = self.heroes[t].pos;
            let near: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(at) <= MARTYR_REACH)
                .map(|e| e.id)
                .collect();
            for id in near {
                self.damage_enemy(player, ability, id, amount, false);
            }
        }
    }

    /// Hero `t` was healed by `amount`: a paladin's Glimmer of Light strikes every glimmered
    /// enemy.
    pub(super) fn paladin_healed(&mut self, t: usize, amount: f32) {
        let player = self.heroes[t].player;
        if !self.knows(t, Augment::GlimmerOfLight) {
            return;
        }
        let Some(shock) = self.content.ability("holy_shock") else {
            return;
        };
        let glimmered: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && e.status.fx.glimmer_by == Some(player))
            .map(|e| e.id)
            .collect();
        for id in glimmered {
            self.damage_enemy(player, shock, id, amount * 0.5, false);
        }
    }

    /// A hit of a paladin's spell `spec` on enemy `id`: the capstones that act on a hit or a
    /// crit.
    pub(super) fn paladin_strike(
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
        if Some(spec.ability) == self.content.ability("judgement") {
            self.enemies[ei].status.fx.judged_by = Some(player);
        }
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        for a in augments {
            match a {
                Augment::BlessedHammer if crit => {
                    let hero = &mut self.heroes[h];
                    hero.hp = (hero.hp + hero.max_hp * 0.03).min(hero.max_hp);
                }
                Augment::InfusionOfLight if crit => {
                    if let Some(light) = self.content.ability("holy_light") {
                        self.add_charge(h, light);
                    }
                }
                Augment::GlimmerOfLight => {
                    let st = &mut self.enemies[ei].status.fx;
                    st.glimmer_by = Some(player);
                    st.glimmer_secs = GLIMMER_SECS;
                }
                Augment::SealOfLight => {
                    let hero = &mut self.heroes[h];
                    hero.hp = (hero.hp + hero.max_hp * 0.01).min(hero.max_hp);
                }
                Augment::SealOfRighteousness if scale > 0.99 => {
                    let near: Vec<u32> = self
                        .enemies
                        .iter()
                        .filter(|e| e.id != id && e.hp > 0.0 && e.pos.distance(at) <= SEAL_REACH)
                        .map(|e| e.id)
                        .collect();
                    for other in near {
                        self.strike(player, spec, other, 0.4, at, Crit::Roll);
                    }
                }
                Augment::SealOfWisdom => {
                    let light = self.content.ability("holy_light");
                    if let Some(o) = self.heroes[h]
                        .abilities
                        .iter_mut()
                        .find(|o| Some(o.ability) == light)
                    {
                        o.cd = (o.cd - 0.2).max(0.0);
                    }
                }
                _ => {}
            }
        }
        if crit && spec.tags.has(Tag::Holy) {
            let (exorcism, justice) = (
                self.content.ability("exorcism"),
                self.content.ability("hammer_of_justice"),
            );
            let art = self.knows(h, Augment::ArtOfWar) && self.rng.f32() < 0.2;
            let fist = self.knows(h, Augment::FistOfJustice);
            for o in &mut self.heroes[h].abilities {
                if art && Some(o.ability) == exorcism {
                    o.cd = 0.0;
                }
                if fist && Some(o.ability) == justice {
                    o.cd = (o.cd - 1.0).max(0.0);
                }
            }
        }
    }

    /// A paladin's nova hit `hit`: Divine Purpose brings the defensive button back sooner.
    pub(super) fn paladin_nova(&mut self, h: usize, spec: &CastSpec, _center: V2, hit: &[u32]) {
        let player = self.heroes[h].player;
        if self.spell_has(player, spec.ability, Augment::DivinePurpose) {
            let fx = &mut self.heroes[h].spell.fx;
            fx.defense_cd = (fx.defense_cd - 0.2 * hit.len() as f32).max(0.0);
        }
    }

    /// What dying enemies set off for paladins: Wrath of the Executioner's kill.
    pub(super) fn paladin_deaths(&mut self, deaths: &[Death]) {
        let Some(hammer) = self.content.ability("hammer_of_wrath") else {
            return;
        };
        for d in deaths {
            let Some(by) = d.killer else {
                continue;
            };
            if d.status.last_ability == Some(hammer)
                && self.spell_has(by, hammer, Augment::WrathOfTheExecutioner)
            {
                if let Some(h) = self.hero_ix(by) {
                    if let Some(o) = self.heroes[h]
                        .abilities
                        .iter_mut()
                        .find(|o| o.ability == hammer)
                    {
                        o.cd = 0.0;
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

    fn with_friend() -> World {
        let mut w = hero_of("paladin");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        w
    }

    #[test]
    fn divine_shield_takes_every_blow() {
        let mut w = hero_of("paladin");
        spells(&mut w, &[]);
        press_defense(&mut w);
        let hp = w.heroes[0].hp;
        assert_eq!(w.hurt_hero_from(0, 500.0, None), 0.0);
        assert_eq!(w.heroes[0].hp, hp);
        assert!(w.heroes[0].spell.fx.defense_cd > 80.0);
    }

    #[test]
    fn holy_shock_heals_and_the_aura_answers_blows_on_friends() {
        let mut w = hero_of("paladin");
        spells(&mut w, &[("holy_shock", 1, &[])]);
        w.heroes[0].hp = 100.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].hp > 100.0);

        let mut w = with_friend();
        spells(&mut w, &[("retribution_aura", 1, &[])]);
        let id = foe(&mut w, 6.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(1, 5.0, Some(id));
        assert!(w.enemy(id).unwrap().hp < 1e6, "the aura did not answer");
        let aura = ability(&w, "retribution_aura");
        assert!(w.heroes[0]
            .damage_by
            .iter()
            .any(|&(a, d)| a == aura && d > 0.0));
    }

    #[test]
    fn lay_on_hands_saves_a_friend_at_the_brink() {
        let mut w = with_friend();
        spells(&mut w, &[("lay_on_hands", 1, &[])]);
        w.step();
        let max = w.heroes[1].max_hp;
        w.heroes[1].hp = max * 0.3;
        w.step();
        assert!(w.heroes[1].hp < max * 0.31, "it fired above 20%");
        w.heroes[1].hp = max * 0.1;
        w.step();
        assert!((w.heroes[1].hp - max).abs() < 1e-3);
    }

    #[test]
    fn hammer_of_wrath_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "hammer_of_wrath",
                10,
                &[Augment::Precise, Augment::HammerOfTheRighteous],
            )],
        );
        foe(&mut w, 10.0, 0.0, 1e6);
        let near = foe(&mut w, 11.0, 2.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(near).unwrap().hp < 1e6);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "hammer_of_wrath",
                10,
                &[Augment::Precise, Augment::WrathOfTheExecutioner],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let low = foe(&mut w, 15.0, 0.0, 100.0);
        w.enemies[1].hp = 10.0;
        let events = run(&mut w, 0.6);
        assert!(w.enemy(low).is_none(), "the wounded one lived");
        let hammer = ability(&w, "hammer_of_wrath");
        assert!(casts_of(&events, hammer) >= 2, "the kill did not ready it");

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "hammer_of_wrath",
                10,
                &[Augment::Precise, Augment::BlessedHammer],
            )],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        w.heroes[0].hp = 100.0;
        foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.heroes[0].hp > 100.0);
    }

    #[test]
    fn judgement_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "judgement",
                10,
                &[Augment::Precise, Augment::JudgementOfLight],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].hp = 100.0;
        w.hurt_hero_from(0, 1.0, Some(id));
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("judgement", 10, &[Augment::Precise, Augment::DivineStorm])],
        );
        let ids: Vec<u32> = (0..5)
            .map(|i| foe(&mut w, 3.0, i as f32 - 2.0, 1e6))
            .collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "judgement",
                10,
                &[Augment::Precise, Augment::JudgementOfTheCrusader],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.mark >= 0.1);
    }

    #[test]
    fn exorcism_capstones() {
        let w = hero_of("paladin");
        let mut s = w.spec_of(0, "exorcism").unwrap();
        Augment::Exorcist.shape(&mut s);
        assert_eq!(s.fx.vs_elite, 1.0);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[
                ("exorcism", 10, &[Augment::Precise, Augment::ArtOfWar]),
                ("judgement", 1, &[]),
            ],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 5.0, 0.0, 1e7);
        let mut readied = false;
        for _ in 0..60 {
            w.heroes[0].abilities[0].cd = 50.0;
            ready(&mut w, 1);
            w.step();
            readied |= w.heroes[0].abilities[0].cd <= 0.0;
        }
        assert!(readied);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("exorcism", 10, &[Augment::Precise, Augment::BanishEvil])],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.stun > 1.5);
    }

    #[test]
    fn holy_shock_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "holy_shock",
                10,
                &[Augment::Precise, Augment::InfusionOfLight],
            )],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        let light = ability(&w, "holy_light");
        assert_eq!(w.heroes[0].spell.fx.charges(light), 1);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "holy_shock",
                10,
                &[Augment::Precise, Augment::GlimmerOfLight],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let before = w.enemy(id).unwrap().hp;
        w.heroes[0].hp = 10.0;
        w.heal_hero(0, 100.0, None);
        assert!((before - w.enemy(id).unwrap().hp - 50.0).abs() < 1e-3);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("holy_shock", 10, &[Augment::Precise, Augment::Daybreak])],
        );
        let owned = w.heroes[0].abilities[0].clone();
        let plain = w.spell_spec(0, &owned);
        w.heroes[0].hp = w.heroes[0].max_hp * 0.3;
        let low = w.spell_spec(0, &owned);
        assert!(low.fx.heal >= 0.15 && (low.cooldown / plain.cooldown - 0.5).abs() < 1e-4);
    }

    #[test]
    fn consecration_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("consecration", 10, &[Augment::Wide, Augment::SacredGround])],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        w.heroes[0].hp = 100.0;
        run(&mut w, 1.2);
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "consecration",
                10,
                &[Augment::Wide, Augment::HallowedGround],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!((w.paladin_guard(0) - 0.85).abs() < 1e-4);
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "consecration",
                10,
                &[Augment::Wide, Augment::ConsecratedChill],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.chill > 0.0);
    }

    #[test]
    fn holy_wrath_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("holy_wrath", 10, &[Augment::Wide, Augment::DivinePurpose])],
        );
        w.heroes[0].spell.fx.defense_cd = 50.0;
        for i in 0..10 {
            foe(&mut w, 3.0, i as f32 * 0.4, 1e6);
        }
        w.step();
        assert!(w.heroes[0].spell.fx.defense_cd < 48.1);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("holy_wrath", 10, &[Augment::Wide, Augment::StunningWrath])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.stun > 0.0);

        let w = hero_of("paladin");
        let mut s = w.spec_of(0, "holy_wrath").unwrap();
        Augment::Wrathful.shape(&mut s);
        assert_eq!(s.fx.vs_elite, 1.0);
    }

    #[test]
    fn hammer_of_justice_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[
                (
                    "hammer_of_justice",
                    10,
                    &[Augment::Precise, Augment::FistOfJustice],
                ),
                ("judgement", 1, &[]),
            ],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        w.heroes[0].abilities[0].cd = 5.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].abilities[0].cd < 4.0);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "hammer_of_justice",
                10,
                &[Augment::Precise, Augment::ShockwaveHammer],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        foe(&mut w, 6.0, 0.0, 1e6);
        let near = foe(&mut w, 6.0, 3.0, 1e6);
        w.step();
        assert!(w.enemy(near).unwrap().status.stun > 0.0);

        let w = hero_of("paladin");
        let mut s = w.spec_of(0, "hammer_of_justice").unwrap();
        Augment::LastingJustice.shape(&mut s);
        assert_eq!(s.stun, 4.5);
    }

    #[test]
    fn holy_shield_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "holy_shield",
                10,
                &[Augment::Hasted, Augment::AvengersShield],
            )],
        );
        let ids: Vec<u32> = (0..3).map(|i| foe(&mut w, 4.0, i as f32, 1e6)).collect();
        for _ in 0..5 {
            ready(&mut w, 0);
            w.step();
        }
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().status.stun > 0.0));

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "holy_shield",
                10,
                &[Augment::Hasted, Augment::HolyRetribution],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        let near = foe(&mut w, -2.0, 1.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(near).unwrap().hp < 1e6);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "holy_shield",
                10,
                &[Augment::DeepReserves, Augment::Reckoning],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e7);
        let judgement = ability(&w, "judgement");
        let mut judged = false;
        for _ in 0..60 {
            ready(&mut w, 0);
            w.step();
            w.drain_events();
            w.hurt_hero_from(0, 1.0, Some(id));
            judged |= casts_of(&w.drain_events(), judgement) > 0;
        }
        assert!(judged);
    }

    #[test]
    fn holy_light_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[
                ("holy_light", 10, &[Augment::Hasted, Augment::HolyPower]),
                ("exorcism", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let plain = w.spec_of(0, "exorcism").unwrap().damage;
        w.step();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.3;
        w.step();
        assert!((w.spec_of(0, "exorcism").unwrap().damage / plain - 1.5).abs() < 1e-4);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("holy_light", 10, &[Augment::Hasted, Augment::DivineFavor])],
        );
        let mut doubled = false;
        for _ in 0..40 {
            ready(&mut w, 0);
            w.step();
            let max = w.heroes[0].max_hp;
            w.heroes[0].hp = max * 0.1;
            w.step();
            doubled |= w.heroes[0].hp > max * 0.6;
        }
        assert!(doubled);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[("holy_light", 10, &[Augment::Hasted, Augment::LightOfDawn])],
        );
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let ids: Vec<u32> = (0..4)
            .map(|i| foe(&mut w, 3.0 + 2.0 * i as f32, 0.0, 1e6))
            .collect();
        for _ in 0..5 {
            ready(&mut w, 0);
            w.step();
        }
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));
    }

    #[test]
    fn retribution_aura_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "retribution_aura",
                10,
                &[Augment::Heavy, Augment::AuraMastery],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 500.0;
        w.hurt_hero_from(0, 1.0, Some(id));
        assert!(w.heroes[0].hp > 499.0, "no heal from the answer");

        let w = hero_of("paladin");
        let mut s = w.spec_of(0, "retribution_aura").unwrap();
        Augment::Devotion.shape(&mut s);
        Augment::Sanctity.shape(&mut s);
        assert_eq!(s.fx.buff.guard, 0.1);
        assert!((s.fx.buff.power - 0.1).abs() < 1e-4);
    }

    #[test]
    fn seal_of_command_capstones() {
        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "seal_of_command",
                10,
                &[Augment::Precise, Augment::SealOfLight],
            )],
        );
        w.heroes[0].hp = 100.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[(
                "seal_of_command",
                10,
                &[Augment::Precise, Augment::SealOfRighteousness],
            )],
        );
        foe(&mut w, 2.0, 0.0, 1e6);
        let near = foe(&mut w, 2.0, 7.0, 1e6);
        let at = w.enemies[0].pos;
        w.enemies[1].pos = at + V2::new(3.0, 0.0);
        w.step();
        assert!(w.enemy(near).unwrap().hp < 1e6);

        let mut w = hero_of("paladin");
        spells(
            &mut w,
            &[
                (
                    "seal_of_command",
                    10,
                    &[Augment::Precise, Augment::SealOfWisdom],
                ),
                ("holy_light", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 5.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].abilities[1].cd < 4.85);
    }

    #[test]
    fn lay_on_hands_capstones() {
        let mut w = with_friend();
        spells(
            &mut w,
            &[(
                "lay_on_hands",
                10,
                &[Augment::Hasted, Augment::ImprovedLayOnHands],
            )],
        );
        w.step();
        w.heroes[1].hp = 1.0;
        w.step();
        assert!(w.heroes[1].spell.fx.guard() >= 0.3);

        let mut w = with_friend();
        spells(
            &mut w,
            &[(
                "lay_on_hands",
                10,
                &[Augment::Hasted, Augment::MartyrsHands],
            )],
        );
        let id = foe(&mut w, 7.0, 0.0, 1e6);
        w.step();
        w.heroes[1].hp = w.heroes[1].max_hp * 0.35;
        w.step();
        assert!(w.enemy(id).unwrap().hp < 1e6 - 30.0, "no martyr's blast");

        let mut w = with_friend();
        spells(
            &mut w,
            &[(
                "lay_on_hands",
                10,
                &[Augment::Hasted, Augment::HandOfProtection],
            )],
        );
        w.step();
        w.heroes[1].hp = 1.0;
        w.step();
        assert_eq!(w.hurt_hero_from(1, 50.0, None), 0.0);
    }

    #[test]
    fn every_paladin_spell_has_three_capstones() {
        let w = hero_of("paladin");
        let class = &w.content.classes[w.content.class("paladin").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
