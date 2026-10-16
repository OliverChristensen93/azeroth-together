//! The shaman's capstones (docs/SHAMAN-DRAFT.md): the mechanics of its 36 rank-10 augments,
//! called from the hooks in `classes.rs`. Capstones that only change a cast's numbers (Storm
//! Nexus, Frozen Power, Earthquake Totem, Tidal Focus and the rest) are in `Augment::shape`.

use super::combat::{Crit, Death, Ground};
use super::*;
use crate::augments::Augment;
use crate::spell::{control_secs, BuffFx, Tag};

/// Earthen Grip's and Frost Nova Shock's reach around the target.
const GRIP_REACH: f32 = 6.0;
const NOVA_SHOCK_REACH: f32 = 5.0;
/// Searing Pact's reach from the totem.
const PACT_REACH: f32 = 8.0;
/// Fire Nova's and Cloudburst's, Thunderstorm's and Liquid Magma's reach.
const FIRE_NOVA_REACH: f32 = 8.0;
const CLOUDBURST_REACH: f32 = 10.0;
const THUNDERSTORM_REACH: f32 = 8.0;
const MAGMA_REACH: f32 = 20.0;
/// Volcanic Fury's eruption: reach and seconds.
const ERUPTION_REACH: f32 = 5.0;
const ERUPTION_SECS: f32 = 4.0;

impl World {
    /// Hero `h`'s grounds of spell `key`.
    fn grounds_of(&self, h: usize, key: &str) -> Vec<usize> {
        let Some(ability) = self.content.ability(key) else {
            return Vec::new();
        };
        let player = self.heroes[h].player;
        (0..self.grounds.len())
            .filter(|&i| self.grounds[i].by == player && self.grounds[i].spec.ability == ability)
            .collect()
    }

    /// The shaman capstones that change a cast with the hero's state: Searing Pact, Static
    /// Charge, Maelstrom Weapon, Fulmination's orbs.
    pub(super) fn shaman_spec(&self, h: usize, owned: &Owned, s: &mut CastSpec) {
        let hero = &self.heroes[h];
        let fx = &hero.spell.fx;
        if s.tags.has(Tag::Fire) {
            let pact = self.grounds_of(h, "searing_totem").into_iter().any(|g| {
                let g = &self.grounds[g];
                g.pos.distance(hero.pos) <= PACT_REACH
                    && self.spell_has(g.by, g.spec.ability, Augment::SearingPact)
            });
            if pact {
                s.damage *= 1.2;
            }
        }
        if s.tags.has(Tag::Nature) {
            s.damage *= 1.0 + fx.boon(Augment::StaticCharge);
        }
        let bolts = [
            self.content.ability("lightning_bolt"),
            self.content.ability("chain_lightning"),
        ];
        if bolts.contains(&Some(owned.ability)) {
            s.damage *= 1.0 + fx.boon(Augment::MaelstromWeapon);
        }
        if owned.has(Augment::Fulmination) {
            if let Some(shield) = self.content.ability("lightning_shield") {
                s.damage *= 1.0 + 0.6 * f32::from(fx.charges(shield));
            }
        }
    }

    /// Before a shaman spell fires on its own cooldown: Stormkeeper's every 5th bolt.
    pub(super) fn shaman_before(&mut self, _h: usize, owned: &Owned, spec: &mut CastSpec) {
        if owned.has(Augment::Stormkeeper) && owned.casts % 5 == 4 {
            spec.fx.jumps = spec.fx.jumps.max(4);
        }
    }

