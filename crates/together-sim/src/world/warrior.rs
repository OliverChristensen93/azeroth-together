//! The warrior's capstones (docs/WARRIOR-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Heroic
//! Throw, Shockwave, Massacre and the rest) are in `Augment::shape`.

use super::combat::{Death, Shot, Trigger};
use super::*;
use crate::augments::Augment;
use crate::spell::{control_secs, BuffFx, Tag, SUNDER_MAX, SUNDER_SECS};

/// Rampage: damage per enemy hit, its cap, and seconds it lasts.
const RAMPAGE_EACH: f32 = 0.01;
const RAMPAGE_MAX: f32 = 0.3;
const RAMPAGE_SECS: f32 = 8.0;
/// Colossus Smash: its reach around the target, the share it deals there, and its stun.
const SMASH_REACH: f32 = 6.0;
const SMASH_SHARE: f32 = 0.5;
const SMASH_STUN: f32 = 1.0;
/// Revenge: its reach and how many times the block's damage it deals.
const REVENGE_REACH: f32 = 6.0;
const REVENGE_SHARE: f32 = 1.5;
/// Gushing Wound's reach.
const GUSH_REACH: f32 = 5.0;

impl World {
    /// The warrior capstones that change a cast with the hero's state: Rampage's damage,
    /// Overpower's next hit, Blood Craze at low health.
    pub(super) fn warrior_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let hero = &self.heroes[h];
        let fx = &hero.spell.fx;
        s.damage *= 1.0 + fx.boon(Augment::Rampage);
        if owned.has(Augment::Overpower) {
            s.damage *= 1.0 + fx.boon(Augment::Overpower);
        }
        if owned.has(Augment::BloodCraze) && hero.hp < hero.max_hp * 0.5 {
            s.cooldown *= 0.5;
            s.fx.leech *= 2.0;
        }
    }

    /// Before a warrior spell fires on its own cooldown: Cleaving Swings' every 3rd Cleave.
    pub(super) fn warrior_before(&mut self, _h: usize, owned: &Owned, spec: &mut CastSpec) {
        if owned.has(Augment::CleavingSwings) && owned.casts % 3 == 2 {
            spec.count = 99;
        }
    }

    /// After a warrior spell fired: Overpower's bonus is spent, War Cry and Challenging Shout
    /// give their buffs.
    pub(super) fn warrior_after(&mut self, h: usize, owned: &Owned, spec: &CastSpec) {
        let player = self.heroes[h].player;
        if owned.has(Augment::Overpower) {
            self.heroes[h]
                .spell
                .fx
                .boons
                .retain(|b| b.key != Augment::Overpower);
        }
        if owned.has(Augment::WarCry) {
            let pos = self.heroes[h].pos;
            let fx = BuffFx {
                secs: 6.0,
                power: 0.15,
                ..BuffFx::default()
            };
            for i in 0..self.heroes.len() {
                if self.heroes[i].alive && self.heroes[i].pos.distance(pos) <= 20.0 {
                    self.grant(i, player, spec.ability, fx);
                }
            }
        }
        if owned.has(Augment::ChallengingShout) {
            let fx = BuffFx {
                secs: 4.0,
                guard: 0.4,
                ..BuffFx::default()
            };
            self.grant(h, player, spec.ability, fx);
        }
    }

    /// A full Ward's overflow by a warrior: nothing of its own yet.
    pub(super) fn warrior_overflow(&mut self, _h: usize, _spec: &CastSpec) {}

    /// How much harder a warrior's hit lands: nothing beyond the shared bonuses yet.
    pub(super) fn warrior_scale(&self, _h: usize, _ei: usize, _spec: &CastSpec) -> f32 {
        1.0
    }

    /// What is left of a blow at a warrior after its own guards: all of it (Challenging Shout's
    /// guard is a buff).
    pub(super) fn warrior_guard(&self, _h: usize) -> f32 {
        1.0
    }

    /// A warrior's Bleeds heal it with Blood Frenzy.
    pub(super) fn warrior_dot_leech(&self, h: usize, spec: &CastSpec) -> f32 {
        if spec.tags.has(Tag::Bleed) && self.knows(h, Augment::BloodFrenzy) {
            0.2
        } else {
            0.0
        }
    }

    /// A hit of a warrior's spell `spec` on enemy `id`: the capstones that act on a hit or a crit.
    pub(super) fn warrior_strike(
        &mut self,
        h: usize,
        id: u32,
        spec: &CastSpec,
        _damage: f32,
        crit: bool,
        scale: f32,
    ) {
        let player = self.heroes[h].player;
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        let Some(ei) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        let (at, elite) = (
            self.enemies[ei].pos,
            self.enemies[ei].elite || self.enemies[ei].boss,
        );
        let rend = self.content.ability("rend");
        for a in augments {
            match a {
                Augment::BloodAndThunder => {
                    if !self.enemies[ei].status.has_dot(player, Tag::Bleed) {
                        self.cast_on(h, "rend", id);
                    }
                }
                Augment::Rampage => {
                    self.heroes[h].spell.fx.add_boon(
                        Augment::Rampage,
                        RAMPAGE_EACH,
                        RAMPAGE_MAX,
                        RAMPAGE_SECS,
                    );
                }
                Augment::Overpower if crit => {
                    if let Some(o) = self.heroes[h]
                        .abilities
                        .iter_mut()
                        .find(|o| o.ability == spec.ability)
                    {
                        o.cd = 0.0;
                    }
                    self.heroes[h]
                        .spell
                        .fx
                        .add_boon(Augment::Overpower, 0.5, 0.5, 5.0);
                }
                Augment::ChampionsGuard if elite => {
                    if let Some(block) = self.content.ability("shield_block") {
                        self.add_charge(h, block);
                    }
                }
                Augment::ColossusSmash if scale > 0.99 => {
                    let around: Vec<u32> = self
                        .enemies
                        .iter()
                        .filter(|e| e.id != id && e.hp > 0.0 && e.pos.distance(at) <= SMASH_REACH)
                        .map(|e| e.id)
                        .collect();
                    let mut smash = spec.clone();
                    smash.stun = smash.stun.max(SMASH_STUN);
                    for other in around {
                        self.strike(
                            player,
                            &smash,
                            other,
                            SMASH_SHARE,
                            at,
                            super::combat::Crit::Roll,
                        );
                    }
                }
                Augment::ExposeArmor => {
                    let st = &mut self.enemies[ei].status.fx;
                    if st.sunder >= SUNDER_MAX {
                        st.sunder = 0;
                        st.mark = st.mark.max(0.25);
                        st.mark_secs = st.mark_secs.max(8.0);
                    }
                }
                _ => {}
            }
        }
        if crit && spec.tags.has(Tag::Melee) {
            if self.knows(h, Augment::DeepWounds) && Some(spec.ability) != rend {
                self.cast_on(h, "rend", id);
            }
            if self.knows(h, Augment::SunderingBlows) {
                if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                    if e.status.fx.sunder > 0 {
                        e.status.fx.sunder = (e.status.fx.sunder + 1).min(SUNDER_MAX);
                        e.status.fx.sunder_secs = SUNDER_SECS;
                    }
                }
            }
        }
        if crit
            && self.spell_has(player, spec.ability, Augment::UnbridledWrath)
            && self.heroes[h].spell.ready(Augment::UnbridledWrath)
        {
            self.heroes[h].spell.spend(Augment::UnbridledWrath, 4.0);
            if let Some(ww) = self.content.ability("whirlwind") {
                self.triggers.push(Trigger {
                    player,
                    ability: ww,
                    shot: Shot {
                        primary: false,
                        ..Shot::plain()
                    },
                });
            }
        }
    }

    /// A blow blocked by a block charge of `spec`: Revenge's spin and Shield Bash's stun.
    pub(super) fn warrior_blocked(&mut self, h: usize, spec: &CastSpec, from: Option<u32>) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        if self.knows(h, Augment::Revenge) {
            let near: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= REVENGE_REACH)
                .map(|e| e.id)
                .collect();
            self.events.push(Event::Cast {
                caster: player,
                ability: spec.ability,
                visual: self.content.abilities[spec.ability].visual,
                targets: near.iter().map(|&id| Target::Enemy(id)).collect(),
                dest: Some(pos),
                cooldown: 0.0,
            });
            for id in near {
                self.strike(
                    player,
                    spec,
                    id,
                    REVENGE_SHARE,
                    pos,
                    super::combat::Crit::Roll,
                );
            }
        }
        if self.knows(h, Augment::ShieldBash) {
            if let Some(e) = from.and_then(|id| self.enemies.iter_mut().find(|e| e.id == id)) {
                if !e.boss {
                    e.status.stun = e.status.stun.max(control_secs(1.5, e.elite, false));
                }
            }
        }
    }

    /// Each step for a warrior: Last Stand refills the block charges at low health.
    pub(super) fn warrior_tend(&mut self, h: usize) {
        let hero = &self.heroes[h];
        if hero.hp >= hero.max_hp * 0.3
            || !hero.spell.ready(Augment::LastStand)
            || !self.knows(h, Augment::LastStand)
        {
            return;
        }
        let Some(block) = self.content.ability("shield_block") else {
            return;
        };
        self.heroes[h].spell.spend(Augment::LastStand, 20.0);
        let cap = self
            .spell_spec(h, &self.owned_or_new(h, block))
            .charges_held();
        for _ in 0..cap {
            self.add_charge(h, block);
        }
    }

    /// What dying enemies set off for warriors: Sudden Death, Furious Spin, Executioner's Chain,
    /// Victory Rush and Gushing Wound.
    pub(super) fn warrior_deaths(&mut self, deaths: &[Death]) {
        let (ms, ww, exe) = (
            self.content.ability("mortal_strike"),
            self.content.ability("whirlwind"),
            self.content.ability("execute"),
        );
        for d in deaths {
            if let Some(by) = d.killer {
                if let Some(h) = self.hero_ix(by) {
                    let last = d.status.last_ability;
                    let melee =
                        last.is_some_and(|a| self.content.abilities[a].tag_set.has(Tag::Melee));
                    if melee && self.knows(h, Augment::SuddenDeath) && self.rng.f32() < 0.1 {
                        if let Some(o) = self.heroes[h]
                            .abilities
                            .iter_mut()
                            .find(|o| Some(o.ability) == ms)
                        {
                            o.cd = 0.0;
                        }
                    }
                    if last.is_some()
                        && last == ww
                        && self.spell_has(by, ww.unwrap_or(0), Augment::FuriousSpin)
                    {
                        if let Some(o) = self.heroes[h]
                            .abilities
                            .iter_mut()
                            .find(|o| Some(o.ability) == ww)
                        {
                            o.cd = (o.cd - 0.5).max(0.0);
                        }
                    }
                    if let (Some(exe), true) = (exe, last.is_some() && last == exe) {
                        if self.spell_has(by, exe, Augment::ExecutionersChain) {
                            self.triggers.push(Trigger {
                                player: by,
                                ability: exe,
                                shot: Shot {
                                    primary: false,
                                    ..Shot::plain()
                                },
                            });
                        }
                        if self.spell_has(by, exe, Augment::VictoryRush) {
                            let hero = &mut self.heroes[h];
                            hero.hp = (hero.hp + hero.max_hp * 0.05).min(hero.max_hp);
                        }
                    }
                }
            }
            let gushing: Vec<super::combat::Burn> = d
                .status
                .burns
                .iter()
                .filter(|b| b.tags.has(Tag::Bleed))
                .filter(|b| {
                    self.hero_ix(b.by)
                        .is_some_and(|h| self.knows(h, Augment::GushingWound))
                })
                .cloned()
                .collect();
            if gushing.is_empty() {
                continue;
            }
            for e in self
                .enemies
                .iter_mut()
                .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= GUSH_REACH)
            {
                for b in &gushing {
                    match e
                        .status
                        .burns
                        .iter_mut()
                        .find(|x| x.by == b.by && x.ability == b.ability)
                    {
                        Some(old) if old.left < b.left => *old = b.clone(),
                        Some(_) => {}
                        None => e.status.burns.push(b.clone()),
                    }
                }
            }
        }
    }

    /// A warrior's nova hit `hit`: Second Wind and Rallying Cry heal by how many it reached.
    pub(super) fn warrior_nova(&mut self, h: usize, spec: &CastSpec, _center: V2, hit: &[u32]) {
        let player = self.heroes[h].player;
        let n = hit.len() as f32;
        let mut share = 0.0;
        if self.spell_has(player, spec.ability, Augment::SecondWind) {
            share += (0.01 * n).min(0.10);
        }
        if self.spell_has(player, spec.ability, Augment::RallyingCry) {
            share += (0.01 * n).min(0.15);
        }
        if share > 0.0 {
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + hero.max_hp * share).min(hero.max_hp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::classes::testkit::*;
    use super::*;

    #[test]
    fn charge_leaps_the_way_you_face_and_stuns_where_you_land() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[]);
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let start = w.heroes[0].pos;
        let id = foe(&mut w, 13.0, 0.0, 1e6);
        press_defense(&mut w);
        assert!(w.heroes[0].pos.distance(start + V2::new(12.0, 0.0)) < 0.1);
        assert!(w.enemy(id).unwrap().status.stun > 0.0, "no stun at landing");
        let cd = w.heroes[0].spell.fx.defense_cd;
        assert!(cd > 25.0, "cooldown {cd}");
        // Pressed again at once: nothing happens.
        let at = w.heroes[0].pos;
        press_defense(&mut w);
        assert_eq!(w.heroes[0].pos, at);
    }

    #[test]
    fn rend_bleeds_without_a_hit_and_execute_seeks_the_most_wounded() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("rend", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.enemy(id).unwrap().hp, 1e6, "Rend hit directly");
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));
        run(&mut w, 2.0);
        assert!(w.enemy(id).unwrap().hp < 1e6, "no bleed ticks");

        let mut w = hero_of("warrior");
        spells(&mut w, &[("execute", 1, &[])]);
        let near = foe(&mut w, 2.0, 0.0, 100.0);
        let hurt = foe(&mut w, 6.0, 0.0, 100.0);
        foe(&mut w, 7.0, 0.0, 100.0);
        w.enemies[1].hp = 20.0;
        w.enemies[2].hp = 30.0;
        let events = run(&mut w, DT * 2.0);
        assert_eq!(
            first_target(&w, &events, "execute"),
            Some(Target::Enemy(hurt))
        );
        assert_eq!(w.enemy(near).unwrap().hp, 100.0);
    }

    #[test]
    fn shield_block_banks_blocks_that_strike_back_then_overflows() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("shield_block", 1, &[])]);
        let block = ability(&w, "shield_block");
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        for _ in 0..3 {
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
        }
        assert_eq!(w.heroes[0].spell.fx.charges(block), 3);
        let hp = w.heroes[0].hp;
        let took = w.hurt_hero_from(0, 50.0, Some(id));
        assert_eq!(took, 0.0, "the blow was not blocked");
        assert_eq!(w.heroes[0].hp, hp);
        assert!(w.enemy(id).unwrap().hp < 1e6, "no retaliation");
        assert_eq!(w.heroes[0].spell.fx.charges(block), 2);
        // Full again, the next cast strikes instead.
        w.heroes[0].abilities[0].cd = 0.0;
        w.step();
        let before = w.enemy(id).unwrap().hp;
        w.heroes[0].abilities[0].cd = 0.0;
        w.step();
        assert!(w.enemy(id).unwrap().hp < before, "no overflow");
    }

    #[test]
    fn shouts_weaken_and_fear_and_a_feared_enemy_runs() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("intimidating_shout", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.enemies[0].speed = 5.0;
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.fear > 0.0, "not feared");
        let d0 = w.enemy(id).unwrap().pos.distance(w.heroes[0].pos);
        run(&mut w, 1.0);
        let d1 = w.enemy(id).unwrap().pos.distance(w.heroes[0].pos);
        assert!(d1 > d0 + 2.0, "it did not run: {d0} -> {d1}");

        let mut w = hero_of("warrior");
        spells(&mut w, &[("demoralizing_shout", 1, &[])]);
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.enemies[0].damage = 100.0;
        w.enemies[0].attack_cd = 0.5;
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0, "not weakened");
        let events = run(&mut w, 1.0);
        let swing = events.iter().find_map(|e| match e {
            Event::Swing { amount, .. } => Some(*amount),
            _ => None,
        });
        assert_eq!(swing, Some(75.0));
    }

    #[test]
    fn sunder_stacks_make_the_target_take_more_from_everyone() {
        let mut w = hero_of("warrior");
        spells(&mut w, &[("sunder_armor", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        for _ in 0..7 {
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
        }
        let st = &w.enemy(id).unwrap().status.fx;
        assert_eq!(st.sunder, SUNDER_MAX);
        assert!((st.exposure() - 1.2).abs() < 1e-4);
    }

    #[test]
    fn rampage_stacks_damage_from_every_cleave_hit() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("cleave", 10, &[Augment::Precise, Augment::Rampage])],
        );
        for i in 0..3 {
            foe(&mut w, 2.0, i as f32 - 1.0, 1e6);
        }
        w.step();
        let boon = w.heroes[0].spell.fx.boon(Augment::Rampage);
        assert!((boon - 0.03).abs() < 0.011, "rampage {boon}");
    }

    #[test]
    fn blood_and_thunder_spreads_rend_and_cleaving_swings_hits_everyone() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("cleave", 10, &[Augment::Sweeping, Augment::BloodAndThunder])],
        );
        let id = foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("cleave", 10, &[Augment::Sweeping, Augment::CleavingSwings])],
        );
        w.heroes[0].abilities[0].casts = 2;
        let ids: Vec<u32> = (0..6)
            .map(|i| foe(&mut w, 2.0 + 0.3 * i as f32, 0.0, 1e6))
            .collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));
    }

    #[test]
    fn overpower_crits_reset_heroic_strike() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("heroic_strike", 10, &[Augment::Precise, Augment::Overpower])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 3.0, 0.0, 1e7);
        w.step();
        assert_eq!(w.heroes[0].abilities[0].cd, 0.0, "not reset");
        assert!(w.heroes[0].spell.fx.boon(Augment::Overpower) > 0.0);
    }

    #[test]
    fn heroic_throw_reaches_and_champions_guard_banks_blocks() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "heroic_strike",
                10,
                &[Augment::Precise, Augment::HeroicThrow],
            )],
        );
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().hp < 1e6, "the throw did not reach");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "heroic_strike",
                10,
                &[Augment::Precise, Augment::ChampionsGuard],
            )],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        w.enemies[0].elite = true;
        w.step();
        let block = ability(&w, "shield_block");
        assert_eq!(w.heroes[0].spell.fx.charges(block), 1);
    }

    #[test]
    fn mortal_wounds_marks_and_colossus_smash_stuns_the_guard() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "mortal_strike",
                10,
                &[Augment::Precise, Augment::MortalWounds],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        let st = &w.enemy(id).unwrap().status.fx;
        assert!(st.mark >= 0.2 && st.mark_secs > 0.0, "not marked");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "mortal_strike",
                10,
                &[Augment::Precise, Augment::ColossusSmash],
            )],
        );
        let boss = foe(&mut w, 3.0, 0.0, 1e6);
        w.enemies[0].elite = true;
        let guard = foe(&mut w, 5.0, 2.0, 1e6);
        w.step();
        assert!(w.enemy(boss).unwrap().hp < 1e6);
        let g = w.enemy(guard).unwrap();
        assert!(
            g.hp < 1e6 && g.status.stun > 0.0,
            "the guard was not smashed"
        );
    }

    #[test]
    fn sudden_death_can_ready_mortal_strike() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[
                ("cleave", 1, &[]),
                (
                    "mortal_strike",
                    10,
                    &[Augment::Precise, Augment::SuddenDeath],
                ),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let mut readied = false;
        for _ in 0..200 {
            foe(&mut w, 2.0, 0.0, 1.0);
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
            if w.heroes[0].abilities[1].cd == 0.0 {
                readied = true;
                break;
            }
        }
        assert!(readied, "200 cleave kills never readied Mortal Strike");
    }

    #[test]
    fn bladestorm_follows_you_and_keeps_cutting() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("whirlwind", 10, &[Augment::Wide, Augment::Bladestorm])],
        );
        w.heroes[0].abilities[0].cd = 0.0;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        // Walk away: a foe that was far is now beside the hero.
        let far = foe(&mut w, 30.0, 0.0, 1e6);
        w.heroes[0].pos += V2::new(29.0, 0.0);
        run(&mut w, 1.5);
        assert!(w.enemy(far).unwrap().hp < 1e6, "the storm did not follow");
    }

    #[test]
    fn whirlwind_kills_shorten_it_and_second_wind_heals() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("whirlwind", 10, &[Augment::Wide, Augment::FuriousSpin])],
        );
        for i in 0..4 {
            foe(&mut w, 2.0, i as f32 * 0.4, 1.0);
        }
        w.step();
        let full = w.spell_spec(0, &w.heroes[0].abilities[0].clone()).cooldown;
        assert!(w.heroes[0].abilities[0].cd < full - 1.0, "no time back");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("whirlwind", 10, &[Augment::Wide, Augment::SecondWind])],
        );
        w.heroes[0].hp = w.heroes[0].max_hp * 0.5;
        for i in 0..5 {
            foe(&mut w, 2.0, i as f32 * 0.4, 1e6);
        }
        w.step();
        let gained = w.heroes[0].hp / w.heroes[0].max_hp - 0.5;
        assert!((gained - 0.05).abs() < 0.001, "healed {gained}");
    }

    #[test]
    fn shockwave_stuns_a_cone_and_aftershock_keeps_chilling() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("thunder_clap", 10, &[Augment::Wide, Augment::Shockwave])],
        );
        w.heroes[0].facing = V2::new(1.0, 0.0);
        let ahead = foe(&mut w, 10.0, 0.0, 1e6);
        let behind = foe(&mut w, -3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(ahead).unwrap().status.stun > 0.0);
        assert_eq!(w.enemy(behind).unwrap().hp, 1e6);

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("thunder_clap", 10, &[Augment::Wide, Augment::Aftershock])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        run(&mut w, 3.5);
        let e = w.enemy(id).unwrap();
        assert!(e.status.chill > 0.0, "the shaking ground did not chill");
    }

    #[test]
    fn thunderstruck_weakens() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("thunder_clap", 10, &[Augment::Wide, Augment::Thunderstruck])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0);
    }

    #[test]
    fn blood_frenzy_heals_from_bleeds_and_gushing_wound_spreads_them() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("rend", 10, &[Augment::Lacerate, Augment::BloodFrenzy])],
        );
        w.heroes[0].hp = 100.0;
        foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 3.0);
        assert!(w.heroes[0].hp > 100.0, "no heal from the bleed");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("rend", 10, &[Augment::Lacerate, Augment::GushingWound])],
        );
        let dying = foe(&mut w, 3.0, 0.0, 1e6);
        let near = foe(&mut w, 30.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        // Move the bystander beside the bleeding one, then let it die.
        let at = w.enemy(dying).unwrap().pos;
        w.enemies[1].pos = at + V2::new(2.0, 0.0);
        w.enemies[0].hp = 0.1;
        run(&mut w, 1.5);
        assert!(
            w.enemy(near).unwrap().status.has_dot(0, Tag::Bleed),
            "no gush"
        );
    }

    #[test]
    fn deep_wounds_bleeds_on_melee_crits() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[
                ("heroic_strike", 1, &[]),
                ("rend", 10, &[Augment::Lacerate, Augment::DeepWounds]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));
    }

    #[test]
    fn executioners_chain_and_victory_rush_act_on_kills() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "execute",
                10,
                &[Augment::Precise, Augment::ExecutionersChain],
            )],
        );
        let exe = ability(&w, "execute");
        for i in 0..4 {
            foe(&mut w, 2.0 + i as f32, 0.0, 1.0);
        }
        let events = run(&mut w, DT * 3.0);
        assert!(casts_of(&events, exe) >= 3, "the chain stopped");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("execute", 10, &[Augment::Precise, Augment::VictoryRush])],
        );
        w.heroes[0].hp = 100.0;
        foe(&mut w, 2.0, 0.0, 1.0);
        run(&mut w, DT * 3.0);
        assert!(w.heroes[0].hp > 100.0);
    }

    #[test]
    fn massacre_raises_the_execute_threshold() {
        let w = hero_of("warrior");
        let mut s = w.spec_of(0, "execute").unwrap();
        Augment::Massacre.shape(&mut s);
        assert_eq!(s.fx.execute_at, 0.5);
    }

    #[test]
    fn bloodthirst_heals_and_blood_craze_doubles_it_when_low() {
        let heal = |craze: bool| {
            let mut w = hero_of("warrior");
            let augments: &[Augment] = if craze {
                &[Augment::Precise, Augment::BloodCraze]
            } else {
                &[Augment::Precise]
            };
            spells(&mut w, &[("bloodthirst", 10, augments)]);
            w.heroes[0].max_hp = 1000.0;
            w.heroes[0].hp = 100.0;
            foe(&mut w, 3.0, 0.0, 1e6);
            w.step();
            w.heroes[0].hp - 100.0
        };
        let (plain, crazed) = (heal(false), heal(true));
        assert!(plain > 0.0 && crazed > plain * 1.9, "{plain} vs {crazed}");
    }

    #[test]
    fn unbridled_wrath_spins_and_bloodbath_bleeds() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "bloodthirst",
                10,
                &[Augment::Precise, Augment::UnbridledWrath],
            )],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 3.0, 0.0, 1e6);
        let ww = ability(&w, "whirlwind");
        let events = run(&mut w, DT * 3.0);
        assert!(casts_of(&events, ww) > 0, "no Whirlwind");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("bloodthirst", 10, &[Augment::Precise, Augment::Bloodbath])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.has_dot(0, Tag::Bleed));
    }

    #[test]
    fn revenge_spins_on_a_block_and_shield_bash_stuns() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "shield_block",
                10,
                &[Augment::DeepReserves, Augment::Revenge],
            )],
        );
        let attacker = foe(&mut w, 1.0, 0.0, 1e6);
        let bystander = foe(&mut w, -2.0, 1.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 10.0, Some(attacker));
        assert!(w.enemy(bystander).unwrap().hp < 1e6, "no revenge");

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "shield_block",
                10,
                &[Augment::DeepReserves, Augment::ShieldBash],
            )],
        );
        let attacker = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 10.0, Some(attacker));
        assert!(w.enemy(attacker).unwrap().status.stun > 0.0);
    }

    #[test]
    fn last_stand_refills_the_blocks_at_low_health() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "shield_block",
                10,
                &[Augment::DeepReserves, Augment::LastStand],
            )],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        let block = ability(&w, "shield_block");
        w.heroes[0].hp = w.heroes[0].max_hp * 0.2;
        w.step();
        assert_eq!(w.heroes[0].spell.fx.charges(block), 5);
    }

    #[test]
    fn rallying_cry_heals_by_the_crowd_and_war_cry_rallies_the_party() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "demoralizing_shout",
                10,
                &[Augment::Wide, Augment::RallyingCry],
            )],
        );
        w.heroes[0].hp = w.heroes[0].max_hp * 0.5;
        for i in 0..20 {
            foe(&mut w, 2.0, i as f32 * 0.3, 1e6);
        }
        w.step();
        let gained = w.heroes[0].hp / w.heroes[0].max_hp - 0.5;
        assert!((gained - 0.15).abs() < 0.001, "healed {gained}");

        let mut w = hero_of("warrior");
        w.add_hero(1, "mage").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(
            &mut w,
            &[("demoralizing_shout", 10, &[Augment::Wide, Augment::WarCry])],
        );
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!(
            w.heroes[1].spell.fx.power() > 0.1,
            "the friend was not rallied"
        );
    }

    #[test]
    fn crippling_presence_chills() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "demoralizing_shout",
                10,
                &[Augment::Wide, Augment::CripplingPresence],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.chill > 0.0);
    }

    #[test]
    fn challenging_shout_gathers_and_guards() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "intimidating_shout",
                10,
                &[Augment::Wide, Augment::ChallengingShout],
            )],
        );
        let id = foe(&mut w, 12.0, 0.0, 1e6);
        w.step();
        let e = w.enemy(id).unwrap();
        assert!(e.pos.distance(w.heroes[0].pos) < 3.0, "not pulled");
        assert_eq!(e.status.fx.fear, 0.0, "it should not flee");
        assert!(w.heroes[0].spell.fx.guard() >= 0.4);
    }

    #[test]
    fn shattering_roar_sunders_and_fearsome_holds_elites() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "intimidating_shout",
                10,
                &[Augment::Wide, Augment::ShatteringRoar],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert_eq!(w.enemy(id).unwrap().status.fx.sunder, 2);

        let fear = |fearsome: bool| {
            let mut w = hero_of("warrior");
            let augments: &[Augment] = if fearsome {
                &[Augment::Wide, Augment::Fearsome]
            } else {
                &[Augment::Wide]
            };
            spells(&mut w, &[("intimidating_shout", 10, augments)]);
            let id = foe(&mut w, 3.0, 0.0, 1e6);
            w.enemies[0].elite = true;
            w.step();
            w.enemy(id).unwrap().status.fx.fear
        };
        assert!(fear(true) > fear(false) * 1.9);
    }

    #[test]
    fn expose_armor_turns_five_stacks_into_a_mark() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[(
                "sunder_armor",
                10,
                &[Augment::Precise, Augment::ExposeArmor],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        for _ in 0..5 {
            w.heroes[0].abilities[0].cd = 0.0;
            w.step();
        }
        let st = &w.enemy(id).unwrap().status.fx;
        assert_eq!(st.sunder, 0);
        assert!(st.mark >= 0.25);
    }

    #[test]
    fn devastate_sunders_everything_near_and_sundering_blows_adds_stacks() {
        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[("sunder_armor", 10, &[Augment::Precise, Augment::Devastate])],
        );
        let ids: Vec<u32> = (0..4).map(|i| foe(&mut w, 2.0, i as f32, 1e6)).collect();
        w.step();
        assert!(ids
            .iter()
            .all(|&id| w.enemy(id).unwrap().status.fx.sunder == 1));

        let mut w = hero_of("warrior");
        spells(
            &mut w,
            &[
                ("heroic_strike", 1, &[]),
                (
                    "sunder_armor",
                    10,
                    &[Augment::Precise, Augment::SunderingBlows],
                ),
            ],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.sunder >= 2);
    }

    #[test]
    fn every_warrior_capstone_has_its_spell_in_the_pool() {
        let w = hero_of("warrior");
        let class = &w.content.classes[w.content.class("warrior").unwrap()];
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
        assert_eq!(class.pool.len(), 12);
        assert_eq!(
            class
                .defensive_ix
                .map(|a| w.content.abilities[a].key.as_str()),
            Some("charge")
        );
    }
}
