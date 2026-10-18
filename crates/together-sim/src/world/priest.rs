//! The priest's capstones (docs/PRIEST-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Martyrdom's
//! damage, Spreading Pain's reach, Misery, Starfall, Hex of Domination and the rest) are in
//! `Augment::shape`.

use super::combat::{Burn, Crit, Death, Ground, Shot, Trigger};
use super::*;
use crate::augments::Augment;
use crate::spell::{BuffFx, Tag};

/// Atonement's and Vampiric Embrace's reach for allies.
const ATONE_REACH: f32 = 30.0;
const EMBRACE_REACH: f32 = 20.0;
/// Mind Sear's reach around the target, Psychic Horror's, Rapture's and Light's Wrath's.
const SEAR_REACH: f32 = 4.0;
const HORROR_REACH: f32 = 5.0;
const RAPTURE_REACH: f32 = 8.0;
const WRATH_REACH: f32 = 10.0;
/// How far a jumping damage over time (Purge the Wicked, Plaguebearer) reaches.
const JUMP_REACH: f32 = 10.0;

impl World {
    /// The priest capstones that change a cast with the hero's state: Searing Light's focus,
    /// Insatiable at low health, Inner Focus with a full bank.
    pub(super) fn priest_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let hero = &self.heroes[h];
        let fx = &hero.spell.fx;
        if owned.has(Augment::SearingLight) {
            s.damage *= 1.0 + fx.boon(Augment::SearingLight);
        }
        if owned.has(Augment::Insatiable) && hero.hp < hero.max_hp * 0.5 {
            s.fx.leech *= 2.0;
        }
        if s.tags.has(Tag::Holy) && self.knows(h, Augment::InnerFocus) {
            if let Some(renew) = self.content.ability("renew") {
                if owned.ability != renew {
                    let cap = self
                        .spell_spec(h, &self.owned_or_new(h, renew))
                        .charges_held();
                    if fx.charges(renew) >= cap {
                        s.damage *= 1.15;
                    }
                }
            }
        }
    }

    /// Before a priest spell fires on its own cooldown: Divine Fury's every 4th Smite.
    pub(super) fn priest_before(&mut self, _h: usize, owned: &Owned, spec: &mut CastSpec) {
        if owned.has(Augment::DivineFury) && owned.casts % 4 == 3 {
            spec.count = spec.count.max(3);
        }
    }

    /// After a priest spell fired: Martyrdom's price.
    pub(super) fn priest_after(&mut self, h: usize, owned: &Owned, _spec: &CastSpec) {
        if owned.has(Augment::Martyrdom) {
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp - hero.max_hp * 0.03).max(1.0);
        }
    }

    /// The most wounded living hero within `reach` of `pos` (by share of health).
    fn most_wounded(&self, pos: V2, reach: f32) -> Option<usize> {
        (0..self.heroes.len())
            .filter(|&t| self.heroes[t].alive && self.heroes[t].pos.distance(pos) <= reach)
            .min_by(|&a, &b| {
                let share = |t: usize| self.heroes[t].hp / self.heroes[t].max_hp;
                share(a).total_cmp(&share(b))
            })
    }

    /// Heal every living hero within `reach` of `pos` by `amount`.
    fn heal_around(&mut self, pos: V2, reach: f32, amount: f32) {
        let mut healed = Vec::new();
        for hero in &mut self.heroes {
            if hero.alive && hero.pos.distance(pos) <= reach {
                let before = hero.hp;
                hero.hp = (hero.hp + amount).min(hero.max_hp);
                healed.push((hero.player, hero.hp - before));
            }
        }
        for (target, amount) in healed {
            self.credit_heal(target, amount);
        }
    }

    /// Whether some enemy carries `player`'s Devouring Plague.
    fn plague_out(&self, player: PlayerId) -> bool {
        let Some(plague) = self.content.ability("devouring_plague") else {
            return false;
        };
        self.enemies.iter().any(|e| {
            e.status
                .burns
                .iter()
                .any(|b| b.by == player && b.ability == plague)
        })
    }

    /// A hit of a priest's spell `spec` on enemy `id` for `damage`: the capstones that act on a hit
    /// or a crit.
    pub(super) fn priest_strike(
        &mut self,
        h: usize,
        id: u32,
        spec: &CastSpec,
        damage: f32,
        crit: bool,
        scale: f32,
    ) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        let Some(ei) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        let at = self.enemies[ei].pos;
        if Some(spec.ability) == self.content.ability("hex_of_weakness") {
            self.enemies[ei].status.fx.hexed_by = Some(player);
        }
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        let primary = scale > 0.99;
        for a in augments {
            match a {
                Augment::Atonement => {
                    if let Some(t) = self.most_wounded(pos, ATONE_REACH) {
                        self.heal_hero(t, damage * 0.5, None);
                    }
                }
                Augment::SearingLight => {
                    let fx = &mut self.heroes[h].spell.fx;
                    if fx.focus == Some(id) {
                        fx.add_boon(Augment::SearingLight, 0.1, 0.5, 10.0);
                    } else {
                        fx.focus = Some(id);
                        fx.boons.retain(|b| b.key != Augment::SearingLight);
                    }
                }
                Augment::PsychicHorror if primary => {
                    let mut horror = spec.clone();
                    horror.fx.fear = 2.0;
                    let near: Vec<usize> = (0..self.enemies.len())
                        .filter(|&i| self.enemies[i].pos.distance(at) <= HORROR_REACH)
                        .collect();
                    for i in near {
                        self.afflict_fx(i, &horror, pos);
                    }
                }
                Augment::ShadowyApparitions if crit => {
                    let other = self
                        .enemies
                        .iter()
                        .filter(|e| e.id != id && e.hp > 0.0 && e.pos.distance(at) <= 20.0)
                        .min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at)))
                        .map(|e| e.id);
                    if let Some(other) = other {
                        self.damage_enemy(player, spec.ability, other, damage * 0.5, false);
                    }
                }
                Augment::MindSear if primary => {
                    let near: Vec<u32> = self
                        .enemies
                        .iter()
                        .filter(|e| e.id != id && e.hp > 0.0 && e.pos.distance(at) <= SEAR_REACH)
                        .map(|e| e.id)
                        .collect();
                    for other in near {
                        self.strike(player, spec, other, 0.5, at, Crit::Roll);
                    }
                }
                Augment::PsychicLink if primary => {
                    let Some(pain) = self.content.ability("shadow_word_pain") else {
                        continue;
                    };
                    let pained: Vec<u32> = self
                        .enemies
                        .iter()
                        .filter(|e| {
                            e.id != id
                                && e.hp > 0.0
                                && e.status
                                    .burns
                                    .iter()
                                    .any(|b| b.by == player && b.ability == pain)
                        })
                        .map(|e| e.id)
                        .collect();
                    for other in pained {
                        self.strike(player, spec, other, 0.3, at, Crit::Roll);
                    }
                }
                Augment::ShootingStars if crit && primary => {
                    let far: Vec<V2> = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0.0 && e.pos.distance(at) > 6.0)
                        .filter(|e| e.pos.distance(pos) <= 20.0)
                        .map(|e| e.pos)
                        .collect();
                    let spot = far
                        .iter()
                        .max_by_key(|&&p| far.iter().filter(|&&q| p.distance(q) <= 5.0).count())
                        .copied();
                    if let Some(spot) = spot {
                        let hit: Vec<u32> = self
                            .enemies
                            .iter()
                            .filter(|e| e.hp > 0.0 && e.pos.distance(spot) <= 5.0)
                            .map(|e| e.id)
                            .collect();
                        for other in hit {
                            self.strike(player, spec, other, 1.0, spot, Crit::Never);
                        }
                    }
                }
                _ => {}
            }
        }
        if spec.tags.has(Tag::Shadow)
            && self.knows(h, Augment::VampiricEmbrace)
            && self.plague_out(player)
        {
            self.heal_around(pos, EMBRACE_REACH, damage * 0.15);
        }
    }

    /// A tick of a priest's damage over time: Holy Fervor, Shadowy Insight, Vampiric Embrace.
    pub(super) fn priest_tick(&mut self, h: usize, ability: usize, _id: u32, dps: f32) {
        let player = self.heroes[h].player;
        let (fire, pain, blast) = (
            self.content.ability("holy_fire"),
            self.content.ability("shadow_word_pain"),
            self.content.ability("mind_blast"),
        );
        let mut ready = None;
        if Some(ability) == fire
            && self.spell_has(player, ability, Augment::HolyFervor)
            && self.rng.f32() < 0.05
        {
            ready = fire;
        }
        if Some(ability) == pain && self.knows(h, Augment::ShadowyInsight) && self.rng.f32() < 0.1 {
            ready = blast;
        }
        if let Some(r) = ready {
            if let Some(o) = self.heroes[h].abilities.iter_mut().find(|o| o.ability == r) {
                o.cd = 0.0;
            }
        }
        if self.content.abilities[ability].tag_set.has(Tag::Shadow)
            && self.knows(h, Augment::VampiricEmbrace)
            && self.plague_out(player)
        {
            let pos = self.heroes[h].pos;
            self.heal_around(pos, EMBRACE_REACH, dps * 0.15);
        }
    }

    /// A priest's nova hit `hit`: Inspiration's guard grows with the crowd.
    pub(super) fn priest_nova(&mut self, h: usize, spec: &CastSpec, _center: V2, hit: &[u32]) {
        let player = self.heroes[h].player;
        if self.spell_has(player, spec.ability, Augment::Inspiration) {
            let n = hit.len() as f32;
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::Inspiration, 0.01 * n, 0.2, 4.0);
        }
    }

    /// A heal from hero `h`'s nova went `spare` past hero `t`'s full health: Divine Aegis keeps it
    /// as a shield.
    pub(super) fn priest_overheal(&mut self, h: usize, t: usize, spec: &CastSpec, spare: f32) {
        let player = self.heroes[h].player;
        if !self.spell_has(player, spec.ability, Augment::DivineAegis) {
            return;
        }
        let hero = &mut self.heroes[t];
        let sp = &mut hero.spell;
        sp.shield = (sp.shield + spare).min(hero.max_hp * 0.2).max(sp.shield);
        sp.shield_time = sp.shield_time.max(10.0);
    }

    /// Hero `h` spent a heal charge of `ability` on hero `t`: Holy Word: Sanctify when it was the
    /// last.
    pub(super) fn priest_charge_spent(
        &mut self,
        h: usize,
        ability: usize,
        _t: usize,
        _amount: f32,
    ) {
        let player = self.heroes[h].player;
        if self.heroes[h].spell.fx.charges(ability) > 0
            || !self.spell_has(player, ability, Augment::HolyWordSanctify)
        {
            return;
        }
        let hero = &mut self.heroes[h];
        hero.hp = (hero.hp + hero.max_hp * 0.25).min(hero.max_hp);
        if let Some(nova) = self.content.ability("holy_nova") {
            self.triggers.push(Trigger {
                player,
                ability: nova,
                shot: Shot {
                    primary: false,
                    ..Shot::plain()
                },
            });
        }
    }

    /// A blow of `amount` at a priest from enemy `from`: Blessed Recovery on a heavy one, Touch
    /// of Weakness on a hexed attacker.
    pub(super) fn priest_blow(&mut self, h: usize, amount: f32, from: Option<u32>) {
        let player = self.heroes[h].player;
        if amount >= self.heroes[h].max_hp * 0.15 && self.knows(h, Augment::BlessedRecovery) {
            if let Some(renew) = self.content.ability("renew") {
                if self.heroes[h].spell.fx.charges(renew) > 0 {
                    self.spend_charge(h, renew);
                    let spec = self.spell_spec(h, &self.owned_or_new(h, renew));
                    let heal = spec.fx.heal * self.heroes[h].max_hp;
                    self.heal_hero(h, heal, Some(&spec));
                }
            }
        }
        let hexed = from
            .and_then(|id| self.enemies.iter().find(|e| e.id == id))
            .is_some_and(|e| e.status.fx.hexed_by == Some(player) && e.status.fx.weaken > 0.0);
        if hexed && self.knows(h, Augment::TouchOfWeakness) {
            if let (Some(hex), Some(id)) = (self.content.ability("hex_of_weakness"), from) {
                let damage = self.spell_spec(h, &self.owned_or_new(h, hex)).damage * 3.0;
                self.damage_enemy(player, hex, id, damage, false);
            }
        }
    }

    /// What is left of a blow at a priest after its own guards: Inspiration, and a Lightwell's
    /// Sanctuary around it.
    pub(super) fn priest_guard(&self, h: usize) -> f32 {
        let mut m = 1.0 - self.heroes[h].spell.fx.boon(Augment::Inspiration);
        let pos = self.heroes[h].pos;
        let sheltered = self.grounds.iter().any(|g| {
            g.pos.distance(pos) <= g.spec.radius
                && self.spell_has(g.by, g.spec.ability, Augment::Sanctuary)
        });
        if sheltered {
            m *= 0.8;
        }
        m
    }

    /// Each step for a priest: Body and Soul while the shield holds.
    pub(super) fn priest_tend(&mut self, h: usize) {
        if self.heroes[h].spell.shield > 0.0 && self.knows(h, Augment::BodyAndSoul) {
            let before = self.heroes[h].spell.fx.haste();
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::BodyAndSoul, 0.25, 0.25, 0.2);
            if self.heroes[h].spell.fx.haste() != before {
                self.refresh(h);
            }
        }
    }

    /// A priest's shield broke: Rapture's Fear, Borrowed Time's haste.
    pub(super) fn priest_shield_broke(&mut self, h: usize) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        let Some(shield) = self.content.ability("power_word_shield") else {
            return;
        };
        if self.spell_has(player, shield, Augment::Rapture) {
            let mut fear = CastSpec::base(shield, &self.content.abilities[shield]);
            fear.fx.fear = 3.0;
            let near: Vec<usize> = (0..self.enemies.len())
                .filter(|&i| self.enemies[i].pos.distance(pos) <= RAPTURE_REACH)
                .collect();
            for i in near {
                self.afflict_fx(i, &fear, pos);
            }
        }
        if self.spell_has(player, shield, Augment::BorrowedTime) {
            let fx = BuffFx {
                secs: 4.0,
                cd_rate: 0.5,
                ..BuffFx::default()
            };
            self.grant(h, player, shield, fx);
        }
    }

    /// A priest's healing ground of `spec` healed `healed`: Light's Wrath keeps count.
    pub(super) fn priest_ground_healed(&mut self, h: usize, spec: &CastSpec, healed: f32) {
        let player = self.heroes[h].player;
        if healed > 0.0 && self.spell_has(player, spec.ability, Augment::LightsWrath) {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::LightsWrath, healed, f32::MAX, 60.0);
        }
    }

    /// A priest's ground `g` ran out at `pos`: Light's Wrath bursts for what it healed.
    pub(super) fn priest_ground_ended(&mut self, g: &Ground, pos: V2) {
        let Some(h) = self.hero_ix(g.by) else {
            return;
        };
        if !self.spell_has(g.by, g.spec.ability, Augment::LightsWrath) {
            return;
        }
        let stored = self.heroes[h].spell.fx.boon(Augment::LightsWrath);
        self.heroes[h]
            .spell
            .fx
            .boons
            .retain(|b| b.key != Augment::LightsWrath);
        if stored <= 0.0 {
            return;
        }
        let hit: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= WRATH_REACH)
            .map(|e| e.id)
            .collect();
        self.events.push(Event::Cast {
            caster: g.by,
            ability: g.spec.ability,
            visual: self.content.abilities[g.spec.ability].visual,
            targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
            dest: Some(pos),
            cooldown: 0.0,
        });
        for id in hit {
            self.damage_enemy(g.by, g.spec.ability, id, stored, false);
        }
    }

    /// A damage over time `burn` jumps from a death at `from` to the `n` closest enemies that do
    /// not carry it.
    pub(super) fn spread_dot(&mut self, burn: &Burn, from: V2, n: usize) {
        let mut near: Vec<(usize, f32)> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0.0 && e.pos.distance(from) <= JUMP_REACH)
            .filter(|(_, e)| {
                !e.status
                    .burns
                    .iter()
                    .any(|b| b.by == burn.by && b.ability == burn.ability)
            })
            .map(|(i, e)| (i, e.pos.distance(from)))
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (i, _) in near.into_iter().take(n) {
            self.enemies[i].status.burns.push(burn.clone());
        }
    }

    /// What dying enemies set off for priests: Purge the Wicked, Plaguebearer, Death's Echo,
    /// Spreading Hex.
    pub(super) fn priest_deaths(&mut self, deaths: &[Death]) {
        let (fire, plague, pain) = (
            self.content.ability("holy_fire"),
            self.content.ability("devouring_plague"),
            self.content.ability("shadow_word_pain"),
        );
        for d in deaths {
            for b in &d.status.burns {
                let jump = (Some(b.ability) == fire
                    && self.spell_has(b.by, b.ability, Augment::PurgeTheWicked))
                    || (Some(b.ability) == plague
                        && self.spell_has(b.by, b.ability, Augment::Plaguebearer));
                if jump {
                    let mut fresh = b.clone();
                    fresh.left = fresh.left.max(fresh.every * 4.0);
                    self.spread_dot(&fresh, d.pos, 2);
                }
                if Some(b.ability) == pain && self.spell_has(b.by, b.ability, Augment::DeathsEcho) {
                    let Some(h) = self.hero_ix(b.by) else {
                        continue;
                    };
                    let s = self.spell_spec(h, &self.owned_or_new(h, b.ability));
                    let total = s.damage * s.burn * s.dot_mult * s.dot_secs * s.tick_rate;
                    let next = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= 20.0)
                        .min_by(|a, c| a.pos.distance(d.pos).total_cmp(&c.pos.distance(d.pos)))
                        .map(|e| e.id);
                    if let Some(next) = next {
                        self.damage_enemy(b.by, b.ability, next, total * 0.5, false);
                    }
                }
            }
            if let Some(by) = d.status.fx.hexed_by {
                if let Some(h) = self.hero_ix(by) {
                    if self.knows(h, Augment::SpreadingHex) {
                        let next = self
                            .enemies
                            .iter()
                            .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= JUMP_REACH)
                            .min_by(|a, c| a.pos.distance(d.pos).total_cmp(&c.pos.distance(d.pos)))
                            .map(|e| e.id);
                        if let Some(next) = next {
                            self.cast_on(h, "hex_of_weakness", next);
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

    fn lost(w: &World, id: u32) -> f32 {
        w.enemy(id).unwrap().max_hp - w.enemy(id).unwrap().hp
    }

    /// A priest with a friend standing `dx` yards off.
    fn with_friend(dx: f32) -> World {
        let mut w = hero_of("priest");
        w.add_hero(1, "warrior").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(dx, 0.0);
        w
    }

    #[test]
    fn renew_heals_a_friend_in_trouble_and_smites_when_full() {
        let mut w = with_friend(6.0);
        spells(&mut w, &[("renew", 1, &[])]);
        let renew = ability(&w, "renew");
        w.step();
        assert_eq!(w.heroes[0].spell.fx.charges(renew), 1);
        let max = w.heroes[1].max_hp;
        w.heroes[1].hp = max * 0.3;
        w.step();
        assert!((w.heroes[1].hp - max * 0.5).abs() < 1.0, "not healed 20%");

        let mut w = hero_of("priest");
        spells(&mut w, &[("renew", 1, &[])]);
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        for _ in 0..6 {
            ready(&mut w, 0);
            w.step();
        }
        assert!(w.enemy(id).unwrap().hp < 1e6, "a full bank did not smite");
    }

    #[test]
    fn holy_nova_heals_and_lightwell_heals_who_stands_in_it() {
        let mut w = with_friend(4.0);
        spells(&mut w, &[("holy_nova", 1, &[])]);
        w.heroes[1].hp = 50.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[1].hp > 50.0);

        let mut w = with_friend(3.0);
        spells(&mut w, &[("lightwell", 1, &[])]);
        w.heroes[1].hp = 50.0;
        let id = foe(&mut w, 4.0, 1.0, 1e6);
        run(&mut w, 1.1);
        assert!(w.heroes[1].hp > 50.0, "no heal in the well");
        assert!(w.enemy(id).unwrap().hp < 1e6, "no smite in the well");
    }

    #[test]
    fn shadow_word_pain_has_no_hit_and_devouring_plague_heals() {
        let mut w = hero_of("priest");
        spells(&mut w, &[("shadow_word_pain", 1, &[])]);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.enemy(id).unwrap().hp, 1e6);
        run(&mut w, 1.5);
        assert!(w.enemy(id).unwrap().hp < 1e6);

        let mut w = hero_of("priest");
        spells(&mut w, &[("devouring_plague", 1, &[])]);
        w.heroes[0].hp = 100.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 2.5);
        assert!(w.heroes[0].hp > 100.0);
    }

    #[test]
    fn psychic_scream_scatters_and_hex_weakens() {
        let mut w = hero_of("priest");
        spells(&mut w, &[]);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        press_defense(&mut w);
        assert!(w.enemy(id).unwrap().status.fx.fear > 3.0);

        let mut w = hero_of("priest");
        spells(&mut w, &[("hex_of_weakness", 1, &[])]);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.weaken > 7.0);
    }

    #[test]
    fn smite_capstones() {
        let mut w = with_friend(5.0);
        spells(
            &mut w,
            &[("smite", 10, &[Augment::Precise, Augment::Atonement])],
        );
        w.heroes[1].hp = 50.0;
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!((w.heroes[1].hp - 50.0 - lost(&w, id) * 0.5).abs() < 0.01);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("smite", 10, &[Augment::Precise, Augment::DivineFury])],
        );
        w.heroes[0].abilities[0].casts = 3;
        let ids: Vec<u32> = (0..3)
            .map(|i| foe(&mut w, 5.0 + i as f32, 0.0, 1e6))
            .collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("smite", 10, &[Augment::Precise, Augment::SearingLight])],
        );
        foe(&mut w, 5.0, 0.0, 1e7);
        for _ in 0..4 {
            ready(&mut w, 0);
            w.step();
        }
        let boon = w.heroes[0].spell.fx.boon(Augment::SearingLight);
        assert!((boon - 0.3).abs() < 1e-4, "{boon}");
    }

    #[test]
    fn holy_fire_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("holy_fire", 10, &[Augment::Precise, Augment::Martyrdom])],
        );
        w.heroes[0].max_hp = 100.0;
        w.heroes[0].hp = 100.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!((w.heroes[0].hp - 97.0).abs() < 1e-3);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "holy_fire",
                10,
                &[Augment::Precise, Augment::PurgeTheWicked],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 7.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.enemies[0].hp = 0.1;
        run(&mut w, 1.2);
        assert!(w.enemy(near).unwrap().status.burning(), "no jump");

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("holy_fire", 10, &[Augment::Precise, Augment::HolyFervor])],
        );
        foe(&mut w, 5.0, 0.0, 1e9);
        let fire = ability(&w, "holy_fire");
        let mut readied = false;
        for _ in 0..100 {
            ready(&mut w, 0);
            w.step();
            w.heroes[0].abilities[0].cd = 50.0;
            readied |= casts_of(&run(&mut w, 1.0), fire) > 0;
            if readied {
                break;
            }
        }
        assert!(readied, "Holy Fervor never readied it");
    }

    #[test]
    fn mind_blast_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "mind_blast",
                10,
                &[Augment::Precise, Augment::PsychicHorror],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 7.0, 2.0, 1e6);
        w.step();
        assert!(w.enemy(near).unwrap().status.fx.fear > 0.0);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "mind_blast",
                10,
                &[Augment::Precise, Augment::ShadowyApparitions],
            )],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 5.0, 0.0, 1e6);
        let other = foe(&mut w, 9.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(other).unwrap().hp < 1e6);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[
                ("shadow_word_pain", 1, &[]),
                (
                    "mind_blast",
                    10,
                    &[Augment::Precise, Augment::ShadowyInsight],
                ),
            ],
        );
        foe(&mut w, 5.0, 0.0, 1e9);
        let mut readied = false;
        for _ in 0..60 {
            w.heroes[0].abilities[1].cd = 50.0;
            run(&mut w, 1.0);
            if w.heroes[0].abilities[1].cd <= 0.0 {
                readied = true;
                break;
            }
        }
        assert!(readied);
    }

    #[test]
    fn shadow_word_pain_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "shadow_word_pain",
                10,
                &[Augment::Quickened, Augment::SpreadingPain],
            )],
        );
        let ids: Vec<u32> = (0..6)
            .map(|i| foe(&mut w, 3.0 + 4.0 * i as f32, 0.0, 1e6))
            .collect();
        w.step();
        ready(&mut w, 0);
        w.step();
        let pained = ids
            .iter()
            .filter(|&&id| w.enemy(id).unwrap().status.burning())
            .count();
        assert_eq!(pained, 6, "the pain did not spread to fresh enemies");

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "shadow_word_pain",
                10,
                &[Augment::Quickened, Augment::Misery],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.mark >= 0.1);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "shadow_word_pain",
                10,
                &[Augment::Quickened, Augment::DeathsEcho],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let next = foe(&mut w, 30.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let at = w.enemies[0].pos;
        w.enemies[1].pos = at + V2::new(3.0, 0.0);
        w.enemies[0].hp = 0.1;
        run(&mut w, 1.2);
        assert!(w.enemy(next).unwrap().hp < 1e6, "no echo");
    }

    #[test]
    fn mind_flay_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("mind_flay", 10, &[Augment::Precise, Augment::MindSear])],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 7.0, 1.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(near).unwrap().hp < 1e6);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[
                ("shadow_word_pain", 1, &[]),
                ("mind_flay", 10, &[Augment::Precise, Augment::PsychicLink]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        let far = foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let before = w.enemy(far).unwrap().hp;
        w.enemies[1].pos = w.heroes[0].pos + V2::new(25.0, 0.0);
        ready(&mut w, 1);
        w.step();
        assert!(
            w.enemy(far).unwrap().hp < before - 1.5,
            "no link to the pained"
        );

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("mind_flay", 10, &[Augment::Precise, Augment::MindTrauma])],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0);
    }

    #[test]
    fn devouring_plague_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "devouring_plague",
                10,
                &[Augment::Quickened, Augment::Plaguebearer],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let near = foe(&mut w, 7.0, 0.0, 1e6);
        w.enemies[0].hp = 0.1;
        run(&mut w, 1.2);
        assert!(w.enemy(near).unwrap().status.burning());

        let mut w = with_friend(5.0);
        spells(
            &mut w,
            &[
                (
                    "devouring_plague",
                    10,
                    &[Augment::Quickened, Augment::VampiricEmbrace],
                ),
                ("mind_blast", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[1].hp = 50.0;
        ready(&mut w, 1);
        w.step();
        assert!(w.heroes[1].hp > 50.0, "the friend was not healed");

        let heal = |low: bool| {
            let mut w = hero_of("priest");
            spells(
                &mut w,
                &[(
                    "devouring_plague",
                    10,
                    &[Augment::Quickened, Augment::Insatiable],
                )],
            );
            w.heroes[0].max_hp = 1000.0;
            w.heroes[0].hp = if low { 100.0 } else { 600.0 };
            let before = w.heroes[0].hp;
            foe(&mut w, 5.0, 0.0, 1e6);
            run(&mut w, 2.0);
            w.heroes[0].hp - before
        };
        assert!(heal(true) > heal(false) * 1.9);
    }

    #[test]
    fn holy_nova_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("holy_nova", 10, &[Augment::Wide, Augment::SearingNova])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.burning());

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("holy_nova", 10, &[Augment::Wide, Augment::DivineAegis])],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.heroes[0].spell.shield > 0.0, "no aegis at full health");

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("holy_nova", 10, &[Augment::Wide, Augment::Inspiration])],
        );
        for i in 0..10 {
            foe(&mut w, 3.0, i as f32 * 0.3, 1e6);
        }
        w.step();
        assert!((w.priest_guard(0) - 0.9).abs() < 1e-3);
    }

    #[test]
    fn starshards_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("starshards", 10, &[Augment::Wide, Augment::Starfall])],
        );
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        run(&mut w, 1.2);
        w.heroes[0].abilities[0].cd = 99.0;
        let hp = w.enemy(id).unwrap().hp;
        assert!(hp < 1e6);
        // It goes where the priest goes.
        w.heroes[0].pos += V2::new(40.0, 0.0);
        let far = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 1.2);
        assert!(w.enemy(far).unwrap().hp < 1e6);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("starshards", 10, &[Augment::Wide, Augment::LunarGuidance])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.fx.mark >= 0.1);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("starshards", 10, &[Augment::Wide, Augment::ShootingStars])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 10.0, 0.0, 1e6);
        let other = foe(&mut w, -10.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(other).unwrap().hp < 1e6, "no shooting star");
    }

    #[test]
    fn renew_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "renew",
                10,
                &[Augment::DeepReserves, Augment::HolyWordSanctify],
            )],
        );
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.heroes[0].hp = w.heroes[0].max_hp * 0.2;
        let events = run(&mut w, DT * 2.0);
        let nova = ability(&w, "holy_nova");
        assert!(casts_of(&events, nova) > 0, "no Holy Nova");
        assert!(w.enemy(id).unwrap().hp < 1e6);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "renew",
                10,
                &[Augment::DeepReserves, Augment::BlessedRecovery],
            )],
        );
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 1000.0;
        w.hurt_hero_from(0, 200.0, None);
        assert!(
            w.heroes[0].hp > 900.0,
            "no blessed recovery: {}",
            w.heroes[0].hp
        );

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[
                ("renew", 10, &[Augment::Hasted, Augment::InnerFocus]),
                ("smite", 1, &[]),
            ],
        );
        let plain = w.spec_of(0, "smite").unwrap().damage;
        let renew = ability(&w, "renew");
        w.heroes[0].spell.fx.banks.push(Bank {
            ability: renew,
            kind: crate::content::WardKind::Heal,
            charges: 5,
        });
        let full = w.spec_of(0, "smite").unwrap().damage;
        assert!((full / plain - 1.15).abs() < 1e-4);
    }

    #[test]
    fn power_word_shield_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "power_word_shield",
                10,
                &[Augment::Hasted, Augment::BodyAndSoul],
            )],
        );
        let speed = w.heroes[0].speed;
        run(&mut w, DT * 2.0);
        assert!(w.heroes[0].speed > speed * 1.2);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "power_word_shield",
                10,
                &[Augment::Hasted, Augment::Rapture],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        let shield = w.heroes[0].spell.shield;
        w.hurt_hero_from(0, shield + 1.0, Some(id));
        assert!(w.enemy(id).unwrap().status.fx.fear > 0.0);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "power_word_shield",
                10,
                &[Augment::Hasted, Augment::BorrowedTime],
            )],
        );
        w.step();
        let shield = w.heroes[0].spell.shield;
        w.hurt_hero_from(0, shield + 1.0, None);
        assert!(w.heroes[0].spell.fx.cd_rate() >= 0.5);
    }

    #[test]
    fn lightwell_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("lightwell", 10, &[Augment::Hasted, Augment::Sanctuary])],
        );
        foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        assert!((w.priest_guard(0) - 0.8).abs() < 1e-4);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("lightwell", 10, &[Augment::Hasted, Augment::LightsWrath])],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.heroes[0].hp = 1000.0;
        run(&mut w, 15.5);
        let lost_hp = 1e6 - w.enemy(id).unwrap().hp;
        assert!(lost_hp > 1000.0, "no wrath: {lost_hp}");

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[("lightwell", 10, &[Augment::Hasted, Augment::SpringOfLight])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.chill > 0.0);
    }

    #[test]
    fn hex_capstones() {
        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "hex_of_weakness",
                10,
                &[Augment::Precise, Augment::TouchOfWeakness],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        let before = w.enemy(id).unwrap().hp;
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(id).unwrap().hp < before - 10.0);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "hex_of_weakness",
                10,
                &[Augment::Precise, Augment::HexOfDomination],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.enemies[0].damage = 100.0;
        w.enemies[0].attack_cd = 0.5;
        w.step();
        let e = w.enemy(id).unwrap();
        assert!(e.status.fx.mark >= 0.3);
        assert_eq!(World::weakness(e), 0.5);

        let mut w = hero_of("priest");
        spells(
            &mut w,
            &[(
                "hex_of_weakness",
                10,
                &[Augment::Precise, Augment::SpreadingHex],
            )],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let next = foe(&mut w, 5.0, 0.0, 1e6);
        // The hexed one dies.
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        assert!(
            w.enemy(next).unwrap().status.fx.weaken > 0.0,
            "the hex did not jump"
        );
    }

    #[test]
    fn every_priest_spell_has_three_capstones() {
        let w = hero_of("priest");
        let class = &w.content.classes[w.content.class("priest").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