    /// After a shaman spell fired: Fulmination spends the orbs, Shock Mastery readies the
    /// shocks, Maelstrom Weapon's charge is spent or built.
    pub(super) fn shaman_after(&mut self, h: usize, owned: &Owned, _spec: &CastSpec) {
        if owned.has(Augment::Fulmination) {
            if let Some(shield) = self.content.ability("lightning_shield") {
                if let Some(b) = self.heroes[h]
                    .spell
                    .fx
                    .banks
                    .iter_mut()
                    .find(|b| b.ability == shield)
                {
                    b.charges = 0;
                }
            }
        }
        if owned.has(Augment::ShockMastery) {
            let shocks = [
                self.content.ability("flame_shock"),
                self.content.ability("frost_shock"),
            ];
            for o in &mut self.heroes[h].abilities {
                if shocks.contains(&Some(o.ability)) {
                    o.cd = 0.0;
                }
            }
        }
        let bolts = [
            self.content.ability("lightning_bolt"),
            self.content.ability("chain_lightning"),
        ];
        if bolts.contains(&Some(owned.ability)) {
            self.heroes[h]
                .spell
                .fx
                .boons
                .retain(|b| b.key != Augment::MaelstromWeapon);
        }
        if owned.has(Augment::MaelstromWeapon) && owned.casts % 5 == 4 {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::MaelstromWeapon, 1.0, 1.0, 30.0);
        }
    }

    /// Extra crit chance: Lava Burst on Flame-Shocked enemies.
    pub(super) fn shaman_crit(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        if Some(spec.ability) != self.content.ability("lightning_bolt")
            || !self.knows(h, Augment::LavaBurst)
        {
            return 0.0;
        }
        let player = self.heroes[h].player;
        let shock = self.content.ability("flame_shock");
        let shocked = self.enemies[ei]
            .status
            .burns
            .iter()
            .any(|b| b.by == player && Some(b.ability) == shock);
        if shocked {
            1.0
        } else {
            0.0
        }
    }

    /// How much harder a shaman's hit lands: Frozen Power on rooted enemies.
    pub(super) fn shaman_scale(&self, h: usize, ei: usize, spec: &CastSpec) -> f32 {
        if spec.tags.has(Tag::Nature)
            && self.enemies[ei].status.fx.root > 0.0
            && self.knows(h, Augment::FrozenPower)
        {
            1.3
        } else {
            1.0
        }
    }

    /// What is left of a blow at a hero in the reach of a Stoneskin totem.
    pub(super) fn shaman_guard(&self, h: usize) -> f32 {
        let pos = self.heroes[h].pos;
        let sheltered = self.grounds.iter().any(|g| {
            g.pos.distance(pos) <= g.spec.radius
                && self.spell_has(g.by, g.spec.ability, Augment::Stoneskin)
        });
        if sheltered {
            0.85
        } else {
            1.0
        }
    }

    /// How much more a shaman's healing ground of `spec` heals hero `t`: Healing Tide.
    pub(super) fn shaman_heal_bonus(&self, h: usize, spec: &CastSpec, t: usize) -> f32 {
        let player = self.heroes[h].player;
        let low = self.heroes[t].hp < self.heroes[t].max_hp * 0.3;
        if low && self.spell_has(player, spec.ability, Augment::HealingTide) {
            2.0
        } else {
            1.0
        }
    }

    /// A shaman's healing ground of `spec` healed `healed`: Cloudburst keeps count.
    pub(super) fn shaman_ground_healed(&mut self, h: usize, spec: &CastSpec, healed: f32) {
        let player = self.heroes[h].player;
        if healed > 0.0 && self.spell_has(player, spec.ability, Augment::Cloudburst) {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::Cloudburst, healed, f32::MAX, 60.0);
        }
    }

    /// Strike every enemy within `reach` of `pos` for `damage`, as `ability` of `by`.
    fn burst(&mut self, by: PlayerId, ability: usize, pos: V2, reach: f32, damage: f32) {
        let hit: Vec<u32> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= reach)
            .map(|e| e.id)
            .collect();
        self.events.push(Event::Cast {
            caster: by,
            ability,
            visual: self.content.abilities[ability].visual,
            targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
            dest: Some(pos),
            cooldown: 0.0,
        });
        for id in hit {
            self.damage_enemy(by, ability, id, damage, false);
        }
    }

    /// A shaman's ground `g` ran out at `pos`: Fire Nova, Cloudburst.
    pub(super) fn shaman_ground_ended(&mut self, g: &Ground, pos: V2) {
        if self.spell_has(g.by, g.spec.ability, Augment::FireNova) {
            self.burst(
                g.by,
                g.spec.ability,
                pos,
                FIRE_NOVA_REACH,
                g.spec.damage * 3.0,
            );
        }
        if self.spell_has(g.by, g.spec.ability, Augment::Cloudburst) {
            let Some(h) = self.hero_ix(g.by) else {
                return;
            };
            let stored = self.heroes[h].spell.fx.boon(Augment::Cloudburst);
            self.heroes[h]
                .spell
                .fx
                .boons
                .retain(|b| b.key != Augment::Cloudburst);
            if stored > 0.0 {
                self.burst(g.by, g.spec.ability, pos, CLOUDBURST_REACH, stored);
            }
        }
    }

    /// Hero `h` spent a charge of its Ward `ability` (on hero `t` for a heal): Thunderstorm,
    /// Static Charge, Ancestral Fortitude.
    pub(super) fn shaman_charge_spent(&mut self, h: usize, ability: usize, t: Option<usize>) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        if self.spell_has(player, ability, Augment::StaticCharge) {
            self.heroes[h]
                .spell
                .fx
                .add_boon(Augment::StaticCharge, 0.05, 0.15, 6.0);
        }
        if self.heroes[h].spell.fx.charges(ability) == 0
            && self.spell_has(player, ability, Augment::Thunderstorm)
        {
            let spec = self.spell_spec(h, &self.owned_or_new(h, ability));
            let hit: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(pos) <= THUNDERSTORM_REACH)
                .map(|e| e.id)
                .collect();
            let mut storm = spec.clone();
            storm.knockback = storm.knockback.max(4.0);
            self.events.push(Event::Cast {
                caster: player,
                ability,
                visual: self.content.abilities[ability].visual,
                targets: hit.iter().map(|&id| Target::Enemy(id)).collect(),
                dest: Some(pos),
                cooldown: 0.0,
            });
            for id in hit {
                self.strike(player, &storm, id, 2.0, pos, Crit::Roll);
            }
        }
        if let Some(t) = t {
            if self.spell_has(player, ability, Augment::AncestralFortitude) {
                let fx = BuffFx {
                    secs: 6.0,
                    guard: 0.2,
                    ..BuffFx::default()
                };
                self.grant(t, player, ability, fx);
            }
        }
    }

    /// A hit of a shaman's spell `spec` on enemy `id`: the capstones that act on a hit or a crit.
    pub(super) fn shaman_strike(
        &mut self,
        h: usize,
        id: u32,
        spec: &CastSpec,
        _damage: f32,
        crit: bool,
        scale: f32,
    ) {
        let (player, pos) = (self.heroes[h].player, self.heroes[h].pos);
        let Some(ei) = self.enemies.iter().position(|e| e.id == id) else {
            return;
        };
        let at = self.enemies[ei].pos;
        let primary = scale > 0.99;
        let augments: Vec<Augment> = self.heroes[h]
            .abilities
            .iter()
            .find(|o| o.ability == spec.ability)
            .map(|o| o.augments.clone())
            .unwrap_or_default();
        let shield = self.content.ability("lightning_shield");
        for a in augments {
            match a {
                Augment::LightningOverload if primary && self.rng.f32() < 0.2 => {
                    self.strike(player, spec, id, 0.5, pos, Crit::Roll);
                }
                Augment::RollingThunder if crit => {
                    if let Some(shield) = shield {
                        self.add_charge(h, shield);
                    }
                }
                Augment::EarthenGrip if primary => {
                    for e in self
                        .enemies
                        .iter_mut()
                        .filter(|e| e.pos.distance(at) <= GRIP_REACH)
                    {
                        e.status.fx.root = e.status.fx.root.max(control_secs(2.0, e.elite, e.boss));
                    }
                }
                Augment::FrostNovaShock if primary => {
                    for e in self
                        .enemies
                        .iter_mut()
                        .filter(|e| e.id != id && e.pos.distance(at) <= NOVA_SHOCK_REACH)
                    {
                        e.status.chill = e.status.chill.max(spec.chill.max(1.0));
                        e.status.slow = e.status.slow.max(spec.slow);
                    }
                }
                Augment::Icefury if crit => {
                    let e = &mut self.enemies[ei];
                    e.status.freeze = e.status.freeze.max(control_secs(2.0, e.elite, e.boss));
                }
                Augment::Earthgrab if !self.enemies[ei].status.fx.grabbed => {
                    let e = &mut self.enemies[ei];
                    e.status.fx.grabbed = true;
                    e.status.fx.root = e.status.fx.root.max(control_secs(2.0, e.elite, e.boss));
                }
                Augment::Windfury if primary && self.rng.f32() < 0.2 => {
                    for _ in 0..2 {
                        self.strike(player, spec, id, 0.99, pos, Crit::Roll);
                        if let Some(shield) = shield {
                            self.add_charge(h, shield);
                        }
                    }
                }
                _ => {}
            }
        }
        let totem = self.content.ability("searing_totem");
        if Some(spec.ability) == totem && self.knows(h, Augment::SearingFlames) {
            let shock = self.content.ability("flame_shock");
            let full = self
                .spec_of(h, "flame_shock")
                .map(|s| s.fx.dot_time * s.dot_secs / s.dot_speed.max(0.1))
                .unwrap_or(0.0);
            if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                for b in &mut e.status.burns {
                    if b.by == player && Some(b.ability) == shock {
                        b.left = b.left.max(full);
                    }
                }
            }
        }
    }

    /// Each step for a shaman: Liquid Magma's globs, Nature's Swiftness at low health.
    pub(super) fn shaman_tend(&mut self, h: usize) {
        let player = self.heroes[h].player;
        for g in self.grounds_of(h, "magma_totem") {
            let (ability, at) = (self.grounds[g].spec.ability, self.grounds[g].pos);
            if !self.spell_has(player, ability, Augment::LiquidMagma)
                || !self.heroes[h].spell.ready(Augment::LiquidMagma)
            {
                continue;
            }
            self.heroes[h].spell.spend(Augment::LiquidMagma, 1.0);
            let spec = self.grounds[g].spec.clone();
            let near: Vec<u32> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0.0 && e.pos.distance(at) <= MAGMA_REACH)
                .map(|e| e.id)
                .collect();
            for _ in 0..3.min(near.len()) {
                let id = near[self.rng.index(near.len())];
                self.strike(player, &spec, id, 1.0, at, Crit::Roll);
            }
        }
        let hero = &self.heroes[h];
        if hero.hp >= hero.max_hp * 0.25
            || !self.knows(h, Augment::NaturesSwiftness)
            || !hero.spell.ready(Augment::NaturesSwiftness)
        {
            return;
        }
        let Some(wave) = self.content.ability("healing_wave") else {
            return;
        };
        self.heroes[h].spell.spend(Augment::NaturesSwiftness, 30.0);
        let spec = self.spell_spec(h, &self.owned_or_new(h, wave));
        while self.heroes[h].spell.fx.charges(wave) > 0 {
            self.spend_charge(h, wave);
            let amount = spec.fx.heal * self.heroes[h].max_hp;
            self.heal_hero(h, amount, Some(&spec));
        }
        let fx = BuffFx {
            secs: 4.0,
            cd_rate: 1.0,
            ..BuffFx::default()
        };
        self.grant(h, player, wave, fx);
    }

    /// What dying enemies set off for shamans: Chain Reaction, Volcanic Fury.
    pub(super) fn shaman_deaths(&mut self, deaths: &[Death]) {
        let (chain, shock) = (
            self.content.ability("chain_lightning"),
            self.content.ability("flame_shock"),
        );
        for d in deaths {
            if let (Some(by), Some(chain)) = (d.killer, chain) {
                if d.status.last_ability == Some(chain)
                    && self.spell_has(by, chain, Augment::ChainReaction)
                {
                    if let Some(h) = self.hero_ix(by) {
                        if self.heroes[h].spell.ready(Augment::ChainReaction) {
                            self.heroes[h].spell.spend(Augment::ChainReaction, 4.0);
                            if let Some(o) = self.heroes[h]
                                .abilities
                                .iter_mut()
                                .find(|o| o.ability == chain)
                            {
                                o.cd = 0.0;
                            }
                        }
                    }
                }
            }
            let erupt: Vec<PlayerId> = d
                .status
                .burns
                .iter()
                .filter(|b| Some(b.ability) == shock)
                .filter(|b| self.spell_has(b.by, b.ability, Augment::VolcanicFury))
                .map(|b| b.by)
                .collect();
            if let (Some(&by), Some(shock)) = (erupt.first(), shock) {
                if let Some(h) = self.hero_ix(by) {
                    let mut fire = self.spell_spec(h, &self.owned_or_new(h, shock));
                    fire.radius = ERUPTION_REACH;
                    fire.damage *= 0.3;
                    self.add_ground(by, &fire, d.pos, ERUPTION_SECS);
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

    fn walk(w: &mut World) {
        w.set_input(
            0,
            PlayerInput {
                move_dir: V2::new(0.0, 1.0),
                ..Default::default()
            },
        );
    }

    #[test]
    fn ghost_wolf_runs_and_totems_stand_where_they_are_put() {
        let mut w = hero_of("shaman");
        spells(&mut w, &[]);
        let speed = w.heroes[0].speed;
        press_defense(&mut w);
        assert!(w.heroes[0].speed > speed * 1.55);

        let mut w = hero_of("shaman");
        spells(&mut w, &[("searing_totem", 1, &[])]);
        let start = w.heroes[0].pos;
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        w.step();
        assert!(w.grounds[0].pos.distance(start) < 0.1);
        walk(&mut w);
        run(&mut w, 2.0);
        assert!(w.grounds[0].pos.distance(start) < 0.1, "the totem walked");
        assert!(w.enemy(id).unwrap().hp < 1e6);
    }

    #[test]
    fn chain_lightning_jumps_and_lightning_shield_zaps_attackers() {
        let mut w = hero_of("shaman");
        spells(&mut w, &[("chain_lightning", 1, &[])]);
        let ids: Vec<u32> = (0..4)
            .map(|i| foe(&mut w, 5.0 + 4.0 * i as f32, 0.0, 1e6))
            .collect();
        w.step();
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("shaman");
        spells(&mut w, &[("lightning_shield", 1, &[])]);
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        let took = w.hurt_hero_from(0, 10.0, Some(id));
        assert!(took > 0.0, "thorns do not block");
        assert!(w.enemy(id).unwrap().hp < 1e6);
    }

    #[test]
    fn healing_stream_heals_or_strikes_and_healing_wave_banks() {
        let mut w = hero_of("shaman");
        spells(&mut w, &[("healing_stream_totem", 1, &[])]);
        let id = foe(&mut w, 10.0, 0.0, 1e6);
        w.heroes[0].hp = 100.0;
        run(&mut w, 1.2);
        assert!(w.heroes[0].hp > 100.0);
        assert_eq!(
            w.enemy(id).unwrap().hp,
            1e6,
            "it struck while someone was hurt"
        );
        w.heroes[0].hp = w.heroes[0].max_hp;
        run(&mut w, 1.2);
        assert!(w.enemy(id).unwrap().hp < 1e6, "no overflow strike");

        let mut w = hero_of("shaman");
        spells(&mut w, &[("healing_wave", 1, &[])]);
        w.step();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.3;
        w.step();
        assert!(w.heroes[0].hp > w.heroes[0].max_hp * 0.55);
    }

    #[test]
    fn lightning_bolt_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "lightning_bolt",
                10,
                &[Augment::Precise, Augment::LightningOverload],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e7);
        let mut hits = 0;
        for _ in 0..40 {
            ready(&mut w, 0);
            let events = run(&mut w, 0.5);
            hits += events
                .iter()
                .filter(|e| matches!(e, Event::Damage { target: Target::Enemy(t), .. } if *t == id))
                .count();
        }
        assert!(hits > 40, "never overloaded: {hits}");

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "lightning_bolt",
                10,
                &[Augment::Precise, Augment::Stormkeeper],
            )],
        );
        w.heroes[0].abilities[0].casts = 4;
        let ids: Vec<u32> = (0..5)
            .map(|i| foe(&mut w, 5.0 + 4.0 * i as f32, 0.0, 1e6))
            .collect();
        run(&mut w, 1.0);
        assert!(ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6));

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "lightning_bolt",
                10,
                &[Augment::Precise, Augment::RollingThunder],
            )],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.5);
        let shield = ability(&w, "lightning_shield");
        assert_eq!(w.heroes[0].spell.fx.charges(shield), 1);
    }

    #[test]
    fn chain_lightning_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "chain_lightning",
                10,
                &[Augment::Forked, Augment::ChainReaction],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1.0);
        run(&mut w, DT * 2.0);
        // The kill made it ready, and its internal cooldown started.
        assert!(!w.heroes[0].spell.ready(Augment::ChainReaction));

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "chain_lightning",
                10,
                &[Augment::Forked, Augment::StormNexus],
            )],
        );
        let ids: Vec<u32> = (0..12)
            .map(|i| foe(&mut w, 5.0 + 2.5 * i as f32, 0.0, 1e6))
            .collect();
        w.step();
        assert!(
            ids.iter().all(|&id| w.enemy(id).unwrap().hp < 1e6),
            "the nexus broke"
        );

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "chain_lightning",
                10,
                &[Augment::Forked, Augment::Electrified],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.chill > 0.0);
    }

    #[test]
    fn earth_shock_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[
                ("lightning_shield", 1, &[]),
                ("earth_shock", 10, &[Augment::Precise, Augment::Fulmination]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        for _ in 0..3 {
            ready(&mut w, 0);
            w.step();
        }
        w.heroes[0].abilities[0].cd = 99.0;
        let plain = {
            let mut s = w.spec_of(0, "earth_shock").unwrap();
            s.damage /= 1.0 + 0.6 * 3.0;
            s.damage
        };
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.heroes[0].stats.add(Stat::Crit, -1.0);
        ready(&mut w, 1);
        w.step();
        assert!((1e6 - w.enemy(id).unwrap().hp - plain * 2.8).abs() < 0.5);
        let shield = ability(&w, "lightning_shield");
        assert_eq!(w.heroes[0].spell.fx.charges(shield), 0);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("earth_shock", 10, &[Augment::Precise, Augment::EarthenGrip])],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 7.0, 3.0, 1e6);
        w.step();
        assert!(w.enemy(near).unwrap().status.fx.root > 0.0);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[
                (
                    "earth_shock",
                    10,
                    &[Augment::Precise, Augment::ShockMastery],
                ),
                ("flame_shock", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 50.0;
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        // Flame Shock was made ready, and fired at once.
        assert!(w.heroes[0].abilities[1].cd < 49.0);
    }

    #[test]
    fn flame_shock_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[
                ("flame_shock", 10, &[Augment::Precise, Augment::LavaBurst]),
                ("lightning_bolt", 1, &[]),
            ],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let bolt = w.spec_of(0, "lightning_bolt").unwrap();
        assert_eq!(w.shaman_crit(0, ei, &bolt), 1.0);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "flame_shock",
                10,
                &[Augment::Precise, Augment::VolcanicFury],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.enemies[0].hp = -1.0;
        run(&mut w, DT * 2.0);
        assert_eq!(w.grounds.len(), 1, "no eruption");

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[
                (
                    "flame_shock",
                    10,
                    &[Augment::Precise, Augment::SearingFlames],
                ),
                ("searing_totem", 1, &[]),
            ],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        run(&mut w, 8.0);
        assert!(
            w.enemy(id).unwrap().status.burns[0].left > 9.0,
            "the totem did not renew it"
        );
    }

    #[test]
    fn frost_shock_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("frost_shock", 10, &[Augment::Precise, Augment::FrozenPower])],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        let e = w.enemy(id).unwrap();
        assert!(e.status.fx.root > 0.0 && e.status.chill == 0.0);
        let ei = w.enemies.iter().position(|e| e.id == id).unwrap();
        let s = w.spec_of(0, "lightning_bolt").unwrap();
        assert!((w.shaman_scale(0, ei, &s) - 1.3).abs() < 1e-4);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "frost_shock",
                10,
                &[Augment::Precise, Augment::FrostNovaShock],
            )],
        );
        foe(&mut w, 5.0, 0.0, 1e6);
        let near = foe(&mut w, 5.0, 4.0, 1e6);
        let at = w.enemies[0].pos;
        w.enemies[1].pos = at + V2::new(0.0, 4.0);
        w.step();
        assert!(w.enemy(near).unwrap().status.chill > 0.0);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("frost_shock", 10, &[Augment::Precise, Augment::Icefury])],
        );
        w.heroes[0].stats.add(Stat::Crit, 1.0);
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        w.step();
        assert!(w.enemy(id).unwrap().status.freeze > 1.0);
    }

    #[test]
    fn searing_totem_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "searing_totem",
                10,
                &[Augment::Frenzy, Augment::SearingBurn],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        run(&mut w, 1.5);
        assert!(w.enemy(id).unwrap().status.burning());

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[
                (
                    "searing_totem",
                    10,
                    &[Augment::Frenzy, Augment::SearingPact],
                ),
                ("flame_shock", 1, &[]),
            ],
        );
        let plain = w.spec_of(0, "flame_shock").unwrap().damage;
        foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        assert!((w.spec_of(0, "flame_shock").unwrap().damage / plain - 1.2).abs() < 1e-4);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("searing_totem", 10, &[Augment::Frenzy, Augment::TwinFires])],
        );
        let a = foe(&mut w, 8.0, 0.0, 1e6);
        let b = foe(&mut w, 9.0, 0.0, 1e6);
        run(&mut w, 1.5);
        assert!(w.enemy(a).unwrap().hp < 1e6 && w.enemy(b).unwrap().hp < 1e6);
    }

    #[test]
    fn magma_totem_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("magma_totem", 10, &[Augment::Wide, Augment::FireNova])],
        );
        let id = foe(&mut w, 15.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        let at = w.grounds[0].pos;
        w.enemies[0].pos = at + V2::new(7.0, 0.0);
        run(&mut w, 21.0);
        let s = w.spec_of(0, "magma_totem").unwrap();
        assert!(
            1e6 - w.enemy(id).unwrap().hp >= s.damage * 3.0,
            "no fire nova"
        );

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("magma_totem", 10, &[Augment::Wide, Augment::LiquidMagma])],
        );
        foe(&mut w, 3.0, 0.0, 1e6);
        let far = foe(&mut w, 18.0, 0.0, 1e6);
        run(&mut w, 3.0);
        assert!(w.enemy(far).unwrap().hp < 1e6, "no magma glob");

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("magma_totem", 10, &[Augment::Wide, Augment::MoltenEarth])],
        );
        let id = foe(&mut w, 3.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.burning());
    }

    #[test]
    fn earthbind_totem_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("earthbind_totem", 10, &[Augment::Wide, Augment::Earthgrab])],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.fx.root > 0.0);
        run(&mut w, 2.5);
        assert_eq!(
            w.enemy(id).unwrap().status.fx.root,
            0.0,
            "it was rooted twice"
        );

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "earthbind_totem",
                10,
                &[Augment::Wide, Augment::EarthquakeTotem],
            )],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.2);
        let e = w.enemy(id).unwrap();
        assert!(e.status.stun > 0.0 && e.status.chill == 0.0 && e.hp < 1e6);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("earthbind_totem", 10, &[Augment::Wide, Augment::Stoneclaw])],
        );
        let id = foe(&mut w, 5.0, 0.0, 1e6);
        run(&mut w, 0.2);
        assert!(w.enemy(id).unwrap().status.fx.weaken > 0.0);
    }

    #[test]
    fn healing_stream_capstones() {
        let heal = |tide: bool| {
            let mut w = hero_of("shaman");
            let augments: &[Augment] = if tide {
                &[Augment::Frenzy, Augment::HealingTide]
            } else {
                &[Augment::Frenzy]
            };
            spells(&mut w, &[("healing_stream_totem", 10, augments)]);
            w.heroes[0].max_hp = 1000.0;
            w.heroes[0].hp = 100.0;
            foe(&mut w, 10.0, 0.0, 1e6);
            run(&mut w, 0.1);
            w.heroes[0].hp - 100.0
        };
        assert!((heal(true) / heal(false) - 2.0).abs() < 1e-3);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "healing_stream_totem",
                10,
                &[Augment::Frenzy, Augment::Cloudburst],
            )],
        );
        let id = foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.heroes[0].max_hp = 1000.0;
        for _ in 0..20 {
            w.heroes[0].hp = 100.0;
            run(&mut w, 1.0);
        }
        run(&mut w, 1.0);
        assert!(1e6 - w.enemy(id).unwrap().hp > 200.0, "no cloudburst");

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "healing_stream_totem",
                10,
                &[Augment::Frenzy, Augment::Stoneskin],
            )],
        );
        foe(&mut w, 8.0, 0.0, 1e6);
        w.step();
        assert!((w.shaman_guard(0) - 0.85).abs() < 1e-4);
    }

    #[test]
    fn lightning_shield_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "lightning_shield",
                10,
                &[Augment::Hasted, Augment::Thunderstorm],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        let near = foe(&mut w, -3.0, 0.0, 1e6);
        w.step();
        w.heroes[0].abilities[0].cd = 99.0;
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(near).unwrap().hp < 1e6, "no thunderstorm");

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "lightning_shield",
                10,
                &[Augment::Hasted, Augment::StaticShield],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.enemy(id).unwrap().status.chill > 0.0);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "lightning_shield",
                10,
                &[Augment::Hasted, Augment::StaticCharge],
            )],
        );
        let id = foe(&mut w, 1.0, 0.0, 1e6);
        w.step();
        w.hurt_hero_from(0, 5.0, Some(id));
        assert!(w.heroes[0].spell.fx.boon(Augment::StaticCharge) > 0.04);
    }

    #[test]
    fn healing_wave_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "healing_wave",
                10,
                &[Augment::Hasted, Augment::AncestralFortitude],
            )],
        );
        w.step();
        w.heroes[0].hp = w.heroes[0].max_hp * 0.3;
        w.step();
        assert!(w.heroes[0].spell.fx.guard() >= 0.2);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[(
                "healing_wave",
                10,
                &[Augment::Hasted, Augment::NaturesSwiftness],
            )],
        );
        let wave = ability(&w, "healing_wave");
        for _ in 0..3 {
            ready(&mut w, 0);
            w.step();
        }
        w.heroes[0].abilities[0].cd = 99.0;
        w.heroes[0].max_hp = 1000.0;
        w.heroes[0].hp = 100.0;
        w.step();
        assert_eq!(w.heroes[0].spell.fx.charges(wave), 0);
        assert!(w.heroes[0].spell.fx.cd_rate() >= 1.0);

        let w = hero_of("shaman");
        let plain = w.spec_of(0, "healing_wave").unwrap();
        let mut s = plain.clone();
        Augment::TidalFocus.shape(&mut s);
        assert!((s.cooldown / plain.cooldown - 0.7).abs() < 1e-4);
    }

    #[test]
    fn stormstrike_capstones() {
        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[("stormstrike", 10, &[Augment::Precise, Augment::Windfury])],
        );
        foe(&mut w, 2.0, 0.0, 1e7);
        let shield = ability(&w, "lightning_shield");
        for _ in 0..40 {
            ready(&mut w, 0);
            w.step();
        }
        assert!(w.heroes[0].spell.fx.charges(shield) > 0, "no windfury");

        let w = hero_of("shaman");
        let mut s = w.spec_of(0, "stormstrike").unwrap();
        Augment::Stormlash.shape(&mut s);
        assert_eq!(s.fx.mark, 0.3);

        let mut w = hero_of("shaman");
        spells(
            &mut w,
            &[
                (
                    "stormstrike",
                    10,
                    &[Augment::Precise, Augment::MaelstromWeapon],
                ),
                ("lightning_bolt", 1, &[]),
            ],
        );
        w.heroes[0].abilities[1].cd = 99.0;
        w.heroes[0].abilities[0].casts = 4;
        let plain = w.spec_of(0, "lightning_bolt").unwrap().damage;
        foe(&mut w, 2.0, 0.0, 1e6);
        w.step();
        assert!((w.spec_of(0, "lightning_bolt").unwrap().damage / plain - 2.0).abs() < 1e-4);
    }

    #[test]
    fn every_shaman_spell_has_three_capstones() {
        let w = hero_of("shaman");
        let class = &w.content.classes[w.content.class("shaman").unwrap()];
        assert_eq!(class.pool.len(), 12);
        for key in &class.pool {
            assert_eq!(Augment::capstones_of(key).len(), 3, "{key}");
        }
    }
}
