//! The warlock's capstones (docs/WARLOCK-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Shadow
//! Bolt Volley, Siphon Life, Amplify Curse, Shadowfury, Demonic Sacrifice and the rest) are in
//! `Augment::shape`.

use super::combat::{Crit, Death, Ground};
use super::*;
use crate::augments::Augment;
use crate::spell::{control_secs, Tag};

/// Seed of Corruption's burst, Horrify's spread, Dark Bargain's reach.
const SEED_REACH: f32 = 6.0;
const HORRIFY_REACH: f32 = 5.0;
const BARGAIN_REACH: f32 = 10.0;
/// Improved Shadow Bolt: the most stacks, the Shadow damage each adds, seconds they last.
const VULN_MAX: u8 = 5;
const VULN_EACH: f32 = 0.04;
const VULN_SECS: f32 = 10.0;

impl World {
    /// Whether hero `h` has a ground of spell `key` out (a summoned demon, a Hellfire).
    fn has_ground(&self, h: usize, key: &str) -> Option<usize> {
        let ability = self.content.ability(key)?;
        let player = self.heroes[h].player;
        self.grounds
            .iter()
            .position(|g| g.by == player && g.spec.ability == ability)
    }

    /// The warlock capstones that change a cast with the hero's state: Nightfall's primed bolt,
    /// Soul Harvest, Demonic Sacrifice's Fire.
    pub(super) fn warlock_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let fx = &self.heroes[h].spell.fx;
        if Some(owned.ability) == self.content.ability("shadow_bolt") {
            s.damage *= 1.0 + fx.boon(Augment::Nightfall);
        }
        s.damage *= 1.0 + fx.boon(Augment::SoulHarvest);
        if s.tags.has(Tag::Fire) && self.knows(h, Augment::DemonicSacrifice) {
            s.damage *= 1.25;
        }
    }

    /// How fast a warlock's spell comes back: Backdraft for Fire, Death's Embrace when low.
    pub(super) fn warlock_rate(&self, h: usize, owned: &Owned) -> f32 {
        let hero = &self.heroes[h];
        let mut rate = 1.0;
        if self.content.abilities[owned.ability].tag_set.has(Tag::Fire)
            && hero.spell.fx.boon(Augment::Backdraft) > 0.0
        {
            rate *= 1.3;
        }
        if owned.has(Augment::DeathsEmbrace) && hero.hp < hero.max_hp * 0.35 {
            rate *= 3.0;
        }
        rate
    }

    /// Before a warlock spell fires on its own cooldown: Soul Fire's every 5th Searing Pain.
    pub(super) fn warlock_before(&mut self, _h: usize, owned: &Owned, spec: &mut CastSpec) {
        if owned.has(Augment::SoulFire) && owned.casts % 5 == 4 {
            spec.damage *= 4.0;
            spec.stun = spec.stun.max(1.0);
            spec.tags = spec.tags.with(Tag::Stun);
        }
    }

    /// After a warlock spell fired: Nightfall's primed bolt is spent.
    pub(super) fn warlock_after(&mut self, h: usize, owned: &Owned, _spec: &CastSpec) {
        if Some(owned.ability) == self.content.ability("shadow_bolt") {
            self.heroes[h]
                .spell
                .fx
                .boons
                .retain(|b| b.key != Augment::Nightfall);
        }
    }

    /// Extra crit chance: Improved Searing Pain up close.
    pub(super) fn warlock_crit(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let close = self.enemies[ei].pos.distance(self.heroes[h].pos) <= 5.0;
        let player = self.heroes[h].player;
        if close && self.spell_has(player, spec.ability, Augment::ImprovedSearingPain) {
            0.5
        } else {
            0.0
        }
    }

    /// How much harder a hit lands on an enemy under Improved Shadow Bolt's stacks.
    pub(super) fn warlock_scale(&self, _h: usize, ei: usize, spec: &CastSpec) -> f32 {
        let st = &self.enemies[ei].status.fx;
        if spec.tags.has(Tag::Shadow) && st.shadow_vuln > 0 {
            1.0 + VULN_EACH * f32::from(st.shadow_vuln)
        } else {
            1.0
        }
    }

    /// What is left of a blow at a warlock: its Voidwalker and Hellfire Ward guard it.
    pub(super) fn warlock_guard(&self, h: usize) -> f32 {
        let mut m = 1.0;
        if self.has_ground(h, "summon_voidwalker").is_some() {
            m *= 0.8;
        }
        if let Some(g) = self.has_ground(h, "hellfire") {
            let g = &self.grounds[g];
            if self.spell_has(g.by, g.spec.ability, Augment::HellfireWard) {
                m *= 0.8;
            }
        }
        m
    }

    /// Where a warlock's ground stands: Firestorm Rain follows the thickest crowd.
    pub(super) fn warlock_ground_pos(&self, g: &Ground, pos: V2) -> V2 {
        if !self.spell_has(g.by, g.spec.ability, Augment::FirestormRain) {
            return pos;
        }
        let Some(h) = self.hero_ix(g.by) else {
            return pos;
        };
        self.densest(self.heroes[h].pos, g.spec.range, g.spec.radius)
            .unwrap_or(pos)
    }

    /// A hit of a warlock's spell `spec` on enemy `id` for `damage`: the capstones that act on a
    /// hit or a crit.
    pub(super) fn warlock_strike(
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
        let elite = self.enemies[ei].elite || self.enemies[ei].boss;
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        let mine = |b: &&mut super::combat::Burn| b.by == player && b.ability == spec.ability;
        for a in augments {
            match a {
                Augment::ImprovedShadowBolt => {
                    let st = &mut self.enemies[ei].status.fx;
                    st.shadow_vuln = (st.shadow_vuln + 1).min(VULN_MAX);
                    st.shadow_vuln_secs = VULN_SECS;
                }
                Augment::Backdraft if crit => {
                    self.heroes[h]
                        .spell
                        .fx
                        .add_boon(Augment::Backdraft, 1.0, 1.0, 4.0);
                }
                Augment::Emberstorm if elite => {
                    if let Some(b) = self.enemies[ei].status.burns.iter_mut().find(mine) {
                        b.dps *= 1.5;
                    }
                }
                Augment::ImprovedDrain if elite => {
                    if let Some(b) = self.enemies[ei].status.burns.iter_mut().find(mine) {
                        b.leech *= 2.0;
                    }
                }
                Augment::Pyroclasm if self.rng.f32() < 0.15 => {
                    let e = &mut self.enemies[ei];
                    if !e.boss {
                        e.status.stun = e.status.stun.max(control_secs(1.0, e.elite, false));
                    }
                }
                Augment::Horrify if scale > 0.99 => {
                    let near: Vec<usize> = (0..self.enemies.len())
                        .filter(|&i| i != ei && self.enemies[i].pos.distance(at) <= HORRIFY_REACH)
                        .collect();
                    for i in near {
                        self.afflict_fx(i, spec, pos);
                    }
                }
                Augment::DarkBargain => {
                    let heal = damage * spec.fx.leech;
                    for (t, hero) in self.heroes.iter_mut().enumerate() {
                        if t != h && hero.alive && hero.pos.distance(pos) <= BARGAIN_REACH {
                            hero.hp = (hero.hp + heal).min(hero.max_hp);
                        }
                    }
                }
                Augment::FelFlame => {
                    for b in &mut self.enemies[ei].status.burns {
                        if b.by == player {
                            b.left += 2.0;
                        }
                    }
                }
                _ => {}
            }
        }
        let bolt = self.content.ability("shadow_bolt");
        if Some(spec.ability) == bolt && self.knows(h, Augment::EverlastingAffliction) {
            self.cast_on(h, "corruption", id);
        }
    }

    /// A tick of a warlock's damage over time: Nightfall primes the bolt; Curse of Doom when the
    /// curse runs out.
    pub(super) fn warlock_tick(&mut self, h: usize, ability: usize, id: u32, _dps: f32, left: f32) {
        let player = self.heroes[h].player;
        if self.knows(h, Augment::Nightfall) && self.rng.f32() < 0.05 {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::Nightfall, 2.0, 2.0, 30.0);
        }
        let curse = self.content.ability("curse_of_agony");
        if left <= 0.0
            && Some(ability) == curse
            && self.spell_has(player, ability, Augment::CurseOfDoom)
        {
            let s = self.spell_spec(h, &self.owned_or_new(h, ability));
            let total = s.damage * s.burn * s.dot_mult * s.dot_secs * s.tick_rate;
            self.damage_enemy(player, ability, id, total * 2.0, true);
        }
    }

    /// Each step for a warlock: Infernal Pact's price, Consume Shadows, the Voidwalker's
    /// Sacrifice.
    pub(super) fn warlock_tend(&mut self, h: usize) {
        if let Some(g) = self.has_ground(h, "hellfire") {
            let g = &self.grounds[g];
            if self.spell_has(g.by, g.spec.ability, Augment::InfernalPact) {
                let hero = &mut self.heroes[h];
                hero.hp = (hero.hp - hero.max_hp * 0.01 * DT).max(1.0);
            }
        }
        let Some(void) = self.has_ground(h, "summon_voidwalker") else {
            return;
        };
        let (by, ability) = (self.grounds[void].by, self.grounds[void].spec.ability);
        if self.spell_has(by, ability, Augment::ConsumeShadows) {
            let hero = &mut self.heroes[h];
            hero.hp = (hero.hp + hero.max_hp * 0.01 * DT).min(hero.max_hp);
        }
        let hero = &self.heroes[h];
        if hero.hp < hero.max_hp * 0.3 && self.spell_has(by, ability, Augment::Sacrifice) {
            self.grounds.remove(void);
            let hero = &mut self.heroes[h];
            hero.spell.shield = hero.spell.shield.max(hero.max_hp * 0.4);
            hero.spell.shield_time = hero.spell.shield_time.max(15.0);
        }
    }

    /// What dying enemies set off for warlocks: Seed of Corruption, Soul Harvest, Infernal
    /// Pact's heal.
    pub(super) fn warlock_deaths(&mut self, deaths: &[Death]) {
        let (corruption, drain) = (
            self.content.ability("corruption"),
            self.content.ability("drain_life"),
        );
        for d in deaths {
            for b in &d.status.burns {
                let Some(h) = self.hero_ix(b.by) else {
                    continue;
                };
                if Some(b.ability) == corruption
                    && self.spell_has(b.by, b.ability, Augment::SeedOfCorruption)
                {
                    let s = self.spell_spec(h, &self.owned_or_new(h, b.ability));
                    let total = s.damage * s.burn * s.dot_mult * s.dot_secs * s.tick_rate;
                    let near: Vec<u32> = self
                        .enemies
                        .iter()
                        .filter(|e| e.hp > 0.0 && e.pos.distance(d.pos) <= SEED_REACH)
                        .map(|e| e.id)
                        .collect();
                    self.events.push(Event::Cast {
                        caster: b.by,
                        ability: b.ability,
                        visual: self.content.abilities[b.ability].visual,
                        targets: near.iter().map(|&id| Target::Enemy(id)).collect(),
                        dest: Some(d.pos),
                        cooldown: 0.0,
                    });
                    for id in near {
                        self.damage_enemy(b.by, b.ability, id, total, false);
                        self.strike(b.by, &s, id, 1.0, d.pos, Crit::Never);
                    }
                }
                if Some(b.ability) == drain && self.spell_has(b.by, b.ability, Augment::SoulHarvest)
                {
                    self.heroes[h]
                        .spell
                        .fx
                        .add_boon(Augment::SoulHarvest, 0.05, 0.25, 20.0);
                }
            }
            let Some(h) = d.killer.and_then(|k| self.hero_ix(k)) else {
                continue;
            };
            if let Some(g) = self.has_ground(h, "hellfire") {
                let g = &self.grounds[g];
                let inside = g.pos.distance(d.pos) <= g.spec.radius;
                if inside && self.spell_has(g.by, g.spec.ability, Augment::InfernalPact) {
                    let hero = &mut self.heroes[h];
                    hero.hp = (hero.hp + hero.max_hp * 0.03).min(hero.max_hp);
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

    fn total_dot(w: &World, key: &str) -> f32 {
        let s = w.spec_of(0, key).unwrap();
        s.damage * s.burn * s.dot_mult * s.dot_secs * s.tick_rate
    }

    #[test]
    fn soulstone_turns_a_killing_blow_aside() {
        let mut w = hero_of("warlock");
        spells(&mut w, &[]);
        press_defense(&mut w);
        w.heroes[0].max_hp = 100.0;
        w.heroes[0].hp = 5.0;
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.hurt_hero_from(0, 50.0, Some(id));
        assert!(w.heroes[0].alive);
        assert!((w.heroes[0].hp - 40.0).abs() < 1e-3);
        assert!(w.enemy(id).unwrap().status.fx.fear > 0.0);
    }

    #[test]
    fn the_imp_firebolts_and_the_voidwalker_guards() {
        let mut w = hero_of("warlock");
        spells(&mut w, &[("summon_imp", 1, &[])]);
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 2.5);
        assert!(w.enemy(id).unwrap().hp < 1e6, "the imp did nothing");

        let mut w = hero_of("warlock");
        spells(&mut w, &[("summon_voidwalker", 1, &[])]);
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 1.5);
        assert!(w.enemy(id).unwrap().hp < 1e6);
        assert!((w.warlock_guard(0) - 0.8).abs() < 1e-4);
    }

    #[test]
    fn drain_life_and_death_coil_heal_and_howl_fears() {
        let mut w = hero_of("warlock");
        spells(&mut w, &[("drain_life", 1, &[])]);
        w.heroes[0].hp = 100.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 2.5);
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("warlock");
        spells(&mut w, &[("death_coil", 1, &[])]);
        w.heroes[0].hp = 100.0;
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.heroes[0].hp > 100.0);
        assert!(w.enemy(id).unwrap().status.fx.fear > 0.0);

        let mut w = hero_of("warlock");
        spells(&mut w, &[("howl_of_terror", 1, &[])]);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.fear > 3.0);
    }

    #[test]
    fn shadow_bolt_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[
                ("shadow_bolt", 10, &[Augment::Precise, Augment::Nightfall]),
                ("corruption", 1, &[]),
            ],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        foe(&mut w, 5.0, 0.0, 1e9);
        let mut primed = false;
        for _ in 0..80 {
            ready(&mut w, 1);
            run(&mut w, 1.0);
            if w.heroes[0].spell.fx.boon(Augment::Nightfall) > 0.0 {
                primed = true;
                break;
            }
        }
        assert!(primed);
        let plain = w.spec_of(0, "shadow_bolt").unwrap().damage;
        assert!(plain > 0.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "shadow_bolt",
                10,
                &[Augment::Precise, Augment::ShadowBoltVolley],
            )],
        );
        let ids: Vec<u32> = (0..5)
            .map(|i| foe(&mut w, 4.0 + i as f32, 0.0, 1e6))
            .collect();
        run(&mut w, 1.0);
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "shadow_bolt",
                10,
                &[Augment::Precise, Augment::ImprovedShadowBolt],
            )],
        );
        let id = foe(&mut w, 4.0, 0.0, 1e6);
        for _ in 0..7 {
            ready(&mut w, 0);
            run(&mut w, 0.3);
        }
        assert_eq!(w.enemy(id).unwrap().status.fx.shadow_vuln, 5);
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let bolt = w.spec_of(0, "shadow_bolt").unwrap();
        assert!((w.warlock_scale(0, ei, &bolt) - 1.2).abs() < 1e-4);
    }

    #[test]
    fn corruption_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("corruption", 10, &[Augment::Quickened, Augment::SiphonLife])],
        );
        w.heroes[0].hp = 100.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 2.0);
        assert!(w.heroes[0].hp > 100.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "corruption",
                10,
                &[Augment::Quickened, Augment::SeedOfCorruption],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let near = foe(&mut w, 30.0, 0.0, 1e6);
        let at = w.enemies[0].pos;
        w.enemies[1].pos = at + V2::new(3.0, 0.0);
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        let e = w.enemy(near).unwrap();
        assert!(e.hp < 1e6 && e.status.burning(), "no seed burst");

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[
                (
                    "corruption",
                    10,
                    &[Augment::Quickened, Augment::EverlastingAffliction],
                ),
                ("shadow_bolt", 1, &[]),
            ],
        );
        w.heroes[0].abilities[0].cd = 99.0;
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(id).unwrap().status.burning());
    }

    #[test]
    fn curse_of_agony_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "curse_of_agony",
                10,
                &[Augment::Quickened, Augment::CurseOfDoom],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e5);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let doom = total_dot(&w, "curse_of_agony") * 2.0;
        run(&mut w, 16.0);
        let lost = 1e5 - w.enemy(id).unwrap().hp;
        assert!(lost > doom, "no doom: {lost} vs {doom}");

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "curse_of_agony",
                10,
                &[Augment::Quickened, Augment::AmplifyCurse],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.weaken > 10.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "curse_of_agony",
                10,
                &[Augment::Quickened, Augment::CurseOfExhaustion],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.chill > 10.0);
    }

    #[test]
    fn immolate_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "immolate",
                10,
                &[Augment::Precise, Augment::FireAndBrimstone],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 7.0, 2.0, 1e6);
        w.enemies[0].elite = true;
        w.enemies.swap(0, 1);
        w.heroes[0].abilities[0].cd = 0.0;
        w.step();
        let e = w.enemy(near).unwrap();
        assert!(e.hp < 1e6 && e.status.burning());

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("immolate", 10, &[Augment::Precise, Augment::Backdraft])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        let owned = w.heroes[0].abilities[0].clone();
        assert!((w.warlock_rate(0, &owned) - 1.3).abs() < 1e-4);

        let dps = |elite: bool| {
            let mut w = hero_of("warlock");
            spells(
                &mut w,
                &[("immolate", 10, &[Augment::Precise, Augment::Emberstorm])],
            );
            w.heroes[0].stats.add(Stat::Crit, -1.0);
            let id = foe(&mut w, 5.0, 0.0, 1e6);
            w.enemies[0].elite = elite;
            w.step();
            w.enemy(id).unwrap().status.burns[0].dps
        };
        assert!((dps(true) / dps(false) - 1.5).abs() < 1e-3);
    }

    #[test]
    fn rain_of_fire_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("rain_of_fire", 10, &[Augment::Wide, Augment::FirestormRain])],
        );
        foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        // The crowd moves; the rain follows it.
        let moved = foe(&mut w, -12.0, 0.0, 1e6);
        w.enemies.retain(|e| e.id == moved);
        run(&mut w, 1.5);
        assert!(w.enemy(moved).unwrap().hp < 1e6, "the rain did not follow");

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("rain_of_fire", 10, &[Augment::Wide, Augment::Aftermath])],
        );
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.chill > 0.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("rain_of_fire", 10, &[Augment::Wide, Augment::Pyroclasm])],
        );
        for i in 0..10 {
            foe(&mut w, 10.0, i as f32 * 0.4, 1e6);
        }
        let mut stunned = false;
        for _ in 0..(5.0 / DT) as usize {
            w.step();
            stunned |= w.enemies.iter().any(|e| e.status.stun > 0.0);
        }
        assert!(stunned, "no Pyroclasm stun in 50 ticks");
    }

    #[test]
    fn hellfire_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("hellfire", 10, &[Augment::Wide, Augment::InfernalPact])],
        );
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 1000.0;
        foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 2.0);
        assert!(w.heroes[0].hp < 1000.0, "the pact took nothing");
        let hp = w.heroes[0].hp;
        foe(&mut w, 2.0, 1.0, 0.5);
        run(&mut w, 1.2);
        assert!(w.heroes[0].hp > hp, "a kill inside did not heal");

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("hellfire", 10, &[Augment::Wide, Augment::RitualOfFire])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.burning());

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("hellfire", 10, &[Augment::Wide, Augment::HellfireWard])],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!((w.warlock_guard(0) - 0.8).abs() < 1e-4);
    }

    #[test]
    fn death_coil_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("death_coil", 10, &[Augment::Precise, Augment::Horrify])],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 6.0, 2.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(near).unwrap().status.fx.fear > 0.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "death_coil",
                10,
                &[Augment::Precise, Augment::DeathsEmbrace],
            )],
        );
        w.heroes[0].hp = w.heroes[0].max_hp * 0.2;
        let owned = w.heroes[0].abilities[0].clone();
        assert_eq!(w.warlock_rate(0, &owned), 3.0);

        let mut w = hero_of("warlock");
        w.add_hero(1, "warrior").unwrap();
        w.heroes[1].pos = w.heroes[0].pos + V2::new(5.0, 0.0);
        spells(
            &mut w,
            &[("death_coil", 10, &[Augment::Precise, Augment::DarkBargain])],
        );
        w.heroes[1].hp = 50.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.heroes[1].hp > 50.0);
    }

    #[test]
    fn drain_life_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "drain_life",
                10,
                &[Augment::Quickened, Augment::SoulHarvest],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        assert!(w.heroes[0].spell.fx.boon(Augment::SoulHarvest) > 0.04);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("drain_life", 10, &[Augment::Quickened, Augment::DrainAura])],
        );
        let ids: Vec<u32> = (0..4).map(|i| foe(&mut w, 2.0, i as f32, 1e6)).collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().status.burning()));

        let leech = |elite: bool| {
            let mut w = hero_of("warlock");
            spells(
                &mut w,
                &[(
                    "drain_life",
                    10,
                    &[Augment::Quickened, Augment::ImprovedDrain],
                )],
            );
            let id = foe(&mut w, 5.0, 0.0, 1e6);
            w.enemies[0].elite = elite;
            w.step();
            w.enemy(id).unwrap().status.burns[0].leech
        };
        assert!((leech(true) / leech(false) - 2.0).abs() < 1e-3);
    }

    #[test]
    fn howl_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("howl_of_terror", 10, &[Augment::Wide, Augment::Nightmare])],
        );
        let id = foe(&mut w, 4.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.burning());

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("howl_of_terror", 10, &[Augment::Wide, Augment::Shadowfury])],
        );
        let id = foe(&mut w, 4.0, 0.0, 1e6);
        w.step();
        let e = w.enemy(id).unwrap();
        assert!(e.status.stun > 1.5 && e.status.fx.fear == 0.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "howl_of_terror",
                10,
                &[Augment::Wide, Augment::DemonicPresence],
            )],
        );
        let id = foe(&mut w, 4.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.fx.mark >= 0.15);
    }

    #[test]
    fn imp_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("summon_imp", 10, &[Augment::Frenzy, Augment::BurningImp])],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        run(&mut w, 1.5);
        assert!(w.enemy(id).unwrap().status.burning());

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[
                (
                    "summon_imp",
                    10,
                    &[Augment::Frenzy, Augment::DemonicSacrifice],
                ),
                ("searing_pain", 1, &[]),
            ],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.heroes[0].abilities[1].cd = 99.0;
        run(&mut w, 1.5);
        assert_eq!(w.enemy(id).unwrap().hp, 1e6, "the imp came anyway");
        assert!(w.grounds.is_empty());
        let mut plain = hero_of("warlock");
        spells(&mut plain, &[("searing_pain", 1, &[])]);
        let base = plain.spec_of(0, "searing_pain").unwrap().damage;
        assert!((w.spec_of(0, "searing_pain").unwrap().damage / base - 1.25).abs() < 1e-4);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("summon_imp", 10, &[Augment::Frenzy, Augment::ImprovedImp])],
        );
        let a = foe(&mut w, 8.0, 0.0, 1e6);
        let b = foe(&mut w, 9.0, 0.0, 1e6);
        run(&mut w, 1.5);
        assert!(w.enemy(a).unwrap().hp < 1e6 && w.enemy(b).unwrap().hp < 1e6);
    }

    #[test]
    fn voidwalker_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "summon_voidwalker",
                10,
                &[Augment::Frenzy, Augment::Sacrifice],
            )],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.2;
        w.step();
        assert!(w.heroes[0].spell.shield > 0.0, "no sacrifice");
        assert!(w.grounds.is_empty());

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "summon_voidwalker",
                10,
                &[Augment::Frenzy, Augment::Torment],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.5);
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "summon_voidwalker",
                10,
                &[Augment::Frenzy, Augment::ConsumeShadows],
            )],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        w.heroes[0].hp = 100.0;
        run(&mut w, 1.0);
        assert!(w.heroes[0].hp > 100.0);
    }

    #[test]
    fn searing_pain_capstones() {
        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[(
                "searing_pain",
                10,
                &[Augment::Precise, Augment::ImprovedSearingPain],
            )],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "searing_pain").unwrap();
        assert_eq!(w.warlock_crit(0, ei, &s), 0.5);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[("searing_pain", 10, &[Augment::Precise, Augment::SoulFire])],
        );
        w.heroes[0].abilities[0].casts = 4;
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.stun > 0.0);

        let mut w = hero_of("warlock");
        spells(
            &mut w,
            &[
                ("immolate", 1, &[]),
                ("searing_pain", 10, &[Augment::Precise, Augment::FelFlame]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let before = w.enemy(id).unwrap().status.burns[0].left;
        ready(&mut w, 1);
        w.step();
        assert!(w.enemy(id).unwrap().status.burns[0].left > before + 1.5);
    }

    #[test]
    fn every_warlock_spell_has_three_capstones() {
        let w = hero_of("warlock");
        let class = &w.content.classes[w.content.class("warlock").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
